//! `pida-workflow` – expliziter Reducer statt impliziter Re-Entry-Semantik (Plan §3.5;
//! docs/analyse/01 §3).
//!
//! `reduce(state, event, ctx) -> Transition { state, effects }` ist eine **reine Funktion**.
//! Rendern, HTTP und Dateioperationen laufen als [`Effect`] in Tokio-Tasks (`pida-core-api`) und
//! melden sich als [`Event`] zurück. Die Übergangstabelle aus docs/analyse/01 §3.4 ist direkt in
//! Tabellentests übertragen (`tests/transitions.rs`); dabei werden B-03 (INVISIBLE beim ersten
//! Dokument), B-04 (Platzhalter-Flags nicht zurückgesetzt), B-05 (Feldwahl dauerhaft gespeichert),
//! B-06 (Abbruch der Platzhalterfrage ohne Auswahl-Screen), B-10 (Dateiname ohne Erweiterung),
//! B-14 (Batch nach Speicherfehler) und B-15 (`IllegalStateException` als Abbruch: nur
//! `BackendError::Cancelled` gilt als Abbruch) behoben.
//!
//! Zustände (ohne Re-Entry): `Starting → Idle → Analyzing → AnalysisWarning? → PlaceholderQuestion? → Positioning? →
//! ChoosingBackend → PrivacyNotice? → Signing{phase} → Output → (nächstes Dokument | Idle | Finished)`;
//! auf Mobile zusätzlich `Idle → Resuming → PrivacyNotice? → Signing{phase}` („Signatur fortsetzen“).
//! Einstellungen sind ein Overlay ([`State::settings_open`]) und kein eigener Zustand.
//!
//! # Abbildung der PDF-Over-Übergänge (docs/analyse/01 §3.4)
//!
//! | PDF-Over | hier |
//! |---|---|
//! | PrepareConfiguration → Open | `Starting` + [`Event::Started`] → `Idle` bzw. `Analyzing` (Dateien aus CLI/„Öffnen mit“) |
//! | Init-Fehler, `error.Initialization` (Ja = Werksreset) | [`Event::SettingsUnreadable`] → `Error{Config, ResetSettings}`; Wiederholen → [`Effect::ResetSettings`], Abbrechen → Ende |
//! | Warnung der Analyse (neu, Plan §6.1: `/Perms /UR3`, Hybrid-XFA, Seed Values) | `Analyzed{warnings}` → `AnalysisWarning`; [`Event::WarningAnswered`] `true` → weiter, `false`/Abbrechen → Dokument verworfen; ohne Oberfläche nur Hinweis |
//! | Open: Feld „Ja“ / QR „Ja“ | `PlaceholderQuestion` + `PlaceholderAnswered{Yes}` → `ChoosingBackend` mit [`PositionChoice::Field`] bzw. [`PositionChoice::Placeholder`] (Positionierung entfällt) |
//! | Open: Feldauswahl abgebrochen | `Yes` ohne gültigen Index bei mehreren Feldern → Frage bleibt |
//! | Open: „Abbrechen“ | Dokument verworfen, nächstes Dokument bzw. `Idle` (B-06) |
//! | Open → Positioning | sonst; `invisible` (auch beim ersten Dokument, B-03) bzw. `auto_position` → direkt `ChoosingBackend` |
//! | Positioning: PDF nicht ladbar → Retry/Cancel | [`Event::AnalysisFailed`] → `Error{Load}`; Wiederholen lädt neu, Abbrechen verwirft das Dokument |
//! | Positioning: Composite-Fehler | [`Event::PreviewUnavailable`] → Hinweis `error-PositioningNotPossible`, automatische Position |
//! | Positioning: bestätigt | [`Event::PositionConfirmed`] → `ChoosingBackend` mit [`PositionChoice::Manual`] |
//! | Positioning: „Neue Seite“ auf signiertem Dokument | wie PDF-AS (docs/analyse/04 §4.5) dieselben Koordinaten auf der letzten Seite, zusätzlich Hinweis `placement.notice.new_page_not_allowed` |
//! | „Unsichtbar signieren“ (neu) | [`Event::InvisibleChosen`] → `ChoosingBackend` mit [`PositionChoice::Invisible`] (nur dieses Dokument) |
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
//! | ConfigurationUI | Overlay in `pida-core-api`; [`Event::SettingsChanged`] baut die Vorschau neu |
//! | Hauptleiste (`jumpToState`) | [`Event::Navigate`] nach der Matrix [`State::main_bar`] |
//!
//! # Mobile-Lebenszyklus (Plan §3.5, §5.2)
//!
//! | Ereignis | hier |
//! |---|---|
//! | App in den Hintergrund / zurück (z. B. Wechsel in die App „ID Austria“) | [`Event::Backgrounded`] / [`Event::Foregrounded`]: **kein Abbruch**, Zustand und Effekte unverändert (den Long-Poll startet `pida-core-api` nach der Rückkehr neu) |
//! | App vom System beendet, Neustart | [`Event::ResumeRequested`] (nur in `Idle`) → [`State::Resuming`]: Dokument laden, dann direkt das gespeicherte Signiermittel (Datenschutzhinweis nur, falls noch nicht bestätigt); ist es nicht verfügbar, Auswahl mit Hinweis [`MSG_RESUME_BACKEND_UNAVAILABLE`] und [`Effect::DeletePersisted`] |
//! | Erfolg, Abbruch, Fehler | [`Effect::DeletePersisted`] (der persistierte Zustand gilt nur für genau diesen Vorgang) |
//! | Ergebnis teilen / exportieren | [`Event::ShareResultRequested`] / [`Event::ExportResultRequested`] → [`Effect::ShareResult`] / [`Effect::ExportResult`] |
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
#[cfg(feature = "ts")]
pub mod ts_mirror;

pub use output::{
    ExitCode, FsProbe, OutputDecision, PathProbe, chosen_path, decide_output, output_file_name, resolve_without_dialog,
    save_dialog,
};
pub use queue::{DocId, DocOutcome, DocResult, InputDocument, Queue};
pub use reducer::{grown_reserve, reduce};
pub use state::{
    BackendOption, Context, DocFacts, Effect, Event, MSG_BACKEND_CHOICE_REQUIRED, MSG_CMS_TOO_LARGE,
    MSG_DEFAULT_BACKEND_UNAVAILABLE, MSG_INITIALIZATION, MSG_INVARIANT, MSG_NO_BACKEND, MSG_POSITIONING_NOT_POSSIBLE,
    MSG_RESUME_BACKEND_UNAVAILABLE, MSG_SAVE_FAILED, MSG_UNEXPECTED, MainBarStep, PlaceholderAnswer, PlaceholderFound,
    PositionChoice, ResumePlan, RetryTarget, SignFlow, SigningPhase, State, Step, Transition, WorkflowError,
    backend_for_choice,
};
