//! Geparkte Arbeit in der Projektion (ADR 0004 Nachtrag 10): „Zur Startseite“ (`go_home`) parkt das offene
//! Dokument bzw. den Stapel im Reducer (`pida_workflow::Queue::parked`); die Startansicht zeigt dazu die Karte
//! „In Bearbeitung“ ([`ParkedWorkView`]) mit „Weiter bearbeiten“ (`resume_parked`) und „Schließen“
//! (`discard_parked`). Solange geparkt ist, projiziert der Kern Schritt, Hauptleiste und Freigaben wie in der
//! Dokumentauswahl, ohne `document` und `batch` (`CoreTask::project`).

use pida_workflow::State;

use crate::runtime::CoreTask;
use crate::view::ParkedWorkView;

impl CoreTask {
    /// `ViewState::parked`: Name bzw. Stapel, Zahl der Dokumente und ob „Schließen“ Arbeit der Nutzerin verwirft
    /// (eigene Platzierung, Stapeleinträge mit eigenem Plan, signiert und nicht gespeichert).
    pub(crate) fn parked_view(&self) -> Option<ParkedWorkView> {
        if !self.state.parked() {
            return None;
        }
        let queue = self.state.queue()?;
        let edit = match &self.state {
            State::Positioning { edit, .. } | State::Output { edit, .. } => edit.as_ref(),
            _ => None,
        };
        // wie `DocumentDetectionView::auto_placed`: „Unsichtbar signieren“ allein gilt nicht als eigene Platzierung
        let by_hand = edit.is_some_and(|e| !e.automatic);
        let unsaved = matches!(self.state, State::Output { saved: None, .. });
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Some(match queue.batch.as_ref() {
            // Stapelübersicht bzw. ein aus ihr geöffnetes Dokument
            Some(batch) => ParkedWorkView {
                doc_name: None,
                documents: count(batch.items.len()),
                placed_by_hand: by_hand || batch.manual_plans() > 0,
                unsaved,
            },
            None => ParkedWorkView {
                doc_name: queue.current.as_ref().map(|d| d.display_name.clone()),
                documents: count(queue.len()),
                placed_by_hand: by_hand,
                unsaved,
            },
        })
    }
}
