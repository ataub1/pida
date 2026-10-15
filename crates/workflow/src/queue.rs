//! Warteschlange/Batch (F-14, F-88, F-89).
//!
//! Die Warteschlange hält das **aktuelle** Dokument, die **wartenden** Dokumente und die
//! **Ergebnisse** bereits abgeschlossener Dokumente ([`Queue::done`]). Aus den Ergebnissen leiten die
//! CLI (Exit-Code, `--json`) und das Warteschlangen-Panel der UI ihre Anzeige ab; sie gehen auch bei
//! schnellen Übergängen (automatisch weiter, F-88) nicht verloren.

use std::collections::VecDeque;
use std::path::PathBuf;

use pdfover_pades::OutputInfo;

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
    /// und alle wartenden Dokumente gelten als abgebrochen.
    pub fn clear(&mut self) {
        let mut rest: Vec<InputDocument> = self.current.take().into_iter().collect();
        rest.extend(self.pending.drain(..));
        self.done.extend(rest.into_iter().map(|doc| DocOutcome { doc, result: DocResult::Cancelled }));
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
    fn serde_roundtrip_without_done_field() {
        let json = r#"{"current":null,"pending":[]}"#;
        let q: Queue = serde_json::from_str(json).unwrap();
        assert!(q.done.is_empty());
    }
}
