//! Verbindung zur Kern-Fassade `pdfover-core-api` (eine Instanz je Prozess).
//!
//! Die Shell hält **keinen Fachzustand**: Sie startet den Kern, reicht [`UiEvent`]s durch, leitet den
//! [`ViewState`] an das Fenster weiter und bedient das Custom Protocol. Kann der Kern nicht starten
//! (Fehler oder – solange `core-api` in Welle 2 noch Stub ist – Panik), zeigt die Oberfläche einen
//! Fehlerschritt `ng-ui-core-unavailable` mit „Wiederholen“, statt die App zu beenden.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pdfover_core_api::{Core, CoreConfig, MainBarView, StepView, UiEvent, ViewState};
use pdfover_i18n::Lang;
use pdfover_workflow::{MainBarStep, Step};

use crate::protocol::{self, RenderRequest};

/// Stabiler Fehlercode: Kern nicht verfügbar (i18n-ID).
pub const CORE_UNAVAILABLE: &str = "ng-ui-core-unavailable";
/// Stabiler Fehlercode: Ereignis ungültig (i18n-ID).
pub const EVENT_INVALID: &str = "ng-ui-event-invalid";

/// Zustand der Shell (Tauri-State).
pub struct Shell {
    config: CoreConfig,
    core: Mutex<Option<Arc<Core>>>,
    fallback: Mutex<ViewState>,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell").field("core_available", &self.core().is_some()).finish_non_exhaustive()
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

impl Shell {
    /// Shell ohne laufenden Kern (Start mit [`Shell::start`]).
    pub fn new(config: CoreConfig) -> Shell {
        Shell { config, core: Mutex::new(None), fallback: Mutex::new(unavailable_view("", Lang::system())) }
    }

    /// Startet den Kern; Fehler und Paniken (Stub) werden als Fehlerschritt abgebildet.
    pub fn start(&self) -> Started {
        let config = self.config.clone();
        let result = catch_unwind(AssertUnwindSafe(move || Core::new(config)));
        match result {
            Ok(Ok(core)) => {
                let core = Arc::new(core);
                *lock(&self.core) = Some(Arc::clone(&core));
                Started::Running(core)
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "Kern konnte nicht starten");
                *lock(&self.fallback) = unavailable_view(&e.to_string(), Lang::system());
                Started::Unavailable
            }
            Err(panic) => {
                let detail = panic_message(&panic);
                tracing::error!(detail = %detail, "Kern-Start abgebrochen (Panik)");
                *lock(&self.fallback) = unavailable_view(&detail, Lang::system());
                Started::Unavailable
            }
        }
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
    /// Kerns) bzw. „Beenden“ sinnvoll; alles andere liefert [`CORE_UNAVAILABLE`].
    pub fn dispatch_json(&self, event: serde_json::Value) -> Result<DispatchOutcome, &'static str> {
        let event: UiEvent = serde_json::from_value(event).map_err(|_| EVENT_INVALID)?;
        match self.core() {
            Some(core) => {
                catch_unwind(AssertUnwindSafe(|| core.dispatch(event))).map_err(|_| CORE_UNAVAILABLE)?;
                Ok(DispatchOutcome::Dispatched)
            }
            None => match event {
                UiEvent::Retry => Ok(DispatchOutcome::Restart),
                UiEvent::Quit => Ok(DispatchOutcome::Quit),
                _ => Err(CORE_UNAVAILABLE),
            },
        }
    }

    /// Dokumente aus „Öffnen mit“, Drop auf das Dock-Icon, zweiter Instanz.
    pub fn open_files(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        if let Some(core) = self.core() {
            let _ = catch_unwind(AssertUnwindSafe(|| core.dispatch(UiEvent::OpenFiles { paths })));
        } else {
            tracing::warn!(count = paths.len(), "Dokumente ohne laufenden Kern verworfen");
        }
    }

    /// Bedient eine `pdfover-render`-Anfrage.
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
        }));
        match result {
            Ok(Ok(png)) => protocol::png(png),
            Ok(Err(pdfover_core_api::CoreError::UnknownDocument(_))) => protocol::error(404, "unknown-document"),
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

    /// Log-Ordner (Über-Dialog „Logs anzeigen“).
    pub fn log_dir(&self) -> PathBuf {
        self.config.paths.log_dir.clone()
    }
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
    ViewState {
        step: StepView::Error { code: CORE_UNAVAILABLE.to_owned(), params, retry: true },
        main_bar: MainBarView {
            steps: [Step::Open, Step::Positioning, Step::Signing, Step::Finish]
                .into_iter()
                .map(|step| MainBarStep { step, active: false, enabled: false })
                .collect(),
            settings_enabled: false,
        },
        settings: None,
        language,
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
            StepView::Error { code, params, retry } => {
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
            "pdfover-app-shell-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let config = CoreConfig {
            paths: pdfover_config::Paths::under(&root),
            overlay: pdfover_config::Overlay::default(),
            platform: pdfover_backend_api::Platform::current(),
            pdfium_dir: None,
            headless: false,
            fido2: None,
            app_version: "0.0.0-test".into(),
        };
        (Shell::new(config), root)
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
                assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "cancel" })), Err(CORE_UNAVAILABLE));
                assert_eq!(shell.render("/block", Some("scale=1")).status(), 503);
                assert_eq!(shell.output_path(), None);
            }
            Started::Running(_) => assert!(shell.core().is_some()),
        }
        assert_eq!(shell.dispatch_json(serde_json::json!({ "type": "choose_invisible" })), Err(EVENT_INVALID));
        assert_eq!(shell.render("/../x", None).status(), 404);
        assert_eq!(shell.render("/page/a/0", None).status(), 400);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn debug_has_no_details() {
        let (shell, _) = test_shell();
        assert_eq!(format!("{shell:?}"), "Shell { core_available: false, .. }");
    }
}
