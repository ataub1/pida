//! `pdfover-config` – Konfiguration, Migration, Richtlinien, Pfade, Secrets (Plan §7.1–§7.4).
//!
//! - [`Settings`]: neues, versioniertes Schema (TOML, `schema_version`) am plattformüblichen Ort
//!   ([`Paths`]); unbekannte Keys bleiben beim Speichern erhalten (auch in Untertabellen).
//! - [`legacy`]: `~/.pdf-over/PDF-Over.config` mit bit-kompatiblem Java-Properties-Parser
//!   (`pdfover-java-properties`) und **exakter Lese-Semantik aller 31 Keys** (docs/analyse/05 §1.5),
//!   geprüft gegen ein Java-Orakel.
//! - [`mod@migrate`]: Import-Assistent (docs/analyse/05 §1.9, Plan §7.2) – Altdatei bleibt unverändert;
//!   [`migrate::apply_migration`] schreibt nur neue Dateien und lässt sich rückgängig machen.
//! - [`Overlay`]: CLI-Overlays, die nie persistiert werden, mit den Vorrangregeln aus
//!   docs/analyse/05 §1.4 ([`Effective`]); [`source`]: `-c <name>` inklusive Altdateien.
//! - [`Policy`]: verwaltete Richtlinien (nur lesend).
//! - [`secrets`]: OS-Schlüsselbund bzw. RAM; das ID-Austria-Passwort hat **keinen** [`SecretId`]
//!   und kann damit strukturell nicht persistiert werden (Vorgabe des Auftraggebers).
//! - [`CertificateCache`]: Opt-in-Cache öffentlicher Signaturzertifikate.
//!
//! Meldungen für UI/CLI tragen stabile i18n-IDs (`ng-config-*`, `ng-migrate-*`) aus
//! `pdfover-i18n/locales/*/pdfover-ng.ftl`.
//!
//! Ownership: Paket **E**.

pub mod certcache;
mod java;
pub mod legacy;
pub mod migrate;
pub mod overlay;
pub mod paths;
pub mod policy;
pub mod secrets;
pub mod settings;
pub mod source;

pub use certcache::{CertCacheKey, CertificateCache};
pub use legacy::{LegacyConfig, LegacyKey, LegacyNote, LegacyValues, LegacyWarning, normalize_mobile_number};
pub use migrate::{
    AppliedMigration, ApplyOptions, MigrationContext, MigrationReport, MigrationResult, ReportMessage, apply_migration,
    migrate, undo_migration,
};
pub use overlay::{Effective, Overlay};
pub use paths::Paths;
pub use policy::Policy;
pub use secrets::{KeyringSecretStore, MemorySecretStore, SecretId, SecretStore, platform_secret_store};
pub use settings::{
    BackendChoice, BackendSettings, CertificateCacheSettings, CmsProfileChoice, GeneralSettings, IdAustriaSettings,
    KeystoreSettings, LoadOutcome, NoteSetting, OutputSettings, PasswordStorage, PlaceholderKind, PlaceholderSettings,
    PrivacySettings, ProxyMode, ProxySettings, SCHEMA_VERSION, Settings, SignatureProfile, SignatureSettings,
    WindowSize,
};
pub use source::{ConfigSource, load_config_source, resolve_config_file};

use pdfover_i18n::{ArgValue, Catalog, Lang};

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

impl ConfigError {
    /// Stabile Meldungs-ID im i18n-Katalog (`ng-config-error-*`, fehlende Datei:
    /// `ng-config-file-missing`).
    pub fn message_id(&self) -> &'static str {
        match self {
            ConfigError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => "ng-config-file-missing",
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
        match self {
            ConfigError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let detail = e.to_string();
                let path = detail.strip_suffix(" nicht gefunden").unwrap_or(&detail).to_owned();
                vec![("path", ArgValue::Str(path))]
            }
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
            ConfigError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "/a/b nicht gefunden")),
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
    }
}
