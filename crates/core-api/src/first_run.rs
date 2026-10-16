//! Ersteinrichtung (ADR 0004 §3.12, Paket K4).
//!
//! - **Wann:** nur im Arbeitsbereich mit Oberfläche und nur, wenn beim Start die Einstellungsdatei fehlt
//!   (`LoadOutcome::Missing`, ohne `-c`): dann setzt der Kern `general.first_run_completed = false` bis
//!   „Fertig“ bzw. „Später einrichten“ (`FinishFirstRun`). Bestandsnutzer (Datei ohne Schlüssel → `true`)
//!   sehen sie nie; legt zuerst die CLI die Datei an, gilt sie als erledigt. Wird die App vor „Fertig“
//!   beendet, erscheint sie beim nächsten Start erneut (Datei fehlt noch bzw. trägt `false`).
//! - **Schritt 1** nur mit Übernahmeangebot (`~/.pdf-over/PDF-Over.config`): `MigrationOfferView::items`
//!   aus einem Probelauf von `pida_config::migrate` (ohne zu schreiben). Solange das Angebot offen ist,
//!   schreibt der Kern `settings.toml` nicht (sonst verschwände das Angebot beim nächsten Start, bevor die
//!   Nutzerin entschieden hat); `AcceptMigration`/`DeclineMigration` wirken sofort.
//! - **Neuinstallation** (§8 F4): ohne Übernahme (kein Angebot, abgelehnt, „Später einrichten“) speichert
//!   PIDA „neben dem Original“ (`output.folder = "."`, nur Desktop); mit Übernahme gilt der PDF-Over-Wert.
//!   `OutputSettings::default()` bleibt `None` (Bestandsnutzer behalten den Speichern-Dialog).
//! - **Bildmarke:** übernommen wird nur eine eigene (`pida_config::is_pdfas_default_logo`), sonst bleibt das
//!   PIDA-Siegel (ADR 0005).
//! - **Schritte 2/3:** `UpdateFirstRun` übernimmt Standard-Signiermittel, Profil und Blocksprache sofort (wie
//!   `ApplySettings`, inkl. Richtlinie) und baut die Vorschau neu (`FirstRunView::preview_revision`).
//! - **Erneut:** Einstellungen › Erweitert → `OfferMigration` sucht die Altkonfiguration erneut (auch nach
//!   `DeclineMigration`); ohne Fund Hinweis `ng-migration-not-found`.

use std::collections::BTreeMap;
use std::path::Path;

use pida_config::{BackendChoice, LegacyKey, MigrationResult, ProxyMode, SignatureProfile};
use pida_i18n::Lang;
use pida_workflow::Event;

use crate::runtime::CoreTask;
use crate::view::{FirstRunView, MigrationItemView};

/// Meldungs-ID: Ersteinrichtung abgeschlossen.
pub(crate) const MSG_FIRSTRUN_DONE: &str = "ng-firstrun-done";
/// Meldungs-ID: keine PDF-Over-Einstellungen gefunden (`OfferMigration`).
pub(crate) const MSG_MIGRATION_NOT_FOUND: &str = "ng-migration-not-found";

/// Übernahmeliste aus einem Probelauf (`report.imported` und die übernommenen Werte; Telefonnummer
/// maskiert, Pfade nur als Dateiname bzw. Ordner).
pub(crate) fn migration_items(r: &MigrationResult) -> Vec<MigrationItemView> {
    use LegacyKey::*;
    let imported = |keys: &[LegacyKey]| keys.iter().any(|k| r.report.imported.contains(k));
    let s = &r.settings;
    let file_name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    let mut items = Vec::new();
    let mut push = |code: &str, detail: Option<String>| {
        items.push(MigrationItemView { code: format!("ng-migration-item-{code}"), detail });
    };
    if imported(&[SignatureProfile, SignLocale, SignatureNote, LogoOnlySize, SignaturePdfaCompat]) {
        push("block", None);
    }
    if let Some(emblem) = &s.signature.emblem {
        push("emblem", file_name(emblem));
    }
    if let Some(user) = s.id_austria.username.as_deref().filter(|u| !u.trim().is_empty()) {
        push("username", Some(crate::workspace::mask_account(user)));
    }
    if imported(&[OutputFolder, SaveFilePostfix]) {
        push("output", s.output.folder.as_ref().map(|f| f.display().to_string()));
    }
    if s.keystore.enabled || s.keystore.file.is_some() {
        push("keystore", s.keystore.file.as_deref().and_then(file_name));
    }
    if s.proxy.mode == ProxyMode::Manual {
        let detail = s.proxy.host.as_ref().map(|h| match s.proxy.port {
            Some(p) => format!("{h}:{p}"),
            None => h.clone(),
        });
        push("proxy", detail);
    }
    if s.placeholder.search {
        push("placeholder", None);
    }
    if let Some(lang) = s.general.ui_language {
        // Eigenname der Sprache (unabhängig von der Oberflächensprache verständlich)
        let name = match lang {
            Lang::De => "Deutsch",
            Lang::En => "English",
        };
        push("language", Some(name.to_owned()));
    }
    items
}

impl CoreTask {
    /// Ersteinrichtung aktiv (Arbeitsbereich mit Oberfläche, noch nicht abgeschlossen)?
    pub(crate) fn first_run_active(&self) -> bool {
        self.ws.enabled && !self.headless && !self.cfg.settings.general.first_run_completed
    }

    /// Beim Start: Neuinstallation → Ersteinrichtung; ohne Übernahmeangebot gleich „neben dem Original“.
    pub(crate) fn begin_first_run(&mut self) {
        if !self.ws.enabled || self.headless || !self.cfg.fresh_install {
            return;
        }
        tracing::info!(offer = self.cfg.migration_offer.is_some(), "Neuinstallation – Ersteinrichtung");
        self.cfg.settings.general.first_run_completed = false;
        if self.cfg.migration_offer.is_some() {
            self.cfg.defer_save = true;
        } else {
            self.new_install_output();
        }
        self.refresh_ctx_pub();
    }

    /// Neuinstallation ohne Übernahme: „neben dem Original“ (nur Desktop; Mobile speichert in die
    /// App-Dokumente).
    fn new_install_output(&mut self) {
        if self.mobile.platform.is_desktop() && self.cfg.settings.output.folder.is_none() {
            self.cfg.settings.output.folder = Some(".".into());
        }
    }

    /// Nach `AcceptMigration` bzw. `DeclineMigration`: das Angebot ist entschieden, gespeichert wird wieder.
    /// Abgelehnt in der Ersteinrichtung = Neuinstallation (`output.folder = "."`); übernommen: die
    /// Einrichtung bleibt offen (Schritte 2/3 zeigen die übernommenen Werte).
    pub(crate) fn migration_decided(&mut self, accepted: bool) {
        let first_run = self.first_run_active() || (accepted && self.cfg.defer_save);
        self.cfg.defer_save = false;
        if accepted && first_run {
            self.cfg.settings.general.first_run_completed = false;
        } else if first_run {
            self.new_install_output();
        }
    }

    /// `UpdateFirstRun` (Schritte 2/3): sofort übernehmen und speichern; Vorschau neu.
    pub(crate) fn update_first_run(
        &mut self,
        default_backend: Option<BackendChoice>,
        profile: SignatureProfile,
        block_language: Option<Lang>,
    ) {
        if !self.first_run_active() {
            tracing::debug!("update_first_run ohne Ersteinrichtung verworfen");
            return;
        }
        let mut changes = BTreeMap::new();
        let json = |v: serde_json::Result<serde_json::Value>| v.unwrap_or(serde_json::Value::Null);
        changes.insert("backend.default".to_owned(), json(serde_json::to_value(default_backend)));
        changes.insert("signature.profile".to_owned(), json(serde_json::to_value(profile)));
        changes.insert("signature.block_language".to_owned(), json(serde_json::to_value(block_language)));
        self.apply_changes_now(&changes);
    }

    /// `FinishFirstRun` („Fertig“ bzw. „Später einrichten“): abgeschlossen und gespeichert. Ein noch offenes
    /// Übernahmeangebot gilt als „später“ (nichts übernommen; erneut über Einstellungen › Erweitert).
    pub(crate) fn finish_first_run(&mut self) {
        if !self.first_run_active() {
            tracing::debug!("finish_first_run ohne Ersteinrichtung verworfen");
            return;
        }
        if self.cfg.migration_offer.take().is_some() {
            self.new_install_output();
        }
        self.cfg.defer_save = false;
        self.cfg.settings.general.first_run_completed = true;
        if let Err(e) = self.cfg.save() {
            tracing::warn!(id = e.message_id(), "Einstellungen nicht gespeichert");
            self.notify_code("error-FailedToSaveSettings");
        }
        self.refresh_ctx_pub();
        self.apply(Event::SettingsChanged);
        self.notify_code(MSG_FIRSTRUN_DONE);
    }

    /// `OfferMigration` (Einstellungen › Erweitert): Altkonfiguration erneut suchen und anbieten.
    pub(crate) fn offer_migration(&mut self) {
        if self.cfg.read_only() {
            tracing::debug!("offer_migration: Einstellungen schreibgeschützt");
            return;
        }
        match self.cfg.legacy_dir.as_deref().and_then(pida_config::find_legacy_config) {
            Some(file) => {
                // neu auswerten (die Datei kann sich seit dem letzten Angebot geändert haben)
                self.invalidate_migration_view();
                self.cfg.migration_offer = Some(file);
            }
            None => self.notify_code(MSG_MIGRATION_NOT_FOUND),
        }
    }

    /// `ViewState::first_run`.
    pub(crate) fn first_run_view(&self) -> Option<FirstRunView> {
        if !self.first_run_active() {
            return None;
        }
        let s = &self.cfg.settings;
        Some(FirstRunView {
            migration: self.projected_migration(),
            default_backend: s.backend.default,
            profile: s.signature.profile,
            block_language: s.signature.block_language,
            preview_revision: crate::runtime::lock(&self.shared.block).revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_config::{LegacyConfig, MigrationContext, migrate};

    fn items(config: &str) -> Vec<(String, Option<String>)> {
        let legacy = LegacyConfig::parse(config.as_bytes()).unwrap();
        let ctx = MigrationContext { legacy_dir: std::env::temp_dir(), system_language: Lang::De, check_paths: false };
        migration_items(&migrate(&legacy, &ctx)).into_iter().map(|i| (i.code, i.detail)).collect()
    }

    #[test]
    fn items_follow_the_imported_keys() {
        let all = items(
            "SIGNATURE_PROFILE=SIGNATURBLOCK_SMALL\nSIGNLOCALE=de\nMOBILE_NUMBER=+43 660 1234567\n\
             OUTPUT_FOLDER=/home/test/Signiert\nSAVE_FILE_POSTFIX=_signiert\nKEYSTORE_ENABLED=true\n\
             KEYSTORE_FILE=/home/test/test.p12\nPROXY_HOST=proxy.example.test\nPROXY_PORT=3128\n\
             USE_PLACEHOLDER_SEARCH=true\nUSE_MARKER=true\nLOCALE=en\nEMBLEM=/home/test/Bilder/firma.png\n",
        );
        let code = |c: &str| format!("ng-migration-item-{c}");
        assert_eq!(
            all,
            vec![
                (code("block"), None),
                (code("emblem"), Some("firma.png".into())),
                (code("username"), Some("+43 660 ···567".into())),
                (code("output"), Some("/home/test/Signiert".into())),
                (code("keystore"), Some("test.p12".into())),
                (code("proxy"), Some("proxy.example.test:3128".into())),
                (code("placeholder"), None),
                (code("language"), Some("English".into())),
            ]
        );
        // nur, was in der Altdatei steht; Platzhaltersuche aus = keine Zeile
        assert_eq!(
            items("SIGNATURE_PROFILE=SIGNATURBLOCK_SMALL\nUSE_PLACEHOLDER_SEARCH=false\n"),
            vec![(code("block"), None)]
        );
        assert!(items("UPDATE_CHECK=false\n").is_empty());
        // Standard-Bildmarke von PDF-AS ist keine eigene → keine Zeile „Bildmarke“
        assert!(items("EMBLEM=/home/test/.pdf-over/cfg/images/signatur-logo_de.png\n").is_empty());
    }
}
