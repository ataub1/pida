//! Tabellentests: jeder Übergang aus docs/analyse/01 §3.4 (und die NG-Ergänzungen aus Plan §3.5,
//! §6.2, ADR 0001 §8.8) als eigener Fall.

mod common;

use common::*;
use pdfover_backend_api::{BackendError, BackendId};
use pdfover_config::BackendChoice;
use pdfover_pades::{AcceptanceClass, Invariant, InvariantViolation};
use pdfover_workflow::{
    Context, DocId, DocResult, Effect, Event, ExitCode, OutputDecision, PlaceholderAnswer, PlaceholderFound,
    PositionChoice, Queue, RetryTarget, SigningPhase, State, Step, WorkflowError, grown_reserve,
};

fn gui() -> Context {
    Context::default()
}

fn headless() -> Context {
    Context { headless: true, ..Context::default() }
}

// ---------------------------------------------------------------------------------------------
// PrepareConfiguration
// ---------------------------------------------------------------------------------------------

#[test]
fn start_without_documents_goes_to_idle() {
    let t = step(State::Starting, Event::Started { documents: vec![] }, &gui());
    assert_eq!(t.state, State::Idle { queue: Queue::default() });
    assert!(t.effects.is_empty());
}

#[test]
fn start_with_cli_documents_analyzes_first() {
    let t = step(State::Starting, Event::Started { documents: vec![doc(1), doc(2)] }, &gui());
    let State::Analyzing { queue } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(queue.current_id(), Some(DocId(1)));
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(t.effects, vec![Effect::Analyze(DocId(1))]);
}

#[test]
fn unreadable_settings_offer_reset() {
    let t = step(State::Starting, Event::SettingsUnreadable("kaputt".into()), &gui());
    let State::Error { error, retry, .. } = &t.state else { panic!() };
    assert_eq!(error, &WorkflowError::Config("kaputt".into()));
    assert_eq!(*retry, Some(RetryTarget::ResetSettings));
    assert_eq!(error.code(), "error-Initialization");
    // Ja = Werksreset, danach Neustart des Kerns
    let r = step(t.state.clone(), Event::Retry, &gui());
    assert_eq!(r.state, State::Starting);
    assert_eq!(r.effects, vec![Effect::ResetSettings]);
    // Nein = Ende
    let c = step(t.state, Event::Cancel, &gui());
    assert_eq!(c.state.exit_code(), Some(ExitCode::General));
    assert_eq!(c.effects, vec![Effect::Exit(ExitCode::General)]);
}

#[test]
fn unreadable_settings_headless_end_immediately() {
    let t = step(State::Starting, Event::SettingsUnreadable("x".into()), &headless());
    assert_eq!(t.state.exit_code(), Some(ExitCode::General));
}

// ---------------------------------------------------------------------------------------------
// Open
// ---------------------------------------------------------------------------------------------

#[test]
fn open_waits_without_selection() {
    let s = State::Idle { queue: Queue::default() };
    let t = step(s.clone(), Event::Navigate(Step::Open), &gui());
    assert_eq!(t.state, s);
    assert!(t.effects.is_empty());
}

#[test]
fn files_added_in_idle_start_analysis_and_remember_directory() {
    let t = step(State::Idle { queue: Queue::default() }, Event::FilesAdded(vec![doc(1), doc(2)]), &gui());
    assert!(matches!(t.state, State::Analyzing { .. }));
    assert_eq!(t.effects, vec![Effect::RememberDirectory("/in".into()), Effect::Analyze(DocId(1))]);
}

#[test]
fn analysis_without_placeholder_goes_to_positioning_on_last_page() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let t = step(s, analyzed(1, 3, None, false), &gui());
    let State::Positioning { page, placement, new_page, doc, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(*page, 2, "F-32: Startseite = letzte Seite");
    assert_eq!(placement.unwrap().page_index, 2);
    assert!(!new_page);
    assert_eq!(doc.page_count, 3);
    assert_eq!(t.effects, vec![Effect::RenderPage { doc: DocId(1), page: 2 }, Effect::BuildBlockPreview(DocId(1))]);
}

#[test]
fn signature_field_yes_single_field_skips_positioning() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let ctx = Context { placeholder_search: true, placeholder_fields: true, ..gui() };
    let t = step(s, analyzed(1, 1, Some(PlaceholderFound::Fields(vec![field("Sig1")])), false), &ctx);
    assert!(matches!(t.state, State::PlaceholderQuestion { .. }));
    let t = step(t.state, Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index: None }, &ctx);
    let State::ChoosingBackend { position, auto_choice, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(position, &PositionChoice::Field(field("Sig1")));
    assert!(*auto_choice);
    assert_eq!(t.effects, vec![Effect::ProbeBackends]);
}

#[test]
fn signature_field_yes_with_selection_from_several() {
    let ctx = Context { placeholder_search: true, placeholder_fields: true, ..gui() };
    let found = PlaceholderFound::Fields(vec![field("A"), field("B")]);
    let s = State::PlaceholderQuestion { queue: queue(&[1]), found, doc: facts(1, 1) };
    let t = step(s, Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index: Some(1) }, &ctx);
    let State::ChoosingBackend { position, .. } = &t.state else { panic!() };
    assert_eq!(position, &PositionChoice::Field(field("B")));
}

#[test]
fn field_selection_cancelled_asks_again() {
    let ctx = Context { placeholder_search: true, ..gui() };
    let found = PlaceholderFound::Fields(vec![field("A"), field("B")]);
    let s = State::PlaceholderQuestion { queue: queue(&[1]), found, doc: facts(1, 1) };
    // mehrere Felder, kein bzw. ungültiger Index → Frage bleibt (OpenState.java:165-171)
    let t = step(s.clone(), Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index: None }, &ctx);
    assert_eq!(t.state, s);
    let t = step(s.clone(), Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index: Some(7) }, &ctx);
    assert_eq!(t.state, s);
}

#[test]
fn qr_placeholder_yes_skips_positioning() {
    let ctx = Context { placeholder_search: true, ..gui() };
    let s = State::Analyzing { queue: queue(&[1]) };
    let t = step(s, analyzed(1, 1, Some(PlaceholderFound::Qr(qr())), false), &ctx);
    let t = step(t.state, Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index: None }, &ctx);
    let State::ChoosingBackend { position, .. } = &t.state else { panic!() };
    assert_eq!(position, &PositionChoice::Placeholder(qr()));
}

#[test]
fn placeholder_no_goes_to_normal_positioning() {
    let ctx = Context { placeholder_search: true, ..gui() };
    let s = State::PlaceholderQuestion { queue: queue(&[1]), found: PlaceholderFound::Qr(qr()), doc: facts(1, 2) };
    let t = step(s, Event::PlaceholderAnswered { answer: PlaceholderAnswer::No, field_index: None }, &ctx);
    assert!(matches!(t.state, State::Positioning { page: 1, .. }));
}

#[test]
fn placeholder_cancel_discards_document_and_continues_b06() {
    let ctx = Context { placeholder_search: true, ..gui() };
    let s = State::PlaceholderQuestion { queue: queue(&[1, 2]), found: PlaceholderFound::Qr(qr()), doc: facts(1, 1) };
    let t = step(s, Event::PlaceholderAnswered { answer: PlaceholderAnswer::Cancel, field_index: None }, &ctx);
    let State::Analyzing { queue: q } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(q.current_id(), Some(DocId(2)));
    assert_eq!(q.done[0].result, DocResult::Cancelled);
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1)), Effect::Analyze(DocId(2))]);
    // letztes Dokument: zurück zur Auswahl (B-06: der Auswahl-Screen ist erreichbar)
    let s = State::PlaceholderQuestion { queue: queue(&[1]), found: PlaceholderFound::Qr(qr()), doc: facts(1, 1) };
    let t = step(s, Event::Cancel, &ctx);
    assert!(matches!(t.state, State::Idle { .. }));
}

#[test]
fn placeholder_search_disabled_ignores_found_placeholder() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let t = step(s, analyzed(1, 1, Some(PlaceholderFound::Qr(qr())), false), &gui());
    assert!(matches!(t.state, State::Positioning { .. }));
}

#[test]
fn empty_field_list_does_not_ask() {
    let ctx = Context { placeholder_search: true, placeholder_fields: true, ..gui() };
    let s = State::Analyzing { queue: queue(&[1]) };
    let t = step(s, analyzed(1, 1, Some(PlaceholderFound::Fields(vec![])), false), &ctx);
    assert!(matches!(t.state, State::Positioning { .. }));
}

#[test]
fn invisible_profile_skips_positioning_even_for_first_document_b03() {
    let ctx = Context { invisible: true, placeholder_search: true, ..gui() };
    let t = step(State::Starting, Event::Started { documents: vec![doc(1)] }, &ctx);
    let t = step(t.state, analyzed(1, 2, Some(PlaceholderFound::Qr(qr())), false), &ctx);
    let State::ChoosingBackend { position, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(position, &PositionChoice::Invisible);
}

#[test]
fn auto_position_skips_positioning() {
    let ctx = Context { auto_position: true, ..gui() };
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed(1, 2, None, false), &ctx);
    let State::ChoosingBackend { position, .. } = &t.state else { panic!() };
    assert_eq!(position, &PositionChoice::Auto);
}

#[test]
fn placeholder_choice_is_per_document_b04_b05() {
    // Dokument 1 mit QR-Platzhalter, Dokument 2 ohne: Dokument 2 wird normal positioniert.
    let ctx = Context { placeholder_search: true, ..gui() };
    let s = State::Analyzing { queue: queue(&[1, 2]) };
    let t = step(s, analyzed(1, 1, Some(PlaceholderFound::Qr(qr())), false), &ctx);
    let t = step(t.state, Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index: None }, &ctx);
    assert!(matches!(&t.state, State::ChoosingBackend { position: PositionChoice::Placeholder(_), .. }));
    // Dokument 1 abschließen → Dokument 2
    let s = State::Output {
        queue: t.state.queue().unwrap().clone(),
        saved: None,
        decision: None,
        doc: facts(1, 1),
        position: PositionChoice::Placeholder(qr()),
        backend: BackendId::Keystore,
    };
    let t = step(s, Event::Saved(output_info(1)), &ctx);
    let t = step(t.state, analyzed(2, 1, None, false), &ctx);
    assert!(matches!(t.state, State::Positioning { .. }), "{:?}", t.state);
}

/// Plan §6.1: `/Perms /UR3`, Hybrid-XFA, Seed Values → Warnung mit Abbruchmöglichkeit vor dem Weiter.
#[test]
fn analysis_warning_can_be_accepted_or_aborts() {
    let warned_with = |id, placeholder| Event::Analyzed {
        doc: DocId(id),
        class: AcceptanceClass::K1Regular,
        placeholder,
        signed: false,
        page_count: 3,
        start: Some(start_placement(3)),
        warnings: vec!["pdf-usage-rights".into()],
    };
    let warned = |id| warned_with(id, None);
    let t = step(State::Analyzing { queue: queue(&[1, 2]) }, warned(1), &gui());
    let State::AnalysisWarning { warnings, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(warnings, &vec!["pdf-usage-rights".to_owned()]);
    assert!(t.effects.is_empty());
    assert!(t.state.settings_enabled());
    assert_eq!(t.state.step(), Some(Step::Open));
    // Fortfahren → normaler Ablauf (Positionierung auf der letzten Seite)
    let y = step(t.state.clone(), Event::WarningAnswered(true), &gui());
    assert!(matches!(y.state, State::Positioning { page: 2, .. }), "{:?}", y.state);
    // Abbrechen → Dokument verworfen, nächstes Dokument
    let n = step(t.state.clone(), Event::WarningAnswered(false), &gui());
    let State::Analyzing { queue: q } = &n.state else { panic!("{:?}", n.state) };
    assert_eq!(q.current_id(), Some(DocId(2)));
    assert_eq!(q.done[0].result, DocResult::Cancelled);
    assert_eq!(n.effects, vec![Effect::Discard(DocId(1)), Effect::Analyze(DocId(2))]);
    // Platzhalter gilt nach dem Fortfahren
    let ctx = Context { placeholder_search: true, ..gui() };
    let ev = warned_with(1, Some(PlaceholderFound::Fields(vec![field("Sig1")])));
    let t = step(State::Analyzing { queue: queue(&[1]) }, ev, &ctx);
    let y = step(t.state, Event::WarningAnswered(true), &ctx);
    assert!(matches!(y.state, State::PlaceholderQuestion { .. }), "{:?}", y.state);
    // ohne Oberfläche: keine Rückfrage (der Kern zeigt die Warnung als Hinweis)
    let h = step(State::Analyzing { queue: queue(&[1]) }, warned(1), &headless());
    assert!(matches!(h.state, State::ChoosingBackend { position: PositionChoice::Auto, .. }), "{:?}", h.state);
}

#[test]
fn rejected_document_shows_error_without_retry() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let ev = Event::Analyzed {
        doc: DocId(1),
        class: AcceptanceClass::K5Rejected,
        placeholder: None,
        signed: false,
        page_count: 1,
        start: None,
        warnings: Vec::new(),
    };
    let t = step(s, ev, &gui());
    let State::Error { error, retry, .. } = &t.state else { panic!() };
    assert!(matches!(error, WorkflowError::Rejected { class: AcceptanceClass::K5Rejected, .. }));
    assert_eq!(*retry, None);
    assert_eq!(error.exit_code(), ExitCode::Rejected);
}

#[test]
fn stale_analysis_result_is_ignored() {
    let s = State::Analyzing { queue: queue(&[2]) };
    let t = step(s.clone(), analyzed(1, 1, None, false), &gui());
    assert_eq!(t.state, s);
}

// ---------------------------------------------------------------------------------------------
// Positioning
// ---------------------------------------------------------------------------------------------

#[test]
fn pdf_not_loadable_retry_reloads() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let err = WorkflowError::Rejected { class: AcceptanceClass::K1Regular, code: "error-MayNotBeAPDF".into() };
    let t = step(s, Event::AnalysisFailed { doc: DocId(1), error: err }, &gui());
    let State::Error { retry, .. } = &t.state else { panic!() };
    assert_eq!(*retry, Some(RetryTarget::Load));
    let r = step(t.state, Event::Retry, &gui());
    assert!(matches!(r.state, State::Analyzing { .. }));
    assert_eq!(r.effects, vec![Effect::Analyze(DocId(1))]);
}

#[test]
fn pdf_not_loadable_cancel_goes_back_to_open() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let err = WorkflowError::Rejected { class: AcceptanceClass::K1Regular, code: "error-PDFPwdProtected".into() };
    let t = step(s, Event::AnalysisFailed { doc: DocId(1), error: err.clone() }, &gui());
    let c = step(t.state, Event::Cancel, &gui());
    let State::Idle { queue } = &c.state else { panic!("{:?}", c.state) };
    assert_eq!(queue.done[0].result, DocResult::Failed { error: err });
}

#[test]
fn preview_unavailable_notifies_and_positions_automatically() {
    let t = step(positioning(&[1]), Event::PreviewUnavailable { doc: DocId(1) }, &gui());
    let State::ChoosingBackend { position, .. } = &t.state else { panic!() };
    assert_eq!(position, &PositionChoice::Auto);
    assert_eq!(t.effects, vec![Effect::Notify("error-PositioningNotPossible".into()), Effect::ProbeBackends]);
}

#[test]
fn no_start_placement_means_automatic_position() {
    let s = State::Analyzing { queue: queue(&[1]) };
    let ev = Event::Analyzed {
        doc: DocId(1),
        class: AcceptanceClass::K1Regular,
        placeholder: None,
        signed: false,
        page_count: 2,
        start: None,
        warnings: Vec::new(),
    };
    let t = step(s, ev, &gui());
    assert!(matches!(&t.state, State::ChoosingBackend { position: PositionChoice::Auto, .. }));
    assert_eq!(t.effects[0], Effect::Notify("error-PositioningNotPossible".into()));
}

#[test]
fn user_confirms_position() {
    let mut s = positioning(&[1]);
    let p = pdfover_placement::Placement { page_index: 0, x: 10.0, y: 300.0, width: 230.0, height: 49.0 };
    s = step(s, Event::PlacementChanged(p), &gui()).state;
    let State::Positioning { page, .. } = &s else { panic!() };
    assert_eq!(*page, 0);
    let t = step(s, Event::PositionConfirmed, &gui());
    let State::ChoosingBackend { position, auto_choice, .. } = &t.state else { panic!() };
    assert_eq!(position, &PositionChoice::Manual(p));
    assert!(*auto_choice);
}

#[test]
fn page_navigation_moves_block_and_renders() {
    let t = step(positioning(&[1]), Event::PageChanged(0), &gui());
    let State::Positioning { page, placement, .. } = &t.state else { panic!() };
    assert_eq!(*page, 0);
    assert_eq!(placement.unwrap().page_index, 0);
    // Blockvorschau für die neue Seite neu (Breite hängt bei Profilen ohne `w` von der Seite ab, KR-F01)
    assert_eq!(t.effects, vec![Effect::RenderPage { doc: DocId(1), page: 0 }, Effect::BuildBlockPreview(DocId(1))]);
    // außerhalb des Bereichs ignoriert (ohne neue Seite höchstens letzte Seite)
    let t2 = step(t.state.clone(), Event::PageChanged(3), &gui());
    assert_eq!(t2.state, t.state);
}

#[test]
fn new_page_toggle_and_undo() {
    let t = step(positioning(&[1]), Event::NewPageToggled, &gui());
    let State::Positioning { page, placement, new_page, .. } = &t.state else { panic!() };
    assert!(*new_page);
    assert_eq!(*page, 3, "virtuelle Seite numPages");
    assert_eq!(placement.unwrap().page_index, 3);
    assert_eq!(t.effects, vec![Effect::BuildBlockPreview(DocId(1))]);
    // Bestätigen → manuelle Position auf der neuen Seite
    let c = step(t.state.clone(), Event::PositionConfirmed, &gui());
    let State::ChoosingBackend { position: PositionChoice::Manual(p), .. } = &c.state else { panic!() };
    assert_eq!(p.page_index, 3);
    // rückgängig
    let u = step(t.state, Event::NewPageToggled, &gui());
    let State::Positioning { page, new_page, placement, .. } = &u.state else { panic!() };
    assert!(!new_page);
    assert_eq!(*page, 2);
    assert_eq!(placement.unwrap().page_index, 2);
    assert_eq!(u.effects, vec![Effect::RenderPage { doc: DocId(1), page: 2 }, Effect::BuildBlockPreview(DocId(1))]);
}

#[test]
fn new_page_on_signed_documents_falls_back_to_last_page_with_notice() {
    let State::Positioning { queue, page, placement, doc, .. } = positioning(&[1]) else { unreachable!() };
    let doc = pdfover_workflow::DocFacts { signed: true, ..doc };
    let s = State::Positioning { queue, page, placement, new_page: false, doc };
    // „Neue Seite“ bleibt auf der letzten Seite (PDF-AS, docs/analyse/04 §4.5) – mit Hinweis
    let t = step(s.clone(), Event::NewPageToggled, &gui());
    assert_eq!(t.effects, vec![Effect::Notify("placement.notice.new_page_not_allowed".into())]);
    let State::Positioning { new_page: false, page: 2, placement: Some(p), .. } = &t.state else { panic!("{t:?}") };
    assert_eq!(p.page_index, 2);
    // eine Platzierung auf der virtuellen Seite landet mit denselben Koordinaten auf der letzten Seite
    let p = pdfover_placement::Placement { page_index: 3, x: 1.0, y: 100.0, width: 10.0, height: 10.0 };
    let t = step(s.clone(), Event::PlacementChanged(p), &gui());
    let State::Positioning { page: 2, placement: Some(moved), new_page: false, .. } = &t.state else { panic!("{t:?}") };
    assert_eq!(*moved, pdfover_placement::Placement { page_index: 2, ..p });
    assert_eq!(t.effects, vec![Effect::Notify("placement.notice.new_page_not_allowed".into())]);
    // von einer anderen Seite aus: letzte Seite rendern und Vorschau neu bauen
    let s0 = step(s, Event::PageChanged(0), &gui()).state;
    let t = step(s0, Event::NewPageToggled, &gui());
    assert!(matches!(t.state, State::Positioning { page: 2, new_page: false, .. }));
    assert_eq!(
        t.effects,
        vec![
            Effect::Notify("placement.notice.new_page_not_allowed".into()),
            Effect::RenderPage { doc: DocId(1), page: 2 },
            Effect::BuildBlockPreview(DocId(1)),
        ]
    );
    // Bestätigen → manuelle Position auf der letzten Seite (keine neue Seite)
    let c = step(t.state, Event::PositionConfirmed, &gui());
    let State::ChoosingBackend { position: PositionChoice::Manual(p), .. } = &c.state else { panic!() };
    assert_eq!(p.page_index, 2);
}

#[test]
fn invisible_can_be_chosen_per_document() {
    let t = step(positioning(&[1, 2]), Event::InvisibleChosen, &gui());
    let State::ChoosingBackend { position, auto_choice, .. } = &t.state else { panic!("{t:?}") };
    assert_eq!(position, &PositionChoice::Invisible);
    assert!(*auto_choice);
    assert_eq!(t.effects, vec![Effect::ProbeBackends]);
    // zurück zur Positionierung: Startposition (keine manuelle Position gemerkt)
    let back = step(t.state, Event::Navigate(Step::Positioning), &gui());
    assert!(matches!(back.state, State::Positioning { .. }), "{back:?}");
    // außerhalb der Positionierung ohne Wirkung
    let s = State::Idle { queue: Queue::default() };
    assert_eq!(step(s.clone(), Event::InvisibleChosen, &gui()).state, s);
}

#[test]
fn settings_change_rebuilds_block_preview() {
    let t = step(positioning(&[1]), Event::SettingsChanged, &gui());
    assert!(matches!(t.state, State::Positioning { .. }));
    assert_eq!(t.effects, vec![Effect::BuildBlockPreview(DocId(1))]);
    let t = step(choosing(&[1], PositionChoice::Auto, false), Event::SettingsChanged, &gui());
    assert_eq!(t.effects, vec![Effect::ProbeBackends]);
}

#[test]
fn position_is_kept_when_returning_f43() {
    let p = pdfover_placement::Placement { page_index: 1, x: 50.0, y: 400.0, width: 230.0, height: 49.0 };
    let s = choosing(&[1], PositionChoice::Manual(p), false);
    let t = step(s, Event::Navigate(Step::Positioning), &gui());
    let State::Positioning { page, placement, .. } = &t.state else { panic!() };
    assert_eq!(*page, 1);
    assert_eq!(placement.unwrap(), p);
}

// ---------------------------------------------------------------------------------------------
// BKUSelection
// ---------------------------------------------------------------------------------------------

#[test]
fn default_backend_skips_selection_f53() {
    let ctx = Context { default_backend: Some(BackendChoice::Keystore), ..gui() };
    let t = step(choosing(&[1], PositionChoice::Auto, true), Event::BackendsProbed(options(true, true)), &ctx);
    let State::Signing { backend, phase, flow, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(*backend, BackendId::Keystore);
    assert_eq!(*phase, SigningPhase::OpeningSession);
    assert!(flow.needs_certificate);
    assert_eq!(t.effects, vec![Effect::OpenSession(BackendId::Keystore)]);
}

#[test]
fn unavailable_default_backend_shows_selection() {
    let ctx = Context { default_backend: Some(BackendChoice::LocalBku), ..gui() };
    let t = step(choosing(&[1], PositionChoice::Auto, true), Event::BackendsProbed(options(false, true)), &ctx);
    let State::ChoosingBackend { options, auto_choice, .. } = &t.state else { panic!() };
    assert_eq!(options.len(), 3);
    assert!(!auto_choice);
}

#[test]
fn only_id_austria_is_chosen_automatically_f54() {
    let ctx = Context { privacy_acknowledged: vec![BackendId::AtrustNative], ..gui() };
    let t = step(choosing(&[1], PositionChoice::Auto, true), Event::BackendsProbed(options(false, false)), &ctx);
    let State::Signing { backend, .. } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(*backend, BackendId::AtrustNative);
}

#[test]
fn several_backends_require_a_choice() {
    let t = step(choosing(&[1], PositionChoice::Auto, true), Event::BackendsProbed(options(true, false)), &gui());
    assert!(matches!(t.state, State::ChoosingBackend { auto_choice: false, .. }));
    let t = step(t.state, Event::BackendChosen(BackendId::LocalBku), &gui());
    assert!(matches!(&t.state, State::Signing { backend: BackendId::LocalBku, .. }));
}

#[test]
fn forced_selection_ignores_default_f55() {
    let ctx = Context { default_backend: Some(BackendChoice::Keystore), ..gui() };
    let t = step(choosing(&[1], PositionChoice::Auto, false), Event::BackendsProbed(options(true, true)), &ctx);
    assert!(matches!(t.state, State::ChoosingBackend { .. }));
}

#[test]
fn unavailable_backend_cannot_be_chosen() {
    let s =
        step(choosing(&[1], PositionChoice::Auto, false), Event::BackendsProbed(options(false, false)), &gui()).state;
    let t = step(s, Event::BackendChosen(BackendId::LocalBku), &gui());
    assert!(matches!(t.state, State::ChoosingBackend { .. }));
    assert!(t.effects.is_empty());
}

#[test]
fn privacy_notice_before_first_upload() {
    let s =
        step(choosing(&[1], PositionChoice::Auto, false), Event::BackendsProbed(options(true, false)), &gui()).state;
    let t = step(s, Event::BackendChosen(BackendId::AtrustNative), &gui());
    assert!(matches!(&t.state, State::PrivacyNotice { backend: BackendId::AtrustNative, .. }));
    // Ablehnen → zurück zur Auswahl
    let no = step(t.state.clone(), Event::PrivacyAnswered(false), &gui());
    assert!(matches!(no.state, State::ChoosingBackend { .. }));
    // Annehmen → Bestätigung speichern, Signieren (unsichtbar wäre ohne Zertifikat)
    let yes = step(t.state, Event::PrivacyAnswered(true), &gui());
    let State::Signing { flow, .. } = &yes.state else { panic!() };
    assert!(flow.needs_certificate, "automatische Position ist sichtbar: Zertifikat für den Block");
    assert_eq!(
        yes.effects,
        vec![Effect::AcknowledgePrivacy(BackendId::AtrustNative), Effect::OpenSession(BackendId::AtrustNative)]
    );
}

#[test]
fn privacy_notice_skipped_when_acknowledged() {
    let ctx = Context { privacy_acknowledged: vec![BackendId::AtrustNative], ..gui() };
    let s = step(choosing(&[1], PositionChoice::Auto, false), Event::BackendsProbed(options(true, false)), &ctx).state;
    let t = step(s, Event::BackendChosen(BackendId::AtrustNative), &ctx);
    assert!(matches!(t.state, State::Signing { .. }));
}

#[test]
fn certificate_need_follows_visibility() {
    let ctx = Context { privacy_acknowledged: vec![BackendId::AtrustNative], ..gui() };
    // Modell B, unsichtbar: kein InfoboxRead (ein einziger SL-Durchlauf)
    let s = step(choosing(&[1], PositionChoice::Invisible, false), Event::BackendsProbed(options(false, false)), &ctx)
        .state;
    let t = step(s, Event::BackendChosen(BackendId::AtrustNative), &ctx);
    let State::Signing { flow, .. } = &t.state else { panic!() };
    assert!(!flow.needs_certificate);
    let t = step(t.state, Event::PhaseCompleted(SigningPhase::OpeningSession), &ctx);
    assert!(matches!(t.state, State::Signing { phase: SigningPhase::Preparing, .. }));
    assert!(matches!(t.effects[0], Effect::Prepare { .. }));
    // sichtbar über ein Feld: Zertifikat nötig
    let s = step(
        choosing(&[1], PositionChoice::Field(field("A")), false),
        Event::BackendsProbed(options(false, false)),
        &ctx,
    )
    .state;
    let t = step(s, Event::BackendChosen(BackendId::AtrustNative), &ctx);
    let State::Signing { flow, .. } = &t.state else { panic!() };
    assert!(flow.needs_certificate);
}

// ---------------------------------------------------------------------------------------------
// PrepareSigning / LocalBKU / MobileBKU / KS / Signing
// ---------------------------------------------------------------------------------------------

#[test]
fn signing_phases_in_order() {
    let mut s = signing(&[1], SigningPhase::OpeningSession, BackendId::Keystore, PositionChoice::Auto);
    let expected = [
        (SigningPhase::OpeningSession, vec![Effect::ReadCertificate]),
        (
            SigningPhase::ReadingCertificate,
            vec![Effect::Prepare { doc: DocId(1), position: PositionChoice::Auto, min_reserve: None }],
        ),
        (SigningPhase::Preparing, vec![Effect::Sign]),
        (SigningPhase::Signing, vec![Effect::CloseSession, Effect::Finish]),
    ];
    for (phase, effects) in expected {
        let t = step(s, Event::PhaseCompleted(phase), &gui());
        assert_eq!(t.effects, effects, "{phase:?}");
        s = t.state;
    }
    let t = step(s, Event::PhaseCompleted(SigningPhase::Finishing), &gui());
    assert!(matches!(t.state, State::Output { saved: None, decision: None, .. }));
    assert_eq!(t.effects, vec![Effect::DeletePersisted, Effect::DecideOutput(DocId(1))]);
}

#[test]
fn stale_phase_completion_is_ignored() {
    let s = signing(&[1], SigningPhase::Preparing, BackendId::Keystore, PositionChoice::Auto);
    let t = step(s.clone(), Event::PhaseCompleted(SigningPhase::OpeningSession), &gui());
    assert_eq!(t.state, s);
    assert!(t.effects.is_empty());
}

#[test]
fn user_cancel_returns_to_backend_selection_f72() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    let t = step(s, Event::BackendFailed(BackendError::Cancelled), &gui());
    let State::ChoosingBackend { auto_choice, .. } = &t.state else { panic!("{:?}", t.state) };
    assert!(!auto_choice, "F-55: nach Abbruch wird gefragt");
    assert_eq!(t.effects, vec![Effect::CancelSession, Effect::DeletePersisted, Effect::ProbeBackends]);
}

#[test]
fn cancel_button_during_signing() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    let t = step(s, Event::Cancel, &gui());
    assert!(matches!(t.state, State::ChoosingBackend { .. }));
    assert_eq!(t.effects[0], Effect::CancelSession);
}

#[test]
fn only_explicit_cancel_counts_as_user_cancel_b15() {
    // ein Protokollfehler ist kein Nutzerabbruch (PDF-Over: IllegalStateException in der Kette)
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    let t = step(s, Event::BackendFailed(BackendError::Protocol("x".into())), &gui());
    assert!(matches!(t.state, State::Error { .. }));
}

#[test]
fn signing_error_retry_prepares_again_f71() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::LocalBku, PositionChoice::Auto);
    let t = step(s, Event::BackendFailed(BackendError::Network("reset".into())), &gui());
    let State::Error { error, retry, .. } = &t.state else { panic!() };
    assert_eq!(error.code(), "backend-network");
    assert_eq!(*retry, Some(RetryTarget::Prepare));
    assert_eq!(t.effects, vec![Effect::CloseSession, Effect::DeletePersisted]);
    let r = step(t.state.clone(), Event::Retry, &gui());
    assert!(matches!(
        &r.state,
        State::Signing { phase: SigningPhase::OpeningSession, backend: BackendId::LocalBku, .. }
    ));
    assert_eq!(r.effects, vec![Effect::OpenSession(BackendId::LocalBku)]);
    // Abbrechen → Auswahl
    let c = step(t.state, Event::Cancel, &gui());
    assert!(matches!(c.state, State::ChoosingBackend { auto_choice: false, .. }));
}

#[test]
fn non_retryable_backend_error() {
    let s = signing(&[1], SigningPhase::OpeningSession, BackendId::Keystore, PositionChoice::Auto);
    let t = step(s, Event::BackendFailed(BackendError::UnsupportedKeystoreFormat { format: "JKS".into() }), &gui());
    let State::Error { error, retry, .. } = &t.state else { panic!() };
    assert_eq!(*retry, None);
    assert_eq!(error.code(), "backend-keystore-format");
    assert_eq!(error.params()["detail"], "JKS");
    // Wiederholen ohne Ziel ändert nichts
    let r = step(t.state.clone(), Event::Retry, &gui());
    assert_eq!(r.state, t.state);
}

#[test]
fn invariant_violation_saves_nothing() {
    let s = signing(&[1], SigningPhase::Finishing, BackendId::Keystore, PositionChoice::Auto);
    let v = InvariantViolation { invariant: Invariant::I3Cms, detail: "digest".into() };
    let t = step(s, Event::SelfCheckFailed(v.clone()), &gui());
    let State::Error { error, retry, .. } = &t.state else { panic!() };
    assert_eq!(error, &WorkflowError::Invariant(v));
    assert_eq!(*retry, None);
    assert_eq!(error.exit_code(), ExitCode::Invariant);
    assert!(!t.effects.iter().any(|e| matches!(e, Effect::Write(_) | Effect::Overwrite(_))));
}

#[test]
fn cms_too_large_prepares_with_bigger_reserve() {
    let s = signing(&[1], SigningPhase::Finishing, BackendId::AtrustNative, PositionChoice::Auto);
    let t = step(s, Event::CmsTooLarge { needed: 20_000, reserve: 16_384 }, &gui());
    let State::Error { retry: Some(RetryTarget::PrepareWithReserve(n)), .. } = t.state.clone() else { panic!() };
    assert_eq!(n, grown_reserve(20_000, 16_384));
    assert!(n >= 40_000);
    let r = step(t.state, Event::Retry, &gui());
    let State::Signing { flow, phase, .. } = &r.state else { panic!() };
    assert_eq!(*phase, SigningPhase::OpeningSession);
    assert_eq!(flow.min_reserve, Some(n));
    // die Reserve wird bis zum Prepare weitergereicht
    let r = step(r.state, Event::PhaseCompleted(SigningPhase::OpeningSession), &gui());
    let r = step(r.state, Event::PhaseCompleted(SigningPhase::ReadingCertificate), &gui());
    assert!(matches!(r.effects[0], Effect::Prepare { min_reserve: Some(m), .. } if m == n));
}

#[test]
fn prepare_error_offers_retry() {
    let s = signing(&[1], SigningPhase::Preparing, BackendId::Keystore, PositionChoice::Auto);
    let t = step(s, Event::SigningFailed(WorkflowError::Unexpected("sigblock".into())), &gui());
    assert!(matches!(t.state, State::Error { retry: Some(RetryTarget::Prepare), .. }));
}

// ---------------------------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------------------------

#[test]
fn output_saved_automatically() {
    let t = step(
        output_state(&[1]),
        Event::OutputDecided(OutputDecision::Save { path: "/out/doc1_signed.pdf".into(), exists: false }),
        &gui(),
    );
    assert_eq!(t.effects, vec![Effect::Write("/out/doc1_signed.pdf".into())]);
    let t = step(t.state, Event::Saved(output_info(1)), &gui());
    let State::Output { saved, .. } = &t.state else { panic!() };
    assert_eq!(saved.as_ref().unwrap().path, std::path::PathBuf::from("/in/doc1_signed.pdf"));
    // Öffnen-Link (F-87)
    let o = step(t.state, Event::OpenResultRequested, &gui());
    assert_eq!(o.effects, vec![Effect::OpenResult("/in/doc1_signed.pdf".into())]);
}

#[test]
fn output_overwrite_question_f84() {
    let d = OutputDecision::Save { path: "/out/x.pdf".into(), exists: true };
    let t = step(output_state(&[1]), Event::OutputDecided(d), &gui());
    assert_eq!(t.effects, vec![Effect::AskOverwrite("/out/x.pdf".into())]);
    let yes = step(t.state.clone(), Event::OverwriteAnswered(true), &gui());
    assert_eq!(yes.effects, vec![Effect::Overwrite("/out/x.pdf".into())]);
    let no = step(t.state, Event::OverwriteAnswered(false), &gui());
    assert!(matches!(no.state, State::Output { saved: None, decision: None, .. }));
    assert!(no.effects.is_empty());
}

#[test]
fn output_save_dialog_f83() {
    let d = OutputDecision::Ask { directory: "/in".into(), file_name: "doc1_signed.pdf".into() };
    let t = step(output_state(&[1]), Event::OutputDecided(d.clone()), &gui());
    assert_eq!(t.effects, vec![Effect::AskSavePath(d)]);
    // Dialog liefert einen vorhandenen Pfad → Rückfrage
    let c = step(
        t.state.clone(),
        Event::OutputDecided(OutputDecision::Save { path: "/a.pdf".into(), exists: true }),
        &gui(),
    );
    assert_eq!(c.effects, vec![Effect::AskOverwrite("/a.pdf".into())]);
    // Dialog abgebrochen → Speichern-Knopf
    let x = step(t.state, Event::OutputChosen(None), &gui());
    assert!(matches!(x.state, State::Output { saved: None, decision: None, .. }));
}

#[test]
fn save_button_always_opens_dialog_f86() {
    let ctx = Context { output_postfix: "_sig".into(), ..gui() };
    let t = step(output_state(&[1]), Event::SaveRequested, &ctx);
    assert_eq!(
        t.effects,
        vec![Effect::AskSavePath(OutputDecision::Ask { directory: "/in".into(), file_name: "doc1_sig.pdf".into() })]
    );
}

#[test]
fn save_failure_retry_and_cancel_f85() {
    let t = step(
        output_state(&[1]),
        Event::OutputDecided(OutputDecision::Save { path: "/ro/x.pdf".into(), exists: false }),
        &gui(),
    );
    let t = step(t.state, Event::SaveFailed("Zugriff verweigert".into()), &gui());
    let State::Error { error, retry, .. } = &t.state else { panic!() };
    assert_eq!(error.exit_code(), ExitCode::Write);
    assert_eq!(*retry, Some(RetryTarget::Save));
    let r = step(t.state.clone(), Event::Retry, &gui());
    assert_eq!(r.effects, vec![Effect::Write("/ro/x.pdf".into())]);
    let c = step(t.state, Event::Cancel, &gui());
    assert!(matches!(c.state, State::Output { saved: None, decision: None, .. }));
}

#[test]
fn retry_without_fixed_target_reopens_save_dialog() {
    // Fehler im Ergebnis-Screen, während der Speichern-Dialog offen war: „Wiederholen“ zeigt ihn erneut
    let t = step(output_state(&[1]), Event::SaveRequested, &gui());
    let State::Output { decision: Some(ask @ OutputDecision::Ask { .. }), .. } = &t.state else { panic!() };
    let ask = ask.clone();
    let t = step(t.state, Event::SaveFailed("Zugriff verweigert".into()), &gui());
    let r = step(t.state, Event::Retry, &gui());
    assert_eq!(r.effects, vec![Effect::AskSavePath(ask)]);
    assert!(matches!(r.state, State::Output { saved: None, decision: Some(OutputDecision::Ask { .. }), .. }));
}

#[test]
fn retry_after_confirmed_overwrite_overwrites_again() {
    let d = OutputDecision::Save { path: "/out/x.pdf".into(), exists: true };
    let t = step(output_state(&[1]), Event::OutputDecided(d), &gui());
    let t = step(t.state, Event::OverwriteAnswered(true), &gui());
    let t = step(t.state, Event::SaveFailed("voll".into()), &gui());
    let r = step(t.state, Event::Retry, &gui());
    assert_eq!(r.effects, vec![Effect::Overwrite("/out/x.pdf".into())]);
}

#[test]
fn saved_with_pending_documents_continues_f88() {
    let t = step(output_state(&[1, 2]), Event::Saved(output_info(1)), &gui());
    let State::Analyzing { queue } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(queue.current_id(), Some(DocId(2)));
    assert_eq!(queue.done.len(), 1);
    assert!(matches!(queue.done[0].result, DocResult::Signed { .. }));
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1)), Effect::Analyze(DocId(2))]);
}

#[test]
fn saved_with_skip_finish_ends_f90() {
    let ctx = Context { skip_finish: true, ..gui() };
    let t = step(output_state(&[1]), Event::Saved(output_info(1)), &ctx);
    assert_eq!(t.state.exit_code(), Some(ExitCode::Success));
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1)), Effect::Exit(ExitCode::Success)]);
}

#[test]
fn files_dropped_on_result_screen_f89() {
    let s = step(output_state(&[1]), Event::Saved(output_info(1)), &gui()).state;
    let t = step(s, Event::FilesAdded(vec![doc(2), doc(2)]), &gui());
    let State::Analyzing { queue } = &t.state else { panic!("{:?}", t.state) };
    assert_eq!(queue.current_id(), Some(DocId(2)));
    // nicht gespeichert: nur anhängen
    let t = step(output_state(&[1]), Event::FilesAdded(vec![doc(2)]), &gui());
    let State::Output { queue, .. } = &t.state else { panic!() };
    assert_eq!(queue.pending.len(), 1);
}

#[test]
fn batch_continues_after_failed_save_via_save_button_b14() {
    let t = step(output_state(&[1, 2]), Event::OutputChosen(None), &gui());
    let t = step(t.state, Event::SaveRequested, &gui());
    let t = step(t.state, Event::OutputDecided(OutputDecision::Save { path: "/x.pdf".into(), exists: false }), &gui());
    let t = step(t.state, Event::Saved(output_info(1)), &gui());
    assert!(matches!(&t.state, State::Analyzing { queue } if queue.current_id() == Some(DocId(2))));
}

#[test]
fn resign_same_document_f91() {
    let p = pdfover_placement::Placement { page_index: 0, x: 1.0, y: 100.0, width: 230.0, height: 49.0 };
    let State::Output { queue, doc, backend, .. } = output_state(&[1]) else { unreachable!() };
    let s = State::Output {
        queue,
        saved: Some(output_info(1)),
        decision: None,
        doc,
        position: PositionChoice::Manual(p),
        backend,
    };
    let t = step(s.clone(), Event::Navigate(Step::Positioning), &gui());
    assert!(matches!(&t.state, State::Positioning { placement: Some(pp), .. } if *pp == p));
    let t = step(s, Event::Navigate(Step::Signing), &gui());
    assert!(matches!(t.state, State::ChoosingBackend { auto_choice: false, .. }));
}

#[test]
fn open_from_result_takes_next_document_f16() {
    let mut s = output_state(&[1, 2]);
    if let State::Output { saved, .. } = &mut s {
        *saved = Some(output_info(1));
    }
    let t = step(s, Event::Navigate(Step::Open), &gui());
    assert!(matches!(&t.state, State::Analyzing { queue } if queue.current_id() == Some(DocId(2))));
}

#[test]
fn open_elsewhere_discards_queue_f16() {
    let t = step(choosing(&[1, 2, 3], PositionChoice::Auto, false), Event::Navigate(Step::Open), &gui());
    let State::Idle { queue } = &t.state else { panic!() };
    assert!(queue.is_empty());
    assert_eq!(queue.done.len(), 3);
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1)), Effect::Discard(DocId(2)), Effect::Discard(DocId(3))]);
}

#[test]
fn files_added_while_signing_are_queued() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    let t = step(s, Event::FilesAdded(vec![doc(5)]), &gui());
    assert_eq!(t.state.queue().unwrap().pending.len(), 1);
    assert!(matches!(t.state, State::Signing { .. }));
}

// ---------------------------------------------------------------------------------------------
// Lebenszyklus, Hauptleiste
// ---------------------------------------------------------------------------------------------

#[test]
fn backgrounding_is_not_a_cancel() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    for e in [Event::Backgrounded, Event::Foregrounded] {
        let t = step(s.clone(), e, &gui());
        assert_eq!(t.state, s);
        assert!(t.effects.is_empty());
    }
}

#[test]
fn quit_during_signing_cancels_session() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
    let t = step(s, Event::Quit, &gui());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Cancelled));
    assert_eq!(t.effects, vec![Effect::CancelSession, Effect::DeletePersisted, Effect::Exit(ExitCode::Cancelled)]);
    let t = step(State::Idle { queue: Queue::default() }, Event::Quit, &gui());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Success));
}

#[test]
fn quit_after_saving_keeps_the_saved_document_signed() {
    // gespeichert, weitere Dokumente warten: das gespeicherte bleibt „signiert“, die wartenden gelten als
    // abgebrochen
    let t = step(output_state(&[1]), Event::Saved(output_info(1)), &gui());
    let State::Output { mut queue, saved, decision, doc, position, backend } = t.state else { panic!() };
    queue.add([pdfover_workflow::InputDocument::new(DocId(2), "/in/doc2.pdf")]);
    let s = State::Output { queue, saved, decision, doc, position, backend };
    let q = step(s, Event::Quit, &gui());
    assert_eq!(q.state.exit_code(), Some(ExitCode::Cancelled));
    let results = q.state.results();
    assert_eq!(results.len(), 2);
    assert!(matches!(results[0].result, DocResult::Signed { .. }), "{results:?}");
    assert_eq!(results[1].result, DocResult::Cancelled);
    // ohne wartende Dokumente: Erfolg
    let t = step(output_state(&[1]), Event::Saved(output_info(1)), &gui());
    let q = step(t.state, Event::Quit, &gui());
    assert_eq!(q.state.exit_code(), Some(ExitCode::Success));
}

/// Hauptleisten-Matrix docs/analyse/01 §3.5 („A“ aktiv, „E“ klickbar).
#[test]
fn main_bar_matrix() {
    fn bar(s: &State) -> String {
        s.main_bar()
            .iter()
            .map(|b| match (b.active, b.enabled) {
                (true, true) => "AE",
                (true, false) => "A",
                (false, true) => "E",
                (false, false) => "-",
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    let idle = State::Idle { queue: Queue::default() };
    assert_eq!(bar(&State::Starting), "- - - -");
    assert_eq!(bar(&idle), "AE - - -");
    assert_eq!(bar(&positioning(&[1])), "AE A - -");
    assert_eq!(bar(&choosing(&[1], PositionChoice::Auto, false)), "AE AE A -");
    assert_eq!(bar(&signing(&[1], SigningPhase::Signing, BackendId::Keystore, PositionChoice::Auto)), "A A A -");
    assert_eq!(bar(&output_state(&[1])), "AE AE AE A");
    assert_eq!(bar(&State::Finished { exit: ExitCode::Success, results: vec![] }), "- - - -");
    // Fehler sind modal: aktive Schritte des Ursprungs, nichts klickbar
    let err = step(
        signing(&[1], SigningPhase::Signing, BackendId::Keystore, PositionChoice::Auto),
        Event::BackendFailed(BackendError::Timeout),
        &gui(),
    )
    .state;
    assert_eq!(bar(&err), "A A A -");
    // Konfig-Button
    assert!(idle.settings_enabled());
    assert!(positioning(&[1]).settings_enabled());
    assert!(!signing(&[1], SigningPhase::Signing, BackendId::Keystore, PositionChoice::Auto).settings_enabled());
    assert!(output_state(&[1]).settings_enabled());
    // „Finish“ ist nie klickbar (B-02 ist Absicht: reine Statusanzeige)
    assert!(!output_state(&[1]).step_enabled(Step::Finish));
}

#[test]
fn disabled_navigation_is_ignored() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::Keystore, PositionChoice::Auto);
    for step_ in [Step::Open, Step::Positioning, Step::Signing, Step::Finish] {
        let t = step(s.clone(), Event::Navigate(step_), &gui());
        assert_eq!(t.state, s, "{step_:?}");
    }
    let t = step(positioning(&[1]), Event::Navigate(Step::Signing), &gui());
    assert!(matches!(t.state, State::Positioning { .. }));
}

// ---------------------------------------------------------------------------------------------
// Ohne Oberfläche (CLI)
// ---------------------------------------------------------------------------------------------

#[test]
fn headless_batch_runs_through_and_reports_first_failure() {
    let ctx = Context { default_backend: Some(BackendChoice::Keystore), ..headless() };
    let (s, effects) = run(
        State::Starting,
        [
            Event::Started { documents: vec![doc(1), doc(2)] },
            analyzed(1, 1, None, false),
            Event::BackendsProbed(options(false, true)),
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            // ohne Dialog: neben der Quelle
            Event::OutputDecided(OutputDecision::Ask { directory: "/in".into(), file_name: "doc1_signed.pdf".into() }),
            Event::Saved(output_info(1)),
            // Dokument 2 scheitert
            analyzed(2, 1, None, false),
            Event::BackendsProbed(options(false, true)),
            Event::BackendFailed(BackendError::WrongPassword),
        ],
        &ctx,
    );
    assert_eq!(s.exit_code(), Some(ExitCode::Backend));
    assert!(effects.contains(&Effect::Write("/in/doc1_signed.pdf".into())));
    // Ergebnisse bleiben nach dem Ende erhalten (CLI `--json`)
    assert_eq!(s.results().len(), 2);
    assert!(matches!(s.results()[0].result, DocResult::Signed { .. }));
    assert!(matches!(&s.results()[1].result, DocResult::Failed { error } if error.code() == "backend-wrong-password"));
    assert_eq!(effects.last(), Some(&Effect::Exit(ExitCode::Backend)));
}

/// Ohne Oberfläche gibt es keine Hauptleiste: „Positionierung“ führte sonst in einen Dialog.
#[test]
fn headless_ignores_main_bar_navigation() {
    let s = choosing(&[1], PositionChoice::Auto, false);
    for step_ in [Step::Open, Step::Positioning, Step::Signing] {
        let t = step(s.clone(), Event::Navigate(step_), &headless());
        assert_eq!(t.state, s, "{step_:?}");
        assert!(t.effects.is_empty());
    }
    // mit Oberfläche wirkt dieselbe Navigation
    let t = step(s, Event::Navigate(Step::Positioning), &gui());
    assert!(matches!(t.state, State::Positioning { .. }));
}

#[test]
fn headless_ambiguous_backend_fails() {
    let t = step(choosing(&[1], PositionChoice::Auto, true), Event::BackendsProbed(options(true, true)), &headless());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Backend));
}

#[test]
fn headless_single_backend_is_chosen() {
    let mut opts = options(false, true);
    opts.retain(|o| o.id == BackendId::Keystore);
    let t = step(choosing(&[1], PositionChoice::Auto, true), Event::BackendsProbed(opts), &headless());
    assert!(matches!(t.state, State::Signing { backend: BackendId::Keystore, .. }));
}

#[test]
fn headless_placeholder_rules() {
    let ctx = Context { placeholder_search: true, ..headless() };
    let t =
        step(State::Analyzing { queue: queue(&[1]) }, analyzed(1, 1, Some(PlaceholderFound::Qr(qr())), false), &ctx);
    assert!(matches!(&t.state, State::ChoosingBackend { position: PositionChoice::Placeholder(_), .. }));
    let t = step(
        State::Analyzing { queue: queue(&[1]) },
        analyzed(1, 1, Some(PlaceholderFound::Fields(vec![field("A"), field("B")])), false),
        &ctx,
    );
    assert!(matches!(&t.state, State::ChoosingBackend { position: PositionChoice::Auto, .. }));
}

#[test]
fn headless_cancel_counts_as_cancelled() {
    let s = signing(&[1], SigningPhase::Signing, BackendId::Keystore, PositionChoice::Auto);
    let t = step(s, Event::BackendFailed(BackendError::Cancelled), &headless());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Cancelled));
}

#[test]
fn headless_declined_overwrite_is_cancelled() {
    let t = step(
        output_state(&[1]),
        Event::OutputDecided(OutputDecision::Save { path: "/x.pdf".into(), exists: true }),
        &headless(),
    );
    let t = step(t.state, Event::OverwriteAnswered(false), &headless());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Cancelled));
}

#[test]
fn headless_save_failure_is_write_error() {
    let t = step(
        output_state(&[1]),
        Event::OutputDecided(OutputDecision::Save { path: "/x.pdf".into(), exists: false }),
        &headless(),
    );
    let t = step(t.state, Event::SaveFailed("disk full".into()), &headless());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Write));
}

#[test]
fn headless_without_documents_finishes() {
    let t = step(State::Starting, Event::Started { documents: vec![] }, &headless());
    assert_eq!(t.state.exit_code(), Some(ExitCode::Success));
}

// ---------------------------------------------------------------------------------------------
// Fehler-IDs und Serialisierung
// ---------------------------------------------------------------------------------------------

#[test]
fn error_codes_and_params() {
    let e = WorkflowError::from_backend(&BackendError::SecurityLayer { code: 6002, info: "abc".into() });
    assert_eq!(e.code(), "backend-security-layer");
    assert_eq!(e.params()["detail"], "6002: abc");
    let e = WorkflowError::CmsTooLarge { needed: 5, reserve: 4 };
    assert_eq!(e.code(), "ng-cms-too-large");
    assert_eq!(e.params()["needed"], "5");
    assert_eq!(WorkflowError::Save("x".into()).code(), "output-save_failed");
    assert_eq!(WorkflowError::from_backend(&BackendError::Cancelled).exit_code(), ExitCode::Cancelled);
}

#[test]
fn state_is_serializable() {
    let s = step(
        output_state(&[1, 2]),
        Event::OutputDecided(OutputDecision::Save { path: "/x".into(), exists: true }),
        &gui(),
    )
    .state;
    let json = serde_json::to_string(&s).unwrap();
    let back: State = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}
