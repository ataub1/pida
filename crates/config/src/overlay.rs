//! CLI-Overlay (nie persistiert) und effektive Werte (docs/analyse/05 §1.4, docs/analyse/01 §5).
//!
//! Vorrang wie `ConfigurationManager` (Getter ohne Suffix = Overlay vor Persistent), ergänzt um die
//! Richtlinie, die immer gewinnt:
//!
//! | Wert | Regel |
//! |---|---|
//! | Standard-Backend | Richtlinie (`forced_backend`) > Overlay `-b` (nur ≠ `NONE`) > Einstellung; nicht erlaubte Backends fallen weg |
//! | Auto-Positionierung | Overlay **oder** Einstellung |
//! | Proxy-Port | Overlay nur, wenn gesetzt (PDF-Over: ≠ −1) |
//! | Keystore aktiv | Einstellung **oder** `-ks` |
//! | Keystore-Passwörter | Overlay; sonst Schlüsselbund nur bei Speicherart „Permanent“ |
//! | ID-Austria-Passwort | **nur** Overlay (`-p`, RAM) |
//! | Unsichtbar | `-v` mit nicht verneinendem Wert (B-07) |
//!
//! Die Einstellungen werden nur aus [`Settings`] gespeichert; das [`Overlay`] erreicht nie die Datei.

use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::Result;
use crate::policy::Policy;
use crate::secrets::{SecretId, SecretStore};
use crate::settings::{BackendChoice, PasswordStorage, ProxyMode, ProxySettings, Settings, SignatureProfile};

/// Werte aus der Kommandozeile bzw. „Öffnen mit“.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Overlay {
    /// `-i` bzw. nackte Tokens (erstes = aktuelles Dokument, weitere = Warteschlange).
    pub input_files: Vec<PathBuf>,
    /// `-o` (Ordner oder Datei).
    pub output: Option<PathBuf>,
    /// `-b` (`NONE` → `None`: persistente Einstellung gilt).
    pub backend: Option<BackendChoice>,
    /// `-n` (normalisiert).
    pub username: Option<String>,
    /// `-p` (Szenario A: nur RAM; Länge 5–200).
    pub password: Option<Zeroizing<String>>,
    /// `-ks` (aktiviert zugleich den Keystore).
    pub keystore_file: Option<PathBuf>,
    /// `-kstype`
    pub keystore_type: Option<String>,
    /// `-kspass` bzw. `--kspass-stdin`/`--kspass-env`
    pub keystore_store_password: Option<Zeroizing<String>>,
    /// `-ksalias`
    pub keystore_alias: Option<String>,
    /// `-kskeypass`
    pub keystore_key_password: Option<Zeroizing<String>>,
    /// `-proxy`
    pub proxy_host: Option<String>,
    /// `-proxyport` (1–65535)
    pub proxy_port: Option<u16>,
    /// `-proxyuser`
    pub proxy_user: Option<String>,
    /// `-proxypass`
    pub proxy_password: Option<Zeroizing<String>>,
    /// `-e`
    pub emblem: Option<PathBuf>,
    /// `-a`
    pub auto_position: bool,
    /// `-s`
    pub skip_finish: bool,
    /// `-v <wert>` (B-07: `false`/`0`/`no`/`nein` = sichtbar, sonst wie PDF-Over unsichtbar).
    pub invisible: Option<bool>,
    /// `-c` (relativ zum Konfigurationsordner; Altdateien unter `~/.pdf-over/<name>`).
    pub config_file: Option<PathBuf>,
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |o: &Option<Zeroizing<String>>| o.as_ref().map(|_| "***");
        f.debug_struct("Overlay")
            .field("input_files", &self.input_files)
            .field("output", &self.output)
            .field("backend", &self.backend)
            .field("username", &self.username)
            .field("password", &redact(&self.password))
            .field("keystore_file", &self.keystore_file)
            .field("keystore_type", &self.keystore_type)
            .field("keystore_store_password", &redact(&self.keystore_store_password))
            .field("keystore_alias", &self.keystore_alias)
            .field("keystore_key_password", &redact(&self.keystore_key_password))
            .field("proxy_host", &self.proxy_host)
            .field("proxy_port", &self.proxy_port)
            .field("proxy_user", &self.proxy_user)
            .field("proxy_password", &redact(&self.proxy_password))
            .field("emblem", &self.emblem)
            .field("auto_position", &self.auto_position)
            .field("skip_finish", &self.skip_finish)
            .field("invisible", &self.invisible)
            .field("config_file", &self.config_file)
            .finish()
    }
}

/// Mindest- und Höchstlänge des ID-Austria-Passworts (`MobileBKUValidator.validatePassword`).
pub const ID_AUSTRIA_PASSWORD_LEN: std::ops::RangeInclusive<usize> = 5..=200;

/// Prüft `-p` wie PDF-Over (Länge in UTF-16-Einheiten wie `String.length()`); liefert bei einem
/// Problem die Meldungs-ID (`exception-PasswordTooShort` bzw. `exception-PasswordTooLong`).
pub fn password_problem(password: &str) -> Option<&'static str> {
    let len = password.encode_utf16().count();
    if len < *ID_AUSTRIA_PASSWORD_LEN.start() {
        Some("exception-PasswordTooShort")
    } else if len > *ID_AUSTRIA_PASSWORD_LEN.end() {
        Some("exception-PasswordTooLong")
    } else {
        None
    }
}

/// Wert von `-v`: `false`/`0`/`no`/`nein`/`off` (ohne Groß-/Kleinschreibung) → sichtbar, **jeder
/// andere** Wert → unsichtbar (ADR 0001 §8.9; Skripte mit `-v x` laufen weiter).
pub fn invisible_from_cli_value(value: &str) -> bool {
    !matches!(value.trim().to_ascii_lowercase().as_str(), "false" | "0" | "no" | "nein" | "off")
}

/// Liest zuerst mit kanonischem Pfad, dann mit dem Pfad wie gespeichert (die Datei kann beim Import
/// gefehlt haben, z. B. Laufwerk nicht verbunden, oder über einen Symlink erreichbar sein).
fn get_with_raw_fallback(
    store: &dyn SecretStore,
    canonical: SecretId,
    raw: impl FnOnce() -> SecretId,
) -> Result<Option<Zeroizing<String>>> {
    if let Some(v) = store.get(&canonical)? {
        return Ok(Some(v));
    }
    let raw = raw();
    if raw == canonical { Ok(None) } else { store.get(&raw) }
}

/// Sicht auf Einstellungen + Overlay + Richtlinie mit den Vorrangregeln von PDF-Over.
#[derive(Debug, Clone, Copy)]
pub struct Effective<'a> {
    /// Persistente Einstellungen.
    pub settings: &'a Settings,
    /// Overlay.
    pub overlay: &'a Overlay,
    /// Richtlinie (gewinnt immer).
    pub policy: Option<&'a Policy>,
}

impl<'a> Effective<'a> {
    /// Neue Sicht.
    pub fn new(settings: &'a Settings, overlay: &'a Overlay, policy: Option<&'a Policy>) -> Self {
        Self { settings, overlay, policy }
    }

    fn allowed(&self, b: BackendChoice) -> bool {
        self.policy.is_none_or(|p| p.allows(b))
    }

    /// Standard-Backend: Richtlinie > Overlay (nur ≠ NONE) > Einstellung. Von der Richtlinie nicht
    /// erlaubte Werte fallen weg (dann gilt der nächste bzw. die Auswahl wird angezeigt).
    pub fn default_backend(&self) -> Option<BackendChoice> {
        if let Some(forced) = self.policy.and_then(|p| p.forced_backend) {
            return Some(forced);
        }
        [self.overlay.backend, self.settings.backend.default].into_iter().flatten().find(|b| self.allowed(*b))
    }

    /// Backends, die angeboten werden dürfen (Richtlinie; Keystore nur, wenn aktiviert).
    pub fn offered_backends(&self) -> Vec<BackendChoice> {
        [BackendChoice::IdAustria, BackendChoice::LocalBku, BackendChoice::Keystore]
            .into_iter()
            .filter(|b| self.allowed(*b))
            .filter(|b| *b != BackendChoice::Keystore || self.keystore_enabled())
            .collect()
    }

    /// Automatische Positionierung: Overlay **oder** Einstellung.
    pub fn auto_position(&self) -> bool {
        self.overlay.auto_position || self.settings.signature.auto_position
    }

    /// Unsichtbare Signatur erzwungen (`-v` mit nicht verneinendem Wert).
    pub fn force_invisible(&self) -> bool {
        self.overlay.invisible.unwrap_or(false)
    }

    /// Effektives Profil: `-v` → `INVISIBLE`; sonst Richtlinie, sonst Einstellung (ein von der
    /// Richtlinie ausgeblendetes Amtssignatur-Profil wird zum Standard-Signaturblock).
    pub fn profile(&self) -> SignatureProfile {
        if self.force_invisible() {
            return SignatureProfile::Invisible;
        }
        if let Some(p) = self.policy.and_then(|p| p.profile) {
            return p;
        }
        let p = self.settings.signature.profile;
        if self.policy.is_some_and(|pol| !pol.allows_profile(p)) { SignatureProfile::SignaturblockSmall } else { p }
    }

    /// Keystore aktiv (Einstellung oder `-ks`).
    pub fn keystore_enabled(&self) -> bool {
        self.settings.keystore.enabled || self.overlay.keystore_file.is_some()
    }

    /// Keystore-Datei (Overlay vor Einstellung).
    pub fn keystore_file(&self) -> Option<&'a Path> {
        self.overlay.keystore_file.as_deref().or(self.settings.keystore.file.as_deref())
    }

    /// Keystore-Typ (Overlay vor Einstellung).
    pub fn keystore_type(&self) -> Option<&'a str> {
        self.overlay.keystore_type.as_deref().or(self.settings.keystore.declared_type.as_deref())
    }

    /// Schlüssel-Alias (Overlay vor Einstellung).
    pub fn keystore_alias(&self) -> Option<&'a str> {
        self.overlay.keystore_alias.as_deref().or(self.settings.keystore.alias.as_deref())
    }

    /// `true`, wenn die Keystore-Datei aus dem Overlay stammt (dann werden Passwörter nie
    /// persistiert, `KSState.java:119-129`).
    pub fn keystore_from_overlay(&self) -> bool {
        self.overlay.keystore_file.is_some()
    }

    /// Keystore-Passwort: Overlay; sonst aus dem Schlüsselbund nur bei Speicherart „Permanent“ und
    /// nur für die persistente Keystore-Datei.
    pub fn keystore_store_password(&self, store: &dyn SecretStore) -> Result<Option<Zeroizing<String>>> {
        if let Some(p) = &self.overlay.keystore_store_password {
            return Ok(Some(p.clone()));
        }
        match self.persistent_keystore() {
            Some(file) => get_with_raw_fallback(store, SecretId::keystore_store(file), || {
                SecretId::KeystoreStorePassword { keystore: file.to_path_buf() }
            }),
            None => Ok(None),
        }
    }

    /// Schlüssel-Passwort (Regeln wie [`Effective::keystore_store_password`]).
    pub fn keystore_key_password(&self, store: &dyn SecretStore) -> Result<Option<Zeroizing<String>>> {
        if let Some(p) = &self.overlay.keystore_key_password {
            return Ok(Some(p.clone()));
        }
        let alias = self.keystore_alias().unwrap_or("");
        match self.persistent_keystore() {
            Some(file) => get_with_raw_fallback(store, SecretId::keystore_key(file, alias), || {
                SecretId::KeystoreKeyPassword { keystore: file.to_path_buf(), alias: alias.to_owned() }
            }),
            None => Ok(None),
        }
    }

    fn persistent_keystore(&self) -> Option<&'a Path> {
        if self.settings.keystore.password_storage != PasswordStorage::Keychain || self.keystore_from_overlay() {
            return None;
        }
        self.settings.keystore.file.as_deref()
    }

    /// Ausgabeziel (Overlay vor Einstellung).
    pub fn output(&self) -> Option<&'a PathBuf> {
        self.overlay.output.as_ref().or(self.settings.output.folder.as_ref())
    }

    /// Bildmarke (Overlay `-e` vor Einstellung).
    pub fn emblem(&self) -> Option<&'a Path> {
        self.overlay.emblem.as_deref().or(self.settings.signature.emblem.as_deref())
    }

    /// Effektiver Proxy: Richtlinie > Overlay (Host/Port/Benutzer einzeln) > Einstellung. Ein Host im
    /// Overlay schaltet auf „manuell“.
    pub fn proxy(&self) -> ProxySettings {
        if let Some(p) = self.policy.and_then(|p| p.proxy.clone()) {
            return p;
        }
        let s = &self.settings.proxy;
        let host = self.overlay.proxy_host.clone().or_else(|| s.host.clone());
        let mode = if self.overlay.proxy_host.is_some() { ProxyMode::Manual } else { s.mode };
        ProxySettings { mode, host, port: self.proxy_port(), user: self.proxy_user().map(str::to_owned) }
    }

    /// Proxy-Port: Overlay nur, wenn gesetzt.
    pub fn proxy_port(&self) -> Option<u16> {
        self.overlay.proxy_port.or(self.settings.proxy.port)
    }

    /// Proxy-Benutzer (Overlay vor Einstellung).
    pub fn proxy_user(&self) -> Option<&'a str> {
        self.overlay.proxy_user.as_deref().or(self.settings.proxy.user.as_deref())
    }

    /// Proxy-Passwort: Overlay; sonst Schlüsselbund für den effektiven Host/Benutzer.
    pub fn proxy_password(&self, store: &dyn SecretStore) -> Result<Option<Zeroizing<String>>> {
        if let Some(p) = &self.overlay.proxy_password {
            return Ok(Some(p.clone()));
        }
        let proxy = self.proxy();
        match (proxy.host.as_deref(), proxy.user.as_deref()) {
            (Some(h), Some(u)) => store.get(&SecretId::proxy(h, u)),
            _ => Ok(None),
        }
    }

    /// Update-Check: Richtlinie vor Einstellung.
    pub fn update_check(&self) -> bool {
        self.policy.and_then(|p| p.update_check).unwrap_or(self.settings.general.update_check)
    }

    /// Benutzername ID Austria.
    pub fn username(&self) -> Option<&'a str> {
        self.overlay.username.as_deref().or(self.settings.id_austria.username.as_deref())
    }

    /// ID-Austria-Passwort: **nur** aus dem Overlay (`-p`), nie persistiert.
    pub fn password(&self) -> Option<&'a Zeroizing<String>> {
        self.overlay.password.as_ref()
    }

    /// Ergebnis-Bildschirm überspringen (`-s`).
    pub fn skip_finish(&self) -> bool {
        self.overlay.skip_finish
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;

    fn pw(s: &str) -> Option<Zeroizing<String>> {
        Some(Zeroizing::new(s.to_owned()))
    }

    #[test]
    fn backend_precedence() {
        let mut s = Settings::default();
        let mut o = Overlay::default();
        assert_eq!(Effective::new(&s, &o, None).default_backend(), None);
        s.backend.default = Some(BackendChoice::LocalBku);
        assert_eq!(Effective::new(&s, &o, None).default_backend(), Some(BackendChoice::LocalBku));
        o.backend = Some(BackendChoice::Keystore);
        assert_eq!(Effective::new(&s, &o, None).default_backend(), Some(BackendChoice::Keystore));
        let deny_ks = Policy { allowed_backends: Some(vec![BackendChoice::LocalBku]), ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&deny_ks)).default_backend(), Some(BackendChoice::LocalBku));
        let only_ida = Policy { allowed_backends: Some(vec![BackendChoice::IdAustria]), ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&only_ida)).default_backend(), None);
        let forced = Policy { forced_backend: Some(BackendChoice::IdAustria), ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&forced)).default_backend(), Some(BackendChoice::IdAustria));
    }

    #[test]
    fn offered_backends_respect_keystore_and_policy() {
        let mut s = Settings::default();
        let mut o = Overlay::default();
        assert_eq!(
            Effective::new(&s, &o, None).offered_backends(),
            vec![BackendChoice::IdAustria, BackendChoice::LocalBku]
        );
        o.keystore_file = Some("k.p12".into());
        assert_eq!(Effective::new(&s, &o, None).offered_backends().len(), 3);
        s.keystore.enabled = true;
        o.keystore_file = None;
        let p = Policy { allowed_backends: Some(vec![BackendChoice::Keystore]), ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&p)).offered_backends(), vec![BackendChoice::Keystore]);
    }

    #[test]
    fn flags_and_overrides() {
        let mut s = Settings::default();
        let mut o = Overlay::default();
        let e = Effective::new(&s, &o, None);
        assert!(!e.auto_position() && !e.force_invisible() && !e.keystore_enabled() && !e.skip_finish());
        s.signature.auto_position = true;
        assert!(Effective::new(&s, &o, None).auto_position());
        s.signature.auto_position = false;
        o.auto_position = true;
        o.invisible = Some(true);
        o.skip_finish = true;
        o.output = Some("/tmp/out.pdf".into());
        o.emblem = Some("/tmp/logo.png".into());
        s.output.folder = Some(".".into());
        s.signature.emblem = Some("/alt/logo.png".into());
        let e = Effective::new(&s, &o, None);
        assert!(e.auto_position() && e.force_invisible() && e.skip_finish());
        assert_eq!(e.profile(), SignatureProfile::Invisible);
        assert_eq!(e.output(), Some(&PathBuf::from("/tmp/out.pdf")));
        assert_eq!(e.emblem(), Some(Path::new("/tmp/logo.png")));
    }

    #[test]
    fn profile_policy() {
        let mut s = Settings::default();
        s.signature.profile = SignatureProfile::Amtssignaturblock;
        let o = Overlay::default();
        let hide = Policy { hide_amtssignatur: true, ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&hide)).profile(), SignatureProfile::SignaturblockSmall);
        assert_eq!(Effective::new(&s, &o, None).profile(), SignatureProfile::Amtssignaturblock);
        let fixed = Policy { profile: Some(SignatureProfile::BaseLogo), ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&fixed)).profile(), SignatureProfile::BaseLogo);
    }

    #[test]
    fn proxy_precedence() {
        let s = Settings {
            proxy: ProxySettings {
                mode: ProxyMode::Manual,
                host: Some("alt".into()),
                port: Some(8080),
                user: Some("u".into()),
            },
            ..Default::default()
        };
        let mut o = Overlay::default();
        assert_eq!(Effective::new(&s, &o, None).proxy_port(), Some(8080));
        o.proxy_port = Some(3128);
        o.proxy_host = Some("neu".into());
        let e = Effective::new(&s, &o, None);
        assert_eq!(
            e.proxy(),
            ProxySettings {
                mode: ProxyMode::Manual,
                host: Some("neu".into()),
                port: Some(3128),
                user: Some("u".into())
            }
        );
        let pol =
            Policy { proxy: Some(ProxySettings { mode: ProxyMode::None, ..Default::default() }), ..Default::default() };
        assert_eq!(Effective::new(&s, &o, Some(&pol)).proxy().mode, ProxyMode::None);
        let store = MemorySecretStore::default();
        store.set(&SecretId::proxy("neu", "u"), "test-pw").unwrap();
        assert_eq!(e.proxy_password(&store).unwrap().as_deref().map(String::as_str), Some("test-pw"));
        o.proxy_password = pw("cli");
        assert_eq!(
            Effective::new(&s, &o, None).proxy_password(&store).unwrap().as_deref().map(String::as_str),
            Some("cli")
        );
    }

    #[test]
    fn keystore_password_found_under_raw_path() {
        // Beim Import fehlte die Datei → Secret unter dem Rohpfad; jetzt existiert sie (kanonisch anders).
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join(".").join("k.p12");
        let store = MemorySecretStore::default();
        store.set(&SecretId::KeystoreStorePassword { keystore: raw.clone() }, "raw-pw").unwrap();
        std::fs::write(&raw, b"x").unwrap();
        assert_ne!(SecretId::keystore_store(&raw), SecretId::KeystoreStorePassword { keystore: raw.clone() });
        let mut s = Settings::default();
        s.keystore.file = Some(raw);
        s.keystore.password_storage = PasswordStorage::Keychain;
        let o = Overlay::default();
        let got = Effective::new(&s, &o, None).keystore_store_password(&store).unwrap();
        assert_eq!(got.as_deref().map(String::as_str), Some("raw-pw"));
        assert_eq!(Effective::new(&s, &o, None).keystore_key_password(&store).unwrap(), None);
    }

    #[test]
    fn keystore_passwords_only_from_keychain_when_permanent() {
        let store = MemorySecretStore::default();
        let mut s = Settings::default();
        s.keystore.file = Some("/nicht/da.p12".into());
        s.keystore.alias = Some("a".into());
        store.set(&SecretId::keystore_store(Path::new("/nicht/da.p12")), "store-pw").unwrap();
        store.set(&SecretId::keystore_key(Path::new("/nicht/da.p12"), "a"), "key-pw").unwrap();
        let mut o = Overlay::default();
        s.keystore.password_storage = PasswordStorage::Memory;
        assert_eq!(Effective::new(&s, &o, None).keystore_store_password(&store).unwrap(), None);
        s.keystore.password_storage = PasswordStorage::Keychain;
        let e = Effective::new(&s, &o, None);
        assert_eq!(e.keystore_store_password(&store).unwrap().as_deref().map(String::as_str), Some("store-pw"));
        assert_eq!(e.keystore_key_password(&store).unwrap().as_deref().map(String::as_str), Some("key-pw"));
        // Keystore aus dem Overlay: nie aus dem Schlüsselbund.
        o.keystore_file = Some("/anderer.p12".into());
        assert_eq!(Effective::new(&s, &o, None).keystore_store_password(&store).unwrap(), None);
        o.keystore_store_password = pw("cli");
        assert_eq!(
            Effective::new(&s, &o, None).keystore_store_password(&store).unwrap().as_deref().map(String::as_str),
            Some("cli")
        );
        assert_eq!(Effective::new(&s, &o, None).keystore_file(), Some(Path::new("/anderer.p12")));
    }

    #[test]
    fn username_password_update_check() {
        let mut s = Settings::default();
        s.id_austria.username = Some("+43664".into());
        let mut o = Overlay::default();
        assert_eq!(Effective::new(&s, &o, None).username(), Some("+43664"));
        o.username = Some("max".into());
        o.password = pw("geheim-test");
        let e = Effective::new(&s, &o, None);
        assert_eq!(e.username(), Some("max"));
        assert!(e.password().is_some());
        assert!(e.update_check());
        let pol = Policy { update_check: Some(false), ..Default::default() };
        assert!(!Effective::new(&s, &o, Some(&pol)).update_check());
        assert!(!format!("{o:?}").contains("geheim-test"));
    }

    #[test]
    fn cli_helpers() {
        assert_eq!(password_problem("1234"), Some("exception-PasswordTooShort"));
        assert_eq!(password_problem("12345"), None);
        assert_eq!(password_problem(&"x".repeat(201)), Some("exception-PasswordTooLong"));
        for v in ["false", "0", "no", "NEIN", " off "] {
            assert!(!invisible_from_cli_value(v), "{v}");
        }
        for v in ["true", "x", "1", ""] {
            assert!(invisible_from_cli_value(v), "{v}");
        }
    }

    #[test]
    fn overlay_is_never_persisted() {
        let s = Settings::default();
        let o = Overlay {
            username: Some("cli-user".into()),
            password: pw("cli-secret"),
            output: Some("/cli/out".into()),
            backend: Some(BackendChoice::Keystore),
            ..Default::default()
        };
        let e = Effective::new(&s, &o, None);
        assert_eq!(e.username(), Some("cli-user"));
        let toml = e.settings.to_toml().unwrap();
        assert!(!toml.contains("cli-user") && !toml.contains("cli-secret") && !toml.contains("/cli/out"));
        assert!(!toml.contains("keystore\"") && !toml.contains("default = \"keystore\""));
    }
}
