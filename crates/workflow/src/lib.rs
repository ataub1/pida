//! `pdfover-workflow` – expliziter Reducer statt impliziter Re-Entry-Semantik (Plan §3.5;
//! docs/analyse/01 §3).
//!
//! `reduce(state, event, ctx) -> Transition { state, effects }` ist eine **reine Funktion**.
//! Rendern, HTTP und Dateioperationen laufen als [`Effect`] in Tokio-Tasks (`pdfover-core-api`) und
//! melden sich als [`Event`] zurück. Die Übergangstabelle aus docs/analyse/01 §3.4 ist direkt in
//! Tabellentests übertragen (`tests/transitions.rs`); dabei werden B-03 (INVISIBLE beim ersten
//! Dokument), B-04 (Platzhalter-Flags nicht zurückgesetzt), B-05 (Feldwahl dauerhaft gespeichert),
//! B-06 (Abbruch der Platzhalterfrage ohne Auswahl-Screen), B-10 (Dateiname ohne Erweiterung),
//! B-14 (Batch nach Speicherfehler) und B-15 (`IllegalStateException` als Abbruch: nur
//! `BackendError::Cancelled` gilt als Abbruch) behoben.
//!
//! Zustände (ohne Re-Entry): `Starting → Idle → Analyzing → PlaceholderQuestion? → Positioning? →
//! ChoosingBackend → PrivacyNotice? → Signing{phase} → Output → (nächstes Dokument | Idle | Finished)`.
//! Einstellungen sind ein Overlay ([`State::settings_open`]) und kein eigener Zustand.
//!
//! # Abbildung der PDF-Over-Übergänge (docs/analyse/01 §3.4)
//!
//! | PDF-Over | hier |
//! |---|---|
//! | PrepareConfiguration → Open | `Starting` + [`Event::Started`] → `Idle` bzw. `Analyzing` (Dateien aus CLI/„Öffnen mit“) |
//! | Init-Fehler, `error.Initialization` (Ja = Werksreset) | [`Event::SettingsUnreadable`] → `Error{Config, ResetSettings}`; Wiederholen → [`Effect::ResetSettings`], Abbrechen → Ende |
//! | Open: Feld „Ja“ / QR „Ja“ | `PlaceholderQuestion` + `PlaceholderAnswered{Yes}` → `ChoosingBackend` mit [`PositionChoice::Field`] bzw. [`PositionChoice::Placeholder`] (Positionierung entfällt) |
//! | Open: Feldauswahl abgebrochen | `Yes` ohne gültigen Index bei mehreren Feldern → Frage bleibt |
//! | Open: „Abbrechen“ | Dokument verworfen, nächstes Dokument bzw. `Idle` (B-06) |
//! | Open → Positioning | sonst; `invisible` (auch beim ersten Dokument, B-03) bzw. `auto_position` → direkt `ChoosingBackend` |
//! | Positioning: PDF nicht ladbar → Retry/Cancel | [`Event::AnalysisFailed`] → `Error{Load}`; Wiederholen lädt neu, Abbrechen verwirft das Dokument |
//! | Positioning: Composite-Fehler | [`Event::PreviewUnavailable`] → Hinweis `error-PositioningNotPossible`, automatische Position |
//! | Positioning: bestätigt | [`Event::PositionConfirmed`] → `ChoosingBackend` mit [`PositionChoice::Manual`] |
//! | BKUSelection: Default/automatisch/Auswahl | [`Event::BackendsProbed`] wendet Standard-Backend (F-53) bzw. „nur ID Austria verfügbar“ (F-54) an; [`Event::BackendChosen`] |
//! | Datenschutzhinweis (neu, Plan §3.3) | `document_leaves_device` und nicht bestätigt → `PrivacyNotice`; „Ja“ speichert die Bestätigung ([`Effect::AcknowledgePrivacy`]) |
//! | PrepareSigning/LocalBKU/MobileBKU/KS/Signing | `Signing{OpeningSession → ReadingCertificate? → Preparing → Signing → Finishing}` |
//! | Signing: Erfolg → Output | `Finishing` fertig → `Output` + [`Effect::DecideOutput`] |
//! | Signing: Nutzerabbruch → BKUSelection | `BackendError::Cancelled` bzw. [`Event::Cancel`] → `ChoosingBackend` ohne automatische Wahl (F-55, F-72) |
//! | Signing: Fehler → Retry/Cancel | `Error{Backend, Prepare}`; Wiederholen öffnet die Sitzung neu (neu vorbereiten), Abbrechen → `ChoosingBackend` |
//! | CMS zu groß (neu, Plan §6.2) | [`Event::CmsTooLarge`] → `Error{CmsTooLarge, PrepareWithReserve(n)}` |
//! | Output: gespeichert, Queue nicht leer → Open | [`Event::Saved`] → nächstes Dokument (F-88) |
//! | Output: gespeichert, `-s` → Ende | [`Event::Saved`] + `skip_finish` → `Finished` (F-90) |
//! | Output: Dateien abgelegt | [`Event::FilesAdded`] im Ergebnis-Screen nach erfolgreichem Speichern → sofort weiter (F-89) |
//! | Output: Überschreiben/Speichern-Dialog/Schreibfehler | `OutputDecided` → `Write`/`AskOverwrite`/`AskSavePath`; `SaveFailed` → `Error{Save}` |
//! | ConfigurationUI | Overlay in `pdfover-core-api`; [`Event::SettingsChanged`] baut die Vorschau neu |
//! | Hauptleiste (`jumpToState`) | [`Event::Navigate`] nach der Matrix [`State::main_bar`] |
//!
//! Ohne grafische Oberfläche ([`Context::headless`], CLI) gelten feste Regeln: automatische
//! Positionierung, QR-Platzhalter bzw. genau ein leeres Feld werden verwendet, kein Speichern-Dialog
//! (neben der Quelle), Fehler werden je Dokument verbucht ([`queue::DocResult`]) und es geht mit dem
//! nächsten Dokument weiter; nach dem letzten Dokument endet der Ablauf mit dem Exit-Code des ersten
//! fehlgeschlagenen Dokuments.
//!
//! Ownership: **Integration**.

mod output;
pub mod queue;
mod reducer;
mod state;

pub use output::{
    ExitCode, FsProbe, OutputDecision, PathProbe, chosen_path, decide_output, output_file_name, resolve_without_dialog,
    save_dialog,
};
pub use queue::{DocId, DocOutcome, DocResult, InputDocument, Queue};
pub use reducer::{grown_reserve, reduce};
pub use state::{
    BackendOption, Context, DocFacts, Effect, Event, MSG_BACKEND_CHOICE_REQUIRED, MSG_CMS_TOO_LARGE,
    MSG_DEFAULT_BACKEND_UNAVAILABLE, MSG_INITIALIZATION, MSG_INVARIANT, MSG_NO_BACKEND, MSG_POSITIONING_NOT_POSSIBLE,
    MSG_SAVE_FAILED, MSG_UNEXPECTED, MainBarStep, PlaceholderAnswer, PlaceholderFound, PositionChoice, RetryTarget,
    SignFlow, SigningPhase, State, Step, Transition, WorkflowError, backend_for_choice,
};
