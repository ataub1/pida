//! `pdfover-core-api` – Fassade des Kerns für Shell (Tauri 2), CLI und Plan-B-Shell
//! (uniffi-tauglich, ohne Tauri-Abhängigkeit; Plan §2.5, §3.4).
//!
//! Die UI hält **keinen Fachzustand**: Sie projiziert nur [`ViewState`] und sendet [`UiEvent`].
//! Beide Typen sind serde-serialisierbar (`tag`-basiert) für die Tauri-Bridge
//! (`tauri-specta`/`ts-rs` erzeugen daraus die TypeScript-Typen).
//!
//! # Aufbau
//!
//! - [`Core::new`] lädt die Einstellungen (inkl. `-c`/Altdateien, Migrationsangebot, Richtlinie),
//!   bindet PDFium, registriert die Backends (ID Austria Szenario A, lokale BKU mit
//!   Erreichbarkeits-Polling 5 s/30 s auf dem Desktop, PKCS#12-Keystore mit Schlüsselbund-Adapter)
//!   und startet den Kern-Task (Tokio).
//! - Der Kern-Task wendet den Reducer aus `pdfover-workflow` an und führt die Effekte aus: Dokument
//!   laden und analysieren, Platzhaltersuche, Seitenkacheln (LRU), Blockvorschau aus dem echten
//!   Appearance-Objekt, Signierplan, Backend-Sitzung mit [`InteractionView`]s als Kanal zur UI,
//!   `finish`, Selbstprüfung, Ausgabe.
//! - Fehler erreichen die UI als i18n-Code mit Parametern ([`StepView::Error`], [`NoticeView`]).
//!
//! Die PAdES-Engine wird über [`engine::PadesEngine`] angesprochen; Tests setzen über
//! [`CoreOptions::engine`] eine Fake-Engine ein.
//!
//! # Bridge-Vertrag (Shell ↔ Kern)
//!
//! | Schritt (`ViewState::step`) | Anzeige | Antworten der UI |
//! |---|---|---|
//! | `open` | Drop-Zone, Warteschlange | `open_files` (nicht vorhandene Pfade werden ignoriert, F-12) |
//! | `loading` | Dokument wird geladen/analysiert | `cancel`, `navigate{open}` |
//! | `warning` | Warnung der Analyse (`codes`, z. B. `pdf-usage-rights`) mit Abbruchmöglichkeit | `answer_warning{proceed}`, `cancel` |
//! | `placeholder` | „Platzhalter verwenden?“ (Felder bzw. QR) | `placeholder_answer{answer, field_index}` |
//! | `position` | Seite (`pdfover-render://` → [`Core::page_tile`]), Block ([`Core::block_preview`], `preview_revision`), `announce` | `move_block`, `nudge_block`, `change_page`, `toggle_new_page`, `auto_place`, `confirm_position`, `choose_invisible` |
//! | `backend` | Optionen; `privacy_notice` = Datenschutzhinweis | `choose_backend`, `acknowledge_privacy{accepted}` |
//! | `signing` | `interaction`: `waiting`/`form`/`secret`/`notice` | `submit_form`, `submit_secret`, `answer_notice`, `open_signature_data`, `cancel` |
//! | `output` | Ergebnis; `ask_overwrite` = Rückfrage, `save_dialog` = Speichern-Dialog öffnen, `save_pending` = Knopf „Speichern“ | `confirm_overwrite`, `save{path}` (`null` = Dialog abgebrochen bzw. Knopf), `open_result`, `share_result`/`export_result` (Mobile), `navigate` |
//! | `error` | `code` + `params` (i18n), `retry` | `retry`, `cancel` |
//! | `finished` | Exit-Code | – |
//!
//! Querschnitt: `notices` (Hinweise ohne Rückfrage, `dismiss_notice{id}`), `migration`
//! (`accept_migration{store_secrets}`/`decline_migration`), `settings` (Overlay: `open_settings`,
//! `save_settings{settings}` – unbekannte Keys unverändert zurückgeben –, `close_settings`,
//! `reset_settings`), `done` (Ergebnisliste), `main_bar` (Matrix docs/analyse/01 §3.5), `platform`
//! (Desktop/Mobile), `resume` („Signatur fortsetzen“: `resume_signature`/`discard_resume`),
//! `keystore_import` (Ergebnis von `import_keystore{path, password, remember}`, schließen mit
//! `dismiss_keystore_import`). Lebenszyklus: `backgrounded`/`foregrounded` (kein Abbruch).
//! Dateien öffnen bzw. URLs im Browser, Teilen und Exportieren übernimmt die Shell über [`ShellHooks`]
//! ([`Core::set_shell`]).
//!
//! # Mobile (Plan §3.5, §5.2; docs/integration/mobile.md)
//!
//! - **Lebenszyklus:** `UiEvent::Backgrounded`/`Foregrounded` sind kein Abbruch. Nach der Rückkehr
//!   beantwortet der Kern eine wartende App-/QR-Maske mit „weiter“ – das Backend lädt die A-Trust-Seite
//!   neu und startet den Long-Poll frisch (die alte Verbindung hat das System oft getrennt).
//! - **Persistenz:** mit [`CoreOptions::persist_prepared`] (Standard auf iOS/Android) sichert der Kern
//!   nach `prepare` den vorbereiteten Zustand in der Sandbox (`resume`-Modul, `pades::Prepared::persist`);
//!   nach einem App-Kill bietet er „Signatur fortsetzen“ an und stellt das Dokument mit
//!   `Prepared::restore` wieder her. `Effect::DeletePersisted` löscht ihn nach Erfolg/Abbruch/Fehler.
//! - **Ausgabe:** [`CoreOptions::default_output_dir`] (App-Dokumente) statt Speichern-Dialog;
//!   Teilen/Exportieren über [`ShellHooks::share_path`]/[`ShellHooks::export_path`].
//! - **Signiermittel:** keine lokale BKU; FIDO2 nur mit Hinweis `ng-fido2-unavailable-mobile`
//!   (`BackendOptionView::notes`); PKCS#12 nur nach Import in die Sandbox (`import_keystore`).
//! - **Secrets:** die Shell setzt [`CoreOptions::secret_store`] auf `pdfover_config::ShellSecretStore`
//!   (Keychain bzw. Android Keystore); das ID-Austria-Passwort wird nie persistiert.
//!
//! # Vertrag Kern ↔ Oberfläche (Welle 3)
//!
//! `view.rs` ist die **einzige Quelle**: die TypeScript-Typen der Oberfläche
//! (`apps/app/src/core/generated.ts`) entstehen daraus mit ts-rs (Modul `ts`, Feature `ts`); der Test
//! `tests/ts_bindings.rs` schlägt bei jeder Abweichung fehl. Entscheidungen zu den Vorschlägen der
//! Oberfläche (`extensions.ts`, entfernt):
//!
//! | Vorschlag der UI | Entscheidung |
//! |---|---|
//! | `OutputView.prompt` (`save_path`/`overwrite`) | ersetzt durch die vorhandenen Felder [`OutputView::save_dialog`] (Startordner + Namensvorschlag) und [`OutputView::ask_overwrite`]; Antworten `save{path}` bzw. `confirm_overwrite` |
//! | `ViewState.notification` (ein Toast mit `id`) | ersetzt durch [`ViewState::notices`] (Liste, älteste zuerst, schließen mit `dismiss_notice{id}`) – mehrere Hinweise gehen nicht verloren |
//! | `UiEvent::ChooseInvisible` | übernommen: [`UiEvent::ChooseInvisible`] → `Event::InvisibleChosen` → unsichtbare Signatur nur für dieses Dokument |
//! | Schlüssel von `SettingsView::problems` | TOML-Punktpfade der Felder (`proxy.host`, `output.folder`, `keystore.file`, `signature.emblem` …) |
//! | `Position.announce` | Schlüssel `page`, `pages`, `left` (linke Kante), `top` (Oberkante von unten, wie das Feld Y; mm, ganzzahlig) |
//! | Migrationsassistent | [`MigrationOfferView::plaintext_secrets`] ergänzt: die UI fragt nur dann nach der Übernahme der Passwörter in den Schlüsselbund |
//!
//! Ownership: **Integration**.

mod backends;
mod broker;
mod config;
mod docs;
pub mod engine;
pub mod logging;
pub mod messages;
pub mod plan;
mod resume;
mod runtime;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod tools;
#[cfg(feature = "ts")]
pub mod ts;
mod view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub use backends::{SecretStoreCache, password_storage};
pub use broker::ShellHooks;
pub use config::PRIVACY_TEXT_VERSION;
pub use view::{
    BackendOptionView, BlockView, DocInfo, DoneItemView, FieldInfo, FormAnswerView, InteractionView,
    KeystoreImportView, MainBarView, MigrationOfferView, NoticeView, OutputView, PlatformOs, PlatformView,
    PrivacyNoticeView, QrInfo, QueueItemView, ResumeOfferView, SaveDialogView, SettingsView, StepView, UiEvent,
    ViewState,
};

use pdfover_backend_api::{Platform, SignatureBackend};
use pdfover_config::{Overlay, Paths, Policy, SecretStore};
use pdfover_i18n::Lang;
use pdfover_render::RenderBackend;
use pdfover_workflow::{DocId, ExitCode};
use tokio::sync::{mpsc, watch};

use crate::runtime::{CoreTask, Msg, Shared};

/// Fehler der Fassade.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// Konfiguration
    #[error(transparent)]
    Config(#[from] pdfover_config::ConfigError),
    /// Renderer
    #[error(transparent)]
    Render(#[from] pdfover_render::RenderError),
    /// Dokument unbekannt
    #[error("Dokument {0:?} unbekannt")]
    UnknownDocument(DocId),
    /// Kern beendet
    #[error("Kern beendet")]
    Stopped,
    /// Keine Blockvorschau (unsichtbares Profil bzw. noch nicht gebaut)
    #[error("keine Blockvorschau verfügbar")]
    NoPreview,
    /// Tokio-Laufzeit nicht erstellbar
    #[error("Laufzeit: {0}")]
    Runtime(String),
    /// Prüfung (`verify`) nicht möglich (Datei nicht lesbar)
    #[error("Prüfung nicht möglich: {0}")]
    Verify(String),
}

impl CoreError {
    /// Stabile Meldungs-ID.
    pub fn message_id(&self) -> &'static str {
        match self {
            CoreError::Config(e) => e.message_id(),
            CoreError::Render(e) => e.code(),
            CoreError::UnknownDocument(_) => "ng-unknown-document",
            CoreError::Stopped => "ng-core-stopped",
            CoreError::NoPreview => "ng-no-preview",
            CoreError::Runtime(_) => "error-Unexpected",
            CoreError::Verify(_) => "ng-verify-failed",
        }
    }
}

/// Start-Konfiguration.
#[derive(Debug, Clone)]
pub struct CoreConfig {
    /// Pfade (Plattform oder Sandbox)
    pub paths: Paths,
    /// CLI-Overlay
    pub overlay: Overlay,
    /// Plattform
    pub platform: Platform,
    /// Ordner der PDFium-Bibliothek (`None` = neben der Binary/System)
    pub pdfium_dir: Option<PathBuf>,
    /// ohne UI (CLI `--headless`)
    pub headless: bool,
    /// Plattform-FIDO2 (nur Desktop; von der Shell bzw. aus `backend-atrust`-Features)
    pub fido2: Option<Arc<dyn pdfover_backend_atrust::Fido2Provider>>,
    /// App-Version (User-Agent der lokalen BKU, Über-Dialog)
    pub app_version: String,
}

/// Erweiterte Optionen (additiv zu [`CoreConfig`]; Shell, CLI und Tests).
#[derive(Clone)]
pub struct CoreOptions {
    /// Backends dürfen über den Broker fragen (Masken, Passwörter, Meldungen). `None` = `!headless`.
    /// Die CLI mit Terminal setzt `Some(true)` trotz `headless` (Terminal-Broker).
    pub interactive: Option<bool>,
    /// PAdES-Engine (Standard: `pdfover-pades`)
    pub engine: Option<Arc<dyn engine::PadesEngine>>,
    /// fest vorgegebene Backends statt der Registrierung (Tests, Einbettung)
    pub backends: Option<Vec<Arc<dyn SignatureBackend>>>,
    /// Secret-Speicher (Standard: Schlüsselbund der Plattform, sonst RAM)
    pub secret_store: Option<Arc<dyn SecretStore>>,
    /// Richtlinie der Shell (Registry, Managed Preferences) – hat Vorrang vor der Datei
    pub policy: Option<Policy>,
    /// Richtliniendatei des Systems lesen (`Policy::load_system`)
    pub load_system_policy: bool,
    /// Renderer (Standard: PDFium aus [`CoreConfig::pdfium_dir`])
    pub render: Option<Arc<dyn RenderBackend>>,
    /// PDFium binden, wenn kein Renderer vorgegeben ist
    pub use_pdfium: bool,
    /// Shell-Funktionen (Öffnen von Dateien/URLs)
    pub shell: Option<Arc<dyn ShellHooks>>,
    /// Systemsprache der Shell (`Lang::set_system`)
    pub system_language: Option<Lang>,
    /// Logdatei einrichten (mit Rotation; ein bereits gesetzter Subscriber bleibt)
    pub init_logging: bool,
    /// Log-Stufe
    pub log_level: String,
    /// Altordner `~/.pdf-over` (Migration, `-c`); `None` mit `detect_legacy_dir` = automatisch
    pub legacy_dir: Option<PathBuf>,
    /// Altordner automatisch suchen
    pub detect_legacy_dir: bool,
    /// lokale BKU per TCP-Probe überwachen (Desktop)
    pub probe_local_bku: bool,
    /// Budget des Kachel-Caches in Bytes
    pub tile_cache_bytes: usize,
    /// Vorbereiteten Zustand für „Signatur fortsetzen“ in der Sandbox sichern (Mobile-Lebenszyklus,
    /// Plan §3.5). `None` = nach Plattform (iOS/Android ja, Desktop nein).
    pub persist_prepared: Option<bool>,
    /// Ausgabeordner, wenn weder Overlay noch Einstellung einen nennen (Mobile: App-Dokumente, dann ohne
    /// Speichern-Dialog; Teilen/Exportieren im Ergebnis-Screen). `None` = Speichern-Dialog (Desktop).
    pub default_output_dir: Option<PathBuf>,
}

impl Default for CoreOptions {
    fn default() -> Self {
        Self {
            interactive: None,
            engine: None,
            backends: None,
            secret_store: None,
            policy: None,
            load_system_policy: true,
            render: None,
            use_pdfium: true,
            shell: None,
            system_language: None,
            init_logging: true,
            log_level: "info".into(),
            legacy_dir: None,
            detect_legacy_dir: true,
            probe_local_bku: true,
            tile_cache_bytes: 64 * 1024 * 1024,
            persist_prepared: None,
            default_output_dir: None,
        }
    }
}

impl CoreOptions {
    /// Für Tests: keine Systemrichtlinie, kein PDFium, kein Logging, kein Altordner, keine
    /// TCP-Probe, Secrets nur im RAM.
    pub fn isolated() -> CoreOptions {
        CoreOptions {
            load_system_policy: false,
            use_pdfium: false,
            init_logging: false,
            detect_legacy_dir: false,
            probe_local_bku: false,
            secret_store: Some(Arc::new(pdfover_config::MemorySecretStore::default())),
            ..CoreOptions::default()
        }
    }
}

impl std::fmt::Debug for CoreOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreOptions")
            .field("interactive", &self.interactive)
            .field("engine", &self.engine)
            .field("backends", &self.backends.as_ref().map(Vec::len))
            .field("secret_store", &self.secret_store)
            .field("policy", &self.policy)
            .field("render", &self.render.is_some())
            .field("use_pdfium", &self.use_pdfium)
            .field("shell", &self.shell.is_some())
            .field("init_logging", &self.init_logging)
            .field("persist_prepared", &self.persist_prepared)
            .finish_non_exhaustive()
    }
}

/// FIDO2-Authenticator der Plattform (nur mit den Features `fido2-windows`/`fido2-hid` von
/// `pdfover-backend-atrust`; sonst `None`, z. B. Mobile).
pub fn platform_fido2() -> Option<Arc<dyn pdfover_backend_atrust::Fido2Provider>> {
    pdfover_backend_atrust::fido2::platform_provider()
}

/// Der Kern (ein Prozess, eine Instanz; Single-Instance übernimmt die Shell).
pub struct Core {
    tx: mpsc::UnboundedSender<Msg>,
    view_rx: watch::Receiver<ViewState>,
    finished_rx: watch::Receiver<Option<ExitCode>>,
    shared: Arc<Shared>,
    runtime: Option<tokio::runtime::Runtime>,
    _log: Mutex<Option<logging::LogGuard>>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core").field("shared", &self.shared).field("own_runtime", &self.runtime.is_some()).finish()
    }
}

fn initial_view(lang: Lang, platform: Platform) -> ViewState {
    ViewState {
        step: StepView::Starting,
        main_bar: MainBarView { steps: pdfover_workflow::State::Starting.main_bar().to_vec(), settings_enabled: false },
        settings: None,
        language: lang,
        notices: Vec::new(),
        migration: None,
        done: Vec::new(),
        platform: PlatformView::from(platform),
        resume: None,
        keystore_import: None,
    }
}

impl Core {
    /// Startet den Kern (Einstellungen laden, Richtlinie, Backends registrieren, Reducer-Task).
    pub fn new(config: CoreConfig) -> Result<Core, CoreError> {
        Core::new_with(config, CoreOptions::default())
    }

    /// Wie [`Core::new`] mit erweiterten Optionen.
    pub fn new_with(config: CoreConfig, options: CoreOptions) -> Result<Core, CoreError> {
        let (runtime, handle) = match tokio::runtime::Handle::try_current() {
            Ok(h) => (None, h),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("pdfover-core")
                    .build()
                    .map_err(|e| CoreError::Runtime(e.to_string()))?;
                let h = rt.handle().clone();
                (Some(rt), h)
            }
        };
        let _enter = handle.enter();
        if let Some(lang) = options.system_language {
            Lang::set_system(Some(lang));
        }
        let log = options.init_logging.then(|| {
            logging::init(
                &config.paths,
                &logging::LogOptions { level: options.log_level.clone(), stderr: false, file: true },
            )
        });
        config.paths.ensure()?;
        tracing::info!(version = %config.app_version, platform = ?config.platform, headless = config.headless, "Kern startet");

        let policy = match options.policy.clone() {
            Some(p) => Some(p),
            None if options.load_system_policy => match Policy::load_system() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(id = e.message_id(), "Richtlinie nicht lesbar – ignoriert");
                    None
                }
            },
            None => None,
        };
        let secrets = options.secret_store.clone().unwrap_or_else(pdfover_config::platform_secret_store);
        let legacy_dir =
            options.legacy_dir.clone().or_else(|| if options.detect_legacy_dir { Paths::legacy_dir() } else { None });
        let (cfg, startup) = config::ConfigState::load(
            config.paths.clone(),
            config.overlay.clone(),
            policy,
            secrets,
            legacy_dir,
            !config.headless,
        )?;

        let render: Option<Arc<dyn RenderBackend>> = match options.render.clone() {
            Some(r) => Some(r),
            None if options.use_pdfium => match pdfover_render::PdfiumBackend::bind(config.pdfium_dir.as_deref()) {
                Ok(b) => Some(Arc::new(b)),
                Err(e) => {
                    tracing::warn!(
                        code = e.code(),
                        "PDFium nicht verfügbar – keine Vorschau, automatische Positionierung"
                    );
                    None
                }
            },
            None => None,
        };
        let profiles = match pdfover_sigblock::ProfileStore::builtin() {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                tracing::error!(code = e.code(), "Signaturprofile nicht ladbar");
                None
            }
        };

        let (tx, rx) = mpsc::unbounded_channel();
        let interactive = options.interactive.unwrap_or(!config.headless);
        let broker = Arc::new(broker::CoreBroker::new(interactive, runtime::broker_sink(tx.clone())));
        broker.set_shell(options.shell.clone());
        let fido2_available = config.fido2.as_ref().is_some_and(|f| f.is_available());
        let shared = Arc::new(Shared {
            docs: docs::DocStore::new(options.tile_cache_bytes),
            render,
            engine: options.engine.clone().unwrap_or_else(engine::default_engine),
            profiles,
            fonts: Arc::new(pdfover_sigblock::FontProvider::builtin()),
            block: Mutex::new(runtime::BlockInfo::default()),
            broker,
            sign: Mutex::new(runtime::SignSlot::default()),
            tx: tx.clone(),
            shell: Mutex::new(options.shell.clone()),
            tile_scale: Mutex::new(1.0),
        });
        let registry = backends::Registry::new(
            config.platform,
            config.fido2.clone(),
            &config.app_version,
            options.backends.clone(),
            options.probe_local_bku,
        );
        let (view_tx, view_rx) = watch::channel(initial_view(cfg.language(), config.platform));
        let (finished_tx, finished_rx) = watch::channel(None);
        let mobile = runtime::MobileSetup {
            platform: config.platform,
            persist_prepared: options.persist_prepared.unwrap_or(!config.platform.is_desktop()) && !config.headless,
            default_output_dir: options.default_output_dir.clone(),
        };
        let task = CoreTask::new(
            shared.clone(),
            cfg,
            registry,
            config.headless,
            view_tx,
            finished_tx,
            startup,
            fido2_available,
            mobile,
        );
        handle.spawn(task.run(rx));
        Ok(Core { tx, view_rx, finished_rx, shared, runtime, _log: Mutex::new(log) })
    }

    /// Sendet ein UI-Ereignis (nicht blockierend).
    pub fn dispatch(&self, event: UiEvent) {
        if self.tx.send(Msg::Ui(event)).is_err() {
            tracing::warn!("Kern beendet – Ereignis verworfen");
        }
    }

    /// Abonniert den ViewState (aktueller Wert sofort verfügbar).
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<ViewState> {
        self.view_rx.clone()
    }

    /// Aktueller ViewState.
    pub fn view(&self) -> ViewState {
        self.view_rx.borrow().clone()
    }

    /// PNG einer Seitenkachel (Custom Protocol `pdfover-render://`); `page == Seitenzahl` liefert die
    /// weiße virtuelle neue Seite.
    pub fn page_tile(&self, doc: DocId, page: usize, scale: f32) -> Result<Vec<u8>, CoreError> {
        let render = self
            .shared
            .render
            .as_ref()
            .ok_or(pdfover_render::RenderError::LibraryNotFound("PDFium nicht gebunden".into()))?;
        if scale.is_finite() && scale > 0.0 {
            *self.shared.tile_scale.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = scale;
        }
        match self.shared.docs.tile(doc, page, scale, render.as_ref())? {
            Some(png) => Ok(png.to_vec()),
            None => Err(CoreError::UnknownDocument(doc)),
        }
    }

    /// PNG der Blockvorschau aus dem echten Appearance-Objekt (Plan §6.6).
    pub fn block_preview(&self, scale: f32) -> Result<Vec<u8>, CoreError> {
        let render = self
            .shared
            .render
            .as_ref()
            .ok_or(pdfover_render::RenderError::LibraryNotFound("PDFium nicht gebunden".into()))?;
        let pdf = self
            .shared
            .block
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pdf
            .clone()
            .ok_or(CoreError::NoPreview)?;
        let scale = if scale.is_finite() && scale > 0.0 { scale.min(16.0) } else { 1.0 };
        let renderer = render.open(pdf, None)?;
        let image = renderer.render_page(0, scale)?;
        Ok(pdfover_render::encode_png(&image)?)
    }

    /// Wartet bis `Finished` (CLI) und liefert den Exit-Code.
    pub async fn wait_finished(&self) -> ExitCode {
        let mut rx = self.finished_rx.clone();
        loop {
            if let Some(code) = *rx.borrow_and_update() {
                return code;
            }
            if rx.changed().await.is_err() {
                return ExitCode::General;
            }
        }
    }

    /// Renderer (PDFium) gebunden? Ohne ihn gibt es keine Vorschau, die Position ist automatisch.
    pub fn has_renderer(&self) -> bool {
        self.shared.render.is_some()
    }

    /// Setzt die Shell-Funktionen (Öffnen von Dateien/URLs, Elternfenster).
    pub fn set_shell(&self, shell: Option<Arc<dyn ShellHooks>>) {
        self.shared.broker.set_shell(shell.clone());
        *self.shared.shell.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = shell;
    }

    /// Ergebnisse der abgeschlossenen Dokumente (CLI `--json`, Warteschlangen-Panel).
    pub fn results(&self) -> Vec<DoneItemView> {
        self.view().done
    }

    /// Anzahl der im Speicher gehaltenen Dokumente (Diagnose; Feature `testing`).
    #[cfg(any(test, feature = "testing"))]
    pub fn open_document_count(&self) -> usize {
        self.shared.docs.len()
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
    }
}
