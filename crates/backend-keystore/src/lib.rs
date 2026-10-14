//! `pdfover-backend-keystore` – PKCS#12-Signatur (Modell A: Hash über die SignedAttributes →
//! Rohsignatur; Plan §3.7, §4.9; docs/analyse/02 §5).
//!
//! - `p12-keystore` 0.4.1: PKCS#12 inkl. Legacy-PBE (RC2-40, 3DES/SHA-1) und PBES2/AES-256.
//!   **Ein** Passwort für Store und Schlüssel. Weicht `-kskeypass`/`KEYSTORE_KEYPASS` ab, gibt es
//!   einen klaren Fehler mit Anleitung zum Neu-Export ([`KeystoreError::DifferentKeyPassword`]).
//!   Nicht lesbare Varianten (MD5-PBE, unverschlüsselt) → [`KeystoreError::UnsupportedPbe`].
//!   `p12-keystore` nutzt intern die RustCrypto-Folgegeneration; die Grenze zu `pdfover-cades`
//!   sind DER-Bytes (ADR 0001 §5.2).
//! - JKS (`0xFEEDFEED`) / JCEKS (`0xCECECECE`): nur Erkennung + Konvertierungshinweis
//!   (`keytool -importkeystore … -deststoretype PKCS12`), kein eigener Reader.
//! - Passwort-Speicherarten `none` / `memory` / `disk` → Schlüsselbund über [`PasswordCache`]
//!   (Implementierung für den Schlüsselbund liefert die Integration aus `pdfover-config`).
//! - Algorithmen wie PDF-AS (RSA → sha256WithRSA; EC ≥ 512 → SHA-512; EC ≥ 256 inkl. P-384 →
//!   SHA-256; EC < 256 und DSA abgelehnt) über `pdfover_cades::select_algorithm`.
//!
//! Ownership: Paket **B**.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use pdfover_backend_api::{
    BackendError, BackendId, BackendSession, Capabilities, CertChain, Environment, InteractionBroker, SessionContext,
    SignJob, SignResult, SignatureBackend,
};
use pdfover_cades::{Digest, RawSignature, RawSigner, SignatureAlg};
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

/// Fehler des Keystores.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    /// Datei fehlt (`error.KeyStoreFileNotExist`).
    #[error("Keystore-Datei nicht gefunden: {0}")]
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
    /// PBE-Verfahren nicht unterstützt (MD5-basiert, unverschlüsselt).
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
    Cms(#[from] pdfover_cades::CmsError),
    /// E/A.
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
}

impl From<KeystoreError> for BackendError {
    fn from(e: KeystoreError) -> Self {
        match e {
            KeystoreError::WrongPassword => BackendError::WrongPassword,
            KeystoreError::ConversionNeeded(f) => {
                BackendError::UnsupportedKeystoreFormat { format: format!("{f:?}").to_uppercase() }
            }
            other => BackendError::Keystore(other.to_string()),
        }
    }
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

impl std::fmt::Debug for KeystoreConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeystoreConfig")
            .field("path", &self.path)
            .field("declared_type", &self.declared_type)
            .field("alias", &self.alias)
            .field("store_password", &self.store_password.as_ref().map(|_| "***"))
            .field("key_password", &self.key_password.as_ref().map(|_| "***"))
            .field("storage", &self.storage)
            .field("cache", &self.cache)
            .finish()
    }
}

/// Eintrag eines Keystores (Einstellungen: „Keystore laden“ füllt die Alias-Liste).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasInfo {
    /// Alias
    pub alias: String,
    /// Schlüsseleintrag (sonst nur Zertifikat)
    pub is_key_entry: bool,
    /// Subject des Zertifikats
    pub subject: Option<String>,
}

/// Listet die Einträge (Passwortprüfung inklusive).
pub fn list_aliases(bytes: &[u8], password: &str) -> Result<Vec<AliasInfo>, KeystoreError> {
    todo!("Paket B")
}

/// Geladener Schlüssel; signiert lokal.
pub struct LoadedKey {
    chain: CertChain,
    alias: String,
    algorithm: SignatureAlg,
    _key: KeyMaterial,
}

enum KeyMaterial {
    Rsa(Box<rsa::RsaPrivateKey>),
    P256(p256::ecdsa::SigningKey),
    P384(p384::ecdsa::SigningKey),
    P521(p521::ecdsa::SigningKey),
}

impl std::fmt::Debug for LoadedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedKey")
            .field("alias", &self.alias)
            .field("algorithm", &self.algorithm)
            .finish_non_exhaustive()
    }
}

impl LoadedKey {
    /// Öffnet einen PKCS#12-Keystore.
    pub fn open_pkcs12(bytes: &[u8], password: &str, alias: Option<&str>) -> Result<LoadedKey, KeystoreError> {
        todo!("Paket B: p12_keystore::KeyStore::from_pkcs12 → DER → pdfover_cades::Certificate / PKCS#8")
    }

    /// Alias.
    pub fn alias(&self) -> &str {
        &self.alias
    }
}

impl RawSigner for LoadedKey {
    fn chain(&self) -> &CertChain {
        &self.chain
    }

    fn algorithm(&self) -> SignatureAlg {
        self.algorithm
    }

    fn sign_digest(&self, digest: &Digest) -> pdfover_cades::Result<RawSignature> {
        todo!("Paket B: RSA PKCS#1 v1.5 (prehash) bzw. ECDSA (RFC 6979, DER)")
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

#[async_trait]
impl SignatureBackend for KeystoreBackend {
    fn id(&self) -> BackendId {
        BackendId::Keystore
    }

    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        todo!(
            "Paket B: Digest{{SHA-256/512}}, RawSignature, Required, Secret, max_batch unbegrenzt, Gerät verlässt nichts"
        )
    }

    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        todo!("Paket B: Datei prüfen, Passwort (Overlay → Cache → ui.secret mit retry), Schlüssel laden")
    }
}

/// Offene Keystore-Sitzung.
#[derive(Debug)]
pub struct KeystoreSession {
    key: Arc<LoadedKey>,
}

#[async_trait]
impl BackendSession for KeystoreSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        Ok(self.key.chain().clone())
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        todo!("Paket B: nur SignJob::Digest; Document → BackendError::Internal")
    }

    async fn cancel(&mut self) {}
}
