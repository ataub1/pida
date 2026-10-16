//! Einstellungen im Kern: Laden (inkl. `-c`, Altdateien, Migration-Angebot), Richtlinie, Secrets,
//! effektive Werte, Reducer-Kontext und Validierung der Einstellungsansicht
//! (Plan §7.1–§7.4; docs/analyse/05 §1.4; ADR 0050).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pida_backend_api::BackendId;
use pida_config::{
    ConfigError, ConfigSource, Effective, LoadOutcome, MemorySecretStore, Overlay, Paths, PlaceholderKind, Policy,
    SecretStore, Settings, SignatureProfile, load_config_source_with_secrets, resolve_config_file,
};
use pida_i18n::Lang;
use pida_workflow::Context;

/// Ergebnis des Ladens beim Start.
#[derive(Debug)]
pub(crate) enum StartupProblem {
    /// Einstellungen nicht lesbar (F-05: Zurücksetzen anbieten)
    Unreadable(ConfigError),
}

/// Konfigurationszustand des Kerns. `Debug` ohne Einstellungswerte (Benutzername, Pfade; Regel 9).
pub(crate) struct ConfigState {
    pub paths: Paths,
    /// Datei, in die gespeichert wird (`None` = schreibgeschützt: Altdatei per `-c` bzw. unlesbare
    /// Einstellungen bis zum Zurücksetzen)
    pub target: Option<PathBuf>,
    pub settings: Settings,
    pub overlay: Overlay,
    pub policy: Option<Policy>,
    /// Schlüsselbund bzw. RAM-Speicher der Plattform
    pub secrets: Arc<dyn SecretStore>,
    /// Klartext-Passwörter einer Altdatei (`-c`, nur RAM)
    pub session_secrets: Arc<MemorySecretStore>,
    /// Import-Angebot (`~/.pdf-over/PDF-Over.config`, solange keine `settings.toml` existiert)
    pub migration_offer: Option<PathBuf>,
    /// Altordner `~/.pdf-over` (erneutes Angebot über Einstellungen › Erweitert, ADR 0004 §3.12)
    pub legacy_dir: Option<PathBuf>,
    /// Neuinstallation: beim Start fehlte die Einstellungsdatei (`LoadOutcome::Missing`, ohne `-c`)
    pub fresh_install: bool,
    /// Speichern zurückgestellt: Ersteinrichtung mit offenem Übernahmeangebot (die Datei entsteht erst mit
    /// der Entscheidung, sonst verschwände das Angebot beim nächsten Start; ADR 0004 §3.12)
    pub defer_save: bool,
}

impl std::fmt::Debug for ConfigState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigState")
            .field("read_only", &self.target.is_none())
            .field("overlay", &self.overlay)
            .field("policy", &self.policy.is_some())
            .field("secrets", &self.secrets)
            .field("session_secrets", &self.session_secrets)
            .field("migration_offer", &self.migration_offer.is_some())
            .field("fresh_install", &self.fresh_install)
            .field("defer_save", &self.defer_save)
            .finish_non_exhaustive()
    }
}

impl ConfigState {
    /// Lädt die Einstellungen (Overlay `-c` zuerst, ADR 0050 §11) und die Richtlinie.
    pub(crate) fn load(
        paths: Paths,
        overlay: Overlay,
        policy: Option<Policy>,
        secrets: Arc<dyn SecretStore>,
        legacy_dir: Option<PathBuf>,
        offer_migration: bool,
    ) -> Result<(ConfigState, Option<StartupProblem>), ConfigError> {
        let session_secrets = Arc::new(MemorySecretStore::default());
        let mut problem = None;
        let mut migration_offer = None;
        let mut fresh_install = false;
        let (settings, target) = if let Some(name) = overlay.config_file.clone() {
            let source = resolve_config_file(&paths, legacy_dir.as_deref(), &name)?;
            let (settings, report, plain) = load_config_source_with_secrets(&source, Lang::system())?;
            if let Some(report) = report {
                tracing::info!(
                    imported = report.imported.len(),
                    skipped = report.skipped.len(),
                    "Altdatei per -c gelesen (nur lesen)"
                );
            }
            for (id, value) in &plain {
                session_secrets.set(id, value)?;
            }
            let target = match &source {
                ConfigSource::Settings(p) => Some(p.clone()),
                ConfigSource::Legacy(_) => None,
            };
            (settings, target)
        } else {
            let file = paths.settings_file();
            match Settings::load(&file) {
                LoadOutcome::Loaded(s) => (*s, Some(file)),
                LoadOutcome::Missing => {
                    fresh_install = true;
                    if offer_migration {
                        migration_offer = pida_config::import_offer(&paths, legacy_dir.as_deref());
                    }
                    (Settings::default(), Some(file))
                }
                LoadOutcome::Unreadable(e) => {
                    tracing::warn!(id = e.message_id(), "Einstellungen nicht lesbar");
                    problem = Some(StartupProblem::Unreadable(e));
                    (Settings::default(), None)
                }
            }
        };
        let state = ConfigState {
            paths,
            target,
            settings,
            overlay,
            policy,
            secrets,
            session_secrets,
            migration_offer,
            legacy_dir,
            fresh_install,
            defer_save: false,
        };
        Ok((state, problem))
    }

    pub(crate) fn effective(&self) -> Effective<'_> {
        Effective::new(&self.settings, &self.overlay, self.policy.as_ref())
    }

    /// Schreibgeschützt (Altdatei per `-c`, unlesbare Einstellungen)?
    pub(crate) fn read_only(&self) -> bool {
        self.target.is_none()
    }

    /// Speichert (nur wenn beschreibbar und nicht zurückgestellt, [`Self::defer_save`]). Overlay-Werte
    /// erreichen nie die Datei.
    pub(crate) fn save(&self) -> Result<(), ConfigError> {
        if self.defer_save {
            tracing::debug!("Speichern zurückgestellt (Übernahmeangebot der Ersteinrichtung offen)");
            return Ok(());
        }
        match &self.target {
            Some(path) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                self.settings.save_atomic(path)
            }
            None => Ok(()),
        }
    }

    /// UI-Sprache.
    pub(crate) fn language(&self) -> Lang {
        self.settings.effective_ui_language()
    }

    /// Eigener Datenschutztext der Organisation (Richtlinie), sonst `None` (Text aus PDF-Over).
    pub(crate) fn privacy_text(&self) -> Option<String> {
        self.policy.as_ref().and_then(|p| p.privacy_notice_text.clone()).filter(|t| !t.trim().is_empty())
    }

    /// Reducer-Kontext.
    pub(crate) fn context(&self, headless: bool) -> Context {
        let eff = self.effective();
        Context {
            auto_position: eff.auto_position(),
            invisible: eff.profile() == SignatureProfile::Invisible,
            placeholder_search: self.settings.placeholder.search,
            placeholder_fields: self.settings.placeholder.kind == PlaceholderKind::Fields,
            default_backend: eff.default_backend(),
            skip_finish: eff.skip_finish(),
            headless,
            output_postfix: self.settings.output.postfix.clone(),
            // setzt der Kern-Task (`CoreOptions::workspace_ui`, `batch_overview`, Startargumente)
            workspace: false,
            cli_auto: false,
            batch_overview: false,
        }
    }

    /// Erlaubte Backends laut Richtlinie (`None` = alle).
    pub(crate) fn allowed_backends(&self) -> Option<Vec<BackendId>> {
        let allowed = self.policy.as_ref()?.allowed_backends.as_ref()?;
        Some(allowed.iter().map(|c| pida_workflow::backend_for_choice(*c)).collect())
    }

    /// Ausgabeziel (Overlay vor Einstellung).
    pub(crate) fn output(&self) -> Option<PathBuf> {
        self.effective().output().cloned()
    }

    /// Letzten Ordner merken (nur beschreibbare Einstellungen).
    pub(crate) fn remember_directory(&mut self, dir: &Path) {
        if self.settings.general.last_source_directory.as_deref() == Some(dir) {
            return;
        }
        self.settings.general.last_source_directory = Some(dir.to_path_buf());
        if let Err(e) = self.save() {
            tracing::debug!(id = e.message_id(), "letzter Ordner nicht gespeichert");
        }
    }
}

/// Validierung der Einstellungsansicht: Feld (TOML-Pfad) → i18n-ID. `blocking` = Speichern
/// abgelehnt; sonst nur Hinweis (wie „Ignorieren“ in PDF-Over, F-102). Regeln:
/// [`crate::settings_apply::validate`].
pub(crate) fn validate(settings: &Settings) -> (BTreeMap<String, String>, bool) {
    let list = crate::settings_apply::validate(settings);
    let blocking = list.iter().any(|(_, _, b)| *b);
    (list.into_iter().map(|(p, c, _)| (p, c.to_owned())).collect(), blocking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_config::ProxyMode;

    fn state(dir: &Path, overlay: Overlay) -> (ConfigState, Option<StartupProblem>) {
        ConfigState::load(Paths::under(dir), overlay, None, Arc::new(MemorySecretStore::default()), None, true).unwrap()
    }

    #[test]
    fn missing_settings_are_defaults_and_writable() {
        let tmp = tempfile::tempdir().unwrap();
        let (s, p) = state(tmp.path(), Overlay::default());
        assert!(p.is_none());
        assert!(!s.read_only());
        assert_eq!(s.settings, Settings::default());
        s.save().unwrap();
        assert!(s.paths.settings_file().is_file());
    }

    #[test]
    fn unreadable_settings_are_read_only_until_reset() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::under(tmp.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.settings_file(), "schema_version = 99\n").unwrap();
        let (s, p) = state(tmp.path(), Overlay::default());
        assert!(matches!(p, Some(StartupProblem::Unreadable(_))));
        assert!(s.read_only());
        s.save().unwrap();
        assert_eq!(std::fs::read_to_string(paths.settings_file()).unwrap(), "schema_version = 99\n");
    }

    #[test]
    fn missing_config_file_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let overlay = Overlay { config_file: Some("fehlt.config".into()), ..Overlay::default() };
        let err = ConfigState::load(
            Paths::under(tmp.path()),
            overlay,
            None,
            Arc::new(MemorySecretStore::default()),
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(err.message_id(), "ng-config-file-missing");
    }

    #[test]
    fn context_follows_settings_and_overlay() {
        let tmp = tempfile::tempdir().unwrap();
        let overlay = Overlay { auto_position: true, invisible: Some(true), ..Overlay::default() };
        let (mut s, _) = state(tmp.path(), overlay);
        s.settings.placeholder.search = true;
        s.settings.placeholder.kind = PlaceholderKind::Fields;
        s.settings.output.postfix = "_x".into();
        let ctx = s.context(true);
        assert!(ctx.auto_position && ctx.invisible && ctx.placeholder_search && ctx.placeholder_fields && ctx.headless);
        assert_eq!(ctx.output_postfix, "_x");
        assert!(!s.context(false).headless);
    }

    #[test]
    fn privacy_text_comes_from_the_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut s, _) = state(tmp.path(), Overlay::default());
        assert_eq!(s.privacy_text(), None);
        s.policy = Some(Policy { privacy_notice_text: Some("  ".into()), ..Policy::default() });
        assert_eq!(s.privacy_text(), None, "leerer Text = Text aus PDF-Over");
        s.policy = Some(Policy { privacy_notice_text: Some("Hinweis der Organisation".into()), ..Policy::default() });
        assert_eq!(s.privacy_text().as_deref(), Some("Hinweis der Organisation"));
    }

    #[test]
    fn validation() {
        let mut s = Settings::default();
        assert!(validate(&s).0.is_empty());
        s.output.postfix = "a/b".into();
        s.proxy.mode = ProxyMode::Manual;
        s.signature.logo_only_size_mm = 500.0;
        s.keystore.enabled = true;
        let (p, blocking) = validate(&s);
        assert!(blocking);
        assert_eq!(p["output.postfix"], "ng-settings-invalid-postfix");
        assert_eq!(p["proxy.host"], "ng-settings-proxy-host-missing");
        assert_eq!(p["signature.logo_only_size_mm"], "ng-settings-invalid-logo-size");
        assert_eq!(p["keystore.file"], "error-KeyStoreFileNotExist");
    }
}
