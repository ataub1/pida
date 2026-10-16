//! Backend-Registrierung (ADR 0001 §3: `core-api` ist die einzige Stelle, die konkrete Backends
//! kennt): ID Austria Szenario A, lokale BKU (nur Desktop, Erreichbarkeits-Polling 5 s/30 s),
//! PKCS#12-Keystore (nur Desktop, ADR 0004 Nachtrag 11) mit [`SecretStoreCache`] als `PasswordCache` auf
//! `config::SecretStore`. Auf iOS/Android ist ID Austria das einzige Signiermittel.

use std::path::Path;
use std::sync::Arc;

use pida_backend_api::{BackendId, Environment, Platform, SignatureBackend};
use pida_backend_atrust::{AtrustConfig, AtrustNativeBackend, Fido2Provider};
use pida_backend_keystore::{
    KeystoreBackend, KeystoreConfig, MemoryPasswordCache, NoPasswordCache, PasswordCache, PasswordKind, PasswordStorage,
};
use pida_backend_localbku::{Availability, LocalBkuBackend, LocalBkuConfig};
use pida_config::{ProxyMode, SecretId, SecretStore};
use pida_net::{NetConfig, ProxyAuth, ProxyConfig};
use pida_workflow::BackendOption;
use zeroize::Zeroizing;

use crate::config::ConfigState;

/// `PasswordCache` des Keystores auf dem Schlüsselbund (Speicherart „Permanent“, ADR 0001 §5.5).
pub struct SecretStoreCache {
    store: Arc<dyn SecretStore>,
    alias: String,
}

impl SecretStoreCache {
    /// Adapter für Keystore-Datei-Passwörter mit Alias `alias` (Schlüsselpasswort).
    pub fn new(store: Arc<dyn SecretStore>, alias: impl Into<String>) -> Self {
        Self { store, alias: alias.into() }
    }

    fn id(&self, keystore: &Path, kind: PasswordKind) -> SecretId {
        match kind {
            PasswordKind::Store => SecretId::keystore_store(keystore),
            PasswordKind::Key => SecretId::keystore_key(keystore, &self.alias),
        }
    }
}

impl std::fmt::Debug for SecretStoreCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretStoreCache").field("store", &self.store).finish_non_exhaustive()
    }
}

impl PasswordCache for SecretStoreCache {
    fn get(&self, keystore: &Path, kind: PasswordKind) -> Option<Zeroizing<String>> {
        match self.store.get(&self.id(keystore, kind)) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(id = e.message_id(), "Schlüsselbund nicht lesbar");
                None
            }
        }
    }
    fn put(&self, keystore: &Path, kind: PasswordKind, password: &str) {
        if let Err(e) = self.store.set(&self.id(keystore, kind), password) {
            tracing::warn!(id = e.message_id(), "Passwort nicht im Schlüsselbund gespeichert");
        }
    }
    fn forget(&self, keystore: &Path, kind: PasswordKind) {
        if let Err(e) = self.store.delete(&self.id(keystore, kind)) {
            tracing::warn!(id = e.message_id(), "Passwort nicht aus dem Schlüsselbund gelöscht");
        }
    }
}

/// `PasswordCache` des Keystores, solange die Speicherart noch nie gewählt wurde (Neuinstallation,
/// `keystore.storage_chosen = false`): Die erste Passworteingabe fragt „Passwort merken?“; die Antwort
/// ([`Registry::choose_keystore_storage`]) gilt schon für dieses Passwort, auch wenn das Backend mit der
/// alten Konfiguration weiterläuft. Ohne Antwort (klassische Oberfläche) wie der Standard „bis Programmende“.
struct FirstUseCache {
    choice: Arc<std::sync::Mutex<Option<pida_config::PasswordStorage>>>,
    memory: Arc<MemoryPasswordCache>,
    keychain: SecretStoreCache,
}

impl FirstUseCache {
    fn choice(&self) -> Option<pida_config::PasswordStorage> {
        *self.choice.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl std::fmt::Debug for FirstUseCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FirstUseCache").field("choice", &self.choice()).finish_non_exhaustive()
    }
}

impl PasswordCache for FirstUseCache {
    fn get(&self, keystore: &Path, kind: PasswordKind) -> Option<Zeroizing<String>> {
        match self.choice() {
            Some(pida_config::PasswordStorage::None) => None,
            Some(pida_config::PasswordStorage::Keychain) => self.keychain.get(keystore, kind),
            _ => self.memory.get(keystore, kind),
        }
    }
    fn put(&self, keystore: &Path, kind: PasswordKind, password: &str) {
        match self.choice() {
            Some(pida_config::PasswordStorage::None) => {}
            Some(pida_config::PasswordStorage::Keychain) => self.keychain.put(keystore, kind, password),
            _ => self.memory.put(keystore, kind, password),
        }
    }
    fn forget(&self, keystore: &Path, kind: PasswordKind) {
        self.memory.forget(keystore, kind);
        if self.choice() == Some(pida_config::PasswordStorage::Keychain) {
            self.keychain.forget(keystore, kind);
        }
    }
}

/// Speicherart der Konfiguration → Keystore-Speicherart.
pub fn password_storage(p: pida_config::PasswordStorage) -> PasswordStorage {
    match p {
        pida_config::PasswordStorage::None => PasswordStorage::None,
        pida_config::PasswordStorage::Memory => PasswordStorage::Memory,
        pida_config::PasswordStorage::Keychain => PasswordStorage::Keychain,
    }
}

/// Proxy der Einstellung → Netz-Konfiguration (Passwort aus Overlay bzw. Schlüsselbund).
pub(crate) fn proxy_config(cfg: &ConfigState) -> ProxyConfig {
    let eff = cfg.effective();
    let proxy = eff.proxy();
    match proxy.mode {
        ProxyMode::System => ProxyConfig::System,
        ProxyMode::None => ProxyConfig::None,
        ProxyMode::Manual => match proxy.host.filter(|h| !h.trim().is_empty()) {
            Some(host) => {
                let password = eff.proxy_password(cfg.secrets.as_ref()).ok().flatten();
                let auth = match (proxy.user, password) {
                    (Some(user), Some(password)) => Some(ProxyAuth { user, password }),
                    _ => None,
                };
                ProxyConfig::Manual { host, port: proxy.port.unwrap_or(8080), auth }
            }
            None => ProxyConfig::System,
        },
    }
}

/// Teile der Konfiguration, die eine neue ID-Austria-Instanz erfordern (sonst bleibt das im RAM
/// gemerkte Passwort über Einstellungsänderungen erhalten).
#[derive(Clone, PartialEq, Eq)]
struct AtrustKey {
    username: Option<String>,
    remember: bool,
    fido2_default: bool,
    proxy: ProxyConfig,
}

/// Registrierte Backends.
pub(crate) struct Registry {
    atrust: Option<(AtrustKey, Arc<AtrustNativeBackend>)>,
    localbku: Option<Arc<LocalBkuBackend>>,
    keystore: Option<Arc<KeystoreBackend>>,
    keystore_configured: bool,
    /// fest vorgegebene Backends (Tests, Einbettung)
    fixed: Option<Vec<Arc<dyn SignatureBackend>>>,
    memory_cache: Arc<MemoryPasswordCache>,
    /// Antwort auf „Passwort merken?“ bei der ersten Verwendung ([`FirstUseCache`])
    first_use_choice: Arc<std::sync::Mutex<Option<pida_config::PasswordStorage>>>,
    availability: Option<Availability>,
    fido2: Option<Arc<dyn Fido2Provider>>,
    platform: Platform,
    app_version: String,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("atrust", &self.atrust.is_some())
            .field("localbku", &self.localbku.is_some())
            .field("keystore", &self.keystore.is_some())
            .field("fixed", &self.fixed.as_ref().map(Vec::len))
            .finish()
    }
}

impl Registry {
    /// Registriert die Backends; startet unter dem Desktop die Erreichbarkeitsprüfung der lokalen BKU
    /// (braucht eine laufende Tokio-Laufzeit, sonst Hintergrund-Thread).
    pub(crate) fn new(
        platform: Platform,
        fido2: Option<Arc<dyn Fido2Provider>>,
        app_version: &str,
        fixed: Option<Vec<Arc<dyn SignatureBackend>>>,
        probe_localbku: bool,
    ) -> Registry {
        let availability = if fixed.is_none() && platform.is_desktop() && probe_localbku {
            Some(Availability::spawn(&LocalBkuConfig::new(app_version)))
        } else {
            None
        };
        Registry {
            atrust: None,
            localbku: None,
            keystore: None,
            keystore_configured: false,
            fixed,
            memory_cache: Arc::new(MemoryPasswordCache::default()),
            first_use_choice: Arc::default(),
            availability,
            fido2,
            platform,
            app_version: app_version.to_owned(),
        }
    }

    fn atrust_key(cfg: &ConfigState) -> AtrustKey {
        let eff = cfg.effective();
        AtrustKey {
            username: eff.username().map(str::to_owned),
            remember: cfg.settings.id_austria.remember,
            fido2_default: cfg.settings.id_austria.fido2_default,
            proxy: proxy_config(cfg),
        }
    }

    /// Das ID-Austria-Backend hat Benutzername bzw. „Merken“ selbst geändert (`BackendEvent`) und der Kern
    /// hat das gespeichert: Schlüssel übernehmen, **ohne** das Backend neu zu bauen – sonst ginge das im
    /// RAM gemerkte Passwort beim nächsten [`Registry::configure`] verloren.
    pub(crate) fn adopt_atrust_settings(&mut self, cfg: &ConfigState) {
        if let Some((key, _)) = &mut self.atrust {
            *key = Self::atrust_key(cfg);
        }
    }

    /// Baut die Backends aus der aktuellen Konfiguration (nach Start und nach jeder Änderung).
    pub(crate) fn configure(&mut self, cfg: &ConfigState) {
        if self.fixed.is_some() {
            return;
        }
        let eff = cfg.effective();
        // ID Austria (Szenario A)
        let key = Self::atrust_key(cfg);
        let proxy = key.proxy.clone();
        if self.atrust.as_ref().is_none_or(|(k, _)| *k != key) {
            let mut c = AtrustConfig::production();
            c.net = NetConfig { proxy, ..c.net };
            c.prefill_username = key.username.clone();
            c.prefill_password = eff.password().cloned();
            c.remember = key.remember;
            c.fido2_default = key.fido2_default;
            c.fido2 = self.fido2.clone();
            self.atrust = Some((key, Arc::new(AtrustNativeBackend::new(c))));
        }
        // lokale BKU (nur Desktop)
        if self.platform.is_desktop() && self.localbku.is_none() {
            let config = LocalBkuConfig::new(&self.app_version);
            self.localbku = Some(Arc::new(LocalBkuBackend::new(config, self.availability.clone())));
        }
        // Keystore (nur Desktop: auf iOS/Android nur ID Austria, ADR 0004 Nachtrag 11 – die Werte unter
        // `[keystore]` bleiben dort unberührt und werden ignoriert)
        self.keystore = None;
        self.keystore_configured = false;
        if self.keystore_offered()
            && eff.keystore_enabled()
            && let Some(file) = eff.keystore_file()
        {
            self.keystore_configured = file.is_file();
            let (storage, cache) = self.keystore_cache(cfg);
            let session = cfg.session_secrets.as_ref();
            let mut kc = KeystoreConfig::new(file.to_path_buf());
            kc.declared_type = eff.keystore_type().map(str::to_owned);
            kc.alias = eff.keystore_alias().map(str::to_owned);
            kc.store_password = eff.keystore_store_password(session).ok().flatten();
            kc.key_password = eff.keystore_key_password(session).ok().flatten();
            kc.storage = storage;
            kc.cache = cache;
            self.keystore = Some(Arc::new(KeystoreBackend::new(kc)));
        }
    }

    /// Speicherart und Ablage der Keystore-Passwörter wie beim Signieren: Passwörter einer Datei aus dem
    /// Overlay werden nie dauerhaft abgelegt (KSState:119-129); solange die Speicherart noch nie gewählt
    /// wurde, entscheidet die Antwort auf „Passwort merken?“ ([`FirstUseCache`]).
    pub(crate) fn keystore_cache(&self, cfg: &ConfigState) -> (PasswordStorage, Arc<dyn PasswordCache>) {
        let eff = cfg.effective();
        let storage = password_storage(cfg.settings.keystore.password_storage);
        let alias = eff.keystore_alias().unwrap_or_default().to_owned();
        if Self::asks_storage(cfg) {
            let cache = FirstUseCache {
                choice: self.first_use_choice.clone(),
                memory: self.memory_cache.clone(),
                keychain: SecretStoreCache::new(cfg.secrets.clone(), alias),
            };
            // die Ablage folgt der Antwort; ohne Antwort „bis Programmende“ (Standard)
            return (PasswordStorage::Memory, Arc::new(cache));
        }
        let cache: Arc<dyn PasswordCache> = match storage {
            PasswordStorage::None => Arc::new(NoPasswordCache),
            PasswordStorage::Keychain if !eff.keystore_from_overlay() => {
                Arc::new(SecretStoreCache::new(cfg.secrets.clone(), alias))
            }
            _ => self.memory_cache.clone(),
        };
        (storage, cache)
    }

    /// Fragt die nächste Keystore-Passworteingabe „Passwort merken?“ (Speicherart noch nie gewählt, Datei aus
    /// den Einstellungen – eine Datei aus dem Overlay wird ohnehin nie dauerhaft gemerkt)?
    pub(crate) fn asks_storage(cfg: &ConfigState) -> bool {
        !cfg.settings.keystore.storage_chosen && !cfg.effective().keystore_from_overlay()
    }

    /// Antwort auf „Passwort merken?“: gilt für das gerade eingegebene Passwort (auch im schon laufenden
    /// Signiervorgang); danach baut [`Registry::configure`] den Keystore mit der gespeicherten Speicherart.
    pub(crate) fn choose_keystore_storage(&self, storage: pida_config::PasswordStorage) {
        *self.first_use_choice.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(storage);
    }

    /// Bekanntes Passwort der Keystore-Datei `file` (Overlay/Sitzung bzw. gemäß Speicherart gemerkt) – für das
    /// automatische Lesen der Einträge in den Einstellungen.
    pub(crate) fn known_keystore_password(&self, cfg: &ConfigState, file: &Path) -> Option<Zeroizing<String>> {
        let eff = cfg.effective();
        if eff.keystore_file() == Some(file)
            && let Ok(Some(p)) = eff.keystore_store_password(cfg.session_secrets.as_ref())
        {
            return Some(p);
        }
        let (storage, cache) = self.keystore_cache(cfg);
        if storage == PasswordStorage::None {
            return None;
        }
        cache.get(file, PasswordKind::Store)
    }

    /// Bekanntes Keystore-Passwort OHNE Zugriff auf den Schlüsselbund (Kommandozeile/Sitzung, „bis
    /// Programmende“): für automatische Anzeigen wie die Schlüsselliste beim Öffnen der Einstellungen. Ein
    /// Schlüsselbund-Zugriff löst unter macOS eine Systemabfrage aus und darf nur auf eine Nutzeraktion hin
    /// geschehen (Signieren, „Schlüssel anzeigen …“, Wahl „Im Schlüsselbund“).
    pub(crate) fn known_keystore_password_without_keychain(
        &self,
        cfg: &ConfigState,
        file: &Path,
    ) -> Option<Zeroizing<String>> {
        let eff = cfg.effective();
        if eff.keystore_file() == Some(file)
            && let Ok(Some(p)) = eff.keystore_store_password(cfg.session_secrets.as_ref())
        {
            return Some(p);
        }
        if password_storage(cfg.settings.keystore.password_storage) == PasswordStorage::None {
            return None;
        }
        self.memory_cache.get(file, PasswordKind::Store)
    }

    /// Ein geprüftes Keystore-Passwort gemäß Speicherart behalten (wie nach der Eingabe beim Signieren:
    /// `none` verwirft, `memory` bis Programmende, Schlüsselbund dauerhaft).
    pub(crate) fn remember_keystore_password(&self, cfg: &ConfigState, file: &Path, password: &str) {
        let (storage, cache) = self.keystore_cache(cfg);
        if storage != PasswordStorage::None {
            cache.put(file, PasswordKind::Store, password);
        }
    }

    /// Fest vorgegebene Backends (Tests, Einbettung)?
    pub(crate) fn is_fixed(&self) -> bool {
        self.fixed.is_some()
    }

    /// Signieren mit Keystore auf dieser Plattform (Desktop; iOS/Android nur ID Austria)?
    pub(crate) fn keystore_offered(&self) -> bool {
        crate::mobile_signers::keystore_offered(self.platform)
    }

    /// Alle registrierten Backends (auf iOS/Android nie ein Keystore, auch nicht unter fest vorgegebenen).
    pub(crate) fn all(&self) -> Vec<Arc<dyn SignatureBackend>> {
        if let Some(fixed) = &self.fixed {
            let offered = self.keystore_offered();
            return fixed.iter().filter(|b| offered || b.id() != BackendId::Keystore).cloned().collect();
        }
        let mut out: Vec<Arc<dyn SignatureBackend>> = Vec::new();
        if let Some((_, a)) = &self.atrust {
            out.push(a.clone());
        }
        if let Some(l) = &self.localbku {
            out.push(l.clone());
        }
        if let Some(k) = &self.keystore {
            out.push(k.clone());
        }
        out
    }

    /// Backend nach Kennung.
    pub(crate) fn get(&self, id: BackendId) -> Option<Arc<dyn SignatureBackend>> {
        self.all().into_iter().find(|b| b.id() == id)
    }

    /// ID-Austria-Backend (Vergessen gemerkter Daten).
    pub(crate) fn atrust(&self) -> Option<Arc<AtrustNativeBackend>> {
        self.atrust.as_ref().map(|(_, a)| a.clone())
    }

    /// Erwartete Anmeldungen eines Stapellaufs (ADR 0004 §3.8, `BatchEstimateView::logins`): 0, wenn das
    /// Signiermittel ohne Rückfrage anmeldet bzw. entsperrt – lokale BKU (PIN je Dokument in der
    /// Chipkarten-Software), Keystore mit Passwort aus dem Overlay bzw. im Passwortspeicher, ID Austria mit
    /// Passwort im „Merken“-Speicher (`AtrustNativeBackend::has_remembered_password`) –, sonst 1. Fest
    /// vorgegebene Backends (Tests) zählen eine Anmeldung. Eine Erwartung, keine Zusage (§1.2).
    pub(crate) fn expected_logins(&self, id: BackendId) -> u32 {
        if self.fixed.is_some() {
            return u32::from(id != BackendId::LocalBku);
        }
        match id {
            BackendId::LocalBku => 0,
            BackendId::AtrustNative => {
                u32::from(!self.atrust.as_ref().is_some_and(|(_, a)| a.has_remembered_password()))
            }
            BackendId::Keystore => {
                let stored = self.keystore.as_ref().is_some_and(|k| {
                    let c = k.config();
                    c.store_password.is_some()
                        || (c.storage != PasswordStorage::None && c.cache.get(&c.path, PasswordKind::Store).is_some())
                });
                u32::from(!stored)
            }
            _ => 1,
        }
    }

    /// Umgebung für `capabilities` (F-51/F-52, Richtlinie).
    pub(crate) fn environment(&self, interactive: bool, cfg: &ConfigState) -> Environment {
        let keystore_configured = if self.fixed.is_some() {
            let eff = cfg.effective();
            self.keystore_offered() && eff.keystore_enabled() && eff.keystore_file().is_some_and(Path::is_file)
        } else {
            self.keystore_configured
        };
        // Die Hintergrundprobe (5 s/30 s) kann beim Start noch kein Ergebnis haben: dann einmal
        // direkt prüfen (Loopback, kurzer Timeout), damit `-b LOCAL` nicht am Zeitpunkt scheitert.
        let localbku_reachable = self.availability.as_ref().is_some_and(|a| {
            a.is_available() || {
                let addr: std::net::SocketAddr = pida_backend_localbku::PROBE_ADDR.parse().expect("konstante Adresse");
                std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok()
            }
        });
        Environment {
            platform: self.platform,
            headless: !interactive,
            fido2_available: self.fido2.as_ref().is_some_and(|f| f.is_available()),
            localbku_reachable,
            keystore_configured,
            allowed_by_policy: cfg.allowed_backends(),
        }
    }

    /// Optionen der Backend-Auswahl in fester Reihenfolge (ID Austria, Karte, Keystore).
    pub(crate) fn options(&self, env: &Environment) -> Vec<BackendOption> {
        let mut all = self.all();
        all.sort_by_key(|b| match b.id() {
            BackendId::AtrustNative => 0,
            BackendId::LocalBku => 1,
            BackendId::Keystore => 2,
            _ => 3,
        });
        all.into_iter().map(|b| BackendOption { id: b.id(), capabilities: b.capabilities(env) }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_config::{MemorySecretStore, Overlay, Paths};

    #[test]
    fn secret_store_cache_roundtrip() {
        let store: Arc<dyn SecretStore> = Arc::new(MemorySecretStore::default());
        let cache = SecretStoreCache::new(store.clone(), "alias");
        let p = Path::new("/tmp/does-not-exist.p12");
        assert!(cache.get(p, PasswordKind::Store).is_none());
        cache.put(p, PasswordKind::Store, "test");
        cache.put(p, PasswordKind::Key, "key");
        assert_eq!(cache.get(p, PasswordKind::Store).unwrap().as_str(), "test");
        assert_eq!(store.get(&SecretId::keystore_key(p, "alias")).unwrap().unwrap().as_str(), "key");
        cache.forget(p, PasswordKind::Store);
        assert!(cache.get(p, PasswordKind::Store).is_none());
        assert!(!format!("{cache:?}").contains("test"));
    }

    /// `-n` (config) und gespeicherter Benutzername (backend-atrust) normalisieren gleich.
    #[test]
    fn phone_normalization_is_identical() {
        for n in ["0664 1234567", "0043 664 1234567", "+43664123", "103011234567", "user", "  0699 99 ", "0000664", ""]
        {
            assert_eq!(
                pida_config::normalize_mobile_number(n),
                pida_backend_atrust::validate::normalize_phone_number(n),
                "{n:?}"
            );
        }
    }

    #[test]
    fn storage_mapping() {
        assert_eq!(password_storage(pida_config::PasswordStorage::Keychain), PasswordStorage::Keychain);
        assert_eq!(password_storage(pida_config::PasswordStorage::None), PasswordStorage::None);
    }

    #[tokio::test]
    async fn registry_offers_keystore_only_when_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let ks = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.p12");
        let overlay = Overlay { keystore_file: Some(ks.into()), ..Overlay::default() };
        let (mut cfg, _) = ConfigState::load(
            Paths::under(tmp.path()),
            Overlay::default(),
            None,
            Arc::new(MemorySecretStore::default()),
            None,
            false,
        )
        .unwrap();
        // ohne TCP-Probe (kein Netzverkehr in Tests): lokale BKU registriert, aber nicht verfügbar
        let mut reg = Registry::new(Platform::Linux, None, "0.1.0", None, false);
        reg.configure(&cfg);
        let env = reg.environment(true, &cfg);
        let opts = reg.options(&env);
        assert_eq!(opts.iter().map(|o| o.id).collect::<Vec<_>>(), vec![BackendId::AtrustNative, BackendId::LocalBku]);
        assert!(opts[0].available());
        assert!(!opts[1].available());
        cfg.overlay = overlay;
        reg.configure(&cfg);
        let env = reg.environment(false, &cfg);
        let opts = reg.options(&env);
        let ks = opts.iter().find(|o| o.id == BackendId::Keystore).unwrap();
        assert!(ks.available());
        // ohne Oberfläche kein ID Austria
        assert!(!opts.iter().find(|o| o.id == BackendId::AtrustNative).unwrap().available());
    }

    /// iOS/Android (ADR 0004 Nachtrag 11): nur ID Austria – ein eingerichteter Keystore (Einstellung bzw.
    /// Overlay, Datei vorhanden) wird nicht registriert, auch nicht unter fest vorgegebenen Backends.
    #[tokio::test]
    async fn registry_offers_only_id_austria_on_mobile() {
        let tmp = tempfile::tempdir().unwrap();
        let ks = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.p12");
        let (mut cfg, _) = ConfigState::load(
            Paths::under(tmp.path()),
            Overlay { keystore_file: Some(ks.into()), ..Overlay::default() },
            None,
            Arc::new(MemorySecretStore::default()),
            None,
            false,
        )
        .unwrap();
        cfg.settings.keystore.enabled = true;
        cfg.settings.keystore.file = Some(ks.into());
        for platform in [Platform::Ios, Platform::Android] {
            let mut reg = Registry::new(platform, None, "0.1.0", None, false);
            reg.configure(&cfg);
            assert!(!reg.keystore_offered());
            let env = reg.environment(true, &cfg);
            assert!(!env.keystore_configured, "{platform:?}");
            let ids: Vec<BackendId> = reg.options(&env).iter().map(|o| o.id).collect();
            assert_eq!(ids, vec![BackendId::AtrustNative], "{platform:?}");
            assert!(reg.get(BackendId::Keystore).is_none());
        }
        // Desktop unverändert: Keystore registriert und verfügbar
        let mut reg = Registry::new(Platform::MacOs, None, "0.1.0", None, false);
        reg.configure(&cfg);
        let env = reg.environment(true, &cfg);
        assert!(reg.options(&env).iter().any(|o| o.id == BackendId::Keystore && o.available()));
        // fest vorgegebene Backends: auf Mobile ohne Keystore
        let key = KeystoreBackend::new(KeystoreConfig::new(ks));
        let fixed: Vec<Arc<dyn SignatureBackend>> = vec![Arc::new(key)];
        let reg = Registry::new(Platform::Ios, None, "0.1.0", Some(fixed.clone()), false);
        assert!(reg.all().is_empty());
        let reg = Registry::new(Platform::Linux, None, "0.1.0", Some(fixed), false);
        assert_eq!(reg.all().len(), 1);
    }

    /// „Merken“ bei der Anmeldung: der Kern speichert den Benutzernamen; das ID-Austria-Backend (mit dem
    /// im RAM gemerkten Passwort) bleibt bei der nächsten Konfiguration erhalten.
    #[test]
    fn remembered_login_keeps_the_atrust_instance() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut cfg, _) = ConfigState::load(
            Paths::under(tmp.path()),
            Overlay::default(),
            None,
            Arc::new(MemorySecretStore::default()),
            None,
            false,
        )
        .unwrap();
        let mut reg = Registry::new(Platform::Linux, None, "0.1.0", None, false);
        reg.configure(&cfg);
        let before = reg.atrust().unwrap();
        // BackendEvent::LoginSucceeded { remember: true } → Kern speichert
        cfg.settings.id_austria.username = Some("+436641234567".into());
        cfg.settings.id_austria.remember = true;
        reg.adopt_atrust_settings(&cfg);
        reg.configure(&cfg);
        assert!(Arc::ptr_eq(&before, &reg.atrust().unwrap()), "gemerktes Passwort bliebe sonst nicht erhalten");
        // echte Änderung der Einstellungen (Proxy) baut neu
        cfg.settings.proxy.mode = ProxyMode::None;
        reg.configure(&cfg);
        assert!(!Arc::ptr_eq(&before, &reg.atrust().unwrap()));
    }
}
