//! Secrets nur im OS-Schlüsselbund bzw. RAM (Plan §2.1, §4.9, §7.1).
//!
//! Es gibt bewusst **keinen** [`SecretId`] für das ID-Austria-Passwort: es wird nie persistiert,
//! auch nicht im Schlüsselbund (Vorgabe des Auftraggebers; recherche/03 §9.1).
//!
//! - [`KeyringSecretStore`]: macOS Keychain, Windows Credential Manager, Linux Secret Service (über
//!   `keyring` 4, Feature `v1`; nur Desktop-Targets). Unter iOS/Android gibt es keinen Standardspeicher
//!   und keine `keyring`-Abhängigkeit: [`KeyringSecretStore`] meldet sich dort als nicht verfügbar,
//!   [`platform_secret_store`] liefert den Platzhalter [`MobileSecretStorePlaceholder`] (nur RAM).
//! - [`ShellSecretStore`]: Mobile – der sichere Speicher der Shell ([`SecureStorage`], iOS Keychain bzw.
//!   Android Keystore über das Tauri-Plugin `pida-mobile`); die Mobile-Shell setzt ihn über
//!   `CoreOptions::secret_store` ein (Plan §3.1, §5.1). Ohne native Umsetzung: Rückfall auf RAM.
//! - [`MemorySecretStore`]: Speicherart „Temporär“, Rückfall ohne Schlüsselbund, Tests.
//! - [`platform_secret_store`]: Schlüsselbund, wenn verfügbar, sonst RAM
//!   ([`SecretStore::is_persistent`] = `false` → Hinweis `ng-secrets-not-persistent` in der UI).
//! - Klartext-Passwörter aus `PDF-Over.config` übernimmt [`crate::migrate::apply_migration`] nur nach
//!   Zustimmung ([`store_all`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use zeroize::Zeroizing;

use crate::Result;

/// Dienstname im Schlüsselbund.
pub const KEYRING_SERVICE: &str = "PIDA";

/// Kennung eines Secrets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SecretId {
    /// Keystore-Passwort (`KEYSTORE_STOREPASS`).
    KeystoreStorePassword {
        /// Keystore-Datei (kanonisiert)
        keystore: PathBuf,
    },
    /// Schlüssel-Passwort (`KEYSTORE_KEYPASS`); bei PKCS#12 identisch zum Store-Passwort.
    KeystoreKeyPassword {
        /// Keystore-Datei
        keystore: PathBuf,
        /// Alias
        alias: String,
    },
    /// Proxy-Passwort (`PROXY_PASS`).
    ProxyPassword {
        /// Proxy-Host
        host: String,
        /// Benutzer
        user: String,
    },
}

/// Kanonischer Pfad, falls die Datei existiert (sonst unverändert).
fn canonical(path: &Path) -> PathBuf {
    pida_fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl SecretId {
    /// Keystore-Passwort für eine Keystore-Datei (Pfad kanonisiert, wenn sie existiert).
    pub fn keystore_store(keystore: &Path) -> SecretId {
        SecretId::KeystoreStorePassword { keystore: canonical(keystore) }
    }

    /// Schlüssel-Passwort für Datei und Alias (Pfad kanonisiert, wenn sie existiert).
    pub fn keystore_key(keystore: &Path, alias: &str) -> SecretId {
        SecretId::KeystoreKeyPassword { keystore: canonical(keystore), alias: alias.to_owned() }
    }

    /// Proxy-Passwort für Host und Benutzer (Host ohne Groß-/Kleinschreibung).
    pub fn proxy(host: &str, user: &str) -> SecretId {
        SecretId::ProxyPassword { host: host.trim().to_ascii_lowercase(), user: user.to_owned() }
    }

    /// Schlüssel im Schlüsselbund (Dienst „PIDA“, Konto = diese Kennung). Eindeutig: im Alias
    /// sind `%` und `:`, in Benutzer und Host `%` und `@` prozentkodiert, damit z. B. Pfad `/a:b` +
    /// Alias `c` nicht mit Pfad `/a` + Alias `b:c` zusammenfällt.
    pub fn account(&self) -> String {
        match self {
            SecretId::KeystoreStorePassword { keystore } => format!("keystore-store:{}", keystore.display()),
            SecretId::KeystoreKeyPassword { keystore, alias } => {
                format!("keystore-key:{}:{}", keystore.display(), escape_component(alias, ':'))
            }
            SecretId::ProxyPassword { host, user } => {
                format!("proxy:{}@{}", escape_component(user, '@'), escape_component(host, '@'))
            }
        }
    }
}

/// Prozentkodiert `%` und das Trennzeichen `sep`.
fn escape_component(s: &str, sep: char) -> std::borrow::Cow<'_, str> {
    if !s.contains(['%', sep]) {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if c == '%' || c == sep {
            out.push_str(&format!("%{:02X}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    std::borrow::Cow::Owned(out)
}

/// Speicher für Secrets.
pub trait SecretStore: Send + Sync + std::fmt::Debug {
    /// Liest ein Secret.
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>>;
    /// Schreibt ein Secret.
    fn set(&self, id: &SecretId, value: &str) -> Result<()>;
    /// Löscht ein Secret (fehlt es, ist das kein Fehler).
    fn delete(&self, id: &SecretId) -> Result<()>;
    /// `true`, wenn über einen Neustart hinweg gespeichert wird (sonst Hinweis in der UI,
    /// z. B. Linux ohne Secret Service).
    fn is_persistent(&self) -> bool;
}

/// OS-Schlüsselbund über `keyring` (Credential Manager, Keychain, Secret Service).
#[derive(Debug)]
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    /// Neuer Store mit Dienstnamen (Default „PIDA“, [`KEYRING_SERVICE`]).
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }

    /// Dienstname.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// Prüft, ob ein Schlüsselbund verfügbar ist (einmalige Initialisierung des Plattformspeichers,
    /// ohne Lese- oder Schreibzugriff; unter iOS/Android immer `false`).
    pub fn is_available(&self) -> bool {
        desktop_keyring::available()
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        desktop_keyring::get(&self.service, &id.account())
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        desktop_keyring::set(&self.service, &id.account(), value)
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        desktop_keyring::delete(&self.service, &id.account())
    }
    fn is_persistent(&self) -> bool {
        desktop_keyring::available()
    }
}

/// Zugriff über `keyring` (nur Desktop-Targets).
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod desktop_keyring {
    use zeroize::Zeroizing;

    use crate::{ConfigError, Result};

    /// Fehlertext ohne Secret-Inhalt (keyring-Fehler enthalten keine Passwörter).
    fn keyring_error(e: keyring::Error) -> ConfigError {
        ConfigError::Secret(e.to_string())
    }

    fn entry(service: &str, account: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(service, account).map_err(keyring_error)
    }

    pub(super) fn available() -> bool {
        keyring::Entry::store_status().is_ok()
    }

    pub(super) fn get(service: &str, account: &str) -> Result<Option<Zeroizing<String>>> {
        match entry(service, account)?.get_password() {
            Ok(p) => Ok(Some(Zeroizing::new(p))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_error(e)),
        }
    }

    pub(super) fn set(service: &str, account: &str, value: &str) -> Result<()> {
        entry(service, account)?.set_password(value).map_err(keyring_error)
    }

    pub(super) fn delete(service: &str, account: &str) -> Result<()> {
        match entry(service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_error(e)),
        }
    }
}

/// iOS/Android: kein `keyring` – der Speicher kommt von der Mobile-Shell ([`super::MobileSecretStorePlaceholder`]).
#[cfg(any(target_os = "ios", target_os = "android"))]
mod desktop_keyring {
    use zeroize::Zeroizing;

    use crate::{ConfigError, Result};

    fn unavailable() -> ConfigError {
        ConfigError::Secret("kein Schlüsselbund auf dieser Plattform (Mobile-Shell setzt den SecretStore)".into())
    }

    pub(super) fn available() -> bool {
        false
    }

    pub(super) fn get(_service: &str, _account: &str) -> Result<Option<Zeroizing<String>>> {
        Err(unavailable())
    }

    pub(super) fn set(_service: &str, _account: &str, _value: &str) -> Result<()> {
        Err(unavailable())
    }

    pub(super) fn delete(_service: &str, _account: &str) -> Result<()> {
        Err(unavailable())
    }
}

/// Platzhalter für iOS/Android (Mobile-Welle): hält Secrets nur im RAM und meldet sich als nicht
/// persistent (UI-Hinweis `ng-secrets-not-persistent`), bis die Mobile-Shell über
/// `CoreOptions::secret_store` den Speicher des `secure-store`-Plugins (Keychain bzw. Android Keystore)
/// einsetzt. Auf dem Desktop nur in Tests verwendet.
#[derive(Debug, Default)]
pub struct MobileSecretStorePlaceholder {
    inner: MemorySecretStore,
}

impl SecretStore for MobileSecretStorePlaceholder {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        self.inner.get(id)
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        self.inner.set(id, value)
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        self.inner.delete(id)
    }
    fn is_persistent(&self) -> bool {
        false
    }
}

/// Sicherer Speicher, den die **Shell** bereitstellt (Mobile: iOS Keychain bzw. Android Keystore über
/// das Tauri-Plugin `pida-mobile`, Befehle `secure_store_get/set/delete`). `pida-config` kennt
/// das Plugin nicht; die Shell implementiert diese Schnittstelle und setzt [`ShellSecretStore`] über
/// `CoreOptions::secret_store` ein (keyring bleibt Desktop-only).
///
/// Schlüssel sind kurze ASCII-Kennungen (`<Dienst>/<SecretId::account>`, ohne Secrets); Werte sind
/// Passwörter und dürfen weder geloggt noch in Fehlertexten zurückgegeben werden. Fehlertexte enthalten
/// nur Codes (z. B. `ng-mobile-not-implemented`, solange die native Seite fehlt).
pub trait SecureStorage: Send + Sync + std::fmt::Debug {
    /// Liest einen Wert (`None` = nicht vorhanden).
    fn get(&self, key: &str) -> std::result::Result<Option<Zeroizing<String>>, String>;
    /// Schreibt einen Wert (ersetzt einen vorhandenen).
    fn set(&self, key: &str, value: &str) -> std::result::Result<(), String>;
    /// Löscht einen Wert (fehlt er, ist das kein Fehler).
    fn delete(&self, key: &str) -> std::result::Result<(), String>;
    /// `true`, wenn der Speicher dauerhaft über einen Neustart hinweg sichert (Keychain/Keystore
    /// eingerichtet); `false` z. B. solange die native Seite des Plugins noch leer ist.
    fn is_available(&self) -> bool;
}

/// [`SecretStore`] auf einem [`SecureStorage`] der Shell (Mobile, Plan §5.1 „Secrets: Keychain bzw.
/// Android Keystore“). Ist der Speicher nicht verfügbar, gelten Secrets nur im RAM (Rückfall wie
/// [`MobileSecretStorePlaceholder`]) und `is_persistent` ist `false` (UI-Hinweis
/// `ng-secrets-not-persistent`). Das ID-Austria-Passwort hat keinen [`SecretId`] und erreicht diesen
/// Speicher strukturell nie.
pub struct ShellSecretStore {
    storage: Arc<dyn SecureStorage>,
    service: String,
    /// beim ersten Zugriff ermittelt (nicht beim Anlegen: die Shell legt den Speicher auf dem
    /// Hauptthread an, die native Seite darf dort aber nicht blockierend gefragt werden)
    available: std::sync::OnceLock<bool>,
    fallback: MemorySecretStore,
}

impl std::fmt::Debug for ShellSecretStore {
    /// Ohne den Speicher der Shell selbst (dessen `Debug` ist nicht unter unserer Kontrolle, Regel 9).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellSecretStore")
            .field("service", &self.service)
            .field("available", &self.available.get())
            .field("fallback", &self.fallback)
            .finish_non_exhaustive()
    }
}

impl ShellSecretStore {
    /// Speicher der Shell mit dem Dienstnamen [`KEYRING_SERVICE`]; die Verfügbarkeit wird einmal beim
    /// ersten Zugriff ermittelt ([`SecureStorage::is_available`]).
    pub fn new(storage: Arc<dyn SecureStorage>) -> Self {
        Self::with_service(storage, KEYRING_SERVICE)
    }

    /// Wie [`ShellSecretStore::new`] mit eigenem Dienstnamen (Tests, Varianten).
    pub fn with_service(storage: Arc<dyn SecureStorage>, service: impl Into<String>) -> Self {
        Self {
            storage,
            service: service.into(),
            available: std::sync::OnceLock::new(),
            fallback: MemorySecretStore::default(),
        }
    }

    fn available(&self) -> bool {
        *self.available.get_or_init(|| {
            let available = self.storage.is_available();
            if !available {
                tracing::warn!("sicherer Speicher der Shell nicht verfügbar – Secrets nur im RAM");
            }
            available
        })
    }

    /// Schlüssel im Speicher der Shell: `<Dienst>/<Konto>` (Konto = [`SecretId::account`]).
    pub fn key(&self, id: &SecretId) -> String {
        format!("{}/{}", self.service, id.account())
    }

    fn error(e: String) -> crate::ConfigError {
        crate::ConfigError::Secret(e)
    }
}

impl SecretStore for ShellSecretStore {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        if !self.available() {
            return self.fallback.get(id);
        }
        self.storage.get(&self.key(id)).map_err(Self::error)
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        if !self.available() {
            return self.fallback.set(id, value);
        }
        self.storage.set(&self.key(id), value).map_err(Self::error)
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        if !self.available() {
            return self.fallback.delete(id);
        }
        self.storage.delete(&self.key(id)).map_err(Self::error)
    }
    fn is_persistent(&self) -> bool {
        self.available()
    }
}

/// Nur im RAM (Speicherart `memory`, Fallback ohne Schlüsselbund, Tests).
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<HashMap<SecretId, Zeroizing<String>>>,
}

impl std::fmt::Debug for MemorySecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemorySecretStore").field("entries", &self.len()).finish()
    }
}

impl MemorySecretStore {
    /// Map; ein vergifteter Mutex (Panik in einem anderen Thread) wird weiterverwendet, statt erneut
    /// zu paniken – die Map selbst bleibt konsistent (jede Operation ist ein einzelner Aufruf).
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<SecretId, Zeroizing<String>>> {
        self.map.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Anzahl gespeicherter Secrets.
    pub fn len(&self) -> usize {
        self.map().len()
    }

    /// `true`, wenn leer.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Löscht alles (z. B. beim Beenden oder beim Wechsel der Speicherart).
    pub fn clear(&self) {
        self.map().clear();
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        Ok(self.map().get(id).cloned())
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        self.map().insert(id.clone(), Zeroizing::new(value.to_owned()));
        Ok(())
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        self.map().remove(id);
        Ok(())
    }
    fn is_persistent(&self) -> bool {
        false
    }
}

/// Schlüsselbund der Plattform, wenn verfügbar, sonst RAM. Unter iOS/Android der Platzhalter
/// [`MobileSecretStorePlaceholder`] (die Mobile-Shell setzt ihren eigenen Speicher).
pub fn platform_secret_store() -> Arc<dyn SecretStore> {
    if cfg!(any(target_os = "ios", target_os = "android")) {
        tracing::info!("Mobile: Secrets bis zum Speicher der Shell nur im RAM");
        return Arc::new(MobileSecretStorePlaceholder::default());
    }
    let keyring = KeyringSecretStore::new(KEYRING_SERVICE);
    if keyring.is_available() {
        Arc::new(keyring)
    } else {
        tracing::warn!("kein Schlüsselbund verfügbar, Secrets nur im RAM");
        Arc::new(MemorySecretStore::default())
    }
}

/// Speichert mehrere Secrets; liefert die gespeicherten Kennungen. Beim ersten Fehler werden die
/// bereits geschriebenen wieder gelöscht (alles oder nichts).
pub fn store_all(store: &dyn SecretStore, secrets: &[(SecretId, Zeroizing<String>)]) -> Result<Vec<SecretId>> {
    let mut done = Vec::new();
    for (id, value) in secrets {
        if let Err(e) = store.set(id, value) {
            for d in &done {
                let _ = store.delete(d);
            }
            return Err(e);
        }
        done.push(id.clone());
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigError;

    #[test]
    fn memory_store_roundtrip() {
        let s = MemorySecretStore::default();
        let id = SecretId::proxy("Proxy.Example ", "max");
        assert_eq!(id, SecretId::ProxyPassword { host: "proxy.example".into(), user: "max".into() });
        assert_eq!(s.get(&id).unwrap(), None);
        s.set(&id, "test-only").unwrap();
        assert_eq!(s.get(&id).unwrap().as_deref().map(String::as_str), Some("test-only"));
        assert_eq!(s.len(), 1);
        assert!(!format!("{s:?}").contains("test-only"));
        s.delete(&id).unwrap();
        s.delete(&id).unwrap();
        assert!(s.is_empty());
        assert!(!s.is_persistent());
    }

    #[test]
    fn accounts_are_distinct() {
        let a = SecretId::keystore_store(Path::new("/nicht/vorhanden.p12"));
        let b = SecretId::keystore_key(Path::new("/nicht/vorhanden.p12"), "alias");
        let c = SecretId::proxy("h", "u");
        assert_eq!(a.account(), "keystore-store:/nicht/vorhanden.p12");
        assert_eq!(b.account(), "keystore-key:/nicht/vorhanden.p12:alias");
        assert_eq!(c.account(), "proxy:u@h");
    }

    #[test]
    fn accounts_are_unambiguous() {
        let p = |s: &str| Path::new(s).to_path_buf();
        let a = SecretId::KeystoreKeyPassword { keystore: p("/a:b"), alias: "c".into() };
        let b = SecretId::KeystoreKeyPassword { keystore: p("/a"), alias: "b:c".into() };
        assert_ne!(a.account(), b.account());
        assert_eq!(b.account(), "keystore-key:/a:b%3Ac");
        let c = SecretId::proxy("c", "a@b");
        let d = SecretId::proxy("b@c", "a");
        assert_ne!(c.account(), d.account());
        assert_eq!(c.account(), "proxy:a%40b@c");
        assert_eq!(SecretId::proxy("h", "100%").account(), "proxy:100%25@h");
        // Windows-Pfade bleiben lesbar.
        assert_eq!(SecretId::KeystoreStorePassword { keystore: p(r"C:\k.p12") }.account(), r"keystore-store:C:\k.p12");
    }

    #[test]
    fn memory_store_survives_poisoned_mutex() {
        let s = std::sync::Arc::new(MemorySecretStore::default());
        s.set(&SecretId::proxy("h", "u"), "x").unwrap();
        let s2 = std::sync::Arc::clone(&s);
        let _ = std::thread::spawn(move || {
            let _guard = s2.map.lock().unwrap();
            panic!("absichtlich (Test)");
        })
        .join();
        assert!(s.map.is_poisoned());
        assert_eq!(s.len(), 1);
        assert!(s.get(&SecretId::proxy("h", "u")).unwrap().is_some());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn canonical_keystore_path() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("k.p12");
        pida_fs::write(&f, b"x").unwrap();
        let via_dot = dir.path().join(".").join("k.p12");
        assert_eq!(SecretId::keystore_store(&via_dot), SecretId::keystore_store(&f));
    }

    #[derive(Debug, Default)]
    struct FailingStore {
        inner: MemorySecretStore,
        fail_on: usize,
        calls: Mutex<usize>,
    }

    impl SecretStore for FailingStore {
        fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
            self.inner.get(id)
        }
        fn set(&self, id: &SecretId, value: &str) -> Result<()> {
            let mut c = self.calls.lock().unwrap();
            *c += 1;
            if *c == self.fail_on {
                return Err(ConfigError::Secret("gesperrt".into()));
            }
            self.inner.set(id, value)
        }
        fn delete(&self, id: &SecretId) -> Result<()> {
            self.inner.delete(id)
        }
        fn is_persistent(&self) -> bool {
            true
        }
    }

    #[test]
    fn store_all_is_all_or_nothing() {
        let secrets = vec![
            (SecretId::proxy("a", "b"), Zeroizing::new("x".to_owned())),
            (SecretId::proxy("c", "d"), Zeroizing::new("y".to_owned())),
        ];
        let ok = MemorySecretStore::default();
        assert_eq!(store_all(&ok, &secrets).unwrap().len(), 2);
        let failing = FailingStore { fail_on: 2, ..Default::default() };
        assert!(store_all(&failing, &secrets).is_err());
        assert!(failing.inner.is_empty());
    }

    #[test]
    fn mobile_placeholder_keeps_secrets_in_ram_only() {
        let store = MobileSecretStorePlaceholder::default();
        let id = SecretId::ProxyPassword { host: "proxy".into(), user: "u".into() };
        store.set(&id, "x").unwrap();
        assert_eq!(store.get(&id).unwrap().as_deref().map(String::as_str), Some("x"));
        assert!(!store.is_persistent());
        store.delete(&id).unwrap();
        assert_eq!(store.get(&id).unwrap(), None);
    }

    /// Sicherer Speicher der Shell im Test (wie das Plugin `pida-mobile`): Schlüssel → Wert.
    #[derive(Debug, Default)]
    struct FakeStorage {
        map: Mutex<HashMap<String, String>>,
        unavailable: bool,
        fail: bool,
    }

    impl SecureStorage for FakeStorage {
        fn get(&self, key: &str) -> std::result::Result<Option<Zeroizing<String>>, String> {
            if self.fail {
                return Err("ng-mobile-secure-store-failed".into());
            }
            Ok(self.map.lock().unwrap().get(key).cloned().map(Zeroizing::new))
        }
        fn set(&self, key: &str, value: &str) -> std::result::Result<(), String> {
            if self.fail {
                return Err("ng-mobile-secure-store-failed".into());
            }
            self.map.lock().unwrap().insert(key.to_owned(), value.to_owned());
            Ok(())
        }
        fn delete(&self, key: &str) -> std::result::Result<(), String> {
            self.map.lock().unwrap().remove(key);
            Ok(())
        }
        fn is_available(&self) -> bool {
            !self.unavailable
        }
    }

    #[test]
    fn shell_secret_store_uses_the_secure_storage_of_the_shell() {
        let storage = Arc::new(FakeStorage::default());
        let store = ShellSecretStore::new(storage.clone());
        let id = SecretId::keystore_store(Path::new("/sandbox/data/keystores/test.p12"));
        assert!(store.is_persistent());
        assert_eq!(store.get(&id).unwrap(), None);
        store.set(&id, "test-only").unwrap();
        assert_eq!(store.get(&id).unwrap().as_deref().map(String::as_str), Some("test-only"));
        let key = store.key(&id);
        assert_eq!(key, "PIDA/keystore-store:/sandbox/data/keystores/test.p12");
        assert_eq!(storage.map.lock().unwrap().get(&key).map(String::as_str), Some("test-only"));
        assert!(!format!("{store:?}").contains("test-only"));
        store.delete(&id).unwrap();
        assert!(storage.map.lock().unwrap().is_empty());
    }

    #[test]
    fn shell_secret_store_falls_back_to_ram_without_native_storage() {
        let storage = Arc::new(FakeStorage { unavailable: true, ..FakeStorage::default() });
        let store = ShellSecretStore::new(storage.clone());
        let id = SecretId::proxy("proxy", "u");
        assert!(!store.is_persistent(), "UI-Hinweis ng-secrets-not-persistent");
        store.set(&id, "x").unwrap();
        assert_eq!(store.get(&id).unwrap().as_deref().map(String::as_str), Some("x"));
        assert!(storage.map.lock().unwrap().is_empty(), "nichts an die (leere) native Seite");
        store.delete(&id).unwrap();
        assert_eq!(store.get(&id).unwrap(), None);
    }

    #[test]
    fn shell_secret_store_reports_errors_without_values() {
        let store = ShellSecretStore::new(Arc::new(FakeStorage { fail: true, ..FakeStorage::default() }));
        let err = store.set(&SecretId::proxy("h", "u"), "geheim").unwrap_err();
        assert_eq!(err.message_id(), "ng-config-error-secret");
        assert!(!err.to_string().contains("geheim"));
    }

    #[test]
    fn keyring_availability_does_not_panic() {
        // Kein Lese-/Schreibzugriff auf den echten Schlüsselbund im Standardlauf.
        let _ = KeyringSecretStore::new(KEYRING_SERVICE).is_available();
        assert!(!format!("{:?}", platform_secret_store()).is_empty());
    }

    /// Echter Schlüsselbund nur auf ausdrücklichen Wunsch (`PIDA_TEST_KEYCHAIN=1`), mit eigenem
    /// Testdienst und Testwert; räumt immer auf.
    #[test]
    fn keyring_roundtrip_opt_in() {
        if std::env::var_os("PIDA_TEST_KEYCHAIN").is_none() {
            eprintln!("übersprungen: PIDA_TEST_KEYCHAIN nicht gesetzt");
            return;
        }
        let store = KeyringSecretStore::new("PIDA Test");
        if !store.is_available() {
            eprintln!("übersprungen: kein Schlüsselbund");
            return;
        }
        let id = SecretId::proxy("test.invalid", &format!("pida-test-{}", std::process::id()));
        store.set(&id, "nur-test").unwrap();
        let got = store.get(&id);
        store.delete(&id).unwrap();
        assert_eq!(got.unwrap().as_deref().map(String::as_str), Some("nur-test"));
        assert_eq!(store.get(&id).unwrap(), None);
    }
}
