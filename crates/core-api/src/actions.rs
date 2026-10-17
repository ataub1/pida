//! Freigaben der neuen Oberfläche (ADR 0004 §3.13): aus dem Workflow-Zustand berechnet wie
//! `State::main_bar`. Die UI aktiviert danach Knöpfe, Befehlspalette und Tastenkürzel.
//!
//! `start_batch`/`show_batch` setzt der Kern nur mit Stapelübersicht (`batch_overview`, K2:
//! [`batch_start_allowed`], [`batch_show_allowed`]). `sign` und `edit_placement` folgen der Regel aus §3.13
//! (Positionieren); K1 verfeinert `sign` um „Platzierung zulässig“. Die Freigabe des Prüfmodus (`verify`) ist mit
//! ihm entfallen (ADR 0004 Nachtrag 15).

use pida_workflow::{State, Step, Target};

use crate::view::ActionsView;

/// Freigaben zum Zustand; `settings_enabled` = `main_bar.settings_enabled` (berücksichtigt das offene
/// Einstellungs-Overlay).
pub(crate) fn actions_for(state: &State, settings_enabled: bool) -> ActionsView {
    let signing = matches!(state, State::Signing { .. } | State::Resuming { .. } | State::BatchSigning { .. });
    let positioning = matches!(state, State::Positioning { .. });
    ActionsView {
        open_files: !signing && !matches!(state, State::Starting | State::Finished { .. }),
        sign: positioning,
        edit_placement: positioning,
        start_batch: false,
        show_batch: false,
        settings: settings_enabled,
        home: state.step_enabled(Step::Open),
    }
}

/// Stapelübersicht mit mindestens einem signierbaren Eintrag (`Ready`/`Positioned`, ADR 0004 §3.13)?
pub(crate) fn batch_start_allowed(state: &State) -> bool {
    matches!(state, State::BatchOverview { queue } if queue.batch.as_ref().is_some_and(|b| b.signable() > 0))
}

/// „Zur Stapelübersicht“: Einzeldokument (Arbeitsbereich bzw. „Fertig“ nach dem Speichern) mit weiteren
/// Dokumenten in der Warteschlange oder aus der Übersicht geöffnet.
pub(crate) fn batch_show_allowed(state: &State) -> bool {
    let more = |q: &pida_workflow::Queue| !q.pending.is_empty();
    match state {
        State::Positioning { queue, .. } => more(queue) || state.opened_from_batch(),
        State::Output { queue, saved: Some(_), .. } => more(queue) && !state.opened_from_batch(),
        _ => false,
    }
}

/// Arbeitsbereich: ist „Signieren“ mit der aktuellen Platzierung zulässig (ADR 0004 §3.13)? Nicht bei einem
/// neuen sichtbaren Feld auf einem DocMDP-zertifizierten Dokument (nur vorhandene Felder oder unsichtbar)
/// und nicht bei einem Ziel, das es nicht gibt.
pub(crate) fn workspace_sign_allowed(state: &State) -> bool {
    let State::Positioning { doc, edit: Some(e), .. } = state else { return true };
    if e.invisible {
        return true;
    }
    match e.target {
        Target::Field(i) => i < e.fields.len(),
        Target::Qr => e.qr.is_some() && doc.new_visible_field_allowed,
        Target::Free => doc.new_visible_field_allowed,
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
        DocFacts {
            id: DocId(1),
            page_count: 1,
            signed: false,
            class: AcceptanceClass::K1Regular,
            start: None,
            new_visible_field_allowed: true,
        }
    }

    /// Tabelle Zustand → Freigaben (ADR 0004 §3.17 „Tabellentest Zustand → Freigaben“).
    #[test]
    fn actions_follow_the_state() {
        let q = Queue::default;
        let positioning =
            State::Positioning { queue: q(), page: 0, placement: None, new_page: false, doc: facts(), edit: None };
        let signing = State::Signing {
            queue: q(),
            position: PositionChoice::Auto,
            backend: BackendId::Keystore,
            phase: SigningPhase::Signing,
            doc: facts(),
            flow: SignFlow::default(),
            edit: None,
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
            edit: None,
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
            (State::BatchOverview { queue: q() }, true, false, true),
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
            assert!(!a.start_batch && !a.show_batch, "nur mit Stapelübersicht (core-api): {}", state.name());
            assert!(!actions_for(&state, false).settings);
        }
    }

    /// Arbeitsbereich: „Signieren“ nur mit zulässiger Platzierung (DocMDP: nur vorhandene Felder oder
    /// unsichtbar; Ziel muss existieren).
    #[test]
    fn workspace_sign_needs_an_allowed_placement() {
        use pida_workflow::{Target, WorkspaceEdit};
        let field = pida_placement::EmptyFieldCandidate {
            qualified_name: "Sig1".into(),
            field_id: (5, 0),
            widget_id: (5, 0),
            page_index: Some(0),
            rect: pida_pdf_cos::Rect { llx: 0.0, lly: 0.0, urx: 100.0, ury: 40.0 },
        };
        let state = |certified: bool, target: Target, invisible: bool| {
            let mut edit = WorkspaceEdit::new(vec![field.clone()], None, invisible);
            edit.target = target;
            let doc = DocFacts { new_visible_field_allowed: !certified, ..facts() };
            State::Positioning {
                queue: Queue::default(),
                page: 0,
                placement: None,
                new_page: false,
                doc,
                edit: Some(edit),
            }
        };
        assert!(workspace_sign_allowed(&state(false, Target::Free, false)));
        assert!(!workspace_sign_allowed(&state(true, Target::Free, false)), "DocMDP: kein neues sichtbares Feld");
        assert!(workspace_sign_allowed(&state(true, Target::Field(0), false)), "vorhandenes Feld");
        assert!(workspace_sign_allowed(&state(true, Target::Free, true)), "unsichtbar");
        assert!(!workspace_sign_allowed(&state(false, Target::Field(3), false)), "Feld fehlt");
        assert!(!workspace_sign_allowed(&state(false, Target::Qr, false)), "kein QR-Platzhalter");
    }
}
