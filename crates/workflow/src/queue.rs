//! Warteschlange/Batch (F-14, F-88, F-89).
//!
//! Die Warteschlange hält das **aktuelle** Dokument, die **wartenden** Dokumente und die
//! **Ergebnisse** bereits abgeschlossener Dokumente ([`Queue::done`]). Aus den Ergebnissen leiten die
//! CLI (Exit-Code, `--json`) und das Warteschlangen-Panel der UI ihre Anzeige ab; sie gehen auch bei
//! schnellen Übergängen (automatisch weiter, F-88) nicht verloren.

use std::collections::VecDeque;
use std::path::PathBuf;

use pida_pades::OutputInfo;

use crate::batch::Batch;
use crate::output::ExitCode;
use crate::state::WorkflowError;

/// Kennung eines Dokuments innerhalb einer Sitzung.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct DocId(pub u64);

/// Eingabedokument.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InputDocument {
    /// Kennung
    pub id: DocId,
    /// Pfad (Desktop) bzw. Sandbox-Kopie (Mobile)
    pub path: PathBuf,
    /// Anzeigename
    pub display_name: String,
}

impl InputDocument {
    /// Dokument mit dem Dateinamen als Anzeigenamen.
    pub fn new(id: DocId, path: impl Into<PathBuf>) -> InputDocument {
        let path = path.into();
        let display_name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        InputDocument { id, path, display_name }
    }
}

/// Ergebnis eines abgeschlossenen Dokuments.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocResult {
    /// signiert und gespeichert
    Signed {
        /// Ausgabe
        #[cfg_attr(feature = "ts", ts(as = "crate::ts_mirror::OutputInfo"))]
        output: OutputInfo,
    },
    /// von der Nutzerin verworfen bzw. abgebrochen (auch: Speichern abgelehnt)
    Cancelled,
    /// fehlgeschlagen
    Failed {
        /// Fehler (Code und Parameter für i18n)
        error: WorkflowError,
    },
}

impl DocResult {
    /// Exit-Code dieses Ergebnisses (Plan §7.4).
    pub fn exit_code(&self) -> ExitCode {
        match self {
            DocResult::Signed { .. } => ExitCode::Success,
            DocResult::Cancelled => ExitCode::Cancelled,
            DocResult::Failed { error } => error.exit_code(),
        }
    }
}

/// Abgeschlossenes Dokument.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocOutcome {
    /// Dokument
    pub doc: InputDocument,
    /// Ergebnis
    pub result: DocResult,
}

/// Aktuelles Dokument plus wartende.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Queue {
    /// aktuelles Dokument
    pub current: Option<InputDocument>,
    /// wartende Dokumente
    pub pending: VecDeque<InputDocument>,
    /// abgeschlossene Dokumente in Reihenfolge (Ergebnisliste für CLI und Warteschlangen-Panel)
    #[serde(default)]
    pub done: Vec<DocOutcome>,
    /// Stapel (Arbeitsbereich mit Stapelübersicht, ADR 0004 §3.8): Einträge der Übersicht; reist mit der
    /// Warteschlange auch durch die Zustände eines aus der Übersicht geöffneten Einzeldokuments
    #[serde(default)]
    pub batch: Option<Batch>,
    /// Arbeit geparkt („Zur Startseite“, ADR 0004 Nachtrag 10): Der Zustand (Arbeitsbereich, Analysewarnung,
    /// Stapelübersicht, „Fertig“ ohne Speichern) bleibt mit Dokument, Platzierung bzw. Stapel erhalten, die
    /// Oberfläche zeigt die Startansicht mit der Karte „In Bearbeitung“. Bedienung der verdeckten Arbeit bleibt
    /// ohne Wirkung, bis sie fortgesetzt (`Event::ParkedResumed`) bzw. verworfen wird (`Event::ParkedDiscarded`)
    #[serde(default)]
    pub parked: bool,
}

impl Queue {
    /// Fügt Dokumente an (ohne Duplikate nach Pfad, F-89); das erste wird aktuell, falls keines.
    pub fn add(&mut self, docs: impl IntoIterator<Item = InputDocument>) {
        for d in docs {
            let dup = self.current.as_ref().is_some_and(|c| c.path == d.path)
                || self.pending.iter().any(|p| p.path == d.path);
            if dup {
                continue;
            }
            if self.current.is_none() {
                self.current = Some(d);
            } else {
                self.pending.push_back(d);
            }
        }
    }

    /// Nächstes Dokument wird aktuell; liefert es.
    pub fn advance(&mut self) -> Option<&InputDocument> {
        self.current = self.pending.pop_front();
        self.current.as_ref()
    }

    /// Schließt das aktuelle Dokument mit `result` ab (Ergebnisliste) und macht das nächste aktuell.
    /// Ohne aktuelles Dokument wird nur weitergeschaltet.
    pub fn finish_current(&mut self, result: DocResult) -> Option<&InputDocument> {
        if let Some(doc) = self.current.take() {
            self.done.push(DocOutcome { doc, result });
        }
        self.advance()
    }

    /// Alles verwerfen (Hauptleiste „Öffnen“ außerhalb des Ergebnis-Screens, F-16). Das aktuelle
    /// und alle wartenden Dokumente gelten als abgebrochen; ein Stapel bzw. geparkte Arbeit entfällt.
    pub fn clear(&mut self) {
        let mut rest: Vec<InputDocument> = self.current.take().into_iter().collect();
        rest.extend(self.pending.drain(..));
        self.done.extend(rest.into_iter().map(|doc| DocOutcome { doc, result: DocResult::Cancelled }));
        self.batch = None;
        self.parked = false;
    }

    /// Fügt Dokumente **nur** an die wartenden an (ohne Duplikate nach Pfad) – auch ohne aktuelles Dokument
    /// (Stapelübersicht). Liefert die tatsächlich angefügten.
    pub fn append(&mut self, docs: impl IntoIterator<Item = InputDocument>) -> Vec<InputDocument> {
        let mut added = Vec::new();
        for d in docs {
            let dup = self.current.as_ref().is_some_and(|c| c.path == d.path)
                || self.pending.iter().any(|p| p.path == d.path)
                || added.iter().any(|a: &InputDocument| a.path == d.path);
            if !dup {
                added.push(d);
            }
        }
        self.pending.extend(added.iter().cloned());
        added
    }

    /// Macht `id` zum aktuellen Dokument (ADR 0004 §3.8); ein bisheriges aktuelles Dokument kommt an den
    /// Anfang der wartenden. `false`, wenn das Dokument nicht (mehr) offen ist.
    pub fn make_current(&mut self, id: DocId) -> bool {
        if self.current_id() == Some(id) {
            return true;
        }
        let Some(pos) = self.pending.iter().position(|d| d.id == id) else { return false };
        let doc = self.pending.remove(pos).expect("Index geprüft");
        if let Some(old) = self.current.replace(doc) {
            self.pending.push_front(old);
        }
        true
    }

    /// Das aktuelle Dokument kommt zurück zu den wartenden (Stapelübersicht ohne aktuelles Dokument).
    pub fn park_current(&mut self) {
        if let Some(doc) = self.current.take() {
            self.pending.push_front(doc);
        }
    }

    /// Entfernt ein offenes Dokument ohne Ergebnis (aus dem Stapel entfernt). `false`, wenn es nicht offen ist.
    pub fn remove(&mut self, id: DocId) -> bool {
        if self.current_id() == Some(id) {
            self.current = None;
            return true;
        }
        match self.pending.iter().position(|d| d.id == id) {
            Some(pos) => {
                self.pending.remove(pos);
                true
            }
            None => false,
        }
    }

    /// Schließt ein offenes Dokument mit `result` ab (Ergebnisliste), **ohne** weiterzuschalten.
    pub fn complete(&mut self, id: DocId, result: DocResult) {
        let doc = if self.current_id() == Some(id) {
            self.current.take()
        } else {
            self.pending.iter().position(|d| d.id == id).and_then(|pos| self.pending.remove(pos))
        };
        if let Some(doc) = doc {
            self.done.push(DocOutcome { doc, result });
        }
    }

    /// Eingabedokument zur Kennung (offen oder abgeschlossen).
    pub fn find(&self, id: DocId) -> Option<&InputDocument> {
        self.current
            .iter()
            .chain(self.pending.iter())
            .find(|d| d.id == id)
            .or_else(|| self.done.iter().rev().map(|o| &o.doc).find(|d| d.id == id))
    }

    /// Anzahl offener Dokumente (aktuell + wartend).
    pub fn len(&self) -> usize {
        usize::from(self.current.is_some()) + self.pending.len()
    }

    /// Keine offenen Dokumente?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Kennung des aktuellen Dokuments.
    pub fn current_id(&self) -> Option<DocId> {
        self.current.as_ref().map(|d| d.id)
    }

    /// Exit-Code aus den Ergebnissen: 0, wenn alle signiert wurden (oder es keine gab), sonst der Code
    /// des **ersten** nicht erfolgreichen Dokuments (deterministisch, Plan §7.4).
    pub fn exit_code(&self) -> ExitCode {
        self.done.iter().map(|o| o.result.exit_code()).find(|c| *c != ExitCode::Success).unwrap_or(ExitCode::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: u64, path: &str) -> InputDocument {
        InputDocument::new(DocId(id), path)
    }

    #[test]
    fn add_dedupes_and_sets_current() {
        let mut q = Queue::default();
        q.add([doc(1, "/a.pdf"), doc(2, "/b.pdf"), doc(3, "/a.pdf")]);
        assert_eq!(q.current_id(), Some(DocId(1)));
        assert_eq!(q.pending.len(), 1);
        assert_eq!(q.len(), 2);
        assert_eq!(q.current.as_ref().unwrap().display_name, "a.pdf");
    }

    #[test]
    fn finish_records_results_and_exit_code() {
        let mut q = Queue::default();
        q.add([doc(1, "/a.pdf"), doc(2, "/b.pdf"), doc(3, "/c.pdf")]);
        let out = OutputInfo { path: "/a_signed.pdf".into(), len: 1, sha256: "00".into() };
        q.finish_current(DocResult::Signed { output: out });
        assert_eq!(q.exit_code(), ExitCode::Success);
        q.finish_current(DocResult::Failed { error: WorkflowError::Save("x".into()) });
        q.finish_current(DocResult::Cancelled);
        assert!(q.is_empty());
        assert_eq!(q.done.len(), 3);
        // erster Fehler bestimmt den Code
        assert_eq!(q.exit_code(), ExitCode::Write);
    }

    #[test]
    fn clear_marks_everything_cancelled() {
        let mut q = Queue::default();
        q.add([doc(1, "/a.pdf"), doc(2, "/b.pdf")]);
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.done.len(), 2);
        assert!(q.done.iter().all(|o| o.result == DocResult::Cancelled));
        assert_eq!(q.exit_code(), ExitCode::Cancelled);
    }

    #[test]
    fn batch_helpers_keep_the_documents() {
        let mut q = Queue::default();
        let added = q.append([doc(1, "/a.pdf"), doc(2, "/b.pdf"), doc(3, "/a.pdf")]);
        assert_eq!(added.len(), 2);
        assert_eq!(q.current_id(), None, "append macht kein Dokument aktuell");
        assert!(q.make_current(DocId(2)));
        assert_eq!(q.current_id(), Some(DocId(2)));
        assert!(q.make_current(DocId(1)));
        assert_eq!(q.pending.front().map(|d| d.id), Some(DocId(2)), "bisheriges aktuelles vorn");
        assert!(!q.make_current(DocId(9)));
        q.complete(DocId(1), DocResult::Cancelled);
        assert_eq!(q.current_id(), None, "complete schaltet nicht weiter");
        assert_eq!(q.find(DocId(1)).map(|d| d.display_name.as_str()), Some("a.pdf"), "auch abgeschlossene");
        assert!(q.remove(DocId(2)) && q.is_empty());
        assert!(!q.remove(DocId(2)));
        q.add([doc(4, "/c.pdf")]);
        q.park_current();
        assert_eq!((q.current_id(), q.pending.len()), (None, 1));
    }

    #[test]
    fn serde_roundtrip_without_done_field() {
        let json = r#"{"current":null,"pending":[]}"#;
        let q: Queue = serde_json::from_str(json).unwrap();
        assert!(q.done.is_empty());
    }
}
