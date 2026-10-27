//! `~/.pdf-over/PDF-Over.config` (Java-Properties) mit exakter Lese-Semantik
//! (docs/analyse/05 §1.3–§1.5; Plan §7.2 Punkt 2). Jede Regel wird als Tabellentest umgesetzt und
//! gegen das Java-Orakel `testdata/config/oracle/LegacyOracle.java` geprüft (Nachbau von
//! `ConfigurationManager.loadFromDisk`, PDF-Over 4.4.x).
//!
//! Abweichungen von PDF-Over (bewusst, als Warnung gemeldet statt Startabbruch bzw. stillem Fehler):
//! - ungültiger `PROXY_PORT` → `None` + Warnung (PDF-Over: `NumberFormatException`, Reset-Dialog),
//! - `LOGO_ONLY_SIZE` = `NaN`/`Infinity`/≤ 0 → Standardwert + Warnung,
//! - negative Werte in `MAINWINDOW_SIZE` → Standardwert + Warnung,
//! - ein UTF-8-BOM am Dateianfang wird entfernt (von Hand in UTF-8 gespeicherte Dateien).

use std::path::{Path, PathBuf};

use pida_i18n::{ArgValue, Catalog, Lang};
use pida_java_properties::{Encoding, Properties, StoreOptions};

use crate::settings::{BackendChoice, PasswordStorage, SignatureProfile};
use crate::{ConfigError, Result, java};

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

    /// Key zum Namen (case-sensitiv wie `Properties.getProperty`).
    pub fn from_name(name: &str) -> Option<LegacyKey> {
        LegacyKey::ALL.into_iter().find(|k| k.name() == name)
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
    /// Key vorhanden, Wert nach `trim()` leer → kein Hinweis (Profil ohne `_NOTE`).
    Empty,
    /// Eigener Text (unverändert, nicht getrimmt).
    Text(String),
}

/// Warnung beim Lesen (blockiert den Start nicht, Plan §7.2: ungültiger `PROXY_PORT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyWarning {
    /// Key
    pub key: LegacyKey,
    /// Rohwert (nie bei Secrets)
    pub raw: String,
    /// Beschreibung (deutsch, für Logs)
    pub message: String,
    /// Meldungs-ID im i18n-Katalog (`ng-config-legacy-*`) mit den Argumenten `key`, `value` und
    /// [`LegacyWarning::args`].
    pub code: &'static str,
    /// Zusätzliche Argumente (z. B. `lang`).
    pub args: Vec<(&'static str, String)>,
}

impl LegacyWarning {
    fn new(key: LegacyKey, raw: &str, code: &'static str, message: impl Into<String>) -> Self {
        Self { key, raw: raw.to_owned(), message: message.into(), code, args: Vec::new() }
    }

    /// Übersetzte Meldung.
    pub fn localized(&self, lang: Lang) -> String {
        let mut args: Vec<(&str, ArgValue)> =
            vec![("key", ArgValue::from(self.key.name())), ("value", ArgValue::from(&self.raw))];
        args.extend(self.args.iter().map(|(k, v)| (*k, ArgValue::from(v))));
        Catalog::global().format(lang, self.code, &args).into_owned()
    }
}

/// Typisierte Werte nach den Lese-Regeln aus docs/analyse/05 §1.5.
#[derive(Clone, PartialEq)]
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
    /// `SIGNLOCALE` (`None` = System): `en…` → EN; jede andere gültige Java-Locale → DE (wie die
    /// Profil-ID `_DE` in PDF-Over); ungültig/leer → `None` (+ Warnung).
    pub sign_locale: Option<Lang>,
    /// `SIGNATURE_PDFA_COMPAT`
    pub pdfa_compat: bool,
    /// `SIGNATURE_PROFILE` (unbekannt → Default)
    pub profile: SignatureProfile,
    /// `EMBLEM` (leer → `None`; Existenz wird bei der Migration geprüft)
    pub emblem: Option<PathBuf>,
    /// `LOGO_ONLY_SIZE` in mm (Default 23; Parse-Fehler ignoriert)
    pub logo_only_size_mm: f64,
    /// `LOCALE` (`None` = System): `de…` → DE, `en…` → EN; jede andere gültige Java-Locale → EN
    /// (PDF-Over fällt auf das Basis-Bundle `messages.properties` = Englisch zurück).
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
    /// `SAVE_FILE_POSTFIX` (fehlt → `_signed`; vorhanden, aber leer → `""` wie PDF-Over)
    pub save_file_postfix: String,
    /// `MAINWINDOW_SIZE` (fehlt bzw. Parse-Fehler → `None` = Default 600×780; ist nur die Höhe
    /// ungültig, gilt wie in Java die gelesene Breite mit Standardhöhe)
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
    /// `LAST_SOURCE_DIRECTORY` (leer → `None`)
    pub last_source_directory: Option<PathBuf>,
}

impl std::fmt::Debug for LegacyValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |o: &Option<String>| o.as_ref().map(|_| "***");
        f.debug_struct("LegacyValues")
            .field("bku", &self.bku)
            .field("auto_position", &self.auto_position)
            .field("use_marker", &self.use_marker)
            .field("use_fields", &self.use_fields)
            .field("use_placeholder_search", &self.use_placeholder_search)
            .field("mobile_number", &self.mobile_number)
            .field("mobile_remember_password", &self.mobile_remember_password)
            .field("signature_note", &self.signature_note)
            .field("sign_locale", &self.sign_locale)
            .field("pdfa_compat", &self.pdfa_compat)
            .field("profile", &self.profile)
            .field("emblem", &self.emblem)
            .field("logo_only_size_mm", &self.logo_only_size_mm)
            .field("locale", &self.locale)
            .field("update_check", &self.update_check)
            .field("proxy_host", &self.proxy_host)
            .field("proxy_port", &self.proxy_port)
            .field("proxy_user", &self.proxy_user)
            .field("proxy_pass", &redact(&self.proxy_pass))
            .field("output_folder", &self.output_folder)
            .field("save_file_postfix", &self.save_file_postfix)
            .field("mainwindow_size", &self.mainwindow_size)
            .field("keystore_enabled", &self.keystore_enabled)
            .field("keystore_file", &self.keystore_file)
            .field("keystore_type", &self.keystore_type)
            .field("keystore_alias", &self.keystore_alias)
            .field("keystore_pass_store_type", &self.keystore_pass_store_type)
            .field("keystore_storepass", &redact(&self.keystore_storepass))
            .field("keystore_keypass", &redact(&self.keystore_keypass))
            .field("fido2_default", &self.fido2_default)
            .field("last_source_directory", &self.last_source_directory)
            .finish()
    }
}

/// Standard-Fenstergröße von PDF-Over (`Constants.DEFAULT_MAINWINDOW_WIDTH/HEIGHT`).
pub const DEFAULT_MAINWINDOW_SIZE: (u32, u32) = (600, 780);
/// Standard-Logo-Größe (`Constants.DEFAULT_LOGO_ONLY_SIZE`) in mm.
pub const DEFAULT_LOGO_ONLY_SIZE_MM: f64 = 23.0;
/// Standard-Postfix (`Constants.DEFAULT_POSTFIX`).
pub const DEFAULT_POSTFIX: &str = "_signed";

/// Geladene Altdatei (unverändert; wird nie zurückgeschrieben).
#[derive(Clone, PartialEq)]
pub struct LegacyConfig {
    /// Rohwerte.
    pub props: Properties,
    /// Erkannte Kodierung (Latin-1 bzw. von Hand gespeichertes UTF-8).
    pub encoding: Encoding,
}

impl std::fmt::Debug for LegacyConfig {
    /// Nur Key-Namen, keine Werte (Klartext-Passwörter, Benutzername; ADR 0001 Regel 9).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys: Vec<&str> = self.props.keys().collect();
        f.debug_struct("LegacyConfig").field("encoding", &self.encoding).field("keys", &keys).finish()
    }
}

impl LegacyConfig {
    /// Parst die Bytes (Java-Properties, Kodierungsheuristik).
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let (props, encoding) = Properties::load_auto(bytes).map_err(|e| ConfigError::Legacy(e.to_string()))?;
        Ok(Self { props, encoding })
    }

    /// Liest eine Datei.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = pida_fs::read(path)?;
        LegacyConfig::parse(&bytes)
    }

    /// Rohwert eines Keys.
    pub fn raw(&self, key: LegacyKey) -> Option<&str> {
        self.props.get(key.name())
    }

    /// Keys der Datei, die PDF-Over nicht (mehr) kennt (z. B. `SIGNATURE_PLACEHOLDER_TRANSPARENCY`).
    pub fn unknown_keys(&self) -> Vec<&str> {
        self.props.keys().filter(|k| LegacyKey::from_name(k).is_none()).collect()
    }

    /// Vorhandene Klartext-Secrets (nicht leer).
    pub fn plaintext_secrets(&self) -> Vec<LegacyKey> {
        LegacyKey::ALL.into_iter().filter(|k| k.is_secret() && self.raw(*k).is_some_and(|v| !v.is_empty())).collect()
    }

    /// Inhalt der Altdatei **ohne** Klartext-Secrets (für „Klartext entfernen“, das die UI nur nach
    /// ausdrücklicher Zustimmung anbietet). Schreibt nichts; Format wie `Properties.store`
    /// (ISO-8859-1, `\uXXXX`, Kopfzeile von PDF-Over).
    pub fn to_bytes_without_secrets(&self) -> Vec<u8> {
        let mut props = self.props.clone();
        for k in LegacyKey::ALL.into_iter().filter(|k| k.is_secret()) {
            props.remove(k.name());
        }
        props.store_latin1(&StoreOptions {
            comment: Some("Configuration file was generated!".into()),
            ..Default::default()
        })
    }

    /// Typisierte Werte nach den exakten Lese-Regeln plus Warnungen.
    pub fn values(&self) -> (LegacyValues, Vec<LegacyWarning>) {
        let mut w = Vec::new();
        let get = |k: LegacyKey| self.raw(k);
        let non_blank = |k: LegacyKey| get(k).filter(|v| !java::trim(v).is_empty()).map(str::to_owned);
        let eq_true = |k: LegacyKey| get(k).is_some_and(|v| java::equals_ignore_case(v, "true"));

        // BKU: BKUs.valueOf, case-sensitiv; NONE und ungültig → keine Voreinstellung.
        let bku = match get(LegacyKey::Bku) {
            None | Some("NONE") => None,
            Some(v) => {
                let b = BackendChoice::from_legacy(v);
                if b.is_none() {
                    w.push(LegacyWarning::new(
                        LegacyKey::Bku,
                        v,
                        "ng-config-legacy-invalid-bku",
                        format!("BKU-Wert {v:?} ungültig, keine Voreinstellung"),
                    ));
                }
                b
            }
        };

        let auto_position = get(LegacyKey::SignaturePosition).is_some_and(|v| java::trim(v) == "auto");

        // USE_MARKER wird vor USE_FIELDS gelesen; ein gesetztes Flag löscht jeweils das andere.
        let mut use_marker = false;
        let mut use_fields = false;
        if get(LegacyKey::UseMarker).is_some() && eq_true(LegacyKey::UseMarker) {
            use_marker = true;
            use_fields = false;
        }
        if get(LegacyKey::UseFields).is_some() && eq_true(LegacyKey::UseFields) {
            use_fields = true;
            use_marker = false;
        }
        let use_placeholder_search = eq_true(LegacyKey::UsePlaceholderSearch);

        let mobile_number = non_blank(LegacyKey::MobileNumber).map(|n| normalize_mobile_number(&n));
        let mobile_remember_password = get(LegacyKey::MobileRememberPassword) == Some("true");

        let signature_note = match get(LegacyKey::SignatureNote) {
            None => LegacyNote::Missing,
            Some(v) if java::trim(v).is_empty() => LegacyNote::Empty,
            Some(v) => LegacyNote::Text(v.to_owned()),
        };

        let sign_locale = self.locale_value(LegacyKey::SignLocale, Lang::De, &mut w);
        let locale = self.locale_value(LegacyKey::Locale, Lang::En, &mut w);

        let pdfa_compat = eq_true(LegacyKey::SignaturePdfaCompat);

        let profile = match get(LegacyKey::SignatureProfile) {
            None => SignatureProfile::default(),
            Some(v) => SignatureProfile::from_legacy_name(v).unwrap_or_else(|| {
                w.push(LegacyWarning::new(
                    LegacyKey::SignatureProfile,
                    v,
                    "ng-config-legacy-unknown-profile",
                    format!("Profil {v:?} unbekannt, Standard-Signaturblock"),
                ));
                SignatureProfile::default()
            }),
        };

        let emblem = non_blank(LegacyKey::Emblem).map(PathBuf::from);

        let logo_only_size_mm = match get(LegacyKey::LogoOnlySize) {
            None => DEFAULT_LOGO_ONLY_SIZE_MM,
            Some(v) => match java::parse_double(v) {
                Some(d) if d.is_finite() && d > 0.0 => d,
                _ => {
                    w.push(LegacyWarning::new(
                        LegacyKey::LogoOnlySize,
                        v,
                        "ng-config-legacy-invalid-value",
                        format!("LOGO_ONLY_SIZE {v:?} ungültig, 23 mm"),
                    ));
                    DEFAULT_LOGO_ONLY_SIZE_MM
                }
            },
        };

        // UPDATE_CHECK: jeder Wert außer „false“ (equalsIgnoreCase) lässt den Check an.
        let update_check = !get(LegacyKey::UpdateCheck).is_some_and(|v| java::equals_ignore_case(v, "false"));

        let proxy_host = non_blank(LegacyKey::ProxyHost);
        let proxy_user = non_blank(LegacyKey::ProxyUser);
        let proxy_pass = non_blank(LegacyKey::ProxyPass);

        // PROXY_PORT: nur geprüft, wenn nicht leer; Integer.parseInt ohne trim().
        let proxy_port = match get(LegacyKey::ProxyPort) {
            Some(v) if !java::trim(v).is_empty() => match java::parse_int(v) {
                Some(p) if (1..=0xFFFF).contains(&p) => u16::try_from(p).ok(),
                Some(_) => {
                    w.push(LegacyWarning::new(
                        LegacyKey::ProxyPort,
                        v,
                        "ng-config-legacy-proxy-port-out-of-range",
                        format!("PROXY_PORT {v} außerhalb 1–65535"),
                    ));
                    None
                }
                None => {
                    w.push(LegacyWarning::new(
                        LegacyKey::ProxyPort,
                        v,
                        "ng-config-legacy-invalid-proxy-port",
                        format!("PROXY_PORT {v:?} keine Zahl (PDF-Over bricht hier den Start ab)"),
                    ));
                    None
                }
            },
            _ => None,
        };

        let output_folder = non_blank(LegacyKey::OutputFolder).map(PathBuf::from);
        let save_file_postfix = get(LegacyKey::SaveFilePostfix).unwrap_or(DEFAULT_POSTFIX).to_owned();

        let mainwindow_size = get(LegacyKey::MainwindowSize).and_then(|v| self.window_size(v, &mut w));

        let keystore_enabled = eq_true(LegacyKey::KeystoreEnabled);
        let keystore_file = non_blank(LegacyKey::KeystoreFile).map(PathBuf::from);
        let keystore_type = non_blank(LegacyKey::KeystoreType);
        let keystore_alias = non_blank(LegacyKey::KeystoreAlias);
        let keystore_storepass = get(LegacyKey::KeystoreStorepass).map(str::to_owned);
        let keystore_keypass = get(LegacyKey::KeystoreKeypass).map(str::to_owned);
        let keystore_pass_store_type = match get(LegacyKey::KeystorePassStoreType) {
            None => {
                let has = |p: &Option<String>| p.as_deref().is_some_and(|s| !java::trim(s).is_empty());
                if has(&keystore_keypass) || has(&keystore_storepass) {
                    PasswordStorage::Keychain
                } else {
                    PasswordStorage::Memory
                }
            }
            Some("disk") => PasswordStorage::Keychain,
            Some("memory") => PasswordStorage::Memory,
            Some("none") => PasswordStorage::None,
            Some(other) => {
                w.push(LegacyWarning::new(
                    LegacyKey::KeystorePassStoreType,
                    other,
                    "ng-config-legacy-unknown-pass-store-type",
                    format!("KEYSTORE_PASS_STORE_TYPE {other:?} unbekannt, „none“"),
                ));
                PasswordStorage::None
            }
        };

        let fido2_default = get(LegacyKey::Fido2Default).is_some_and(|v| java::equals_ignore_case(v, "true"));
        // PDF-Over übernimmt den Rohwert; ein leerer Wert ist kein Ordner.
        let last_source_directory = get(LegacyKey::LastSourceDirectory).filter(|v| !v.is_empty()).map(PathBuf::from);

        (
            LegacyValues {
                bku,
                auto_position,
                use_marker,
                use_fields,
                use_placeholder_search,
                mobile_number,
                mobile_remember_password,
                signature_note,
                sign_locale,
                pdfa_compat,
                profile,
                emblem,
                logo_only_size_mm,
                locale,
                update_check,
                proxy_host,
                proxy_port,
                proxy_user,
                proxy_pass,
                output_folder,
                save_file_postfix,
                mainwindow_size,
                keystore_enabled,
                keystore_file,
                keystore_type,
                keystore_alias,
                keystore_pass_store_type,
                keystore_storepass,
                keystore_keypass,
                fido2_default,
                last_source_directory,
            },
            w,
        )
    }

    /// `LocaleSerializer.parseFromString` + Abbildung auf DE/EN; `other` = Sprache für gültige,
    /// aber nicht unterstützte Locales.
    fn locale_value(&self, key: LegacyKey, other: Lang, w: &mut Vec<LegacyWarning>) -> Option<Lang> {
        let v = self.raw(key)?;
        if v.is_empty() {
            return None;
        }
        match java::java_locale_language(v) {
            Some("de") => Some(Lang::De),
            Some("en") => Some(Lang::En),
            Some(_) => {
                let mut warning = LegacyWarning::new(
                    key,
                    v,
                    "ng-config-legacy-unsupported-locale",
                    format!("{} {v:?} nicht unterstützt, {}", key.name(), other.tag()),
                );
                warning.args.push(("lang", other.tag().to_owned()));
                w.push(warning);
                Some(other)
            }
            None => {
                w.push(LegacyWarning::new(
                    key,
                    v,
                    "ng-config-legacy-unknown-locale",
                    format!("{} {v:?} keine gültige Java-Locale, Systemsprache", key.name()),
                ));
                None
            }
        }
    }

    /// `MAINWINDOW_SIZE`: `indexOf(',') <= 0` → Default; Breite und Höhe je `parseInt(trim)`; wirft die
    /// Höhe, bleibt die bereits gelesene Breite (Java-Zuweisungsreihenfolge).
    fn window_size(&self, v: &str, w: &mut Vec<LegacyWarning>) -> Option<(u32, u32)> {
        let (_, def_h) = DEFAULT_MAINWINDOW_SIZE;
        let pos = v.find(',').filter(|&p| p > 0);
        let parsed = pos.and_then(|pos| {
            let width = java::parse_int(java::trim(&v[..pos]))?;
            let height = java::parse_int(java::trim(&v[pos + 1..]));
            Some((width, height))
        });
        let warn = |w: &mut Vec<LegacyWarning>| {
            w.push(LegacyWarning::new(
                LegacyKey::MainwindowSize,
                v,
                "ng-config-legacy-invalid-value",
                format!("MAINWINDOW_SIZE {v:?} ungültig, 600,780"),
            ))
        };
        match parsed {
            Some((width, Some(height))) => match (u32::try_from(width), u32::try_from(height)) {
                (Ok(a), Ok(b)) => Some((a, b)),
                _ => {
                    warn(w);
                    None
                }
            },
            Some((width, None)) => {
                warn(w);
                u32::try_from(width).ok().map(|a| (a, def_h))
            }
            None => {
                warn(w);
                None
            }
        }
    }
}

/// Normalisierung der Telefonnummer wie `MobileBKUValidator` (docs/analyse/02 §3.7a):
/// Whitespace entfernen, `^((\+\d{2})|(00\d{2})|(0)|(10301))([1-9]\d+)$`; `00xx` → `+xx`,
/// `0` → `+43`, `10301…` unverändert; passt der Regex nicht, bleibt der Wert (Benutzername).
///
/// Exakt wie Java: `trim()`, dann `\s` (ASCII-Leerraum) entfernen; ohne Treffer wird der
/// **getrimmte** Wert (mit innerem Leerraum) geliefert; `countryCode.replace("00", "+")` ersetzt jedes
/// Vorkommen (z. B. `0000…` → `++…`).
pub fn normalize_mobile_number(input: &str) -> String {
    let number = java::trim(input);
    let compact: String =
        number.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')).collect();
    let rest_ok = |rest: &str| {
        let b = rest.as_bytes();
        b.len() >= 2 && (b'1'..=b'9').contains(&b[0]) && b[1..].iter().all(u8::is_ascii_digit)
    };
    let digit = |b: Option<&u8>| b.is_some_and(u8::is_ascii_digit);
    let bytes = compact.as_bytes();
    // Alternativen in Regex-Reihenfolge; jede hat eine feste Länge.
    let candidates: [(usize, bool); 4] = [
        (3, bytes.first() == Some(&b'+') && digit(bytes.get(1)) && digit(bytes.get(2))),
        (4, bytes.starts_with(b"00") && digit(bytes.get(2)) && digit(bytes.get(3))),
        (1, bytes.first() == Some(&b'0')),
        (5, bytes.starts_with(b"10301")),
    ];
    for (len, matches_prefix) in candidates {
        if !matches_prefix || !rest_ok(&compact[len..]) {
            continue;
        }
        let country = &compact[..len];
        let national = &compact[len..];
        if country == "10301" {
            return compact;
        }
        let mut country = country.replace("00", "+");
        if country == "0" {
            country = "+43".into();
        }
        return format!("{country}{national}");
    }
    number.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(text: &str) -> (LegacyValues, Vec<LegacyWarning>) {
        LegacyConfig::parse(text.as_bytes()).unwrap().values()
    }

    #[test]
    fn defaults_for_empty_file() {
        let (v, w) = values("");
        assert!(w.is_empty());
        assert_eq!(v.bku, None);
        assert!(v.update_check);
        assert_eq!(v.signature_note, LegacyNote::Missing);
        assert_eq!(v.keystore_pass_store_type, PasswordStorage::Memory);
        assert_eq!(v.save_file_postfix, "_signed");
        assert_eq!(v.logo_only_size_mm, 23.0);
        assert_eq!(v.mainwindow_size, None);
        assert_eq!(v.profile, SignatureProfile::SignaturblockSmall);
    }

    #[test]
    fn update_check_only_false_disables() {
        for (raw, expected) in [
            ("false", false),
            ("FALSE", false),
            ("False", false),
            ("fal\u{17f}e", false),
            ("no", true),
            ("0", true),
            ("", true),
            ("\\ false", true),
            ("false ", true),
            (" false", false),
        ] {
            assert_eq!(values(&format!("UPDATE_CHECK={raw}\n")).0.update_check, expected, "{raw:?}");
        }
    }

    #[test]
    fn remember_password_is_case_sensitive() {
        assert!(values("MOBILE_REMEMBER_PASSWORD=true\n").0.mobile_remember_password);
        assert!(!values("MOBILE_REMEMBER_PASSWORD=TRUE\n").0.mobile_remember_password);
        assert!(!values("MOBILE_REMEMBER_PASSWORD=True\n").0.mobile_remember_password);
        assert!(!values("MOBILE_REMEMBER_PASSWORD=true \n").0.mobile_remember_password);
    }

    #[test]
    fn fields_win_over_marker() {
        let table = [
            ("USE_MARKER=true\nUSE_FIELDS=true\n", false, true),
            ("USE_MARKER=TRUE\nUSE_FIELDS=false\n", true, false),
            ("USE_MARKER=false\nUSE_FIELDS=True\n", false, true),
            ("USE_FIELDS=true\nUSE_MARKER=true\n", false, true),
            ("", false, false),
        ];
        for (text, marker, fields) in table {
            let v = values(text).0;
            assert_eq!((v.use_marker, v.use_fields), (marker, fields), "{text:?}");
        }
    }

    #[test]
    fn note_missing_vs_empty() {
        assert_eq!(values("").0.signature_note, LegacyNote::Missing);
        assert_eq!(values("SIGNATURE_NOTE=\n").0.signature_note, LegacyNote::Empty);
        assert_eq!(values("SIGNATURE_NOTE=   \n").0.signature_note, LegacyNote::Empty);
        assert_eq!(values("SIGNATURE_NOTE=\\u00A0\n").0.signature_note, LegacyNote::Text("\u{a0}".into()));
        assert_eq!(values("SIGNATURE_NOTE=Gr\\u00FC\\u00DFe \n").0.signature_note, LegacyNote::Text("Grüße ".into()));
    }

    #[test]
    fn keystore_pass_store_type_auto_migration() {
        let t = |s: &str| values(s).0.keystore_pass_store_type;
        assert_eq!(t(""), PasswordStorage::Memory);
        assert_eq!(t("KEYSTORE_STOREPASS=x\n"), PasswordStorage::Keychain);
        assert_eq!(t("KEYSTORE_KEYPASS=x\n"), PasswordStorage::Keychain);
        assert_eq!(t("KEYSTORE_STOREPASS=  \n"), PasswordStorage::Memory);
        assert_eq!(t("KEYSTORE_PASS_STORE_TYPE=disk\n"), PasswordStorage::Keychain);
        assert_eq!(t("KEYSTORE_PASS_STORE_TYPE=memory\nKEYSTORE_STOREPASS=x\n"), PasswordStorage::Memory);
        assert_eq!(t("KEYSTORE_PASS_STORE_TYPE=none\n"), PasswordStorage::None);
        let (v, w) = values("KEYSTORE_PASS_STORE_TYPE=DISK\n");
        assert_eq!(v.keystore_pass_store_type, PasswordStorage::None);
        assert_eq!(w[0].code, "ng-config-legacy-unknown-pass-store-type");
    }

    #[test]
    fn proxy_port_invalid_is_a_warning() {
        let (v, w) = values("PROXY_PORT=abc\n");
        assert_eq!(v.proxy_port, None);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, "ng-config-legacy-invalid-proxy-port");
        assert!(w[0].localized(Lang::De).contains("abc"));
        assert_eq!(values("PROXY_PORT=8080\n").0.proxy_port, Some(8080));
        assert_eq!(values("PROXY_PORT=+3128\n").0.proxy_port, Some(3128));
        assert_eq!(values("PROXY_PORT= \n").0, values("").0);
        assert_eq!(values("PROXY_PORT=8080 \n").0.proxy_port, None);
        let (v, w) = values("PROXY_PORT=70000\n");
        assert_eq!(v.proxy_port, None);
        assert_eq!(w[0].code, "ng-config-legacy-proxy-port-out-of-range");
        assert_eq!(values("PROXY_PORT=0\n").1[0].code, "ng-config-legacy-proxy-port-out-of-range");
        assert_eq!(values("PROXY_PORT=-1\n").1.len(), 1);
    }

    #[test]
    fn bku_values() {
        assert_eq!(values("BKU=MOBILE\n").0.bku, Some(BackendChoice::IdAustria));
        assert_eq!(values("BKU=LOCAL\n").0.bku, Some(BackendChoice::LocalBku));
        assert_eq!(values("BKU=KS\n").0.bku, Some(BackendChoice::Keystore));
        let (v, w) = values("BKU=NONE\n");
        assert_eq!((v.bku, w.len()), (None, 0));
        let (v, w) = values("BKU=mobile\n");
        assert_eq!((v.bku, w[0].code), (None, "ng-config-legacy-invalid-bku"));
    }

    #[test]
    fn locales() {
        assert_eq!(values("LOCALE=de\n").0.locale, Some(Lang::De));
        assert_eq!(values("LOCALE=en\n").0.locale, Some(Lang::En));
        assert_eq!(values("LOCALE=de_AT\n").0.locale, Some(Lang::De));
        assert_eq!(values("LOCALE=\n").0.locale, None);
        let (v, w) = values("LOCALE=de-AT\n");
        assert_eq!((v.locale, w[0].code), (None, "ng-config-legacy-unknown-locale"));
        let (v, w) = values("LOCALE=fr\nSIGNLOCALE=fr\n");
        assert_eq!((v.locale, v.sign_locale), (Some(Lang::En), Some(Lang::De)));
        assert_eq!(w.len(), 2);
        assert!(w[0].localized(Lang::De).contains("fr"));
    }

    #[test]
    fn window_size_rules() {
        let s = |t: &str| values(&format!("MAINWINDOW_SIZE={t}\n")).0.mainwindow_size;
        assert_eq!(s("800,600"), Some((800, 600)));
        assert_eq!(s(" 800 , 600 "), Some((800, 600)));
        assert_eq!(s("800,x"), Some((800, 780)));
        assert_eq!(s("x,600"), None);
        assert_eq!(s(",600"), None);
        assert_eq!(s("800"), None);
        assert_eq!(s("-5,600"), None);
    }

    #[test]
    fn other_values() {
        let (v, w) = values(
            "SIGNATURE_POSITION= auto \nLOGO_ONLY_SIZE=42.5\nSAVE_FILE_POSTFIX=\nSIGNATURE_PROFILE=BASE_LOGO\nFIDO2_DEFAULT=TRUE\nKEYSTORE_ENABLED=True\nPROXY_HOST=  \nOUTPUT_FOLDER=.\n",
        );
        assert!(w.is_empty(), "{w:?}");
        assert!(v.auto_position);
        assert_eq!(v.logo_only_size_mm, 42.5);
        assert_eq!(v.save_file_postfix, "");
        assert_eq!(v.profile, SignatureProfile::BaseLogo);
        assert!(v.fido2_default && v.keystore_enabled);
        assert_eq!(v.proxy_host, None);
        assert_eq!(v.output_folder, Some(PathBuf::from(".")));
        assert!(values("SIGNATURE_POSITION=x=1;y=2\n").0.auto_position.eq(&false));
        assert_eq!(values("SIGNATURE_PROFILE=invisible\n").1[0].code, "ng-config-legacy-unknown-profile");
        assert_eq!(values("LOGO_ONLY_SIZE=NaN\n").0.logo_only_size_mm, 23.0);
        assert_eq!(values("LOGO_ONLY_SIZE=12,5\n").1.len(), 1);
    }

    #[test]
    fn mobile_number_normalization() {
        let table = [
            ("0664 1234567", "+436641234567"),
            ("+43 664 1234567", "+436641234567"),
            ("0043664123456", "+43664123456"),
            ("1030112345678", "1030112345678"),
            ("10301 12345", "1030112345"),
            ("max.mustermann", "max.mustermann"),
            ("  max  ", "max"),
            ("0043 0664", "0043 0664"),
            ("00001234", "++1234"),
            ("+0012345", "++12345"),
            ("06", "06"),
            ("0\t6 6", "+4366"),
            ("+43\u{a0}664", "+43\u{a0}664"),
        ];
        for (input, expected) in table {
            assert_eq!(normalize_mobile_number(input), expected, "{input:?}");
        }
        assert_eq!(values("MOBILE_NUMBER=0664 1234567\n").0.mobile_number.as_deref(), Some("+436641234567"));
        assert_eq!(values("MOBILE_NUMBER= \n").0.mobile_number, None);
    }

    #[test]
    fn debug_hides_secrets() {
        let cfg =
            LegacyConfig::parse(b"KEYSTORE_STOREPASS=geheim1\nPROXY_PASS=geheim2\nKEYSTORE_KEYPASS=geheim3\n").unwrap();
        let (v, _) = cfg.values();
        let dbg = format!("{cfg:?} {v:?}");
        assert!(!dbg.contains("geheim"), "{dbg}");
        assert!(dbg.contains("KEYSTORE_STOREPASS"));
        assert_eq!(cfg.plaintext_secrets().len(), 3);
        let cleaned = LegacyConfig::parse(&cfg.to_bytes_without_secrets()).unwrap();
        assert!(cleaned.plaintext_secrets().is_empty());
    }

    #[test]
    fn bom_and_unknown_keys() {
        let cfg = LegacyConfig::parse(b"\xEF\xBB\xBFBKU=KS\nSIGNATURE_PLACEHOLDER_TRANSPARENCY=50\n").unwrap();
        assert_eq!(cfg.values().0.bku, Some(BackendChoice::Keystore));
        assert_eq!(cfg.unknown_keys(), vec!["SIGNATURE_PLACEHOLDER_TRANSPARENCY"]);
        assert_eq!(LegacyKey::from_name("BKU"), Some(LegacyKey::Bku));
        assert_eq!(LegacyKey::ALL.iter().filter(|k| k.is_migrated()).count(), 31);
    }
}
