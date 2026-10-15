//! Der reine Reducer (docs/analyse/01 §3.4 ohne implizite Re-Entry-Semantik).
//!
//! Jeder Übergang der PDF-Over-Tabelle ist hier ein expliziter Fall; die Abbildung steht in der
//! Crate-Doku ([`crate`]). Ereignisse, die im aktuellen Zustand keine Bedeutung haben (z. B. ein
//! verspätetes Ergebnis eines abgebrochenen Effekts), lassen den Zustand unverändert.

use pdfover_backend_api::BackendId;
use pdfover_pades::AcceptanceClass;
use pdfover_placement::Placement;

use crate::output::{ExitCode, OutputDecision, save_dialog};
use crate::queue::{DocResult, Queue};
use crate::state::{
    BackendOption, Context, DocFacts, Effect, Event, MSG_BACKEND_CHOICE_REQUIRED, MSG_DEFAULT_BACKEND_UNAVAILABLE,
    MSG_NO_BACKEND, MSG_POSITIONING_NOT_POSSIBLE, PlaceholderAnswer, PlaceholderFound, PositionChoice, RetryTarget,
    SignFlow, SigningPhase, State, Step, Transition, WorkflowError, backend_for_choice,
};

/// Meldungs-ID des Platzierungshinweises „neue Seite nicht erlaubt“ (`PlacementNotice::code`).
const MSG_NEW_PAGE_NOT_ALLOWED: &str = "placement.notice.new_page_not_allowed";

/// Mindestreserve nach „CMS zu groß“ (Plan §6.2: `max(16 KiB, 2 × gemessenes CMS)`).
const MIN_GROWN_RESERVE: usize = 16 * 1024;

/// Neue `/Contents`-Reserve nach [`WorkflowError::CmsTooLarge`]: `max(16 KiB, 2 × benötigt, alt + 1 KiB)`,
/// aufgerundet auf volle KiB.
pub fn grown_reserve(needed: usize, reserve: usize) -> usize {
    let target = needed.saturating_mul(2).max(reserve.saturating_add(1024)).max(MIN_GROWN_RESERVE);
    target.div_ceil(1024).saturating_mul(1024)
}

/// Reiner Reducer.
pub fn reduce(state: State, event: Event, ctx: &Context) -> Transition {
    // Ereignisse, die in jedem Zustand gleich behandelt werden
    match event {
        Event::Quit => return quit(state),
        Event::FilesAdded(docs) => return files_added(state, docs, ctx),
        Event::SettingsChanged => return settings_changed(state),
        Event::Backgrounded | Event::Foregrounded => return Transition::stay(state),
        // ohne Oberfläche gibt es keine Hauptleiste (sonst führte „Positionierung“ in einen Dialog)
        Event::Navigate(_) if ctx.headless => return Transition::stay(state),
        _ => {}
    }
    match state {
        State::Starting => starting(event, ctx),
        State::Idle { queue } => idle(queue, event),
        State::Analyzing { queue } => analyzing(queue, event, ctx),
        s @ State::AnalysisWarning { .. } => analysis_warning(s, event, ctx),
        s @ State::PlaceholderQuestion { .. } => placeholder(s, event, ctx),
        s @ State::Positioning { .. } => positioning(s, event, ctx),
        s @ State::ChoosingBackend { .. } => choosing_backend(s, event, ctx),
        s @ State::PrivacyNotice { .. } => privacy(s, event, ctx),
        s @ State::Signing { .. } => signing(s, event, ctx),
        s @ State::Output { .. } => output(s, event, ctx),
        s @ State::Error { .. } => error(s, event, ctx),
        s @ State::Finished { .. } => Transition::stay(s),
    }
}

// ------------------------------------------------------------------------------------------------
// Gemeinsame Bausteine
// ------------------------------------------------------------------------------------------------

/// Leere Warteschlange: Dokumentauswahl bzw. (ohne Oberfläche) Programmende mit dem Exit-Code der
/// Ergebnisse.
fn idle_or_finish(queue: Queue, ctx: &Context, effects: Vec<Effect>) -> Transition {
    if ctx.headless {
        return finish(queue, effects);
    }
    Transition::with(State::Idle { queue }, effects)
}

/// Programmende mit dem Exit-Code der Ergebnisse.
fn finish(queue: Queue, mut effects: Vec<Effect>) -> Transition {
    let exit = queue.exit_code();
    effects.push(Effect::Exit(exit));
    Transition::with(State::Finished { exit, results: queue.done }, effects)
}

/// Programmende mit festem Exit-Code.
fn finish_with(exit: ExitCode, results: Vec<crate::queue::DocOutcome>, mut effects: Vec<Effect>) -> Transition {
    effects.push(Effect::Exit(exit));
    Transition::with(State::Finished { exit, results }, effects)
}

/// Aktuelles Dokument analysieren bzw. ohne Dokument auf Auswahl/Ende gehen.
fn start_current(queue: Queue, ctx: &Context, mut effects: Vec<Effect>) -> Transition {
    match queue.current_id() {
        Some(id) => {
            effects.push(Effect::Analyze(id));
            Transition::with(State::Analyzing { queue }, effects)
        }
        None => idle_or_finish(queue, ctx, effects),
    }
}

/// Aktuelles Dokument mit `result` abschließen und mit dem nächsten weitermachen (Batch, F-88).
fn next_doc(mut queue: Queue, result: DocResult, ctx: &Context, mut effects: Vec<Effect>) -> Transition {
    if let Some(id) = queue.current_id() {
        effects.push(Effect::Discard(id));
    }
    queue.finish_current(result);
    start_current(queue, ctx, effects)
}

/// Hauptleiste „Öffnen“ außerhalb des Ergebnis-Screens: Dokument und Warteschlange verwerfen (F-16).
fn discard_all(mut queue: Queue, mut effects: Vec<Effect>) -> Transition {
    effects.extend(queue.current.iter().chain(queue.pending.iter()).map(|d| Effect::Discard(d.id)));
    queue.clear();
    Transition::with(State::Idle { queue }, effects)
}

/// Fehler: mit Oberfläche als Fehlerzustand mit „Wiederholen“/„Abbrechen“, ohne Oberfläche wird das
/// Dokument als fehlgeschlagen verbucht und das nächste begonnen (kein Wiederholen ohne Nutzerin).
fn fail(
    from: State,
    error: WorkflowError,
    retry: Option<RetryTarget>,
    ctx: &Context,
    effects: Vec<Effect>,
) -> Transition {
    let Some(queue) = from.queue().cloned() else {
        // Fehler beim Start: ohne Oberfläche Ende, sonst Rückfrage (Werksreset)
        if ctx.headless {
            return finish_with(error.exit_code(), Vec::new(), effects);
        }
        return Transition::with(
            State::Error { queue: Queue::default(), error, retry, from: Some(Box::new(from)) },
            effects,
        );
    };
    if ctx.headless {
        return next_doc(queue, DocResult::Failed { error }, ctx, effects);
    }
    Transition::with(State::Error { queue, error, retry, from: Some(Box::new(from)) }, effects)
}

/// Positionierung bzw. – wenn das Profil unsichtbar ist, automatisch positioniert wird oder keine
/// Vorschau möglich ist – direkt die Backend-Auswahl.
fn after_position_question(queue: Queue, doc: DocFacts, ctx: &Context, effects: Vec<Effect>) -> Transition {
    if ctx.invisible {
        return to_backend_choice(queue, doc, PositionChoice::Invisible, true, effects);
    }
    if ctx.auto_position || ctx.headless {
        return to_backend_choice(queue, doc, PositionChoice::Auto, true, effects);
    }
    to_positioning(queue, doc, None, ctx, effects)
}

/// Positionierung (Startseite = letzte Seite, Startposition 50 %/25 %, F-32/F-33; vorherige
/// manuelle Position bleibt beim Rücksprung erhalten, F-43).
fn to_positioning(
    queue: Queue,
    doc: DocFacts,
    previous: Option<Placement>,
    ctx: &Context,
    mut effects: Vec<Effect>,
) -> Transition {
    let placement = previous.or(doc.start);
    let Some(placement) = placement else {
        // keine Vorschau möglich (F-44): Hinweis und automatische Positionierung
        effects.push(Effect::Notify(MSG_POSITIONING_NOT_POSSIBLE.into()));
        return to_backend_choice(queue, doc, PositionChoice::Auto, true, effects);
    };
    let _ = ctx;
    let new_page = doc.page_count > 0 && placement.page_index >= doc.page_count;
    let page = placement.page_index.min(doc.page_count);
    if page < doc.page_count {
        effects.push(Effect::RenderPage { doc: doc.id, page });
    }
    effects.push(Effect::BuildBlockPreview(doc.id));
    Transition::with(State::Positioning { queue, page, placement: Some(placement), new_page, doc }, effects)
}

/// Backend-Auswahl; die Optionen kommen über [`Effect::ProbeBackends`].
fn to_backend_choice(
    queue: Queue,
    doc: DocFacts,
    position: PositionChoice,
    auto_choice: bool,
    mut effects: Vec<Effect>,
) -> Transition {
    effects.push(Effect::ProbeBackends);
    Transition::with(State::ChoosingBackend { queue, position, options: Vec::new(), doc, auto_choice }, effects)
}

/// Ergebnis der automatischen Backend-Wahl.
enum AutoChoice {
    Choose(BackendId),
    Ask,
    Fail(WorkflowError),
}

/// F-53 (Standard-BKU überspringt die Auswahl), F-54 (ohne lokale BKU und Keystore automatisch ID
/// Austria); ohne Oberfläche zusätzlich: genau ein verfügbares Backend wird genommen, sonst Fehler.
fn auto_choice(options: &[BackendOption], ctx: &Context) -> AutoChoice {
    let available = |id: BackendId| options.iter().any(|o| o.id == id && o.available());
    if let Some(choice) = ctx.default_backend {
        let id = backend_for_choice(choice);
        if available(id) {
            return AutoChoice::Choose(id);
        }
        if ctx.headless {
            return AutoChoice::Fail(WorkflowError::Backend {
                code: MSG_DEFAULT_BACKEND_UNAVAILABLE.into(),
                detail: id.as_str().into(),
            });
        }
        return AutoChoice::Ask;
    }
    if !available(BackendId::LocalBku) && !available(BackendId::Keystore) && available(BackendId::AtrustNative) {
        return AutoChoice::Choose(BackendId::AtrustNative);
    }
    if !ctx.headless {
        return AutoChoice::Ask;
    }
    let avail: Vec<&BackendOption> = options.iter().filter(|o| o.available()).collect();
    match avail.as_slice() {
        [] => AutoChoice::Fail(WorkflowError::Backend { code: MSG_NO_BACKEND.into(), detail: String::new() }),
        [one] => AutoChoice::Choose(one.id),
        _ => AutoChoice::Fail(WorkflowError::Backend {
            code: MSG_BACKEND_CHOICE_REQUIRED.into(),
            detail: avail.iter().map(|o| o.id.as_str()).collect::<Vec<_>>().join(","),
        }),
    }
}

/// Backend gewählt: Datenschutzhinweis (einmalig je Textversion) oder Signieren.
fn choose_backend(
    queue: Queue,
    doc: DocFacts,
    position: PositionChoice,
    options: Vec<BackendOption>,
    id: BackendId,
    ctx: &Context,
    effects: Vec<Effect>,
) -> Transition {
    let caps = options.iter().find(|o| o.id == id).and_then(|o| o.capabilities.clone());
    let Some(caps) = caps else {
        // nicht verfügbar – Auswahl bleibt
        return Transition::with(State::ChoosingBackend { queue, position, options, doc, auto_choice: false }, effects);
    };
    if caps.document_leaves_device && !ctx.privacy_acknowledged.contains(&id) {
        return Transition::with(State::PrivacyNotice { queue, position, backend: id, doc, options }, effects);
    }
    let flow = SignFlow::new(&caps, &position);
    start_signing(queue, doc, position, id, flow, effects)
}

fn start_signing(
    queue: Queue,
    doc: DocFacts,
    position: PositionChoice,
    backend: BackendId,
    flow: SignFlow,
    mut effects: Vec<Effect>,
) -> Transition {
    effects.push(Effect::OpenSession(backend));
    Transition::with(
        State::Signing { queue, position, backend, phase: SigningPhase::OpeningSession, doc, flow },
        effects,
    )
}

// ------------------------------------------------------------------------------------------------
// Globale Ereignisse
// ------------------------------------------------------------------------------------------------

fn quit(state: State) -> Transition {
    match state {
        s @ State::Finished { .. } => Transition::stay(s),
        State::Signing { mut queue, .. } => {
            queue.clear();
            finish_with(ExitCode::Cancelled, queue.done, vec![Effect::CancelSession, Effect::DeletePersisted])
        }
        State::Output { mut queue, saved: Some(output), .. } => {
            // das aktuelle Dokument ist gespeichert; nur noch wartende gelten als abgebrochen
            queue.finish_current(DocResult::Signed { output });
            if queue.is_empty() {
                return finish(queue, Vec::new());
            }
            queue.clear();
            finish_with(ExitCode::Cancelled, queue.done, Vec::new())
        }
        State::Idle { queue } if queue.is_empty() => finish(queue, Vec::new()),
        State::Starting => finish_with(ExitCode::Success, Vec::new(), Vec::new()),
        mut other => {
            let results = match other.queue_mut() {
                Some(q) => {
                    q.clear();
                    std::mem::take(&mut q.done)
                }
                None => Vec::new(),
            };
            finish_with(ExitCode::Cancelled, results, Vec::new())
        }
    }
}

/// Dateien hinzugefügt (Dialog, Drop, „Öffnen mit“): im Auswahl-Screen sofort öffnen, im
/// Ergebnis-Screen anhängen und – wenn zuletzt gespeichert – sofort weiter (F-89), sonst nur an die
/// Warteschlange anhängen (Single-Instance).
fn files_added(state: State, docs: Vec<crate::queue::InputDocument>, ctx: &Context) -> Transition {
    match state {
        State::Idle { mut queue } => {
            let mut effects = Vec::new();
            if let Some(dir) = docs.first().and_then(|d| d.path.parent()).filter(|p| !p.as_os_str().is_empty()) {
                effects.push(Effect::RememberDirectory(dir.to_path_buf()));
            }
            queue.add(docs);
            match queue.current_id() {
                Some(id) => {
                    effects.push(Effect::Analyze(id));
                    Transition::with(State::Analyzing { queue }, effects)
                }
                None => Transition::with(State::Idle { queue }, effects),
            }
        }
        State::Output { mut queue, saved: Some(output), decision, doc, position, backend } => {
            queue.add(docs);
            if queue.pending.is_empty() {
                return Transition::stay(State::Output {
                    queue,
                    saved: Some(output),
                    decision,
                    doc,
                    position,
                    backend,
                });
            }
            // F-89: zuletzt erfolgreich gespeichert → sofort weiter
            next_doc(queue, DocResult::Signed { output }, ctx, Vec::new())
        }
        s @ (State::Starting | State::Finished { .. }) => Transition::stay(s),
        mut s => {
            if let Some(q) = s.queue_mut() {
                q.add(docs);
            }
            Transition::stay(s)
        }
    }
}

/// Einstellungen geändert: Blockvorschau neu (Positionierung), Backends neu prüfen (Auswahl).
fn settings_changed(state: State) -> Transition {
    let effects = match &state {
        State::Positioning { doc, .. } => vec![Effect::BuildBlockPreview(doc.id)],
        State::ChoosingBackend { .. } | State::PrivacyNotice { .. } => vec![Effect::ProbeBackends],
        _ => Vec::new(),
    };
    Transition::with(state, effects)
}

// ------------------------------------------------------------------------------------------------
// Zustände
// ------------------------------------------------------------------------------------------------

fn starting(event: Event, ctx: &Context) -> Transition {
    match event {
        Event::Started { documents } => {
            let mut queue = Queue::default();
            queue.add(documents);
            start_current(queue, ctx, Vec::new())
        }
        Event::SettingsUnreadable(detail) => {
            fail(State::Starting, WorkflowError::Config(detail), Some(RetryTarget::ResetSettings), ctx, Vec::new())
        }
        _ => Transition::stay(State::Starting),
    }
}

fn idle(queue: Queue, event: Event) -> Transition {
    // „Öffnen“ im Auswahl-Screen öffnet den Dateidialog der Shell (kein Zustandswechsel).
    let _ = event;
    Transition::stay(State::Idle { queue })
}

fn analyzing(queue: Queue, event: Event, ctx: &Context) -> Transition {
    let current = queue.current_id();
    match event {
        Event::Analyzed { doc, class, placeholder, signed, page_count, start, warnings } if Some(doc) == current => {
            if matches!(class, AcceptanceClass::K5Rejected | AcceptanceClass::K2Repairable) {
                let error = WorkflowError::Rejected { class, code: "ng-document-rejected".into() };
                return fail(State::Analyzing { queue }, error, None, ctx, Vec::new());
            }
            let facts = DocFacts { id: doc, page_count, signed, class, start };
            // Warnung mit Abbruchmöglichkeit (Plan §6.1); ohne Oberfläche nur Hinweis (core-api), weiter
            if !warnings.is_empty() && !ctx.headless {
                return Transition::stay(State::AnalysisWarning { queue, warnings, doc: facts, placeholder });
            }
            after_analysis(queue, facts, placeholder, ctx)
        }
        Event::AnalysisFailed { doc, error } if Some(doc) == current => {
            let retry = match &error {
                WorkflowError::Rejected {
                    class: AcceptanceClass::K5Rejected | AcceptanceClass::K2Repairable, ..
                } => None,
                _ => Some(RetryTarget::Load),
            };
            fail(State::Analyzing { queue }, error, retry, ctx, Vec::new())
        }
        Event::Navigate(Step::Open) => discard_all(queue, Vec::new()),
        Event::Cancel => next_doc(queue, DocResult::Cancelled, ctx, Vec::new()),
        _ => Transition::stay(State::Analyzing { queue }),
    }
}

/// Nach der Analyse: Platzhalterfrage (F-20/F-21), sonst Positionierung bzw. Auswahl.
fn after_analysis(queue: Queue, doc: DocFacts, placeholder: Option<PlaceholderFound>, ctx: &Context) -> Transition {
    let found = placeholder.filter(|p| ctx.placeholder_search && !p.is_empty() && !ctx.invisible);
    match found {
        Some(found) if ctx.headless => {
            // ohne Oberfläche: QR-Platzhalter bzw. genau ein Feld verwenden, sonst automatisch
            match found {
                PlaceholderFound::Qr(qr) => {
                    to_backend_choice(queue, doc, PositionChoice::Placeholder(qr), true, Vec::new())
                }
                PlaceholderFound::Fields(mut fields) if fields.len() == 1 => {
                    let field = fields.remove(0);
                    to_backend_choice(queue, doc, PositionChoice::Field(field), true, Vec::new())
                }
                PlaceholderFound::Fields(_) => after_position_question(queue, doc, ctx, Vec::new()),
            }
        }
        Some(found) => Transition::stay(State::PlaceholderQuestion { queue, found, doc }),
        None => after_position_question(queue, doc, ctx, Vec::new()),
    }
}

fn analysis_warning(state: State, event: Event, ctx: &Context) -> Transition {
    let State::AnalysisWarning { queue, warnings, doc, placeholder } = state else { unreachable!("AnalysisWarning") };
    match event {
        Event::WarningAnswered(true) => after_analysis(queue, doc, placeholder, ctx),
        Event::WarningAnswered(false) | Event::Cancel => next_doc(queue, DocResult::Cancelled, ctx, Vec::new()),
        Event::Navigate(Step::Open) => discard_all(queue, Vec::new()),
        _ => Transition::stay(State::AnalysisWarning { queue, warnings, doc, placeholder }),
    }
}

fn placeholder(state: State, event: Event, ctx: &Context) -> Transition {
    let State::PlaceholderQuestion { queue, found, doc } = state else { unreachable!("PlaceholderQuestion") };
    match event {
        Event::PlaceholderAnswered { answer: PlaceholderAnswer::Yes, field_index } => match found {
            PlaceholderFound::Qr(qr) => {
                to_backend_choice(queue, doc, PositionChoice::Placeholder(qr), true, Vec::new())
            }
            PlaceholderFound::Fields(fields) => {
                let index = match (fields.len(), field_index) {
                    (1, None) => Some(0),
                    (n, Some(i)) if i < n => Some(i),
                    _ => None,
                };
                match index {
                    // Feldwahl gilt nur für diese Signatur (B-05), keine globalen Flags (B-04)
                    Some(i) => {
                        let field = fields[i].clone();
                        to_backend_choice(queue, doc, PositionChoice::Field(field), true, Vec::new())
                    }
                    // Feldauswahl abgebrochen bzw. ungültig → erneut fragen (OpenState.java:165-171)
                    None => Transition::stay(State::PlaceholderQuestion {
                        queue,
                        found: PlaceholderFound::Fields(fields),
                        doc,
                    }),
                }
            }
        },
        Event::PlaceholderAnswered { answer: PlaceholderAnswer::No, .. } => {
            after_position_question(queue, doc, ctx, Vec::new())
        }
        Event::PlaceholderAnswered { answer: PlaceholderAnswer::Cancel, .. } | Event::Cancel => {
            // Dokument verworfen; B-06 behoben: es geht mit dem nächsten Dokument bzw. der Auswahl weiter
            next_doc(queue, DocResult::Cancelled, ctx, Vec::new())
        }
        Event::Navigate(Step::Open) => discard_all(queue, Vec::new()),
        _ => Transition::stay(State::PlaceholderQuestion { queue, found, doc }),
    }
}

fn positioning(state: State, event: Event, ctx: &Context) -> Transition {
    let State::Positioning { queue, page, placement, new_page, doc } = state else { unreachable!("Positioning") };
    let n = doc.page_count;
    let stay = |queue, page, placement, new_page, doc, effects| {
        Transition::with(State::Positioning { queue, page, placement, new_page, doc }, effects)
    };
    match event {
        Event::PlacementChanged(p) => {
            if p.page_index > n || !(p.x.is_finite() && p.y.is_finite()) {
                return stay(queue, page, placement, new_page, doc, Vec::new());
            }
            if p.page_index == n && !doc.new_page_allowed() {
                // wie PDF-Over/PDF-AS (docs/analyse/04 §4.5): auf signierten Dokumenten keine neue Seite,
                // sondern dieselben Koordinaten auf der letzten Seite – mit Hinweis statt stillschweigend
                let last = n.saturating_sub(1);
                let moved = Placement { page_index: last, ..p };
                let mut effects = vec![Effect::Notify(MSG_NEW_PAGE_NOT_ALLOWED.into())];
                if page != last {
                    effects.push(Effect::RenderPage { doc: doc.id, page: last });
                    effects.push(Effect::BuildBlockPreview(doc.id));
                }
                return stay(queue, last, Some(moved), false, doc, effects);
            }
            let new_page = new_page || p.page_index == n;
            stay(queue, p.page_index, Some(p), new_page, doc, Vec::new())
        }
        Event::PageChanged(target) => {
            let max = if new_page { n } else { n.saturating_sub(1) };
            if target > max || target == page {
                return stay(queue, page, placement, new_page, doc, Vec::new());
            }
            let placement = placement.map(|p| Placement { page_index: target, ..p });
            let mut effects =
                if target < n { vec![Effect::RenderPage { doc: doc.id, page: target }] } else { Vec::new() };
            // Die Blockbreite hängt bei Profilen ohne `w` von der Seitenbreite ab (KR-F01: Vorschau =
            // Ergebnis) – Vorschau für die neue Seite neu bauen.
            effects.push(Effect::BuildBlockPreview(doc.id));
            stay(queue, target, placement, new_page, doc, effects)
        }
        Event::NewPageToggled => {
            if new_page {
                let last = n.saturating_sub(1);
                let placement = placement.map(|p| Placement { page_index: p.page_index.min(last), ..p });
                let page = page.min(last);
                let effects = vec![Effect::RenderPage { doc: doc.id, page }, Effect::BuildBlockPreview(doc.id)];
                stay(queue, page, placement, false, doc, effects)
            } else if !doc.new_page_allowed() {
                // wie PDF-Over: „Neue Seite“ auf einem signierten Dokument landet auf der letzten Seite
                // (docs/analyse/04 §4.5); NG zeigt dazu einen Hinweis
                let last = n.saturating_sub(1);
                let placement = placement.map(|p| Placement { page_index: last, ..p });
                let mut effects = vec![Effect::Notify(MSG_NEW_PAGE_NOT_ALLOWED.into())];
                if page != last {
                    effects.push(Effect::RenderPage { doc: doc.id, page: last });
                    effects.push(Effect::BuildBlockPreview(doc.id));
                }
                stay(queue, last, placement, false, doc, effects)
            } else {
                let placement = placement.map(|p| Placement { page_index: n, ..p });
                let effects = vec![Effect::BuildBlockPreview(doc.id)];
                stay(queue, n, placement, true, doc, effects)
            }
        }
        Event::PositionConfirmed => match placement {
            Some(mut p) => {
                // „Neue Seite“ nur, wenn die Position auf der virtuellen Seite liegt
                if !new_page && p.page_index >= n {
                    p.page_index = n.saturating_sub(1);
                }
                to_backend_choice(queue, doc, PositionChoice::Manual(p), true, Vec::new())
            }
            None => stay(queue, page, placement, new_page, doc, Vec::new()),
        },
        Event::InvisibleChosen => to_backend_choice(queue, doc, PositionChoice::Invisible, true, Vec::new()),
        Event::PreviewUnavailable { doc: d } if d == doc.id => {
            let effects = vec![Effect::Notify(MSG_POSITIONING_NOT_POSSIBLE.into())];
            to_backend_choice(queue, doc, PositionChoice::Auto, true, effects)
        }
        Event::Navigate(Step::Open) => discard_all(queue, Vec::new()),
        Event::Cancel => next_doc(queue, DocResult::Cancelled, ctx, Vec::new()),
        _ => stay(queue, page, placement, new_page, doc, Vec::new()),
    }
}

fn choosing_backend(state: State, event: Event, ctx: &Context) -> Transition {
    let State::ChoosingBackend { queue, position, options, doc, auto_choice: auto } = state else {
        unreachable!("ChoosingBackend")
    };
    match event {
        Event::BackendsProbed(options) => {
            if !auto {
                return Transition::stay(State::ChoosingBackend { queue, position, options, doc, auto_choice: false });
            }
            match auto_choice(&options, ctx) {
                AutoChoice::Choose(id) => choose_backend(queue, doc, position, options, id, ctx, Vec::new()),
                AutoChoice::Ask => {
                    Transition::stay(State::ChoosingBackend { queue, position, options, doc, auto_choice: false })
                }
                AutoChoice::Fail(error) => {
                    let from = State::ChoosingBackend { queue, position, options, doc, auto_choice: false };
                    fail(from, error, None, ctx, Vec::new())
                }
            }
        }
        Event::BackendChosen(id) => choose_backend(queue, doc, position, options, id, ctx, Vec::new()),
        Event::Navigate(Step::Open) => discard_all(queue, Vec::new()),
        Event::Navigate(Step::Positioning) if !ctx.invisible => {
            let previous = position.manual().copied();
            to_positioning(queue, doc, previous, ctx, Vec::new())
        }
        Event::Cancel => match position {
            PositionChoice::Manual(p) if !ctx.headless => to_positioning(queue, doc, Some(p), ctx, Vec::new()),
            _ => next_doc(queue, DocResult::Cancelled, ctx, Vec::new()),
        },
        _ => Transition::stay(State::ChoosingBackend { queue, position, options, doc, auto_choice: auto }),
    }
}

fn privacy(state: State, event: Event, ctx: &Context) -> Transition {
    let State::PrivacyNotice { queue, position, backend, doc, options } = state else { unreachable!("PrivacyNotice") };
    match event {
        Event::PrivacyAnswered(true) => {
            let caps = options.iter().find(|o| o.id == backend).and_then(|o| o.capabilities.clone());
            let Some(caps) = caps else {
                return Transition::stay(State::ChoosingBackend { queue, position, options, doc, auto_choice: false });
            };
            let flow = SignFlow::new(&caps, &position);
            start_signing(queue, doc, position, backend, flow, vec![Effect::AcknowledgePrivacy(backend)])
        }
        Event::PrivacyAnswered(false) | Event::Cancel => {
            if ctx.headless {
                return next_doc(queue, DocResult::Cancelled, ctx, Vec::new());
            }
            Transition::stay(State::ChoosingBackend { queue, position, options, doc, auto_choice: false })
        }
        Event::Navigate(Step::Open) => discard_all(queue, Vec::new()),
        Event::Navigate(Step::Positioning) if !ctx.invisible => {
            let previous = position.manual().copied();
            to_positioning(queue, doc, previous, ctx, Vec::new())
        }
        _ => Transition::stay(State::PrivacyNotice { queue, position, backend, doc, options }),
    }
}

/// Rückkehr zur Backend-Auswahl ohne automatische Wahl (Abbruch, F-55/F-72).
fn back_to_backend(state: State, ctx: &Context, effects: Vec<Effect>) -> Transition {
    let (queue, doc, position) = match state {
        State::Signing { queue, doc, position, .. } | State::Output { queue, doc, position, .. } => {
            (queue, doc, position)
        }
        other => return Transition::with(other, effects),
    };
    if ctx.headless {
        return next_doc(queue, DocResult::Cancelled, ctx, effects);
    }
    to_backend_choice(queue, doc, position, false, effects)
}

fn signing(state: State, event: Event, ctx: &Context) -> Transition {
    let State::Signing { queue, position, backend, phase, doc, flow } = state else { unreachable!("Signing") };
    match event {
        Event::PhaseCompleted(done) if done == phase => {
            let (next, effect) = match phase {
                SigningPhase::OpeningSession if flow.needs_certificate => {
                    (SigningPhase::ReadingCertificate, vec![Effect::ReadCertificate])
                }
                SigningPhase::OpeningSession | SigningPhase::ReadingCertificate => (
                    SigningPhase::Preparing,
                    vec![Effect::Prepare { doc: doc.id, position: position.clone(), min_reserve: flow.min_reserve }],
                ),
                SigningPhase::Preparing => (SigningPhase::Signing, vec![Effect::Sign]),
                SigningPhase::Signing => (SigningPhase::Finishing, vec![Effect::CloseSession, Effect::Finish]),
                SigningPhase::Finishing | SigningPhase::Writing => {
                    let id = doc.id;
                    return Transition::with(
                        State::Output { queue, saved: None, decision: None, doc, position, backend },
                        vec![Effect::DeletePersisted, Effect::DecideOutput(id)],
                    );
                }
            };
            Transition::with(State::Signing { queue, position, backend, phase: next, doc, flow }, effect)
        }
        Event::BackendFailed(e) => {
            let s = State::Signing { queue, position, backend, phase, doc, flow };
            if e.is_user_cancel() {
                // still zurück zur Auswahl; gemerktes Passwort wird gelöscht (F-72)
                return back_to_backend(s, ctx, vec![Effect::CancelSession, Effect::DeletePersisted]);
            }
            let retry = e.is_retryable().then_some(RetryTarget::Prepare);
            fail(s, WorkflowError::from_backend(&e), retry, ctx, vec![Effect::CloseSession, Effect::DeletePersisted])
        }
        Event::SelfCheckFailed(v) => {
            let s = State::Signing { queue, position, backend, phase, doc, flow };
            fail(s, WorkflowError::Invariant(v), None, ctx, vec![Effect::CloseSession, Effect::DeletePersisted])
        }
        Event::SigningFailed(error) => {
            let retry = match error {
                WorkflowError::Rejected { .. } | WorkflowError::Invariant(_) => None,
                _ => Some(RetryTarget::Prepare),
            };
            let s = State::Signing { queue, position, backend, phase, doc, flow };
            fail(s, error, retry, ctx, vec![Effect::CloseSession, Effect::DeletePersisted])
        }
        Event::CmsTooLarge { needed, reserve } => {
            let retry = Some(RetryTarget::PrepareWithReserve(grown_reserve(needed, reserve)));
            let s = State::Signing { queue, position, backend, phase, doc, flow };
            fail(
                s,
                WorkflowError::CmsTooLarge { needed, reserve },
                retry,
                ctx,
                vec![Effect::CloseSession, Effect::DeletePersisted],
            )
        }
        Event::Cancel => {
            let s = State::Signing { queue, position, backend, phase, doc, flow };
            back_to_backend(s, ctx, vec![Effect::CancelSession, Effect::DeletePersisted])
        }
        _ => Transition::stay(State::Signing { queue, position, backend, phase, doc, flow }),
    }
}

fn output(state: State, event: Event, ctx: &Context) -> Transition {
    let State::Output { queue, saved, decision, doc, position, backend } = state else { unreachable!("Output") };
    let mk = |queue, saved, decision, doc, position, backend| State::Output {
        queue,
        saved,
        decision,
        doc,
        position,
        backend,
    };
    match event {
        Event::OutputDecided(d) if saved.is_none() => decided(mk(queue, saved, None, doc, position, backend), d, ctx),
        Event::OutputChosen(Some(path)) if saved.is_none() => {
            let d = OutputDecision::Save { path, exists: false };
            decided(mk(queue, saved, None, doc, position, backend), d, ctx)
        }
        Event::OutputChosen(None) if saved.is_none() => {
            // Dialog abgebrochen: nicht gespeichert, Knopf „Speichern“ (F-86)
            if ctx.headless {
                return next_doc(queue, DocResult::Cancelled, ctx, Vec::new());
            }
            Transition::stay(mk(queue, None, None, doc, position, backend))
        }
        Event::OverwriteAnswered(yes) if saved.is_none() => match decision {
            Some(OutputDecision::Save { path, exists: true }) if yes => {
                let d = Some(OutputDecision::Save { path: path.clone(), exists: true });
                Transition::with(mk(queue, None, d, doc, position, backend), vec![Effect::Overwrite(path)])
            }
            Some(OutputDecision::Save { exists: true, .. }) => {
                if ctx.headless {
                    return next_doc(queue, DocResult::Cancelled, ctx, Vec::new());
                }
                Transition::stay(mk(queue, None, None, doc, position, backend))
            }
            other => Transition::stay(mk(queue, None, other, doc, position, backend)),
        },
        Event::SaveRequested if saved.is_none() => {
            let Some(input) = queue.current.as_ref().map(|d| d.path.clone()) else {
                return Transition::stay(mk(queue, saved, decision, doc, position, backend));
            };
            let d = save_dialog(&input, &ctx.output_postfix);
            if ctx.headless {
                return decided(mk(queue, None, None, doc, position, backend), d, ctx);
            }
            Transition::with(mk(queue, None, Some(d.clone()), doc, position, backend), vec![Effect::AskSavePath(d)])
        }
        Event::Saved(info) if saved.is_none() => {
            if !queue.pending.is_empty() {
                // F-88: Ergebnis-Screen überspringen, nächstes Dokument
                return next_doc(queue, DocResult::Signed { output: info }, ctx, Vec::new());
            }
            if ctx.skip_finish || ctx.headless {
                // F-90: Ende nach dem Speichern
                let mut queue = queue;
                let mut effects = Vec::new();
                if let Some(id) = queue.current_id() {
                    effects.push(Effect::Discard(id));
                }
                queue.finish_current(DocResult::Signed { output: info });
                return finish(queue, effects);
            }
            Transition::stay(mk(queue, Some(info), None, doc, position, backend))
        }
        Event::SaveFailed(detail) if saved.is_none() => {
            let from = mk(queue, None, decision, doc, position, backend);
            fail(from, WorkflowError::Save(detail), Some(RetryTarget::Save), ctx, Vec::new())
        }
        Event::OpenResultRequested => {
            let effects = saved.as_ref().map(|s| vec![Effect::OpenResult(s.path.clone())]).unwrap_or_default();
            Transition::with(mk(queue, saved, decision, doc, position, backend), effects)
        }
        Event::Cancel if decision.is_some() && saved.is_none() => {
            if ctx.headless {
                return next_doc(queue, DocResult::Cancelled, ctx, Vec::new());
            }
            Transition::stay(mk(queue, None, None, doc, position, backend))
        }
        Event::Navigate(Step::Open) => {
            // F-16: aus dem Ergebnis-Screen wird das nächste Dokument der Warteschlange genommen
            let result = match saved {
                Some(output) => DocResult::Signed { output },
                None => DocResult::Cancelled,
            };
            next_doc(queue, result, ctx, Vec::new())
        }
        Event::Navigate(Step::Positioning) if !ctx.invisible => {
            // F-91: Original erneut signieren, vorherige Position (F-43)
            let previous = position.manual().copied();
            to_positioning(queue, doc, previous, ctx, Vec::new())
        }
        Event::Navigate(Step::Signing) => {
            // F-55: Auswahl erzwingen
            to_backend_choice(queue, doc, position, false, Vec::new())
        }
        _ => Transition::stay(mk(queue, saved, decision, doc, position, backend)),
    }
}

/// Ausgabeziel entschieden: schreiben, Überschreib-Rückfrage oder Speichern-Dialog.
fn decided(state: State, d: OutputDecision, ctx: &Context) -> Transition {
    let State::Output { queue, doc, position, backend, .. } = state else { unreachable!("Output") };
    let (decision, effects) = match d {
        OutputDecision::Save { path, exists: false } => {
            (OutputDecision::Save { path: path.clone(), exists: false }, vec![Effect::Write(path)])
        }
        OutputDecision::Save { path, exists: true } => {
            (OutputDecision::Save { path: path.clone(), exists: true }, vec![Effect::AskOverwrite(path)])
        }
        OutputDecision::Ask { directory, file_name } if ctx.headless => {
            // ohne Dialog neben der Quelle; eine vorhandene Datei wird nicht ersetzt (Write)
            let path = directory.join(file_name);
            (OutputDecision::Save { path: path.clone(), exists: false }, vec![Effect::Write(path)])
        }
        ask @ OutputDecision::Ask { .. } => (ask.clone(), vec![Effect::AskSavePath(ask)]),
    };
    Transition::with(State::Output { queue, saved: None, decision: Some(decision), doc, position, backend }, effects)
}

fn error(state: State, event: Event, ctx: &Context) -> Transition {
    let State::Error { queue, error, retry, from } = state else { unreachable!("Error") };
    match event {
        Event::Retry => match (retry, from) {
            (Some(RetryTarget::ResetSettings), _) => Transition::with(State::Starting, vec![Effect::ResetSettings]),
            (Some(RetryTarget::Load), Some(f)) if matches!(*f, State::Analyzing { .. }) => {
                start_current(queue, ctx, Vec::new())
            }
            (Some(target @ (RetryTarget::Prepare | RetryTarget::PrepareWithReserve(_))), Some(f))
                if matches!(*f, State::Signing { .. }) =>
            {
                let State::Signing { position, backend, doc, mut flow, .. } = *f else { unreachable!("Signing") };
                if let RetryTarget::PrepareWithReserve(n) = target {
                    flow.min_reserve = Some(n.max(flow.min_reserve.unwrap_or(0)));
                }
                start_signing(queue, doc, position, backend, flow, Vec::new())
            }
            (Some(RetryTarget::Save), Some(f)) if matches!(*f, State::Output { .. }) => {
                let State::Output { decision, doc, position, backend, .. } = *f else { unreachable!("Output") };
                let effect = match &decision {
                    Some(OutputDecision::Save { path, exists: true }) => Effect::Overwrite(path.clone()),
                    Some(OutputDecision::Save { path, exists: false }) => Effect::Write(path.clone()),
                    // ohne festes Ziel: Speichern-Dialog erneut zeigen (sonst wartete der Schritt ohne Dialog)
                    Some(ask @ OutputDecision::Ask { .. }) => Effect::AskSavePath(ask.clone()),
                    None => {
                        let s = State::Output { queue, saved: None, decision: None, doc, position, backend };
                        return Transition::stay(s);
                    }
                };
                let s = State::Output { queue, saved: None, decision, doc, position, backend };
                Transition::with(s, vec![effect])
            }
            (retry, from) => Transition::stay(State::Error { queue, error, retry, from }),
        },
        Event::Cancel => match from.map(|b| *b) {
            Some(State::Starting) | None if matches!(error, WorkflowError::Config(_)) => {
                finish_with(ExitCode::General, queue.done, Vec::new())
            }
            Some(State::Analyzing { .. }) => next_doc(queue, DocResult::Failed { error }, ctx, Vec::new()),
            Some(State::Signing { doc, position, .. }) | Some(State::ChoosingBackend { doc, position, .. }) => {
                to_backend_choice(queue, doc, position, false, Vec::new())
            }
            Some(State::Output { doc, position, backend, .. }) => {
                // nicht gespeichert: Knopf „Speichern“ (F-86); der Batch läuft über „Öffnen“ weiter (B-14:
                // die Warteschlange bleibt erhalten)
                Transition::stay(State::Output { queue, saved: None, decision: None, doc, position, backend })
            }
            Some(mut other) => {
                if let Some(q) = other.queue_mut() {
                    *q = queue;
                }
                Transition::stay(other)
            }
            None => Transition::stay(State::Idle { queue }),
        },
        _ => Transition::stay(State::Error { queue, error, retry, from }),
    }
}
