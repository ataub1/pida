//! Konfigurationsquelle für CLI `-c <name>` (Plan §7.4, docs/analyse/05 §1.3).
//!
//! - Relativer Name: zuerst `<config_dir>/<name>` (TOML), sonst `~/.pdf-over/<name>` als
//!   Java-Properties mit derselben Lese-Semantik wie beim Import – **ohne** zurückzuschreiben
//!   ([`ConfigSource::Legacy`] ist schreibgeschützt). So laufen Skripte mit eigenen
//!   Konfigurationsdateien weiter.
//! - Absoluter Pfad: diese Datei; TOML, wenn sie als Einstellungen lesbar ist, sonst Java-Properties.
//! - Fehlt die Datei, gibt es eine Fehlermeldung (`ng-config-file-missing`) statt des Werksreset-Dialogs
//!   von PDF-Over.

use std::path::{Path, PathBuf};

use pida_i18n::Lang;
use zeroize::Zeroizing;

use crate::legacy::LegacyConfig;
use crate::migrate::{MigrationContext, MigrationReport, migrate};
use crate::paths::Paths;
use crate::secrets::SecretId;
use crate::settings::{LoadOutcome, Settings};
use crate::{ConfigError, Result};

/// Aufgelöste Konfigurationsquelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    /// Neue Einstellungen (TOML), lesen und schreiben.
    Settings(PathBuf),
    /// Altdatei von PDF-Over (Java-Properties), nur lesen.
    Legacy(PathBuf),
}

impl ConfigSource {
    /// Pfad.
    pub fn path(&self) -> &Path {
        match self {
            ConfigSource::Settings(p) | ConfigSource::Legacy(p) => p,
        }
    }

    /// `true` für Altdateien (Änderungen werden nicht in die Datei geschrieben).
    pub fn is_read_only(&self) -> bool {
        matches!(self, ConfigSource::Legacy(_))
    }
}

fn looks_like_settings(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|t| Settings::from_toml(&t).is_ok() && t.contains("schema_version"))
}

/// Löst `-c <name>` auf; `legacy_dir` ist `~/.pdf-over` (falls vorhanden).
pub fn resolve_config_file(paths: &Paths, legacy_dir: Option<&Path>, name: &Path) -> Result<ConfigSource> {
    if name.is_absolute() {
        if !name.is_file() {
            return Err(missing(name));
        }
        return Ok(if looks_like_settings(name) {
            ConfigSource::Settings(name.to_path_buf())
        } else {
            ConfigSource::Legacy(name.to_path_buf())
        });
    }
    let new = paths.config_dir.join(name);
    if new.is_file() {
        return Ok(ConfigSource::Settings(new));
    }
    if let Some(dir) = legacy_dir {
        let old = dir.join(name);
        if old.is_file() {
            return Ok(ConfigSource::Legacy(old));
        }
    }
    Err(missing(&new))
}

fn missing(path: &Path) -> ConfigError {
    ConfigError::missing_file(path)
}

/// Lädt eine Quelle: TOML direkt, Altdateien über [`migrate`] (im Speicher, ohne Pfadprüfung des
/// Bildmarken-Caches außerhalb von `legacy_dir`). Liefert bei Altdateien zusätzlich den Bericht.
/// Klartext-Passwörter einer Altdatei werden – wie beim Import ohne Zustimmung – nicht verwendet.
pub fn load_config_source(source: &ConfigSource, system_language: Lang) -> Result<(Settings, Option<MigrationReport>)> {
    let (settings, report, _secrets) = load_config_source_with_secrets(source, system_language)?;
    Ok((settings, report))
}

/// Klartext-Passwörter einer Altdatei für **diese Sitzung**.
pub type SessionSecrets = Vec<(SecretId, Zeroizing<String>)>;

/// Wie [`load_config_source`], liefert bei Altdateien zusätzlich die Klartext-Passwörter, die PDF-Over
/// aus dieser Datei verwendet hätte (Keystore-Passwörter nur bei `KEYSTORE_PASS_STORE_TYPE=disk` und
/// mit Keystore-Datei, Proxy-Passwort nur mit Host und Benutzer). Damit laufen Skripte mit eigenen
/// Konfigurationsdateien (`-c`) auch ohne Rückfrage weiter.
///
/// Die Passwörter sind **nur für den RAM** bestimmt (z. B. ein [`crate::MemorySecretStore`] der Sitzung,
/// den [`crate::Effective`] wie den Schlüsselbund abfragt); sie werden nie gespeichert. Ob die Shell sie
/// verwendet, ist eine Entscheidung der Integration (Hinweis an die Nutzerin empfohlen).
pub fn load_config_source_with_secrets(
    source: &ConfigSource,
    system_language: Lang,
) -> Result<(Settings, Option<MigrationReport>, SessionSecrets)> {
    match source {
        ConfigSource::Settings(p) => match Settings::load(p) {
            LoadOutcome::Loaded(s) => Ok((*s, None, Vec::new())),
            LoadOutcome::Missing => Err(missing(p)),
            LoadOutcome::Unreadable(e) => Err(e),
        },
        ConfigSource::Legacy(p) => {
            let legacy = LegacyConfig::load(p)?;
            let legacy_dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
            let ctx = MigrationContext { legacy_dir, system_language, check_paths: true };
            let result = migrate(&legacy, &ctx);
            Ok((result.settings, Some(result.report), result.secrets))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BackendChoice;

    #[test]
    fn resolution_order() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let legacy_dir = home.path().join(".pdf-over");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::create_dir_all(&paths.config_dir).unwrap();

        let err = resolve_config_file(&paths, Some(&legacy_dir), Path::new("other.config")).unwrap_err();
        assert!(matches!(err, ConfigError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound));
        assert_eq!(err.message_id(), "ng-config-file-missing");
        assert_eq!(err.missing_path(), Some(paths.config_dir.join("other.config").as_path()));

        std::fs::write(legacy_dir.join("other.config"), "BKU=LOCAL\n").unwrap();
        let src = resolve_config_file(&paths, Some(&legacy_dir), Path::new("other.config")).unwrap();
        assert_eq!(src, ConfigSource::Legacy(legacy_dir.join("other.config")));
        assert!(src.is_read_only());
        let before = std::fs::read(src.path()).unwrap();
        let (s, report) = load_config_source(&src, Lang::De).unwrap();
        assert_eq!(s.backend.default, Some(BackendChoice::LocalBku));
        assert!(report.is_some());
        assert_eq!(std::fs::read(src.path()).unwrap(), before, "Altdatei unverändert");

        let mut toml = Settings::default();
        toml.backend.default = Some(BackendChoice::Keystore);
        toml.save_atomic(&paths.config_dir.join("other.config")).unwrap();
        let src = resolve_config_file(&paths, Some(&legacy_dir), Path::new("other.config")).unwrap();
        assert_eq!(src, ConfigSource::Settings(paths.config_dir.join("other.config")));
        assert_eq!(load_config_source(&src, Lang::De).unwrap().0.backend.default, Some(BackendChoice::Keystore));
    }

    #[test]
    fn legacy_source_secrets_for_the_session_only() {
        use crate::{Effective, MemorySecretStore, Overlay, SecretStore};
        let home = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let file = home.path().join("skript.config");
        std::fs::write(
            &file,
            "BKU=KS\nKEYSTORE_FILE=/x/k.p12\nKEYSTORE_PASS_STORE_TYPE=disk\nKEYSTORE_STOREPASS=test-only-pw\n",
        )
        .unwrap();
        let src = resolve_config_file(&paths, Some(home.path()), Path::new("skript.config")).unwrap();
        let (settings, report, secrets) = load_config_source_with_secrets(&src, Lang::De).unwrap();
        assert!(report.is_some());
        assert_eq!(secrets.len(), 1);
        // Nur im RAM: über einen Sitzungsspeicher findet Effective das Passwort wie im Schlüsselbund.
        let session = MemorySecretStore::default();
        for (id, value) in &secrets {
            session.set(id, value).unwrap();
        }
        let overlay = Overlay::default();
        let pw = Effective::new(&settings, &overlay, None).keystore_store_password(&session).unwrap();
        assert_eq!(pw.as_deref().map(String::as_str), Some("test-only-pw"));
        assert!(!settings.to_toml().unwrap().contains("test-only-pw"));
        // Ohne die Variante „with_secrets“ bleiben sie unberücksichtigt.
        assert_eq!(load_config_source(&src, Lang::De).unwrap().0, settings);
        // TOML-Quellen haben keine Klartext-Passwörter.
        Settings::default().save_atomic(&paths.config_dir.join("neu.toml")).unwrap();
        let toml_src = resolve_config_file(&paths, None, Path::new("neu.toml")).unwrap();
        assert!(load_config_source_with_secrets(&toml_src, Lang::De).unwrap().2.is_empty());
    }

    #[test]
    fn absolute_paths_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let toml = dir.path().join("a.toml");
        Settings::default().save_atomic(&toml).unwrap();
        let legacy = dir.path().join("b.config");
        std::fs::write(&legacy, "BKU=MOBILE\n").unwrap();
        assert!(matches!(resolve_config_file(&paths, None, &toml).unwrap(), ConfigSource::Settings(_)));
        assert!(matches!(resolve_config_file(&paths, None, &legacy).unwrap(), ConfigSource::Legacy(_)));
        assert!(resolve_config_file(&paths, None, &dir.path().join("fehlt")).is_err());
    }
}
