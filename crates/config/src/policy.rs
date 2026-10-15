//! Verwaltete Richtlinien (nur lesend, Plan §7.1): Windows Registry/`%ProgramData%`, macOS Managed
//! Preferences, Linux `/etc/pida/policy.toml`, iOS/Android Managed App Configuration
//! (dort liefert die Shell die Werte als TOML/JSON).
//!
//! Dateibasiert liest [`Policy::load_system`]:
//! - Windows: `%ProgramData%\PIDA\policy.toml` (Registry `HKLM\Software\Policies\…` liest die
//!   Shell und übergibt sie über [`Policy::from_toml`]/[`Policy::from_json`]),
//! - macOS: `/Library/Application Support/at.pida/policy.toml` (Managed Preferences aus
//!   Configuration Profiles liefert die Shell),
//! - Linux/BSD: `/etc/pida/policy.toml`,
//! - iOS/Android: keine Datei (Managed App Configuration über die Shell).
//!
//! Die Richtlinie gewinnt immer ([`crate::Effective`]); unbekannte Keys werden ignoriert. Bewusst gibt
//! es **keine eigene** Umgebungsvariable zum Umleiten des Pfads (sonst könnte jede Nutzerin die
//! Richtlinie aushebeln). Einschränkung unter Windows: der Ordner `%ProgramData%` stammt aus der
//! Prozessumgebung, die die Nutzerin für ihre eigenen Prozesse setzen kann; manipulationsfest ist dort
//! nur der Registry-Zweig `HKLM\Software\Policies\…`, den die Shell liest und per
//! [`Policy::from_toml`]/[`Policy::from_json`] übergibt (die Shell gibt ihm Vorrang vor der Datei).

use std::path::{Path, PathBuf};

use crate::settings::{BackendChoice, ProxySettings, SignatureProfile};
use crate::{ConfigError, Result};

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
    /// Eigener Datenschutztext der Organisation; ersetzt den Text aus PDF-Over in Einstellungen ›
    /// Datenschutz und in den „Details“ des Kurzhinweises (ADR 0003). Eine Zustimmung erzwingt er nicht.
    pub privacy_notice_text: Option<String>,
    /// „Zuletzt signiert“ erzwingen bzw. verbieten (ADR 0004 §3.10, wie `update_check`): `Some(false)`
    /// schaltet die Liste ab (dann wird auch keine Datei geschrieben), der Schalter erscheint gesperrt.
    /// Wirksam ab K3.
    pub recent_documents: Option<bool>,
}

impl Policy {
    /// Pfad der dateibasierten Systemrichtlinie dieser Plattform (`None` auf Mobile; Windows:
    /// `%ProgramData%`, sonst `C:\ProgramData`, siehe Einschränkung oben).
    pub fn system_path() -> Option<PathBuf> {
        if cfg!(any(target_os = "ios", target_os = "android")) {
            None
        } else if cfg!(target_os = "windows") {
            let base =
                std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
            Some(base.join(crate::paths::APP_NAME).join("policy.toml"))
        } else if cfg!(target_os = "macos") {
            Some(PathBuf::from("/Library/Application Support").join(crate::paths::MACOS_APP_DIR).join("policy.toml"))
        } else {
            Some(PathBuf::from("/etc").join(crate::paths::UNIX_APP_DIR).join("policy.toml"))
        }
    }

    /// Lädt die Systemrichtlinie der Plattform, falls vorhanden.
    pub fn load_system() -> Result<Option<Policy>> {
        match Policy::system_path() {
            Some(p) if p.is_file() => Policy::load_file(&p).map(Some),
            _ => Ok(None),
        }
    }

    /// Aus TOML (Linux-Datei, Tests, Mobile-Übergabe).
    pub fn from_toml(text: &str) -> Result<Policy> {
        toml::from_str(text).map_err(|e| ConfigError::Parse(format!("Richtlinie: {}", e.message())))
    }

    /// Aus JSON (Managed App Configuration der Mobile-Shell).
    pub fn from_json(text: &str) -> Result<Policy> {
        serde_json::from_str(text).map_err(|e| ConfigError::Parse(format!("Richtlinie: {e}")))
    }

    /// Aus einer Datei (`.json` als JSON, sonst TOML).
    pub fn load_file(path: &Path) -> Result<Policy> {
        let text = std::fs::read_to_string(path)?;
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            Policy::from_json(&text)
        } else {
            Policy::from_toml(&text)
        }
    }

    /// `true`, wenn die Richtlinie das Backend zulässt (ein vorgegebenes Backend ist immer zulässig).
    pub fn allows(&self, backend: BackendChoice) -> bool {
        self.forced_backend == Some(backend)
            || self.allowed_backends.as_ref().is_none_or(|list| list.contains(&backend))
    }

    /// `true`, wenn die Richtlinie das Profil zulässt.
    pub fn allows_profile(&self, profile: SignatureProfile) -> bool {
        match self.profile {
            Some(p) => p == profile,
            None => !(self.hide_amtssignatur && profile == SignatureProfile::Amtssignaturblock),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ProxyMode;

    #[test]
    fn toml_and_json() {
        let p = Policy::from_toml(
            r#"
            allowed_backends = ["keystore", "local_bku"]
            update_check = false
            hide_amtssignatur = true
            future_key = "ignoriert"
            [proxy]
            mode = "manual"
            host = "proxy.example"
            port = 3128
            "#,
        )
        .unwrap();
        assert_eq!(p.allowed_backends, Some(vec![BackendChoice::Keystore, BackendChoice::LocalBku]));
        assert_eq!(p.update_check, Some(false));
        assert_eq!(p.proxy.as_ref().map(|x| x.mode), Some(ProxyMode::Manual));
        assert!(!p.allows(BackendChoice::IdAustria));
        assert!(p.allows(BackendChoice::Keystore));
        assert!(!p.allows_profile(SignatureProfile::Amtssignaturblock));
        assert!(p.allows_profile(SignatureProfile::Invisible));
        let j = Policy::from_json(r#"{"forced_backend":"id_austria","profile":"INVISIBLE"}"#).unwrap();
        assert!(j.allows(BackendChoice::IdAustria));
        assert!(!j.allows_profile(SignatureProfile::SignaturblockSmall));
        assert!(Policy::from_toml("allowed_backends = 3").is_err());
        assert!(Policy::from_json("{").is_err());
    }

    #[test]
    fn files() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("policy.toml");
        std::fs::write(&t, "update_check = true\n").unwrap();
        assert_eq!(Policy::load_file(&t).unwrap().update_check, Some(true));
        let j = dir.path().join("policy.JSON");
        std::fs::write(&j, r#"{"privacy_notice_text":"Hinweis"}"#).unwrap();
        assert_eq!(Policy::load_file(&j).unwrap().privacy_notice_text.as_deref(), Some("Hinweis"));
        assert!(Policy::load_file(&dir.path().join("fehlt.toml")).is_err());
        // Die Systemrichtlinie existiert auf Entwicklungsrechnern normalerweise nicht.
        if let Some(path) = Policy::system_path() {
            assert!(path.ends_with("policy.toml"));
        }
        let _ = Policy::load_system();
    }
}
