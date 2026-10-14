//! Serde-Typen der Bridge (Plan §3.4, TypeScript-Skizze `ViewState`/`UiEvent`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use pdfover_backend_api::{BackendId, Capabilities, NativeForm, Notice, NoticeAnswer, Progress, SecretPrompt};
use pdfover_i18n::Lang;
use pdfover_workflow::{DocId, MainBarStep, PlaceholderAnswer, Step};

/// Dokument (Anzeige).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocInfo {
    /// Kennung
    pub id: DocId,
    /// Name
    pub name: String,
    /// Seitenzahl
    pub page_count: usize,
    /// sichtbare Seitengrößen in pt
    pub page_sizes: Vec<(f32, f32)>,
    /// bereits signiert
    pub signed: bool,
}

/// Eintrag der Warteschlange.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueueItemView {
    /// Kennung
    pub id: DocId,
    /// Name
    pub name: String,
    /// aktuell
    pub current: bool,
}

/// Leeres Feld (Auswahlliste).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldInfo {
    /// qualifizierter Name
    pub name: String,
    /// Seite
    pub page: Option<usize>,
}

/// QR-Platzhalter.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QrInfo {
    /// Seite
    pub page: usize,
    /// id
    pub id: Option<String>,
}

/// Blockgröße und Vorschau.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockView {
    /// Breite in pt
    pub width: f32,
    /// Höhe in pt
    pub height: f32,
    /// Vorschau-Revision (Cache-Busting für `blockPreviewUrl`)
    pub preview_revision: u64,
}

/// Backend-Option.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendOptionView {
    /// Backend
    pub id: BackendId,
    /// verfügbar
    pub available: bool,
    /// Fähigkeiten
    pub capabilities: Option<Capabilities>,
}

/// Datenschutzhinweis.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrivacyNoticeView {
    /// Backend
    pub backend: BackendId,
    /// Textversion
    pub version: u32,
    /// eigener Text per Richtlinie
    pub custom_text: Option<String>,
}

/// Interaktion während des Signierens.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionView {
    /// Warten (Fortschritt)
    Waiting {
        /// Fortschritt
        progress: Progress,
    },
    /// Native Maske (Szenario A)
    Form {
        /// Anfrage-ID (Antwort über [`UiEvent::SubmitForm`])
        request: u64,
        /// Maske
        form: NativeForm,
    },
    /// Passwortabfrage
    Secret {
        /// Anfrage-ID
        request: u64,
        /// Abfrage
        prompt: SecretPrompt,
    },
    /// Meldung
    Notice {
        /// Anfrage-ID
        request: u64,
        /// Meldung
        notice: Notice,
    },
}

/// Ergebnis.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputView {
    /// gespeichert unter
    pub path: Option<PathBuf>,
    /// Speichern fehlgeschlagen/abgebrochen (Knopf „Speichern“, F-86)
    pub save_pending: bool,
}

/// Hauptleiste.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MainBarView {
    /// Schritte
    pub steps: Vec<MainBarStep>,
    /// Einstellungen erreichbar
    pub settings_enabled: bool,
}

/// Schritt-spezifische Sicht.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum StepView {
    /// Start
    Starting,
    /// Öffnen
    Open {
        /// Warteschlange
        queue: Vec<QueueItemView>,
    },
    /// Platzhalter-Frage/Feldauswahl
    Placeholder {
        /// Dokument
        doc: DocInfo,
        /// leere Felder
        fields: Vec<FieldInfo>,
        /// QR-Platzhalter
        qr: Option<QrInfo>,
    },
    /// Positionierung
    Position {
        /// Dokument
        doc: DocInfo,
        /// Seite
        page: usize,
        /// Block
        block: BlockView,
        /// Position (PAGE_ABS, linke obere Ecke)
        placement: pdfover_placement::Placement,
        /// virtuelle neue Seite
        new_page: bool,
        /// Live-Region-Text-Parameter (Seite, mm von links/unten)
        announce: BTreeMap<String, String>,
    },
    /// Backend-Auswahl
    Backend {
        /// Optionen
        options: Vec<BackendOptionView>,
        /// Datenschutzhinweis
        privacy_notice: Option<PrivacyNoticeView>,
    },
    /// Signieren
    Signing {
        /// Interaktion
        interaction: InteractionView,
        /// Warteschlange
        queue: Vec<QueueItemView>,
    },
    /// Ergebnis
    Output {
        /// Ergebnis
        result: OutputView,
        /// nächstes Dokument
        next: Option<DocInfo>,
    },
    /// Fehler
    Error {
        /// Fehlercode (i18n-ID)
        code: String,
        /// Parameter
        params: BTreeMap<String, String>,
        /// Wiederholen möglich
        retry: bool,
    },
    /// Ende
    Finished {
        /// Exit-Code
        exit_code: u8,
    },
}

/// Einstellungen (Overlay über jedem Schritt).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SettingsView {
    /// Einstellungen (Schema `pdfover-config`)
    pub settings: pdfover_config::Settings,
    /// Validierungsprobleme je Feld (i18n-IDs)
    pub problems: BTreeMap<String, String>,
    /// Schlüsselbund verfügbar
    pub keychain_available: bool,
    /// FIDO2 verfügbar
    pub fido2_available: bool,
}

/// Gesamtzustand für die UI.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewState {
    /// Schritt
    pub step: StepView,
    /// Hauptleiste
    pub main_bar: MainBarView,
    /// Einstellungen, falls geöffnet
    pub settings: Option<SettingsView>,
    /// UI-Sprache
    pub language: Lang,
}

/// Antwort auf eine Maske (serde-Variante von `FormAnswer`; Passwort wird sofort in `Zeroizing`
/// überführt und nie geloggt).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormAnswerView {
    /// Anmeldung
    Login {
        /// Benutzername
        username: String,
        /// Passwort
        password: String,
        /// Merken
        remember: bool,
    },
    /// TAN
    Tan {
        /// TAN
        tan: String,
    },
    /// SMS
    SwitchToSms,
    /// FIDO2
    SwitchToFido2,
    /// FIDO2 starten
    StartFido2,
    /// weiter
    Continue,
}

impl std::fmt::Debug for FormAnswerView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormAnswerView::Login { username, remember, .. } => f
                .debug_struct("Login")
                .field("username", username)
                .field("password", &"***")
                .field("remember", remember)
                .finish(),
            FormAnswerView::Tan { .. } => f.write_str("Tan(***)"),
            FormAnswerView::SwitchToSms => f.write_str("SwitchToSms"),
            FormAnswerView::SwitchToFido2 => f.write_str("SwitchToFido2"),
            FormAnswerView::StartFido2 => f.write_str("StartFido2"),
            FormAnswerView::Continue => f.write_str("Continue"),
        }
    }
}

/// UI-Ereignis.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    /// Dateien öffnen (Dialog, Drop, „Öffnen mit“, Share)
    OpenFiles {
        /// Pfade
        paths: Vec<PathBuf>,
    },
    /// Platzhalter-Antwort
    PlaceholderAnswer {
        /// Antwort
        answer: PlaceholderAnswer,
        /// gewähltes Feld
        field_index: Option<usize>,
    },
    /// Block verschieben (PAGE_ABS)
    MoveBlock {
        /// Seite
        page: usize,
        /// x
        x: f32,
        /// y
        y: f32,
    },
    /// Tastatur: Schritte (15/1/50 pt)
    NudgeBlock {
        /// dx in Schritten
        dx: i32,
        /// dy in Schritten
        dy: i32,
        /// fein (Shift)
        fine: bool,
        /// grob (Strg/⌘)
        coarse: bool,
    },
    /// Seite wechseln
    ChangePage {
        /// Seite
        page: usize,
    },
    /// „Neue Seite“
    ToggleNewPage,
    /// „Automatisch platzieren“
    AutoPlace,
    /// Position bestätigen
    ConfirmPosition,
    /// Backend wählen
    ChooseBackend {
        /// Backend
        backend: BackendId,
    },
    /// Datenschutzhinweis
    AcknowledgePrivacy {
        /// angenommen
        accepted: bool,
    },
    /// Maskenantwort
    SubmitForm {
        /// Anfrage-ID
        request: u64,
        /// Antwort
        answer: FormAnswerView,
    },
    /// Passwort
    SubmitSecret {
        /// Anfrage-ID
        request: u64,
        /// Passwort
        secret: String,
    },
    /// Meldung beantwortet
    AnswerNotice {
        /// Anfrage-ID
        request: u64,
        /// Antwort
        answer: NoticeAnswer,
    },
    /// Abbrechen (aktuelle Interaktion bzw. Schritt)
    Cancel,
    /// Wiederholen
    Retry,
    /// Speichern (F-86) bzw. Speichern-Dialog beantwortet
    Save {
        /// Ziel (`None` = Dialog abgebrochen)
        path: Option<PathBuf>,
    },
    /// Überschreiben bestätigt
    ConfirmOverwrite {
        /// ja
        overwrite: bool,
    },
    /// Ergebnis öffnen
    OpenResult,
    /// Hauptleiste
    Navigate {
        /// Schritt
        step: Step,
    },
    /// Einstellungen öffnen
    OpenSettings,
    /// Einstellungen speichern
    SaveSettings {
        /// neue Einstellungen
        settings: Box<pdfover_config::Settings>,
    },
    /// Einstellungen schließen (verwerfen)
    CloseSettings,
    /// „Einstellungen zurücksetzen“
    ResetSettings,
    /// Lebenszyklus: Hintergrund (kein Abbruch)
    Backgrounded,
    /// Lebenszyklus: Vordergrund
    Foregrounded,
    /// Beenden
    Quit,
}

impl std::fmt::Debug for UiEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiEvent::SubmitSecret { request, .. } => {
                f.debug_struct("SubmitSecret").field("request", request).field("secret", &"***").finish()
            }
            UiEvent::SubmitForm { request, answer } => {
                f.debug_struct("SubmitForm").field("request", request).field("answer", answer).finish()
            }
            other => {
                // Alle übrigen Varianten enthalten keine Secrets.
                let s = serde_json::to_string(other).unwrap_or_else(|_| "UiEvent".into());
                f.write_str(&s)
            }
        }
    }
}
