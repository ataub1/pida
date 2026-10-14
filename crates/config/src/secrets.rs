//! Secrets nur im OS-Schlüsselbund bzw. RAM (Plan §2.1, §4.9, §7.1).
//!
//! Es gibt bewusst **keinen** [`SecretId`] für das ID-Austria-Passwort: es wird nie persistiert,
//! auch nicht im Schlüsselbund (Vorgabe des Auftraggebers; recherche/03 §9.1).
//!
//! - [`KeyringSecretStore`]: macOS Keychain, Windows Credential Manager, Linux Secret Service (über
//!   `keyring` 4, Feature `v1`). Unter iOS/Android gibt es keinen Standardspeicher; dort liefert die
//!   Shell eine eigene [`SecretStore`]-Implementierung (Plan §3.1, `secure-store`-Plugin).
//! - [`MemorySecretStore`]: Speicherart „Temporär“, Rückfall ohne Schlüsselbund, Tests.
//! - [`platform_secret_store`]: Schlüsselbund, wenn verfügbar, sonst RAM
//!   ([`SecretStore::is_persistent`] = `false` → Hinweis `ng-secrets-not-persistent` in der UI).
//! - Klartext-Passwörter aus `PDF-Over.config` übernimmt [`crate::migrate::apply_migration`] nur nach
//!   Zustimmung ([`store_all`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use zeroize::Zeroizing;

use crate::{ConfigError, Result};

/// Dienstname im Schlüsselbund.
pub const KEYRING_SERVICE: &str = "PDF-Over NG";

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
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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

    /// Schlüssel im Schlüsselbund (Dienst „PDF-Over NG“, Konto = diese Kennung).
    pub fn account(&self) -> String {
        match self {
            SecretId::KeystoreStorePassword { keystore } => format!("keystore-store:{}", keystore.display()),
            SecretId::KeystoreKeyPassword { keystore, alias } => format!("keystore-key:{}:{alias}", keystore.display()),
            SecretId::ProxyPassword { host, user } => format!("proxy:{user}@{host}"),
        }
    }
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
    /// Neuer Store mit Dienstnamen (Default „PDF-Over NG“, [`KEYRING_SERVICE`]).
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
        keyring::Entry::store_status().is_ok()
    }

    fn entry(&self, id: &SecretId) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &id.account()).map_err(keyring_error)
    }
}

/// Fehlertext ohne Secret-Inhalt (keyring-Fehler enthalten keine Passwörter).
fn keyring_error(e: keyring::Error) -> ConfigError {
    ConfigError::Secret(e.to_string())
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        match self.entry(id)?.get_password() {
            Ok(p) => Ok(Some(Zeroizing::new(p))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_error(e)),
        }
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        self.entry(id)?.set_password(value).map_err(keyring_error)
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_error(e)),
        }
    }
    fn is_persistent(&self) -> bool {
        true
    }
}

/// Nur im RAM (Speicherart `memory`, Fallback ohne Schlüsselbund, Tests).
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<HashMap<SecretId, Zeroizing<String>>>,
}

impl std::fmt::Debug for MemorySecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.map.lock().map(|m| m.len()).unwrap_or(0);
        f.debug_struct("MemorySecretStore").field("entries", &n).finish()
    }
}

impl MemorySecretStore {
    /// Anzahl gespeicherter Secrets.
    pub fn len(&self) -> usize {
        self.map.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// `true`, wenn leer.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Löscht alles (z. B. beim Beenden oder beim Wechsel der Speicherart).
    pub fn clear(&self) {
        if let Ok(mut m) = self.map.lock() {
            m.clear();
        }
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        Ok(self.map.lock().expect("poisoned").get(id).cloned())
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        self.map.lock().expect("poisoned").insert(id.clone(), Zeroizing::new(value.to_owned()));
        Ok(())
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        self.map.lock().expect("poisoned").remove(id);
        Ok(())
    }
    fn is_persistent(&self) -> bool {
        false
    }
}

/// Schlüsselbund der Plattform, wenn verfügbar, sonst RAM.
pub fn platform_secret_store() -> Arc<dyn SecretStore> {
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
    fn canonical_keystore_path() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("k.p12");
        std::fs::write(&f, b"x").unwrap();
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
    fn keyring_availability_does_not_panic() {
        // Kein Lese-/Schreibzugriff auf den echten Schlüsselbund im Standardlauf.
        let _ = KeyringSecretStore::new(KEYRING_SERVICE).is_available();
        assert!(!format!("{:?}", platform_secret_store()).is_empty());
    }

    /// Echter Schlüsselbund nur auf ausdrücklichen Wunsch (`PDFOVER_TEST_KEYCHAIN=1`), mit eigenem
    /// Testdienst und Testwert; räumt immer auf.
    #[test]
    fn keyring_roundtrip_opt_in() {
        if std::env::var_os("PDFOVER_TEST_KEYCHAIN").is_none() {
            eprintln!("übersprungen: PDFOVER_TEST_KEYCHAIN nicht gesetzt");
            return;
        }
        let store = KeyringSecretStore::new("PDF-Over NG Test");
        if !store.is_available() {
            eprintln!("übersprungen: kein Schlüsselbund");
            return;
        }
        let id = SecretId::proxy("test.invalid", &format!("pdfover-test-{}", std::process::id()));
        store.set(&id, "nur-test").unwrap();
        let got = store.get(&id);
        store.delete(&id).unwrap();
        assert_eq!(got.unwrap().as_deref().map(String::as_str), Some("nur-test"));
        assert_eq!(store.get(&id).unwrap(), None);
    }
}
