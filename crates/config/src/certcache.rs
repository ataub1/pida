//! Zertifikats-Cache (Opt-in, Plan §7.3): speichert **nur öffentliche** Signaturzertifikate (DER) je
//! Backend und Konto, damit nach der ersten Signatur keine zweite Bestätigung für das Zertifikat nötig
//! ist. Ohne Zustimmung ([`CertificateCacheSettings::enabled`]) liest und schreibt der Cache nichts.
//!
//! Dateiname = SHA-256 über Backend und Konto (hex) – Benutzernamen oder Pfade erscheinen weder im
//! Dateisystem noch im `Debug`-Text. Gültigkeit und Zuordnung (Fingerprint, Ablauf) prüft der Kern mit
//! `pdfover-cades`, bevor er ein Zertifikat aus dem Cache verwendet.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::paths::{Paths, restrict_dir};
use crate::settings::CertificateCacheSettings;
use crate::{ConfigError, Result};

/// Größte akzeptierte Zertifikatsgröße (DER).
pub const MAX_CERTIFICATE_BYTES: usize = 64 * 1024;

/// Schlüssel eines Cache-Eintrags.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CertCacheKey {
    /// Backend (z. B. `atrust-native`, `localbku`, `keystore`).
    pub backend: String,
    /// Konto (normalisierter Benutzername, Keystore-Pfad + Alias …).
    pub account: String,
}

impl CertCacheKey {
    /// Neuer Schlüssel.
    pub fn new(backend: impl Into<String>, account: impl Into<String>) -> Self {
        Self { backend: backend.into(), account: account.into() }
    }

    /// Hex-SHA-256 über `backend NUL account` (Dateiname ohne Endung).
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.backend.as_bytes());
        h.update([0u8]);
        h.update(self.account.as_bytes());
        hex::encode(h.finalize())
    }
}

impl std::fmt::Debug for CertCacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let d = self.digest();
        f.debug_struct("CertCacheKey").field("backend", &self.backend).field("account", &&d[..12]).finish()
    }
}

/// Zertifikats-Cache im Datenordner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateCache {
    dir: PathBuf,
    enabled: bool,
}

impl CertificateCache {
    /// Cache unter `<data_dir>/certificates` gemäß Einstellung.
    pub fn new(paths: &Paths, settings: &CertificateCacheSettings) -> Self {
        Self::with_dir(paths.certificate_cache_dir(), settings.enabled)
    }

    /// Cache in einem bestimmten Ordner.
    pub fn with_dir(dir: impl Into<PathBuf>, enabled: bool) -> Self {
        Self { dir: dir.into(), enabled }
    }

    /// Opt-in aktiv?
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Ordner.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, key: &CertCacheKey) -> PathBuf {
        self.dir.join(format!("{}.der", key.digest()))
    }

    /// Zertifikat lesen (`None`, wenn deaktiviert, nicht vorhanden oder offensichtlich kein DER).
    pub fn get(&self, key: &CertCacheKey) -> Result<Option<Vec<u8>>> {
        if !self.enabled {
            return Ok(None);
        }
        match std::fs::read(self.file(key)) {
            Ok(der) if plausible_der(&der) => Ok(Some(der)),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(ConfigError::Io(e)),
        }
    }

    /// Zertifikat speichern (ohne Opt-in: keine Wirkung). Nur DER-`SEQUENCE` bis
    /// [`MAX_CERTIFICATE_BYTES`].
    pub fn put(&self, key: &CertCacheKey, der: &[u8]) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if !plausible_der(der) {
            return Err(ConfigError::Serialize("kein DER-Zertifikat".into()));
        }
        std::fs::create_dir_all(&self.dir)?;
        restrict_dir(&self.dir)?;
        crate::settings::write_atomic(&self.file(key), der)
    }

    /// Eintrag entfernen (z. B. Zertifikat abgelaufen oder passt nicht mehr).
    pub fn remove(&self, key: &CertCacheKey) -> Result<()> {
        match std::fs::remove_file(self.file(key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ConfigError::Io(e)),
        }
    }

    /// Alle Einträge löschen (auch bei deaktiviertem Cache, z. B. nach Widerruf des Opt-ins);
    /// liefert die Anzahl entfernter Einträge.
    pub fn clear(&self) -> Result<usize> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(ConfigError::Io(e)),
        };
        let mut n = 0;
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "der") {
                std::fs::remove_file(&path)?;
                n += 1;
            }
        }
        Ok(n)
    }
}

fn plausible_der(der: &[u8]) -> bool {
    der.len() >= 2 && der.len() <= MAX_CERTIFICATE_BYTES && der[0] == 0x30
}

#[cfg(test)]
mod tests {
    use super::*;

    const DER: &[u8] = include_bytes!("../../../testdata/keystores/test-ecp256.crt.der");

    #[test]
    fn opt_in_roundtrip() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let key = CertCacheKey::new("atrust-native", "+43664123456789");
        let off = CertificateCache::new(&paths, &CertificateCacheSettings { enabled: false });
        off.put(&key, DER).unwrap();
        assert_eq!(off.get(&key).unwrap(), None);
        assert!(!paths.certificate_cache_dir().exists());

        let on = CertificateCache::new(&paths, &CertificateCacheSettings { enabled: true });
        assert_eq!(on.get(&key).unwrap(), None);
        on.put(&key, DER).unwrap();
        assert_eq!(on.get(&key).unwrap().as_deref(), Some(DER));
        // kein Benutzername im Dateisystem oder im Debug-Text
        let names: Vec<String> = std::fs::read_dir(on.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(!names[0].contains("664") && names[0].ends_with(".der"));
        assert!(!format!("{key:?}").contains("664"));
        // anderer Account → anderer Eintrag
        assert_eq!(on.get(&CertCacheKey::new("atrust-native", "anders")).unwrap(), None);
        on.remove(&key).unwrap();
        on.remove(&key).unwrap();
        assert_eq!(on.get(&key).unwrap(), None);
        on.put(&key, DER).unwrap();
        assert_eq!(off.clear().unwrap(), 1);
        assert_eq!(on.clear().unwrap(), 0);
    }

    #[test]
    fn rejects_non_der() {
        let root = tempfile::tempdir().unwrap();
        let c = CertificateCache::with_dir(root.path().join("c"), true);
        let key = CertCacheKey::new("keystore", "k");
        assert!(c.put(&key, b"-----BEGIN CERTIFICATE-----").is_err());
        assert!(c.put(&key, &vec![0x30; MAX_CERTIFICATE_BYTES + 1]).is_err());
        std::fs::create_dir_all(c.dir()).unwrap();
        std::fs::write(c.dir().join(format!("{}.der", key.digest())), b"garbage").unwrap();
        assert_eq!(c.get(&key).unwrap(), None);
    }

    #[test]
    fn digest_separates_fields() {
        assert_ne!(CertCacheKey::new("ab", "c").digest(), CertCacheKey::new("a", "bc").digest());
        assert_eq!(CertCacheKey::new("a", "b").digest().len(), 64);
    }
}
