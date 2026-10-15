//! Freigaben der neuen Oberfläche (ADR 0004 §3.13): aus dem Workflow-Zustand berechnet wie
//! `State::main_bar`. Die UI aktiviert danach Knöpfe, Befehlspalette und Tastenkürzel.
//!
//! Stand E0 (Paket K0): Freigaben für Ereignisse, die der Kern noch ignoriert, bleiben aus –
//! `start_batch`/`show_batch` bis K2 (Stapelübersicht, `batch_overview`), `verify` bis K4 (Prüfmodus).
//! `sign` und `edit_placement` folgen der Regel aus §3.13 (Positionieren); K1 verfeinert `sign` um
//! „Platzierung zulässig“.

use pida_workflow::{State, Step};

use crate::view::ActionsView;

/// Freigaben zum Zustand; `settings_enabled` = `main_bar.settings_enabled` (berücksichtigt das offene
/// Einstellungs-Overlay).
pub(crate) fn actions_for(state: &State, settings_enabled: bool) -> ActionsView {
    let signing = matches!(state, State::Signing { .. } | State::Resuming { .. });
    let positioning = matches!(state, State::Positioning { .. });
    ActionsView {
        open_files: !signing && !matches!(state, State::Starting | State::Finished { .. }),
        sign: positioning,
        edit_placement: positioning,
        start_batch: false,
        show_batch: false,
        verify: false,
        settings: settings_enabled,
        home: state.step_enabled(Step::Open),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_backend_api::BackendId;
    use pida_pades::AcceptanceClass;
    use pida_workflow::{
        DocFacts, DocId, ExitCode, PositionChoice, Queue, ResumePlan, SignFlow, SigningPhase, WorkflowError,
    };

    fn facts() -> DocFacts {
        DocFacts { id: DocId(1), page_count: 1, signed: false, class: AcceptanceClass::K1Regular, start: None }
    }

    /// Tabelle Zustand → Freigaben (ADR 0004 §3.17 „Tabellentest Zustand → Freigaben“).
    #[test]
    fn actions_follow_the_state() {
        let q = Queue::default;
        let positioning = State::Positioning { queue: q(), page: 0, placement: None, new_page: false, doc: facts() };
        let signing = State::Signing {
            queue: q(),
            position: PositionChoice::Auto,
            backend: BackendId::Keystore,
            phase: SigningPhase::Signing,
            doc: facts(),
            flow: SignFlow::default(),
        };
        let resuming = State::Resuming {
            queue: q(),
            plan: ResumePlan { position: PositionChoice::Auto, backend: BackendId::AtrustNative },
            doc: None,
        };
        let output = State::Output {
            queue: q(),
            saved: None,
            decision: None,
            doc: facts(),
            position: PositionChoice::Auto,
            backend: BackendId::Keystore,
        };
        let error = State::Error {
            queue: q(),
            error: WorkflowError::Unexpected("x".into()),
            retry: None,
            from: Some(Box::new(positioning.clone())),
        };
        //                      (Zustand, open_files, sign/edit, home)
        let table: Vec<(State, bool, bool, bool)> = vec![
            (State::Starting, false, false, false),
            (State::Idle { queue: q() }, true, false, true),
            (State::Analyzing { queue: q() }, true, false, true),
            (positioning, true, true, true),
            (signing, false, false, false),
            (resuming, false, false, true),
            (output, true, false, true),
            (error, true, false, false),
            (State::Finished { exit: ExitCode::Success, results: Vec::new() }, false, false, false),
        ];
        for (state, open_files, sign, home) in table {
            let a = actions_for(&state, true);
            assert_eq!(
                (a.open_files, a.sign, a.edit_placement, a.home),
                (open_files, sign, sign, home),
                "{}",
                state.name()
            );
            assert!(a.settings);
            assert!(!a.start_batch && !a.show_batch && !a.verify, "erst ab K2/K4: {}", state.name());
            assert!(!actions_for(&state, false).settings);
        }
    }
}
