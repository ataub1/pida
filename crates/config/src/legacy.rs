//! `~/.pdf-over/PDF-Over.config` (Java-Properties) mit exakter Lese-Semantik
//! (docs/analyse/05 §1.3–§1.5; Plan §7.2 Punkt 2). Jede Regel wird als Tabellentest umgesetzt.

use std::path::{Path, PathBuf};

use pdfover_i18n::Lang;
use pdfover_java_properties::{Encoding, Properties};

use crate::Result;
use crate::settings::{BackendChoice, PasswordStorage, SignatureProfile};

/// Alle Keys der Altdatei. 31 werden migriert; `THEME` und `CRASH` werden ignoriert
/// (Kategorie X, docs/analyse/01 §2.1 F-08/F-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(missing_docs)]
pub enum LegacyKey {
    Bku,
    SignaturePosition,
    UseMarker,
    UseFields,
    UsePlaceholderSearch,
    MobileNumber,
    MobileRememberPassword,
    SignatureNote,
    SignLocale,
    SignaturePdfaCompat,
    SignatureProfile,
    Emblem,
    LogoOnlySize,
    Locale,
    UpdateCheck,
    ProxyHost,
    ProxyPort,
    ProxyUser,
    ProxyPass,
    OutputFolder,
    SaveFilePostfix,
    MainwindowSize,
    KeystoreEnabled,
    KeystoreFile,
    KeystoreType,
    KeystoreAlias,
    KeystorePassStoreType,
    KeystoreStorepass,
    KeystoreKeypass,
    Fido2Default,
    LastSourceDirectory,
    Theme,
    Crash,
}

impl LegacyKey {
    /// Alle 33 Keys.
    pub const ALL: [LegacyKey; 33] = [
        LegacyKey::Bku,
        LegacyKey::SignaturePosition,
        LegacyKey::UseMarker,
        LegacyKey::UseFields,
        LegacyKey::UsePlaceholderSearch,
        LegacyKey::MobileNumber,
        LegacyKey::MobileRememberPassword,
        LegacyKey::SignatureNote,
        LegacyKey::SignLocale,
        LegacyKey::SignaturePdfaCompat,
        LegacyKey::SignatureProfile,
        LegacyKey::Emblem,
        LegacyKey::LogoOnlySize,
        LegacyKey::Locale,
        LegacyKey::UpdateCheck,
        LegacyKey::ProxyHost,
        LegacyKey::ProxyPort,
        LegacyKey::ProxyUser,
        LegacyKey::ProxyPass,
        LegacyKey::OutputFolder,
        LegacyKey::SaveFilePostfix,
        LegacyKey::MainwindowSize,
        LegacyKey::KeystoreEnabled,
        LegacyKey::KeystoreFile,
        LegacyKey::KeystoreType,
        LegacyKey::KeystoreAlias,
        LegacyKey::KeystorePassStoreType,
        LegacyKey::KeystoreStorepass,
        LegacyKey::KeystoreKeypass,
        LegacyKey::Fido2Default,
        LegacyKey::LastSourceDirectory,
        LegacyKey::Theme,
        LegacyKey::Crash,
    ];

    /// Name in der Datei.
    pub fn name(self) -> &'static str {
        match self {
            LegacyKey::Bku => "BKU",
            LegacyKey::SignaturePosition => "SIGNATURE_POSITION",
            LegacyKey::UseMarker => "USE_MARKER",
            LegacyKey::UseFields => "USE_FIELDS",
            LegacyKey::UsePlaceholderSearch => "USE_PLACEHOLDER_SEARCH",
            LegacyKey::MobileNumber => "MOBILE_NUMBER",
            LegacyKey::MobileRememberPassword => "MOBILE_REMEMBER_PASSWORD",
            LegacyKey::SignatureNote => "SIGNATURE_NOTE",
            LegacyKey::SignLocale => "SIGNLOCALE",
            LegacyKey::SignaturePdfaCompat => "SIGNATURE_PDFA_COMPAT",
            LegacyKey::SignatureProfile => "SIGNATURE_PROFILE",
            LegacyKey::Emblem => "EMBLEM",
            LegacyKey::LogoOnlySize => "LOGO_ONLY_SIZE",
            LegacyKey::Locale => "LOCALE",
            LegacyKey::UpdateCheck => "UPDATE_CHECK",
            LegacyKey::ProxyHost => "PROXY_HOST",
            LegacyKey::ProxyPort => "PROXY_PORT",
            LegacyKey::ProxyUser => "PROXY_USER",
            LegacyKey::ProxyPass => "PROXY_PASS",
            LegacyKey::OutputFolder => "OUTPUT_FOLDER",
            LegacyKey::SaveFilePostfix => "SAVE_FILE_POSTFIX",
            LegacyKey::MainwindowSize => "MAINWINDOW_SIZE",
            LegacyKey::KeystoreEnabled => "KEYSTORE_ENABLED",
            LegacyKey::KeystoreFile => "KEYSTORE_FILE",
            LegacyKey::KeystoreType => "KEYSTORE_TYPE",
            LegacyKey::KeystoreAlias => "KEYSTORE_ALIAS",
            LegacyKey::KeystorePassStoreType => "KEYSTORE_PASS_STORE_TYPE",
            LegacyKey::KeystoreStorepass => "KEYSTORE_STOREPASS",
            LegacyKey::KeystoreKeypass => "KEYSTORE_KEYPASS",
            LegacyKey::Fido2Default => "FIDO2_DEFAULT",
            LegacyKey::LastSourceDirectory => "LAST_SOURCE_DIRECTORY",
            LegacyKey::Theme => "THEME",
            LegacyKey::Crash => "CRASH",
        }
    }

    /// `true` für die 31 migrierten Keys.
    pub fn is_migrated(self) -> bool {
        !matches!(self, LegacyKey::Theme | LegacyKey::Crash)
    }

    /// `true` für Klartext-Secrets (wandern nach Zustimmung in den Schlüsselbund).
    pub fn is_secret(self) -> bool {
        matches!(self, LegacyKey::KeystoreStorepass | LegacyKey::KeystoreKeypass | LegacyKey::ProxyPass)
    }
}

/// `SIGNATURE_NOTE`: „fehlt“ (Profil-Default) ≠ „leer“ (kein Hinweis).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyNote {
    /// Key fehlt → Standardtext.
    Missing,
    /// Key vorhanden, Wert leer → kein Hinweis (Profil ohne `_NOTE`).
    Empty,
    /// Eigener Text.
    Text(String),
}

/// Warnung beim Lesen (blockiert den Start nicht, Plan §7.2: ungültiger `PROXY_PORT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyWarning {
    /// Key
    pub key: LegacyKey,
    /// Rohwert
    pub raw: String,
    /// Beschreibung
    pub message: String,
}

/// Typisierte Werte nach den Lese-Regeln aus docs/analyse/05 §1.5.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyValues {
    /// `BKU` (ungültig/`NONE` → `None`)
    pub bku: Option<BackendChoice>,
    /// `SIGNATURE_POSITION` (`trim() == "auto"`)
    pub auto_position: bool,
    /// `USE_MARKER` (nach Anwendung des Vorrangs von `USE_FIELDS`)
    pub use_marker: bool,
    /// `USE_FIELDS` (gewinnt, wenn beide true)
    pub use_fields: bool,
    /// `USE_PLACEHOLDER_SEARCH`
    pub use_placeholder_search: bool,
    /// `MOBILE_NUMBER` (normalisiert; leer → `None`)
    pub mobile_number: Option<String>,
    /// `MOBILE_REMEMBER_PASSWORD` (**case-sensitiv** `"true"`)
    pub mobile_remember_password: bool,
    /// `SIGNATURE_NOTE`
    pub signature_note: LegacyNote,
    /// `SIGNLOCALE` (unbekannt → `None` = System)
    pub sign_locale: Option<Lang>,
    /// `SIGNATURE_PDFA_COMPAT`
    pub pdfa_compat: bool,
    /// `SIGNATURE_PROFILE` (unbekannt → Default)
    pub profile: SignatureProfile,
    /// `EMBLEM` (Existenz wird bei der Migration geprüft)
    pub emblem: Option<PathBuf>,
    /// `LOGO_ONLY_SIZE` in mm (Default 23; Parse-Fehler ignoriert)
    pub logo_only_size_mm: f64,
    /// `LOCALE`
    pub locale: Option<Lang>,
    /// `UPDATE_CHECK` (nur `false` schaltet ab)
    pub update_check: bool,
    /// `PROXY_HOST`
    pub proxy_host: Option<String>,
    /// `PROXY_PORT` (ungültig → `None` + Warnung statt Abbruch)
    pub proxy_port: Option<u16>,
    /// `PROXY_USER`
    pub proxy_user: Option<String>,
    /// `PROXY_PASS` (Klartext-Secret)
    pub proxy_pass: Option<String>,
    /// `OUTPUT_FOLDER`
    pub output_folder: Option<PathBuf>,
    /// `SAVE_FILE_POSTFIX` (fehlt → `_signed`)
    pub save_file_postfix: String,
    /// `MAINWINDOW_SIZE` (Parse-Fehler → Default)
    pub mainwindow_size: Option<(u32, u32)>,
    /// `KEYSTORE_ENABLED`
    pub keystore_enabled: bool,
    /// `KEYSTORE_FILE`
    pub keystore_file: Option<PathBuf>,
    /// `KEYSTORE_TYPE`
    pub keystore_type: Option<String>,
    /// `KEYSTORE_ALIAS`
    pub keystore_alias: Option<String>,
    /// `KEYSTORE_PASS_STORE_TYPE` (Auto-Migration: Passwörter vorhanden → `disk`, sonst `memory`;
    /// unbekannt → `none`)
    pub keystore_pass_store_type: PasswordStorage,
    /// `KEYSTORE_STOREPASS` (Klartext-Secret)
    pub keystore_storepass: Option<String>,
    /// `KEYSTORE_KEYPASS` (Klartext-Secret)
    pub keystore_keypass: Option<String>,
    /// `FIDO2_DEFAULT`
    pub fido2_default: bool,
    /// `LAST_SOURCE_DIRECTORY`
    pub last_source_directory: Option<PathBuf>,
}

/// Geladene Altdatei (unverändert; wird nie zurückgeschrieben).
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyConfig {
    /// Rohwerte.
    pub props: Properties,
    /// Erkannte Kodierung (Latin-1 bzw. von Hand gespeichertes UTF-8).
    pub encoding: Encoding,
}

impl LegacyConfig {
    /// Parst die Bytes (Java-Properties, Kodierungsheuristik).
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        todo!("Paket E: Properties::load_auto")
    }

    /// Liest eine Datei.
    pub fn load(path: &Path) -> Result<Self> {
        todo!("Paket E")
    }

    /// Rohwert eines Keys.
    pub fn raw(&self, key: LegacyKey) -> Option<&str> {
        self.props.get(key.name())
    }

    /// Typisierte Werte nach den exakten Lese-Regeln plus Warnungen.
    pub fn values(&self) -> (LegacyValues, Vec<LegacyWarning>) {
        todo!("Paket E: Tabelle docs/analyse/05 §1.5 Spalte „Lese-Regel“")
    }
}

/// Normalisierung der Telefonnummer wie `MobileBKUValidator` (docs/analyse/02 §3.7a):
/// Whitespace entfernen, `^((\+\d{2})|(00\d{2})|(0)|(10301))([1-9]\d+)$`; `00xx` → `+xx`,
/// `0` → `+43`, `10301…` unverändert; passt der Regex nicht, bleibt der Wert (Benutzername).
pub fn normalize_mobile_number(input: &str) -> String {
    todo!("Paket E (die Integration nutzt sie für `-n` und beim Speichern des Benutzernamens)")
}
