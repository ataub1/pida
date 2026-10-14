//! `pdfover-core-api` – Fassade des Kerns für Shell (Tauri 2), CLI und Plan-B-Shell
//! (uniffi-tauglich, ohne Tauri-Abhängigkeit; Plan §2.5, §3.4).
//!
//! Die UI hält **keinen Fachzustand**: Sie projiziert nur [`ViewState`] und sendet [`UiEvent`].
//! Beide Typen sind serde-serialisierbar (`tag`-basiert) für die Tauri-Bridge
//! (`tauri-specta`/`ts-rs` erzeugen daraus die TypeScript-Typen).
//!
//! Ownership: **Integration**.

mod view;

use std::path::PathBuf;
use std::sync::Arc;

pub use view::{
    BackendOptionView, BlockView, DocInfo, FieldInfo, FormAnswerView, InteractionView, MainBarView, OutputView,
    PrivacyNoticeView, QrInfo, QueueItemView, SettingsView, StepView, UiEvent, ViewState,
};

use pdfover_backend_api::Platform;
use pdfover_config::{Overlay, Paths};
use pdfover_workflow::{DocId, ExitCode};

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

/// Der Kern (ein Prozess, eine Instanz; Single-Instance übernimmt die Shell).
#[derive(Debug)]
pub struct Core {
    _private: (),
}

impl Core {
    /// Startet den Kern (Einstellungen laden, Richtlinie, Backends registrieren, Reducer-Task).
    pub fn new(config: CoreConfig) -> Result<Core, CoreError> {
        todo!("Integration")
    }

    /// Sendet ein UI-Ereignis (nicht blockierend).
    pub fn dispatch(&self, event: UiEvent) {
        todo!("Integration")
    }

    /// Abonniert den ViewState (aktueller Wert sofort verfügbar).
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<ViewState> {
        todo!("Integration")
    }

    /// Aktueller ViewState.
    pub fn view(&self) -> ViewState {
        todo!("Integration")
    }

    /// PNG einer Seitenkachel (Custom Protocol `pdfover-render://`).
    pub fn page_tile(&self, doc: DocId, page: usize, scale: f32) -> Result<Vec<u8>, CoreError> {
        todo!("Integration")
    }

    /// PNG der Blockvorschau aus dem echten Appearance-Objekt (Plan §6.6).
    pub fn block_preview(&self, scale: f32) -> Result<Vec<u8>, CoreError> {
        todo!("Integration")
    }

    /// Wartet bis `Finished` (CLI) und liefert den Exit-Code.
    pub async fn wait_finished(&self) -> ExitCode {
        todo!("Integration")
    }
}
