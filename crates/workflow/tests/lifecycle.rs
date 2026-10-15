//! Mobile-Lebenszyklus (Plan §3.5, §5.2) als Tabellentests: App-Wechsel ist kein Abbruch,
//! „Signatur fortsetzen“ nach einem App-Kill ([`State::Resuming`]), Teilen und Exportieren.

mod common;

use common::*;
use pdfover_backend_api::BackendId;
use pdfover_pades::AcceptanceClass;
use pdfover_workflow::{
    Context, DocId, DocResult, Effect, Event, ExitCode, InputDocument, PlaceholderFound, PositionChoice, Queue,
    ResumePlan, SigningPhase, State, Step, WorkflowError,
};

fn gui() -> Context {
    Context::default()
}

fn idle() -> State {
    State::Idle { queue: Queue::default() }
}

fn plan(backend: BackendId) -> ResumePlan {
    ResumePlan { position: PositionChoice::Manual(start_placement(2)), backend }
}

fn resume(backend: BackendId, pending: Vec<InputDocument>) -> Event {
    Event::ResumeRequested { document: doc(1), plan: plan(backend), pending }
}

/// Ein Wechsel in die App „ID Austria“ (Hintergrund/Vordergrund) ist in jedem Zustand **kein Abbruch**:
/// weder Zustand noch Effekte ändern sich (keine `CancelSession`, kein `DeletePersisted`).
#[test]
fn app_switch_is_not_a_cancel_in_any_state() {
    let states = [
        idle(),
        positioning(&[1]),
        choosing(&[1], PositionChoice::Auto, false),
        signing(&[1], SigningPhase::OpeningSession, BackendId::AtrustNative, PositionChoice::Auto),
        signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto),
        signing(&[1], SigningPhase::Finishing, BackendId::Keystore, PositionChoice::Auto),
        output_state(&[1]),
        State::Resuming { queue: queue(&[1]), plan: plan(BackendId::AtrustNative), doc: None },
    ];
    for s in states {
        for event in [Event::Backgrounded, Event::Foregrounded, Event::Backgrounded, Event::Foregrounded] {
            let t = step(s.clone(), event, &gui());
            assert_eq!(t.state, s, "{}", s.name());
            assert!(t.effects.is_empty(), "{}: {:?}", s.name(), t.effects);
        }
    }
}

/// Nach der Rückkehr geht der Vorgang normal weiter (die Signatur kommt nach dem Vordergrund an).
#[test]
fn signing_continues_after_returning_from_the_id_austria_app() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    let (s, effects) = run(s, [Event::Backgrounded, Event::Foregrounded], &gui());
    assert!(effects.is_empty());
    let t = step(s, Event::PhaseCompleted(SigningPhase::Signing), &gui());
    assert_eq!(t.effects, vec![Effect::CloseSession, Effect::Finish]);
}

#[test]
fn resume_loads_document_and_signs_with_the_saved_backend() {
    let ctx = Context { privacy_acknowledged: vec![BackendId::AtrustNative], ..gui() };
    let t = step(idle(), resume(BackendId::AtrustNative, vec![doc(2)]), &ctx);
    let State::Resuming { queue, doc: None, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(queue.current_id(), Some(DocId(1)));
    assert_eq!(queue.pending.len(), 1, "wartende Dokumente des Batches bleiben");
    assert_eq!(t.effects, vec![Effect::Analyze(DocId(1))]);
    assert_eq!(t.state.step(), Some(Step::Signing));
    assert!(!t.state.settings_enabled());
    assert!(t.state.step_enabled(Step::Open) && !t.state.step_enabled(Step::Positioning));
    // Analyse: keine Platzhalterfrage, keine Warnung, keine Positionierung
    let mut event = analyzed(1, 2, Some(PlaceholderFound::Qr(qr())), false);
    if let Event::Analyzed { warnings, .. } = &mut event {
        warnings.push("pdf-usage-rights".into());
    }
    let t = step(t.state, event, &ctx);
    assert!(matches!(&t.state, State::Resuming { doc: Some(d), .. } if d.id == DocId(1)));
    assert_eq!(t.effects, vec![Effect::ProbeBackends]);
    assert!(t.state.doc().is_some());
    // gespeichertes Signiermittel verfügbar → direkt Signieren (Hinweis bereits bestätigt)
    let t = step(t.state, Event::BackendsProbed(options(false, true)), &ctx);
    let State::Signing { backend, position, phase, flow, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(*backend, BackendId::AtrustNative);
    assert_eq!(*position, PositionChoice::Manual(start_placement(2)));
    assert_eq!(*phase, SigningPhase::OpeningSession);
    assert!(flow.needs_certificate, "sichtbarer Block: Zertifikat wie beim ersten Versuch");
    assert_eq!(t.effects, vec![Effect::OpenSession(BackendId::AtrustNative)]);
}

#[test]
fn resume_asks_privacy_when_not_yet_acknowledged() {
    let events = [
        resume(BackendId::AtrustNative, vec![]),
        analyzed(1, 2, None, false),
        Event::BackendsProbed(options(false, false)),
    ];
    let (s, _) = run(idle(), events, &gui());
    assert!(matches!(s, State::PrivacyNotice { backend: BackendId::AtrustNative, .. }), "{s:?}");
}

#[test]
fn resume_with_unavailable_backend_shows_selection_and_discards_persisted_state() {
    let events = [
        resume(BackendId::Keystore, vec![]),
        analyzed(1, 2, None, false),
        Event::BackendsProbed(options(false, false)),
    ];
    let (s, effects) = run(idle(), events, &gui());
    let State::ChoosingBackend { auto_choice, position, .. } = &s else { panic!("{s:?}") };
    assert!(!auto_choice);
    assert_eq!(*position, PositionChoice::Manual(start_placement(2)));
    assert!(effects.contains(&Effect::Notify(pdfover_workflow::MSG_RESUME_BACKEND_UNAVAILABLE.into())));
    assert!(effects.contains(&Effect::DeletePersisted));
}

#[test]
fn resume_cancel_and_failure_discard_persisted_state() {
    let start = step(idle(), resume(BackendId::AtrustNative, vec![]), &gui()).state;
    // Abbrechen während des Ladens
    let t = step(start.clone(), Event::Cancel, &gui());
    assert!(matches!(&t.state, State::Idle { queue } if queue.done.len() == 1));
    assert!(t.effects.contains(&Effect::DeletePersisted));
    // Hauptleiste „Öffnen“
    let t = step(start.clone(), Event::Navigate(Step::Open), &gui());
    assert!(matches!(t.state, State::Idle { .. }));
    assert!(t.effects.contains(&Effect::DeletePersisted));
    // Dokument nicht mehr ladbar → Fehler ohne Wiederholen, Abbrechen → nächstes Dokument
    let error = WorkflowError::Rejected { class: AcceptanceClass::K5Rejected, code: "x".into() };
    let t = step(start.clone(), Event::AnalysisFailed { doc: DocId(1), error }, &gui());
    let State::Error { retry: None, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(t.effects, vec![Effect::DeletePersisted]);
    let t = step(t.state, Event::Cancel, &gui());
    assert!(matches!(&t.state, State::Idle { queue } if matches!(queue.done[0].result, DocResult::Failed { .. })));
    // abgelehnte Klasse
    let mut rejected = analyzed(1, 2, None, false);
    if let Event::Analyzed { class, .. } = &mut rejected {
        *class = AcceptanceClass::K5Rejected;
    }
    let t = step(start.clone(), rejected, &gui());
    assert!(matches!(t.state, State::Error { .. }));
    assert_eq!(t.effects, vec![Effect::DeletePersisted]);
    // Beenden
    let t = step(start, Event::Quit, &gui());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Cancelled));
    assert!(t.effects.contains(&Effect::DeletePersisted));
}

#[test]
fn resume_ignores_stale_and_unrelated_events() {
    let start = step(idle(), resume(BackendId::AtrustNative, vec![]), &gui()).state;
    for event in [analyzed(9, 1, None, false), Event::BackendsProbed(options(false, false)), Event::PositionConfirmed] {
        let t = step(start.clone(), event, &gui());
        assert_eq!(t.state, start);
        assert!(t.effects.is_empty());
    }
}

#[test]
fn resume_is_only_accepted_in_the_open_screen() {
    let states = [
        positioning(&[1]),
        output_state(&[1]),
        signing(&[1], SigningPhase::Signing, BackendId::Keystore, PositionChoice::Auto),
    ];
    for s in states {
        let t = step(s.clone(), resume(BackendId::Keystore, vec![]), &gui());
        assert_eq!(t.state, s);
        assert!(t.effects.is_empty());
    }
}

#[test]
fn successful_resumed_signature_deletes_persisted_state() {
    let ctx = Context { privacy_acknowledged: vec![BackendId::AtrustNative], ..gui() };
    let events = [
        resume(BackendId::AtrustNative, vec![]),
        analyzed(1, 2, None, false),
        Event::BackendsProbed(options(false, false)),
    ];
    let (s, _) = run(idle(), events, &ctx);
    let phases = [
        Event::PhaseCompleted(SigningPhase::OpeningSession),
        Event::PhaseCompleted(SigningPhase::ReadingCertificate),
        Event::PhaseCompleted(SigningPhase::Preparing),
        Event::PhaseCompleted(SigningPhase::Signing),
        Event::PhaseCompleted(SigningPhase::Finishing),
    ];
    let (s, effects) = run(s, phases, &ctx);
    assert!(matches!(s, State::Output { .. }));
    let prepare =
        Effect::Prepare { doc: DocId(1), position: PositionChoice::Manual(start_placement(2)), min_reserve: None };
    assert!(effects.contains(&prepare));
    assert!(effects.contains(&Effect::DeletePersisted));
}

#[test]
fn resumed_state_is_serializable() {
    let s = step(idle(), resume(BackendId::AtrustNative, vec![doc(2)]), &gui()).state;
    let json = serde_json::to_string(&s).unwrap();
    let back: State = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert_eq!(s.name(), "Resuming");
}

#[test]
fn share_and_export_only_after_saving() {
    let t = step(output_state(&[1]), Event::ShareResultRequested, &gui());
    assert!(t.effects.is_empty(), "nicht gespeichert: nichts zu teilen");
    let saved = step(output_state(&[1]), Event::Saved(output_info(1)), &gui()).state;
    let t = step(saved.clone(), Event::ShareResultRequested, &gui());
    assert_eq!(t.effects, vec![Effect::ShareResult("/in/doc1_signed.pdf".into())]);
    assert_eq!(t.state, saved);
    let t = step(saved.clone(), Event::ExportResultRequested, &gui());
    assert_eq!(t.effects, vec![Effect::ExportResult("/in/doc1_signed.pdf".into())]);
    assert_eq!(t.state, saved);
}
