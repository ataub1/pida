//! Migration aus `~/.pdf-over` mit den Fixtures unter `testdata/config/`: Sonderfälle, Kodierungen,
//! Windows-Pfade, Bildmarke samt Cache, `signature_field_name` (B-05), eigene Profile, Secrets,
//! Idempotenz, „Altdatei nie verändern“, Anwenden und Rückgängig.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pdfover_config::migrate::{find_legacy_config, read_signature_field_name, scan_custom_profiles};
use pdfover_config::{
    ApplyOptions, BackendChoice, LegacyConfig, LegacyKey, LoadOutcome, MemorySecretStore, MigrationContext,
    NoteSetting, PasswordStorage, Paths, PlaceholderKind, ProxyMode, SecretId, SecretStore, Settings, SignatureProfile,
    WindowSize, apply_migration, migrate, undo_migration,
};
use pdfover_i18n::Lang;
use pdfover_java_properties::Encoding;

const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/config");

fn fixture(name: &str) -> PathBuf {
    Path::new(TESTDATA).join(name)
}

fn ctx(dir: &Path, check_paths: bool) -> MigrationContext {
    MigrationContext { legacy_dir: dir.to_path_buf(), system_language: Lang::De, check_paths }
}

fn migrate_fixture(name: &str) -> pdfover_config::MigrationResult {
    let legacy = LegacyConfig::load(&fixture(name)).unwrap();
    migrate(&legacy, &ctx(Path::new(TESTDATA), true))
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

/// Inhalt aller Dateien eines Ordners (für „nichts verändert“).
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.clone(), std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

fn skipped(res: &pdfover_config::MigrationResult, key: LegacyKey) -> bool {
    res.report.skipped.iter().any(|(k, _)| *k == key)
}

fn has_message(res: &pdfover_config::MigrationResult, id: &str) -> bool {
    res.report.messages.iter().any(|m| m.id == id)
}

/// Vollständiger Altordner wie nach jahrelanger Nutzung von PDF-Over.
struct LegacyHome {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    emblem: PathBuf,
}

fn legacy_home() -> LegacyHome {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("home").join(".pdf-over");
    copy_dir(&fixture("legacy-home/.pdf-over"), &dir);
    // Original-Logo existiert nicht mehr; der Cache von PDF-Over verweist darauf.
    let emblem = tmp.path().join("Bilder").join("logo-geloescht.png");
    let mut config = std::fs::read_to_string(fixture("pdf-over-generated-de.config")).unwrap();
    config.push_str(&format!("EMBLEM={}\n", java_escape(&emblem.to_string_lossy())));
    std::fs::write(dir.join("PDF-Over.config"), config).unwrap();
    std::fs::write(
        dir.join(".emblem.properties"),
        format!(
            "#{}\nHSH=0123456789abcdef\nIMG={}\n",
            "Wed Oct 14 10:00:00 CEST 2026",
            java_escape(&emblem.to_string_lossy())
        ),
    )
    .unwrap();
    LegacyHome { _tmp: tmp, dir, emblem }
}

/// Java-Properties-Escapes für Werte (`\`, `:`, `=`, `#`, `!`).
fn java_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if matches!(c, '\\' | ':' | '=' | '#' | '!') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[test]
fn full_legacy_home_import() {
    let home = legacy_home();
    let before = snapshot(&home.dir);
    let file = find_legacy_config(&home.dir).expect("PDF-Over.config gefunden");
    let legacy = LegacyConfig::load(&file).unwrap();
    assert_eq!(legacy.encoding, Encoding::Latin1);
    let res = migrate(&legacy, &ctx(&home.dir, true));
    let s = &res.settings;

    assert_eq!(s.backend.default, Some(BackendChoice::IdAustria));
    assert_eq!(s.id_austria.username.as_deref(), Some("+436641234567"));
    assert!(s.id_austria.remember && s.id_austria.fido2_default);
    assert_eq!(s.signature.profile, SignatureProfile::SignaturblockSmall);
    assert_eq!(s.signature.block_language, Some(Lang::De));
    assert_eq!(s.signature.note, NoteSetting::Default, "Standardtext wird erkannt");
    assert!(has_message(&res, "ng-migrate-note-default"));
    assert!(s.placeholder.search);
    assert_eq!(s.placeholder.kind, PlaceholderKind::Marker);
    assert!(s.keystore.enabled);
    assert_eq!(s.keystore.file, Some(PathBuf::from(r"C:\Users\Max Mustermann\Documents\test-rsa2048.p12")));
    assert_eq!(s.keystore.password_storage, PasswordStorage::Memory);
    assert_eq!(s.output.folder, Some(PathBuf::from(r"C:\Users\Max Mustermann\Documents\Signiert")));
    assert_eq!(s.output.postfix, "_signed");
    assert_eq!(s.general.window_size, Some(WindowSize { width: 640, height: 800 }));
    assert_eq!(s.general.ui_language, Some(Lang::De));
    assert!(s.general.update_check);
    assert_eq!(s.proxy.mode, ProxyMode::Manual);
    assert_eq!((s.proxy.host.as_deref(), s.proxy.port), (Some("proxy.example.test"), Some(3128)));
    assert_eq!(s.signature.logo_only_size_mm, 23.0);
    // fehlender Ordner wird nicht übernommen, fehlende Keystore-Datei nur gemeldet
    assert_eq!(s.general.last_source_directory, None);
    assert!(skipped(&res, LegacyKey::LastSourceDirectory));
    assert!(has_message(&res, "ng-migrate-path-missing"));
    // Bildmarke aus dem Cache von PDF-Over
    assert_eq!(s.signature.emblem, Some(home.dir.join(".emblem.png")));
    assert_eq!(res.report.emblem, s.signature.emblem);
    assert!(has_message(&res, "ng-migrate-emblem-from-cache"));
    assert!(!home.emblem.exists());
    // B-05: verworfen, Feldname nicht in Settings/Warnungen
    assert_eq!(res.report.discarded_signature_field_name.as_deref(), Some("SigField-Test"));
    assert!(has_message(&res, "ng-migrate-signature-field-discarded"));
    assert!(!res.settings.to_toml().unwrap().contains("SigField-Test"));
    assert!(!res.report.warnings.iter().any(|w| w.contains("SigField-Test")));
    assert!(!format!("{res:?}").contains("SigField-Test"), "Debug ohne Dokumentinhalt");
    // eigene Profile: geändertes und unbekanntes, nicht die unveränderten
    let names: Vec<String> =
        res.report.custom_profiles.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["MEIN_PROFIL.properties", "SIGNATURBLOCK_SMALL_DE.properties"]);
    // Übernommene Keys
    for k in
        [LegacyKey::Bku, LegacyKey::MobileNumber, LegacyKey::SignatureNote, LegacyKey::ProxyPort, LegacyKey::Emblem]
    {
        assert!(res.report.imported.contains(&k), "{k:?}");
    }
    assert!(res.secrets.is_empty() && res.report.secrets_found.is_empty());

    // idempotent und ohne Schreibzugriff
    let again = migrate(&legacy, &ctx(&home.dir, true));
    assert_eq!(again.settings, res.settings);
    assert_eq!(again.report, res.report);
    assert_eq!(snapshot(&home.dir), before, "Altordner unverändert nach migrate");

    // Anwenden: Bildmarke wird kopiert, Altordner bleibt unverändert, Rückgängig entfernt alles
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::under(root.path());
    let store = MemorySecretStore::default();
    let applied = apply_migration(&res, &paths, &store, &ApplyOptions::default()).unwrap();
    let copied = paths.emblem_dir().join("emblem.png");
    assert_eq!(std::fs::read(&copied).unwrap(), std::fs::read(home.dir.join(".emblem.png")).unwrap());
    let LoadOutcome::Loaded(loaded) = Settings::load(&paths.settings_file()) else { panic!("nicht lesbar") };
    assert_eq!(loaded.signature.emblem, Some(copied.clone()));
    let mut expected = res.settings.clone();
    expected.signature.emblem = Some(copied);
    assert_eq!(*loaded, expected);
    assert_eq!(snapshot(&home.dir), before, "Altordner unverändert nach apply");
    let log = std::fs::read_to_string(&applied.log_file).unwrap();
    assert!(log.contains("übernommen = ") && !log.contains("SigField-Test"));
    undo_migration(&applied, &store).unwrap();
    assert!(!paths.settings_file().exists());
    assert!(!paths.emblem_dir().join("emblem.png").exists());
}

#[test]
fn special_cases() {
    let res = migrate_fixture("special-cases.config");
    let s = &res.settings;
    assert!(!s.general.update_check, "UPDATE_CHECK=FALSE");
    assert!(!s.id_austria.remember, "MOBILE_REMEMBER_PASSWORD=TRUE ist nicht \"true\"");
    assert!(s.placeholder.search);
    assert_eq!(s.placeholder.kind, PlaceholderKind::Fields, "USE_FIELDS gewinnt");
    assert_eq!(s.signature.note, NoteSetting::None, "leerer Hinweis = kein Hinweis");
    assert_eq!(s.note_text(Lang::De), None);
    assert_eq!(s.pdfas_profile_id(Lang::De, false), "AMTSSIGNATURBLOCK_EN_RECOMMENDED");
    assert_eq!(s.signature.profile, SignatureProfile::Amtssignaturblock);
    assert_eq!(s.signature.block_language, Some(Lang::En));
    assert_eq!(s.general.ui_language, Some(Lang::En), "fr → Basis-Bundle Englisch");
    assert_eq!(s.backend.default, None);
    assert!(skipped(&res, LegacyKey::Bku));
    assert_eq!(s.signature.logo_only_size_mm, 42.5);
    assert_eq!(s.general.window_size, Some(WindowSize { width: 1024, height: 780 }));
    assert_eq!(s.proxy.port, None);
    assert!(skipped(&res, LegacyKey::ProxyPort));
    assert_eq!(s.proxy.mode, ProxyMode::System, "ohne PROXY_HOST kein manueller Proxy");
    assert!(skipped(&res, LegacyKey::ProxyUser));
    assert_eq!(s.output.postfix, "");
    assert_eq!(s.id_austria.username.as_deref(), Some("+436641234567"));
    assert!(s.id_austria.fido2_default && s.keystore.enabled && s.signature.auto_position);
    assert!(!s.signature.pdfa_force);
    assert_eq!(s.keystore.password_storage, PasswordStorage::Keychain, "Auto-Migration → disk");
    assert_eq!(s.keystore.alias, None);
    assert_eq!(s.general.last_source_directory, None);
    assert!(skipped(&res, LegacyKey::Theme) && skipped(&res, LegacyKey::Crash));
    assert!(has_message(&res, "ng-migrate-unknown-key"));
    assert!(res.report.warnings.iter().any(|w| w.contains("SIGNATURE_PLACEHOLDER_TRANSPARENCY")));
    // Secrets: Keystore-Passwort (disk), Proxy-Passwort ohne Host nicht zuordenbar
    assert_eq!(res.report.secrets_found, vec![LegacyKey::ProxyPass, LegacyKey::KeystoreStorepass]);
    let ids: Vec<&SecretId> = res.secrets.iter().map(|(i, _)| i).collect();
    assert_eq!(ids, vec![&SecretId::keystore_store(Path::new("/tmp/pdfover-nicht-vorhanden/test.p12"))]);
    assert!(skipped(&res, LegacyKey::ProxyPass));
    // Keine Secret-Werte im Bericht, in Debug oder TOML
    let dbg = format!("{res:?}");
    for secret in ["test-store-pw", "test-proxy-pw"] {
        assert!(!dbg.contains(secret));
        assert!(!res.settings.to_toml().unwrap().contains(secret));
    }
    // Zustimmung: nur dann im Schlüsselbund
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::under(root.path());
    let store = MemorySecretStore::default();
    let applied = apply_migration(&res, &paths, &store, &ApplyOptions::default()).unwrap();
    assert!(store.is_empty() && applied.stored_secrets.is_empty());
    let applied = apply_migration(
        &res,
        &paths,
        &store,
        &ApplyOptions { store_secrets: true, overwrite: true, ..Default::default() },
    )
    .unwrap();
    assert_eq!(applied.stored_secrets.len(), 1);
    let id = SecretId::keystore_store(Path::new("/tmp/pdfover-nicht-vorhanden/test.p12"));
    assert_eq!(store.get(&id).unwrap().as_deref().map(String::as_str), Some("test-store-pw"));
    assert!(!std::fs::read_to_string(paths.settings_file()).unwrap().contains("test-store-pw"));
    // Bericht in beiden Sprachen übersetzt, keine fehlenden IDs
    for m in &res.report.messages {
        for lang in Lang::ALL {
            let t = m.localized(lang);
            assert!(!t.starts_with('!') && !t.contains("{$"), "{}: {t}", m.id);
        }
    }
}

#[test]
fn note_missing_uses_profile_default_and_storage_rules() {
    let res = migrate_fixture("note-missing-amts-en.config");
    let s = &res.settings;
    assert_eq!(s.signature.note, NoteSetting::Default);
    let note = s.note_text(s.effective_block_language(Lang::De)).unwrap();
    assert!(note.contains("official signature"), "{note}");
    assert_eq!(s.pdfas_profile_id(Lang::De, true), "AMTSSIGNATURBLOCK_EN_RECOMMENDED");
    assert_eq!(s.general.ui_language, Some(Lang::En));
    assert!(res.secrets.is_empty(), "memory: Klartext wird ignoriert");
    assert!(skipped(&res, LegacyKey::KeystoreKeypass));
    assert_eq!(s.proxy.port, None);
    assert!(skipped(&res, LegacyKey::ProxyPort));
    assert_eq!(s.proxy.host.as_deref(), Some("10.0.0.1"));
    assert_eq!(s.general.window_size, None);
    assert!(skipped(&res, LegacyKey::MainwindowSize));
    assert_eq!(s.signature.logo_only_size_mm, 23.0);
    assert!(skipped(&res, LegacyKey::LogoOnlySize));
    assert!(!s.placeholder.search, "Suche ohne Art bleibt aus");
    assert!(has_message(&res, "ng-migrate-placeholder-search-off"));
    assert_eq!(s.id_austria.username.as_deref(), Some("max.mustermann"));
}

#[test]
fn encodings() {
    let latin1 = LegacyConfig::load(&fixture("latin1-raw.config")).unwrap();
    assert_eq!(latin1.encoding, Encoding::Latin1);
    let res = migrate(&latin1, &ctx(Path::new(TESTDATA), true));
    assert_eq!(res.settings.signature.note, NoteSetting::Custom("Grüße aus Österreich".into()));
    assert!(!res.settings.general.update_check);

    let utf8 = LegacyConfig::load(&fixture("handedited-utf8.config")).unwrap();
    assert_eq!(utf8.encoding, Encoding::Utf8);
    let res = migrate(&utf8, &ctx(Path::new(TESTDATA), false));
    let s = &res.settings;
    assert_eq!(s.signature.note, NoteSetting::Custom("Geprüft von der „NUR TEST“-Stelle in Österreich – Grüße".into()));
    assert_eq!(s.backend.default, Some(BackendChoice::LocalBku));
    assert_eq!(s.id_austria.username.as_deref(), Some("+436641234567"));
    // nicht escapte Backslashes verschluckt Java (\U → U …) – exakt übernommen
    assert_eq!(s.signature.emblem, Some(PathBuf::from("C:UsersmaxBilderlogo.png")));
    assert_eq!(s.output.folder, Some(PathBuf::from(r"C:\Users\max\Signiert")));
    assert_eq!(s.signature.logo_only_size_mm, 30.5);
    assert_eq!(s.signature.block_language, Some(Lang::De), "de_AT");
    assert_eq!(s.general.ui_language, None, "de-AT ist keine Java-Locale");
    assert_eq!(s.general.window_size, Some(WindowSize { width: 800, height: 900 }));
    assert_eq!(s.output.postfix, "_unterschrieben", "Fortsetzungszeile");
    assert_eq!(s.proxy.port, None, "8080 mit Leerzeichen: parseInt ohne trim");
    assert_eq!(s.proxy.user, None);
    assert!(!s.general.update_check);

    let bom = LegacyConfig::load(&fixture("handedited-utf8-bom-crlf.config")).unwrap();
    assert_eq!(bom.encoding, Encoding::Utf8);
    let s = migrate(&bom, &ctx(Path::new(TESTDATA), true)).settings;
    assert_eq!(s.backend.default, Some(BackendChoice::IdAustria), "BOM entfernt");
    assert_eq!(s.signature.note, NoteSetting::Custom("Ünterschrift geprüft".into()));
    assert!(s.general.update_check, "\"No\" schaltet nicht ab");
    assert_eq!(s.id_austria.username.as_deref(), Some("+436761234567"));
}

#[test]
fn windows_paths_and_crlf() {
    let res = migrate_fixture("windows-crlf.config");
    let s = &res.settings;
    assert_eq!(s.keystore.file, Some(PathBuf::from(r"C:\Users\Max\Schlüssel\test.jks")));
    assert_eq!(s.output.folder, Some(PathBuf::from(r"\\server\freigabe\signiert")));
    assert_eq!(s.signature.note, NoteSetting::Custom("Zeile 1\nZeile 2 : mit Doppelpunkt = und #".into()));
    assert_eq!(s.keystore.password_storage, PasswordStorage::None);
    assert_eq!(s.signature.profile, SignatureProfile::Invisible);
    assert_eq!(s.pdfas_profile_id(Lang::De, true), "INVISIBLE");
    assert_eq!(s.general.window_size, Some(WindowSize { width: 1280, height: 1024 }));
    assert!(has_message(&res, "ng-migrate-keystore-jks"));
    // Pfade existieren hier nicht: Emblem und letzter Ordner fallen weg
    assert_eq!(s.signature.emblem, None);
    assert!(skipped(&res, LegacyKey::Emblem) && skipped(&res, LegacyKey::LastSourceDirectory));
}

#[test]
fn generated_en_keystore() {
    let legacy = LegacyConfig::load(&fixture("pdf-over-generated-en-keystore.config")).unwrap();
    let res = migrate(&legacy, &ctx(Path::new(TESTDATA), false));
    let s = &res.settings;
    assert_eq!(s.backend.default, Some(BackendChoice::Keystore));
    assert_eq!(s.signature.profile, SignatureProfile::BaseLogo);
    assert_eq!(s.signature.emblem, Some(PathBuf::from("/home/max/Bilder/logo #1.png")));
    assert_eq!(s.signature.logo_only_size_mm, 42.0);
    assert_eq!(s.pdfas_profile_id(Lang::De, false), "BASE_LOGO");
    assert!(s.signature.pdfa_force && s.signature.auto_position && !s.general.update_check);
    assert_eq!(s.placeholder.kind, PlaceholderKind::Fields);
    assert_eq!(s.signature.note, NoteSetting::Custom("Signed by NUR TEST – § 1 €".into()));
    assert_eq!(res.secrets.len(), 2);
    assert_eq!(res.report.secrets_found, vec![LegacyKey::KeystoreStorepass, LegacyKey::KeystoreKeypass]);
    assert!(skipped(&res, LegacyKey::Theme));
    // Klartext entfernen: nur als Angebot, Inhalt ohne Secrets
    let cleaned = LegacyConfig::parse(&legacy.to_bytes_without_secrets()).unwrap();
    assert!(cleaned.plaintext_secrets().is_empty());
    assert_eq!(cleaned.raw(LegacyKey::Bku), Some("KS"));
}

#[test]
fn emblem_checks() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("logo.png");
    std::fs::copy(fixture("legacy-home/.pdf-over/.emblem.png"), &png).unwrap();
    let bmp = dir.path().join("logo.bmp");
    std::fs::write(&bmp, b"BM\x00\x00\x00\x00").unwrap();
    let make =
        |p: &Path| LegacyConfig::parse(format!("EMBLEM={}\n", java_escape(&p.to_string_lossy())).as_bytes()).unwrap();
    let res = migrate(&make(&png), &ctx(dir.path(), true));
    assert_eq!(res.settings.signature.emblem, Some(png.clone()));
    assert!(res.report.imported.contains(&LegacyKey::Emblem));
    let res = migrate(&make(&bmp), &ctx(dir.path(), true));
    assert_eq!(res.settings.signature.emblem, None);
    assert!(has_message(&res, "ng-migrate-emblem-unsupported"));
    // ohne Pfadprüfung bleibt der Wert
    let res = migrate(&make(&bmp), &ctx(dir.path(), false));
    assert_eq!(res.settings.signature.emblem, Some(bmp));
}

#[test]
fn cfg_dir_helpers() {
    let home = legacy_home();
    assert_eq!(read_signature_field_name(&home.dir).as_deref(), Some("SigField-Test"));
    assert_eq!(scan_custom_profiles(&home.dir).len(), 2);
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(read_signature_field_name(empty.path()), None);
    assert!(scan_custom_profiles(empty.path()).is_empty());
    assert_eq!(find_legacy_config(empty.path()), None);
}

#[test]
fn every_fixture_roundtrips_through_toml_and_json() {
    for name in [
        "pdf-over-generated-de.config",
        "pdf-over-generated-en-keystore.config",
        "special-cases.config",
        "note-missing-amts-en.config",
        "windows-crlf.config",
        "latin1-raw.config",
        "handedited-utf8.config",
        "handedited-utf8-bom-crlf.config",
    ] {
        let s = migrate_fixture(name).settings;
        let toml = s.to_toml().unwrap();
        assert_eq!(Settings::from_toml(&toml).unwrap(), s, "{name}: TOML");
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s, "{name}: JSON (Tauri-Bridge)");
    }
}

#[test]
fn secrets_store_failure_leaves_nothing_behind() {
    #[derive(Debug)]
    struct Broken;
    impl SecretStore for Broken {
        fn get(&self, _: &SecretId) -> pdfover_config::Result<Option<zeroize::Zeroizing<String>>> {
            Ok(None)
        }
        fn set(&self, _: &SecretId, _: &str) -> pdfover_config::Result<()> {
            Err(pdfover_config::ConfigError::Secret("gesperrt".into()))
        }
        fn delete(&self, _: &SecretId) -> pdfover_config::Result<()> {
            Ok(())
        }
        fn is_persistent(&self) -> bool {
            true
        }
    }
    let res = migrate_fixture("pdf-over-generated-en-keystore.config");
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::under(root.path());
    let err = apply_migration(&res, &paths, &Broken, &ApplyOptions { store_secrets: true, ..Default::default() });
    assert!(err.is_err());
    assert!(!paths.settings_file().exists());
}
