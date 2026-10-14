//! Secrets nur im OS-Schlüsselbund bzw. RAM (Plan §2.1, §4.9, §7.1).
//!
//! Es gibt bewusst **keinen** [`SecretId`] für das ID-Austria-Passwort: es wird nie persistiert,
//! auch nicht im Schlüsselbund (Vorgabe des Auftraggebers; recherche/03 §9.1).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::Result;

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

impl SecretId {
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
    /// Löscht ein Secret.
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
    /// Neuer Store mit Dienstnamen (Default „PDF-Over NG“).
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }

    /// Prüft, ob ein Schlüsselbund verfügbar ist.
    pub fn is_available(&self) -> bool {
        todo!("Paket E")
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, id: &SecretId) -> Result<Option<Zeroizing<String>>> {
        todo!("Paket E")
    }
    fn set(&self, id: &SecretId, value: &str) -> Result<()> {
        todo!("Paket E")
    }
    fn delete(&self, id: &SecretId) -> Result<()> {
        todo!("Paket E")
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
