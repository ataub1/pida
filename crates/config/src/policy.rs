//! Verwaltete Richtlinien (nur lesend, Plan §7.1): Windows Registry/`%ProgramData%`, macOS Managed
//! Preferences, Linux `/etc/pdf-over-ng/policy.toml`, iOS/Android Managed App Configuration
//! (dort liefert die Shell die Werte als TOML/JSON).

use std::path::Path;

use crate::Result;
use crate::settings::{BackendChoice, ProxySettings, SignatureProfile};

/// Richtlinie.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Policy {
    /// Erlaubte Backends (`None` = alle).
    pub allowed_backends: Option<Vec<BackendChoice>>,
    /// Vorgegebenes Backend.
    pub forced_backend: Option<BackendChoice>,
    /// Update-Check erzwingen/verbieten.
    pub update_check: Option<bool>,
    /// Proxy vorgeben.
    pub proxy: Option<ProxySettings>,
    /// Profil vorgeben.
    pub profile: Option<SignatureProfile>,
    /// Amtssignatur-Profil ausblenden.
    pub hide_amtssignatur: bool,
    /// Eigener Text des Datenschutzhinweises.
    pub privacy_notice_text: Option<String>,
}

impl Policy {
    /// Lädt die Systemrichtlinie der Plattform, falls vorhanden.
    pub fn load_system() -> Result<Option<Policy>> {
        todo!("Paket E")
    }

    /// Aus TOML (Linux-Datei, Tests, Mobile-Übergabe).
    pub fn from_toml(text: &str) -> Result<Policy> {
        todo!("Paket E")
    }

    /// Aus einer Datei.
    pub fn load_file(path: &Path) -> Result<Policy> {
        todo!("Paket E")
    }
}
