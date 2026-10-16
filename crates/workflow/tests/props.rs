//! Property-Tests für Warteschlange, Ausgaberegeln und Reducer-Invarianten.

mod common;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use common::*;
use pida_backend_api::BackendError;
use pida_workflow::{
    Context, DocId, Effect, Event, InputDocument, OutputDecision, PathProbe, Queue, SigningPhase, State, Step,
    decide_output, grown_reserve, output_file_name, reduce,
};
use proptest::prelude::*;

struct Fs {
    dirs: HashSet<PathBuf>,
    files: HashSet<PathBuf>,
}

impl PathProbe for Fs {
    fn is_dir(&self, p: &Path) -> bool {
        self.dirs.contains(p)
    }
    fn exists(&self, p: &Path) -> bool {
        self.dirs.contains(p) || self.files.contains(p)
    }
}

fn name_strategy() -> impl Strategy<Value = String> {
    // Dateinamen mit Punkten, Leerzeichen, Umlauten; ohne Pfadtrenner
    "[a-zA-Z0-9äöü ._-]{1,20}".prop_filter("kein . oder ..", |s| s != "." && s != "..")
}

fn state_name(s: &State) -> &'static str {
    s.name()
}

proptest! {
    #[test]
    fn queue_never_holds_duplicate_paths(paths in prop::collection::vec(0u8..12, 0..40)) {
        let mut q = Queue::default();
        for (i, p) in paths.iter().enumerate() {
            q.add([InputDocument::new(DocId(i as u64), format!("/d/{p}.pdf"))]);
        }
        let mut seen = HashSet::new();
        for d in q.current.iter().chain(q.pending.iter()) {
            prop_assert!(seen.insert(d.path.clone()), "Duplikat {:?}", d.path);
        }
        let distinct: HashSet<_> = paths.iter().collect();
        prop_assert_eq!(q.len(), distinct.len());
    }

    #[test]
    fn queue_preserves_order_and_records_all(n in 0usize..20, finish in 0usize..25) {
        let mut q = Queue::default();
        q.add((0..n).map(|i| InputDocument::new(DocId(i as u64), format!("/d/{i}.pdf"))));
        let mut order = Vec::new();
        for _ in 0..finish {
            if let Some(id) = q.current_id() { order.push(id.0); }
            q.finish_current(pida_workflow::DocResult::Cancelled);
        }
        let expect: Vec<u64> = (0..n.min(finish) as u64).collect();
        prop_assert_eq!(&order, &expect);
        prop_assert_eq!(q.done.len(), n.min(finish));
        prop_assert_eq!(q.len() + q.done.len(), n);
    }

    #[test]
    fn output_name_keeps_extension_and_never_ends_with_dot(name in name_strategy(), postfix in "[_a-z]{0,8}") {
        let out = output_file_name(Path::new(&format!("/x/{name}")), &postfix);
        let (base, ext) = match name.rfind('.') {
            Some(i) => (&name[..i], &name[i + 1..]),
            None => (name.as_str(), ""),
        };
        let expected = if ext.is_empty() { format!("{base}{postfix}") } else { format!("{base}{postfix}.{ext}") };
        prop_assert_eq!(&out, &expected);
        // B-10: ohne Erweiterung wird nie ein Punkt angehängt
        if ext.is_empty() {
            prop_assert!(!out.ends_with('.') || (postfix.is_empty() && base.ends_with('.')), "{out}");
        }
        prop_assert!(!out.contains('/'));
    }

    #[test]
    fn output_target_is_folder_join_or_file(name in name_strategy(), is_dir in any::<bool>(), relative in any::<bool>(), exists in any::<bool>()) {
        let input = PathBuf::from(format!("/src/{name}"));
        let configured = if relative { PathBuf::from("out") } else { PathBuf::from("/abs/out") };
        let resolved = if relative { PathBuf::from("/src/out") } else { configured.clone() };
        let mut fs = Fs { dirs: HashSet::new(), files: HashSet::new() };
        if is_dir { fs.dirs.insert(resolved.clone()); }
        let expected_path = if is_dir { resolved.join(output_file_name(&input, "_signed")) } else { resolved.clone() };
        if exists { fs.files.insert(expected_path.clone()); }
        let d = decide_output(&input, Some(&configured), "_signed", &fs);
        prop_assert_eq!(d, OutputDecision::Save { path: expected_path, exists });
    }

    #[test]
    fn grown_reserve_always_fits(needed in 1usize..1_000_000, reserve in 1usize..1_000_000) {
        let n = grown_reserve(needed, reserve);
        prop_assert!(n >= needed * 2);
        prop_assert!(n > reserve);
        prop_assert!(n >= 16 * 1024);
        prop_assert_eq!(n % 1024, 0);
    }

    /// Beliebige Ereignisfolgen: der Reducer paniert nie, die Warteschlange enthält keine Duplikate
    /// und ohne Oberfläche wird nie ein Dialog-Effekt erzeugt.
    #[test]
    fn reducer_is_total_and_keeps_queue_consistent(events in prop::collection::vec(0u8..33, 0..60), headless in any::<bool>()) {
        let ctx = Context { headless, placeholder_search: true, ..Context::default() };
        let mut state = State::Starting;
        let mut next_id = 1u64;
        for e in events {
            let event = match e {
                0 => Event::Started { documents: vec![doc(1), doc(2)] },
                1 => { next_id += 1; Event::FilesAdded(vec![doc(next_id)]) }
                2 => analyzed(state.queue().and_then(Queue::current_id).map(|d| d.0).unwrap_or(1), 2, None, false),
                3 => analyzed(state.queue().and_then(Queue::current_id).map(|d| d.0).unwrap_or(1), 1, Some(pida_workflow::PlaceholderFound::Qr(qr())), true),
                4 => Event::PlaceholderAnswered { answer: pida_workflow::PlaceholderAnswer::Yes, field_index: None },
                5 => Event::PositionConfirmed,
                6 => Event::NewPageToggled,
                7 => Event::BackendsProbed(options(true, true)),
                8 => Event::BackendChosen(pida_backend_api::BackendId::Keystore),
                9 => Event::PhaseCompleted(SigningPhase::OpeningSession),
                10 => Event::PhaseCompleted(SigningPhase::ReadingCertificate),
                11 => Event::PhaseCompleted(SigningPhase::Preparing),
                12 => Event::PhaseCompleted(SigningPhase::Signing),
                13 => Event::PhaseCompleted(SigningPhase::Finishing),
                14 => Event::BackendFailed(BackendError::Timeout),
                15 => Event::OutputDecided(OutputDecision::Save { path: "/o.pdf".into(), exists: e % 2 == 0 }),
                16 => Event::Saved(output_info(1)),
                17 => Event::SaveFailed("x".into()),
                18 => Event::Retry,
                19 => Event::Cancel,
                20 => Event::Navigate(Step::Open),
                21 => Event::Navigate(Step::Positioning),
                22 => Event::OverwriteAnswered(true),
                23 => {
                    let id = state.queue().and_then(Queue::current_id).map(|d| d.0).unwrap_or(1);
                    let Event::Analyzed { doc, class, placeholder, signed, page_count, start, .. } = analyzed(id, 2, None, false) else { unreachable!() };
                    Event::Analyzed { doc, class, placeholder, signed, page_count, start, warnings: vec!["pdf-usage-rights".into()], fields: Vec::new(), new_visible_field_allowed: true }
                }
                24 => Event::WarningAnswered(e % 2 == 0),
                25 => Event::WarningAnswered(true),
                26 => Event::PageChanged(0),
                // Mobile-Lebenszyklus (Plan §3.5): App-Wechsel, „Signatur fortsetzen“, Teilen/Exportieren
                28 => Event::Backgrounded,
                29 => Event::Foregrounded,
                30 => {
                    next_id += 1;
                    let plan = pida_workflow::ResumePlan {
                        position: pida_workflow::PositionChoice::Auto,
                        backend: pida_backend_api::BackendId::Keystore,
                    };
                    Event::ResumeRequested { document: doc(next_id), plan, pending: vec![doc(1)] }
                }
                31 => Event::ShareResultRequested,
                32 => Event::ExportResultRequested,
                _ => Event::Navigate(Step::Signing),
            };
            let lifecycle = matches!(event, Event::Backgrounded | Event::Foregrounded);
            let before = state.clone();
            let t = reduce(state, event, &ctx);
            if lifecycle {
                // ein App-Wechsel ändert nie etwas (kein Abbruch)
                prop_assert_eq!(&t.state, &before);
                prop_assert!(t.effects.is_empty());
            }
            if headless {
                for eff in &t.effects {
                    let dialog = matches!(eff, Effect::AskSavePath(_));
                    prop_assert!(!dialog, "Dialog ohne Oberfläche: {eff:?}");
                }
            }
            if headless {
                let name = state_name(&t.state);
                prop_assert!(!matches!(name, "AnalysisWarning" | "PlaceholderQuestion" | "Positioning"), "Rückfrage ohne Oberfläche: {name}");
            }
            state = t.state;
            if let Some(q) = state.queue() {
                let mut seen = HashSet::new();
                for d in q.current.iter().chain(q.pending.iter()) {
                    prop_assert!(seen.insert(d.path.clone()));
                }
            }
        }
    }

    /// Autostart-Befund F-54 ohne Datenschutzdialog (ADR 0003): Mit Oberfläche öffnet der Reducer bei
    /// beliebigen Ereignisfolgen und Einstellungen (Standard-Signiermittel, automatische Position,
    /// unsichtbares Profil, Platzhalter) eine Sitzung bei ID Austria **nur** auf eine ausdrückliche
    /// Nutzeraktion hin: Wahl in der Auswahl, „Wiederholen“, „Signatur fortsetzen“ oder die automatische
    /// Wahl nach „Signieren“ bzw. „Unsichtbar signieren“.
    #[test]
    fn id_austria_session_only_after_explicit_user_action(
        events in prop::collection::vec(0u8..36, 0..80),
        auto_position in any::<bool>(),
        invisible in any::<bool>(),
        default in 0u8..4,
    ) {
        let default_backend = match default {
            0 => None,
            1 => Some(pida_config::BackendChoice::IdAustria),
            2 => Some(pida_config::BackendChoice::LocalBku),
            _ => Some(pida_config::BackendChoice::Keystore),
        };
        let ctx = Context { auto_position, invisible, placeholder_search: true, default_backend, ..Context::default() };
        let atrust = pida_backend_api::BackendId::AtrustNative;
        let mut state = State::Starting;
        let mut next_id = 2u64;
        for e in events {
            let current = state.queue().and_then(Queue::current_id).map(|d| d.0).unwrap_or(1);
            let event = match e {
                0 => Event::Started { documents: vec![doc(1), doc(2)] },
                1 => { next_id += 1; Event::FilesAdded(vec![doc(next_id)]) }
                2 => analyzed(current, 2, None, false),
                3 => analyzed(current, 1, Some(pida_workflow::PlaceholderFound::Qr(qr())), false),
                4 => Event::PlaceholderAnswered { answer: pida_workflow::PlaceholderAnswer::Yes, field_index: None },
                5 => Event::PlaceholderAnswered { answer: pida_workflow::PlaceholderAnswer::No, field_index: None },
                6 => Event::PositionConfirmed,
                7 => Event::InvisibleChosen,
                8 => Event::BackendsProbed(only_id_austria()),
                9 => Event::BackendsProbed(options(true, true)),
                10 => Event::BackendsProbed(options(false, false)),
                11 => Event::BackendsProbed(options(false, true)),
                12 => Event::BackendChosen(atrust),
                13 => Event::BackendChosen(pida_backend_api::BackendId::Keystore),
                14 => Event::BackendChosen(pida_backend_api::BackendId::LocalBku),
                15 => Event::PhaseCompleted(SigningPhase::OpeningSession),
                16 => Event::PhaseCompleted(SigningPhase::ReadingCertificate),
                17 => Event::PhaseCompleted(SigningPhase::Preparing),
                18 => Event::PhaseCompleted(SigningPhase::Signing),
                19 => Event::PhaseCompleted(SigningPhase::Finishing),
                20 => Event::BackendFailed(BackendError::Timeout),
                21 => Event::BackendFailed(BackendError::Cancelled),
                22 => Event::Retry,
                23 => Event::Cancel,
                24 => Event::Navigate(Step::Open),
                25 => Event::Navigate(Step::Positioning),
                26 => Event::Navigate(Step::Signing),
                27 => Event::SettingsChanged,
                28 => Event::Saved(output_info(current)),
                29 => Event::OutputDecided(OutputDecision::Save { path: "/o.pdf".into(), exists: false }),
                30 => {
                    next_id += 1;
                    let plan = pida_workflow::ResumePlan { position: pida_workflow::PositionChoice::Auto, backend: atrust };
                    Event::ResumeRequested { document: doc(next_id), plan, pending: vec![] }
                }
                31 => Event::PreviewUnavailable { doc: DocId(current) },
                32 => Event::WarningAnswered(true),
                33 => {
                    let Event::Analyzed { doc, class, placeholder, signed, page_count, start, .. } = analyzed(current, 2, None, false) else { unreachable!() };
                    Event::Analyzed { doc, class, placeholder, signed, page_count, start, warnings: vec!["pdf-usage-rights".into()], fields: Vec::new(), new_visible_field_allowed: true }
                }
                34 => Event::Backgrounded,
                _ => Event::Foregrounded,
            };
            let before = state.clone();
            let t = reduce(state, event.clone(), &ctx);
            if t.effects.contains(&Effect::OpenSession(atrust)) {
                let explicit = match (&event, &before) {
                    (Event::BackendChosen(id), State::ChoosingBackend { .. }) => *id == atrust,
                    (Event::Retry, State::Error { .. }) => true,
                    (Event::BackendsProbed(_), State::Resuming { .. }) => true,
                    (Event::BackendsProbed(_), State::ChoosingBackend { sign_requested, .. }) => *sign_requested,
                    _ => false,
                };
                prop_assert!(explicit, "ID Austria ohne Nutzeraktion: {event:?} in {} → {:?}", before.name(), t.effects);
            }
            // „Signieren“ ist die einzige Quelle von `sign_requested`, „Signatur fortsetzen“ die von `Resuming`
            if matches!(t.state, State::ChoosingBackend { sign_requested: true, .. }) {
                let sign = matches!(event, Event::PositionConfirmed | Event::InvisibleChosen)
                    && matches!(before, State::Positioning { .. });
                let kept = matches!(before, State::ChoosingBackend { sign_requested: true, .. });
                prop_assert!(sign || kept, "{event:?} in {}", before.name());
            }
            if matches!(t.state, State::Resuming { .. }) {
                let resumed = matches!(event, Event::ResumeRequested { .. }) || matches!(before, State::Resuming { .. });
                prop_assert!(resumed, "{event:?} in {}", before.name());
            }
            state = t.state;
        }
    }

    /// Invariante A1 (ADR 0004 §3.2, D3, §8 F14): Im Arbeitsbereich-Modus erzeugt der Reducer
    /// `Effect::OpenSession` – für **jedes** Signiermittel – bei beliebigen Ereignisfolgen (inklusive der
    /// neuen Ereignisse) und beliebigen Einstellungen (`auto_position`, `invisible`, Standard-Signiermittel,
    /// Platzhaltersuche, Startargument-Automatik) nur in den Paaren der Tabelle: `SignRequested` in
    /// `Positioning`/`ChoosingBackend`, `BackendChosen` in `ChoosingBackend`, `BackendsProbed` in `Resuming`,
    /// `Retry` im Fehler des Signierens und – nur mit `cli_auto` – `BackendsProbed` in `ChoosingBackend` mit
    /// einem Signiermittel, bei dem das Dokument auf dem Gerät bleibt. Mit Stapelübersicht (ADR 0004 §3.8)
    /// zusätzlich `BatchStarted` in `BatchOverview` und `Retry` einer Sitzungspause im Stapellauf – innerhalb
    /// eines Laufs nie sonst. Zusätzlich: `Resuming` nur nach `ResumeRequested`; ohne `cli_auto` nie eine
    /// automatische Wahl; `Positioning` immer mit Bearbeitungszustand; ein Lauf signiert nur die beim Start
    /// genannten Dokumente.
    #[test]
    fn workspace_session_only_after_explicit_action(
        events in prop::collection::vec(0u8..71, 0..110),
        auto_position in any::<bool>(),
        invisible in any::<bool>(),
        placeholder_search in any::<bool>(),
        placeholder_fields in any::<bool>(),
        cli_auto in any::<bool>(),
        batch_overview in any::<bool>(),
        default in 0u8..4,
    ) {
        use pida_backend_api::BackendId;
        let default_backend = match default {
            0 => None,
            1 => Some(pida_config::BackendChoice::IdAustria),
            2 => Some(pida_config::BackendChoice::LocalBku),
            _ => Some(pida_config::BackendChoice::Keystore),
        };
        let mut ctx = Context {
            auto_position,
            invisible,
            placeholder_search,
            placeholder_fields,
            default_backend,
            workspace: true,
            cli_auto,
            batch_overview,
            ..Context::default()
        };
        let mut state = State::Starting;
        let mut next_id = 2u64;
        let item = |state: &State, n: u64| -> DocId {
            let ids: Vec<DocId> = state.queue().and_then(|q| q.batch.as_ref()).map(|b| b.items.iter().map(|i| i.doc).collect()).unwrap_or_default();
            if ids.is_empty() { DocId(1) } else { ids[(n as usize) % ids.len()] }
        };
        let some_placement = pida_placement::Placement { page_index: 0, x: 40.0, y: 300.0, width: 230.0, height: 49.0 };
        for e in events {
            let current = state.queue().and_then(Queue::current_id).map(|d| d.0).unwrap_or(1);
            let with = |fields: usize, warnings: Vec<String>, placeholder| {
                let Event::Analyzed { doc, class, signed, page_count, start, .. } = analyzed(current, 2, None, false) else { unreachable!() };
                Event::Analyzed {
                    doc, class, placeholder, signed, page_count, start, warnings,
                    fields: (0..fields).map(|i| field(&format!("F{i}"))).collect(),
                    new_visible_field_allowed: fields.is_multiple_of(2),
                }
            };
            let event = match e {
                0 => Event::Started { documents: vec![doc(1), doc(2)] },
                1 => { next_id += 1; Event::FilesAdded(vec![doc(next_id)]) }
                2 => with(0, Vec::new(), None),
                3 => with(0, Vec::new(), Some(pida_workflow::PlaceholderFound::Qr(qr()))),
                4 => with(1, Vec::new(), Some(pida_workflow::PlaceholderFound::Fields(vec![field("F0")]))),
                5 => Event::PlaceholderAnswered { answer: pida_workflow::PlaceholderAnswer::Yes, field_index: None },
                6 => Event::PositionConfirmed,
                7 => Event::InvisibleChosen,
                8 => Event::BackendsProbed(only_id_austria()),
                9 => Event::BackendsProbed(options(true, true)),
                10 => Event::BackendsProbed(options(false, false)),
                11 => Event::BackendsProbed(options(false, true)),
                12 => Event::BackendChosen(BackendId::AtrustNative),
                13 => Event::BackendChosen(BackendId::Keystore),
                14 => Event::PhaseCompleted(SigningPhase::OpeningSession),
                15 => Event::PhaseCompleted(SigningPhase::ReadingCertificate),
                16 => Event::PhaseCompleted(SigningPhase::Preparing),
                17 => Event::PhaseCompleted(SigningPhase::Signing),
                18 => Event::PhaseCompleted(SigningPhase::Finishing),
                19 => Event::BackendFailed(BackendError::Timeout),
                20 => Event::BackendFailed(BackendError::Cancelled),
                21 => Event::BackendFailed(BackendError::UnknownPage { diagnostic: "x".into() }),
                22 => Event::Retry,
                23 => Event::Cancel,
                24 => Event::Navigate(Step::Open),
                25 => Event::Navigate(Step::Positioning),
                26 => Event::Navigate(Step::Signing),
                27 => Event::SettingsChanged,
                28 => Event::Saved(output_info(current)),
                29 => Event::OutputDecided(OutputDecision::Save { path: "/o.pdf".into(), exists: false }),
                30 => {
                    next_id += 1;
                    let backend = if next_id.is_multiple_of(2) { BackendId::AtrustNative } else { BackendId::Keystore };
                    let plan = pida_workflow::ResumePlan { position: pida_workflow::PositionChoice::Auto, backend };
                    Event::ResumeRequested { document: doc(next_id), plan, pending: vec![] }
                }
                31 => Event::PreviewUnavailable { doc: DocId(current) },
                32 => Event::WarningAnswered(true),
                33 => with(1, vec!["pdf-usage-rights".into()], None),
                34 => Event::Backgrounded,
                35 => Event::Foregrounded,
                36 => Event::SignRequested { backend: BackendId::AtrustNative, options: options(true, true) },
                37 => Event::SignRequested { backend: BackendId::Keystore, options: options(false, true) },
                38 => Event::SignRequested { backend: BackendId::LocalBku, options: options(false, false) },
                39 => Event::InvisibleSet(true),
                40 => Event::InvisibleSet(false),
                41 => Event::TargetChosen(pida_workflow::Target::Free),
                42 => Event::TargetChosen(pida_workflow::Target::Field(0)),
                43 => Event::TargetChosen(pida_workflow::Target::Qr),
                44 => Event::AutoPlaced(some_placement),
                45 => Event::BlockResized(some_placement),
                46 => Event::SigningDeclined,
                47 => Event::PlacementChanged(some_placement),
                48 => Event::CmsTooLarge { needed: 20_000, reserve: 16_384 },
                49 => Event::SigningFailed(pida_workflow::WorkflowError::Prepare { code: "x".into(), detail: String::new() }),
                // ADR 0004 §3.8: Stapel
                50 => {
                    let doc = item(&state, next_id);
                    let summary = pida_workflow::DocSummary {
                        page_count: 2, size_bytes: 10, signed: false, class: pida_pades::AcceptanceClass::K1Regular,
                        qr: next_id.is_multiple_of(3), fields: 0,
                        warnings: if next_id.is_multiple_of(5) { vec!["pdf-usage-rights".into()] } else { Vec::new() },
                        placeholder: next_id.is_multiple_of(3).then(qr), new_visible_field_allowed: true,
                        first_field: None, qr_searched: true,
                    };
                    next_id += 1;
                    Event::Preanalyzed { doc, result: Ok(summary) }
                }
                51 => Event::BatchItemOpened(item(&state, next_id)),
                52 => Event::BackToBatch,
                53 => Event::ShowBatch,
                54 => Event::BatchItemRemoved(item(&state, next_id + 1)),
                55 => {
                    let docs = state.queue().and_then(|q| q.batch.as_ref()).map(|b| b.items.iter().map(|i| i.doc).collect()).unwrap_or_default();
                    let backend = if next_id.is_multiple_of(2) { BackendId::AtrustNative } else { BackendId::Keystore };
                    Event::BatchStarted {
                        backend, docs, output: Some(pida_workflow::BatchOutput::Configured),
                        on_existing: if next_id.is_multiple_of(3) { pida_workflow::ExistingPolicy::Skip } else { pida_workflow::ExistingPolicy::KeepBoth },
                        options: options(false, true),
                    }
                }
                56 => Event::BatchSkip,
                57 => Event::BatchAbort,
                58 => Event::BatchOutputDecided { doc: DocId(current), path: "/o.pdf".into(), exists: next_id.is_multiple_of(2) },
                59 => { next_id += 2; Event::FilesAdded(vec![doc(next_id - 1), doc(next_id)]) }
                60 => Event::Preanalyzed { doc: item(&state, next_id), result: Err(pida_workflow::WorkflowError::Rejected { class: pida_pades::AcceptanceClass::K5Rejected, code: "error-PDFPwdProtected".into() }) },
                // Einstellungen › Dokument „sofort wirksam“ (K3): Platzhaltersuche bzw. -art umschalten
                62 => { ctx.placeholder_search = !ctx.placeholder_search; Event::SettingsChanged }
                63 => { ctx.placeholder_fields = !ctx.placeholder_fields; Event::SettingsChanged }
                64 => Event::PlaceholderSearched { doc: DocId(current), qr: next_id.is_multiple_of(2).then(qr) },
                // Einstellung „Automatische Positionierung“ (Nachtrag 8): „Diesmal selbst platzieren“, Umschalten
                // in den Einstellungen („sofort wirksam“), Seitenwechsel
                65 => Event::PlacementUnlocked,
                66 => { ctx.auto_position = !ctx.auto_position; Event::SettingsChanged }
                67 => Event::PageChanged(0),
                // „Für diesen Stapel automatisch positionieren“ (Nachtrag 8)
                68 => Event::BatchAutoPositionSet { enabled: true, replace_manual: false },
                69 => Event::BatchAutoPositionSet { enabled: true, replace_manual: true },
                70 => Event::BatchAutoPositionSet { enabled: false, replace_manual: false },
                _ => Event::BackendFailed(BackendError::Network("x".into())),
            };
            let before = state.clone();
            let t = reduce(state, event.clone(), &ctx);
            for eff in &t.effects {
                let Effect::OpenSession(id) = eff else { continue };
                let local = |options: &[pida_workflow::BackendOption]| {
                    options.iter().any(|o| o.id == *id && o.capabilities.as_ref().is_some_and(|c| !c.document_leaves_device))
                };
                let allowed = match (&event, &before) {
                    (Event::SignRequested { .. }, State::Positioning { .. } | State::ChoosingBackend { .. }) => true,
                    (Event::BackendChosen(_), State::ChoosingBackend { .. }) => true,
                    (Event::BackendsProbed(_), State::Resuming { .. }) => true,
                    (Event::Retry, State::Error { from: Some(f), .. }) => matches!(**f, State::Signing { .. }),
                    (Event::BackendsProbed(options), State::ChoosingBackend { .. }) => cli_auto && local(options),
                    (Event::BatchStarted { .. }, State::BatchOverview { .. }) => true,
                    (Event::Retry, State::BatchSigning { run, .. }) => {
                        matches!(run.pause, Some(pida_workflow::BatchPause::Session { new_session: true, .. }))
                    }
                    _ => false,
                };
                prop_assert!(allowed, "OpenSession({id:?}) ohne Nutzeraktion: {event:?} in {} ({:?})", before.name(), t.effects);
            }
            // gesperrte Platzierung (Nachtrag 8): Ziehen, Tastatur, Seitenwechsel und Zielwahl ändern nichts
            if let State::Positioning { edit: Some(e), .. } = &before
                && e.placement_locked()
                && matches!(event, Event::PlacementChanged(_) | Event::PageChanged(_) | Event::TargetChosen(_))
            {
                prop_assert_eq!(&t.state, &before, "{:?} verschiebt einen gesperrten Block", event);
                prop_assert!(t.effects.is_empty(), "{:?}: {:?}", event, t.effects);
            }
            // gesperrt nur mit der Einstellung, nie für Einträge der Stapelübersicht
            if let State::Positioning { edit: Some(e), .. } = &t.state
                && e.placement_locked()
            {
                prop_assert!(ctx.auto_position && !e.return_to_batch, "{event:?} in {} sperrt", before.name());
            }
            // „Für diesen Stapel automatisch positionieren“ startet nie etwas und gilt nie über einen Lauf hinaus
            if matches!(event, Event::BatchAutoPositionSet { .. }) {
                prop_assert!(!t.effects.iter().any(|e| matches!(e, Effect::OpenSession(_) | Effect::SignDocument)), "{:?}", t.effects);
            }
            if let (State::BatchSigning { .. }, State::BatchOverview { queue }) = (&before, &t.state) {
                prop_assert!(!queue.batch.as_ref().is_some_and(|b| b.auto_position_once), "nach dem Lauf wieder laut Einstellung");
            }
            // Stapel nur mit Stapelübersicht, nie mit der Startargument-Automatik (D-06)
            if matches!(t.state, State::BatchOverview { .. } | State::BatchSigning { .. }) {
                prop_assert!(batch_overview && !cli_auto, "{event:?} in {} → {}", before.name(), t.state.name());
            }
            // ein Lauf signiert nur die beim Start genannten Dokumente (später hinzugefügte nie)
            if let (State::BatchSigning { run: before_run, .. }, State::BatchSigning { run, .. }) = (&before, &t.state) {
                prop_assert_eq!(&before_run.order, &run.order);
            }
            if let State::BatchSigning { queue, run } = &t.state
                && let Some(b) = &queue.batch
            {
                for i in &b.items {
                    if matches!(i.status, pida_workflow::BatchItemStatus::Active | pida_workflow::BatchItemStatus::Queued) {
                        prop_assert!(run.order.contains(&i.doc), "{:?} nicht im Lauf", i.doc);
                    }
                }
            }
            if matches!(t.state, State::Resuming { .. }) {
                let resumed = matches!(event, Event::ResumeRequested { .. }) || matches!(before, State::Resuming { .. });
                prop_assert!(resumed, "{event:?} in {}", before.name());
            }
            if !cli_auto {
                prop_assert!(
                    !matches!(t.state, State::ChoosingBackend { auto_choice: true, .. }),
                    "automatische Wahl ohne Startargument-Automatik: {event:?} in {}", before.name()
                );
            }
            prop_assert!(
                !matches!(t.state, State::Positioning { edit: None, .. } | State::PlaceholderQuestion { .. }),
                "klassischer Schritt im Arbeitsbereich: {event:?} in {} → {}", before.name(), t.state.name()
            );
            state = t.state;
        }
    }
}

/// Antwort „wie core-api, Erfolg“ auf den laufenden Schritt eines Stapellaufs bzw. – außerhalb des Laufs – die
/// nächste Nutzeraktion zum Lauf (Dateien, Voranalyse, „Alle signieren“). `n` variiert Signiermittel, Ausgabe
/// und Regel für vorhandene Dateien.
fn batch_advance(state: &State, last: &[Effect], n: u64) -> Event {
    use pida_workflow::{BatchOutput, BatchPhase, ExistingPolicy};
    match state {
        State::BatchSigning { run, .. } => {
            let doc = run.current().unwrap_or(DocId(1));
            match run.phase {
                BatchPhase::OpeningSession => Event::PhaseCompleted(SigningPhase::OpeningSession),
                BatchPhase::ReadingCertificate => Event::PhaseCompleted(SigningPhase::ReadingCertificate),
                BatchPhase::CheckingTarget => {
                    Event::BatchOutputDecided { doc, path: "/o/x_signed.pdf".into(), exists: n.is_multiple_of(3) }
                }
                BatchPhase::Loading => {
                    let placeholder = n.is_multiple_of(4).then(|| pida_workflow::PlaceholderFound::Qr(qr()));
                    analyzed(doc.0, 2, placeholder, false)
                }
                BatchPhase::Preparing => Event::PhaseCompleted(SigningPhase::Preparing),
                BatchPhase::Confirming => Event::PhaseCompleted(SigningPhase::Signing),
                BatchPhase::Finishing => Event::PhaseCompleted(SigningPhase::Finishing),
                BatchPhase::Writing => match last.last() {
                    Some(Effect::Write(_) | Effect::Overwrite(_)) => Event::Saved(output_info(doc.0)),
                    _ => Event::BatchOutputDecided { doc, path: "/o/x_signed.pdf".into(), exists: n.is_multiple_of(2) },
                },
            }
        }
        State::BatchOverview { queue } => {
            let batch = queue.batch.as_ref();
            if let Some(i) =
                batch.and_then(|b| b.items.iter().find(|i| i.status == pida_workflow::BatchItemStatus::Analyzing))
            {
                let summary = pida_workflow::DocSummary {
                    page_count: 2,
                    size_bytes: 10,
                    signed: false,
                    class: pida_pades::AcceptanceClass::K1Regular,
                    qr: n.is_multiple_of(3),
                    fields: 0,
                    warnings: Vec::new(),
                    placeholder: n.is_multiple_of(3).then(qr),
                    new_visible_field_allowed: true,
                    first_field: None,
                    qr_searched: true,
                };
                return Event::Preanalyzed { doc: i.doc, result: Ok(summary) };
            }
            let docs = batch.map(|b| b.items.iter().map(|i| i.doc).collect()).unwrap_or_default();
            let backend = if n.is_multiple_of(2) {
                pida_backend_api::BackendId::AtrustNative
            } else {
                pida_backend_api::BackendId::Keystore
            };
            let output = match n % 3 {
                0 => Some(BatchOutput::Configured),
                1 => Some(BatchOutput::Folder("/o".into())),
                _ => None,
            };
            let on_existing = match n % 4 {
                0 => ExistingPolicy::Skip,
                1 => ExistingPolicy::Overwrite,
                _ => ExistingPolicy::KeepBoth,
            };
            Event::BatchStarted { backend, docs, output, on_existing, options: options(false, true) }
        }
        _ => Event::FilesAdded(vec![doc(10 + n), doc(11 + n), doc(12 + n)]),
    }
}

proptest! {
    /// Stapellauf (ADR 0004 §3.8) bei beliebigen Folgen aus Erfolg, Fehlern aller Arten, Überspringen,
    /// Abbrechen, Wiederholen, neuen Dateien und Einstellungen – mit hohem Anteil „Erfolg des laufenden
    /// Schritts“, damit auch die tiefen Phasen erreicht werden:
    ///
    /// - A1: `OpenSession` nur für „Alle signieren“ in der Übersicht und „Erneut versuchen“ einer
    ///   Sitzungspause mit neuer Sitzung;
    /// - genau die beim Start genannten Dokumente, höchstens eines in Arbeit;
    /// - in einer Pause geht nichts von selbst weiter (ohne Entscheidung keine Effekte außer Hinweisen und
    ///   der Voranalyse neuer Dateien);
    /// - nichts wird automatisch bestätigt: je Dokument höchstens so viele `SignDocument` wie „Erneut
    ///   versuchen“ + 1;
    /// - nach dem Lauf ist kein Eintrag mehr „wartet“/„wird signiert“.
    #[test]
    fn batch_run_invariants(
        events in prop::collection::vec(0u8..36, 0..160),
        invisible in any::<bool>(),
        placeholder_search in any::<bool>(),
    ) {
        use pida_workflow::{BatchItemStatus, BatchPause};
        let ctx = Context { workspace: true, batch_overview: true, invisible, placeholder_search, ..Context::default() };
        let mut state = State::Idle { queue: Queue::default() };
        let mut last: Vec<Effect> = Vec::new();
        let mut signs: std::collections::HashMap<DocId, (usize, usize)> = std::collections::HashMap::new();
        for (n, e) in events.into_iter().enumerate() {
            let n = n as u64;
            let current = state.queue().and_then(Queue::current_id).unwrap_or(DocId(1));
            let event = match e {
                0..=15 => batch_advance(&state, &last, n),
                16 => Event::BatchSkip,
                17 => Event::BatchAbort,
                18 => Event::Retry,
                19 => Event::Cancel,
                20 => Event::BackendFailed(BackendError::Network("x".into())),
                21 => Event::BackendFailed(BackendError::UnknownPage { diagnostic: "x".into() }),
                22 => Event::BackendFailed(BackendError::Cancelled),
                23 => Event::SigningDeclined,
                24 => Event::BackendFailed(BackendError::DocumentTooLarge),
                25 => Event::BackendFailed(BackendError::SecurityLayer { code: 6002, info: "x".into() }),
                26 => Event::CmsTooLarge { needed: 20_000, reserve: 16_384 },
                27 => Event::SaveFailed("voll".into()),
                28 => Event::SigningFailed(pida_workflow::WorkflowError::Prepare { code: "ng-certcache-mismatch".into(), detail: String::new() }),
                29 => Event::AnalysisFailed { doc: current, error: pida_workflow::WorkflowError::Save("x".into()) },
                30 => Event::FilesAdded(vec![doc(100 + n)]),
                31 => Event::SettingsChanged,
                32 => Event::Backgrounded,
                33 => Event::BatchItemOpened(current),
                34 => Event::BackToBatch,
                _ => Event::Navigate(Step::Open),
            };
            let before = state.clone();
            let t = reduce(state, event.clone(), &ctx);
            // A1
            for eff in &t.effects {
                if let Effect::OpenSession(_) = eff {
                    let allowed = match (&event, &before) {
                        (Event::BatchStarted { .. }, State::BatchOverview { .. }) => true,
                        (Event::Retry, State::BatchSigning { run, .. }) => {
                            matches!(run.pause, Some(BatchPause::Session { new_session: true, .. }))
                        }
                        _ => false,
                    };
                    prop_assert!(allowed, "OpenSession ohne Nutzeraktion: {event:?} in {}", before.name());
                }
            }
            if let State::BatchSigning { run: before_run, .. } = &before {
                // Pause: ohne Entscheidung geht nichts weiter
                let decision = matches!(event, Event::Retry | Event::BatchSkip | Event::BatchAbort | Event::Cancel | Event::Quit);
                if before_run.pause.is_some() && !decision {
                    let quiet = t.effects.iter().all(|e| matches!(e, Effect::Preanalyze(_) | Effect::NotifyWith(..) | Effect::Notify(_)));
                    prop_assert!(quiet, "in der Pause von selbst weiter: {event:?} → {:?}", t.effects);
                }
                // Bestätigungen zählen
                if let Some(doc) = before_run.current() {
                    let entry = signs.entry(doc).or_default();
                    if matches!(event, Event::Retry) && before_run.pause.is_some() {
                        entry.1 += 1;
                    }
                    entry.0 += t.effects.iter().filter(|e| matches!(e, Effect::SignDocument)).count();
                    prop_assert!(entry.0 <= entry.1 + 1, "{doc:?}: {} Bestätigungen bei {} Wiederholungen", entry.0, entry.1);
                }
            }
            if let State::BatchSigning { queue, run } = &t.state {
                if let State::BatchSigning { run: before_run, .. } = &before {
                    prop_assert_eq!(&before_run.order, &run.order);
                }
                let b = queue.batch.as_ref().expect("Stapel");
                let active = b.items.iter().filter(|i| i.status == BatchItemStatus::Active).count();
                prop_assert!(active <= 1, "{active} Dokumente gleichzeitig in Arbeit");
                for i in &b.items {
                    if matches!(i.status, BatchItemStatus::Active | BatchItemStatus::Queued) {
                        prop_assert!(run.order.contains(&i.doc), "{:?} nicht im Lauf", i.doc);
                    }
                }
            } else {
                if matches!(before, State::BatchSigning { .. }) {
                    signs.clear();
                }
                if let Some(b) = t.state.queue().and_then(|q| q.batch.as_ref()) {
                    for i in &b.items {
                        prop_assert!(!matches!(i.status, BatchItemStatus::Active | BatchItemStatus::Queued), "{:?} nach dem Lauf {:?}", i.doc, i.status);
                    }
                }
            }
            last = t.effects;
            state = t.state;
        }
    }
}
