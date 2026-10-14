//! CLI-Overlay (nie persistiert) und effektive Werte (docs/analyse/05 §1.4, docs/analyse/01 §5).

use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::policy::Policy;
use crate::settings::{BackendChoice, Settings};

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
    /// `-v <true|false>` (B-07 korrigiert: Wert wird ausgewertet).
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

    /// Standard-Backend: Richtlinie > Overlay (nur ≠ NONE) > Einstellung.
    pub fn default_backend(&self) -> Option<BackendChoice> {
        todo!("Paket E")
    }

    /// Automatische Positionierung: Overlay **oder** Einstellung.
    pub fn auto_position(&self) -> bool {
        self.overlay.auto_position || self.settings.signature.auto_position
    }

    /// Unsichtbare Signatur erzwungen (`-v true`).
    pub fn force_invisible(&self) -> bool {
        self.overlay.invisible.unwrap_or(false)
    }

    /// Keystore aktiv (Einstellung oder `-ks`).
    pub fn keystore_enabled(&self) -> bool {
        self.settings.keystore.enabled || self.overlay.keystore_file.is_some()
    }

    /// Ausgabeziel (Overlay vor Einstellung).
    pub fn output(&self) -> Option<&'a PathBuf> {
        self.overlay.output.as_ref().or(self.settings.output.folder.as_ref())
    }

    /// Proxy-Port: Overlay nur, wenn gesetzt.
    pub fn proxy_port(&self) -> Option<u16> {
        self.overlay.proxy_port.or(self.settings.proxy.port)
    }

    /// Benutzername ID Austria.
    pub fn username(&self) -> Option<&'a str> {
        self.overlay.username.as_deref().or(self.settings.id_austria.username.as_deref())
    }
}
