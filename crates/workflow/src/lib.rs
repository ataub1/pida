//! `pdfover-workflow` – expliziter Reducer statt impliziter Re-Entry-Semantik (Plan §3.5;
//! docs/analyse/01 §3).
//!
//! `reduce(state, event, ctx) -> Transition { state, effects }` ist eine **reine Funktion**.
//! Rendern, HTTP und Dateioperationen laufen als [`Effect`] in Tokio-Tasks (`pdfover-core-api`) und
//! melden sich als [`Event`] zurück. Die Übergangstabelle aus docs/analyse/01 §3.4 wird direkt in
//! Tabellentests übertragen; dabei werden B-03 (INVISIBLE beim ersten Dokument), B-04 (Platzhalter-
//! Flags nicht zurückgesetzt), B-06, B-14 (Batch nach Speicherfehler) und B-15
//! (`IllegalStateException` als Abbruch) behoben.
//!
//! Zustände (ohne Re-Entry): `Starting → Idle → Analyzing → PlaceholderQuestion? → Positioning →
//! ChoosingBackend → PrivacyNotice? → Signing{phase} → Output → (nächstes Dokument | Idle | Finished)`.
//! Einstellungen sind ein Overlay ([`State::settings_open`]) und kein eigener Zustand.
//!
//! Ownership: **Integration**.

mod output;
mod queue;
mod state;

pub use output::{ExitCode, OutputDecision, PathProbe, decide_output, output_file_name};
pub use queue::{DocId, InputDocument, Queue};
pub use state::{
    BackendOption, Context, Effect, Event, MainBarStep, PlaceholderAnswer, PlaceholderFound, PositionChoice,
    RetryTarget, SigningPhase, State, Step, Transition, WorkflowError, reduce,
};
