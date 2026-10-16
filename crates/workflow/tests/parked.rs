//! Tabellentests „Zur Startseite“ parkt offene Arbeit (ADR 0004 Nachtrag 10): Parken statt Verwerfen
//! (Arbeitsbereich mit Platzierung, Analysewarnung, Stapelübersicht, „Fertig“ ohne Speichern), „Weiter
//! bearbeiten“ eines Einzeldokuments unverändert, eines Stapels in die Stapelübersicht mit allen Änderungen am
//! offenen Dokument (Nachtrag 13), „Schließen“ verwirft wie „Öffnen“, neue Dateien kommen zum Stapel („Zum Stapel
//! hinzufügen“, „Öffnen mit“) bzw. ersetzen („Offene schließen und neu beginnen“), verdeckte Arbeit ist nicht
//! bedienbar (F-54), Ergebnisse von Effekten wirken weiter; Signieren, Stapellauf und Fehler bleiben unberührt.

mod common;

use common::*;
use pida_backend_api::BackendId;
use pida_pades::AcceptanceClass;
use pida_placement::Placement;
use pida_workflow::{
    BatchItemStatus, BatchOutput, Context, DocId, DocResult, DocSummary, Effect, Event, ExistingPolicy,
    MSG_PARKED_FILES_ADDED, PositionChoice, Queue, SigningPhase, State, Step, Target, Transition, WorkflowError,
};

fn ctx() -> Context {
    Context { workspace: true, batch_overview: true, ..Context::default() }
}

fn summary(warnings: Vec<String>) -> DocSummary {
    DocSummary {
        page_count: 3,
        size_bytes: 1234,
        signed: false,
        class: AcceptanceClass::K1Regular,
        qr: false,
        fields: 0,
        warnings,
        placeholder: None,
        new_visible_field_allowed: true,
        first_field: None,
        qr_searched: true,
    }
}

fn opens(effects: &[Effect]) -> usize {
    effects.iter().filter(|e| matches!(e, Effect::OpenSession(_))).count()
}

fn batch_of(s: &State) -> &pida_workflow::Batch {
    s.queue().and_then(|q| q.batch.as_ref()).expect("Stapel")
}

fn notified(effects: &[Effect], code: &str) -> bool {
    effects.iter().any(|e| matches!(e, Effect::NotifyWith(c, _) if c == code))
}

/// Arbeitsbereich mit Dokument `id` (eben analysiert, automatisch platziert).
fn workspace_with(id: u64, c: &Context) -> State {
    let s = step(State::Idle { queue: Queue::default() }, Event::FilesAdded(vec![doc(id)]), c).state;
    let s = step(s, analyzed(id, 3, None, false), c).state;
    assert!(matches!(s, State::Positioning { edit: Some(_), .. }), "{}", s.name());
    s
}

/// Arbeitsbereich mit von Hand verschobenem Block.
fn placed_by_hand(id: u64) -> (State, Placement) {
    let p = Placement { page_index: 1, x: 40.0, y: 300.0, width: 230.0, height: 49.0 };
    let s = step(workspace_with(id, &ctx()), Event::PlacementChanged(p), &ctx()).state;
    (s, p)
}

/// Übersicht mit `ids`, alle voranalysiert (`Ready`).
fn overview(ids: &[u64]) -> State {
    let t =
        step(State::Idle { queue: Queue::default() }, Event::FilesAdded(ids.iter().map(|i| doc(*i)).collect()), &ctx());
    let mut s = t.state;
    for id in ids {
        s = step(s, Event::Preanalyzed { doc: DocId(*id), result: Ok(summary(Vec::new())) }, &ctx()).state;
    }
    assert!(matches!(s, State::BatchOverview { .. }), "{}", s.name());
    s
}

fn home(s: State) -> Transition {
    step(s, Event::WentHome, &ctx())
}

fn unparked(mut s: State) -> State {
    if let Some(q) = s.queue_mut() {
        q.parked = false;
    }
    s
}

#[test]
fn home_parks_the_workspace_and_resume_restores_it_unchanged() {
    let (s, p) = placed_by_hand(1);
    let t = home(s.clone());
    assert!(t.state.parked(), "geparkt statt verworfen");
    assert!(t.effects.is_empty(), "nichts freigegeben, nichts gestartet: {:?}", t.effects);
    assert_eq!(unparked(t.state.clone()), s, "Zustand samt Platzierung unverändert");
    // erneut „Zur Startseite“: bleibt geparkt
    let again = home(t.state.clone());
    assert_eq!((again.state.clone(), again.effects.len()), (t.state.clone(), 0));
    // „Weiter bearbeiten“: genau wie vorher
    let t = step(t.state, Event::ParkedResumed, &ctx());
    assert_eq!(t.state, s);
    assert!(t.effects.is_empty(), "{:?}", t.effects);
    let State::Positioning { placement, .. } = &t.state else { panic!("{}", t.state.name()) };
    assert_eq!(*placement, Some(p), "Platzierung bleibt");
}

#[test]
fn parked_work_cannot_be_signed_or_moved() {
    let parked = home(placed_by_hand(1).0).state;
    let p = Placement { page_index: 0, x: 10.0, y: 100.0, width: 230.0, height: 49.0 };
    for event in [
        Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
        Event::SignRequested { backend: BackendId::AtrustNative, options: options(true, true) },
        Event::PositionConfirmed,
        Event::InvisibleChosen,
        Event::PlacementChanged(p),
        Event::PageChanged(0),
        Event::NewPageToggled,
        Event::TargetChosen(Target::Free),
        Event::InvisibleSet(true),
        Event::PlacementUnlocked,
        Event::Cancel,
        Event::Navigate(Step::Open),
        Event::ShowBatch,
        Event::BackToBatch,
        Event::Retry,
        Event::BackendChosen(BackendId::Keystore),
        Event::BackendsProbed(options(true, true)),
    ] {
        let t = step(parked.clone(), event.clone(), &ctx());
        assert_eq!(t.state, parked, "{event:?} verändert verdeckte Arbeit");
        assert!(t.effects.is_empty(), "{event:?}: {:?}", t.effects);
    }
    // Stapelübersicht: weder starten noch öffnen noch entfernen
    let parked = home(overview(&[1, 2])).state;
    for event in [
        Event::BatchStarted {
            backend: BackendId::Keystore,
            docs: vec![DocId(1), DocId(2)],
            output: Some(BatchOutput::Configured),
            on_existing: ExistingPolicy::KeepBoth,
            options: options(true, true),
        },
        Event::BatchItemOpened(DocId(1)),
        Event::BatchItemRemoved(DocId(2)),
        Event::BatchAutoPositionSet { enabled: true, replace_manual: true },
    ] {
        let t = step(parked.clone(), event.clone(), &ctx());
        assert_eq!((t.state, opens(&t.effects)), (parked.clone(), 0), "{event:?}");
    }
}

#[test]
fn effect_results_and_settings_still_reach_parked_work() {
    // automatische Platzierung eines automatisch platzierten Dokuments kommt nach dem Parken an
    let parked = home(workspace_with(1, &ctx())).state;
    let p = Placement { page_index: 2, x: 300.0, y: 200.0, width: 230.0, height: 49.0 };
    let t = step(parked, Event::AutoPlaced(p), &ctx());
    assert!(t.state.parked());
    assert!(matches!(&t.state, State::Positioning { placement: Some(q), .. } if *q == p), "{:?}", t.state);
    // Einstellungen „sofort wirksam“: Blockvorschau neu, bleibt geparkt
    let t = step(t.state, Event::SettingsChanged, &ctx());
    assert!(t.state.parked() && t.effects.contains(&Effect::BuildBlockPreview(DocId(1))), "{:?}", t.effects);
    // Voranalyse eines geparkten Stapels läuft weiter
    let s = step(State::Idle { queue: Queue::default() }, Event::FilesAdded(vec![doc(1), doc(2)]), &ctx()).state;
    let parked = home(s).state;
    let t = step(parked, Event::Preanalyzed { doc: DocId(1), result: Ok(summary(Vec::new())) }, &ctx());
    assert!(t.state.parked());
    assert_eq!(batch_of(&t.state).item(DocId(1)).unwrap().status, BatchItemStatus::Ready);
    assert_eq!(batch_of(&t.state).item(DocId(2)).unwrap().status, BatchItemStatus::Analyzing);
}

#[test]
fn discard_closes_the_parked_work_like_open() {
    let parked = home(placed_by_hand(1).0).state;
    let t = step(parked, Event::ParkedDiscarded, &ctx());
    let State::Idle { queue } = &t.state else { panic!("{}", t.state.name()) };
    assert!(queue.is_empty() && queue.batch.is_none() && !queue.parked);
    assert_eq!((queue.done[0].doc.id, &queue.done[0].result), (DocId(1), &DocResult::Cancelled));
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1))]);
    // Stapel: signierte Einträge bleiben signiert (Ergebnisliste), die übrigen „abgebrochen“
    let s = overview(&[1, 2]);
    let s = step(s, Event::BatchItemOpened(DocId(1)), &ctx()).state;
    let s = step(s, analyzed(1, 3, None, false), &ctx()).state;
    let (s, _) = run(
        s,
        [
            Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            Event::Saved(output_info(1)),
        ],
        &ctx(),
    );
    assert!(matches!(s, State::BatchOverview { .. }), "{}", s.name());
    let t = step(home(s).state, Event::ParkedDiscarded, &ctx());
    let results: Vec<_> = t.state.results().iter().map(|o| (o.doc.id.0, o.result.clone())).collect();
    assert_eq!(results, vec![(1, DocResult::Signed { output: output_info(1) }), (2, DocResult::Cancelled)]);
    assert_eq!(t.effects, vec![Effect::Discard(DocId(2))]);
}

#[test]
fn add_to_the_parked_document_forms_a_batch_with_its_plan() {
    let (s, p) = placed_by_hand(1);
    let parked = home(s).state;
    let t = step(parked, Event::MoreFilesAdded(vec![doc(2), doc(3)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }) && !t.state.parked(), "{}", t.state.name());
    let b = batch_of(&t.state);
    assert_eq!(b.items.iter().map(|i| i.doc.0).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(b.item(DocId(1)).unwrap().plan, Some(PositionChoice::Manual(p)), "Platzierung bleibt Plan");
    assert!(!notified(&t.effects, MSG_PARKED_FILES_ADDED), "selbst gewählt: kein Hinweis");
    assert_eq!(opens(&t.effects), 0);
    // nur Duplikate: zurück in den Arbeitsbereich, unverändert
    let (s, _) = placed_by_hand(1);
    let same = pida_workflow::InputDocument::new(DocId(9), "/in/doc1.pdf");
    let t = step(home(s.clone()).state, Event::MoreFilesAdded(vec![same]), &ctx());
    assert_eq!(t.state, s);
}

#[test]
fn add_to_a_parked_overview_appends_entries() {
    let parked = home(overview(&[1, 2])).state;
    let t = step(parked, Event::MoreFilesAdded(vec![doc(3)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }) && !t.state.parked());
    assert_eq!(batch_of(&t.state).items.len(), 3);
    assert!(t.effects.contains(&Effect::Preanalyze(DocId(3))));
}

#[test]
fn open_with_joins_the_parked_work_without_asking_and_notifies() {
    let parked = home(placed_by_hand(1).0).state;
    let t = step(parked, Event::FilesAdded(vec![doc(2)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    assert_eq!(batch_of(&t.state).items.len(), 2);
    let notice = t.effects.iter().find_map(|e| match e {
        Effect::NotifyWith(c, p) if c == MSG_PARKED_FILES_ADDED => Some(p.clone()),
        _ => None,
    });
    assert_eq!(notice.and_then(|p| p.get("count").cloned()).as_deref(), Some("1"));
    assert_eq!(opens(&t.effects), 0, "kein Dialog, keine Sitzung ohne Nutzeraktion");
}

#[test]
fn replace_discards_the_parked_work_and_starts_fresh() {
    let parked = home(placed_by_hand(1).0).state;
    let t = step(parked, Event::DocumentReplaced(vec![doc(2)]), &ctx());
    let State::Analyzing { queue } = &t.state else { panic!("{}", t.state.name()) };
    assert_eq!(queue.current_id(), Some(DocId(2)));
    assert!(queue.batch.is_none() && !queue.parked);
    assert_eq!(
        queue.done.iter().map(|o| (o.doc.id.0, o.result.clone())).collect::<Vec<_>>(),
        vec![(1, DocResult::Cancelled)]
    );
    assert_eq!(t.effects.first(), Some(&Effect::Discard(DocId(1))));
    assert!(t.effects.contains(&Effect::Analyze(DocId(2))));
    // geparkter Stapel, zwei neue: neue Übersicht nur mit den neuen
    let parked = home(overview(&[1, 2])).state;
    let t = step(parked, Event::DocumentReplaced(vec![doc(3), doc(4)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }));
    assert_eq!(batch_of(&t.state).items.iter().map(|i| i.doc.0).collect::<Vec<_>>(), vec![3, 4]);
}

#[test]
fn parked_analysis_warning_joins_the_batch_as_an_entry_to_review() {
    let s = step(State::Idle { queue: Queue::default() }, Event::FilesAdded(vec![doc(1)]), &ctx()).state;
    let mut warning = analyzed(1, 3, None, false);
    if let Event::Analyzed { warnings, .. } = &mut warning {
        warnings.push("pdf-usage-rights".into());
    }
    let s = step(s, warning, &ctx()).state;
    assert!(matches!(s, State::AnalysisWarning { .. }));
    let parked = home(s.clone()).state;
    assert!(parked.parked());
    assert_eq!(step(parked.clone(), Event::ParkedResumed, &ctx()).state, s, "Warnung wie vorher");
    let t = step(parked, Event::MoreFilesAdded(vec![doc(2)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    assert!(t.effects.contains(&Effect::Discard(DocId(1))) && t.effects.contains(&Effect::Preanalyze(DocId(1))));
    let t = step(
        t.state,
        Event::Preanalyzed { doc: DocId(1), result: Ok(summary(vec!["pdf-usage-rights".into()])) },
        &ctx(),
    );
    assert!(matches!(batch_of(&t.state).item(DocId(1)).unwrap().status, BatchItemStatus::NeedsReview { .. }));
}

#[test]
fn saved_with_waiting_documents_parks_the_batch() {
    // gespeichert, ein weiteres Dokument wartet: wie „Zum Stapel“, die Übersicht wird geparkt
    let s = workspace_with(1, &ctx());
    let s = step(s, Event::FilesAdded(vec![doc(2)]), &ctx()).state;
    let (s, _) = run(
        s,
        [
            Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            Event::Saved(output_info(1)),
        ],
        &ctx(),
    );
    assert!(matches!(s, State::Output { saved: Some(_), .. }), "{}", s.name());
    let t = home(s);
    assert!(matches!(t.state, State::BatchOverview { .. }) && t.state.parked(), "{}", t.state.name());
    let b = batch_of(&t.state);
    assert!(matches!(b.item(DocId(1)).unwrap().status, BatchItemStatus::Signed { .. }));
    assert_eq!(b.item(DocId(2)).unwrap().status, BatchItemStatus::Analyzing);
    assert!(t.effects.contains(&Effect::Preanalyze(DocId(2))));
    // ohne wartende Dokumente: wie bisher zur Dokumentauswahl (nichts zu parken)
    let s = workspace_with(1, &ctx());
    let (s, _) = run(
        s,
        [
            Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            Event::Saved(output_info(1)),
        ],
        &ctx(),
    );
    let t = home(s);
    assert!(matches!(&t.state, State::Idle { queue } if !queue.parked && queue.is_empty()), "{}", t.state.name());
    assert!(matches!(t.state.results(), [o] if matches!(o.result, DocResult::Signed { .. })));
}

#[test]
fn unsaved_result_is_parked_and_stays_saveable() {
    let s = workspace_with(1, &ctx());
    let (s, _) = run(
        s,
        [
            Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            Event::OutputChosen(None),
        ],
        &ctx(),
    );
    assert!(matches!(s, State::Output { saved: None, decision: None, .. }), "{}", s.name());
    let parked = home(s.clone()).state;
    assert!(parked.parked());
    assert_eq!(step(parked.clone(), Event::SaveRequested, &ctx()).state, parked, "verdeckt: kein Speichern-Dialog");
    let t = step(parked, Event::ParkedResumed, &ctx());
    assert_eq!(t.state, s);
    assert!(matches!(step(t.state, Event::SaveRequested, &ctx()).effects.as_slice(), [Effect::AskSavePath(_)]));
}

#[test]
fn loading_an_entry_returns_to_the_overview_and_parks_it() {
    let s = step(overview(&[1, 2]), Event::BatchItemOpened(DocId(1)), &ctx()).state;
    assert!(matches!(s, State::Analyzing { .. }));
    let t = home(s);
    assert!(matches!(t.state, State::BatchOverview { .. }) && t.state.parked(), "{}", t.state.name());
    assert_eq!(batch_of(&t.state).item(DocId(1)).unwrap().status, BatchItemStatus::Ready, "Eintrag unverändert");
    // ein geöffneter Eintrag im Arbeitsbereich wird mit seiner Platzierung geparkt (Zustand unverändert, nichts
    // freigegeben); „Weiter bearbeiten“ führt wie „Zurück zum Stapel“ in die Übersicht (Nachtrag 13)
    let s = step(overview(&[1, 2]), Event::BatchItemOpened(DocId(1)), &ctx()).state;
    let s = step(s, analyzed(1, 3, None, false), &ctx()).state;
    let t = home(s.clone());
    assert!(t.state.parked() && t.state.opened_from_batch() && t.effects.is_empty());
    assert_eq!(unparked(t.state.clone()), s);
    assert_eq!(resume(t.state), step(s, Event::BackToBatch, &ctx()));
}

// ------------------------------------------------------------------------------------------------
// „Weiter bearbeiten“ eines Stapels führt zur Stapelübersicht (ADR 0004 Nachtrag 13)
// ------------------------------------------------------------------------------------------------

fn resume(s: State) -> Transition {
    step(s, Event::ParkedResumed, &ctx())
}

/// Aus der Übersicht geöffneter Eintrag `id` im Arbeitsbereich (eben analysiert).
fn opened_entry(s: State, id: u64) -> State {
    let s = step(s, Event::BatchItemOpened(DocId(id)), &ctx()).state;
    let s = step(s, analyzed(id, 3, None, false), &ctx()).state;
    assert!(matches!(s, State::Positioning { .. }) && s.opened_from_batch(), "{}", s.name());
    s
}

fn plan_of(s: &State, id: u64) -> Option<PositionChoice> {
    batch_of(s).item(DocId(id)).and_then(|i| i.plan.clone())
}

fn status_of(s: &State, id: u64) -> BatchItemStatus {
    batch_of(s).item(DocId(id)).expect("Eintrag").status.clone()
}

/// Der Fall der Meldung: Eintrag aus der Übersicht geöffnet, Block verschoben, „Zur Startseite“, „Weiter
/// bearbeiten“ → Stapelübersicht (nicht das Dokument), die Platzierung ist Plan des Eintrags („Positioniert“).
#[test]
fn resume_of_an_opened_entry_shows_the_overview_with_its_placement() {
    let p = Placement { page_index: 1, x: 40.0, y: 300.0, width: 230.0, height: 49.0 };
    let s = opened_entry(overview(&[1, 2, 3]), 2);
    let s = step(s, Event::PlacementChanged(p), &ctx()).state;
    assert!(s.resumes_to_overview(&ctx()));
    let parked = home(s.clone()).state;
    assert!(parked.parked() && parked.resumes_to_overview(&ctx()));
    let t = resume(parked);
    assert!(matches!(t.state, State::BatchOverview { .. }) && !t.state.parked(), "{}", t.state.name());
    assert_eq!(plan_of(&t.state, 2), Some(PositionChoice::Manual(p)), "Platzierung bleibt als Plan");
    assert_eq!(status_of(&t.state, 2), BatchItemStatus::Positioned);
    assert_eq!(batch_of(&t.state).open, None, "kein Dokument mehr geöffnet");
    assert_eq!(batch_of(&t.state).items.len(), 3);
    assert_eq!(t.effects, vec![Effect::Discard(DocId(2))], "nur das Dokument freigegeben, nichts gestartet");
    // genau wie „Zurück zum Stapel“ aus dem Arbeitsbereich
    assert_eq!(t, step(s, Event::BackToBatch, &ctx()));
    // „Unsichtbar signieren“ der Nutzerin bleibt ebenso Plan
    let s = step(opened_entry(overview(&[1, 2]), 1), Event::InvisibleSet(true), &ctx()).state;
    let t = resume(home(s).state);
    assert_eq!(plan_of(&t.state, 1), Some(PositionChoice::Invisible));
    // was während des Parkens ankommt (automatische Lage, Einstellungen), geht nicht verloren
    let s = opened_entry(overview(&[1, 2]), 1);
    let q = Placement { page_index: 2, x: 300.0, y: 200.0, width: 230.0, height: 49.0 };
    let parked = step(home(s).state, Event::AutoPlaced(q), &ctx()).state;
    let parked = step(parked, Event::SettingsChanged, &ctx()).state;
    assert!(parked.parked());
    let t = resume(parked);
    assert!(matches!(t.state, State::BatchOverview { .. }));
    assert_eq!(plan_of(&t.state, 1), None, "automatische Lage: kein Plan (PDF-AS-Regel wie „Zurück zum Stapel“)");
    assert!(batch_of(&t.state).item(DocId(1)).unwrap().auto_plan, "folgt weiter den Einstellungen");
}

/// Einzeldokument, zu dem weitere Dokumente gekommen sind (wartend bzw. mit „+ PDF“ zum Stapel geworden): „Weiter
/// bearbeiten“ zeigt den Stapel – wie „Zum Stapel“, die Platzierung des Dokuments bleibt dessen Plan.
#[test]
fn resume_with_waiting_or_added_documents_shows_the_overview() {
    // wartend (Dateien im Arbeitsbereich hinzugefügt): wie „Zum Stapel“
    let (s, p) = placed_by_hand(1);
    let s = step(s, Event::FilesAdded(vec![doc(2)]), &ctx()).state;
    assert!(matches!(&s, State::Positioning { queue, .. } if queue.pending.len() == 1 && queue.batch.is_none()));
    let parked = home(s.clone()).state;
    assert!(parked.resumes_to_overview(&ctx()));
    let t = resume(parked);
    assert!(matches!(t.state, State::BatchOverview { .. }) && !t.state.parked(), "{}", t.state.name());
    assert_eq!(plan_of(&t.state, 1), Some(PositionChoice::Manual(p)));
    assert_eq!(status_of(&t.state, 2), BatchItemStatus::Analyzing);
    assert!(t.effects.contains(&Effect::Discard(DocId(1))) && t.effects.contains(&Effect::Preanalyze(DocId(2))));
    assert_eq!(opens(&t.effects), 0);
    assert_eq!(t, step(s, Event::ShowBatch, &ctx()), "wie „Zum Stapel“");
    // „+ PDF“: Stapel aus dem offenen Dokument, ein Eintrag geöffnet und verschoben, geparkt → Übersicht mit beiden
    // Plänen
    let (s, p1) = placed_by_hand(1);
    let s = step(s, Event::MoreFilesAdded(vec![doc(2)]), &ctx()).state;
    let s = step(s, Event::Preanalyzed { doc: DocId(2), result: Ok(summary(Vec::new())) }, &ctx()).state;
    let p2 = Placement { page_index: 0, x: 80.0, y: 120.0, width: 230.0, height: 49.0 };
    let s = step(opened_entry(s, 2), Event::PlacementChanged(p2), &ctx()).state;
    let t = resume(home(s).state);
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    assert_eq!(
        (plan_of(&t.state, 1), plan_of(&t.state, 2)),
        (Some(PositionChoice::Manual(p1)), Some(PositionChoice::Manual(p2)))
    );
    // ohne Stapelübersicht (nur Arbeitsbereich): unverändert zurück, die wartenden bleiben in der Warteschlange
    let single = Context { workspace: true, ..Context::default() };
    let s = step(workspace_with(1, &single), Event::FilesAdded(vec![doc(2)]), &single).state;
    let parked = step(s.clone(), Event::WentHome, &single).state;
    assert!(parked.parked() && !parked.resumes_to_overview(&single));
    assert_eq!(step(parked, Event::ParkedResumed, &single).state, s);
}

/// Analysewarnung eines Stapels: aus der Übersicht geöffnet zurück (Eintrag unverändert), mit wartenden Dokumenten
/// als Eintrag der Übersicht („Bitte prüfen“ nach der Voranalyse). Ein einzelnes Dokument mit Warnung bleibt.
#[test]
fn resume_of_a_batch_analysis_warning_shows_the_overview() {
    let warning = |id: u64| {
        let mut e = analyzed(id, 3, None, false);
        if let Event::Analyzed { warnings, .. } = &mut e {
            warnings.push("pdf-usage-rights".into());
        }
        e
    };
    // aus der Übersicht geöffnet (Eintrag „Bitte prüfen“)
    let s = step(State::Idle { queue: Queue::default() }, Event::FilesAdded(vec![doc(1), doc(2)]), &ctx()).state;
    let s = step(s, Event::Preanalyzed { doc: DocId(1), result: Ok(summary(vec!["pdf-usage-rights".into()])) }, &ctx())
        .state;
    let s = step(s, Event::Preanalyzed { doc: DocId(2), result: Ok(summary(Vec::new())) }, &ctx()).state;
    let review = status_of(&s, 1);
    assert!(matches!(review, BatchItemStatus::NeedsReview { .. }));
    let s = step(s, Event::BatchItemOpened(DocId(1)), &ctx()).state;
    let s = step(s, warning(1), &ctx()).state;
    assert!(matches!(s, State::AnalysisWarning { .. }) && s.opened_from_batch(), "{}", s.name());
    let t = resume(home(s.clone()).state);
    assert!(matches!(t.state, State::BatchOverview { .. }) && !t.state.parked(), "{}", t.state.name());
    assert_eq!(status_of(&t.state, 1), review, "Eintrag unverändert");
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1))]);
    assert_eq!(t, step(s, Event::BackToBatch, &ctx()), "wie „Zurück zum Stapel“");
    // Einzeldokument mit Warnung, ein weiteres kam dazu („+ PDF“ in der Warnung: an die Warteschlange)
    let s = step(State::Idle { queue: Queue::default() }, Event::FilesAdded(vec![doc(1)]), &ctx()).state;
    let s = step(s, warning(1), &ctx()).state;
    let s = step(s, Event::MoreFilesAdded(vec![doc(2)]), &ctx()).state;
    assert!(matches!(&s, State::AnalysisWarning { queue, .. } if queue.pending.len() == 1));
    let t = resume(home(s).state);
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    assert_eq!(batch_of(&t.state).items.iter().map(|i| i.doc.0).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1)), Effect::Preanalyze(DocId(1)), Effect::Preanalyze(DocId(2))]);
    let t = step(
        t.state,
        Event::Preanalyzed { doc: DocId(1), result: Ok(summary(vec!["pdf-usage-rights".into()])) },
        &ctx(),
    );
    assert!(matches!(status_of(&t.state, 1), BatchItemStatus::NeedsReview { .. }), "Warnung bleibt zu prüfen");
}

/// „Fertig“ ohne Speichern eines Stapeleintrags: zurück zu „Fertig“ (die Signatur ist erst gespeichert sicher); das
/// Speichern führt dann wie bisher in die Übersicht. Eine geparkte Übersicht bleibt, wie sie ist.
#[test]
fn unsaved_entry_resumes_to_done_and_saving_returns_to_the_overview() {
    let s = opened_entry(overview(&[1, 2]), 1);
    let (s, _) = run(
        s,
        [
            Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            Event::OutputChosen(None),
        ],
        &ctx(),
    );
    assert!(matches!(s, State::Output { saved: None, decision: None, .. }) && s.opened_from_batch(), "{}", s.name());
    let parked = home(s.clone()).state;
    assert!(parked.parked() && !parked.resumes_to_overview(&ctx()));
    let t = resume(parked);
    assert_eq!((t.state.clone(), t.effects.len()), (s, 0), "„Fertig“ unverändert");
    let t = step(t.state, Event::Saved(output_info(1)), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    assert!(matches!(status_of(&t.state, 1), BatchItemStatus::Signed { .. }));
    // Übersicht: unverändert
    let s = overview(&[1, 2]);
    let t = resume(home(s.clone()).state);
    assert_eq!((t.state, t.effects.len()), (s, 0));
}

/// „Zum Stapel hinzufügen“ bzw. „Öffnen mit“ bei einem geparkten Eintrag: Übersicht mit dem Plan des Eintrags und
/// den neuen Dokumenten – auch nur mit Duplikaten die Übersicht, nie das Einzeldokument. Ein geparktes
/// Einzeldokument wird zum Stapel; parkt man danach einen Eintrag, führt „Weiter bearbeiten“ ebenfalls zur Übersicht.
#[test]
fn join_of_a_parked_entry_shows_the_overview_and_stays_a_batch() {
    let p = Placement { page_index: 0, x: 60.0, y: 200.0, width: 230.0, height: 49.0 };
    let s = step(opened_entry(overview(&[1, 2]), 1), Event::PlacementChanged(p), &ctx()).state;
    let parked = home(s).state;
    let t = step(parked.clone(), Event::MoreFilesAdded(vec![doc(3)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }) && !t.state.parked(), "{}", t.state.name());
    assert_eq!(batch_of(&t.state).items.iter().map(|i| i.doc.0).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(plan_of(&t.state, 1), Some(PositionChoice::Manual(p)));
    assert_eq!(t.effects, vec![Effect::Discard(DocId(1)), Effect::Preanalyze(DocId(3))]);
    // nur Duplikate: trotzdem die Übersicht (wie „Weiter bearbeiten“)
    let dup = pida_workflow::InputDocument::new(DocId(9), "/in/doc2.pdf");
    let t = step(parked.clone(), Event::MoreFilesAdded(vec![dup]), &ctx());
    assert_eq!(t, resume(parked.clone()));
    // „Öffnen mit“: dazu, mit Hinweis
    let t = step(parked, Event::FilesAdded(vec![doc(3)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }));
    assert!(notified(&t.effects, MSG_PARKED_FILES_ADDED));
    // geparktes Einzeldokument + „Zum Stapel hinzufügen“ → Stapel; Eintrag öffnen, parken, fortsetzen → Übersicht
    let (s, p1) = placed_by_hand(1);
    let t = step(home(s).state, Event::MoreFilesAdded(vec![doc(2)]), &ctx());
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    let s = step(t.state, Event::Preanalyzed { doc: DocId(2), result: Ok(summary(Vec::new())) }, &ctx()).state;
    let t = resume(home(opened_entry(s, 2)).state);
    assert!(matches!(t.state, State::BatchOverview { .. }), "{}", t.state.name());
    assert_eq!(plan_of(&t.state, 1), Some(PositionChoice::Manual(p1)));
    assert_eq!(opens(&t.effects), 0);
}

/// Wohin „Weiter bearbeiten“ führt: Übersicht für Stapel, sonst die Arbeit selbst.
#[test]
fn resumes_to_overview_only_for_batches() {
    let c = ctx();
    assert!(overview(&[1, 2]).resumes_to_overview(&c));
    assert!(opened_entry(overview(&[1, 2]), 1).resumes_to_overview(&c));
    assert!(!workspace_with(1, &c).resumes_to_overview(&c), "Einzeldokument");
    assert!(!placed_by_hand(1).0.resumes_to_overview(&c));
    // ohne Stapelübersicht nie
    let single = Context { workspace: true, ..Context::default() };
    assert!(!positioning(&[1, 2]).resumes_to_overview(&single));
    assert!(!State::Idle { queue: Queue::default() }.resumes_to_overview(&c));
}

#[test]
fn home_while_signing_running_or_failed_has_no_effect() {
    let signing = State::Signing {
        queue: queue(&[1]),
        position: PositionChoice::Auto,
        backend: BackendId::Keystore,
        phase: SigningPhase::Signing,
        doc: facts(1, 3),
        flow: pida_workflow::SignFlow::default(),
        edit: None,
    };
    let error = State::Error {
        queue: queue(&[1]),
        error: WorkflowError::Unexpected("x".into()),
        retry: None,
        from: Some(Box::new(signing.clone())),
    };
    let s = overview(&[1, 2]);
    let running = step(
        s,
        Event::BatchStarted {
            backend: BackendId::Keystore,
            docs: vec![DocId(1), DocId(2)],
            output: Some(BatchOutput::Configured),
            on_existing: ExistingPolicy::KeepBoth,
            options: options(true, true),
        },
        &ctx(),
    )
    .state;
    assert!(matches!(running, State::BatchSigning { .. }));
    for s in [signing, error, running, State::Idle { queue: Queue::default() }] {
        let t = home(s.clone());
        assert_eq!((t.state, t.effects.len()), (s.clone(), 0), "{}", s.name());
    }
}

#[test]
fn without_workspace_home_is_open_as_before() {
    let classic = Context::default();
    let s = positioning(&[1, 2]);
    let a = step(s.clone(), Event::WentHome, &classic);
    let b = step(s, Event::Navigate(Step::Open), &classic);
    assert_eq!(a, b);
    assert!(!a.state.parked());
    // ohne geparkte Arbeit: Fortsetzen/Verwerfen ohne Wirkung
    let s = workspace_with(1, &ctx());
    for e in [Event::ParkedResumed, Event::ParkedDiscarded] {
        assert_eq!(step(s.clone(), e, &ctx()).state, s);
    }
}

#[test]
fn next_document_on_done_and_new_batch_stay_as_before() {
    // „Fertig“ → „Nächstes Dokument“ (`Navigate(Open)`) startet weiterhin das nächste Dokument
    let single = Context { workspace: true, ..Context::default() };
    let s = workspace_with(1, &single);
    let s = step(s, Event::FilesAdded(vec![doc(2)]), &single).state;
    let (s, _) = run(
        s,
        [
            Event::SignRequested { backend: BackendId::Keystore, options: options(true, true) },
            Event::PhaseCompleted(SigningPhase::OpeningSession),
            Event::PhaseCompleted(SigningPhase::ReadingCertificate),
            Event::PhaseCompleted(SigningPhase::Preparing),
            Event::PhaseCompleted(SigningPhase::Signing),
            Event::PhaseCompleted(SigningPhase::Finishing),
            Event::Saved(output_info(1)),
        ],
        &single,
    );
    let t = step(s, Event::Navigate(Step::Open), &single);
    assert!(
        matches!(&t.state, State::Analyzing { queue } if queue.current_id() == Some(DocId(2))),
        "{}",
        t.state.name()
    );
    // „Neuer Stapel“ (`Navigate(Open)` in der Übersicht) verwirft den Stapel weiterhin
    let t = step(overview(&[1, 2]), Event::Navigate(Step::Open), &ctx());
    assert!(matches!(&t.state, State::Idle { queue } if queue.is_empty() && !queue.parked));
}

#[test]
fn quit_while_parked_ends_like_before() {
    let parked = home(placed_by_hand(1).0).state;
    let t = step(parked, Event::Quit, &ctx());
    assert!(matches!(t.state, State::Finished { .. }));
    assert!(matches!(t.state.results(), [o] if o.result == DocResult::Cancelled));
}
