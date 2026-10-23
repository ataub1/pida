//! `pida-config` – Konfiguration, Migration, Richtlinien, Pfade, Secrets (Plan §7.1–§7.4).
//!
//! - [`Settings`]: neues, versioniertes Schema (TOML, `schema_version`) am plattformüblichen Ort
//!   ([`Paths`]); unbekannte Keys bleiben beim Speichern erhalten (auch in Untertabellen).
//! - [`legacy`]: `~/.pdf-over/PDF-Over.config` mit bit-kompatiblem Java-Properties-Parser
//!   (`pida-java-properties`) und **exakter Lese-Semantik aller 31 Keys** (docs/analyse/05 §1.5),
//!   geprüft gegen ein Java-Orakel.
//! - [`mod@migrate`]: Import-Assistent (docs/analyse/05 §1.9, Plan §7.2) – Altdatei bleibt unverändert;
//!   [`migrate::apply_migration`] schreibt nur neue Dateien und lässt sich rückgängig machen.
//! - [`Overlay`]: CLI-Overlays, die nie persistiert werden, mit den Vorrangregeln aus
//!   docs/analyse/05 §1.4 ([`Effective`]); [`source`]: `-c <name>` inklusive Altdateien.
//! - [`Policy`]: verwaltete Richtlinien (nur lesend).
//! - [`secrets`]: OS-Schlüsselbund bzw. RAM, auf Mobile der sichere Speicher der Shell
//!   ([`ShellSecretStore`] über [`SecureStorage`]); das ID-Austria-Passwort hat **keinen** [`SecretId`]
//!   und kann damit strukturell nicht persistiert werden (Vorgabe des Auftraggebers).
//! - [`CertificateCache`]: Opt-in-Cache öffentlicher Signaturzertifikate.
//! - [`RecentStore`]: „Zuletzt signiert“ (nur Dateiname, Pfad, Zeitpunkt; ADR 0004 §3.10).
//!
//! Meldungen für UI/CLI tragen stabile i18n-IDs (`ng-config-*`, `ng-migrate-*`) aus
//! `pida-i18n/locales/*/pida.ftl`.
//!
//! Ownership: Paket **E**.

pub mod certcache;
mod java;
pub mod legacy;
pub mod migrate;
pub mod overlay;
pub mod paths;
pub mod policy;
pub mod recent;
pub mod secrets;
pub mod settings;
pub mod source;

pub use certcache::{CertCacheKey, CertificateCache};
pub use legacy::{LegacyConfig, LegacyKey, LegacyNote, LegacyValues, LegacyWarning, normalize_mobile_number};
pub use migrate::{
    AppliedMigration, ApplyOptions, MigrationContext, MigrationReport, MigrationResult, ReportMessage, apply_migration,
    find_legacy_config, import_offer, is_pdfas_default_logo, migrate, undo_migration,
};
pub use overlay::{Effective, Overlay};
pub use paths::{HOME_ENV, PORTABLE_DIR, Paths, portable_root, portable_root_in};
pub use policy::Policy;
pub use recent::{MAX_RECENT, RecentEntry, RecentStore};
pub use secrets::{
    KeyringSecretStore, MemorySecretStore, MobileSecretStorePlaceholder, SecretId, SecretStore, SecureStorage,
    ShellSecretStore, platform_secret_store,
};
pub use settings::{
    BackendChoice, BackendSettings, CertificateCacheSettings, CmsProfileChoice, GeneralSettings, IdAustriaSettings,
    KeystoreSettings, LoadOutcome, NoteSetting, OutputSettings, PasswordStorage, PlaceholderKind, PlaceholderSettings,
    ProxyMode, ProxySettings, RETIRED_KEYS, SCHEMA_VERSION, Settings, SignatureProfile, SignatureSettings, WindowSize,
};
pub use source::{
    ConfigSource, SessionSecrets, load_config_source, load_config_source_with_secrets, resolve_config_file,
};

use pida_i18n::{ArgValue, Catalog, Lang};

/// Fehler der Konfiguration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// E/A
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
    /// TOML nicht lesbar.
    #[error("Einstellungen nicht lesbar: {0}")]
    Parse(String),
    /// TOML nicht schreibbar.
    #[error("Einstellungen nicht schreibbar: {0}")]
    Serialize(String),
    /// Schema-Version unbekannt (neuer als diese Version).
    #[error("Schema-Version {found} wird nicht unterstützt (maximal {supported})")]
    UnsupportedSchema {
        /// gefunden
        found: u32,
        /// unterstützt
        supported: u32,
    },
    /// Altdatei nicht lesbar.
    #[error("PDF-Over.config nicht lesbar: {0}")]
    Legacy(String),
    /// Plattformpfade nicht ermittelbar.
    #[error("Konfigurationsordner nicht ermittelbar")]
    NoHomeDirectory,
    /// Schlüsselbund-Fehler.
    #[error("Schlüsselbund: {0}")]
    Secret(String),
}

/// Nutzlast von [`ConfigError::missing_file`]: eine ausdrücklich angegebene Konfigurationsdatei fehlt.
#[derive(Debug)]
struct MissingFile(std::path::PathBuf);

impl std::fmt::Display for MissingFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} nicht gefunden", self.0.display())
    }
}

impl std::error::Error for MissingFile {}

impl ConfigError {
    /// Fehler „Konfigurationsdatei fehlt“ (CLI `-c`, Meldung `ng-config-file-missing` statt des
    /// Werksreset-Dialogs von PDF-Over); [`std::io::ErrorKind::NotFound`] mit dem Pfad als Nutzlast.
    pub fn missing_file(path: &std::path::Path) -> ConfigError {
        ConfigError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, MissingFile(path.to_path_buf())))
    }

    /// Pfad, wenn der Fehler aus [`ConfigError::missing_file`] stammt.
    pub fn missing_path(&self) -> Option<&std::path::Path> {
        match self {
            ConfigError::Io(e) => e.get_ref()?.downcast_ref::<MissingFile>().map(|m| m.0.as_path()),
            _ => None,
        }
    }

    /// Stabile Meldungs-ID im i18n-Katalog (`ng-config-error-*`, fehlende Konfigurationsdatei:
    /// `ng-config-file-missing`; andere E/A-Fehler, auch ein fehlender Ordner: `ng-config-error-io`).
    pub fn message_id(&self) -> &'static str {
        match self {
            ConfigError::Io(_) if self.missing_path().is_some() => "ng-config-file-missing",
            ConfigError::Io(_) => "ng-config-error-io",
            ConfigError::Parse(_) => "ng-config-error-parse",
            ConfigError::Serialize(_) => "ng-config-error-serialize",
            ConfigError::UnsupportedSchema { .. } => "ng-config-error-unsupported-schema",
            ConfigError::Legacy(_) => "ng-config-error-legacy",
            ConfigError::NoHomeDirectory => "ng-config-error-no-home",
            ConfigError::Secret(_) => "ng-config-error-secret",
        }
    }

    /// Argumente zur Meldungs-ID.
    pub fn message_args(&self) -> Vec<(&'static str, ArgValue)> {
        if let Some(path) = self.missing_path() {
            return vec![("path", ArgValue::Str(path.display().to_string()))];
        }
        match self {
            ConfigError::Io(e) => vec![("detail", ArgValue::Str(e.to_string()))],
            ConfigError::Parse(d) | ConfigError::Serialize(d) | ConfigError::Legacy(d) | ConfigError::Secret(d) => {
                vec![("detail", ArgValue::Str(d.clone()))]
            }
            ConfigError::UnsupportedSchema { found, supported } => {
                vec![("found", ArgValue::from(*found)), ("supported", ArgValue::from(*supported))]
            }
            ConfigError::NoHomeDirectory => Vec::new(),
        }
    }

    /// Übersetzte Meldung.
    pub fn localized(&self, lang: Lang) -> String {
        Catalog::global().format(lang, self.message_id(), &self.message_args()).into_owned()
    }
}

/// Ergebnis-Alias.
pub type Result<T, E = ConfigError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_translated() {
        let errors = [
            ConfigError::Io(std::io::Error::other("kaputt")),
            ConfigError::missing_file(std::path::Path::new("/a/b")),
            ConfigError::Parse("x".into()),
            ConfigError::Serialize("x".into()),
            ConfigError::UnsupportedSchema { found: 7, supported: 1 },
            ConfigError::Legacy("x".into()),
            ConfigError::NoHomeDirectory,
            ConfigError::Secret("x".into()),
        ];
        for e in &errors {
            for lang in Lang::ALL {
                let text = e.localized(lang);
                assert!(!text.starts_with('!'), "{} fehlt im Katalog", e.message_id());
                assert!(!text.contains("{$"), "{text}");
            }
        }
        assert_eq!(
            errors[4].localized(Lang::De),
            "Die Einstellungen stammen von einer neueren Version (Schema 7, unterstützt bis 1)."
        );
        assert_eq!(errors[1].localized(Lang::En), "The configuration file “/a/b” was not found.");
        assert_eq!(errors[1].missing_path(), Some(std::path::Path::new("/a/b")));
        assert_eq!(errors[1].to_string(), "E/A-Fehler: /a/b nicht gefunden");
    }

    #[test]
    fn os_not_found_is_not_a_missing_config_file() {
        // z. B. LegacyConfig::load auf einen gelöschten Pfad: kein „Konfigurationsdatei … fehlt“ mit
        // dem OS-Fehlertext als Pfad, sondern ein E/A-Fehler mit Details.
        let err = ConfigError::Io(std::fs::read("/nicht/vorhanden/pida-test").unwrap_err());
        assert_eq!(err.message_id(), "ng-config-error-io");
        assert_eq!(err.missing_path(), None);
        assert!(!err.localized(Lang::De).contains("Konfigurationsdatei"));
        assert_eq!(ConfigError::Parse("x".into()).missing_path(), None);
    }
}
