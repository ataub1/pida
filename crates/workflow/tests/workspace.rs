//! Tabellentests des Arbeitsbereich-Modus (ADR 0004 §3.2–§3.7, Paket K1): automatische Wege führen in den
//! Arbeitsbereich, „Signieren mit X“ (`SignRequested`) ist der einzige Weg zur Sitzung (A1), Abbruch und
//! Fehler kehren mit unverändertem Bearbeitungszustand zurück, Wiederholen hält die Sitzung offen.

mod common;

use common::*;
use pida_backend_api::{BackendError, BackendId};
use pida_placement::Placement;
use pida_workflow::{
    Context, DocFacts, DocId, Effect, Event, PlaceholderFound, PlacementOrigin, PositionChoice, RetryTarget,
    SigningPhase, State, Step, Target, WorkflowError, WorkspaceEdit,
};

fn ws() -> Context {
    Context { workspace: true, ..Context::default() }
}

fn analyzed_with(id: u64, pages: usize, placeholder: Option<PlaceholderFound>, fields: usize, start: bool) -> Event {
    let Event::Analyzed { doc, class, signed, page_count, .. } = analyzed(id, pages, None, false) else {
        unreachable!()
    };
    Event::Analyzed {
        doc,
        class,
        placeholder,
        signed,
        page_count,
        start: start.then(|| start_placement(pages)),
        warnings: Vec::new(),
        fields: (0..fields).map(|i| field(&format!("Sig{i}"))).collect(),
        new_visible_field_allowed: true,
    }
}

/// Arbeitsbereich wie nach der Analyse (Dokument 1, drei Seiten, ein leeres Feld, QR-Platzhalter bekannt).
fn workspace_state(target: Target, origin: PlacementOrigin, invisible: bool) -> State {
    let edit = WorkspaceEdit {
        target,
        origin,
        invisible,
        fields: vec![field("Sig1")],
        qr: Some(qr()),
        return_to_batch: false,
        parked: None,
    };
    State::Positioning {
        queue: queue(&[1]),
        page: 2,
        placement: Some(start_placement(3)),
        new_page: false,
        doc: facts(1, 3),
        edit: Some(edit),
    }
}

fn edit_of(s: &State) -> &WorkspaceEdit {
    match s {
        State::Positioning { edit: Some(e), .. }
        | State::Signing { edit: Some(e), .. }
        | State::ChoosingBackend { edit: Some(e), .. }
        | State::Output { edit: Some(e), .. } => e,
        other => panic!("kein Bearbeitungszustand: {other:?}"),
    }
}

fn opens_session(effects: &[Effect]) -> bool {
    effects.iter().any(|e| matches!(e, Effect::OpenSession(_)))
}

fn sign_with(backend: BackendId) -> Event {
    Event::SignRequested { backend, options: options(true, true) }
}

// ---------------------------------------------------------------------------------------------
// Nach der Analyse: immer Arbeitsbereich (keine Platzhalterfrage, keine automatische Wahl)
// ---------------------------------------------------------------------------------------------

#[test]
fn analysis_leads_to_workspace_with_automatic_start_placement() {
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed_with(1, 3, None, 0, true), &ws());
    let State::Positioning { page: 2, placement: Some(p), new_page: false, .. } = &t.state else {
        panic!("{:?}", t.state)
    };
    assert_eq!(*p, start_placement(3), "bis AutoPlaced eintrifft: bisherige Startlage (50 %/25 %)");
    let e = edit_of(&t.state);
    assert_eq!((e.target, e.origin, e.invisible), (Target::Free, PlacementOrigin::Auto, false));
    assert_eq!(
        t.effects,
        vec![
            Effect::RenderPage { doc: DocId(1), page: 2 },
            Effect::BuildBlockPreview(DocId(1)),
            Effect::AutoPlace(DocId(1))
        ]
    );
}

/// Jeder automatische Weg (unsichtbar, `auto_position`, Platzhalter, ohne Vorschau) endet im Arbeitsbereich.
#[test]
fn automatic_paths_end_in_the_workspace() {
    let cases: Vec<(&str, Context, Event)> = vec![
        ("unsichtbar", Context { invisible: true, ..ws() }, analyzed_with(1, 3, None, 0, true)),
        ("auto_position", Context { auto_position: true, ..ws() }, analyzed_with(1, 3, None, 0, true)),
        (
            "QR-Platzhalter",
            Context { placeholder_search: true, ..ws() },
            analyzed_with(1, 3, Some(PlaceholderFound::Qr(qr())), 0, true),
        ),
        (
            "Felder (kind = fields)",
            Context { placeholder_search: true, placeholder_fields: true, ..ws() },
            analyzed_with(1, 3, Some(PlaceholderFound::Fields(vec![field("Sig0")])), 1, true),
        ),
        ("ohne Vorschau", ws(), analyzed_with(1, 3, None, 0, false)),
        (
            "Standard ID Austria",
            Context { default_backend: Some(pida_config::BackendChoice::IdAustria), auto_position: true, ..ws() },
            analyzed_with(1, 3, None, 0, true),
        ),
        (
            "Standard Keystore",
            Context { default_backend: Some(pida_config::BackendChoice::Keystore), auto_position: true, ..ws() },
            analyzed_with(1, 3, None, 0, true),
        ),
    ];
    for (name, ctx, ev) in cases {
        let t = step(State::Analyzing { queue: queue(&[1]) }, ev, &ctx);
        assert!(matches!(t.state, State::Positioning { edit: Some(_), .. }), "{name}: {:?}", t.state);
        assert!(!t.effects.contains(&Effect::ProbeBackends) && !opens_session(&t.effects), "{name}: {:?}", t.effects);
    }
}

#[test]
fn invisible_profile_qr_fields_and_missing_preview() {
    // unsichtbares Profil: unsichtbar, keine automatische Platzierung
    let t = step(
        State::Analyzing { queue: queue(&[1]) },
        analyzed_with(1, 3, None, 0, true),
        &Context { invisible: true, ..ws() },
    );
    assert!(edit_of(&t.state).invisible);
    assert!(!t.effects.contains(&Effect::AutoPlace(DocId(1))));
    // QR-Platzhalter mit Platzhaltersuche (kind = marker): Ziel QR, automatisch, umkehrbar
    let ctx = Context { placeholder_search: true, ..ws() };
    let t = step(
        State::Analyzing { queue: queue(&[1]) },
        analyzed_with(1, 3, Some(PlaceholderFound::Qr(qr())), 0, true),
        &ctx,
    );
    let e = edit_of(&t.state);
    assert_eq!((e.target, e.origin), (Target::Qr, PlacementOrigin::Qr));
    assert!(!t.effects.contains(&Effect::AutoPlace(DocId(1))));
    // ohne Platzhaltersuche kein QR-Ziel
    let t = step(
        State::Analyzing { queue: queue(&[1]) },
        analyzed_with(1, 3, Some(PlaceholderFound::Qr(qr())), 0, true),
        &ws(),
    );
    assert_eq!(edit_of(&t.state).target, Target::Free);
    assert!(edit_of(&t.state).qr.is_none());
    // leere Felder: immer angeboten (auch ohne Platzhaltersuche), nie automatisch benutzt (§8 F2)
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed_with(1, 3, None, 2, true), &ws());
    let e = edit_of(&t.state);
    assert_eq!((e.fields.len(), e.target), (2, Target::Free));
    // ohne Vorschau: Arbeitsbereich ohne Lage, Hinweis, keine automatische Platzierung
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed_with(1, 3, None, 0, false), &ws());
    assert!(matches!(t.state, State::Positioning { placement: None, .. }), "{:?}", t.state);
    assert_eq!(t.effects, vec![Effect::Notify("error-PositioningNotPossible".into())]);
    // Renderer fällt später aus: bleibt im Arbeitsbereich (A1), signiert automatisch platziert
    let s = workspace_state(Target::Free, PlacementOrigin::Auto, false);
    let t = step(s, Event::PreviewUnavailable { doc: DocId(1) }, &ws());
    assert!(matches!(t.state, State::Positioning { placement: None, edit: Some(_), .. }), "{:?}", t.state);
    let t = step(t.state, sign_with(BackendId::Keystore), &ws());
    assert!(matches!(&t.state, State::Signing { position: PositionChoice::Auto, .. }), "{:?}", t.state);
}

#[test]
fn analysis_warning_then_workspace() {
    let Event::Analyzed { doc, class, placeholder, signed, page_count, start, fields, .. } =
        analyzed_with(1, 3, None, 1, true)
    else {
        unreachable!()
    };
    let ev = Event::Analyzed {
        doc,
        class,
        placeholder,
        signed,
        page_count,
        start,
        warnings: vec!["pdf-usage-rights".into()],
        fields,
        new_visible_field_allowed: true,
    };
    let t = step(State::Analyzing { queue: queue(&[1]) }, ev, &ws());
    assert!(matches!(t.state, State::AnalysisWarning { .. }));
    let t = step(t.state, Event::WarningAnswered(true), &ws());
    assert_eq!(edit_of(&t.state).fields.len(), 1, "Felder gelten nach „Fortfahren“");
}

// ---------------------------------------------------------------------------------------------
// Bearbeiten: Ziele, unsichtbar, automatische Platzierung
// ---------------------------------------------------------------------------------------------

#[test]
fn user_moves_set_free_target_and_user_origin() {
    let s = workspace_state(Target::Field(0), PlacementOrigin::Field, false);
    let p = Placement { page_index: 1, x: 10.0, y: 300.0, width: 230.0, height: 49.0 };
    let t = step(s.clone(), Event::PlacementChanged(p), &ws());
    assert_eq!((edit_of(&t.state).target, edit_of(&t.state).origin), (Target::Free, PlacementOrigin::User));
    let t = step(s.clone(), Event::PageChanged(0), &ws());
    assert_eq!(edit_of(&t.state).origin, PlacementOrigin::User);
    let t = step(s, Event::NewPageToggled, &ws());
    assert_eq!(edit_of(&t.state).target, Target::Free);
}

#[test]
fn auto_placement_applies_only_while_automatic() {
    let s = workspace_state(Target::Free, PlacementOrigin::Auto, false);
    let auto = Placement { page_index: 3, x: 50.0, y: 822.0, width: 230.0, height: 49.0 };
    // automatische Lage auf einer neuen Seite
    let t = step(s.clone(), Event::AutoPlaced(auto), &ws());
    let State::Positioning { page: 3, new_page: true, placement: Some(p), .. } = &t.state else {
        panic!("{:?}", t.state)
    };
    assert_eq!(*p, auto);
    assert_eq!(edit_of(&t.state).origin, PlacementOrigin::Auto);
    // nach einer Bewegung der Nutzerin wird ein verspätetes Ergebnis verworfen
    let moved = Placement { page_index: 0, x: 1.0, y: 100.0, width: 230.0, height: 49.0 };
    let m = step(s.clone(), Event::PlacementChanged(moved), &ws()).state;
    let t = step(m.clone(), Event::AutoPlaced(auto), &ws());
    assert_eq!(t.state, m);
    // „Freie Fläche suchen“ / „Frei platzieren“ fordert sie neu an
    let t = step(m, Event::TargetChosen(Target::Free), &ws());
    assert_eq!(edit_of(&t.state).origin, PlacementOrigin::Auto);
    assert_eq!(t.effects, vec![Effect::AutoPlace(DocId(1))]);
    // signiertes Dokument: neue Seite nicht erlaubt → letzte Seite mit Hinweis
    let State::Positioning { queue, page, placement, new_page, doc, edit } = s else { unreachable!() };
    let signed = State::Positioning { queue, page, placement, new_page, doc: DocFacts { signed: true, ..doc }, edit };
    let t = step(signed, Event::AutoPlaced(auto), &ws());
    assert!(matches!(t.state, State::Positioning { page: 2, new_page: false, .. }), "{:?}", t.state);
    assert!(t.effects.contains(&Effect::Notify("placement.notice.new_page_not_allowed".into())));
}

#[test]
fn block_resize_replaces_automatic_placement_only() {
    let resized = Placement { page_index: 2, x: 297.5, y: 210.5, width: 300.0, height: 60.0 };
    let t = step(workspace_state(Target::Free, PlacementOrigin::Auto, false), Event::BlockResized(resized), &ws());
    assert_eq!(t.effects, vec![Effect::AutoPlace(DocId(1))], "automatisch platziert: neu setzen");
    assert_eq!(edit_of(&t.state).origin, PlacementOrigin::Auto);
    let t = step(workspace_state(Target::Free, PlacementOrigin::User, false), Event::BlockResized(resized), &ws());
    assert!(t.effects.is_empty(), "manuell: Lage behalten (geklemmt)");
    assert_eq!(edit_of(&t.state).origin, PlacementOrigin::User);
    assert!(matches!(&t.state, State::Positioning { placement: Some(p), .. } if *p == resized));
    // Feld bleibt Feld
    let t = step(workspace_state(Target::Field(0), PlacementOrigin::Field, false), Event::BlockResized(resized), &ws());
    assert_eq!(edit_of(&t.state).target, Target::Field(0));
}

#[test]
fn targets_and_invisible_switch() {
    let s = workspace_state(Target::Free, PlacementOrigin::Auto, false);
    let t = step(s.clone(), Event::TargetChosen(Target::Field(0)), &ws());
    assert_eq!((edit_of(&t.state).target, edit_of(&t.state).origin), (Target::Field(0), PlacementOrigin::Field));
    // unbekanntes bzw. gesperrtes Feld: abgelehnt mit Hinweis
    let t = step(s.clone(), Event::TargetChosen(Target::Field(9)), &ws());
    assert_eq!(edit_of(&t.state).target, Target::Free);
    assert_eq!(t.effects, vec![Effect::Notify("ng-field-locked".into())]);
    let t = step(s.clone(), Event::TargetChosen(Target::Qr), &ws());
    assert_eq!(edit_of(&t.state).target, Target::Qr);
    // „Unsichtbar“ als Schalter (nur dieses Dokument)
    let t = step(s.clone(), Event::InvisibleSet(true), &ws());
    assert!(edit_of(&t.state).invisible);
    let t = step(t.state, Event::InvisibleSet(false), &ws());
    assert!(!edit_of(&t.state).invisible);
    // unsichtbares Profil: sichtbar nicht möglich
    let inv = Context { invisible: true, ..ws() };
    let t = step(workspace_state(Target::Free, PlacementOrigin::Auto, true), Event::InvisibleSet(false), &inv);
    assert!(edit_of(&t.state).invisible);
    assert_eq!(t.effects, vec![Effect::Notify("ng-constraint-invisible-profile".into())]);
    // Altereignisse starten im Arbeitsbereich nichts
    for e in [Event::PositionConfirmed, Event::InvisibleChosen] {
        let t = step(s.clone(), e, &ws());
        assert_eq!(t.state, s);
        assert!(t.effects.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// „Signieren mit X“ (§3.5)
// ---------------------------------------------------------------------------------------------

#[test]
fn sign_with_takes_the_position_from_the_workspace() {
    let p = start_placement(3);
    let cases = [
        (Target::Free, PlacementOrigin::Auto, false, PositionChoice::Manual(p)),
        (Target::Free, PlacementOrigin::User, false, PositionChoice::Manual(p)),
        (Target::Field(0), PlacementOrigin::Field, false, PositionChoice::Field(field("Sig1"))),
        (Target::Qr, PlacementOrigin::Qr, false, PositionChoice::Placeholder(qr())),
        (Target::Field(0), PlacementOrigin::Field, true, PositionChoice::Invisible),
    ];
    for (target, origin, invisible, expected) in cases {
        let t = step(workspace_state(target, origin, invisible), sign_with(BackendId::AtrustNative), &ws());
        let State::Signing { position, backend, phase: SigningPhase::OpeningSession, flow, .. } = &t.state else {
            panic!("{target:?}: {:?}", t.state)
        };
        assert_eq!(position, &expected, "{target:?}");
        assert_eq!(*backend, BackendId::AtrustNative);
        assert_eq!(flow.needs_certificate, !invisible, "ID Austria braucht das Zertifikat nur sichtbar");
        assert_eq!(t.effects, vec![Effect::OpenSession(BackendId::AtrustNative)], "kein Auswahlschritt");
        let parked = edit_of(&t.state).parked.expect("Lage gemerkt");
        assert_eq!((parked.page, parked.placement, parked.new_page), (2, Some(p), false));
    }
    // neue Seite nur mit „Auf neuer Seite“
    let s = step(workspace_state(Target::Free, PlacementOrigin::User, false), Event::NewPageToggled, &ws()).state;
    let t = step(s, sign_with(BackendId::Keystore), &ws());
    assert!(matches!(&t.state, State::Signing { position: PositionChoice::Manual(p), .. } if p.page_index == 3));
}

#[test]
fn sign_with_unavailable_signer_or_forbidden_position_stays() {
    let s = workspace_state(Target::Free, PlacementOrigin::Auto, false);
    // lokale BKU nicht verfügbar
    let t = step(s.clone(), sign_with(BackendId::LocalBku), &Context { ..ws() });
    let t2 =
        step(s.clone(), Event::SignRequested { backend: BackendId::LocalBku, options: options(false, true) }, &ws());
    assert!(matches!(t.state, State::Signing { .. }), "verfügbar laut Optionen");
    assert_eq!(t2.state, s);
    assert_eq!(t2.effects, vec![Effect::Notify("ng-signer-unavailable".into())]);
    // DocMDP: kein neues sichtbares Feld – vorhandenes Feld bzw. unsichtbar erlaubt
    let certified = |target, invisible| {
        let State::Positioning { queue, page, placement, new_page, doc, edit } =
            workspace_state(target, PlacementOrigin::User, invisible)
        else {
            unreachable!()
        };
        let doc = DocFacts { new_visible_field_allowed: false, signed: true, ..doc };
        State::Positioning { queue, page, placement, new_page, doc, edit }
    };
    let t = step(certified(Target::Free, false), sign_with(BackendId::Keystore), &ws());
    assert!(matches!(t.state, State::Positioning { .. }));
    assert_eq!(t.effects, vec![Effect::Notify("ng-constraint-certified".into())]);
    let t = step(certified(Target::Qr, false), sign_with(BackendId::Keystore), &ws());
    assert!(matches!(t.state, State::Positioning { .. }));
    for (target, invisible) in [(Target::Field(0), false), (Target::Free, true)] {
        let t = step(certified(target, invisible), sign_with(BackendId::Keystore), &ws());
        assert!(matches!(t.state, State::Signing { .. }), "{target:?}/{invisible}: {:?}", t.state);
    }
    // klassischer Ablauf: ohne Wirkung
    let classic = positioning(&[1]);
    let t = step(classic.clone(), sign_with(BackendId::Keystore), &Context::default());
    assert_eq!(t.state, classic);
}

// ---------------------------------------------------------------------------------------------
// Signieren, Abbruch, Fehler, Wiederholen (§3.6)
// ---------------------------------------------------------------------------------------------

fn signing_ws(backend: BackendId) -> State {
    let s = workspace_state(Target::Free, PlacementOrigin::User, false);
    step(s, sign_with(backend), &ws()).state
}

#[test]
fn signing_phases_in_the_workspace() {
    let s = signing_ws(BackendId::AtrustNative);
    let t = step(s, Event::PhaseCompleted(SigningPhase::OpeningSession), &ws());
    assert_eq!(t.effects, vec![Effect::ReadCertificate]);
    let t = step(t.state, Event::PhaseCompleted(SigningPhase::ReadingCertificate), &ws());
    assert!(matches!(t.effects[..], [Effect::Prepare { .. }]));
    let t = step(t.state, Event::PhaseCompleted(SigningPhase::Preparing), &ws());
    assert_eq!(t.effects, vec![Effect::SignDocument], "ein Job mit eigenem Abbruch (sign_one)");
    let t = step(t.state, Event::PhaseCompleted(SigningPhase::Signing), &ws());
    assert_eq!(t.effects, vec![Effect::Finish], "Sitzung bleibt bis nach der Selbstprüfung offen");
    let t = step(t.state, Event::PhaseCompleted(SigningPhase::Finishing), &ws());
    assert!(matches!(t.state, State::Output { edit: Some(_), .. }));
    assert_eq!(t.effects, vec![Effect::CloseSession, Effect::DeletePersisted, Effect::DecideOutput(DocId(1))]);
}

/// Abbruch (Esc/× bzw. Abbruch einer Maske) → Arbeitsbereich mit unveränderter Platzierung und Hinweis.
#[test]
fn cancel_returns_to_the_unchanged_workspace() {
    for target in [Target::Free, Target::Field(0), Target::Qr] {
        let origin = match target {
            Target::Free => PlacementOrigin::User,
            Target::Field(_) => PlacementOrigin::Field,
            Target::Qr => PlacementOrigin::Qr,
        };
        let before = workspace_state(target, origin, false);
        let s = step(before.clone(), sign_with(BackendId::AtrustNative), &ws()).state;
        for ev in [Event::Cancel, Event::BackendFailed(BackendError::Cancelled)] {
            let t = step(s.clone(), ev.clone(), &ws());
            assert_eq!(t.state, before, "{target:?} {ev:?}");
            assert_eq!(
                &t.effects[..3],
                &[Effect::CancelSession, Effect::DeletePersisted, Effect::Notify("ng-signing-cancelled".into())]
            );
            assert!(!opens_session(&t.effects));
        }
    }
}

#[test]
fn declined_in_the_app_keeps_the_session_for_retry() {
    let s = signing_ws(BackendId::AtrustNative);
    let t = step(s, Event::SigningDeclined, &ws());
    let State::Error { error, retry: Some(RetryTarget::Prepare), from: Some(_), .. } = &t.state else {
        panic!("{:?}", t.state)
    };
    assert_eq!(error.code(), "ng-signing-declined");
    assert_eq!(t.effects, vec![Effect::ReleaseDocument, Effect::DeletePersisted], "Sitzung bleibt offen");
    // „Erneut versuchen“: neue Bestätigung in derselben Sitzung (core-api verwendet sie wieder)
    let r = step(t.state.clone(), Event::Retry, &ws());
    assert!(matches!(r.state, State::Signing { phase: SigningPhase::OpeningSession, .. }));
    assert_eq!(r.effects, vec![Effect::OpenSession(BackendId::AtrustNative)]);
    // „Schließen“ = Abbrechen: Arbeitsbereich, Sitzung samt Passwort verworfen (F-72)
    let c = step(t.state, Event::Cancel, &ws());
    assert_eq!(c.state, workspace_state(Target::Free, PlacementOrigin::User, false));
    assert_eq!(c.effects[0], Effect::CancelSession);
}

#[test]
fn errors_keep_the_session_only_when_worth_retrying() {
    let cases: Vec<(Event, bool, Effect)> = vec![
        (Event::BackendFailed(BackendError::Network("x".into())), true, Effect::ReleaseDocument),
        (Event::BackendFailed(BackendError::Timeout), true, Effect::ReleaseDocument),
        (Event::BackendFailed(BackendError::Http { status: 500, reason: "x".into() }), true, Effect::ReleaseDocument),
        (
            Event::BackendFailed(BackendError::SecurityLayer { code: 1, info: "x".into() }),
            true,
            Effect::ReleaseDocument,
        ),
        (Event::BackendFailed(BackendError::WrongPassword), true, Effect::CloseSession),
        (Event::BackendFailed(BackendError::UnknownPage { diagnostic: "x".into() }), false, Effect::CloseSession),
        (Event::BackendFailed(BackendError::DocumentTooLarge), false, Effect::CloseSession),
        (Event::CmsTooLarge { needed: 20_000, reserve: 16_384 }, true, Effect::ReleaseDocument),
        (
            Event::SigningFailed(WorkflowError::Prepare { code: "sigblock-x".into(), detail: String::new() }),
            true,
            Effect::ReleaseDocument,
        ),
        (
            Event::SelfCheckFailed(pida_pades::InvariantViolation {
                invariant: pida_pades::Invariant::I3Cms,
                detail: "x".into(),
            }),
            false,
            Effect::CloseSession,
        ),
    ];
    for (ev, retry, effect) in cases {
        let t = step(signing_ws(BackendId::AtrustNative), ev.clone(), &ws());
        let State::Error { retry: r, .. } = &t.state else { panic!("{ev:?}: {:?}", t.state) };
        assert_eq!(r.is_some(), retry, "{ev:?}");
        assert_eq!(t.effects[0], effect, "{ev:?}");
        // klassisch schließt jeder Fehler die Sitzung
        let classic = signing(&[1], SigningPhase::Signing, BackendId::AtrustNative, PositionChoice::Auto);
        let c = step(classic, ev.clone(), &Context::default());
        if matches!(c.state, State::Error { .. }) {
            assert_eq!(c.effects[0], Effect::CloseSession, "{ev:?}");
        }
    }
}

#[test]
fn error_close_open_and_quit_discard_the_held_session() {
    let err = step(signing_ws(BackendId::AtrustNative), Event::BackendFailed(BackendError::Timeout), &ws()).state;
    let t = step(err.clone(), Event::Navigate(Step::Open), &ws());
    assert!(matches!(t.state, State::Idle { .. }));
    assert_eq!(t.effects[0], Effect::CancelSession);
    let t = step(err.clone(), Event::FilesAdded(vec![doc(7)]), &ws());
    assert!(matches!(t.state, State::Error { .. }));
    assert_eq!(t.effects, vec![Effect::CancelSession]);
    let t = step(err, Event::Quit, &ws());
    assert!(t.effects.starts_with(&[Effect::CancelSession]), "{:?}", t.effects);
}

// ---------------------------------------------------------------------------------------------
// Fertig, Hauptleiste, Startargument-Automatik
// ---------------------------------------------------------------------------------------------

#[test]
fn saved_with_pending_documents_shows_done_in_the_workspace() {
    let mut s = signing_ws(BackendId::Keystore);
    if let Some(q) = s.queue_mut() {
        q.add([doc(2)]);
    }
    for phase in [SigningPhase::OpeningSession, SigningPhase::ReadingCertificate, SigningPhase::Preparing] {
        s = step(s, Event::PhaseCompleted(phase), &ws()).state;
    }
    s = step(s, Event::PhaseCompleted(SigningPhase::Signing), &ws()).state;
    s = step(s, Event::PhaseCompleted(SigningPhase::Finishing), &ws()).state;
    let decided = pida_workflow::OutputDecision::Save { path: "/o.pdf".into(), exists: false };
    s = step(s, Event::OutputDecided(decided), &ws()).state;
    let t = step(s.clone(), Event::Saved(output_info(1)), &ws());
    assert!(matches!(t.state, State::Output { saved: Some(_), .. }), "kein F-88 im Arbeitsbereich: {:?}", t.state);
    // „Nächstes Dokument“
    let n = step(t.state.clone(), Event::Navigate(Step::Open), &ws());
    assert!(matches!(&n.state, State::Analyzing { queue } if queue.current_id() == Some(DocId(2))));
    // Hauptleiste Positionieren/Signieren gibt es im Arbeitsbereich nicht
    for step_ in [Step::Positioning, Step::Signing] {
        let x = step(t.state.clone(), Event::Navigate(step_), &ws());
        assert_eq!(x.state, t.state);
    }
    // mit Startargument-Automatik bleibt F-88 (D-06)
    let cli = Context { cli_auto: true, ..ws() };
    let t = step(s, Event::Saved(output_info(1)), &cli);
    assert!(matches!(&t.state, State::Analyzing { queue } if queue.current_id() == Some(DocId(2))));
}

/// D-06 im Arbeitsbereich: `-b KS -a` signiert automatisch, `-b MOBILE -a` bleibt im Arbeitsbereich.
#[test]
fn cli_auto_signs_only_with_signers_that_keep_the_document_on_the_device() {
    let keystore = Context {
        cli_auto: true,
        auto_position: true,
        default_backend: Some(pida_config::BackendChoice::Keystore),
        ..ws()
    };
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed_with(1, 3, None, 0, true), &keystore);
    assert!(matches!(t.state, State::ChoosingBackend { auto_choice: true, edit: Some(_), .. }), "{:?}", t.state);
    let t = step(t.state, Event::BackendsProbed(options(false, true)), &keystore);
    assert_eq!(t.effects, vec![Effect::OpenSession(BackendId::Keystore)]);
    assert!(matches!(t.state, State::Signing { position: PositionChoice::Auto, .. }));

    let mobile = Context { default_backend: Some(pida_config::BackendChoice::IdAustria), ..keystore.clone() };
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed_with(1, 3, None, 0, true), &mobile);
    let t = step(t.state, Event::BackendsProbed(options(false, true)), &mobile);
    assert!(matches!(t.state, State::Positioning { edit: Some(_), .. }), "{:?}", t.state);
    assert!(!opens_session(&t.effects));
    assert!(t.effects.contains(&Effect::AutoPlace(DocId(1))), "automatische Platzierung als Startlage");
    // ohne Standard (Wahl nicht eindeutig) ebenso
    let none = Context { default_backend: None, ..keystore };
    let t = step(State::Analyzing { queue: queue(&[1]) }, analyzed_with(1, 3, None, 0, true), &none);
    let t = step(t.state, Event::BackendsProbed(only_id_austria()), &none);
    assert!(matches!(t.state, State::Positioning { .. }), "{:?}", t.state);
}

/// Rückfall nach „Signatur fortsetzen“ mit nicht mehr verfügbarem Signiermittel: Auswahl; Wahl dort
/// signiert, Abbrechen führt in den Arbeitsbereich.
#[test]
fn resume_fallback_choice_and_cancel() {
    let plan = pida_workflow::ResumePlan { position: PositionChoice::Auto, backend: BackendId::Keystore };
    let s = State::Resuming { queue: queue(&[1]), plan, doc: None };
    let t = step(s, analyzed_with(1, 3, None, 0, true), &ws());
    let t = step(t.state, Event::BackendsProbed(options(false, false)), &ws());
    assert!(matches!(t.state, State::ChoosingBackend { auto_choice: false, .. }));
    let c = step(t.state.clone(), sign_with(BackendId::AtrustNative), &ws());
    assert_eq!(c.effects, vec![Effect::OpenSession(BackendId::AtrustNative)]);
    let x = step(t.state, Event::Cancel, &ws());
    assert!(
        matches!(x.state, State::Positioning { edit: Some(_), .. })
            || matches!(x.state, State::Analyzing { .. } | State::Idle { .. }),
        "{:?}",
        x.state
    );
}

/// Invariante A1 als Tabelle (ADR 0004 §3.2): je Zeile das Paar aus Ereignis und Vorzustand, das im
/// Arbeitsbereich eine Sitzung öffnet – und die Gegenprobe, die keine öffnet.
#[test]
fn a1_table_of_session_opening_pairs() {
    let cli = Context { cli_auto: true, auto_position: true, ..ws() };
    let with_default = |choice| Context { default_backend: Some(choice), ..cli.clone() };
    let choosing_auto = || {
        let mut s = choosing(&[1], PositionChoice::Auto, true);
        if let State::ChoosingBackend { edit, .. } = &mut s {
            *edit = Some(WorkspaceEdit::new(Vec::new(), None, false));
        }
        s
    };
    let resuming = || {
        let plan = pida_workflow::ResumePlan { position: PositionChoice::Auto, backend: BackendId::Keystore };
        State::Resuming { queue: queue(&[1]), plan, doc: Some(facts(1, 3)) }
    };
    let error_from_signing =
        || step(signing_ws(BackendId::AtrustNative), Event::BackendFailed(BackendError::Timeout), &ws()).state;
    //   (Zeile, Vorzustand, Ereignis, Kontext, öffnet Sitzung)
    let rows: Vec<(&str, State, Event, Context, bool)> = vec![
        (
            "SignRequested in Positioning",
            workspace_state(Target::Free, PlacementOrigin::Auto, false),
            sign_with(BackendId::AtrustNative),
            ws(),
            true,
        ),
        (
            "SignRequested in ChoosingBackend",
            choosing(&[1], PositionChoice::Auto, false),
            sign_with(BackendId::AtrustNative),
            ws(),
            true,
        ),
        (
            "BackendChosen in ChoosingBackend (Rückfall)",
            step(choosing(&[1], PositionChoice::Auto, false), Event::BackendsProbed(options(true, true)), &ws()).state,
            Event::BackendChosen(BackendId::AtrustNative),
            ws(),
            true,
        ),
        ("BackendsProbed in Resuming", resuming(), Event::BackendsProbed(options(false, true)), ws(), true),
        ("Retry im Fehler des Signierens", error_from_signing(), Event::Retry, ws(), true),
        (
            "BackendsProbed, cli_auto, Keystore",
            choosing_auto(),
            Event::BackendsProbed(options(false, true)),
            with_default(pida_config::BackendChoice::Keystore),
            true,
        ),
        // Gegenproben
        (
            "BackendsProbed, cli_auto, ID Austria",
            choosing_auto(),
            Event::BackendsProbed(options(false, true)),
            with_default(pida_config::BackendChoice::IdAustria),
            false,
        ),
        (
            "BackendsProbed ohne cli_auto",
            choosing_auto(),
            Event::BackendsProbed(options(false, true)),
            Context { default_backend: Some(pida_config::BackendChoice::Keystore), ..ws() },
            false,
        ),
        (
            "PositionConfirmed",
            workspace_state(Target::Free, PlacementOrigin::Auto, false),
            Event::PositionConfirmed,
            ws(),
            false,
        ),
        (
            "SettingsChanged",
            workspace_state(Target::Free, PlacementOrigin::Auto, false),
            Event::SettingsChanged,
            ws(),
            false,
        ),
        (
            "Retry im Fehler der Analyse",
            {
                let s = State::Analyzing { queue: queue(&[1]) };
                let err = WorkflowError::Rejected {
                    class: pida_pades::AcceptanceClass::K1Regular,
                    code: "error-MayNotBeAPDF".into(),
                };
                step(s, Event::AnalysisFailed { doc: DocId(1), error: err }, &ws()).state
            },
            Event::Retry,
            ws(),
            false,
        ),
    ];
    for (row, state, event, ctx, expected) in rows {
        let t = step(state, event, &ctx);
        assert_eq!(opens_session(&t.effects), expected, "{row}: {:?}", t.effects);
    }
}
