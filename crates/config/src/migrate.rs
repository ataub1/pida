//! Import-Assistent aus `~/.pdf-over` (Plan §7.2; docs/analyse/05 §1.9).

use std::path::{Path, PathBuf};

use pdfover_i18n::Lang;
use zeroize::Zeroizing;

use crate::legacy::{LegacyConfig, LegacyKey};
use crate::secrets::SecretId;
use crate::settings::Settings;

/// Kontext der Migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationContext {
    /// Altordner (`~/.pdf-over`).
    pub legacy_dir: PathBuf,
    /// Systemsprache (für Default-Hinweistexte).
    pub system_language: Lang,
    /// Pfade auf Existenz prüfen (`EMBLEM`, `KEYSTORE_FILE`, `OUTPUT_FOLDER`, `LAST_SOURCE_DIRECTORY`).
    pub check_paths: bool,
}

/// Bericht (was übernommen wurde und was nicht).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MigrationReport {
    /// Übernommene Keys.
    pub imported: Vec<LegacyKey>,
    /// Nicht übernommen, mit Grund.
    pub skipped: Vec<(LegacyKey, String)>,
    /// Warnungen.
    pub warnings: Vec<String>,
    /// Gefundene Klartext-Secrets (werden nur nach Zustimmung gespeichert).
    pub secrets_found: Vec<LegacyKey>,
    /// Eigene Profile in `cfg/` (Prüfsumme ≠ Auslieferungsstand PDF-AS 4.4.x).
    pub custom_profiles: Vec<PathBuf>,
    /// Verworfener `signature_field_name` aus `cfg/advancedconfig.properties` (B-05).
    pub discarded_signature_field_name: Option<String>,
    /// Übernommene Bildmarke (`EMBLEM` bzw. `.emblem.png`).
    pub emblem: Option<PathBuf>,
}

/// Ergebnis (Debug ohne Secret-Werte).
pub struct MigrationResult {
    /// Neue Einstellungen.
    pub settings: Settings,
    /// Bericht.
    pub report: MigrationReport,
    /// Secrets zum Speichern im Schlüsselbund (nur nach Zustimmung).
    pub secrets: Vec<(SecretId, Zeroizing<String>)>,
}

impl std::fmt::Debug for MigrationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<&SecretId> = self.secrets.iter().map(|(id, _)| id).collect();
        f.debug_struct("MigrationResult")
            .field("settings", &self.settings)
            .field("report", &self.report)
            .field("secrets", &ids)
            .finish()
    }
}

/// Bildet die Altkonfiguration auf das neue Schema ab (idempotent, ohne Schreibzugriff).
pub fn migrate(legacy: &LegacyConfig, ctx: &MigrationContext) -> MigrationResult {
    todo!("Paket E")
}

/// Sucht `PDF-Over.config` im Altordner.
pub fn find_legacy_config(legacy_dir: &Path) -> Option<PathBuf> {
    todo!("Paket E")
}

/// Liest `signature_field_name` aus `cfg/advancedconfig.properties` (wird verworfen und protokolliert).
pub fn read_signature_field_name(legacy_dir: &Path) -> Option<String> {
    todo!("Paket E")
}
