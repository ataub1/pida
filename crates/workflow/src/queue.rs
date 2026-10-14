//! Warteschlange/Batch (F-14, F-88, F-89).

use std::collections::VecDeque;
use std::path::PathBuf;

/// Kennung eines Dokuments innerhalb einer Sitzung.
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

/// Aktuelles Dokument plus wartende.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Queue {
    /// aktuelles Dokument
    pub current: Option<InputDocument>,
    /// wartende Dokumente
    pub pending: VecDeque<InputDocument>,
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

    /// Alles verwerfen (Hauptleiste „Öffnen“ außerhalb des Ergebnis-Screens, F-16).
    pub fn clear(&mut self) {
        self.current = None;
        self.pending.clear();
    }

    /// Anzahl insgesamt.
    pub fn len(&self) -> usize {
        usize::from(self.current.is_some()) + self.pending.len()
    }

    /// Leer?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
