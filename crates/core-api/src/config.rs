//! Einstellungen im Kern: Laden (inkl. `-c`, Altdateien, Migration-Angebot), Richtlinie, Secrets,
//! effektive Werte, Reducer-Kontext und Validierung der Einstellungsansicht
//! (Plan §7.1–§7.4; docs/analyse/05 §1.4; ADR 0050).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdfover_backend_api::BackendId;
use pdfover_config::{
    ConfigError, ConfigSource, Effective, LoadOutcome, MemorySecretStore, Overlay, Paths, PlaceholderKind, Policy,
    ProxyMode, SecretStore, Settings, SignatureProfile, load_config_source_with_secrets, resolve_config_file,
};
use pdfover_i18n::Lang;
use pdfover_workflow::Context;

/// Version des Datenschutzhinweises; eine neue Textversion verlangt eine neue Bestätigung.
pub const PRIVACY_TEXT_VERSION: u32 = 1;

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
                    if offer_migration {
                        migration_offer = pdfover_config::import_offer(&paths, legacy_dir.as_deref());
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
        let state = ConfigState { paths, target, settings, overlay, policy, secrets, session_secrets, migration_offer };
        Ok((state, problem))
    }

    pub(crate) fn effective(&self) -> Effective<'_> {
        Effective::new(&self.settings, &self.overlay, self.policy.as_ref())
    }

    /// Schreibgeschützt (Altdatei per `-c`, unlesbare Einstellungen)?
    pub(crate) fn read_only(&self) -> bool {
        self.target.is_none()
    }

    /// Speichert (nur wenn beschreibbar). Overlay-Werte erreichen nie die Datei.
    pub(crate) fn save(&self) -> Result<(), ConfigError> {
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

    /// Bestätigte Datenschutzhinweise (aktuelle Textversion).
    pub(crate) fn privacy_acknowledged(&self) -> Vec<BackendId> {
        [BackendId::AtrustNative, BackendId::AtrustBrowser, BackendId::Csc, BackendId::Wallet]
            .into_iter()
            .filter(|id| {
                self.settings.privacy.acknowledged.get(id.as_str()).is_some_and(|v| *v >= PRIVACY_TEXT_VERSION)
            })
            .collect()
    }

    /// Bestätigung speichern.
    pub(crate) fn acknowledge_privacy(&mut self, id: BackendId) {
        self.settings.privacy.acknowledged.insert(id.as_str().to_owned(), PRIVACY_TEXT_VERSION);
        if let Err(e) = self.save() {
            tracing::warn!(id = e.message_id(), "Bestätigung des Datenschutzhinweises nicht gespeichert");
        }
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
            privacy_acknowledged: self.privacy_acknowledged(),
            output_postfix: self.settings.output.postfix.clone(),
        }
    }

    /// Erlaubte Backends laut Richtlinie (`None` = alle).
    pub(crate) fn allowed_backends(&self) -> Option<Vec<BackendId>> {
        let allowed = self.policy.as_ref()?.allowed_backends.as_ref()?;
        Some(allowed.iter().map(|c| pdfover_workflow::backend_for_choice(*c)).collect())
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
/// abgelehnt; sonst nur Hinweis (wie „Ignorieren“ in PDF-Over, F-102).
pub(crate) fn validate(settings: &Settings) -> (BTreeMap<String, String>, bool) {
    let mut problems = BTreeMap::new();
    let mut blocking = false;
    let postfix = &settings.output.postfix;
    if postfix.contains(['/', '\\']) || postfix.contains('\0') {
        problems.insert("output.postfix".into(), "ng-settings-invalid-postfix".into());
        blocking = true;
    }
    if settings.proxy.mode == ProxyMode::Manual && settings.proxy.host.as_deref().is_none_or(|h| h.trim().is_empty()) {
        problems.insert("proxy.host".into(), "ng-settings-proxy-host-missing".into());
        blocking = true;
    }
    if let Some(e) = &settings.signature.emblem
        && !e.is_file()
    {
        problems.insert("signature.emblem".into(), "exception-PathNotExist".into());
    }
    if settings.keystore.enabled && settings.keystore.file.as_deref().is_none_or(|f| !f.is_file()) {
        problems.insert("keystore.file".into(), "error-KeyStoreFileNotExist".into());
    }
    let size = settings.signature.logo_only_size_mm;
    if !(size.is_finite() && size > 0.0 && size <= pdfover_sigblock::profile::MAX_LOGO_SIZE_MM) {
        problems.insert("signature.logo_only_size_mm".into(), "ng-settings-invalid-logo-size".into());
    }
    if let Some(dir) = &settings.output.folder
        && dir.is_absolute()
        && !dir.exists()
        && dir.parent().is_some_and(|p| !p.exists())
    {
        problems.insert("output.folder".into(), "exception-PathNotExist".into());
    }
    (problems, blocking)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn context_follows_settings_overlay_and_privacy() {
        let tmp = tempfile::tempdir().unwrap();
        let overlay = Overlay { auto_position: true, invisible: Some(true), ..Overlay::default() };
        let (mut s, _) = state(tmp.path(), overlay);
        s.settings.placeholder.search = true;
        s.settings.placeholder.kind = PlaceholderKind::Fields;
        s.settings.output.postfix = "_x".into();
        let ctx = s.context(true);
        assert!(ctx.auto_position && ctx.invisible && ctx.placeholder_search && ctx.placeholder_fields && ctx.headless);
        assert_eq!(ctx.output_postfix, "_x");
        assert!(ctx.privacy_acknowledged.is_empty());
        s.acknowledge_privacy(BackendId::AtrustNative);
        assert_eq!(s.context(false).privacy_acknowledged, vec![BackendId::AtrustNative]);
        // gespeichert
        let saved = Settings::from_toml(&std::fs::read_to_string(s.paths.settings_file()).unwrap()).unwrap();
        assert_eq!(saved.privacy.acknowledged["atrust_native"], PRIVACY_TEXT_VERSION);
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
