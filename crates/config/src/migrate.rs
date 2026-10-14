//! Import-Assistent aus `~/.pdf-over` (Plan §7.2; docs/analyse/05 §1.9).
//!
//! 1. [`find_legacy_config`] + [`LegacyConfig::load`]: Java-Properties, Kodierungsheuristik.
//! 2. [`migrate`]: bildet alle 31 Keys mit exakter Lese-Semantik auf [`Settings`] ab – **ohne**
//!    Schreibzugriff, idempotent. Ergebnis: neue Einstellungen, Bericht, gefundene Klartext-Secrets.
//! 3. [`apply_migration`]: schreibt `settings.toml` (atomar), kopiert die Bildmarke in den Datenordner,
//!    übernimmt Secrets **nur nach Zustimmung** in den Schlüsselbund und schreibt ein Protokoll.
//!    Es entstehen nur neue Dateien; [`undo_migration`] macht den Import rückgängig.
//!
//! Die Altdatei `PDF-Over.config` und `~/.pdf-over/cfg/` werden **nie** verändert. Der von PDF-Over
//! dauerhaft gespeicherte `signature_field_name` (B-05) wird verworfen und protokolliert (ohne den
//! Feldnamen selbst – er stammt aus einem Dokument).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pdfover_i18n::{ArgValue, Catalog, Lang};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::legacy::{LegacyConfig, LegacyKey, LegacyNote, LegacyValues};
use crate::paths::{LEGACY_CONFIG_FILE, Paths};
use crate::secrets::{SecretId, SecretStore, store_all};
use crate::settings::{
    NoteSetting, PasswordStorage, PlaceholderKind, PlaceholderSettings, ProxyMode, ProxySettings, Settings, WindowSize,
    write_atomic,
};
use crate::{ConfigError, Result};

/// Kontext der Migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationContext {
    /// Altordner (`~/.pdf-over`).
    pub legacy_dir: PathBuf,
    /// Systemsprache (für Default-Hinweistexte und die Texte im Bericht).
    pub system_language: Lang,
    /// Pfade auf Existenz prüfen (`EMBLEM`, `KEYSTORE_FILE`, `OUTPUT_FOLDER`, `LAST_SOURCE_DIRECTORY`).
    pub check_paths: bool,
}

/// Eine Meldung des Berichts (übersetzbar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportMessage {
    /// Meldungs-ID (`ng-migrate-*`, `ng-config-legacy-*`).
    pub id: &'static str,
    /// Betroffener Key.
    pub key: Option<LegacyKey>,
    /// Argumente (`key` wird automatisch ergänzt).
    pub args: Vec<(&'static str, String)>,
}

impl ReportMessage {
    fn new(id: &'static str, key: Option<LegacyKey>) -> Self {
        Self { id, key, args: Vec::new() }
    }

    fn arg(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    /// Übersetzter Text.
    pub fn localized(&self, lang: Lang) -> String {
        let mut args: Vec<(&str, ArgValue)> = Vec::new();
        if let Some(k) = self.key {
            args.push(("key", ArgValue::from(k.name())));
        }
        args.extend(self.args.iter().map(|(k, v)| (*k, ArgValue::from(v))));
        Catalog::global().format(lang, self.id, &args).into_owned()
    }
}

/// Bericht (was übernommen wurde und was nicht). `Debug` zeigt vom verworfenen Feldnamen nur die
/// Länge (er stammt aus einem Dokument, ADR 0001 Regel 9).
#[derive(Clone, PartialEq, Default)]
pub struct MigrationReport {
    /// Übernommene Keys.
    pub imported: Vec<LegacyKey>,
    /// Nicht übernommen, mit Grund (Text in der Systemsprache).
    pub skipped: Vec<(LegacyKey, String)>,
    /// Warnungen (Text in der Systemsprache).
    pub warnings: Vec<String>,
    /// Gefundene Klartext-Secrets (werden nur nach Zustimmung gespeichert).
    pub secrets_found: Vec<LegacyKey>,
    /// Eigene Profile in `cfg/` (Prüfsumme ≠ Auslieferungsstand PDF-AS 4.4.x).
    pub custom_profiles: Vec<PathBuf>,
    /// Verworfener `signature_field_name` aus `cfg/advancedconfig.properties` (B-05).
    pub discarded_signature_field_name: Option<String>,
    /// Übernommene Bildmarke (`EMBLEM` bzw. `.emblem.png`).
    pub emblem: Option<PathBuf>,
    /// Alle Meldungen strukturiert (für die UI in beliebiger Sprache, [`ReportMessage::localized`]).
    pub messages: Vec<ReportMessage>,
}

impl std::fmt::Debug for MigrationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field_name =
            self.discarded_signature_field_name.as_ref().map(|n| format!("<{} Zeichen>", n.chars().count()));
        f.debug_struct("MigrationReport")
            .field("imported", &self.imported)
            .field("skipped", &self.skipped)
            .field("warnings", &self.warnings)
            .field("secrets_found", &self.secrets_found)
            .field("custom_profiles", &self.custom_profiles)
            .field("discarded_signature_field_name", &field_name)
            .field("emblem", &self.emblem)
            .field("messages", &self.messages)
            .finish()
    }
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

/// Sammelt Meldungen und Key-Status.
struct Reporter {
    lang: Lang,
    report: MigrationReport,
    skipped_keys: BTreeSet<LegacyKey>,
}

impl Reporter {
    fn skip(&mut self, key: LegacyKey, msg: ReportMessage) {
        let text = msg.localized(self.lang);
        self.report.skipped.push((key, text));
        self.report.messages.push(msg);
        self.skipped_keys.insert(key);
    }

    fn warn(&mut self, msg: ReportMessage) {
        self.report.warnings.push(msg.localized(self.lang));
        self.report.messages.push(msg);
    }
}

/// Bildet die Altkonfiguration auf das neue Schema ab (idempotent, ohne Schreibzugriff).
pub fn migrate(legacy: &LegacyConfig, ctx: &MigrationContext) -> MigrationResult {
    let (v, warnings) = legacy.values();
    let mut r =
        Reporter { lang: ctx.system_language, report: MigrationReport::default(), skipped_keys: BTreeSet::new() };
    for w in &warnings {
        let mut msg = ReportMessage::new(w.code, Some(w.key)).arg("value", w.raw.clone());
        msg.args.extend(w.args.iter().cloned());
        r.skip(w.key, msg);
    }
    for key in legacy.unknown_keys() {
        r.warn(ReportMessage::new("ng-migrate-unknown-key", None).arg("key", key));
    }
    for key in [LegacyKey::Theme, LegacyKey::Crash] {
        if legacy.raw(key).is_some() {
            r.skip(key, ReportMessage::new("ng-migrate-key-ignored", Some(key)));
        }
    }

    let mut s = Settings::default();
    let exists = |p: &Path| !ctx.check_paths || p.exists();

    // Allgemeines
    s.general.ui_language = v.locale;
    s.general.update_check = v.update_check;
    s.general.window_size = v.mainwindow_size.map(|(width, height)| WindowSize { width, height });
    s.general.last_source_directory = match &v.last_source_directory {
        Some(p) if !exists(p) => {
            r.skip(
                LegacyKey::LastSourceDirectory,
                path_msg("ng-migrate-path-dropped", LegacyKey::LastSourceDirectory, p),
            );
            None
        }
        other => other.clone(),
    };

    // Signatur
    s.signature.profile = v.profile;
    s.signature.block_language = v.sign_locale;
    s.signature.pdfa_force = v.pdfa_compat;
    s.signature.auto_position = v.auto_position;
    s.signature.logo_only_size_mm = v.logo_only_size_mm;
    s.signature.note = match &v.signature_note {
        LegacyNote::Missing => NoteSetting::Default,
        LegacyNote::Empty => NoteSetting::None,
        LegacyNote::Text(t) => {
            let lang = v.sign_locale.unwrap_or(ctx.system_language);
            if v.profile.default_note(lang).as_deref() == Some(t.as_str()) {
                r.report.messages.push(ReportMessage::new("ng-migrate-note-default", Some(LegacyKey::SignatureNote)));
                NoteSetting::Default
            } else {
                NoteSetting::Custom(t.clone())
            }
        }
    };
    s.signature.emblem = migrate_emblem(&v, ctx, &mut r);
    r.report.emblem = s.signature.emblem.clone();

    // Platzhalter: Suche ohne gewählte Art findet in PDF-Over nichts → bleibt aus.
    let kind = if v.use_fields { PlaceholderKind::Fields } else { PlaceholderKind::Marker };
    let search = v.use_placeholder_search && (v.use_fields || v.use_marker);
    if v.use_placeholder_search && !search {
        r.warn(ReportMessage::new("ng-migrate-placeholder-search-off", Some(LegacyKey::UsePlaceholderSearch)));
    }
    s.placeholder = PlaceholderSettings { search, kind };

    // Backend, ID Austria
    s.backend.default = v.bku;
    s.id_austria.username = v.mobile_number.clone();
    s.id_austria.remember = v.mobile_remember_password;
    s.id_austria.fido2_default = v.fido2_default;

    // Keystore
    s.keystore.enabled = v.keystore_enabled;
    s.keystore.declared_type = v.keystore_type.clone();
    s.keystore.alias = v.keystore_alias.clone();
    s.keystore.password_storage = v.keystore_pass_store_type;
    s.keystore.file = v.keystore_file.clone();
    if let Some(f) = &v.keystore_file
        && !exists(f)
    {
        r.warn(path_msg("ng-migrate-path-missing", LegacyKey::KeystoreFile, f));
    }
    let jks_type =
        v.keystore_type.as_deref().is_some_and(|t| t.eq_ignore_ascii_case("JKS") || t.eq_ignore_ascii_case("JCEKS"));
    let jks_ext = v.keystore_file.as_deref().and_then(Path::extension).is_some_and(|e| {
        let e = e.to_string_lossy().to_ascii_lowercase();
        e == "jks" || e == "ks" || e == "jceks"
    });
    if jks_type || jks_ext {
        let shown = v.keystore_type.clone().unwrap_or_else(|| "JKS".into());
        r.warn(ReportMessage::new("ng-migrate-keystore-jks", Some(LegacyKey::KeystoreType)).arg("value", shown));
    }

    // Ausgabe
    s.output.folder = v.output_folder.clone();
    if let Some(f) = &v.output_folder
        && f.is_absolute()
        && !exists(f)
    {
        r.warn(path_msg("ng-migrate-path-missing", LegacyKey::OutputFolder, f));
    }
    s.output.postfix = v.save_file_postfix.clone();

    // Proxy (PDF-Over ohne Host: kein eigener Proxy → NG-Standard „System“)
    match &v.proxy_host {
        Some(host) => {
            s.proxy = ProxySettings {
                mode: ProxyMode::Manual,
                host: Some(host.clone()),
                port: v.proxy_port,
                user: v.proxy_user.clone(),
            };
        }
        None => {
            for key in [LegacyKey::ProxyPort, LegacyKey::ProxyUser] {
                if legacy.raw(key).is_some_and(|x| !crate::java::trim(x).is_empty()) && !r.skipped_keys.contains(&key) {
                    r.skip(key, ReportMessage::new("ng-migrate-proxy-without-host", Some(key)));
                }
            }
        }
    }

    // Secrets
    let secrets = collect_secrets(legacy, &v, &mut r);

    // cfg/: signature_field_name (B-05) und eigene Profile
    if let Some(name) = read_signature_field_name(&ctx.legacy_dir) {
        tracing::info!(
            len = name.chars().count(),
            "signature_field_name aus advancedconfig.properties verworfen (B-05)"
        );
        r.warn(ReportMessage::new("ng-migrate-signature-field-discarded", None));
        r.report.discarded_signature_field_name = Some(name);
    }
    r.report.custom_profiles = scan_custom_profiles(&ctx.legacy_dir);
    for p in &r.report.custom_profiles.clone() {
        let file = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        r.warn(ReportMessage::new("ng-migrate-custom-profile", None).arg("file", file));
    }

    // Übernommene Keys: vorhanden, migriert, kein Secret, nicht übersprungen.
    r.report.imported = LegacyKey::ALL
        .into_iter()
        .filter(|k| k.is_migrated() && !k.is_secret() && legacy.raw(*k).is_some() && !r.skipped_keys.contains(k))
        .collect();

    MigrationResult { settings: s, report: r.report, secrets }
}

fn path_msg(id: &'static str, key: LegacyKey, path: &Path) -> ReportMessage {
    ReportMessage::new(id, Some(key)).arg("path", path.display().to_string())
}

/// Bildformate, die `pdfover-sigblock` verarbeitet (PNG, JPEG, GIF) – geprüft an der Signatur.
fn supported_image(path: &Path) -> bool {
    let mut head = [0u8; 8];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let n = std::io::Read::read(&mut f, &mut head).unwrap_or(0);
    let head = &head[..n];
    head.starts_with(b"\x89PNG\r\n\x1a\n") || head.starts_with(&[0xFF, 0xD8, 0xFF]) || head.starts_with(b"GIF8")
}

/// `EMBLEM`: Datei muss existieren und ein unterstütztes Bild sein; fehlt das Original, wird die von
/// PDF-Over zwischengespeicherte `.emblem.png` übernommen, wenn `.emblem.properties` (`IMG`) auf
/// genau diesen Pfad verweist.
fn migrate_emblem(v: &LegacyValues, ctx: &MigrationContext, r: &mut Reporter) -> Option<PathBuf> {
    let emblem = v.emblem.as_ref()?;
    if !ctx.check_paths {
        return Some(emblem.clone());
    }
    if emblem.is_file() {
        if supported_image(emblem) {
            return Some(emblem.clone());
        }
        r.skip(LegacyKey::Emblem, path_msg("ng-migrate-emblem-unsupported", LegacyKey::Emblem, emblem));
        return None;
    }
    let cached = ctx.legacy_dir.join(".emblem.png");
    let props = ctx.legacy_dir.join(".emblem.properties");
    let refers_to_emblem = std::fs::read(&props)
        .ok()
        .and_then(|b| pdfover_java_properties::Properties::load_auto(&b).ok())
        .is_some_and(|(p, _)| p.get("IMG").map(Path::new) == Some(emblem.as_path()));
    if refers_to_emblem && cached.is_file() && supported_image(&cached) {
        r.warn(path_msg("ng-migrate-emblem-from-cache", LegacyKey::Emblem, emblem));
        return Some(cached);
    }
    r.skip(LegacyKey::Emblem, path_msg("ng-migrate-path-dropped", LegacyKey::Emblem, emblem));
    None
}

fn collect_secrets(legacy: &LegacyConfig, v: &LegacyValues, r: &mut Reporter) -> Vec<(SecretId, Zeroizing<String>)> {
    let mut out = Vec::new();
    r.report.secrets_found = legacy.plaintext_secrets();
    let keystore_pw =
        [(LegacyKey::KeystoreStorepass, &v.keystore_storepass), (LegacyKey::KeystoreKeypass, &v.keystore_keypass)];
    for (key, value) in keystore_pw {
        let Some(value) = value.as_deref().filter(|p| !p.is_empty()) else { continue };
        if v.keystore_pass_store_type != PasswordStorage::Keychain {
            r.skip(key, ReportMessage::new("ng-migrate-secret-ignored", Some(key)));
            continue;
        }
        let Some(file) = &v.keystore_file else {
            r.skip(key, ReportMessage::new("ng-migrate-secret-no-target", Some(key)));
            continue;
        };
        let id = if key == LegacyKey::KeystoreStorepass {
            SecretId::keystore_store(file)
        } else {
            SecretId::keystore_key(file, v.keystore_alias.as_deref().unwrap_or(""))
        };
        out.push((id, Zeroizing::new(value.to_owned())));
    }
    if let Some(pass) = &v.proxy_pass {
        match (&v.proxy_host, &v.proxy_user) {
            (Some(h), Some(u)) => out.push((SecretId::proxy(h, u), Zeroizing::new(pass.clone()))),
            _ => r.skip(
                LegacyKey::ProxyPass,
                ReportMessage::new("ng-migrate-secret-no-target", Some(LegacyKey::ProxyPass)),
            ),
        }
    }
    out
}

/// Sucht `PDF-Over.config` im Altordner.
pub fn find_legacy_config(legacy_dir: &Path) -> Option<PathBuf> {
    let f = legacy_dir.join(LEGACY_CONFIG_FILE);
    f.is_file().then_some(f)
}

/// Liest `signature_field_name` aus `cfg/advancedconfig.properties` (wird verworfen und protokolliert).
pub fn read_signature_field_name(legacy_dir: &Path) -> Option<String> {
    let bytes = std::fs::read(legacy_dir.join("cfg").join("advancedconfig.properties")).ok()?;
    let (props, _) = pdfover_java_properties::Properties::load_auto(&bytes).ok()?;
    props.get("signature_field_name").filter(|v| !crate::java::trim(v).is_empty()).map(str::to_owned)
}

/// SHA-256 der mit PDF-Over 4.4.x ausgelieferten PDF-AS-Konfiguration (`PDFASConfig.zip`, ab
/// `MIN_PDF_AS_CONFIG_VERSION` 4.4.6): `cfg/config.properties` und `cfg/profiles/*.properties`.
pub const SHIPPED_PDFAS_FILES: &[(&str, &str)] = &[
    ("config.properties", "f75960370ebe00fe67d75f8b1604002287eade5b6ee13db4989e708e2d02f150"),
    ("profiles/AMTSSIGNATURBLOCK_BASE.properties", "4db57f32f62604711f43af327e71734719e07d0ecb476ee0591830394c1a7589"),
    (
        "profiles/AMTSSIGNATURBLOCK_BASE_EN.properties",
        "ada7cde9b24fc61b06b426aea09275cd9bce7b150a308ccfb804b655f7056d92",
    ),
    (
        "profiles/AMTSSIGNATURBLOCK_DE_RECOMMENDED.properties",
        "0e8e5735f95e22ad17fc924972a5a816c818d281a73b9c4ade05e2df75a89382",
    ),
    (
        "profiles/AMTSSIGNATURBLOCK_EN_RECOMMENDED.properties",
        "fedaf94d0a394a98afef1800686939a03c0e73544c7c28363de4dcc942805d47",
    ),
    ("profiles/BASE_LOGO.properties", "24a7b98d0a5c46c4d6d1ef5f56a73437a0547d127c8f082eac1c2438bacee9c4"),
    ("profiles/BASE_NORMAL.properties", "47a3bc851741f6ed4b2d5dc95599c4172d478137ec758afa916ce57664200c8f"),
    ("profiles/BASE_NORMAL_EN.properties", "4c6239b3ab296000f9a3e71e27ef2a02f8f9bc57860cc7c829914a8b7bd28a6d"),
    ("profiles/BASE_SMALL.properties", "02c6ac3359e94332381e755ad913b1068616e2e751bd594b2bc10f029c5c4bd9"),
    ("profiles/BASE_SMALL_EN.properties", "9222f532efdd67136a9be4fb2f860d07ed87e33bd8b24b9eee3e2a8fc8ac9862"),
    ("profiles/BASE_SMALL_EN_NOTE.properties", "06a31af22f7f46d4bdd2d655d853ff11eaa45b93dc27a97ab87908c2f776e2b9"),
    ("profiles/BASE_SMALL_NOTE.properties", "1c1aee1d466f140b4f80a94a2fe3fb820d22be31dca5ef18be3c021e0c4073ba"),
    ("profiles/INVISIBLE.properties", "b9ea2733e8c9b27801051ce42ab5867e9697bd6a9e0013695a37dbeaa7607a0f"),
    ("profiles/SIGNATURBLOCK_SMALL_DE.properties", "7995f56aa1c124dd1fc01d8d0c0117fc5ff8fec56885b2e33353d316db36bcce"),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE_NOTE.properties",
        "2e8e22453e59cf27ce6728d59e05b2bf34cf9f3f1f8dd71d5b195bcd16188384",
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE_NOTE_PDFA.properties",
        "f1b75a508dc031f51286a5e36ec7d3668b3eac22ee9f2ffc3a993dc224a6b167",
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE_PDFA.properties",
        "0672274d2b3a9bff4098fb66f5ba82ca36f1da35b1ec7a8fccb951c773fe0740",
    ),
    ("profiles/SIGNATURBLOCK_SMALL_EN.properties", "31b304277c7d42e370a831544be06ace1a6416f7918432e06ca9fa3eefa72516"),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN_NOTE.properties",
        "2bd4f74e08215bebb2cb3ff3d895ec21da16227ba58ae0df4cb6408ca3d9c48f",
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN_NOTE_PDFA.properties",
        "7553257fae3c91d93df88dd4d941920e07b4f73e1636793c624a7e8938674695",
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN_PDFA.properties",
        "398dcdc0befd1860d8a22946e8b5918fce0440997dfd9da3c0cc76b0801b6e2c",
    ),
];

fn sha256_hex(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(hex::encode(Sha256::digest(&bytes)))
}

/// Eigene bzw. veränderte PDF-AS-Profile in `<legacy_dir>/cfg` (sortiert): `config.properties` und
/// `profiles/*.properties`, deren Name oder Prüfsumme nicht dem Auslieferungsstand entspricht.
pub fn scan_custom_profiles(legacy_dir: &Path) -> Vec<PathBuf> {
    let cfg = legacy_dir.join("cfg");
    let mut candidates = Vec::new();
    if cfg.join("config.properties").is_file() {
        candidates.push(("config.properties".to_owned(), cfg.join("config.properties")));
    }
    if let Ok(entries) = std::fs::read_dir(cfg.join("profiles")) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().is_some_and(|x| x == "properties") {
                let name = format!("profiles/{}", e.file_name().to_string_lossy());
                candidates.push((name, p));
            }
        }
    }
    let mut out: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|(name, path)| {
            let shipped = SHIPPED_PDFAS_FILES.iter().find(|(n, _)| n == name).map(|(_, h)| *h);
            shipped.is_none() || sha256_hex(path).as_deref() != shipped
        })
        .map(|(_, p)| p)
        .collect();
    out.sort();
    out
}

/// Optionen für [`apply_migration`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOptions {
    /// Klartext-Secrets in den [`SecretStore`] übernehmen (Zustimmung der Nutzerin).
    pub store_secrets: bool,
    /// Bildmarke in den Datenordner kopieren (sonst bleibt der Verweis auf den Altpfad).
    pub copy_emblem: bool,
    /// Eine vorhandene `settings.toml` ersetzen (sie wird vorher gesichert); sonst Fehler.
    pub overwrite: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self { store_secrets: false, copy_emblem: true, overwrite: false }
    }
}

/// Ergebnis von [`apply_migration`] (für „Rückgängig“).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    /// Geschriebene `settings.toml`.
    pub settings_file: PathBuf,
    /// Neu angelegte Dateien (Einstellungen, Bildmarke, Protokoll).
    pub created: Vec<PathBuf>,
    /// Sicherung einer vorher vorhandenen `settings.toml`.
    pub backup: Option<PathBuf>,
    /// Im Schlüsselbund gespeicherte Secrets.
    pub stored_secrets: Vec<SecretId>,
    /// Protokolldatei.
    pub log_file: PathBuf,
}

/// Name des Import-Protokolls im Konfigurationsordner.
pub const MIGRATION_LOG_FILE: &str = "import-pdf-over.log";

/// Führt den Import aus (einmalig, idempotent wiederholbar mit `overwrite`). Die Altdateien bleiben
/// unverändert.
pub fn apply_migration(
    result: &MigrationResult,
    paths: &Paths,
    secrets: &dyn SecretStore,
    opts: &ApplyOptions,
) -> Result<AppliedMigration> {
    std::fs::create_dir_all(&paths.config_dir)?;
    let settings_file = paths.settings_file();
    let mut created = Vec::new();
    let mut backup = None;
    if settings_file.exists() {
        if !opts.overwrite {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{} existiert bereits", settings_file.display()),
            )));
        }
        let b = settings_file.with_file_name("settings.toml.before-import");
        std::fs::copy(&settings_file, &b)?;
        backup = Some(b);
    }

    let mut settings = result.settings.clone();
    if opts.copy_emblem
        && let Some(src) = settings.signature.emblem.clone()
        && src.is_file()
    {
        let ext = src.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "png".into());
        let target = paths.emblem_dir().join(format!("emblem.{ext}"));
        std::fs::create_dir_all(paths.emblem_dir())?;
        std::fs::copy(&src, &target)?;
        created.push(target.clone());
        settings.signature.emblem = Some(target);
    }

    let stored_secrets = if opts.store_secrets && !result.secrets.is_empty() {
        match store_all(secrets, &result.secrets) {
            Ok(ids) => ids,
            Err(e) => {
                cleanup(&created);
                return Err(e);
            }
        }
    } else {
        Vec::new()
    };

    if let Err(e) = settings.save_atomic(&settings_file) {
        cleanup(&created);
        for id in &stored_secrets {
            let _ = secrets.delete(id);
        }
        return Err(e);
    }
    created.push(settings_file.clone());

    let log_file = paths.config_dir.join(MIGRATION_LOG_FILE);
    write_atomic(&log_file, protocol(result, &stored_secrets, opts).as_bytes())?;
    created.push(log_file.clone());
    tracing::info!(
        imported = result.report.imported.len(),
        skipped = result.report.skipped.len(),
        secrets = stored_secrets.len(),
        "PDF-Over-Einstellungen importiert"
    );
    Ok(AppliedMigration { settings_file, created, backup, stored_secrets, log_file })
}

fn cleanup(files: &[PathBuf]) {
    for f in files {
        let _ = std::fs::remove_file(f);
    }
}

/// Macht einen Import rückgängig: entfernt die angelegten Dateien und Secrets und stellt eine vorher
/// vorhandene `settings.toml` wieder her.
pub fn undo_migration(applied: &AppliedMigration, secrets: &dyn SecretStore) -> Result<()> {
    for f in &applied.created {
        match std::fs::remove_file(f) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(ConfigError::Io(e)),
        }
    }
    if let Some(b) = &applied.backup {
        std::fs::rename(b, &applied.settings_file)?;
    }
    for id in &applied.stored_secrets {
        secrets.delete(id)?;
    }
    Ok(())
}

/// Protokoll ohne Secrets und ohne Feldnamen (nur Key-Namen, Gründe, Pfade).
fn protocol(result: &MigrationResult, stored: &[SecretId], opts: &ApplyOptions) -> String {
    use std::fmt::Write as _;
    let r = &result.report;
    let mut out = String::from("# PDF-Over NG – Import aus ~/.pdf-over\n");
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = writeln!(out, "zeit_unix = {secs}");
    let imported: Vec<&str> = r.imported.iter().map(|k| k.name()).collect();
    let _ = writeln!(out, "übernommen = {}", imported.join(", "));
    for (k, reason) in &r.skipped {
        let _ = writeln!(out, "nicht übernommen: {} – {}", k.name(), reason);
    }
    for w in &r.warnings {
        let _ = writeln!(out, "warnung: {w}");
    }
    let found: Vec<&str> = r.secrets_found.iter().map(|k| k.name()).collect();
    let _ = writeln!(out, "klartext-passwörter gefunden = {}", found.join(", "));
    let _ = writeln!(
        out,
        "passwörter in den schlüsselbund übernommen = {} (zustimmung: {})",
        stored.len(),
        opts.store_secrets
    );
    for p in &r.custom_profiles {
        let _ = writeln!(out, "eigenes profil: {}", p.display());
    }
    if r.discarded_signature_field_name.is_some() {
        out.push_str("signature_field_name verworfen (B-05)\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;

    fn ctx(dir: &Path) -> MigrationContext {
        MigrationContext { legacy_dir: dir.to_path_buf(), system_language: Lang::De, check_paths: true }
    }

    #[test]
    fn note_default_text_is_recognized() {
        let de = crate::SignatureProfile::SignaturblockSmall.default_note(Lang::De).unwrap();
        let mut props = pdfover_java_properties::Properties::new();
        props.insert("SIGNATURE_NOTE", de.as_str());
        props.insert("SIGNLOCALE", "de");
        let legacy = LegacyConfig::parse(&props.store_latin1(&Default::default())).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let res = migrate(&legacy, &ctx(dir.path()));
        assert_eq!(res.settings.signature.note, NoteSetting::Default);
        // Deutscher Standardtext bei englischem Block bleibt eigener Text (wie PDF-Over).
        let mut props2 = props.clone();
        props2.insert("SIGNLOCALE", "en");
        let legacy2 = LegacyConfig::parse(&props2.store_latin1(&Default::default())).unwrap();
        assert_eq!(migrate(&legacy2, &ctx(dir.path())).settings.signature.note, NoteSetting::Custom(de));
    }

    #[test]
    fn secrets_need_disk_and_target() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = LegacyConfig::parse(
            b"KEYSTORE_FILE=/x/k.p12\nKEYSTORE_ALIAS=a\nKEYSTORE_STOREPASS=test-s\nKEYSTORE_KEYPASS=test-k\nPROXY_PASS=test-p\nPROXY_USER=u\n",
        )
        .unwrap();
        let res = migrate(&legacy, &MigrationContext { check_paths: false, ..ctx(dir.path()) });
        assert_eq!(res.report.secrets_found.len(), 3);
        // Auto-Migration → disk → beide Keystore-Passwörter; Proxy ohne Host nicht zuordenbar.
        let ids: Vec<&SecretId> = res.secrets.iter().map(|(i, _)| i).collect();
        assert_eq!(
            ids,
            vec![&SecretId::keystore_store(Path::new("/x/k.p12")), &SecretId::keystore_key(Path::new("/x/k.p12"), "a")]
        );
        assert!(res.report.skipped.iter().any(|(k, _)| *k == LegacyKey::ProxyPass));
        assert!(!format!("{res:?}").contains("test-s"));

        let memory = LegacyConfig::parse(
            b"KEYSTORE_PASS_STORE_TYPE=memory\nKEYSTORE_FILE=/x/k.p12\nKEYSTORE_STOREPASS=test-s\n",
        )
        .unwrap();
        let res = migrate(&memory, &MigrationContext { check_paths: false, ..ctx(dir.path()) });
        assert!(res.secrets.is_empty());
        assert_eq!(res.report.skipped[0].0, LegacyKey::KeystoreStorepass);
    }

    #[test]
    fn apply_and_undo() {
        let legacy_dir = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let legacy = LegacyConfig::parse(
            b"BKU=KS\nKEYSTORE_FILE=/x/k.p12\nKEYSTORE_PASS_STORE_TYPE=disk\nKEYSTORE_STOREPASS=test-s\n",
        )
        .unwrap();
        let res = migrate(&legacy, &MigrationContext { check_paths: false, ..ctx(legacy_dir.path()) });
        let store = MemorySecretStore::default();
        let applied =
            apply_migration(&res, &paths, &store, &ApplyOptions { store_secrets: true, ..Default::default() }).unwrap();
        assert_eq!(store.len(), 1);
        let log = std::fs::read_to_string(&applied.log_file).unwrap();
        assert!(!log.contains("test-s") && log.contains("BKU"));
        assert!(matches!(Settings::load(&paths.settings_file()), crate::LoadOutcome::Loaded(_)));
        // zweiter Lauf ohne overwrite: Fehler; mit overwrite: Sicherung
        assert!(apply_migration(&res, &paths, &store, &ApplyOptions::default()).is_err());
        let again =
            apply_migration(&res, &paths, &store, &ApplyOptions { overwrite: true, ..Default::default() }).unwrap();
        assert!(again.backup.is_some());
        undo_migration(&again, &store).unwrap();
        assert!(paths.settings_file().is_file(), "Sicherung wiederhergestellt");
        undo_migration(&applied, &store).unwrap();
        assert!(!paths.settings_file().exists());
        assert!(store.is_empty());
    }
}
