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
                    Event::Analyzed { doc, class, placeholder, signed, page_count, start, warnings: vec!["pdf-usage-rights".into()] }
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
                    Event::Analyzed { doc, class, placeholder, signed, page_count, start, warnings: vec!["pdf-usage-rights".into()] }
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
}
