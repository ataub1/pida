//! Dateien öffnen (`open_files` aus Dateiauswahl, Ablegen, „+ PDF“, „Öffnen mit“, zweiter Instanz; ADR 0004
//! Nachtrag 17 „Mehrfachauswahl“).
//!
//! Der Kern übernimmt **alle** gewählten Dateien in der Reihenfolge der Auswahl (ab zwei offenen Dokumenten als
//! Stapel). Was er nicht übernimmt, meldet er als Hinweis, statt es still zu verwerfen:
//!
//! - **Duplikate** – dasselbe Dokument ist schon offen (Arbeitsbereich, Warteschlange, Stapel, auch geparkt) oder
//!   steht zweimal in der Auswahl. Was als „schon offen“ gilt, entscheidet weiterhin der Reducer nach dem
//!   Zustand (Pfadvergleich der Warteschlange, F-89); der Kern zählt nach dem Übergang nur nach, welche der neuen
//!   Dokumente nicht offen sind, obwohl ein offenes Dokument denselben Pfad hat → [`MSG_OPEN_DUPLICATES`].
//!   Auf iOS/Android liegt jede gewählte Datei als **neue Kopie** in der Sandbox (eigener Unterordner je
//!   Datei), der Pfad ist also nie gleich: Dort gilt eine Kopie mit gleichem Dateinamen und byte-gleichem Inhalt
//!   wie ein offenes bzw. früher in derselben Auswahl gewähltes Dokument als dasselbe Dokument – sie bekommt
//!   dessen Pfad, danach entscheidet der Reducer wie auf dem Desktop.
//! - **Fehlende Dateien** – kein (vorhandener) Dateipfad, z. B. ein abgelegter Ordner oder eine inzwischen
//!   gelöschte Datei (F-12) → [`MSG_OPEN_MISSING`].
//!
//! Dateien, die zwar vorhanden, aber kein lesbares PDF sind, übernimmt der Kern; die Analyse meldet sie wie bisher
//! (Fehlerschritt bzw. Stapeleintrag „abgelehnt“).

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use pida_workflow::{DocId, InputDocument};

use crate::runtime::CoreTask;

/// Meldungs-ID: gewählte Dokumente sind schon offen (bzw. doppelt gewählt) und kamen nicht noch einmal dazu.
/// Parameter `count`, bei einem Dokument `file` (Dateiname).
pub(crate) const MSG_OPEN_DUPLICATES: &str = "ng-open-duplicates";
/// Meldungs-ID: gewählte Pfade sind keine (vorhandenen) Dateien. Parameter `count`.
pub(crate) const MSG_OPEN_MISSING: &str = "ng-open-missing";

/// Vorbereitete Auswahl eines `open_files` (vor dem Reducer).
pub(crate) struct Selection {
    /// neue Dokumente in der Reihenfolge der Auswahl
    pub docs: Vec<InputDocument>,
    /// Pfade, die keine (vorhandenen) Dateien sind
    missing: usize,
}

impl CoreTask {
    /// Offene Dokumente (aktuelles und wartende, auch Stapeleinträge und geparkte Arbeit).
    fn open_documents(&self) -> Vec<&InputDocument> {
        self.state.queue().map(|q| q.current.iter().chain(q.pending.iter()).collect()).unwrap_or_default()
    }

    /// Mobile: Dokumente, deren Kopien als „dasselbe Dokument“ gelten – die offenen ohne ein eben signiertes und
    /// gespeichertes („Fertig“, `Output` mit `saved`): eine neue Kopie derselben Datei („Teilen“ legt jedes Mal eine
    /// an) öffnet wie vor Nachtrag 12 als nächstes Dokument (F-89 „sofort weiter“), statt als Duplikat gemeldet zu
    /// werden (Nachtrag 12, Nachprüfung 16.10.2026; mit Nachtrag 17 zusammengeführt am 17.10.2026). Derselbe Pfad
    /// bleibt Sache des Reducers wie auf dem Desktop.
    fn copy_targets(&self) -> Vec<PathBuf> {
        let finished = matches!(self.state, pida_workflow::State::Output { saved: Some(_), .. });
        self.state
            .queue()
            .map(|q| q.current.iter().filter(|_| !finished).chain(q.pending.iter()).map(|d| d.path.clone()).collect())
            .unwrap_or_default()
    }

    /// Gewählte Pfade → neue Dokumente (Reihenfolge der Auswahl). Mobile: Kopien eines schon offenen bzw. in
    /// dieser Auswahl früher gewählten Dokuments bekommen dessen Pfad (siehe Modul).
    pub(crate) fn select_documents(&mut self, paths: &[PathBuf]) -> Selection {
        let copies = !self.mobile.platform.is_desktop();
        let open: Vec<PathBuf> = if copies { self.copy_targets() } else { Vec::new() };
        let mut chosen: Vec<PathBuf> = Vec::with_capacity(paths.len());
        let mut missing = 0;
        for path in paths {
            if !path.is_file() {
                missing += 1;
                continue;
            }
            let same = copies.then(|| open.iter().chain(chosen.iter()).find(|o| same_copy(o, path)).cloned()).flatten();
            chosen.push(same.unwrap_or_else(|| path.clone()));
        }
        let docs = self.input_documents(&chosen);
        Selection { docs, missing }
    }

    /// Nach dem Übergang: die neuen Dokumente der Auswahl, die jetzt offen sind (ohne nicht übernommene Duplikate).
    pub(crate) fn taken(&self, selection: &Selection) -> Vec<DocId> {
        let ids: HashSet<DocId> = self.open_documents().iter().map(|d| d.id).collect();
        selection.docs.iter().map(|d| d.id).filter(|id| ids.contains(id)).collect()
    }

    /// Nach dem Übergang: nicht übernommene Duplikate und fehlende Dateien als Hinweis melden.
    pub(crate) fn report_selection(&mut self, selection: &Selection) {
        let open = self.open_documents();
        let ids: HashSet<DocId> = open.iter().map(|d| d.id).collect();
        let duplicates: Vec<&InputDocument> =
            selection.docs.iter().filter(|d| !ids.contains(&d.id) && open.iter().any(|o| o.path == d.path)).collect();
        let count = duplicates.len();
        let first = duplicates.first().map(|d| d.display_name.clone());
        if count > 0 {
            tracing::info!(count, "Gewählte Dokumente schon offen – nicht noch einmal hinzugefügt");
            let mut params = BTreeMap::from([("count".to_owned(), count.to_string())]);
            if let (1, Some(name)) = (count, first) {
                params.insert("file".to_owned(), name);
            }
            self.notify(MSG_OPEN_DUPLICATES, params);
        }
        if selection.missing > 0 {
            self.notify(MSG_OPEN_MISSING, BTreeMap::from([("count".to_owned(), selection.missing.to_string())]));
        }
    }
}

/// Zwei Sandbox-Kopien desselben Dokuments: verschiedene Pfade, gleicher Dateiname, gleiche Größe, gleicher
/// Inhalt (Mobile, siehe Modul).
fn same_copy(open: &Path, chosen: &Path) -> bool {
    if open == chosen || open.file_name() != chosen.file_name() {
        return false;
    }
    match (std::fs::metadata(open), std::fs::metadata(chosen)) {
        (Ok(a), Ok(b)) if a.is_file() && b.is_file() && a.len() == b.len() => same_bytes(open, chosen).unwrap_or(false),
        _ => false,
    }
}

fn same_bytes(a: &Path, b: &Path) -> std::io::Result<bool> {
    let mut a = BufReader::new(File::open(a)?);
    let mut b = BufReader::new(File::open(b)?);
    let mut ba = [0u8; 64 * 1024];
    let mut bb = [0u8; 64 * 1024];
    loop {
        let n = read_full(&mut a, &mut ba)?;
        let m = read_full(&mut b, &mut bb)?;
        if n != m || ba[..n] != bb[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

/// Liest, bis der Puffer voll ist oder die Datei endet.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_with_same_name_and_bytes_are_the_same_document() {
        let dir = tempfile::tempdir().unwrap();
        let file = |sub: &str, name: &str, bytes: &[u8]| {
            let p = dir.path().join(sub).join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, bytes).unwrap();
            p
        };
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let a = file("1", "Vertrag.pdf", &big);
        let b = file("2", "Vertrag.pdf", &big);
        let mut changed = big.clone();
        *changed.last_mut().unwrap() ^= 1;
        let c = file("3", "Vertrag.pdf", &changed);
        let d = file("4", "Angebot.pdf", &big);
        assert!(same_copy(&a, &b), "gleicher Name und Inhalt");
        assert!(!same_copy(&a, &a), "derselbe Pfad ist kein Duplikat einer Kopie (das prüft der Reducer)");
        assert!(!same_copy(&a, &c), "letztes Byte anders");
        assert!(!same_copy(&a, &d), "anderer Name");
        assert!(!same_copy(&a, &dir.path().join("5/Vertrag.pdf")), "fehlt");
    }
}
