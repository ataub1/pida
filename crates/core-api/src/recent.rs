//! „Zuletzt signiert“ im Kern (ADR 0004 §3.10, Paket K3).
//!
//! - Nach jedem erfolgreichen Speichern (Einzeldokument und Stapel) ein Eintrag in
//!   [`pida_config::RecentStore`] – **nur** Dateiname, Pfad und Zeitpunkt, höchstens zehn, neueste zuerst.
//! - Nur mit Oberfläche: ohne Oberfläche (CLI) wird nichts gelesen oder geschrieben, `ViewState::recent`
//!   bleibt `None`.
//! - Abgeschaltet (`general.recent_documents = false` bzw. Richtlinie `recent_documents = false`) löscht der
//!   Kern die Liste **sofort** samt Datei und schreibt nichts mehr; `ViewState::recent = None`.
//! - `exists` prüft der Kern beim Projizieren mit Cache (höchstens alle [`EXISTS_TTL`] bzw. nach jeder
//!   Änderung der Liste).
//! - `VerifyRecent` auf eine fehlende Datei → Hinweis `ng-recent-missing`, der Eintrag bleibt (Entfernen ist
//!   eine ausdrückliche Aktion). Den Prüfmodus selbst öffnet K4.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use pida_config::{Paths, RecentEntry, RecentStore};

use crate::runtime::CoreTask;
use crate::view::RecentItemView;

/// Meldungs-ID: Datei eines Eintrags nicht mehr vorhanden.
pub(crate) const MSG_RECENT_MISSING: &str = "ng-recent-missing";

/// Gültigkeit der Existenzprüfung.
pub(crate) const EXISTS_TTL: Duration = Duration::from_secs(2);

/// Liste und Existenz-Cache.
#[derive(Debug)]
pub(crate) struct RecentState {
    store: RecentStore,
    entries: Vec<RecentEntry>,
    exists: HashMap<u64, bool>,
    checked: Option<Instant>,
}

impl RecentState {
    pub(crate) fn new(paths: &Paths) -> RecentState {
        RecentState { store: RecentStore::new(paths), entries: Vec::new(), exists: HashMap::new(), checked: None }
    }

    /// Existenz neu prüfen.
    fn check(&mut self) {
        self.exists = self.entries.iter().map(|e| (e.id, e.path.is_file())).collect();
        self.checked = Some(Instant::now());
    }

    fn reload(&mut self) {
        self.entries = self.store.load();
        self.check();
    }
}

impl CoreTask {
    /// „Zuletzt signiert“ aktiv? Nur mit Oberfläche; Richtlinie vor Einstellung.
    pub(crate) fn recent_enabled(&self) -> bool {
        !self.headless && self.cfg.effective().recent_documents()
    }

    /// Beim Start und nach jeder Änderung der Einstellungen bzw. Richtlinie: abgeschaltet → Liste und Datei
    /// sofort löschen; eingeschaltet → laden.
    pub(crate) fn sync_recent(&mut self) {
        if self.headless {
            return;
        }
        if self.recent_enabled() {
            self.recent.reload();
            return;
        }
        self.recent.entries.clear();
        self.recent.exists.clear();
        if let Err(e) = self.recent.store.clear() {
            tracing::warn!(id = e.message_id(), "„Zuletzt signiert“ nicht gelöscht");
        }
    }

    /// Nach erfolgreichem Speichern: Eintrag (nur Dateiname, Pfad, Zeitpunkt).
    pub(crate) fn record_recent(&mut self, path: &Path) {
        if !self.recent_enabled() {
            return;
        }
        let entry = RecentEntry {
            id: 0,
            file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.to_path_buf(),
            signed_at: crate::workspace::rfc3339_now(),
        };
        match self.recent.store.push(entry) {
            Ok(_) => self.recent.reload(),
            Err(e) => tracing::warn!(id = e.message_id(), "„Zuletzt signiert“ nicht gespeichert"),
        }
    }

    /// Eintrag entfernen (`RemoveRecent`).
    pub(crate) fn remove_recent(&mut self, id: u64) {
        if !self.recent_enabled() {
            return;
        }
        if let Err(e) = self.recent.store.remove(id) {
            tracing::warn!(id = e.message_id(), "Eintrag nicht entfernt");
        }
        self.recent.reload();
    }

    /// Liste leeren (`ClearRecent`): löscht die Datei.
    pub(crate) fn clear_recent(&mut self) {
        if let Err(e) = self.recent.store.clear() {
            tracing::warn!(id = e.message_id(), "„Zuletzt signiert“ nicht gelöscht");
        }
        self.recent.entries.clear();
        self.recent.exists.clear();
    }

    /// Eintrag prüfen (`VerifyRecent`): fehlt die Datei → Hinweis, der Eintrag bleibt. Den Prüfmodus
    /// öffnet K4 (ADR 0004 §3.9); bis dahin ohne weitere Wirkung.
    pub(crate) fn verify_recent(&mut self, id: u64) {
        let Some(entry) = self.recent.entries.iter().find(|e| e.id == id).cloned() else {
            tracing::debug!(id, "verify_recent: unbekannter Eintrag");
            return;
        };
        let exists = entry.path.is_file();
        self.recent.exists.insert(id, exists);
        if !exists {
            self.notify_code(MSG_RECENT_MISSING);
            return;
        }
        tracing::debug!(id, "verify_recent: Prüfmodus folgt mit K4");
    }

    /// Existenz der Dateien höchstens alle [`EXISTS_TTL`] neu prüfen (`force` = sofort).
    pub(crate) fn refresh_recent_exists(&mut self, force: bool) {
        if self.recent.entries.is_empty() {
            return;
        }
        let stale = self.recent.checked.is_none_or(|t| t.elapsed() >= EXISTS_TTL);
        if force || stale {
            self.recent.check();
        }
    }

    /// `ViewState::recent` (`None` = abgeschaltet bzw. ohne Oberfläche).
    pub(crate) fn recent_view(&self) -> Option<Vec<RecentItemView>> {
        if !self.recent_enabled() {
            return None;
        }
        let mobile = !self.mobile.platform.is_desktop();
        Some(
            self.recent
                .entries
                .iter()
                .map(|e| RecentItemView {
                    id: e.id,
                    file_name: e.file_name.clone(),
                    folder_display: if mobile {
                        None
                    } else {
                        e.path.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.display().to_string())
                    },
                    signed_at: e.signed_at.clone(),
                    exists: self.recent.exists.get(&e.id).copied().unwrap_or(true),
                })
                .collect(),
        )
    }
}
