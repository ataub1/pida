//! Plattformübliche Pfade (Plan §7.1): `%APPDATA%`, `~/Library/Application Support`,
//! `$XDG_CONFIG_HOME`, App-Sandbox. PDF-Over nutzte überall `~/.pdf-over` (docs/analyse/05 §1.1).
//!
//! | Zweck | Windows | macOS | Linux/BSD |
//! |---|---|---|---|
//! | Konfiguration | `%APPDATA%\PIDA\config` | `~/Library/Application Support/at.pida` | `$XDG_CONFIG_HOME/pida` |
//! | Daten (Bildmarke, Zertifikats-Cache) | `%APPDATA%\PIDA\data` | wie Konfiguration | `$XDG_DATA_HOME/pida` |
//! | Cache | `%LOCALAPPDATA%\PIDA\cache` | `~/Library/Caches/at.pida` | `$XDG_CACHE_HOME/pida` |
//! | Logs | `%LOCALAPPDATA%\PIDA\data\logs` | `~/Library/Logs/at.pida` | `$XDG_STATE_HOME/pida/logs` |
//! | Temp (privat, 0700) | `%LOCALAPPDATA%\PIDA\data\tmp` | `…/at.pida/tmp` | `$XDG_DATA_HOME/pida/tmp` |
//!
//! Mobile (iOS/Android) und portable Installationen übergeben ihre Sandbox-Ordner
//! ([`Paths::sandbox`], [`Paths::under`]).

use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

use crate::{ConfigError, Result};

/// Name der Einstellungsdatei.
pub const SETTINGS_FILE: &str = "settings.toml";
/// Altordner relativ zum Home-Verzeichnis.
pub const LEGACY_DIR: &str = ".pdf-over";
/// Name der Altdatei.
pub const LEGACY_CONFIG_FILE: &str = "PDF-Over.config";
/// Anzeigename bzw. Ordnername unter Windows.
pub const APP_NAME: &str = "PIDA";
/// Ordnername unter macOS (Bundle-ID-Form).
pub const MACOS_APP_DIR: &str = "at.pida";
/// Ordnername unter Linux/BSD (wie `/etc/pida/policy.toml`).
pub const UNIX_APP_DIR: &str = "pida";
/// Name der Logdatei im Log-Ordner.
pub const LOG_FILE: &str = "pida.log";
/// Größe, ab der die Logdatei rotiert wird (4 Dateien à 512 KiB ≤ 2 MB, Plan §7.1).
pub const LOG_MAX_FILE_BYTES: u64 = 512 * 1024;
/// Anzahl aufbewahrter rotierter Logdateien (`.1` … `.3`).
pub const LOG_KEEP: u32 = 3;

/// Umgebungsvariable für eine portable bzw. isolierte Installation: Ist sie gesetzt, liegen
/// Einstellungen, Daten, Cache, Logs und Temp-Dateien unter `$PIDA_HOME/{config,data,cache,logs,tmp}`,
/// und die PDF-Over-Altkonfiguration wird in `$PIDA_HOME/.pdf-over` gesucht.
pub const HOME_ENV: &str = "PIDA_HOME";

/// Ordner neben `pida.exe`, der unter Windows den **portablen Modus** einschaltet (ZIP-Paket, Backlog Nr. 31): Gibt
/// es ihn, liegen alle Ordner darin ([`Paths::under`]); [`HOME_ENV`] hat Vorrang. Die Shell merkt Passwörter dann
/// nicht dauerhaft (kein Schlüsselbund, Entscheidung vom 20.10.2026) und hält auch die Daten der WebView und den
/// Fensterzustand aus dem Benutzerprofil heraus.
pub const PORTABLE_DIR: &str = "PIDA-Daten";

/// Portabler Modus ([`PORTABLE_DIR`] neben dem Programm, nur Windows, ohne [`HOME_ENV`]): der Datenordner.
pub fn portable_root() -> Option<PathBuf> {
    if !cfg!(windows) || home_override().is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    portable_root_in(exe.parent()?)
}

/// [`PORTABLE_DIR`] im Ordner `dir`, falls vorhanden (ein Ordner, keine Datei).
pub fn portable_root_in(dir: &Path) -> Option<PathBuf> {
    let root = dir.join(PORTABLE_DIR);
    root.is_dir().then_some(root)
}

fn home_override() -> Option<PathBuf> {
    home_override_from(std::env::var_os(HOME_ENV))
}

fn home_override_from(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let value = value.filter(|v| !v.is_empty())?;
    let path = PathBuf::from(value);
    Some(std::path::absolute(&path).unwrap_or(path))
}

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
    /// Pfade der Plattform (`directories::ProjectDirs` mit den Ordnernamen aus der Tabelle oben).
    ///
    /// Ist [`HOME_ENV`] (`PIDA_HOME`) gesetzt, liegen alle Ordner darunter ([`Paths::under`]) –
    /// portable Installation bzw. isolierte Tests. Unter Windows lassen sich die Profilordner sonst
    /// nicht umlenken (Known-Folder-API statt `APPDATA`/`HOME`). Ohne `PIDA_HOME` gilt unter Windows ein Ordner
    /// [`PORTABLE_DIR`] neben dem Programm ([`portable_root`]).
    pub fn platform() -> Result<Paths> {
        if let Some(root) = home_override().or_else(portable_root) {
            return Ok(Paths::under(&root));
        }
        let project_path = if cfg!(target_os = "windows") {
            APP_NAME
        } else if cfg!(any(target_os = "macos", target_os = "ios")) {
            MACOS_APP_DIR
        } else {
            UNIX_APP_DIR
        };
        let dirs = ProjectDirs::from_path(PathBuf::from(project_path)).ok_or(ConfigError::NoHomeDirectory)?;
        let local = dirs.data_local_dir().to_path_buf();
        let log_dir = if cfg!(target_os = "macos") {
            BaseDirs::new()
                .map(|b| b.home_dir().join("Library/Logs").join(MACOS_APP_DIR))
                .unwrap_or_else(|| local.join("logs"))
        } else if let Some(state) = dirs.state_dir() {
            state.join("logs")
        } else {
            local.join("logs")
        };
        Ok(Paths {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
            cache_dir: dirs.cache_dir().to_path_buf(),
            log_dir,
            temp_dir: local.join("tmp"),
        })
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

    /// App-Sandbox (iOS/Android): `app_data` (gesichert, nicht bereinigbar) und `app_cache` (darf das
    /// System leeren). Der persistierte Signierzustand liegt in `app_data/tmp`, nicht im Cache.
    pub fn sandbox(app_data: &Path, app_cache: &Path) -> Paths {
        Paths {
            config_dir: app_data.join("config"),
            data_dir: app_data.join("data"),
            cache_dir: app_cache.to_path_buf(),
            log_dir: app_data.join("logs"),
            temp_dir: app_data.join("tmp"),
        }
    }

    /// `settings.toml`.
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE)
    }

    /// Aktuelle Logdatei.
    pub fn log_file(&self) -> PathBuf {
        self.log_dir.join(LOG_FILE)
    }

    /// Ordner des Zertifikats-Caches ([`crate::CertificateCache`]).
    pub fn certificate_cache_dir(&self) -> PathBuf {
        self.data_dir.join("certificates")
    }

    /// Ordner für importierte Bildmarken.
    pub fn emblem_dir(&self) -> PathBuf {
        self.data_dir.join("emblem")
    }

    /// Altordner `~/.pdf-over`, falls vorhanden.
    pub fn legacy_dir() -> Option<PathBuf> {
        let home = match home_override() {
            Some(home) => home,
            None => BaseDirs::new()?.home_dir().to_path_buf(),
        };
        Paths::legacy_dir_in(&home)
    }

    /// Altordner unter einem bestimmten Home-Verzeichnis, falls vorhanden.
    pub fn legacy_dir_in(home: &Path) -> Option<PathBuf> {
        let dir = home.join(LEGACY_DIR);
        dir.is_dir().then_some(dir)
    }

    /// „Einstellungen zurücksetzen“: verschiebt den Konfigurationsordner nach `<ordner>.old`
    /// (vorher vorhandenes `.old` wird ersetzt), statt zu löschen (F-05, Plan §7.1).
    ///
    /// Liefert den Sicherungsordner; fehlt der Konfigurationsordner, wird nichts verschoben. Gelingt
    /// das Umbenennen nicht (z. B. offene Dateien unter Windows), wird kopiert und danach gelöscht.
    /// Unter macOS ist der Konfigurationsordner zugleich der Datenordner.
    pub fn reset(&self) -> Result<PathBuf> {
        let backup = backup_path(&self.config_dir);
        if !self.config_dir.exists() {
            return Ok(backup);
        }
        if backup.is_dir() {
            std::fs::remove_dir_all(&backup)?;
        } else if backup.exists() {
            std::fs::remove_file(&backup)?;
        }
        if std::fs::rename(&self.config_dir, &backup).is_err() {
            copy_dir_all(&self.config_dir, &backup)?;
            std::fs::remove_dir_all(&self.config_dir)?;
        }
        tracing::info!("Einstellungen zurückgesetzt, Sicherung angelegt");
        Ok(backup)
    }

    /// Legt alle Ordner an; Temp-Ordner mit Rechten 0700 (Unix).
    pub fn ensure(&self) -> Result<()> {
        for d in [&self.config_dir, &self.data_dir, &self.cache_dir, &self.log_dir, &self.temp_dir] {
            std::fs::create_dir_all(d)?;
        }
        restrict_dir(&self.temp_dir)?;
        Ok(())
    }

    /// Rotiert die Logdatei beim Start, wenn sie `max_bytes` erreicht hat (`.log` → `.log.1` …
    /// `.log.<keep>`, die älteste fällt weg); legt den Log-Ordner an und liefert die Logdatei.
    pub fn rotate_logs(&self, max_bytes: u64, keep: u32) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.log_dir)?;
        let log = self.log_file();
        let size = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
        if size < max_bytes {
            return Ok(log);
        }
        let numbered = |i: u32| self.log_dir.join(format!("{LOG_FILE}.{i}"));
        if keep == 0 {
            std::fs::remove_file(&log)?;
            return Ok(log);
        }
        let _ = std::fs::remove_file(numbered(keep));
        for i in (1..keep).rev() {
            let from = numbered(i);
            if from.exists() {
                std::fs::rename(&from, numbered(i + 1))?;
            }
        }
        std::fs::rename(&log, numbered(1))?;
        Ok(log)
    }
}

/// `<pfad>.old`
fn backup_path(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".old");
    dir.with_file_name(name)
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Rechte 0700 (Unix); anderswo ohne Wirkung (Windows: Benutzerprofil ist bereits privat).
pub(crate) fn restrict_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let _ = dir;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_paths_are_distinct_and_named() {
        let Ok(p) = Paths::platform() else { return };
        let all = [&p.config_dir, &p.data_dir, &p.cache_dir, &p.log_dir, &p.temp_dir];
        for d in all {
            assert!(d.is_absolute(), "{}", d.display());
            let s = d.to_string_lossy().to_lowercase();
            assert!(s.contains("pida"), "{s}");
            assert!(!s.contains(".pdf-over/"), "nicht der Altordner: {s}");
        }
        assert_ne!(p.cache_dir, p.config_dir);
        assert_ne!(p.log_dir, p.config_dir);
        assert!(p.settings_file().ends_with(SETTINGS_FILE));
    }

    /// Portabler Modus (Backlog Nr. 31): nur ein Ordner `PIDA-Daten` zählt, keine gleichnamige Datei.
    #[test]
    fn portable_root_needs_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(portable_root_in(dir.path()), None);
        std::fs::write(dir.path().join(PORTABLE_DIR), b"").unwrap();
        assert_eq!(portable_root_in(dir.path()), None, "Datei statt Ordner");
        std::fs::remove_file(dir.path().join(PORTABLE_DIR)).unwrap();
        std::fs::create_dir(dir.path().join(PORTABLE_DIR)).unwrap();
        let root = portable_root_in(dir.path()).expect("Ordner");
        assert_eq!(Paths::under(&root).settings_file(), dir.path().join("PIDA-Daten/config/settings.toml"));
        if !cfg!(windows) {
            assert_eq!(portable_root(), None, "nur Windows");
        }
    }

    #[test]
    fn sandbox_and_under() {
        let s = Paths::sandbox(Path::new("/app/data"), Path::new("/app/cache"));
        assert_eq!(s.settings_file(), PathBuf::from("/app/data/config/settings.toml"));
        assert_eq!(s.cache_dir, PathBuf::from("/app/cache"));
        assert_eq!(s.temp_dir, PathBuf::from("/app/data/tmp"));
        let u = Paths::under(Path::new("/r"));
        assert_eq!(u.log_file(), PathBuf::from("/r/logs/pida.log"));
        assert_eq!(u.certificate_cache_dir(), PathBuf::from("/r/data/certificates"));
    }

    #[test]
    fn reset_moves_to_old_and_replaces_previous_backup() {
        let root = tempfile::tempdir().unwrap();
        let p = Paths::under(root.path());
        p.ensure().unwrap();
        std::fs::write(p.settings_file(), "schema_version = 1\n").unwrap();
        let backup = p.reset().unwrap();
        assert_eq!(backup, root.path().join("config.old"));
        assert!(!p.config_dir.exists());
        assert!(backup.join(SETTINGS_FILE).is_file());
        // zweiter Reset ersetzt die alte Sicherung
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(p.config_dir.join("neu.txt"), "x").unwrap();
        p.reset().unwrap();
        assert!(backup.join("neu.txt").is_file());
        assert!(!backup.join(SETTINGS_FILE).exists());
        // ohne Konfigurationsordner: nichts zu tun
        assert_eq!(p.reset().unwrap(), backup);
        assert!(backup.join("neu.txt").is_file());
    }

    #[test]
    fn home_override_is_absolute_and_ignores_empty() {
        assert_eq!(home_override_from(None), None);
        assert_eq!(home_override_from(Some("".into())), None);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(home_override_from(Some(dir.path().into())), Some(dir.path().to_path_buf()));
        let rel = home_override_from(Some("portable".into())).unwrap();
        assert!(rel.is_absolute() && rel.ends_with("portable"), "{rel:?}");
        let p = Paths::under(&rel);
        assert!(p.settings_file().starts_with(&rel));
    }

    #[test]
    fn legacy_dir_detection() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(Paths::legacy_dir_in(home.path()), None);
        std::fs::create_dir(home.path().join(LEGACY_DIR)).unwrap();
        assert_eq!(Paths::legacy_dir_in(home.path()), Some(home.path().join(LEGACY_DIR)));
    }

    #[test]
    fn log_rotation() {
        let root = tempfile::tempdir().unwrap();
        let p = Paths::under(root.path());
        let log = p.rotate_logs(10, 2).unwrap();
        assert!(!log.exists());
        for round in 0..4 {
            std::fs::write(&log, format!("runde {round} ....")).unwrap();
            p.rotate_logs(10, 2).unwrap();
        }
        assert!(!log.exists());
        assert_eq!(std::fs::read_to_string(p.log_dir.join("pida.log.1")).unwrap(), "runde 3 ....");
        assert_eq!(std::fs::read_to_string(p.log_dir.join("pida.log.2")).unwrap(), "runde 2 ....");
        assert!(!p.log_dir.join("pida.log.3").exists());
        std::fs::write(&log, "klein").unwrap();
        p.rotate_logs(10, 2).unwrap();
        assert!(log.exists());
    }

    #[cfg(unix)]
    #[test]
    fn temp_dir_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let p = Paths::under(root.path());
        p.ensure().unwrap();
        let mode = std::fs::metadata(&p.temp_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}
