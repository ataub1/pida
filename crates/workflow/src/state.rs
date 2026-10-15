//! Zustände, Ereignisse, Effekte und Hauptleiste (docs/analyse/01 §3.3–§3.5).
//!
//! Der Reducer selbst steht in [`crate::reducer`]; hier liegen die Datentypen und die
//! Hauptleisten-Matrix.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pdfover_backend_api::{BackendError, BackendId, Capabilities, CertNeed};
use pdfover_config::BackendChoice;
use pdfover_pades::{AcceptanceClass, InvariantViolation, OutputInfo};
use pdfover_placement::{EmptyFieldCandidate, Placement, QrPlaceholder};

use crate::output::{ExitCode, OutputDecision};
use crate::queue::{DocId, DocOutcome, InputDocument, Queue};

/// Schritte der Hauptleiste (F-100).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Step {
    /// Öffnen
    Open,
    /// Positionierung
    Positioning,
    /// Signieren
    Signing,
    /// Fertig (nur Anzeige)
    Finish,
}

/// Zustand eines Schritts in der Hauptleiste (Tabelle docs/analyse/01 §3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MainBarStep {
    /// Schritt
    pub step: Step,
    /// erreicht („A“)
    pub active: bool,
    /// klickbar („E“)
    pub enabled: bool,
}

/// Wie positioniert wird.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PositionChoice {
    /// `p:auto;x:auto;y:auto`
    Auto,
    /// manuell
    Manual(Placement),
    /// QR-Platzhalter
    Placeholder(QrPlaceholder),
    /// leeres Signaturfeld
    Field(EmptyFieldCandidate),
    /// unsichtbar
    Invisible,
}

impl PositionChoice {
    /// Sichtbarer Signaturblock?
    pub fn is_visible(&self) -> bool {
        !matches!(self, PositionChoice::Invisible)
    }

    /// Manuelle Position, falls gewählt (F-43: bleibt beim Rücksprung erhalten).
    pub fn manual(&self) -> Option<&Placement> {
        match self {
            PositionChoice::Manual(p) => Some(p),
            _ => None,
        }
    }
}

/// Gefundener Platzhalter beim Öffnen (F-20/F-21).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlaceholderFound {
    /// leere Signaturfelder (`USE_FIELDS`)
    Fields(Vec<EmptyFieldCandidate>),
    /// QR-Platzhalter (`USE_MARKER`, id 1, SORTED)
    Qr(QrPlaceholder),
}

impl PlaceholderFound {
    /// Nichts Verwendbares (leere Feldliste).
    pub fn is_empty(&self) -> bool {
        matches!(self, PlaceholderFound::Fields(f) if f.is_empty())
    }
}

/// Antwort auf „Platzhalter verwenden?“.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlaceholderAnswer {
    /// Ja
    Yes,
    /// Nein → normale Positionierung
    No,
    /// Abbrechen → Dokument verworfen
    Cancel,
}

/// Phasen des Signierens (Plan §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SigningPhase {
    /// Sitzung öffnen (Keystore entsperren, NullOperation)
    OpeningSession,
    /// Zertifikat lesen (InfoboxRead/Keystore)
    ReadingCertificate,
    /// `prepare()`
    Preparing,
    /// Backend `sign` (A-Trust-Dialoge laufen hier)
    Signing,
    /// `finish_*` + Selbstprüfung I1–I6
    Finishing,
    /// Schreiben (reserviert; geschrieben wird im Zustand [`State::Output`], damit Überschreib-Rückfrage
    /// und Speichern-Dialog dort angezeigt werden)
    Writing,
}

/// Ziel von „Wiederholen“.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RetryTarget {
    /// neu vorbereiten (F-71: Retry → PrepareSigning)
    Prepare,
    /// neu vorbereiten mit mindestens dieser `/Contents`-Reserve in Bytes – nach
    /// [`WorkflowError::CmsTooLarge`]; „Wiederholen“ ist die zweite Bestätigung (Plan §6.2, R9)
    PrepareWithReserve(usize),
    /// erneut speichern (F-85)
    Save,
    /// Dokument erneut laden (F-15)
    Load,
    /// Einstellungen zurücksetzen (nach `.old` verschieben) und neu starten (F-05, Plan §7.1)
    ResetSettings,
}

/// Fehler im Workflow (Fehlercode + Parameter für i18n).
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum WorkflowError {
    /// Dokument nicht ladbar/abgelehnt
    #[error("Dokument abgelehnt: {class:?} {code}")]
    Rejected {
        /// Klasse
        class: AcceptanceClass,
        /// Code
        code: String,
    },
    /// Backend-Fehler
    #[error("Signaturfehler: {code}")]
    Backend {
        /// `BackendError::code()`
        code: String,
        /// Detail
        detail: String,
    },
    /// Invariante verletzt – nichts gespeichert
    #[error("Selbstprüfung fehlgeschlagen: {0}")]
    Invariant(InvariantViolation),
    /// CMS größer als die Reserve – nichts gespeichert (`pdfover_pades::FinishError::CmsTooLarge`)
    #[error("Signatur zu groß: {needed} Bytes, Reserve {reserve}")]
    CmsTooLarge {
        /// benötigt
        needed: usize,
        /// vorhanden
        reserve: usize,
    },
    /// Einstellungen nicht lesbar (`LoadOutcome::Unreadable`, F-05)
    #[error("Einstellungen nicht lesbar: {0}")]
    Config(String),
    /// Schreibfehler
    #[error("Speichern fehlgeschlagen: {0}")]
    Save(String),
    /// Vorbereiten fehlgeschlagen (Signaturblock, Positionierung, Engine; PDF-Over
    /// `error.PrepareDocument`, Wiederholen/Abbrechen)
    #[error("Vorbereitung fehlgeschlagen: {code}")]
    Prepare {
        /// Code (`sigblock-…`, `placement.…`, `ng-certificate-not-valid`, `error-PrepareDocument`)
        code: String,
        /// Detail
        detail: String,
    },
    /// Unerwartet
    #[error("unerwarteter Fehler: {0}")]
    Unexpected(String),
}

/// Meldungs-ID: Invariante verletzt (Parameter `invariant`, `detail`).
pub const MSG_INVARIANT: &str = "ng-invariant-violated";
/// Meldungs-ID: CMS größer als die Reserve (Parameter `needed`, `reserve`).
pub const MSG_CMS_TOO_LARGE: &str = "ng-cms-too-large";
/// Meldungs-ID: Speichern fehlgeschlagen (PDF-Over `output.save_failed`, `arg1` Datei, `arg2` Grund).
pub const MSG_SAVE_FAILED: &str = "output-save_failed";
/// Meldungs-ID: Einstellungen beim Start nicht lesbar, Werksreset anbieten (PDF-Over
/// `error.Initialization`, Ja/Nein).
pub const MSG_INITIALIZATION: &str = "error-Initialization";
/// Meldungs-ID: unerwarteter Fehler (PDF-Over `error.Unexpected`).
pub const MSG_UNEXPECTED: &str = "error-Unexpected";
/// Meldungs-ID: Positionierung nicht möglich, automatische Position (PDF-Over
/// `error.PositioningNotPossible`).
pub const MSG_POSITIONING_NOT_POSSIBLE: &str = "error-PositioningNotPossible";
/// Meldungs-ID: Ohne Oberfläche ist das Signiermittel nicht eindeutig (`-b` angeben).
pub const MSG_BACKEND_CHOICE_REQUIRED: &str = "ng-backend-choice-required";
/// Meldungs-ID: Kein Signiermittel verfügbar.
pub const MSG_NO_BACKEND: &str = "ng-no-backend-available";
/// Meldungs-ID: Das Standard-Signiermittel ist nicht verfügbar.
pub const MSG_DEFAULT_BACKEND_UNAVAILABLE: &str = "ng-default-backend-unavailable";

impl WorkflowError {
    /// Aus einem Backend-Fehler (Code `backend-…`, Detail = Fehlertext ohne Secrets).
    pub fn from_backend(e: &BackendError) -> WorkflowError {
        let detail = match e {
            BackendError::UnsupportedKeystoreFormat { format } => format.clone(),
            BackendError::SecurityLayer { code, info } => format!("{code}: {info}"),
            BackendError::Http { status, reason } => format!("{status} {reason}"),
            other => other.to_string(),
        };
        WorkflowError::Backend { code: e.code().to_owned(), detail }
    }

    /// Stabile Meldungs-ID (i18n).
    pub fn code(&self) -> String {
        match self {
            WorkflowError::Rejected { code, .. } => code.clone(),
            WorkflowError::Backend { code, .. } => code.clone(),
            WorkflowError::Invariant(_) => MSG_INVARIANT.to_owned(),
            WorkflowError::CmsTooLarge { .. } => MSG_CMS_TOO_LARGE.to_owned(),
            WorkflowError::Config(_) => MSG_INITIALIZATION.to_owned(),
            WorkflowError::Save(_) => MSG_SAVE_FAILED.to_owned(),
            WorkflowError::Prepare { code, .. } => code.clone(),
            WorkflowError::Unexpected(_) => MSG_UNEXPECTED.to_owned(),
        }
    }

    /// Parameter zur Meldungs-ID (Fluent-Variablen).
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        match self {
            WorkflowError::Rejected { class, .. } => {
                m.insert("class".into(), format!("{class:?}"));
            }
            WorkflowError::Backend { detail, .. } | WorkflowError::Prepare { detail, .. } => {
                m.insert("detail".into(), detail.clone());
            }
            WorkflowError::Invariant(v) => {
                m.insert("invariant".into(), format!("{:?}", v.invariant));
                m.insert("detail".into(), v.detail.clone());
            }
            WorkflowError::CmsTooLarge { needed, reserve } => {
                m.insert("needed".into(), needed.to_string());
                m.insert("reserve".into(), reserve.to_string());
            }
            WorkflowError::Config(d) | WorkflowError::Unexpected(d) => {
                m.insert("detail".into(), d.clone());
            }
            WorkflowError::Save(d) => {
                // `output.save_failed`: arg1 = Datei (setzt core-api), arg2 = Grund
                m.insert("arg2".into(), d.clone());
                m.insert("detail".into(), d.clone());
            }
        }
        m
    }

    /// Exit-Code der CLI (Plan §7.4).
    pub fn exit_code(&self) -> ExitCode {
        match self {
            WorkflowError::Rejected { .. } => ExitCode::Rejected,
            WorkflowError::Backend { code, .. } if code == "backend-cancelled" => ExitCode::Cancelled,
            WorkflowError::Backend { .. } | WorkflowError::CmsTooLarge { .. } => ExitCode::Backend,
            WorkflowError::Invariant(_) => ExitCode::Invariant,
            WorkflowError::Save(_) => ExitCode::Write,
            WorkflowError::Config(_) | WorkflowError::Prepare { .. } | WorkflowError::Unexpected(_) => {
                ExitCode::General
            }
        }
    }
}

/// Option der Backend-Auswahl.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendOption {
    /// Backend
    pub id: BackendId,
    /// Fähigkeiten (`None` = nicht verfügbar)
    pub capabilities: Option<Capabilities>,
}

impl BackendOption {
    /// Verfügbar?
    pub fn available(&self) -> bool {
        self.capabilities.is_some()
    }
}

/// Backend zu einer Einstellung (`BKU`).
pub fn backend_for_choice(choice: BackendChoice) -> BackendId {
    match choice {
        BackendChoice::IdAustria => BackendId::AtrustNative,
        BackendChoice::LocalBku => BackendId::LocalBku,
        BackendChoice::Keystore => BackendId::Keystore,
    }
}

/// Unveränderliche Eingaben des Reducers (aus Einstellungen, Overlay, Richtlinie, Plattform).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// automatische Positionierung (`SIGNATURE_POSITION=auto` oder `-a`)
    pub auto_position: bool,
    /// unsichtbares Profil (auch beim **ersten** Dokument, B-03 behoben)
    pub invisible: bool,
    /// Platzhaltersuche aktiv
    pub placeholder_search: bool,
    /// `USE_FIELDS` (Vorrang) statt `USE_MARKER`
    pub placeholder_fields: bool,
    /// Standard-Backend
    pub default_backend: Option<BackendChoice>,
    /// `-s`: Ergebnis-Screen überspringen, Ende nach Speichern
    pub skip_finish: bool,
    /// ohne grafische Oberfläche (CLI): automatische Positionierung, Platzhalter nach festen Regeln,
    /// kein Speichern-Dialog, Fehler ohne Wiederholen, Ende nach dem letzten Dokument
    pub headless: bool,
    /// Backends, deren Datenschutzhinweis in der aktuellen Textversion bereits bestätigt ist
    /// (`Settings::privacy`, Richtlinie) – dann entfällt [`State::PrivacyNotice`]
    pub privacy_acknowledged: Vec<BackendId>,
    /// Postfix der Ausgabedatei (`SAVE_FILE_POSTFIX`, Standard `_signed`) für den Speichern-Dialog (F-86)
    pub output_postfix: String,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            auto_position: false,
            invisible: false,
            placeholder_search: false,
            placeholder_fields: false,
            default_backend: None,
            skip_finish: false,
            headless: false,
            privacy_acknowledged: Vec::new(),
            output_postfix: "_signed".into(),
        }
    }
}

/// Was der Workflow nach der Analyse über das aktuelle Dokument weiß.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocFacts {
    /// Dokument
    pub id: DocId,
    /// Seitenzahl
    pub page_count: usize,
    /// bereits signiert (keine neue Seite, docs/analyse/04 §4.5)
    pub signed: bool,
    /// Annahmeklasse
    pub class: AcceptanceClass,
    /// Startposition der manuellen Positionierung (letzte Seite, 50 %/25 %, F-32/F-33);
    /// `None` = keine Vorschau möglich
    pub start: Option<Placement>,
}

impl DocFacts {
    /// Neue Seite erlaubt? Nicht auf bereits signierten Dokumenten (das schließt DocMDP P=2/3 ein,
    /// denn eine Zertifizierung ist selbst eine Signatur; Plan §6.1).
    pub fn new_page_allowed(&self) -> bool {
        !self.signed
    }
}

/// Ablaufdaten eines Signiervorgangs (aus den Fähigkeiten des gewählten Backends).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SignFlow {
    /// Zertifikat vorab lesen (Modell A immer; Modell B nur für einen sichtbaren Block)
    pub needs_certificate: bool,
    /// Mindestreserve für `/Contents` nach [`WorkflowError::CmsTooLarge`]
    pub min_reserve: Option<usize>,
}

impl SignFlow {
    /// Aus den Fähigkeiten und der Position.
    pub fn new(caps: &Capabilities, position: &PositionChoice) -> SignFlow {
        let needs_certificate = match caps.certificate {
            CertNeed::Required => true,
            CertNeed::OnlyForVisibleBlock => position.is_visible(),
            CertNeed::No => false,
        };
        SignFlow { needs_certificate, min_reserve: None }
    }
}

/// Zustand.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum State {
    /// Initialisierung (Konfiguration, Migration-Angebot)
    Starting,
    /// Dokumentauswahl
    Idle {
        /// Warteschlange
        queue: Queue,
    },
    /// Laden + Analyse + Platzhaltersuche
    Analyzing {
        /// Warteschlange (aktuelles Dokument wird analysiert)
        queue: Queue,
    },
    /// Warnung der Eingangsanalyse mit Abbruchmöglichkeit (Plan §6.1: `/Perms /UR3`, Hybrid-XFA,
    /// inkompatible Seed Values – `pades::Finding::warning`): „Fortfahren“ oder Dokument verwerfen
    AnalysisWarning {
        /// Warteschlange
        queue: Queue,
        /// Codes der Warnungen (i18n-IDs, z. B. `pdf-usage-rights`)
        warnings: Vec<String>,
        /// Dokument
        doc: DocFacts,
        /// Fund der Platzhaltersuche (gilt nach „Fortfahren“)
        placeholder: Option<PlaceholderFound>,
    },
    /// Frage „Platzhalter verwenden?“ bzw. Feldauswahl
    PlaceholderQuestion {
        /// Warteschlange
        queue: Queue,
        /// Fund
        found: PlaceholderFound,
        /// Dokument
        doc: DocFacts,
    },
    /// Positionierung
    Positioning {
        /// Warteschlange
        queue: Queue,
        /// aktuelle Seite (0-basiert; `== page_count` = virtuelle neue Seite)
        page: usize,
        /// gewählte/zuletzt bekannte Position (F-43)
        placement: Option<Placement>,
        /// virtuelle neue Seite aktiv (F-38)
        new_page: bool,
        /// Dokument
        doc: DocFacts,
    },
    /// Backend-Auswahl
    ChoosingBackend {
        /// Warteschlange
        queue: Queue,
        /// Position
        position: PositionChoice,
        /// Optionen (leer bis [`Event::BackendsProbed`])
        options: Vec<BackendOption>,
        /// Dokument
        doc: DocFacts,
        /// Standard-Backend bzw. automatische Wahl anwenden (nur auf dem Weg Öffnen/Positionieren →
        /// Signieren, F-53/F-54; nach Abbruch, Fehler oder Hauptleiste wird gefragt, F-55)
        auto_choice: bool,
    },
    /// Datenschutzhinweis vor dem ersten Upload (`document_leaves_device`)
    PrivacyNotice {
        /// Warteschlange
        queue: Queue,
        /// Position
        position: PositionChoice,
        /// Backend
        backend: BackendId,
        /// Dokument
        doc: DocFacts,
        /// Optionen der Auswahl (Rückkehr bei „Ablehnen“)
        options: Vec<BackendOption>,
    },
    /// Signieren
    Signing {
        /// Warteschlange
        queue: Queue,
        /// Position
        position: PositionChoice,
        /// Backend
        backend: BackendId,
        /// Phase
        phase: SigningPhase,
        /// Dokument
        doc: DocFacts,
        /// Ablaufdaten
        flow: SignFlow,
    },
    /// Ergebnis
    Output {
        /// Warteschlange
        queue: Queue,
        /// gespeichert unter (sonst Speichern-Knopf, F-86)
        saved: Option<OutputInfo>,
        /// offene Ausgabeentscheidung
        decision: Option<OutputDecision>,
        /// Dokument (Original, F-91: erneut signieren)
        doc: DocFacts,
        /// verwendete Position (F-43/F-91)
        position: PositionChoice,
        /// verwendetes Backend
        backend: BackendId,
    },
    /// Fehler mit Wiederholmöglichkeit
    Error {
        /// Warteschlange
        queue: Queue,
        /// Fehler
        error: WorkflowError,
        /// Wiederholen möglich
        retry: Option<RetryTarget>,
        /// Zustand, in dem der Fehler auftrat (Ziel von „Wiederholen“ bzw. „Abbrechen“)
        from: Option<Box<State>>,
    },
    /// Programmende
    Finished {
        /// Exit-Code
        exit: ExitCode,
        /// Ergebnisse aller abgeschlossenen Dokumente (CLI `--json`)
        #[serde(default)]
        results: Vec<DocOutcome>,
    },
}

impl State {
    /// Ergebnisse der abgeschlossenen Dokumente (auch nach dem Programmende).
    pub fn results(&self) -> &[DocOutcome] {
        match self {
            State::Finished { results, .. } => results,
            other => other.queue().map(|q| q.done.as_slice()).unwrap_or(&[]),
        }
    }

    /// Exit-Code, falls beendet.
    pub fn exit_code(&self) -> Option<ExitCode> {
        match self {
            State::Finished { exit, .. } => Some(*exit),
            _ => None,
        }
    }

    /// Einstellungen sind ein Overlay ohne eigenen Zustand (F-101 ohne Re-Entry): ob sie offen sind,
    /// hält `pdfover-core-api` (`ViewState::settings`); der Reducer sieht nur
    /// [`Event::SettingsChanged`]. Liefert deshalb immer `false`.
    pub fn settings_open(&self) -> bool {
        false
    }

    /// Warteschlange (außer in `Starting`/`Finished`).
    pub fn queue(&self) -> Option<&Queue> {
        match self {
            State::Starting | State::Finished { .. } => None,
            State::Idle { queue }
            | State::Analyzing { queue }
            | State::AnalysisWarning { queue, .. }
            | State::PlaceholderQuestion { queue, .. }
            | State::Positioning { queue, .. }
            | State::ChoosingBackend { queue, .. }
            | State::PrivacyNotice { queue, .. }
            | State::Signing { queue, .. }
            | State::Output { queue, .. }
            | State::Error { queue, .. } => Some(queue),
        }
    }

    /// Veränderbare Warteschlange.
    pub fn queue_mut(&mut self) -> Option<&mut Queue> {
        match self {
            State::Starting | State::Finished { .. } => None,
            State::Idle { queue }
            | State::Analyzing { queue }
            | State::AnalysisWarning { queue, .. }
            | State::PlaceholderQuestion { queue, .. }
            | State::Positioning { queue, .. }
            | State::ChoosingBackend { queue, .. }
            | State::PrivacyNotice { queue, .. }
            | State::Signing { queue, .. }
            | State::Output { queue, .. }
            | State::Error { queue, .. } => Some(queue),
        }
    }

    /// Dokumentdaten des aktuellen Dokuments (nach der Analyse).
    pub fn doc(&self) -> Option<&DocFacts> {
        match self {
            State::AnalysisWarning { doc, .. }
            | State::PlaceholderQuestion { doc, .. }
            | State::Positioning { doc, .. }
            | State::ChoosingBackend { doc, .. }
            | State::PrivacyNotice { doc, .. }
            | State::Signing { doc, .. }
            | State::Output { doc, .. } => Some(doc),
            State::Error { from: Some(f), .. } => f.doc(),
            _ => None,
        }
    }

    /// Schritt, der in der Hauptleiste „aktuell“ ist.
    pub fn step(&self) -> Option<Step> {
        match self {
            State::Starting | State::Finished { .. } => None,
            State::Idle { .. }
            | State::Analyzing { .. }
            | State::AnalysisWarning { .. }
            | State::PlaceholderQuestion { .. } => Some(Step::Open),
            State::Positioning { .. } => Some(Step::Positioning),
            State::ChoosingBackend { .. } | State::PrivacyNotice { .. } | State::Signing { .. } => Some(Step::Signing),
            State::Output { .. } => Some(Step::Finish),
            State::Error { from, .. } => from.as_deref().and_then(State::step).or(Some(Step::Open)),
        }
    }

    /// Hauptleisten-Zustände je State (docs/analyse/01 §3.5). Während des Signierens ist nichts
    /// klickbar (PDF-Over: Verhalten von `SigningState` auch während der ID-Austria-Dialoge);
    /// Fehlermeldungen übernehmen die aktiven Schritte des Ursprungszustands, sind aber modal.
    pub fn main_bar(&self) -> [MainBarStep; 4] {
        // (aktiv, klickbar) für Open, Positioning, Signing, Finish
        let flags: [(bool, bool); 4] = match self {
            State::Starting | State::Finished { .. } => [(false, false); 4],
            State::Idle { .. }
            | State::Analyzing { .. }
            | State::AnalysisWarning { .. }
            | State::PlaceholderQuestion { .. } => [(true, true), (false, false), (false, false), (false, false)],
            State::Positioning { .. } => [(true, true), (true, false), (false, false), (false, false)],
            State::ChoosingBackend { .. } | State::PrivacyNotice { .. } => {
                [(true, true), (true, true), (true, false), (false, false)]
            }
            State::Signing { .. } => [(true, false), (true, false), (true, false), (false, false)],
            State::Output { .. } => [(true, true), (true, true), (true, true), (true, false)],
            State::Error { from, .. } => {
                let base = from.as_deref().map(State::main_bar);
                let mut out = [(true, false), (false, false), (false, false), (false, false)];
                if let Some(base) = base {
                    for (o, b) in out.iter_mut().zip(base) {
                        *o = (b.active, false);
                    }
                }
                out
            }
        };
        let steps = [Step::Open, Step::Positioning, Step::Signing, Step::Finish];
        let mut out = [MainBarStep { step: Step::Open, active: false, enabled: false }; 4];
        for (i, (active, enabled)) in flags.into_iter().enumerate() {
            out[i] = MainBarStep { step: steps[i], active, enabled };
        }
        out
    }

    /// Einstellungen erreichbar (Konfig-Button, Spalte „Config“ in docs/analyse/01 §3.5).
    pub fn settings_enabled(&self) -> bool {
        matches!(
            self,
            State::Idle { .. }
                | State::Analyzing { .. }
                | State::AnalysisWarning { .. }
                | State::PlaceholderQuestion { .. }
                | State::Positioning { .. }
                | State::ChoosingBackend { .. }
                | State::PrivacyNotice { .. }
                | State::Output { .. }
        )
    }

    /// Klickbarer Hauptleisten-Schritt?
    pub fn step_enabled(&self, step: Step) -> bool {
        self.main_bar().iter().any(|s| s.step == step && s.enabled)
    }

    /// Kurzname (Logs, Diagnose; ohne Inhalte).
    pub fn name(&self) -> &'static str {
        match self {
            State::Starting => "Starting",
            State::Idle { .. } => "Idle",
            State::Analyzing { .. } => "Analyzing",
            State::AnalysisWarning { .. } => "AnalysisWarning",
            State::PlaceholderQuestion { .. } => "PlaceholderQuestion",
            State::Positioning { .. } => "Positioning",
            State::ChoosingBackend { .. } => "ChoosingBackend",
            State::PrivacyNotice { .. } => "PrivacyNotice",
            State::Signing { .. } => "Signing",
            State::Output { .. } => "Output",
            State::Error { .. } => "Error",
            State::Finished { .. } => "Finished",
        }
    }
}

/// Ereignis (UI, Effekt-Ergebnis, Lebenszyklus).
///
/// `Analyzed` trägt den gefundenen Platzhalter direkt (QR-Platzhalter mit Matrix und Position);
/// Ereignisse leben nur bis zum nächsten Reducer-Schritt, deshalb wird nicht geboxt.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Start abgeschlossen (CLI-Dateien bereits im Overlay)
    Started {
        /// Eingabedokumente
        documents: Vec<InputDocument>,
    },
    /// Dateien hinzugefügt (Dialog, Drop, „Öffnen mit“, Share)
    FilesAdded(Vec<InputDocument>),
    /// Analyse fertig
    Analyzed {
        /// Dokument
        doc: DocId,
        /// Klasse
        class: AcceptanceClass,
        /// Platzhalter
        placeholder: Option<PlaceholderFound>,
        /// bereits signiert
        signed: bool,
        /// Seitenzahl
        page_count: usize,
        /// Startposition für die manuelle Positionierung (`None` = keine Vorschau)
        start: Option<Placement>,
        /// Warnungen mit Abbruchmöglichkeit (`pades::Finding::warning`, Codes)
        warnings: Vec<String>,
    },
    /// Analyse/Laden fehlgeschlagen
    AnalysisFailed {
        /// Dokument
        doc: DocId,
        /// Fehler
        error: WorkflowError,
    },
    /// Vorschau/Positionierung nicht möglich (Renderer fehlt oder scheitert) → Hinweis
    /// `error.PositioningNotPossible` und automatische Positionierung (docs/analyse/01 §3.4,
    /// `PositioningState.java:171-179`)
    PreviewUnavailable {
        /// Dokument
        doc: DocId,
    },
    /// Antwort auf die Warnung der Analyse (`true` = trotzdem signieren, `false` = Dokument verwerfen)
    WarningAnswered(bool),
    /// Antwort auf die Platzhalterfrage; bei Feldern mit gewähltem Index
    PlaceholderAnswered {
        /// Antwort
        answer: PlaceholderAnswer,
        /// gewähltes Feld
        field_index: Option<usize>,
    },
    /// Block verschoben
    PlacementChanged(Placement),
    /// Seite gewechselt
    PageChanged(usize),
    /// „Neue Seite“ umgeschaltet
    NewPageToggled,
    /// Position bestätigt („Signieren“/Enter)
    PositionConfirmed,
    /// Backends geprüft
    BackendsProbed(Vec<BackendOption>),
    /// Backend gewählt
    BackendChosen(BackendId),
    /// Datenschutzhinweis beantwortet
    PrivacyAnswered(bool),
    /// Phase abgeschlossen
    PhaseCompleted(SigningPhase),
    /// Backend-Fehler
    BackendFailed(BackendError),
    /// Invariante verletzt (auch I3-Fehler aus `finish_cms`/`finish_raw`)
    SelfCheckFailed(InvariantViolation),
    /// Sonstiger Fehler beim Vorbereiten/Abschließen (Engine, Signaturblock, Platzierung)
    SigningFailed(WorkflowError),
    /// CMS passt nicht in die Reserve → Fehler mit [`RetryTarget::PrepareWithReserve`]
    CmsTooLarge {
        /// benötigt
        needed: usize,
        /// vorhanden
        reserve: usize,
    },
    /// Ausgabeziel entschieden (Ergebnis von [`Effect::DecideOutput`] bzw. gewählter Pfad aus dem
    /// Speichern-Dialog mit Existenzprüfung)
    OutputDecided(OutputDecision),
    /// Ausgabeziel entschieden bzw. Dialog beantwortet (`None` = abgebrochen). Ein Pfad ohne
    /// Existenzprüfung wird wie ein nicht vorhandenes Ziel behandelt – die Shell sollte
    /// [`Event::OutputDecided`] mit `exists` senden.
    OutputChosen(Option<PathBuf>),
    /// Überschreib-Rückfrage
    OverwriteAnswered(bool),
    /// Knopf „Speichern“ im Ergebnis-Screen (F-86: öffnet immer den Speichern-Dialog)
    SaveRequested,
    /// Link „signiertes Dokument öffnen“ (F-87)
    OpenResultRequested,
    /// gespeichert
    Saved(OutputInfo),
    /// Speichern fehlgeschlagen
    SaveFailed(String),
    /// Wiederholen
    Retry,
    /// Abbrechen
    Cancel,
    /// Hauptleiste
    Navigate(Step),
    /// Einstellungen geändert (Blockvorschau neu, Sprache neu)
    SettingsChanged,
    /// Einstellungen beim Start nicht lesbar (F-05) → Fehler mit [`RetryTarget::ResetSettings`]
    SettingsUnreadable(String),
    /// App in den Hintergrund (kein Abbruch!)
    Backgrounded,
    /// App wieder im Vordergrund (ggf. „Signatur fortsetzen“)
    Foregrounded,
    /// Beenden
    Quit,
}

/// Effekt, den `pdfover-core-api` ausführt.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Dokument laden, `analyze`, Platzhalter suchen
    Analyze(DocId),
    /// Seitenkachel rendern (Vorschau)
    RenderPage {
        /// Dokument
        doc: DocId,
        /// Seite
        page: usize,
    },
    /// Blockvorschau neu bauen (Profil/Emblem/Hinweis geändert)
    BuildBlockPreview(DocId),
    /// Backends prüfen (TCP-Probe, Keystore-Datei, FIDO2)
    ProbeBackends,
    /// Sitzung öffnen
    OpenSession(BackendId),
    /// Zertifikat lesen
    ReadCertificate,
    /// `prepare()` (persistiert auf Mobile den Zustand)
    Prepare {
        /// Dokument
        doc: DocId,
        /// Position
        position: PositionChoice,
        /// Mindestreserve für `/Contents` in Bytes (`None` = nach Backend-Modell,
        /// `Capabilities::cms_reserve`); gesetzt nach [`Event::CmsTooLarge`]
        min_reserve: Option<usize>,
    },
    /// Backend `sign`
    Sign,
    /// `finish_*` + Selbstprüfung
    Finish,
    /// Ausgabeziel bestimmen (F-82)
    DecideOutput(DocId),
    /// Speichern-Dialog zeigen
    AskSavePath(OutputDecision),
    /// Überschreib-Rückfrage
    AskOverwrite(PathBuf),
    /// atomar schreiben (eine vorhandene Datei wird **nicht** ersetzt)
    Write(PathBuf),
    /// atomar schreiben und eine vorhandene Datei ersetzen (nach bestätigter Rückfrage, F-84)
    Overwrite(PathBuf),
    /// Ergebnis öffnen (Opener/QuickLook/ACTION_VIEW)
    OpenResult(PathBuf),
    /// Sitzung abbrechen (Passwort löschen, Benutzername bleibt)
    CancelSession,
    /// Sitzung regulär schließen (nach `sign`)
    CloseSession,
    /// Hinweis ohne Rückfrage anzeigen (i18n-ID, z. B. `error-PositioningNotPossible`)
    Notify(String),
    /// Datenschutzhinweis für dieses Backend als bestätigt speichern (einmalig je Textversion)
    AcknowledgePrivacy(BackendId),
    /// Einstellungen zurücksetzen (`Paths::reset`) und Kern neu initialisieren
    ResetSettings,
    /// persistierten Zustand löschen
    DeletePersisted,
    /// Dokumentdaten freigeben (Renderer, Kachel-Cache, vorbereitetes PDF)
    Discard(DocId),
    /// letzten Ordner merken
    RememberDirectory(PathBuf),
    /// Programmende
    Exit(ExitCode),
}

/// Ergebnis eines Reducer-Schritts.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// neuer Zustand
    pub state: State,
    /// Effekte in Ausführungsreihenfolge
    pub effects: Vec<Effect>,
}

impl Transition {
    /// Zustand ohne Effekte.
    pub fn stay(state: State) -> Transition {
        Transition { state, effects: Vec::new() }
    }

    /// Zustand mit Effekten.
    pub fn with(state: State, effects: Vec<Effect>) -> Transition {
        Transition { state, effects }
    }
}
