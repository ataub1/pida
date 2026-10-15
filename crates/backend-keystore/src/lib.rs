//! `pida-backend-keystore` – PKCS#12-Signatur (Modell A: Hash über die SignedAttributes →
//! Rohsignatur; Plan §3.7, §4.9; docs/analyse/02 §5).
//!
//! - `p12-keystore` 0.4.1: PKCS#12 inkl. Legacy-PBE (RC2-40, 3DES/SHA-1) und PBES2 (AES, 3DES,
//!   PRF HMAC-SHA-1/2). `p12-keystore` nutzt intern die RustCrypto-Folgegeneration; die Grenze zu
//!   `pida-cades` sind DER-Bytes (ADR 0001 §5.2): Zertifikate über
//!   `pida_cades::Certificate::from_der`, Schlüssel als PKCS#8-DER über
//!   [`pida_cades::LocalSigner`].
//! - **Ein-Passwort-Grenze:** `p12-keystore` entschlüsselt MAC, Zertifikate und Schlüssel mit
//!   **einem** Passwort. Abbildung von PDF-Over `KEYSTORE_STOREPASS`/`KEYSTORE_KEYPASS`
//!   (`-kspass`/`-kskeypass`):
//!   1. Schlüsselpasswort leer/fehlend oder gleich dem Store-Passwort → ein Versuch mit dem
//!      Store-Passwort.
//!   2. Abweichendes Schlüsselpasswort → zuerst Store-, dann Schlüsselpasswort als einziges Passwort
//!      (deckt vertauschte bzw. veraltete Angaben ab; wird geloggt, nie still verworfen).
//!   3. Bestätigt ein Passwort den MAC, scheitert aber an den Schlüsseln, ist der Speicher mit
//!      getrennten Passwörtern geschützt → [`KeystoreError::DifferentKeyPassword`] mit Anleitung zum
//!      Neu-Export (Plan §4.9; ein eigener Low-Level-Pfad wird erst nach Spike S2 gebaut).
//!   4. Sonst → [`KeystoreError::WrongPassword`] (der Aufrufer fragt erneut).
//! - Nicht lesbare Varianten (MD5-/DES-PBES1, RC4, PBMAC1, Public-Key-Modi) erkennt eine
//!   Vorprüfung **ohne Passwort** → [`KeystoreError::UnsupportedPbe`] mit dem Verfahrensnamen.
//! - JKS (`0xFEEDFEED`) / JCEKS (`0xCECECECE`): nur Erkennung + Konvertierungshinweis
//!   ([`conversion_hint`], `keytool -importkeystore … -deststoretype PKCS12`), kein eigener Reader.
//! - Verknüpfung Schlüssel ↔ Zertifikat selbst über `p12_keystore::Pkcs12Archive` wie Java
//!   `PKCS12KeyStore` (PDF-Over): `localKeyId`, sonst öffentlicher Schlüssel, sonst friendlyName
//!   (`link`). `p12-keystore` `KeyStore` (`Strict`) verwirft Schlüssel ohne `localKeyId`, die
//!   PDF-Over aber lädt.
//! - Alias: exakt, sonst ohne Groß-/Kleinschreibung (Java speichert PKCS#12-Aliasse klein); ohne
//!   Alias der einzige Schlüsseleintrag. Ohne friendlyName ist der Alias der Subject-Text – deshalb
//!   erscheint er weder in Logs noch in `Debug` (Regel 9).
//! - Entsperren ist über das `CancelToken` abbrechbar, auch während der PBKDF-Arbeit; Dateien über
//!   16 MiB werden nicht gelesen.
//! - Passwort-Speicherarten `none` / `memory` / `disk` → Schlüsselbund über [`PasswordCache`]
//!   (Implementierung für den Schlüsselbund liefert die Integration aus `pida-config`).
//!   Passwörter aus dem CLI-Overlay werden nie abgelegt (wie PDF-Over `KSState:119-129`).
//! - Algorithmen wie PDF-AS (RSA → sha256WithRSA; EC ≥ 512 → SHA-512; EC ≥ 256 inkl. P-384 →
//!   SHA-256; EC < 256 und DSA abgelehnt) über `pida_cades::select_algorithm`.
//! - Dialoge: fehlende Datei und fehlender Alias → [`NoticeCode::KeystoreFileMissing`] bzw.
//!   [`NoticeCode::KeystoreAliasMissing`] mit Wiederholen/Abbrechen (PDF-Over `KSState`), Abbrechen
//!   = [`BackendError::Cancelled`]; Passwort über [`InteractionBroker::secret`] (`retry` nach
//!   falschem Passwort). Endgültige Fehler (Format, PBE, Schlüssel) gehen ohne eigenen Dialog an den
//!   Workflow, der sie über `BackendError::code` anzeigt (keine Doppelmeldung).
//!
//! Ownership: Paket **B**.

mod inspect;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use p12_keystore::Pkcs12Archive;
use pida_backend_api::{
    BackendError, BackendId, BackendSession, CancelToken, Capabilities, CertChain, CertNeed, Environment, FactorSet,
    Interaction, InteractionBroker, InteractionError, Notice, NoticeAnswer, NoticeButtons, NoticeCode, NoticeKind,
    Progress, Qualification, SecretKind, SecretPrompt, ServerNeed, SessionContext, SignInput, SignJob, SignOutput,
    SignResult, SignatureBackend,
};
use pida_cades::{
    Certificate, CmsError, Digest, DigestAlg, KeyType, LocalSigner, RawSignature, RawSigner, SignatureAlg,
};
use zeroize::Zeroizing;

/// Erkanntes Keystore-Format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeystoreFormat {
    /// PKCS#12 (DER-`SEQUENCE`)
    Pkcs12,
    /// Java KeyStore (`0xFEEDFEED`)
    Jks,
    /// Java Cryptography Extension KeyStore (`0xCECECECE`)
    Jceks,
    /// unbekannt
    Unknown,
}

/// Format am Inhalt erkennen (Magic-Nummern, Plan §4.9).
pub fn detect_format(bytes: &[u8]) -> KeystoreFormat {
    match bytes.get(..4) {
        Some([0xFE, 0xED, 0xFE, 0xED]) => KeystoreFormat::Jks,
        Some([0xCE, 0xCE, 0xCE, 0xCE]) => KeystoreFormat::Jceks,
        Some([0x30, ..]) => KeystoreFormat::Pkcs12,
        _ => KeystoreFormat::Unknown,
    }
}

/// Format aus der Dateiendung wie PDF-Over (`.p12/.pkcs12/.pfx` → PKCS12, `.ks/.jks` → JCEKS).
pub fn format_from_extension(path: &Path) -> Option<KeystoreFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "p12" | "pkcs12" | "pfx" => Some(KeystoreFormat::Pkcs12),
        "ks" | "jks" => Some(KeystoreFormat::Jceks),
        _ => None,
    }
}

/// Konvertierungshinweis für JKS/JCEKS (Plan §4.9, Konvertierungsassistent): Befehlszeile für
/// `keytool` aus einer Java-Installation (z. B. dem JRE einer alten PDF-Over-Installation).
/// `None` für PKCS#12/unbekannt.
pub fn conversion_hint(format: KeystoreFormat, path: &Path) -> Option<String> {
    let src_type = match format {
        KeystoreFormat::Jks => "JKS",
        KeystoreFormat::Jceks => "JCEKS",
        _ => return None,
    };
    let dest = path.with_extension("p12");
    Some(format!(
        "keytool -importkeystore -srckeystore \"{}\" -srcstoretype {src_type} -destkeystore \"{}\" -deststoretype PKCS12",
        path.display(),
        dest.display()
    ))
}

/// Fehler des Keystores.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    /// Datei fehlt (`error.KeyStoreFileNotExist`). Anzeige nur mit Dateinamen (Regel 9).
    #[error("Keystore-Datei nicht gefunden: {}", display_name(.0))]
    FileNotFound(PathBuf),
    /// Falsches Passwort (MAC-Prüfung/Entschlüsselung).
    #[error("falsches Keystore-Passwort")]
    WrongPassword,
    /// Alias fehlt (`error.KeyStoreAliasExist`).
    #[error("Alias {0} nicht gefunden")]
    AliasNotFound(String),
    /// Eintrag ist kein Schlüssel.
    #[error("Alias {0} ist kein Schlüsseleintrag")]
    NotAKeyEntry(String),
    /// JKS/JCEKS: Konvertierung nötig.
    #[error("Format {0:?} nicht unterstützt – Konvertierung nach PKCS#12 nötig")]
    ConversionNeeded(KeystoreFormat),
    /// PBE-Verfahren nicht unterstützt (MD5-basiert, RC4, PBMAC1 …).
    #[error("PKCS#12-Verschlüsselung nicht unterstützt: {0} – bitte neu exportieren")]
    UnsupportedPbe(String),
    /// Abweichendes Schlüsselpasswort (PKCS#12 hat nur ein Passwort).
    #[error("abweichendes Schlüsselpasswort – bitte PKCS#12 mit einheitlichem Passwort exportieren")]
    DifferentKeyPassword,
    /// Schlüsseltyp nicht unterstützt (EC < 256, DSA, Ed25519 …).
    #[error("Schlüssel nicht unterstützt: {0}")]
    UnsupportedKey(String),
    /// CMS/Zertifikat.
    #[error(transparent)]
    Cms(#[from] pida_cades::CmsError),
    /// E/A.
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
    /// Keine lesbare PKCS#12-Datei (Struktur, BER, unbekanntes Format).
    #[error("Keystore nicht lesbar: {0}")]
    Malformed(String),
    /// Der Keystore enthält keinen Schlüsseleintrag (nur Zertifikate).
    #[error("Keystore enthält keinen Schlüssel mit Zertifikat")]
    NoKeyEntry,
    /// Mehrere Schlüsseleinträge, aber kein Alias angegeben.
    #[error("Keystore enthält {0} Schlüsseleinträge – bitte Alias angeben")]
    AliasRequired(usize),
    /// Privater Schlüssel passt nicht zum Zertifikat des Eintrags.
    #[error("privater Schlüssel passt nicht zum Zertifikat")]
    KeyCertificateMismatch,
}

impl KeystoreError {
    /// Stabiler Fehlercode für i18n (`keystore-…`) und Diagnose (ADR 0001 Regel 11).
    pub fn code(&self) -> &'static str {
        match self {
            KeystoreError::FileNotFound(_) => "keystore-file-not-found",
            KeystoreError::WrongPassword => "keystore-wrong-password",
            KeystoreError::AliasNotFound(_) => "keystore-alias-not-found",
            KeystoreError::NotAKeyEntry(_) => "keystore-not-a-key-entry",
            KeystoreError::ConversionNeeded(_) => "keystore-conversion-needed",
            KeystoreError::UnsupportedPbe(_) => "keystore-unsupported-pbe",
            KeystoreError::DifferentKeyPassword => "keystore-different-key-password",
            KeystoreError::UnsupportedKey(_) => "keystore-unsupported-key",
            KeystoreError::Cms(_) => "keystore-cms",
            KeystoreError::Io(_) => "keystore-io",
            KeystoreError::Malformed(_) => "keystore-malformed",
            KeystoreError::NoKeyEntry => "keystore-no-key-entry",
            KeystoreError::AliasRequired(_) => "keystore-alias-required",
            KeystoreError::KeyCertificateMismatch => "keystore-key-mismatch",
        }
    }
}

impl From<KeystoreError> for BackendError {
    fn from(e: KeystoreError) -> Self {
        match e {
            KeystoreError::WrongPassword => BackendError::WrongPassword,
            KeystoreError::ConversionNeeded(f) => {
                BackendError::UnsupportedKeystoreFormat { format: format!("{f:?}").to_uppercase() }
            }
            other => BackendError::KeystoreDetail { code: other.code().to_string(), message: other.to_string() },
        }
    }
}

/// Dateiname ohne Verzeichnis (Masken, Meldungen; keine Nutzerpfade in Logs).
fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Keystore".into())
}

/// Speicherart der Passwörter (`KEYSTORE_PASS_STORE_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PasswordStorage {
    /// jedes Mal fragen
    None,
    /// bis Programmende im RAM
    #[default]
    Memory,
    /// OS-Schlüsselbund (früher Klartext auf Platte)
    Keychain,
}

impl PasswordStorage {
    /// Aus dem PDF-Over-Wert `KEYSTORE_PASS_STORE_TYPE` (`none`, `memory`, `disk`; `disk` →
    /// Schlüsselbund). Unbekannt → `None` (Aufrufer entscheidet über den Default, analyse/02 §5.1).
    pub fn from_pdfover(value: &str) -> Option<PasswordStorage> {
        match value.trim() {
            "none" => Some(PasswordStorage::None),
            "memory" => Some(PasswordStorage::Memory),
            "disk" => Some(PasswordStorage::Keychain),
            _ => None,
        }
    }

    /// PDF-Over-Wert (`none`, `memory`, `disk`).
    pub fn as_pdfover(self) -> &'static str {
        match self {
            PasswordStorage::None => "none",
            PasswordStorage::Memory => "memory",
            PasswordStorage::Keychain => "disk",
        }
    }
}

/// Welches Passwort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PasswordKind {
    /// Store-Passwort
    Store,
    /// Schlüsselpasswort
    Key,
}

/// Ablage für Keystore-Passwörter (RAM oder Schlüsselbund – Adapter in der Integration).
pub trait PasswordCache: Send + Sync + std::fmt::Debug {
    /// Liest.
    fn get(&self, keystore: &Path, kind: PasswordKind) -> Option<Zeroizing<String>>;
    /// Merkt.
    fn put(&self, keystore: &Path, kind: PasswordKind, password: &str);
    /// Vergisst (falsches Passwort, Abbruch).
    fn forget(&self, keystore: &Path, kind: PasswordKind);
}

/// Speicherart `none`.
#[derive(Debug, Default)]
pub struct NoPasswordCache;

impl PasswordCache for NoPasswordCache {
    fn get(&self, _: &Path, _: PasswordKind) -> Option<Zeroizing<String>> {
        None
    }
    fn put(&self, _: &Path, _: PasswordKind, _: &str) {}
    fn forget(&self, _: &Path, _: PasswordKind) {}
}

/// Speicherart `memory`.
#[derive(Default)]
pub struct MemoryPasswordCache {
    map: std::sync::Mutex<std::collections::HashMap<(PathBuf, PasswordKind), Zeroizing<String>>>,
}

impl std::fmt::Debug for MemoryPasswordCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.map.lock().map(|m| m.len()).unwrap_or(0);
        f.debug_struct("MemoryPasswordCache").field("entries", &n).finish()
    }
}

impl PasswordCache for MemoryPasswordCache {
    fn get(&self, keystore: &Path, kind: PasswordKind) -> Option<Zeroizing<String>> {
        self.map.lock().expect("poisoned").get(&(keystore.to_path_buf(), kind)).cloned()
    }
    fn put(&self, keystore: &Path, kind: PasswordKind, password: &str) {
        self.map.lock().expect("poisoned").insert((keystore.to_path_buf(), kind), Zeroizing::new(password.to_owned()));
    }
    fn forget(&self, keystore: &Path, kind: PasswordKind) {
        self.map.lock().expect("poisoned").remove(&(keystore.to_path_buf(), kind));
    }
}

/// Konfiguration.
#[derive(Clone)]
pub struct KeystoreConfig {
    /// Datei (`KEYSTORE_FILE` / `-ks`).
    pub path: PathBuf,
    /// angegebener Typ (`KEYSTORE_TYPE` / `-kstype`), nur Diagnose – erkannt wird am Inhalt.
    pub declared_type: Option<String>,
    /// Alias (`KEYSTORE_ALIAS` / `-ksalias`); `None` = einziger Schlüsseleintrag.
    pub alias: Option<String>,
    /// Store-Passwort aus CLI-Overlay (`-kspass`, `--kspass-stdin/-env`).
    pub store_password: Option<Zeroizing<String>>,
    /// Schlüsselpasswort aus CLI-Overlay (`-kskeypass`).
    pub key_password: Option<Zeroizing<String>>,
    /// Speicherart.
    pub storage: PasswordStorage,
    /// Ablage (passend zur Speicherart).
    pub cache: Arc<dyn PasswordCache>,
}

impl KeystoreConfig {
    /// Konfiguration mit Defaults: kein Alias, keine Overlay-Passwörter, Speicherart `memory` mit
    /// eigenem [`MemoryPasswordCache`].
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            declared_type: None,
            alias: None,
            store_password: None,
            key_password: None,
            storage: PasswordStorage::Memory,
            cache: Arc::new(MemoryPasswordCache::default()),
        }
    }
}

impl std::fmt::Debug for KeystoreConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // nur Dateiname (Nutzerpfade enthalten oft den Namen) und Länge des Alias (Regel 9)
        f.debug_struct("KeystoreConfig")
            .field("file", &display_name(&self.path))
            .field("declared_type", &self.declared_type)
            .field("alias_len", &self.alias.as_ref().map(|a| a.chars().count()))
            .field("store_password", &self.store_password.as_ref().map(|_| "***"))
            .field("key_password", &self.key_password.as_ref().map(|_| "***"))
            .field("storage", &self.storage)
            .field("cache", &self.cache)
            .finish()
    }
}

/// Eintrag eines Keystores (Einstellungen: „Keystore laden“ füllt die Alias-Liste).
#[derive(Clone, PartialEq, Eq)]
pub struct AliasInfo {
    /// Alias
    pub alias: String,
    /// Schlüsseleintrag (sonst nur Zertifikat)
    pub is_key_entry: bool,
    /// Subject des Zertifikats
    pub subject: Option<String>,
}

impl std::fmt::Debug for AliasInfo {
    /// Subject ist Personendatum (Regel 9) – nur „vorhanden“; ebenso der Alias, der ohne
    /// friendlyName der Subject-Text ist (nur Länge).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AliasInfo")
            .field("alias_len", &self.alias.chars().count())
            .field("is_key_entry", &self.is_key_entry)
            .field("subject", &self.subject.as_ref().map(|_| "<redigiert>"))
            .finish()
    }
}

/// Listet die Einträge (Passwortprüfung inklusive), sortiert nach Alias.
pub fn list_aliases(bytes: &[u8], password: &str) -> Result<Vec<AliasInfo>, KeystoreError> {
    let (archive, _) = open_store(bytes, &[password])?;
    let store = link(archive)?;
    Ok(store
        .entries
        .iter()
        .map(|(alias, entry)| {
            let (is_key_entry, cert) = match entry {
                Entry::Key { chain, .. } => (true, chain.first()),
                Entry::Cert(c) => (false, Some(c)),
                Entry::Secret => (false, None),
            };
            AliasInfo { alias: alias.clone(), is_key_entry, subject: cert.map(Certificate::subject_string) }
        })
        .collect())
}

/// Format prüfen und Vorprüfung ohne Passwort.
fn precheck(bytes: &[u8]) -> Result<inspect::Pkcs12Info, KeystoreError> {
    match detect_format(bytes) {
        KeystoreFormat::Pkcs12 => {}
        f @ (KeystoreFormat::Jks | KeystoreFormat::Jceks) => return Err(KeystoreError::ConversionNeeded(f)),
        KeystoreFormat::Unknown => return Err(KeystoreError::Malformed("keine PKCS#12-Datei".into())),
    }
    let info = inspect::inspect(bytes)?;
    if !info.unsupported.is_empty() {
        return Err(KeystoreError::UnsupportedPbe(info.unsupported.join(", ")));
    }
    Ok(info)
}

enum Attempt {
    Opened(Pkcs12Archive),
    /// MAC passt nicht → Passwort falsch.
    MacFailed,
    /// Entschlüsselung nach (bzw. ohne) MAC fehlgeschlagen.
    DecryptFailed,
    /// vom Passwort unabhängiger Fehler
    Fatal(KeystoreError),
}

fn try_open(bytes: &[u8], password: &str) -> Attempt {
    use p12_keystore::error::Error as E;
    match Pkcs12Archive::from_pkcs12(bytes, password) {
        Ok(archive) => Attempt::Opened(archive),
        Err(E::MacError(_)) => Attempt::MacFailed,
        Err(E::Pkcs5Error(_) | E::UnpadError | E::InvalidPrivateKey | E::DerError(_) | E::InvalidLength) => {
            Attempt::DecryptFailed
        }
        Err(E::UnsupportedEncryptionScheme) => {
            Attempt::Fatal(KeystoreError::UnsupportedPbe("Verschlüsselungsverfahren".into()))
        }
        Err(E::UnsupportedMacAlgorithm) => Attempt::Fatal(KeystoreError::UnsupportedPbe("MAC-Verfahren".into())),
        Err(E::InvalidParameters) => {
            Attempt::Fatal(KeystoreError::UnsupportedPbe("Parameter (z. B. mehr als 1 000 000 Iterationen)".into()))
        }
        Err(E::InvalidVersion | E::UnsupportedContentType | E::InvalidData) => {
            Attempt::Fatal(KeystoreError::Malformed("PKCS#12-Struktur".into()))
        }
        Err(E::X509Error(_) | E::UnsupportedCertificateType) => {
            Attempt::Fatal(KeystoreError::Malformed("Zertifikat im Keystore unlesbar".into()))
        }
        Err(E::IoError(e)) => Attempt::Fatal(KeystoreError::Io(e)),
    }
}

/// Öffnet den Speicher mit den Kandidaten in Reihenfolge (Ein-Passwort-Abbildung, siehe Crate-Doku).
/// Liefert den Index des erfolgreichen Kandidaten.
fn open_store(bytes: &[u8], candidates: &[&str]) -> Result<(Pkcs12Archive, usize), KeystoreError> {
    let info = precheck(bytes)?;
    let mut mac_ok_decrypt_failed = false;
    for (i, pw) in candidates.iter().enumerate() {
        match try_open(bytes, pw) {
            Attempt::Opened(archive) => return Ok((archive, i)),
            Attempt::MacFailed => {}
            Attempt::DecryptFailed => mac_ok_decrypt_failed |= info.has_mac,
            Attempt::Fatal(e) => return Err(e),
        }
    }
    if mac_ok_decrypt_failed { Err(KeystoreError::DifferentKeyPassword) } else { Err(KeystoreError::WrongPassword) }
}

/// Passwort-Kandidaten aus Store- und Schlüsselpasswort (Ein-Passwort-Grenze).
fn candidates<'a>(store: &'a str, key: Option<&'a str>) -> Vec<&'a str> {
    match key {
        Some(k) if !k.is_empty() && k != store => vec![store, k],
        _ => vec![store],
    }
}

/// Eintrag nach der Verknüpfung der PKCS#12-Bags (siehe [`link`]).
enum Entry {
    /// Privater Schlüssel (PKCS#8-DER) mit Kette, Endzertifikat zuerst.
    Key { pkcs8: Zeroizing<Vec<u8>>, chain: Vec<Certificate> },
    /// Vertrauenswürdiges Zertifikat (Java `trustedCertEntry`).
    Cert(Certificate),
    /// Geheimer Schlüssel (für Signaturen nicht nutzbar).
    Secret,
}

/// Verknüpfte Einträge, nach Alias sortiert.
struct LinkedStore {
    entries: std::collections::BTreeMap<String, Entry>,
}

impl LinkedStore {
    /// Fügt ein; Kollisionen bekommen wie bei `p12-keystore` die Endung `#2`, `#3` ….
    fn insert(&mut self, alias: String, entry: Entry) {
        let mut candidate = alias.clone();
        let mut n = 2;
        while self.entries.contains_key(&candidate) {
            candidate = format!("{alias}#{n}");
            n += 1;
        }
        self.entries.insert(candidate, entry);
    }
}

/// Verknüpft Schlüssel und Zertifikate eines entschlüsselten PKCS#12 wie Java `PKCS12KeyStore`
/// (PDF-Over/PDF-AS laden Keystores über Java), aber ohne dessen Behelfe:
///
/// 1. Zertifikat mit gleicher `localKeyId` (bei mehreren bevorzugt das mit gleichem friendlyName) –
///    der Normalfall (OpenSSL, Java, Windows, NSS). Ein so verknüpftes, aber nicht passendes
///    Zertifikat bleibt verknüpft und führt zu [`KeystoreError::KeyCertificateMismatch`].
/// 2. sonst das Zertifikat mit passendem öffentlichen Schlüssel (Java: Ein-Schlüssel-Regel
///    „localKeyId fehlt → 01“, die hier robuster über den Schlüssel selbst erfolgt);
/// 3. sonst das Zertifikat mit gleichem friendlyName (Java `aliasMatch`, ohne Groß-/Kleinschreibung).
///
/// `p12-keystore` (`KeyStore`, Modus `Strict`) verwirft Schlüssel ohne `localKeyId` stillschweigend;
/// solche Dateien öffnet PDF-Over aber. Schlüssel ohne Zertifikat werden wie dort übergangen.
/// Kette: vom Endzertifikat über Aussteller = Subject (DER-Vergleich der Namen) bis zur
/// selbstsignierten Wurzel; Java-Vertrauensanker (`trusted`) zählen nicht dazu. Alias: friendlyName
/// des Schlüssels, sonst Subject des Zertifikats (wie `p12-keystore`).
fn link(archive: Pkcs12Archive) -> Result<LinkedStore, KeystoreError> {
    struct CertInfo {
        friendly_name: Option<String>,
        local_key_id: Option<Vec<u8>>,
        trusted: bool,
        cert: Option<Certificate>,
    }
    let certs: Vec<CertInfo> = archive
        .certs
        .into_iter()
        .map(|b| CertInfo {
            cert: Certificate::from_der(b.cert.as_der()).ok(),
            friendly_name: b.friendly_name,
            local_key_id: b.local_key_id,
            trusted: b.trusted,
        })
        .collect();
    let name_eq = |a: &Option<String>, b: &str| a.as_deref().is_some_and(|a| a.to_lowercase() == b.to_lowercase());
    let mut store = LinkedStore { entries: Default::default() };

    for key in archive.keys {
        let pkcs8 = Zeroizing::new(key.key.as_der().to_vec());
        let by_id = key.local_key_id.as_ref().and_then(|id| {
            let matches = |c: &&CertInfo| c.local_key_id.as_deref() == Some(id.as_ref());
            let mut with_id = certs.iter().filter(matches);
            let first = with_id.next()?;
            Some(
                key.friendly_name
                    .as_deref()
                    .and_then(|n| std::iter::once(first).chain(with_id).find(|c| name_eq(&c.friendly_name, n)))
                    .unwrap_or(first),
            )
        });
        let candidates = || certs.iter().filter(|c| !c.trusted);
        let leaf_info = by_id
            .or_else(|| {
                candidates().find(|c| {
                    c.cert.as_ref().is_some_and(|cert| {
                        LocalSigner::from_pkcs8_der(&pkcs8, pida_cades::CertChain::new(cert.clone())).is_ok()
                    })
                })
            })
            .or_else(|| key.friendly_name.as_deref().and_then(|n| candidates().find(|c| name_eq(&c.friendly_name, n))));
        let Some(leaf_info) = leaf_info else { continue };
        let leaf =
            leaf_info.cert.clone().ok_or_else(|| KeystoreError::Malformed("Zertifikat im Keystore unlesbar".into()))?;
        let mut chain = vec![leaf];
        loop {
            let current = chain.last().expect("Kette nicht leer").x509().tbs_certificate.clone();
            if current.issuer == current.subject {
                break;
            }
            let issuer = candidates()
                .filter_map(|c| c.cert.as_ref())
                .find(|c| c.x509().tbs_certificate.subject == current.issuer && !chain.contains(c));
            match issuer {
                Some(c) => chain.push(c.clone()),
                None => break,
            }
        }
        let alias = key.friendly_name.clone().unwrap_or_else(|| chain[0].subject_string());
        store.insert(alias, Entry::Key { pkcs8, chain });
    }
    for c in certs.iter().filter(|c| c.local_key_id.is_none() && c.trusted) {
        if let Some(cert) = &c.cert {
            store.insert(c.friendly_name.clone().unwrap_or_else(|| cert.subject_string()), Entry::Cert(cert.clone()));
        }
    }
    for s in archive.secrets {
        let alias = s
            .friendly_name
            .unwrap_or_else(|| s.key.local_key_id().as_ref().iter().map(|b| format!("{b:02x}")).collect::<String>());
        store.insert(alias, Entry::Secret);
    }
    Ok(store)
}

/// Geladener Schlüssel; signiert lokal.
pub struct LoadedKey {
    chain: CertChain,
    alias: String,
    algorithm: SignatureAlg,
    signer: LocalSigner,
}

impl std::fmt::Debug for LoadedKey {
    /// Ohne Alias: Fehlt der friendlyName, ist der Alias der Subject-Text des Zertifikats
    /// (Personendaten, Regel 9).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedKey")
            .field("alias_len", &self.alias.chars().count())
            .field("algorithm", &self.algorithm)
            .field("signer", &self.chain.signer)
            .finish_non_exhaustive()
    }
}

impl LoadedKey {
    /// Öffnet einen PKCS#12-Keystore (ein Passwort für Speicher und Schlüssel).
    pub fn open_pkcs12(bytes: &[u8], password: &str, alias: Option<&str>) -> Result<LoadedKey, KeystoreError> {
        Self::open_pkcs12_with(bytes, password, None, alias)
    }

    /// Öffnet einen PKCS#12-Keystore mit Store- und optionalem Schlüsselpasswort nach der
    /// dokumentierten Ein-Passwort-Abbildung (Crate-Doku).
    pub fn open_pkcs12_with(
        bytes: &[u8],
        store_password: &str,
        key_password: Option<&str>,
        alias: Option<&str>,
    ) -> Result<LoadedKey, KeystoreError> {
        let (archive, used) = open_store(bytes, &candidates(store_password, key_password))?;
        if used > 0 {
            tracing::warn!(
                "Keystore mit dem Schlüsselpasswort als einzigem Passwort geöffnet (Store-Passwort passte nicht)"
            );
        } else if key_password.is_some_and(|k| !k.is_empty() && k != store_password) {
            tracing::warn!(
                "abweichendes Schlüsselpasswort nicht benötigt: PKCS#12 verwendet ein einheitliches Passwort"
            );
        }
        let store = link(archive)?;
        let (alias, pkcs8, chain) = select_entry(&store, alias)?;
        let signer_cert = chain.first().cloned().ok_or(KeystoreError::NoKeyEntry)?;
        let mut intermediates = Vec::new();
        for c in chain.iter().skip(1) {
            // selbstsignierte Wurzeln gehören nicht in die Kette des CMS
            if c.subject_string() != c.issuer_string() && *c != signer_cert {
                intermediates.push(c.clone());
            }
        }
        let cert_chain = CertChain { signer: signer_cert, intermediates };
        let signer = LocalSigner::from_pkcs8_der(pkcs8, cert_chain.clone()).map_err(|e| match e {
            CmsError::KeyMismatch => KeystoreError::KeyCertificateMismatch,
            CmsError::UnsupportedAlgorithm(m) => KeystoreError::UnsupportedKey(m),
            other => KeystoreError::Cms(other),
        })?;
        drop(store);
        let algorithm = signer.algorithm();
        Ok(LoadedKey { chain: cert_chain, alias, algorithm, signer })
    }

    /// Alias.
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Schlüsseltyp.
    pub fn key_type(&self) -> KeyType {
        self.signer.key_type()
    }
}

/// Alias-Auswahl: exakt, sonst ohne Groß-/Kleinschreibung; ohne Alias der einzige Schlüssel.
fn select_entry<'s>(
    store: &'s LinkedStore,
    alias: Option<&str>,
) -> Result<(String, &'s [u8], &'s [Certificate]), KeystoreError> {
    let as_key = |a: &String, e: &'s Entry| match e {
        Entry::Key { pkcs8, chain } => Ok((a.clone(), pkcs8.as_slice(), chain.as_slice())),
        _ => Err(KeystoreError::NotAKeyEntry(a.clone())),
    };
    match alias {
        Some(wanted) => {
            let lower = wanted.to_lowercase();
            let found = store
                .entries
                .get_key_value(wanted)
                .or_else(|| store.entries.iter().find(|(a, _)| a.to_lowercase() == lower));
            match found {
                None => Err(KeystoreError::AliasNotFound(wanted.to_owned())),
                Some((a, e)) => as_key(a, e),
            }
        }
        None => {
            let mut keys = store.entries.iter().filter(|(_, e)| matches!(e, Entry::Key { .. }));
            match (keys.next(), keys.count()) {
                (None, _) => Err(KeystoreError::NoKeyEntry),
                (Some((a, e)), 0) => as_key(a, e),
                (Some(_), more) => Err(KeystoreError::AliasRequired(more + 1)),
            }
        }
    }
}

impl RawSigner for LoadedKey {
    fn chain(&self) -> &CertChain {
        &self.chain
    }

    fn algorithm(&self) -> SignatureAlg {
        self.algorithm
    }

    fn sign_digest(&self, digest: &Digest) -> pida_cades::Result<RawSignature> {
        self.signer.sign_digest(digest)
    }
}

/// Keystore-Backend.
#[derive(Debug)]
pub struct KeystoreBackend {
    config: KeystoreConfig,
}

impl KeystoreBackend {
    /// Neues Backend.
    pub fn new(config: KeystoreConfig) -> Self {
        Self { config }
    }

    /// Konfiguration.
    pub fn config(&self) -> &KeystoreConfig {
        &self.config
    }
}

/// Hashverfahren, die der Kern laut [`pida_cades::select_algorithm`] anfordern kann.
const ADVERTISED_ALGS: [DigestAlg; 2] = [DigestAlg::Sha256, DigestAlg::Sha512];

#[async_trait]
impl SignatureBackend for KeystoreBackend {
    fn id(&self) -> BackendId {
        BackendId::Keystore
    }

    /// Verfügbar auf allen Plattformen, wenn der Keystore konfiguriert ist und die Richtlinie ihn
    /// erlaubt (F-52). Modell A: Hash rein, Rohsignatur raus; Zertifikat vorab; Passwortabfrage;
    /// beliebig viele Dokumente je Freigabe; nichts verlässt das Gerät.
    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        if !env.allows(BackendId::Keystore) || !env.keystore_configured {
            return None;
        }
        Some(Capabilities {
            input: SignInput::Digest { algs: ADVERTISED_ALGS.to_vec() },
            output: SignOutput::RawSignature,
            certificate: CertNeed::Required,
            interaction: Interaction::Secret,
            server: ServerNeed::None,
            max_batch: u16::MAX,
            login_once_per_batch: true,
            document_leaves_device: false,
            qualification: Qualification::AdES,
            factors: FactorSet::default(),
            cms_reserve: None,
        })
    }

    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        let key = unlock(&self.config, &ctx, ui.as_ref()).await?;
        // ohne Alias: kann der Subject-Text des Zertifikats sein (Regel 9)
        tracing::debug!(algorithm = ?key.algorithm(), "Keystore entsperrt");
        Ok(Box::new(KeystoreSession { key: Some(Arc::new(key)), cancel: ctx.cancel.clone(), ui }))
    }
}

/// Herkunft eines Passworts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Overlay,
    Cache,
    Prompt,
}

fn check_cancel(ctx: &SessionContext) -> Result<(), BackendError> {
    if ctx.cancel.is_cancelled() { Err(BackendError::Cancelled) } else { Ok(()) }
}

/// Wartet auf ein Broker-Future oder den Abbruch.
async fn or_cancel<T>(ctx: &SessionContext, fut: impl std::future::Future<Output = T>) -> Result<T, BackendError> {
    tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => Err(BackendError::Cancelled),
        v = fut => Ok(v),
    }
}

/// Datei lesen, Format prüfen, Passwort beschaffen und Schlüssel laden (PDF-Over `KSState`).
async fn unlock(
    cfg: &KeystoreConfig,
    ctx: &SessionContext,
    ui: &dyn InteractionBroker,
) -> Result<LoadedKey, BackendError> {
    let name = display_name(&cfg.path);
    let use_cache = cfg.storage != PasswordStorage::None;
    let key_password: Option<Zeroizing<String>> =
        cfg.key_password.clone().or_else(|| if use_cache { cfg.cache.get(&cfg.path, PasswordKind::Key) } else { None });
    let mut store_password: Option<(Zeroizing<String>, Source)> =
        cfg.store_password.clone().map(|p| (p, Source::Overlay)).or_else(|| {
            if use_cache { cfg.cache.get(&cfg.path, PasswordKind::Store).map(|p| (p, Source::Cache)) } else { None }
        });
    let mut retry = false;

    'file: loop {
        check_cancel(ctx)?;
        // 1. Datei (fehlt → Wiederholen/Abbrechen)
        let bytes = match read_keystore_file(&cfg.path) {
            Ok(b) => Arc::new(Zeroizing::new(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let err = KeystoreError::FileNotFound(cfg.path.clone());
                if ctx.headless {
                    return Err(err.into());
                }
                let notice = Notice {
                    kind: NoticeKind::Error,
                    code: NoticeCode::KeystoreFileMissing,
                    text: Some(name.clone()),
                    buttons: NoticeButtons::RetryCancel,
                };
                match or_cancel(ctx, ui.notice(notice)).await? {
                    Ok(NoticeAnswer::Retry) => continue 'file,
                    Ok(_) | Err(InteractionError::Cancelled) => return Err(BackendError::Cancelled),
                    Err(InteractionError::Unsupported) => return Err(err.into()),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::FileTooLarge => {
                return Err(KeystoreError::Malformed(format!(
                    "Datei größer als {} MiB – kein PKCS#12-Keystore",
                    MAX_KEYSTORE_FILE >> 20
                ))
                .into());
            }
            Err(e) => return Err(KeystoreError::Io(e).into()),
        };
        // 2. Format und Verfahren (ohne Passwort)
        if let Some(declared) = &cfg.declared_type
            && !declared.eq_ignore_ascii_case("PKCS12")
            && detect_format(&bytes) == KeystoreFormat::Pkcs12
        {
            tracing::debug!(
                declared = declared.as_str(),
                "Keystore-Typ laut Konfiguration abweichend; Inhalt ist PKCS#12"
            );
        }
        precheck(&bytes)?;

        // 3. Passwort und Schlüssel
        loop {
            check_cancel(ctx)?;
            let (password, source) = match store_password.take() {
                Some(p) => p,
                None => {
                    if ctx.headless {
                        return Err(BackendError::Unavailable(
                            "Keystore-Passwort fehlt (ohne Oberfläche: -kspass, --kspass-stdin oder --kspass-env)"
                                .into(),
                        ));
                    }
                    let prompt = SecretPrompt { kind: SecretKind::KeystorePassword, subject: name.clone(), retry };
                    match or_cancel(ctx, ui.secret(prompt)).await? {
                        Ok(p) => (p, Source::Prompt),
                        Err(InteractionError::Cancelled) => return Err(BackendError::Cancelled),
                        Err(InteractionError::Unsupported) => {
                            return Err(BackendError::Unavailable("Passwortabfrage nicht verfügbar".into()));
                        }
                    }
                }
            };
            // Abbruch auch während der PBKDF-Arbeit (bis 10⁶ Iterationen je Bag): Das Ergebnis des
            // Blocking-Tasks wird dann verworfen, der Schlüssel beim Drop genullt.
            let result =
                or_cancel(ctx, load_blocking(bytes.clone(), password.clone(), key_password.clone(), cfg.alias.clone()))
                    .await?;
            match result {
                Ok(key) => {
                    if use_cache && source == Source::Prompt {
                        cfg.cache.put(&cfg.path, PasswordKind::Store, &password);
                    }
                    return Ok(key);
                }
                Err(KeystoreError::WrongPassword) => {
                    if source == Source::Cache {
                        cfg.cache.forget(&cfg.path, PasswordKind::Store);
                    }
                    if ctx.headless {
                        return Err(BackendError::WrongPassword);
                    }
                    tracing::info!("falsches Keystore-Passwort – erneute Abfrage");
                    retry = true;
                }
                Err(KeystoreError::AliasNotFound(alias)) => {
                    let err = KeystoreError::AliasNotFound(alias);
                    if ctx.headless {
                        return Err(err.into());
                    }
                    let notice = Notice {
                        kind: NoticeKind::Error,
                        code: NoticeCode::KeystoreAliasMissing,
                        text: cfg.alias.clone(),
                        buttons: NoticeButtons::RetryCancel,
                    };
                    match or_cancel(ctx, ui.notice(notice)).await? {
                        Ok(NoticeAnswer::Retry) => {
                            // Passwort war richtig – behalten, Datei neu lesen (wie PDF-Over: gleicher Alias)
                            store_password = Some((password, source));
                            continue 'file;
                        }
                        Ok(_) | Err(InteractionError::Cancelled) => return Err(BackendError::Cancelled),
                        Err(InteractionError::Unsupported) => return Err(err.into()),
                    }
                }
                Err(other) => return Err(other.into()),
            }
        }
    }
}

/// Obergrenze für Keystore-Dateien: PKCS#12 mit Schlüssel und Kette sind wenige KiB groß; eine
/// versehentlich gewählte große Datei soll nicht vollständig in den Speicher gelesen werden.
const MAX_KEYSTORE_FILE: u64 = 16 << 20;

/// Liest die Keystore-Datei mit Größengrenze ([`std::io::ErrorKind::FileTooLarge`] bei Überschreitung).
fn read_keystore_file(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let file = std::fs::File::open(path)?;
    let mut out = Vec::new();
    file.take(MAX_KEYSTORE_FILE + 1).read_to_end(&mut out)?;
    if out.len() as u64 > MAX_KEYSTORE_FILE {
        return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
    }
    Ok(out)
}

/// Lädt den Schlüssel; die PBKDF-Arbeit (bis 10⁶ Iterationen) läuft unter Tokio im Blocking-Pool.
async fn load_blocking(
    bytes: Arc<Zeroizing<Vec<u8>>>,
    store: Zeroizing<String>,
    key: Option<Zeroizing<String>>,
    alias: Option<String>,
) -> Result<LoadedKey, KeystoreError> {
    let work = move || LoadedKey::open_pkcs12_with(&bytes, &store, key.as_ref().map(|k| k.as_str()), alias.as_deref());
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::spawn_blocking(work)
            .await
            .map_err(|e| KeystoreError::Io(std::io::Error::other(format!("Keystore-Laden abgebrochen: {e}"))))?
    } else {
        work()
    }
}

/// Offene Keystore-Sitzung.
pub struct KeystoreSession {
    key: Option<Arc<LoadedKey>>,
    cancel: CancelToken,
    ui: Arc<dyn InteractionBroker>,
}

impl std::fmt::Debug for KeystoreSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeystoreSession").field("key", &self.key).finish_non_exhaustive()
    }
}

impl KeystoreSession {
    fn key(&self) -> Result<Arc<LoadedKey>, BackendError> {
        self.key.clone().ok_or(BackendError::Cancelled)
    }
}

#[async_trait]
impl BackendSession for KeystoreSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        Ok(self.key()?.chain().clone())
    }

    /// Nur [`SignJob::Digest`] (Modell A); signiert mit genau `job.alg` (SHA-256/384/512) und prüft
    /// jede Rohsignatur selbst gegen das Zertifikat (wie PDF-AS `PKS:192-193`).
    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        let key = self.key()?;
        let total = jobs.len();
        let mut out = Vec::with_capacity(total);
        self.ui.progress(Progress::Signing);
        for (i, job) in jobs.into_iter().enumerate() {
            if self.cancel.is_cancelled() {
                return Err(BackendError::Cancelled);
            }
            if total > 1 {
                self.ui.progress(Progress::Document { index: i + 1, total });
            }
            let job = match job {
                SignJob::Digest(job) => job,
                SignJob::Document(_) => {
                    return Err(BackendError::Internal(
                        "Keystore signiert nur Hashwerte (Modell A), kein Dokument".into(),
                    ));
                }
            };
            if !matches!(job.alg, DigestAlg::Sha256 | DigestAlg::Sha384 | DigestAlg::Sha512) {
                return Err(BackendError::Internal(format!("Hashverfahren {} nicht unterstützt", job.alg.name())));
            }
            if job.digest.len() != job.alg.output_len() {
                return Err(BackendError::Internal(format!(
                    "Hashwert für {} hat {} statt {} Bytes",
                    job.alg.name(),
                    job.digest.len(),
                    job.alg.output_len()
                )));
            }
            let digest = Digest { alg: job.alg, value: job.digest };
            let signature = key.sign_digest(&digest).map_err(|e| BackendError::Internal(e.to_string()))?;
            pida_cades::verify_raw(&key.chain().signer, &digest, &signature)
                .map_err(|e| BackendError::Internal(format!("Selbstprüfung der Signatur fehlgeschlagen: {e}")))?;
            out.push(SignResult::Raw { signature, chain: key.chain().clone() });
        }
        Ok(out)
    }

    /// Gibt den geladenen Schlüssel frei; danach liefern `certificate`/`sign`
    /// [`BackendError::Cancelled`]. Gemerkte Passwörter bleiben (Speicherart der Nutzerin).
    async fn cancel(&mut self) {
        self.key = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_detection() {
        assert_eq!(detect_format(&[0xFE, 0xED, 0xFE, 0xED, 0]), KeystoreFormat::Jks);
        assert_eq!(detect_format(&[0xCE, 0xCE, 0xCE, 0xCE]), KeystoreFormat::Jceks);
        assert_eq!(detect_format(&[0x30, 0x82, 0x01, 0x00]), KeystoreFormat::Pkcs12);
        assert_eq!(detect_format(&[0x30]), KeystoreFormat::Unknown);
        assert_eq!(detect_format(b"PK\x03\x04"), KeystoreFormat::Unknown);
        assert_eq!(format_from_extension(Path::new("a/B.PFX")), Some(KeystoreFormat::Pkcs12));
        assert_eq!(format_from_extension(Path::new("x.jks")), Some(KeystoreFormat::Jceks));
        assert_eq!(format_from_extension(Path::new("x.ks")), Some(KeystoreFormat::Jceks));
        assert_eq!(format_from_extension(Path::new("x.pem")), None);
        assert_eq!(format_from_extension(Path::new("x")), None);
    }

    #[test]
    fn candidate_mapping() {
        assert_eq!(candidates("a", None), ["a"]);
        assert_eq!(candidates("a", Some("")), ["a"]);
        assert_eq!(candidates("a", Some("a")), ["a"]);
        assert_eq!(candidates("a", Some("b")), ["a", "b"]);
    }

    #[test]
    fn conversion_hints() {
        let h = conversion_hint(KeystoreFormat::Jceks, Path::new("/x/sig.ks")).unwrap();
        assert!(h.starts_with("keytool -importkeystore"), "{h}");
        assert!(h.contains("-srcstoretype JCEKS") && h.contains("-deststoretype PKCS12") && h.contains("sig.p12"));
        assert!(conversion_hint(KeystoreFormat::Jks, Path::new("k.jks")).unwrap().contains("-srcstoretype JKS"));
        assert_eq!(conversion_hint(KeystoreFormat::Pkcs12, Path::new("k.p12")), None);
    }

    #[test]
    fn password_storage_mapping() {
        for s in [PasswordStorage::None, PasswordStorage::Memory, PasswordStorage::Keychain] {
            assert_eq!(PasswordStorage::from_pdfover(s.as_pdfover()), Some(s));
        }
        assert_eq!(PasswordStorage::from_pdfover("DISK"), None);
    }

    #[test]
    fn error_codes_and_mapping() {
        let errs = [
            KeystoreError::FileNotFound(PathBuf::from("/home/nutzer/geheim/k.p12")),
            KeystoreError::WrongPassword,
            KeystoreError::AliasNotFound("a".into()),
            KeystoreError::NotAKeyEntry("a".into()),
            KeystoreError::ConversionNeeded(KeystoreFormat::Jks),
            KeystoreError::UnsupportedPbe("x".into()),
            KeystoreError::DifferentKeyPassword,
            KeystoreError::UnsupportedKey("x".into()),
            KeystoreError::Cms(CmsError::InvalidSignature),
            KeystoreError::Io(std::io::Error::other("x")),
            KeystoreError::Malformed("x".into()),
            KeystoreError::NoKeyEntry,
            KeystoreError::AliasRequired(2),
            KeystoreError::KeyCertificateMismatch,
        ];
        let codes: std::collections::BTreeSet<_> = errs.iter().map(KeystoreError::code).collect();
        assert_eq!(codes.len(), errs.len());
        // Anzeige ohne Verzeichnis (Regel 9)
        assert_eq!(errs[0].to_string(), "Keystore-Datei nicht gefunden: k.p12");
        assert_eq!(BackendError::from(KeystoreError::WrongPassword), BackendError::WrongPassword);
        assert_eq!(
            BackendError::from(KeystoreError::ConversionNeeded(KeystoreFormat::Jceks)),
            BackendError::UnsupportedKeystoreFormat { format: "JCEKS".into() }
        );
        assert!(matches!(
            BackendError::from(KeystoreError::DifferentKeyPassword),
            BackendError::KeystoreDetail { ref code, .. } if code == "keystore-different-key-password"
        ));
    }

    #[test]
    fn memory_cache_and_debug_redaction() {
        let c = MemoryPasswordCache::default();
        let p = Path::new("/k.p12");
        assert!(c.get(p, PasswordKind::Store).is_none());
        c.put(p, PasswordKind::Store, "geheim");
        assert_eq!(c.get(p, PasswordKind::Store).unwrap().as_str(), "geheim");
        assert!(c.get(p, PasswordKind::Key).is_none());
        assert!(!format!("{c:?}").contains("geheim"));
        c.forget(p, PasswordKind::Store);
        assert!(c.get(p, PasswordKind::Store).is_none());
        let mut cfg = KeystoreConfig::new("/k.p12");
        cfg.store_password = Some(Zeroizing::new("geheim".into()));
        assert!(!format!("{cfg:?}").contains("geheim"));
        let info = AliasInfo { alias: "a".into(), is_key_entry: true, subject: Some("CN=Max Mustermann".into()) };
        assert!(!format!("{info:?}").contains("Mustermann"));
    }
}
