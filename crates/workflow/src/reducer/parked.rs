//! Geparkte Arbeit („Zur Startseite“, ADR 0004 Nachtrag 10; nur im Arbeitsbereich-Modus mit Oberfläche).
//!
//! „Zur Startseite“ (Haus-Symbol, Wortmarke, Palette) schließt nichts mehr: Der Zustand bleibt mit Dokument,
//! Platzierung bzw. Stapel erhalten und wird nur als geparkt markiert ([`crate::Queue::parked`]); die Oberfläche
//! zeigt die Startansicht mit der Karte „In Bearbeitung“.
//!
//! | Zustand | Ereignis | Ergebnis |
//! |---|---|---|
//! | Arbeitsbereich (`Positioning`), Analysewarnung, Stapelübersicht, „Fertig“ ohne Speichern | `WentHome` | geparkt, Zustand unverändert |
//! | „Fertig“ gespeichert, weitere Dokumente warten (Stapelübersicht) | `WentHome` | wie „Zum Stapel“ (signierter Eintrag), die Übersicht wird geparkt |
//! | Stapeleintrag wird geladen | `WentHome` | zurück zur Übersicht (Eintrag unverändert), geparkt |
//! | Signieren, Stapellauf, Fehlerschritt, Start, Ende, Startansicht | `WentHome` | ohne Wirkung |
//! | sonst (Laden, „Signatur fortsetzen“, Signiermittel-Auswahl, „Fertig“ gespeichert ohne wartende) | `WentHome` | wie `Navigate(Open)` (wie bisher) |
//! | geparkt | `ParkedResumed` („Weiter bearbeiten“) | Zustand wie vor dem Parken |
//! | geparkt | `ParkedDiscarded` („Schließen“) | verworfen wie „Öffnen“ → `Idle` (Dokumente „abgebrochen“, signierte Ergebnisse bleiben) |
//! | geparkt | `MoreFilesAdded` („Zum Stapel hinzufügen“) | fortsetzen und hinzufügen: Arbeitsbereich wie „Weitere PDFs hinzufügen“ (alle zusammen zum Stapel, die Platzierung bleibt Plan), Übersicht mit neuen Einträgen, Analysewarnung → Übersicht |
//! | geparkt | `FilesAdded` („Öffnen mit“, zweite Instanz; ohne Rückfrage) | wie `MoreFilesAdded`, dazu Hinweis [`MSG_PARKED_FILES_ADDED`] |
//! | geparkt | `DocumentReplaced` („Offene schließen und neu beginnen“) | verwerfen, dann wie in der Dokumentauswahl |
//! | geparkt | Ergebnisse von Effekten, geänderte Einstellungen, Voranalyse, Lebenszyklus | wirken auf die geparkte Arbeit, sofern sie danach geparkt bleiben kann (sonst ohne Wirkung) |
//! | geparkt | alles andere (Signieren, Platzieren, Stapel starten, Abbrechen …) | ohne Wirkung – die Arbeit ist verdeckt (F-54) |
//!
//! Invariante A1: geparkte Arbeit öffnet nie eine Sitzung, Parken und Fortsetzen starten nichts (Property-Test
//! `workspace_session_only_after_explicit_action`).

use super::{active, batch_run, discard_all};
use crate::queue::{InputDocument, Queue};
use crate::state::{Context, Effect, Event, MSG_BATCH_ADDED, MSG_PARKED_FILES_ADDED, State, Step, Transition};

/// Kann dieser Zustand geparkt werden? Ein Dokument bzw. Stapel ist offen, und nichts läuft (kein Signieren,
/// kein Laden, keine offene Speicher-Entscheidung).
fn parkable(state: &State) -> bool {
    matches!(
        state,
        State::Positioning { edit: Some(_), .. }
            | State::AnalysisWarning { .. }
            | State::BatchOverview { .. }
            | State::Output { saved: None, decision: None, .. }
    )
}

fn set_parked(mut state: State, parked: bool) -> State {
    if let Some(q) = state.queue_mut() {
        q.parked = parked;
    }
    state
}

/// „Zur Startseite“ ohne geparkte Arbeit.
pub(super) fn went_home(state: State, ctx: &Context) -> Transition {
    // klassischer Ablauf bzw. ohne Oberfläche: wie die Hauptleiste „Öffnen“ (§3.0 Regel 7)
    if !ctx.workspace || ctx.headless {
        return active(state, Event::Navigate(Step::Open), ctx);
    }
    match state {
        // schon auf der Startansicht bzw. Signieren, Stapellauf, Fehlerschritt: ohne Wirkung (die Oberfläche
        // sperrt „Zur Startseite“ dort, `actions.home`)
        s @ (State::Starting
        | State::Idle { .. }
        | State::Signing { .. }
        | State::BatchSigning { .. }
        | State::Error { .. }
        | State::Finished { .. }) => Transition::stay(s),
        s if parkable(&s) => Transition::stay(set_parked(s, true)),
        s => {
            // gespeichert und weitere Dokumente warten: der Stapel mit dem signierten Eintrag wird geparkt (wie
            // „Zum Stapel“); sonst wie bisher „Öffnen“ – aus einem Stapeleintrag, der gerade geladen wird, zurück
            // zur Übersicht, die dann geparkt wird
            let waiting = s.queue().is_some_and(|q| !q.pending.is_empty());
            let event = if ctx.batch() && waiting && matches!(s, State::Output { saved: Some(_), .. }) {
                Event::ShowBatch
            } else {
                Event::Navigate(Step::Open)
            };
            let t = active(s, event, ctx);
            if parkable(&t.state) { Transition::with(set_parked(t.state, true), t.effects) } else { t }
        }
    }
}

/// Ereignisse, die auf geparkte Arbeit wirken: Ergebnisse von Effekten, die sie selbst angefordert hat
/// (automatische Platzierung, QR-Suche, Blockgröße, Voranalyse, Schreiben nach „Stapel abbrechen“), geänderte
/// Einstellungen („sofort wirksam“) und der Lebenszyklus der App.
fn passes_through(event: &Event) -> bool {
    matches!(
        event,
        Event::SettingsChanged
            | Event::Preanalyzed { .. }
            | Event::AutoPlaced(_)
            | Event::BlockResized(_)
            | Event::PlaceholderSearched { .. }
            | Event::PreviewUnavailable { .. }
            | Event::Saved(_)
            | Event::SaveFailed(_)
            | Event::BatchOutputDecided { .. }
            | Event::Backgrounded
            | Event::Foregrounded
    )
}

/// Reducer für geparkte Arbeit ([`State::parked`]).
pub(super) fn reduce(state: State, event: Event, ctx: &Context) -> Transition {
    match event {
        // „Weiter bearbeiten“: unverändert zurück (Platzierung, Plan, Stapel); startet nichts
        Event::ParkedResumed => Transition::stay(set_parked(state, false)),
        Event::ParkedDiscarded => discard(state),
        Event::Quit => active(state, Event::Quit, ctx),
        Event::MoreFilesAdded(docs) => join(state, docs, false, ctx),
        Event::FilesAdded(docs) => join(state, docs, true, ctx),
        // „Offene schließen und neu beginnen“: verwerfen, dann wie in der Dokumentauswahl
        Event::DocumentReplaced(docs) if !docs.is_empty() => {
            let closed = discard(state);
            let opened = active(closed.state, Event::FilesAdded(docs), ctx);
            let mut effects = closed.effects;
            effects.extend(opened.effects);
            Transition::with(opened.state, effects)
        }
        event if passes_through(&event) => {
            let before = state.clone();
            let t = active(state, event, ctx);
            // die Arbeit bleibt geparkt; was sie aus der Startansicht holen würde, bleibt ohne Wirkung
            if t.state.parked() && parkable(&t.state) { t } else { Transition::stay(before) }
        }
        // alles andere betrifft die verdeckte Arbeit (Signieren, Platzieren, Stapel starten, Abbrechen …)
        _ => Transition::stay(state),
    }
}

/// Geparkte Arbeit verwerfen wie „Öffnen“: Dokumente freigeben und als „abgebrochen“ verbuchen, Stapel entfällt;
/// bereits signierte Ergebnisse bleiben in der Ergebnisliste.
fn discard(state: State) -> Transition {
    let mut state = set_parked(state, false);
    match state.queue_mut().map(std::mem::take) {
        Some(queue) => discard_all(queue, Vec::new()),
        None => Transition::stay(state),
    }
}

/// Neue Dateien zur geparkten Arbeit („Zum Stapel hinzufügen“ bzw. – `open_with` – „Öffnen mit“ und zweite
/// Instanz, ohne Rückfrage mit Hinweis): fortsetzen, dann hinzufügen. Mit Stapelübersicht werden die geparkten und
/// die neuen Dokumente zum Stapel (ein geparktes Dokument behält seine Platzierung als Plan), sonst kommen die
/// neuen an die Warteschlange.
fn join(state: State, docs: Vec<InputDocument>, open_with: bool, ctx: &Context) -> Transition {
    let state = set_parked(state, false);
    let before = state.queue().map_or(0, Queue::len);
    let mut t =
        if ctx.batch() { batch_run::join_parked(state, docs, ctx) } else { super::files_added(state, docs, ctx) };
    let added = t.state.queue().map_or(0, Queue::len).saturating_sub(before);
    let noticed = t.effects.iter().any(|e| matches!(e, Effect::NotifyWith(code, _) if code == MSG_BATCH_ADDED));
    if open_with && added > 0 && !noticed {
        let params = std::iter::once(("count".to_owned(), added.to_string())).collect();
        t.effects.push(Effect::NotifyWith(MSG_PARKED_FILES_ADDED.to_owned(), params));
    }
    t
}
