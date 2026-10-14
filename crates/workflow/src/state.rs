//! Zustände, Ereignisse, Effekte und Reducer (docs/analyse/01 §3.3–§3.5).

use std::path::PathBuf;

use pdfover_backend_api::{BackendError, BackendId, Capabilities};
use pdfover_config::BackendChoice;
use pdfover_pades::{AcceptanceClass, InvariantViolation, OutputInfo};
use pdfover_placement::{EmptyFieldCandidate, Placement, QrPlaceholder};

use crate::output::{ExitCode, OutputDecision};
use crate::queue::{DocId, InputDocument, Queue};

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

/// Gefundener Platzhalter beim Öffnen (F-20/F-21).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlaceholderFound {
    /// leere Signaturfelder (`USE_FIELDS`)
    Fields(Vec<EmptyFieldCandidate>),
    /// QR-Platzhalter (`USE_MARKER`, id 1, SORTED)
    Qr(QrPlaceholder),
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
    /// Schreiben
    Writing,
}

/// Ziel von „Wiederholen“.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RetryTarget {
    /// neu vorbereiten (F-71: Retry → PrepareSigning)
    Prepare,
    /// erneut speichern (F-85)
    Save,
    /// Dokument erneut laden (F-15)
    Load,
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
    /// Schreibfehler
    #[error("Speichern fehlgeschlagen: {0}")]
    Save(String),
    /// Unerwartet
    #[error("unerwarteter Fehler: {0}")]
    Unexpected(String),
}

/// Option der Backend-Auswahl.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendOption {
    /// Backend
    pub id: BackendId,
    /// Fähigkeiten (`None` = nicht verfügbar)
    pub capabilities: Option<Capabilities>,
}

/// Unveränderliche Eingaben des Reducers (aus Einstellungen, Overlay, Richtlinie, Plattform).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    /// headless (CLI)
    pub headless: bool,
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
    /// Frage „Platzhalter verwenden?“ bzw. Feldauswahl
    PlaceholderQuestion {
        /// Warteschlange
        queue: Queue,
        /// Fund
        found: PlaceholderFound,
    },
    /// Positionierung
    Positioning {
        /// Warteschlange
        queue: Queue,
        /// aktuelle Seite (0-basiert)
        page: usize,
        /// gewählte/zuletzt bekannte Position (F-43)
        placement: Option<Placement>,
        /// virtuelle neue Seite aktiv (F-38)
        new_page: bool,
    },
    /// Backend-Auswahl
    ChoosingBackend {
        /// Warteschlange
        queue: Queue,
        /// Position
        position: PositionChoice,
        /// Optionen
        options: Vec<BackendOption>,
    },
    /// Datenschutzhinweis vor dem ersten Upload (`document_leaves_device`)
    PrivacyNotice {
        /// Warteschlange
        queue: Queue,
        /// Position
        position: PositionChoice,
        /// Backend
        backend: BackendId,
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
    },
    /// Ergebnis
    Output {
        /// Warteschlange
        queue: Queue,
        /// gespeichert unter (sonst Speichern-Knopf, F-86)
        saved: Option<OutputInfo>,
        /// offene Ausgabeentscheidung
        decision: Option<OutputDecision>,
    },
    /// Fehler mit Wiederholmöglichkeit
    Error {
        /// Warteschlange
        queue: Queue,
        /// Fehler
        error: WorkflowError,
        /// Wiederholen möglich
        retry: Option<RetryTarget>,
    },
    /// Programmende
    Finished {
        /// Exit-Code
        exit: ExitCode,
    },
}

impl State {
    /// Einstellungen sind ein Overlay; der Zustand selbst bleibt (F-101 ohne Re-Entry).
    pub fn settings_open(&self) -> bool {
        false
    }

    /// Hauptleisten-Zustände je State (docs/analyse/01 §3.5).
    pub fn main_bar(&self) -> [MainBarStep; 4] {
        todo!("Integration")
    }
}

/// Ereignis (UI, Effekt-Ergebnis, Lebenszyklus).
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
    },
    /// Analyse/Laden fehlgeschlagen
    AnalysisFailed {
        /// Dokument
        doc: DocId,
        /// Fehler
        error: WorkflowError,
    },
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
    /// Invariante verletzt
    SelfCheckFailed(InvariantViolation),
    /// Ausgabeziel entschieden bzw. Dialog beantwortet (`None` = abgebrochen)
    OutputChosen(Option<PathBuf>),
    /// Überschreib-Rückfrage
    OverwriteAnswered(bool),
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
    /// atomar schreiben
    Write(PathBuf),
    /// Ergebnis öffnen (Opener/QuickLook/ACTION_VIEW)
    OpenResult(PathBuf),
    /// Sitzung abbrechen (Passwort löschen, Benutzername bleibt)
    CancelSession,
    /// persistierten Zustand löschen
    DeletePersisted,
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

/// Reiner Reducer.
pub fn reduce(state: State, event: Event, ctx: &Context) -> Transition {
    todo!("Integration: Übergangstabelle docs/analyse/01 §3.4 als Tabellentest")
}
