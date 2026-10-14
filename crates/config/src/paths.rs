//! Plattformübliche Pfade (Plan §7.1): `%APPDATA%`, `~/Library/Application Support`,
//! `$XDG_CONFIG_HOME`, App-Sandbox. PDF-Over nutzte überall `~/.pdf-over` (docs/analyse/05 §1.1).

use std::path::{Path, PathBuf};

use crate::Result;

/// Name der Einstellungsdatei.
pub const SETTINGS_FILE: &str = "settings.toml";
/// Altordner relativ zum Home-Verzeichnis.
pub const LEGACY_DIR: &str = ".pdf-over";
/// Name der Altdatei.
pub const LEGACY_CONFIG_FILE: &str = "PDF-Over.config";

/// Alle Pfade der Anwendung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Konfiguration (`settings.toml`, eigene Profile).
    pub config_dir: PathBuf,
    /// Daten (Emblem-Cache, Zertifikats-Cache).
    pub data_dir: PathBuf,
    /// Cache.
    pub cache_dir: PathBuf,
    /// Logs (rotierend ≤ 2 MB, ohne Secrets und Dokumentinhalte).
    pub log_dir: PathBuf,
    /// App-privates Temp-Verzeichnis (Rechte 0600; persistierter `Prepared`-Zustand auf Mobile).
    pub temp_dir: PathBuf,
}

impl Paths {
    /// Pfade der Plattform (`directories::ProjectDirs`, Qualifier `at`, Organisation/App „PDF-Over NG“).
    pub fn platform() -> Result<Paths> {
        todo!("Paket E")
    }

    /// Alle Pfade unter einer Wurzel (Tests, Mobile-Sandbox, portable Installation).
    pub fn under(root: &Path) -> Paths {
        Paths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            temp_dir: root.join("tmp"),
        }
    }

    /// `settings.toml`.
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE)
    }

    /// Altordner `~/.pdf-over`, falls vorhanden.
    pub fn legacy_dir() -> Option<PathBuf> {
        todo!("Paket E")
    }

    /// „Einstellungen zurücksetzen“: verschiebt den Konfigurationsordner nach `<ordner>.old`
    /// (vorher vorhandenes `.old` wird ersetzt), statt zu löschen (F-05, Plan §7.1).
    pub fn reset(&self) -> Result<PathBuf> {
        todo!("Paket E")
    }

    /// Legt alle Ordner an.
    pub fn ensure(&self) -> Result<()> {
        todo!("Paket E")
    }
}
