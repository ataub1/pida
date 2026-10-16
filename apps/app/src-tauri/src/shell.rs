//! Verbindung zur Kern-Fassade `pida-core-api` (eine Instanz je Prozess).
//!
//! Die Shell hält **keinen Fachzustand**: Sie startet den Kern, reicht [`UiEvent`]s durch, leitet den
//! [`ViewState`] an das Fenster weiter und bedient das Custom Protocol. Kann der Kern nicht starten
//! (Fehler oder – solange `core-api` in Welle 2 noch Stub ist – Panik), zeigt die Oberfläche einen
//! Fehlerschritt `ng-ui-core-unavailable` mit „Wiederholen“, statt die App zu beenden. Dokumente, die
//! in dieser Zeit eintreffen („Öffnen mit“, zweite Instanz), werden vorgemerkt und nach einem
//! erfolgreichen Start übergeben.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pida_core_api::{Core, CoreConfig, CoreOptions, MainBarView, ShellHooks, StepView, UiEvent, ViewState};
use pida_i18n::Lang;
use pida_workflow::{MainBarStep, Step};

use crate::protocol::{self, RenderRequest};

/// Stabiler Fehlercode: Kern nicht verfügbar (i18n-ID).
pub const CORE_UNAVAILABLE: &str = "ng-ui-core-unavailable";
/// Stabiler Fehlercode: Ereignis ungültig (i18n-ID).
pub const EVENT_INVALID: &str = "ng-ui-event-invalid";
/// Höchstzahl vorgemerkter Dokumente ohne laufenden Kern (Schutz vor unbegrenztem Wachstum).
pub const MAX_PENDING_DOCUMENTS: usize = 1000;

/// Zustand der Shell (Tauri-State).
pub struct Shell {
    /// `None`: Konfiguration nicht ermittelbar (z. B. kein Benutzerordner) – nur Fehlerschritt.
    config: Option<CoreConfig>,
    /// Optionen des Kerns (Shell-Funktionen, Systemsprache; Logging richtet die Shell ein)
    options: Mutex<CoreOptions>,
    /// Start gesperrt (ungültige Startargumente): nur der Fehlerschritt, „Abbrechen“ beendet
    blocked: bool,
    core: Mutex<Option<Arc<Core>>>,
    fallback: Mutex<ViewState>,
    pending: Mutex<Vec<PathBuf>>,
    quitting: AtomicBool,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("core_available", &self.core().is_some())
            .field("pending", &lock(&self.pending).len())
            .finish_non_exhaustive()
    }
}

/// Ergebnis eines Startversuchs.
#[derive(Debug)]
pub enum Started {
    /// Kern läuft (Abo für den ViewState anlegen).
    Running(Arc<Core>),
    /// Kern nicht verfügbar (Fehlerschritt wird angezeigt).
    Unavailable,
}

/// Ergebnis von [`Shell::request_quit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitRequest {
    /// Kein Kern: sofort beenden.
    NoCore,
    /// `UiEvent::Quit` gesendet – der Kern beendet über `Finished` (Einstellungen sichern, F-73/F-105).
    Requested,
    /// Bereits angefordert; weiter auf den Kern warten.
    AlreadyRequested,
}

impl Shell {
    /// Shell ohne laufenden Kern (Start mit [`Shell::start`]).
    pub fn new(config: CoreConfig) -> Shell {
        Shell::with(Some(config), unavailable_view("", Lang::system()), false)
    }

    /// Shell ohne Konfiguration: Der Kern kann nicht starten, die Oberfläche zeigt den Fehlerschritt
    /// mit `detail` (statt die App beim Start mit einer Panik zu beenden).
    pub fn without_config(detail: &str) -> Shell {
        Shell::with(None, unavailable_view(detail, Lang::system()), false)
    }

    /// Ungültige Startargumente (PDF-Over: Fehlerdialog `error.CmdLineArgs`/`error.FileNotExist`, dann
    /// Ende): Fehlerschritt `code` ohne „Wiederholen“, der Kern startet nicht.
    pub fn start_error(config: Option<CoreConfig>, code: &str, params: BTreeMap<String, String>) -> Shell {
        Shell::with(config, error_view(code, params, false, Lang::system()), true)
    }

    fn with(config: Option<CoreConfig>, fallback: ViewState, blocked: bool) -> Shell {
        // Logging richtet die Shell ein (`pida_core_api::logging`), nicht der Kern.
        let options = CoreOptions { init_logging: false, ..CoreOptions::default() };
        Shell {
            config,
            options: Mutex::new(options),
            blocked,
            core: Mutex::new(None),
            fallback: Mutex::new(fallback),
            pending: Mutex::new(Vec::new()),
            quitting: AtomicBool::new(false),
        }
    }

    /// Optionen für den nächsten Kernstart (Tests, Einbettung).
    pub fn set_options(&self, options: CoreOptions) {
        *lock(&self.options) = options;
    }

    /// Optionen für den nächsten Kernstart anpassen (Mobile: sicherer Speicher, App-Dokumente).
    pub fn update_options(&self, f: impl FnOnce(&mut CoreOptions)) {
        f(&mut lock(&self.options));
    }

    /// Sendet ein Ereignis aus der Shell selbst (Lebenszyklus); ohne Kern wird es verworfen.
    pub fn dispatch(&self, event: UiEvent) {
        if let Some(core) = self.core() {
            let _ = catch_unwind(AssertUnwindSafe(|| core.dispatch(event)));
        }
    }

    /// Systemsprache aus dem Betriebssystem (`sys-locale`) für den Kern (`CoreOptions::system_language`).
    pub fn set_system_language(&self, lang: Option<Lang>) {
        lock(&self.options).system_language = lang;
    }

    /// Shell-Funktionen (Öffnen von Dateien/URLs, Elternfenster) – für den laufenden Kern und jeden
    /// weiteren Start.
    pub fn set_hooks(&self, hooks: Arc<dyn ShellHooks>) {
        lock(&self.options).shell = Some(Arc::clone(&hooks));
        if let Some(core) = self.core() {
            core.set_shell(Some(hooks));
        }
    }

    /// Startet den Kern; Fehler und Paniken werden als Fehlerschritt abgebildet. Vorgemerkte
    /// Dokumente gehen nach einem erfolgreichen Start an den Kern.
    pub fn start(&self) -> Started {
        if self.blocked {
            return Started::Unavailable;
        }
        let Some(config) = self.config.clone() else {
            tracing::error!("Kern ohne Konfiguration nicht startbar");
            return Started::Unavailable;
        };
        let options = lock(&self.options).clone();
        let result = catch_unwind(AssertUnwindSafe(move || Core::new_with(config, options)));
        match result {
            Ok(Ok(core)) => {
                let core = Arc::new(core);
                *lock(&self.core) = Some(Arc::clone(&core));
                let pending = std::mem::take(&mut *lock(&self.pending));
                self.open_files(pending);
                Started::Running(core)
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "Kern konnte nicht starten");
                self.mark_unavailable(&e.to_string());
                Started::Unavailable
            }
            Err(panic) => {
                // Regel 9: Panik-Texte können beliebige Daten tragen – im Log nur die Tatsache.
                tracing::error!("Kern-Start abgebrochen (Panik)");
                self.mark_unavailable(&panic_message(&panic));
                Started::Unavailable
            }
        }
    }

    /// Verwirft den Kern (z. B. Panik beim Abonnieren) und zeigt den Fehlerschritt mit `detail`.
    pub fn mark_unavailable(&self, detail: &str) {
        *lock(&self.core) = None;
        *lock(&self.fallback) = unavailable_view(detail, Lang::system());
    }

    /// Der Kern-Task hat ohne `Finished` geendet: Kern verwerfen, Fehlerschritt `code` (eigene
    /// Meldungs-ID, z. B. `ng-core-stopped`) mit „Wiederholen“ (Neustart) in der zuletzt gezeigten
    /// Sprache.
    pub fn mark_stopped(&self, code: &str) {
        let language = self.core().map_or_else(Lang::system, |core| {
            catch_unwind(AssertUnwindSafe(|| core.view().language)).unwrap_or_else(|_| Lang::system())
        });
        *lock(&self.core) = None;
        *lock(&self.fallback) = error_view(code, BTreeMap::new(), true, language);
    }

    /// Laufender Kern.
    pub fn core(&self) -> Option<Arc<Core>> {
        lock(&self.core).clone()
    }

    /// Aktueller ViewState (Kern oder Fehlerschritt).
    pub fn view(&self) -> ViewState {
        match self.core() {
            Some(core) => catch_unwind(AssertUnwindSafe(|| core.view()))
                .unwrap_or_else(|p| unavailable_view(&panic_message(&p), Lang::system())),
            None => lock(&self.fallback).clone(),
        }
    }

    /// Prüft und sendet ein Ereignis aus der Oberfläche. Ohne Kern ist nur „Wiederholen“ (Neustart des
    /// Kerns) bzw. „Abbrechen“/„Beenden“ (Programmende) sinnvoll; alles andere liefert
    /// [`CORE_UNAVAILABLE`].
    pub fn dispatch_json(&self, event: serde_json::Value) -> Result<DispatchOutcome, &'static str> {
        let event: UiEvent = serde_json::from_value(event).map_err(|_| EVENT_INVALID)?;
        match self.core() {
            Some(core) => {
                catch_unwind(AssertUnwindSafe(|| core.dispatch(event))).map_err(|_| CORE_UNAVAILABLE)?;
                Ok(DispatchOutcome::Dispatched)
            }
            None => match event {
                UiEvent::Retry if !self.blocked => Ok(DispatchOutcome::Restart),
                UiEvent::Quit | UiEvent::Cancel => Ok(DispatchOutcome::Quit),
                _ => Err(CORE_UNAVAILABLE),
            },
        }
    }

    /// Dokumente aus „Öffnen mit“, Drop auf das Dock-Icon, zweiter Instanz. Ohne Kern werden sie
    /// (ohne Duplikate, begrenzt auf [`MAX_PENDING_DOCUMENTS`]) für den nächsten Start vorgemerkt.
    pub fn open_files(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        if let Some(core) = self.core() {
            let _ = catch_unwind(AssertUnwindSafe(|| core.dispatch(UiEvent::OpenFiles { paths, mode: None })));
        } else {
            let mut pending = lock(&self.pending);
            for path in paths {
                if pending.len() >= MAX_PENDING_DOCUMENTS {
                    tracing::warn!("Zu viele Dokumente ohne laufenden Kern; weitere verworfen");
                    break;
                }
                if !pending.contains(&path) {
                    pending.push(path);
                }
            }
            tracing::info!(count = pending.len(), "Dokumente für den Kernstart vorgemerkt");
        }
    }

    /// Anzahl vorgemerkter Dokumente (ohne laufenden Kern).
    pub fn pending_documents(&self) -> usize {
        lock(&self.pending).len()
    }

    /// Beenden über den Kern (Fenster schließen, ⌘Q): Der Kern bekommt `UiEvent::Quit` (Abbruch
    /// eines laufenden Dialogs, F-73; Sichern der Einstellungen, F-105) und meldet `Finished`.
    pub fn request_quit(&self) -> QuitRequest {
        let Some(core) = self.core() else {
            return QuitRequest::NoCore;
        };
        if self.quitting.swap(true, Ordering::SeqCst) {
            return QuitRequest::AlreadyRequested;
        }
        match catch_unwind(AssertUnwindSafe(|| core.dispatch(UiEvent::Quit))) {
            Ok(()) => QuitRequest::Requested,
            Err(_) => QuitRequest::NoCore,
        }
    }

    /// Bedient eine `pida-render`-Anfrage.
    pub fn render(&self, path: &str, query: Option<&str>) -> tauri::http::Response<Vec<u8>> {
        let request = match protocol::parse(path, query) {
            Ok(r) => r,
            Err(protocol::RenderUriError::NotFound) => return protocol::error(404, "not-found"),
            Err(protocol::RenderUriError::BadRequest(what)) => return protocol::error(400, what),
        };
        let Some(core) = self.core() else {
            return protocol::error(503, CORE_UNAVAILABLE);
        };
        let result = catch_unwind(AssertUnwindSafe(|| match request {
            RenderRequest::Page { doc, page, scale } => core.page_tile(doc, page, scale),
            RenderRequest::Block { scale } => core.block_preview(scale),
            RenderRequest::Thumb { doc } => core.thumbnail(doc),
        }));
        match result {
            Ok(Ok(png)) => protocol::png(png),
            Ok(Err(pida_core_api::CoreError::UnknownDocument(_))) => protocol::error(404, "unknown-document"),
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Rendern fehlgeschlagen");
                protocol::error(500, "render-failed")
            }
            Err(_) => protocol::error(500, "render-failed"),
        }
    }

    /// Ergebnisdatei des aktuellen Ergebnis-Schritts (nur diese darf die Oberfläche im Ordner zeigen).
    pub fn output_path(&self) -> Option<PathBuf> {
        match self.view().step {
            StepView::Output { result, .. } => result.path,
            _ => None,
        }
    }

    /// Ausgabe des Dokuments `doc` für „Im Ordner zeigen“ (ADR 0004 §3.14) – nur Pfade, die der Kern
    /// geschrieben hat: das gespeicherte Ergebnis des aktuellen Schritts bzw. ein signiertes Dokument der
    /// Ergebnisliste (`done`). Ohne `doc` wie [`Shell::output_path`].
    pub fn output_path_for(&self, doc: Option<pida_workflow::DocId>) -> Option<PathBuf> {
        let Some(doc) = doc else { return self.output_path() };
        let view = self.view();
        if let StepView::Output { result, .. } = &view.step
            && view.document.as_ref().is_some_and(|d| d.doc.id == doc)
            && let Some(path) = &result.path
        {
            return Some(path.clone());
        }
        let done = view.done.iter().find(|d| d.id == doc).and_then(|d| match &d.result {
            pida_workflow::DocResult::Signed { output } => Some(output.path.clone()),
            _ => None,
        });
        // Stapel: Status `Signed` (vom Kern geschrieben)
        done.or_else(|| {
            view.batch.as_ref()?.items.iter().find(|i| i.doc == doc).and_then(|i| match &i.status {
                pida_core_api::BatchItemStatusView::Signed { path } => Some(path.clone()),
                _ => None,
            })
        })
    }

    /// Log-Ordner (Über-Dialog „Logs anzeigen“); ohne Konfiguration keiner.
    pub fn log_dir(&self) -> Option<PathBuf> {
        self.config.as_ref().map(|c| c.paths.log_dir.clone())
    }
}

/// Oberfläche „Arbeitsbereich“ für die Abnahme vor dem Merge (ADR 0004 §3.14): `PIDA_UI=workspace` (bzw.
/// `workspace,batch`) schaltet in **Debug-Builds** `CoreOptions::workspace_ui` ein; in Release-Builds ohne
/// Wirkung (dort setzt der Merge-Commit von A1 den Schalter). Liefert `None`, wenn nichts zu setzen ist.
pub fn workspace_ui_from_env(value: Option<&str>, debug_build: bool) -> Option<bool> {
    if !debug_build {
        return None;
    }
    let value = value?;
    value.split(',').map(str::trim).any(|p| p.eq_ignore_ascii_case("workspace")).then_some(true)
}

/// Stapelübersicht für die Abnahme vor dem Merge von A2 (ADR 0004 §3.14): `PIDA_UI=workspace,batch` schaltet
/// in **Debug-Builds** zusätzlich `CoreOptions::batch_overview` ein (nur zusammen mit `workspace`); in
/// Release-Builds ohne Wirkung. Liefert `None`, wenn nichts zu setzen ist.
pub fn batch_overview_from_env(value: Option<&str>, debug_build: bool) -> Option<bool> {
    workspace_ui_from_env(value, debug_build)?;
    value?.split(',').map(str::trim).any(|p| p.eq_ignore_ascii_case("batch")).then_some(true)
}

/// Ergebnis von [`Shell::dispatch_json`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// An den Kern übergeben.
    Dispatched,
    /// Kern neu starten (Wiederholen im Fehlerschritt).
    Restart,
    /// App beenden.
    Quit,
}

/// Fehlerschritt „Kern nicht verfügbar“ mit gesperrter Hauptleiste.
pub fn unavailable_view(detail: &str, language: Lang) -> ViewState {
    let mut params = BTreeMap::new();
    if !detail.is_empty() {
        params.insert("detail".to_owned(), detail.to_owned());
    }
    error_view(CORE_UNAVAILABLE, params, true, language)
}

/// Fehlerschritt ohne Kern (gesperrte Hauptleiste, keine Einstellungen).
pub fn error_view(code: &str, params: BTreeMap<String, String>, retry: bool, language: Lang) -> ViewState {
    ViewState {
        step: StepView::Error {
            code: code.to_owned(),
            params,
            retry,
            context: pida_core_api::ErrorContextView::default(),
        },
        main_bar: MainBarView {
            steps: [Step::Open, Step::Positioning, Step::Signing, Step::Finish]
                .into_iter()
                .map(|step| MainBarStep { step, active: false, enabled: false })
                .collect(),
            settings_enabled: false,
        },
        settings: None,
        language,
        notices: Vec::new(),
        migration: None,
        done: Vec::new(),
        platform: pida_core_api::PlatformView::default(),
        resume: None,
        keystore_import: None,
        privacy_text: None,
        // ohne Kern: nichts freigegeben, keine Inhalte der neuen Oberfläche (ADR 0004)
        document: None,
        signers: pida_core_api::SignersView::default(),
        batch: None,
        recent: None,
        first_run: None,
        actions: pida_core_api::ActionsView::default(),
        parked: None,
    }
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_owned()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_view_blocks_navigation_and_offers_retry() {
        let v = unavailable_view("not yet implemented: Integration", Lang::En);
        match &v.step {
            StepView::Error { code, params, retry, .. } => {
                assert_eq!(code, CORE_UNAVAILABLE);
                assert_eq!(params.get("detail").map(String::as_str), Some("not yet implemented: Integration"));
                assert!(retry);
            }
            other => panic!("unerwartet: {other:?}"),
        }
        assert_eq!(v.main_bar.steps.len(), 4);
        assert!(v.main_bar.steps.iter().all(|s| !s.enabled && !s.active));
        assert!(!v.main_bar.settings_enabled);
        assert_eq!(v.language, Lang::En);
        let json = serde_json::to_value(&v).expect("json");
        assert_eq!(json["step"]["step"], "error");
        assert_eq!(json["language"], "en");
    }

    fn test_shell() -> (Shell, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "pida-app-shell-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let config = CoreConfig {
            paths: pida_config::Paths::under(&root),
            overlay: pida_config::Overlay::default(),
            platform: pida_backend_api::Platform::current(),
            pdfium_dir: None,
            headless: false,
            fido2: None,
            app_version: "0.0.0-test".into(),
        };
        let shell = Shell::new(config);
        // ohne Systemrichtlinie, Altordner, PDFium, TCP-Probe und Schlüsselbund (Tests)
        shell.set_options(CoreOptions::isolated());
        (shell, root)
    }

    /// Der Start darf nie panisch abbrechen – weder mit dem Stub von `core-api` (Welle 2) noch mit
    /// einem Kern, der ohne PDFium startet; ohne Kern gibt es einen gültigen Fehlerschritt.
    #[test]
    fn start_never_panics_and_always_yields_a_view() {
        let (shell, root) = test_shell();
        let started = shell.start();
        let view = shell.view();
        match started {
            Started::Unavailable => {
                assert!(shell.core().is_none());
                assert!(matches!(&view.step, StepView::Error { code, retry: true, .. } if code == CORE_UNAVAILABLE));
                // ohne Kern: nur Wiederholen/Beenden
                assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "retry" })), Ok(DispatchOutcome::Restart));
                assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "quit" })), Ok(DispatchOutcome::Quit));
                // „Abbrechen“ im Fehlerschritt ohne Kern beendet das Programm
                assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "cancel" })), Ok(DispatchOutcome::Quit));
                assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "open_settings" })), Err(CORE_UNAVAILABLE));
                assert_eq!(shell.request_quit(), QuitRequest::NoCore);
                assert_eq!(shell.render("/block", Some("scale=1")).status(), 503);
                assert_eq!(shell.output_path(), None);
            }
            Started::Running(_) => assert!(shell.core().is_some()),
        }
        assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "gibt_es_nicht" })), Err(EVENT_INVALID));
        assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "move_block", "page": "x" })), Err(EVENT_INVALID));
        assert_eq!(shell.render("/../x", None).status(), 404);
        assert_eq!(shell.render("/page/a/0", None).status(), 400);
        let _ = std::fs::remove_dir_all(root);
    }

    /// `PIDA_UI` nur in Debug-Builds (ADR 0004 §3.14).
    #[test]
    fn workspace_ui_switch_from_the_environment() {
        assert_eq!(workspace_ui_from_env(Some("workspace"), true), Some(true));
        assert_eq!(workspace_ui_from_env(Some("workspace,batch"), true), Some(true));
        assert_eq!(workspace_ui_from_env(Some(" Workspace "), true), Some(true));
        assert_eq!(workspace_ui_from_env(Some("classic"), true), None);
        assert_eq!(workspace_ui_from_env(None, true), None);
        assert_eq!(workspace_ui_from_env(Some("workspace"), false), None, "Release: ohne Wirkung");
        // Stapelübersicht (K2): nur mit `workspace`, nur Debug
        assert_eq!(batch_overview_from_env(Some("workspace,batch"), true), Some(true));
        assert_eq!(batch_overview_from_env(Some(" workspace , Batch "), true), Some(true));
        assert_eq!(batch_overview_from_env(Some("workspace"), true), None);
        assert_eq!(batch_overview_from_env(Some("batch"), true), None, "nur zusammen mit workspace");
        assert_eq!(batch_overview_from_env(Some("workspace,batch"), false), None, "Release: ohne Wirkung");
        assert_eq!(batch_overview_from_env(None, true), None);
    }

    /// „Im Ordner zeigen“ mit Dokument: nur Pfade, die der Kern geschrieben hat.
    #[test]
    fn reveal_output_only_for_written_documents() {
        let (shell, root) = test_shell();
        assert_eq!(shell.output_path_for(None), None);
        assert_eq!(shell.output_path_for(Some(pida_workflow::DocId(1))), None, "ohne Kern bzw. Ergebnis nichts");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Die Shell gibt Lebenszyklus-Ereignisse nur an einen laufenden Kern; Optionen gelten für den
    /// nächsten Start.
    #[test]
    fn lifecycle_events_and_option_updates() {
        let (shell, root) = test_shell();
        shell.dispatch(UiEvent::Backgrounded); // ohne Kern: verworfen, keine Panik
        shell.update_options(|o| o.default_output_dir = Some(root.join("Documents")));
        assert_eq!(lock(&shell.options).default_output_dir, Some(root.join("Documents")));
        let _ = shell.start();
        shell.dispatch(UiEvent::Backgrounded);
        shell.dispatch(UiEvent::Foregrounded);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn debug_has_no_details() {
        let (shell, _) = test_shell();
        shell.open_files(vec![PathBuf::from("/geheim/Vertrag.pdf")]);
        assert_eq!(format!("{shell:?}"), "Shell { core_available: false, pending: 1, .. }");
    }

    /// Ohne Kern eintreffende Dokumente gehen nicht verloren (Wiederholen nach Startfehler), ohne
    /// Duplikate und begrenzt.
    #[test]
    fn documents_without_core_are_kept_for_the_next_start() {
        let (shell, root) = test_shell();
        let a = PathBuf::from("/x/a.pdf");
        shell.open_files(vec![a.clone(), a.clone()]);
        shell.open_files(Vec::new());
        assert_eq!(shell.pending_documents(), 1);
        let many: Vec<PathBuf> =
            (0..MAX_PENDING_DOCUMENTS + 50).map(|i| PathBuf::from(format!("/x/{i}.pdf"))).collect();
        shell.open_files(many);
        assert_eq!(shell.pending_documents(), MAX_PENDING_DOCUMENTS);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Endet der Kern-Task ohne `Finished`, zeigt die Shell `ng-core-stopped` als eigenen Fehlerschritt
    /// (nicht als Detail von `ng-ui-core-unavailable`); „Wiederholen“ startet den Kern neu.
    #[test]
    fn stopped_core_yields_its_own_error_step_with_retry() {
        let (shell, root) = test_shell();
        let _ = shell.start();
        shell.mark_stopped(crate::CORE_STOPPED);
        assert!(shell.core().is_none());
        match shell.view().step {
            StepView::Error { code, params, retry, .. } => {
                assert_eq!(code, crate::CORE_STOPPED);
                assert!(params.is_empty());
                assert!(retry);
            }
            other => panic!("unerwartet: {other:?}"),
        }
        assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "retry" })), Ok(DispatchOutcome::Restart));
        let _ = std::fs::remove_dir_all(root);
    }

    /// Ohne ermittelbare Konfiguration startet die App trotzdem und zeigt den Fehlerschritt.
    #[test]
    fn missing_configuration_yields_error_view() {
        let shell = Shell::without_config("Konfigurationsordner nicht ermittelbar");
        assert!(matches!(shell.start(), Started::Unavailable));
        assert!(shell.log_dir().is_none());
        match shell.view().step {
            StepView::Error { code, params, retry, .. } => {
                assert_eq!(code, CORE_UNAVAILABLE);
                assert_eq!(params.get("detail").map(String::as_str), Some("Konfigurationsordner nicht ermittelbar"));
                assert!(retry);
            }
            other => panic!("unerwartet: {other:?}"),
        }
        assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "retry" })), Ok(DispatchOutcome::Restart));
    }
}
