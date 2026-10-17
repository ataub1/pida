//! Geparkte Arbeit in der Projektion (ADR 0004 Nachtrag 10): „Zur Startseite“ (`go_home`) parkt das offene
//! Dokument bzw. den Stapel im Reducer (`pida_workflow::Queue::parked`); die Startansicht zeigt dazu die Karte
//! „In Bearbeitung“ ([`ParkedWorkView`]) mit „Weiter bearbeiten“ (`resume_parked`) und „Schließen“
//! (`discard_parked`). Solange geparkt ist, projiziert der Kern Schritt, Hauptleiste und Freigaben wie in der
//! Dokumentauswahl, ohne `document` und `batch` (`CoreTask::project`).
//!
//! Die Karte beschreibt, wohin „Weiter bearbeiten“ führt (Nachtrag 13): ein Stapel
//! ([`State::resumes_to_overview`] – Übersicht, aus ihr geöffnetes Dokument, Dokument mit wartenden) heißt
//! „Stapel · n Dokumente“ (`doc_name = None`), sonst nennt sie das Dokument – auch „Fertig“ ohne Speichern eines
//! Stapeleintrags, denn dorthin geht es zurück (die Signatur ist erst gespeichert sicher).

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
        // „Weiter bearbeiten“ führt zur Stapelübersicht → „Stapel · n Dokumente“, sonst zum Dokument
        let doc_name = if self.state.resumes_to_overview(&self.ctx) {
            None
        } else {
            queue.current.as_ref().map(|d| d.display_name.clone())
        };
        Some(match queue.batch.as_ref() {
            // Stapelübersicht bzw. ein aus ihr geöffnetes Dokument: Einträge des Stapels
            Some(batch) => ParkedWorkView {
                doc_name,
                documents: count(batch.items.len()),
                placed_by_hand: by_hand || batch.manual_plans() > 0,
                unsaved,
            },
            // Dokument samt wartenden
            None => ParkedWorkView { doc_name, documents: count(queue.len()), placed_by_hand: by_hand, unsaved },
        })
    }
}
