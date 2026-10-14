//! Neues Settings-Schema (TOML, versioniert; Plan §7.1).
//!
//! ```toml
//! # PDF-Over NG – Einstellungen
//! schema_version = 1
//!
//! [general]
//! ui_language = "de"          # fehlt = Systemsprache (Fallback DE)
//! update_check = true
//!
//! [signature]
//! profile = "SIGNATURBLOCK_SMALL"
//! block_language = "en"       # fehlt = Systemsprache
//! note = { mode = "custom", text = "Eigener Hinweis" }   # bzw. { mode = "default" } / { mode = "none" }
//! logo_only_size_mm = 23.0
//!
//! [id_austria]
//! username = "+43664123456789"
//! ```
//!
//! - **Versionierung:** `schema_version` (fehlt = 1). Ältere Stände werden beim Lesen schrittweise
//!   angehoben ([`Settings::from_toml`]), neuere ergeben [`crate::ConfigError::UnsupportedSchema`].
//! - **Unbekannte Keys** bleiben beim Speichern erhalten – auf oberster Ebene **und** in den
//!   bekannten Tabellen. Sie stehen in [`Settings::unknown`]; der Schlüssel ist der TOML-Pfad in
//!   Punktschreibweise (`signature.future_key`, Segmente ggf. in Anführungszeichen). So übersteht die
//!   Map auch den JSON-Rundlauf über die Tauri-Bridge.
//! - **Speichern** atomar (Temp-Datei, `fsync`, Umbenennen); Rechte 0600 unter Unix.
//! - Das ID-Austria-Passwort und andere Secrets stehen **nie** in dieser Datei ([`crate::secrets`]).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use pdfover_i18n::{Catalog, Lang};

use crate::{ConfigError, Result};

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
    /// Alle Profile.
    pub const ALL: [SignatureProfile; 4] = [
        SignatureProfile::SignaturblockSmall,
        SignatureProfile::Amtssignaturblock,
        SignatureProfile::BaseLogo,
        SignatureProfile::Invisible,
    ];

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

    /// Fluent-ID des Standard-Hinweistexts (`Profile.getDefaultSignatureBlockNote`): nur
    /// `SIGNATURBLOCK_SMALL` und `AMTSSIGNATURBLOCK` haben einen; sonst `None` (= `""`, kein Hinweis).
    pub fn default_note_id(self) -> Option<&'static str> {
        match self {
            SignatureProfile::SignaturblockSmall => Some("simple_config-Note_Default_Standard"),
            SignatureProfile::Amtssignaturblock => Some("simple_config-Note_Default_OfficialSignature"),
            SignatureProfile::BaseLogo | SignatureProfile::Invisible => None,
        }
    }

    /// Standard-Hinweistext in der Sprache des Signaturblocks (`None` = kein Hinweis).
    pub fn default_note(self, block_language: Lang) -> Option<String> {
        let id = self.default_note_id()?;
        let catalog = Catalog::global();
        catalog.has(block_language, id).then(|| catalog.format(block_language, id, &[]).into_owned())
    }

    /// Fluent-ID des Anzeigenamens (`simple_config.<Profil>`).
    pub fn label_id(self) -> &'static str {
        match self {
            SignatureProfile::SignaturblockSmall => "simple_config-SIGNATURBLOCK_SMALL",
            SignatureProfile::Amtssignaturblock => "simple_config-AMTSSIGNATURBLOCK",
            SignatureProfile::BaseLogo => "simple_config-BASE_LOGO",
            SignatureProfile::Invisible => "simple_config-INVISIBLE",
        }
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

    /// Name in PDF-Over (`BKU`, `-b`).
    pub fn legacy_name(self) -> &'static str {
        match self {
            BackendChoice::IdAustria => "MOBILE",
            BackendChoice::LocalBku => "LOCAL",
            BackendChoice::Keystore => "KS",
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

/// Zertifikats-Cache (Opt-in, nur öffentliches Zertifikat, [`crate::CertificateCache`]).
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
    /// Unbekannte Tabellen/Keys (bleiben beim Speichern erhalten). Schlüssel = TOML-Pfad in
    /// Punktschreibweise (`future`, `signature.future_key`, `general."a b"`).
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

static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Kopfzeile der geschriebenen Datei.
const FILE_HEADER: &str = "# PDF-Over NG – Einstellungen. Unbekannte Einträge bleiben beim Speichern erhalten.\n\n";

impl Settings {
    /// Liest die TOML-Datei.
    pub fn load(path: &Path) -> LoadOutcome {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadOutcome::Missing,
            Err(e) => return LoadOutcome::Unreadable(ConfigError::Io(e)),
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return LoadOutcome::Unreadable(ConfigError::Parse("keine gültige UTF-8-Datei".into()));
        };
        match Settings::from_toml(&text) {
            Ok(s) => LoadOutcome::Loaded(Box::new(s)),
            Err(e) => LoadOutcome::Unreadable(e),
        }
    }

    /// Parst TOML-Text (inkl. Schema-Migration älterer Versionen).
    pub fn from_toml(text: &str) -> Result<Settings> {
        let mut table: toml::Table = toml::from_str(text).map_err(|e| ConfigError::Parse(e.message().to_owned()))?;
        let version = match table.get("schema_version") {
            None => 1,
            Some(toml::Value::Integer(v)) if *v >= 1 => u32::try_from(*v).unwrap_or(u32::MAX),
            Some(other) => return Err(ConfigError::Parse(format!("ungültige schema_version: {other}"))),
        };
        if version > SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema { found: version, supported: SCHEMA_VERSION });
        }
        upgrade(&mut table, version)?;
        table.insert("schema_version".into(), toml::Value::Integer(i64::from(SCHEMA_VERSION)));

        // Bekannte Teile deserialisieren (unbekannte Top-Level-Keys nicht durch `flatten` schicken).
        let known_top = known_top_level_keys();
        let known_only: toml::Table = table
            .iter()
            .filter(|(k, _)| known_top.iter().any(|t| t == *k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut settings: Settings =
            known_only.try_into().map_err(|e: toml::de::Error| ConfigError::Parse(e.message().to_owned()))?;
        settings.unknown.clear();

        // Alles, was die Serialisierung der bekannten Werte nicht reproduziert, ist unbekannt.
        let known = settings.known_table()?;
        let mut unknown = BTreeMap::new();
        collect_unknown(&table, &known, &mut Vec::new(), &mut unknown);
        settings.unknown = unknown;
        Ok(settings)
    }

    /// Bekannte Werte als TOML-Tabelle (ohne [`Settings::unknown`]).
    fn known_table(&self) -> Result<toml::Table> {
        let mut copy = self.clone();
        copy.unknown.clear();
        copy.schema_version = SCHEMA_VERSION;
        toml::Table::try_from(&copy).map_err(|e| ConfigError::Serialize(e.to_string()))
    }

    /// Serialisiert als TOML (inklusive der unbekannten Keys an ihrem Ursprungsort).
    pub fn to_toml(&self) -> Result<String> {
        let mut table = self.known_table()?;
        for (path, value) in &self.unknown {
            insert_unknown(&mut table, &decode_key_path(path), value.clone());
        }
        let body = toml::to_string(&table).map_err(|e| ConfigError::Serialize(e.to_string()))?;
        Ok(format!("{FILE_HEADER}{body}"))
    }

    /// Schreibt atomar (Temp-Datei, fsync, Umbenennen).
    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        let text = self.to_toml()?;
        write_atomic(path, text.as_bytes())
    }

    /// Effektive Sprache des Signaturblocks.
    pub fn effective_block_language(&self, system: Lang) -> Lang {
        self.signature.block_language.unwrap_or(system)
    }

    /// Effektive UI-Sprache (Einstellung, sonst System, Fallback DE).
    pub fn effective_ui_language(&self) -> Lang {
        Lang::effective(self.general.ui_language)
    }

    /// Hinweistext für das Profil: `None` = kein Hinweis; Default-Text aus dem i18n-Katalog
    /// (`simple_config-Note_Default_Standard` bzw. `…_OfficialSignature`).
    ///
    /// Wie PDF-Over (`setSignatureNotePersistent`): ein Text, der nach `trim()` leer ist, gilt als
    /// „kein Hinweis“; das Profil erhält dann kein `_NOTE` ([`SignatureProfile::pdfas_profile_id`]).
    pub fn note_text(&self, block_language: Lang) -> Option<String> {
        match &self.signature.note {
            NoteSetting::None => None,
            NoteSetting::Custom(t) if crate::java::trim(t).is_empty() => None,
            NoteSetting::Custom(t) => Some(t.clone()),
            NoteSetting::Default => self.signature.profile.default_note(block_language),
        }
    }

    /// PDF-AS-Profil-ID für diese Einstellungen (`pdfa_detected` = erkannte PDF/A-Kennung des
    /// Dokuments; wirksam ist Schalter **oder** Kennung, Plan §6.9).
    pub fn pdfas_profile_id(&self, system: Lang, pdfa_detected: bool) -> String {
        let lang = self.effective_block_language(system);
        self.signature.profile.pdfas_profile_id(
            lang,
            self.note_text(lang).is_some(),
            self.signature.pdfa_force || pdfa_detected,
        )
    }
}

/// Hebt ältere Schema-Versionen schrittweise auf [`SCHEMA_VERSION`] an (derzeit gibt es nur 1).
fn upgrade(_table: &mut toml::Table, from: u32) -> Result<()> {
    let mut v = from;
    while v < SCHEMA_VERSION {
        // Hier folgen künftige Schritte `v → v + 1`.
        v += 1;
    }
    Ok(())
}

fn known_top_level_keys() -> &'static [String] {
    static KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    KEYS.get_or_init(|| {
        toml::Table::try_from(Settings::default()).map(|t| t.keys().cloned().collect()).unwrap_or_default()
    })
}

/// Sammelt Keys aus `orig`, die in `known` fehlen (rekursiv nur durch bekannte Tabellen).
fn collect_unknown(
    orig: &toml::Table,
    known: &toml::Table,
    path: &mut Vec<String>,
    out: &mut BTreeMap<String, toml::Value>,
) {
    for (k, v) in orig {
        path.push(k.clone());
        match (known.get(k), v) {
            (None, _) => {
                out.insert(encode_key_path(path), v.clone());
            }
            (Some(toml::Value::Table(kt)), toml::Value::Table(ot)) => collect_unknown(ot, kt, path, out),
            _ => {}
        }
        path.pop();
    }
}

/// Fügt einen unbekannten Wert wieder ein. Alle Segmente außer dem letzten müssen vorhandene Tabellen
/// sein (sonst wurde der bekannte Elternwert entfernt, und der Wert wird verworfen); bekannte Werte
/// werden nie überschrieben.
fn insert_unknown(table: &mut toml::Table, path: &[String], value: toml::Value) {
    let Some((last, parents)) = path.split_last() else { return };
    let mut current = table;
    for p in parents {
        match current.get_mut(p) {
            Some(toml::Value::Table(t)) => current = t,
            _ => return,
        }
    }
    current.entry(last.clone()).or_insert(value);
}

fn is_bare_key(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// TOML-Punktpfad: Segmente als „bare key“ oder als Basic String in Anführungszeichen.
pub(crate) fn encode_key_path(path: &[String]) -> String {
    let mut out = String::new();
    for (i, seg) in path.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        if is_bare_key(seg) {
            out.push_str(seg);
            continue;
        }
        out.push('"');
        for c in seg.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out
}

/// Umkehrung von [`encode_key_path`] mit dem TOML-Parser selbst; nicht parsebare Schlüssel (z. B. aus
/// JSON) gelten als ein einzelnes Segment.
pub(crate) fn decode_key_path(key: &str) -> Vec<String> {
    let parsed: Option<toml::Table> = toml::from_str(&format!("{key} = 0")).ok();
    let mut out = Vec::new();
    let mut current = parsed.as_ref();
    while let Some(t) = current {
        let mut it = t.iter();
        let (Some((k, v)), None) = (it.next(), it.next()) else {
            return vec![key.to_owned()];
        };
        out.push(k.clone());
        current = match v {
            toml::Value::Table(inner) => Some(inner),
            _ => None,
        };
    }
    if out.is_empty() { vec![key.to_owned()] } else { out }
}

/// Schreibt atomar: Temp-Datei im Zielordner (`create_new`, 0600 unter Unix), `fsync`, Umbenennen,
/// Ordner-`fsync` (Unix).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "settings".into());
    let tmp = dir.join(format!(".{name}.tmp-{}-{}", std::process::id(), TMP_COUNTER.fetch_add(1, Ordering::Relaxed)));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let result = (|| -> std::io::Result<()> {
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)?;
        #[cfg(unix)]
        if let Ok(d) = std::fs::File::open(&dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(ConfigError::Io)
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

    #[test]
    fn key_paths_roundtrip() {
        for path in [
            vec!["future".to_owned()],
            vec!["signature".to_owned(), "x-y_1".to_owned()],
            vec!["general".to_owned(), "a b".to_owned()],
            vec!["general".to_owned(), "dotted.key".to_owned()],
            vec!["q".to_owned(), "\"quoted\\".to_owned()],
            vec!["ä".to_owned()],
        ] {
            let enc = encode_key_path(&path);
            assert_eq!(decode_key_path(&enc), path, "{enc}");
        }
        assert_eq!(decode_key_path("not valid toml ["), vec!["not valid toml [".to_owned()]);
    }

    #[test]
    fn note_text_rules() {
        let mut s = Settings::default();
        assert!(s.note_text(Lang::De).unwrap().contains("eIDAS-VO"));
        assert!(s.note_text(Lang::En).unwrap().contains("eIDAS-Regulation"));
        s.signature.profile = SignatureProfile::Amtssignaturblock;
        assert!(s.note_text(Lang::De).unwrap().contains("amtssigniert"));
        s.signature.profile = SignatureProfile::BaseLogo;
        assert_eq!(s.note_text(Lang::De), None);
        s.signature.note = NoteSetting::Custom("  \t".into());
        assert_eq!(s.note_text(Lang::De), None);
        s.signature.note = NoteSetting::Custom(" Mein Hinweis ".into());
        assert_eq!(s.note_text(Lang::De).as_deref(), Some(" Mein Hinweis "));
        s.signature.note = NoteSetting::None;
        assert_eq!(s.note_text(Lang::De), None);
    }

    #[test]
    fn profile_id_from_settings() {
        let mut s = Settings::default();
        assert_eq!(s.pdfas_profile_id(Lang::De, false), "SIGNATURBLOCK_SMALL_DE_NOTE");
        s.signature.block_language = Some(Lang::En);
        assert_eq!(s.pdfas_profile_id(Lang::De, true), "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA");
        s.signature.note = NoteSetting::None;
        s.signature.pdfa_force = true;
        assert_eq!(s.pdfas_profile_id(Lang::De, false), "SIGNATURBLOCK_SMALL_EN_PDFA");
    }
}
