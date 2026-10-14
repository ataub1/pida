//! `pdfover-config` – Konfiguration, Migration, Richtlinien, Pfade, Secrets (Plan §7.1–§7.4).
//!
//! - [`Settings`]: neues, versioniertes Schema (TOML, `schema_version`) am plattformüblichen Ort
//!   ([`Paths`]); unbekannte Keys bleiben beim Speichern erhalten.
//! - [`legacy`]: `~/.pdf-over/PDF-Over.config` mit bit-kompatiblem Java-Properties-Parser
//!   (`pdfover-java-properties`) und **exakter Lese-Semantik aller 31 Keys** (docs/analyse/05 §1.5).
//! - [`mod@migrate`]: Import-Assistent (docs/analyse/05 §1.9, Plan §7.2) – Altdatei bleibt unverändert.
//! - [`Overlay`]: CLI-Overlays, die nie persistiert werden, mit den Vorrangregeln aus
//!   docs/analyse/05 §1.4 ([`Effective`]).
//! - [`Policy`]: verwaltete Richtlinien (nur lesend).
//! - [`secrets`]: OS-Schlüsselbund bzw. RAM; das ID-Austria-Passwort hat **keinen** [`SecretId`]
//!   und kann damit strukturell nicht persistiert werden (Vorgabe des Auftraggebers).
//!
//! Ownership: Paket **E**.

pub mod legacy;
pub mod migrate;
pub mod overlay;
pub mod paths;
pub mod policy;
pub mod secrets;
pub mod settings;

pub use legacy::{LegacyConfig, LegacyKey, LegacyNote, LegacyValues};
pub use migrate::{MigrationContext, MigrationReport, MigrationResult, migrate};
pub use overlay::{Effective, Overlay};
pub use paths::Paths;
pub use policy::Policy;
pub use secrets::{KeyringSecretStore, MemorySecretStore, SecretId, SecretStore};
pub use settings::{
    BackendChoice, BackendSettings, CertificateCacheSettings, CmsProfileChoice, GeneralSettings, IdAustriaSettings,
    KeystoreSettings, LoadOutcome, NoteSetting, OutputSettings, PasswordStorage, PlaceholderKind, PlaceholderSettings,
    PrivacySettings, ProxyMode, ProxySettings, SCHEMA_VERSION, Settings, SignatureProfile, SignatureSettings,
    WindowSize,
};

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

/// Ergebnis-Alias.
pub type Result<T, E = ConfigError> = std::result::Result<T, E>;
