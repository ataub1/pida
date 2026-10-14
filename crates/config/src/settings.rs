//! Neues Settings-Schema (TOML, versioniert; Plan §7.1).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pdfover_i18n::Lang;

use crate::Result;

/// Aktuelle Schema-Version.
pub const SCHEMA_VERSION: u32 = 1;

/// Signaturprofil aus Nutzersicht (PDF-Over `Profile`, docs/analyse/05 §1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignatureProfile {
    /// `SIGNATURBLOCK_SMALL` (Default)
    #[default]
    SignaturblockSmall,
    /// `AMTSSIGNATURBLOCK` (nur öffentlicher Bereich, Plan §10.6)
    Amtssignaturblock,
    /// `BASE_LOGO` (nur Bildmarke)
    BaseLogo,
    /// `INVISIBLE`
    Invisible,
}

impl SignatureProfile {
    /// Name wie in `PDF-Over.config` (`SIGNATURE_PROFILE`).
    pub fn legacy_name(self) -> &'static str {
        match self {
            SignatureProfile::SignaturblockSmall => "SIGNATURBLOCK_SMALL",
            SignatureProfile::Amtssignaturblock => "AMTSSIGNATURBLOCK",
            SignatureProfile::BaseLogo => "BASE_LOGO",
            SignatureProfile::Invisible => "INVISIBLE",
        }
    }

    /// Aus dem Altnamen; unbekannt → `None` (PDF-Over: Default).
    pub fn from_legacy_name(name: &str) -> Option<Self> {
        Some(match name {
            "SIGNATURBLOCK_SMALL" => SignatureProfile::SignaturblockSmall,
            "AMTSSIGNATURBLOCK" => SignatureProfile::Amtssignaturblock,
            "BASE_LOGO" => SignatureProfile::BaseLogo,
            "INVISIBLE" => SignatureProfile::Invisible,
            _ => return None,
        })
    }

    /// PDF-AS-Profil-ID wie `PdfAs4SignatureParameter.getPdfAsSignatureProfileId()`
    /// (docs/analyse/04 §2.8, docs/analyse/05 §1.6) – die 12 von PDF-Over erzeugbaren IDs.
    /// `pdfa` ist bereits die effektive Entscheidung (Schalter **oder** erkannte PDF/A-Kennung,
    /// Plan §6.9); bei AMTS/BASE_LOGO/INVISIBLE wirkt es nicht auf die ID.
    pub fn pdfas_profile_id(self, block_language: Lang, with_note: bool, pdfa: bool) -> String {
        match self {
            SignatureProfile::Invisible => "INVISIBLE".into(),
            SignatureProfile::BaseLogo => "BASE_LOGO".into(),
            SignatureProfile::Amtssignaturblock => match block_language {
                Lang::En => "AMTSSIGNATURBLOCK_EN_RECOMMENDED".into(),
                Lang::De => "AMTSSIGNATURBLOCK_DE_RECOMMENDED".into(),
            },
            SignatureProfile::SignaturblockSmall => {
                let mut id = String::from("SIGNATURBLOCK_SMALL");
                id.push_str(if block_language == Lang::En { "_EN" } else { "_DE" });
                if with_note {
                    id.push_str("_NOTE");
                }
                if pdfa {
                    id.push_str("_PDFA");
                }
                id
            }
        }
    }
}

/// Signaturhinweis (`SIGNATURE_NOTE`: „fehlt“ ≠ „leer“, docs/analyse/05 §1.5).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", content = "text", rename_all = "snake_case")]
pub enum NoteSetting {
    /// Profil- und sprachabhängiger Standardtext (`simple_config.Note_Default_*`).
    #[default]
    Default,
    /// Kein Hinweis (Profil ohne `_NOTE`).
    None,
    /// Eigener Text.
    Custom(String),
}

/// Fenstergröße (`MAINWINDOW_SIZE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WindowSize {
    /// Breite
    pub width: u32,
    /// Höhe
    pub height: u32,
}

/// Allgemeines.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// UI-Sprache; `None` = System (Fallback DE).
    pub ui_language: Option<Lang>,
    /// Update-Check beim Start (nur Desktop außerhalb der Stores).
    pub update_check: bool,
    /// Letzte Fenstergröße.
    pub window_size: Option<WindowSize>,
    /// Zuletzt geöffneter Ordner.
    pub last_source_directory: Option<PathBuf>,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self { ui_language: None, update_check: true, window_size: None, last_source_directory: None }
    }
}

/// Signaturblock und Position.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SignatureSettings {
    /// Profil.
    pub profile: SignatureProfile,
    /// Sprache des Signaturblocks (`SIGNLOCALE`); `None` = System.
    pub block_language: Option<Lang>,
    /// Hinweistext.
    pub note: NoteSetting,
    /// PDF/A-Kompatibilität erzwingen (`SIGNATURE_PDFA_COMPAT`); zusätzlich wählt der Kern bei
    /// erkannter PDF/A-Kennung automatisch die `_PDFA`-Variante (Plan §6.9).
    pub pdfa_force: bool,
    /// Automatische Positionierung (`SIGNATURE_POSITION=auto`).
    pub auto_position: bool,
    /// Bildmarke (`EMBLEM`).
    pub emblem: Option<PathBuf>,
    /// Logo-Größe für `BASE_LOGO` in mm (`LOGO_ONLY_SIZE`, Default 23).
    pub logo_only_size_mm: f64,
    /// CMS-Profil für Modell A (Entscheidung G0, Plan §6.10).
    pub cms_profile: CmsProfileChoice,
}

impl Default for SignatureSettings {
    fn default() -> Self {
        Self {
            profile: SignatureProfile::default(),
            block_language: None,
            note: NoteSetting::Default,
            pdfa_force: false,
            auto_position: false,
            emblem: None,
            logo_only_size_mm: 23.0,
            cms_profile: CmsProfileChoice::PdfasCompat,
        }
    }
}

/// CMS-Profil der lokal erzeugten Signaturen (Abbildung auf `pdfover_cades::CmsProfile` im Workflow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmsProfileChoice {
    /// verhaltensgleich zu PDF-AS
    #[default]
    PdfasCompat,
    /// PAdES-BASELINE-B
    PadesBb,
}

/// Platzhalterart (`USE_FIELDS` hat Vorrang vor `USE_MARKER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceholderKind {
    /// Leere Signaturfelder.
    Fields,
    /// QR-Code-Platzhalter.
    #[default]
    Marker,
}

/// Platzhaltersuche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PlaceholderSettings {
    /// `USE_PLACEHOLDER_SEARCH`
    pub search: bool,
    /// Art.
    pub kind: PlaceholderKind,
}

/// Standard-Signiermittel (`BKU`: `MOBILE`/`LOCAL`/`KS`; `NONE` = keines).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendChoice {
    /// ID Austria (Szenario A: nativer A-Trust-Ablauf).
    IdAustria,
    /// Lokale BKU (nur Desktop).
    LocalBku,
    /// PKCS#12-Keystore.
    Keystore,
}

impl BackendChoice {
    /// Aus PDF-Over-`BKU` (case-sensitive, `NONE`/ungültig → `None`).
    pub fn from_legacy(value: &str) -> Option<Self> {
        match value {
            "MOBILE" => Some(BackendChoice::IdAustria),
            "LOCAL" => Some(BackendChoice::LocalBku),
            "KS" => Some(BackendChoice::Keystore),
            _ => None,
        }
    }
}

/// Backend-Auswahl.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BackendSettings {
    /// Standard; `None` = Auswahl anzeigen (bzw. automatisch ID Austria, F-54).
    pub default: Option<BackendChoice>,
}

/// ID Austria (Szenario A). Das Passwort wird **nie** persistiert.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct IdAustriaSettings {
    /// Vorbelegter Benutzername/Telefonnummer (`MOBILE_NUMBER`, normalisiert).
    pub username: Option<String>,
    /// „Merken“ (`MOBILE_REMEMBER_PASSWORD`): Benutzername persistent, Passwort nur im RAM bis Neustart.
    pub remember: bool,
    /// FIDO2 standardmäßig wählen (`FIDO2_DEFAULT`, nur Desktop).
    pub fido2_default: bool,
}

/// Passwort-Speicherart des Keystores (`KEYSTORE_PASS_STORE_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PasswordStorage {
    /// Jedes Mal fragen (`none`).
    None,
    /// Bis Programmende im RAM (`memory`).
    #[default]
    Memory,
    /// OS-Schlüsselbund (`disk` – früher Klartext, Plan §4.9).
    Keychain,
}

/// Keystore.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct KeystoreSettings {
    /// `KEYSTORE_ENABLED`
    pub enabled: bool,
    /// `KEYSTORE_FILE`
    pub file: Option<PathBuf>,
    /// `KEYSTORE_TYPE` wie angegeben (`PKCS12`, `JKS`, `JCEKS`); erkannt wird am Inhalt.
    pub declared_type: Option<String>,
    /// `KEYSTORE_ALIAS`
    pub alias: Option<String>,
    /// Speicherart.
    pub password_storage: PasswordStorage,
}

/// Ausgabe.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    /// `OUTPUT_FOLDER` (Ordner oder, wenn kein Ordner, Dateipfad; relativ zum Quellordner, F-82).
    pub folder: Option<PathBuf>,
    /// `SAVE_FILE_POSTFIX`, Default `_signed`.
    pub postfix: String,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self { folder: None, postfix: "_signed".into() }
    }
}

/// Proxy-Modus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    /// System-Proxy.
    #[default]
    System,
    /// Kein Proxy.
    None,
    /// Manuell (`PROXY_HOST`/`PROXY_PORT`).
    Manual,
}

/// Proxy (Passwort im Schlüsselbund, [`crate::SecretId::ProxyPassword`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProxySettings {
    /// Modus.
    pub mode: ProxyMode,
    /// Host.
    pub host: Option<String>,
    /// Port.
    pub port: Option<u16>,
    /// Benutzer.
    pub user: Option<String>,
}

/// Bestätigte Datenschutzhinweise (`document_leaves_device`, Plan §4.2/§7.8).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    /// Hinweis-ID (z. B. Backend) → bestätigte Textversion.
    pub acknowledged: BTreeMap<String, u32>,
}

/// Zertifikats-Cache (Opt-in, nur öffentliches Zertifikat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CertificateCacheSettings {
    /// Aktiv.
    pub enabled: bool,
}

/// Gesamte Einstellungen.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Schema-Version.
    pub schema_version: u32,
    /// Allgemeines.
    pub general: GeneralSettings,
    /// Signatur.
    pub signature: SignatureSettings,
    /// Platzhalter.
    pub placeholder: PlaceholderSettings,
    /// Backend.
    pub backend: BackendSettings,
    /// ID Austria.
    pub id_austria: IdAustriaSettings,
    /// Keystore.
    pub keystore: KeystoreSettings,
    /// Ausgabe.
    pub output: OutputSettings,
    /// Proxy.
    pub proxy: ProxySettings,
    /// Datenschutz.
    pub privacy: PrivacySettings,
    /// Zertifikats-Cache.
    pub certificate_cache: CertificateCacheSettings,
    /// Unbekannte Tabellen/Keys (bleiben beim Speichern erhalten).
    #[serde(flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: GeneralSettings::default(),
            signature: SignatureSettings::default(),
            placeholder: PlaceholderSettings::default(),
            backend: BackendSettings::default(),
            id_austria: IdAustriaSettings::default(),
            keystore: KeystoreSettings::default(),
            output: OutputSettings::default(),
            proxy: ProxySettings::default(),
            privacy: PrivacySettings::default(),
            certificate_cache: CertificateCacheSettings::default(),
            unknown: BTreeMap::new(),
        }
    }
}

/// Ergebnis von [`Settings::load`].
#[derive(Debug)]
pub enum LoadOutcome {
    /// Gelesen.
    Loaded(Box<Settings>),
    /// Datei fehlt (Erststart → Import-Assistent anbieten).
    Missing,
    /// Datei nicht lesbar → „Einstellungen zurücksetzen“ anbieten (F-05, Plan §7.1).
    Unreadable(crate::ConfigError),
}

impl Settings {
    /// Liest die TOML-Datei.
    pub fn load(path: &Path) -> LoadOutcome {
        todo!("Paket E")
    }

    /// Parst TOML-Text (inkl. Schema-Migration älterer Versionen).
    pub fn from_toml(text: &str) -> Result<Settings> {
        todo!("Paket E")
    }

    /// Serialisiert als TOML.
    pub fn to_toml(&self) -> Result<String> {
        todo!("Paket E")
    }

    /// Schreibt atomar (Temp-Datei, fsync, Umbenennen).
    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        todo!("Paket E")
    }

    /// Effektive Sprache des Signaturblocks.
    pub fn effective_block_language(&self, system: Lang) -> Lang {
        self.signature.block_language.unwrap_or(system)
    }

    /// Hinweistext für das Profil: `None` = kein Hinweis; Default-Text aus dem i18n-Katalog
    /// (`simple_config-Note_Default_Standard` bzw. `…_OfficialSignature`).
    pub fn note_text(&self, block_language: Lang) -> Option<String> {
        todo!("Paket E")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_profile_ids() {
        use SignatureProfile::*;
        assert_eq!(SignaturblockSmall.pdfas_profile_id(Lang::De, true, false), "SIGNATURBLOCK_SMALL_DE_NOTE");
        assert_eq!(SignaturblockSmall.pdfas_profile_id(Lang::En, true, true), "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA");
        assert_eq!(SignaturblockSmall.pdfas_profile_id(Lang::De, false, true), "SIGNATURBLOCK_SMALL_DE_PDFA");
        assert_eq!(Amtssignaturblock.pdfas_profile_id(Lang::En, true, true), "AMTSSIGNATURBLOCK_EN_RECOMMENDED");
        assert_eq!(BaseLogo.pdfas_profile_id(Lang::En, true, true), "BASE_LOGO");
        assert_eq!(Invisible.pdfas_profile_id(Lang::De, false, false), "INVISIBLE");
    }
}
