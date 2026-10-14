//! Settings: TOML-Rundlauf, Erhalt unbekannter Keys (oben und in Untertabellen), Schema-Versionen,
//! Laden/Speichern (atomar), JSON-Rundlauf über die Tauri-Bridge.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pdfover_config::{
    BackendChoice, CmsProfileChoice, ConfigError, LoadOutcome, NoteSetting, PasswordStorage, PlaceholderKind,
    ProxyMode, SCHEMA_VERSION, Settings, SignatureProfile, WindowSize,
};
use pdfover_i18n::Lang;

fn full_settings() -> Settings {
    let mut s = Settings::default();
    s.general.ui_language = Some(Lang::En);
    s.general.update_check = false;
    s.general.window_size = Some(WindowSize { width: 1024, height: 768 });
    s.general.last_source_directory = Some(PathBuf::from(r"C:\Users\Max\Downloads"));
    s.signature.profile = SignatureProfile::Amtssignaturblock;
    s.signature.block_language = Some(Lang::De);
    s.signature.note = NoteSetting::Custom("Zeile 1\nZeile „2“ = \"x\" # kein Kommentar".into());
    s.signature.pdfa_force = true;
    s.signature.auto_position = true;
    s.signature.emblem = Some(PathBuf::from("/home/max/logo.png"));
    s.signature.logo_only_size_mm = 12.5;
    s.signature.cms_profile = CmsProfileChoice::PadesBb;
    s.placeholder.search = true;
    s.placeholder.kind = PlaceholderKind::Fields;
    s.backend.default = Some(BackendChoice::LocalBku);
    s.id_austria.username = Some("+436641234567".into());
    s.id_austria.remember = true;
    s.id_austria.fido2_default = true;
    s.keystore.enabled = true;
    s.keystore.file = Some(PathBuf::from("/k/test.p12"));
    s.keystore.declared_type = Some("PKCS12".into());
    s.keystore.alias = Some("1".into());
    s.keystore.password_storage = PasswordStorage::Keychain;
    s.output.folder = Some(PathBuf::from("."));
    s.output.postfix = "_sig".into();
    s.proxy.mode = ProxyMode::Manual;
    s.proxy.host = Some("proxy.example.test".into());
    s.proxy.port = Some(3128);
    s.proxy.user = Some("u".into());
    s.privacy.acknowledged.insert("atrust-native".into(), 2);
    s.certificate_cache.enabled = true;
    s
}

#[test]
fn default_and_full_roundtrip() {
    for s in [Settings::default(), full_settings()] {
        let text = s.to_toml().unwrap();
        assert!(text.starts_with("# PDF-Over NG"));
        assert!(text.contains("schema_version = 1"));
        assert_eq!(Settings::from_toml(&text).unwrap(), s);
    }
    for note in [NoteSetting::Default, NoteSetting::None] {
        let mut s = Settings::default();
        s.signature.note = note.clone();
        assert_eq!(Settings::from_toml(&s.to_toml().unwrap()).unwrap().signature.note, note);
    }
}

const WITH_UNKNOWN: &str = r#"
schema_version = 1
future_scalar = 42
when = 1979-05-27T07:32:00Z

[general]
update_check = false
"odd key" = true
future_general = "x"

[general.window_size]
width = 800
height = 600
depth = 3

[signature]
profile = "BASE_LOGO"
logo_only_size_mm = 30
future_key = ["a", "b"]

[signature.future_table]
x = 1

[privacy.acknowledged]
atrust-native = 1

[future]
name = "Zukunft"

[[future.list]]
a = 1

[[future.list]]
a = 2
"#;

#[test]
fn unknown_keys_survive_in_all_tables() {
    let s = Settings::from_toml(WITH_UNKNOWN).unwrap();
    assert!(!s.general.update_check);
    assert_eq!(s.signature.profile, SignatureProfile::BaseLogo);
    assert_eq!(s.signature.logo_only_size_mm, 30.0, "Ganzzahl als f64 gelesen");
    assert_eq!(s.privacy.acknowledged.get("atrust-native"), Some(&1));
    let keys: Vec<&str> = s.unknown.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "future",
            "future_scalar",
            "general.\"odd key\"",
            "general.future_general",
            "general.window_size.depth",
            "signature.future_key",
            "signature.future_table",
            "when",
        ]
    );
    // Änderung bekannter Werte, dann speichern: unbekannte bleiben an ihrem Ort
    let mut changed = s.clone();
    changed.general.update_check = true;
    changed.signature.profile = SignatureProfile::Invisible;
    let text = changed.to_toml().unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(table["future_scalar"].as_integer(), Some(42));
    assert!(table["when"].as_datetime().is_some());
    assert_eq!(table["general"]["odd key"].as_bool(), Some(true));
    assert_eq!(table["general"]["future_general"].as_str(), Some("x"));
    assert_eq!(table["general"]["window_size"]["depth"].as_integer(), Some(3));
    assert_eq!(table["signature"]["future_key"].as_array().map(Vec::len), Some(2));
    assert_eq!(table["signature"]["future_table"]["x"].as_integer(), Some(1));
    assert_eq!(table["future"]["list"].as_array().map(Vec::len), Some(2));
    assert_eq!(table["signature"]["profile"].as_str(), Some("INVISIBLE"));
    let back = Settings::from_toml(&text).unwrap();
    assert_eq!(back, changed);
}

#[test]
fn unknown_child_of_removed_known_table_is_dropped() {
    let mut s = Settings::from_toml(WITH_UNKNOWN).unwrap();
    s.general.window_size = None;
    let text = s.to_toml().unwrap();
    let back = Settings::from_toml(&text).unwrap();
    assert_eq!(back.general.window_size, None);
    assert!(!back.unknown.contains_key("general.window_size.depth"));
    assert!(back.unknown.contains_key("general.future_general"));
}

#[test]
fn json_bridge_keeps_unknown_keys() {
    let s = Settings::from_toml(WITH_UNKNOWN).unwrap();
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["signature.future_key"], serde_json::json!(["a", "b"]));
    assert_eq!(json["general"]["update_check"], serde_json::json!(false));
    let back: Settings = serde_json::from_value(json).unwrap();
    let text = back.to_toml().unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(table["signature"]["future_key"].as_array().map(Vec::len), Some(2));
    assert_eq!(table["general"]["window_size"]["depth"].as_integer(), Some(3));
    assert_eq!(table["future"]["name"].as_str(), Some("Zukunft"));
}

#[test]
fn schema_versions() {
    let s = Settings::from_toml("[general]\nupdate_check = false\n").unwrap();
    assert_eq!(s.schema_version, SCHEMA_VERSION);
    assert!(!s.general.update_check);
    assert!(matches!(
        Settings::from_toml("schema_version = 2\n"),
        Err(ConfigError::UnsupportedSchema { found: 2, supported: 1 })
    ));
    assert!(matches!(Settings::from_toml("schema_version = 0\n"), Err(ConfigError::Parse(_))));
    assert!(matches!(Settings::from_toml("schema_version = \"1\"\n"), Err(ConfigError::Parse(_))));
    assert!(matches!(Settings::from_toml("[general]\nupdate_check = \"ja\"\n"), Err(ConfigError::Parse(_))));
    assert!(matches!(Settings::from_toml("kaputt ="), Err(ConfigError::Parse(_))));
    assert!(matches!(Settings::from_toml("[signature]\nprofile = \"GIBTS_NICHT\"\n"), Err(ConfigError::Parse(_))));
}

#[test]
fn load_outcomes_and_atomic_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("settings.toml");
    assert!(matches!(Settings::load(&path), LoadOutcome::Missing));
    let s = full_settings();
    s.save_atomic(&path).unwrap();
    let LoadOutcome::Loaded(loaded) = Settings::load(&path) else { panic!() };
    assert_eq!(*loaded, s);
    // überschreiben, keine Temp-Dateien übrig
    let mut s2 = s.clone();
    s2.output.postfix = "_neu".into();
    s2.save_atomic(&path).unwrap();
    let LoadOutcome::Loaded(loaded) = Settings::load(&path) else { panic!() };
    assert_eq!(loaded.output.postfix, "_neu");
    let names: Vec<String> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["settings.toml"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    std::fs::write(&path, "schema_version = 99\n").unwrap();
    let LoadOutcome::Unreadable(e) = Settings::load(&path) else { panic!() };
    assert_eq!(e.message_id(), "ng-config-error-unsupported-schema");
    std::fs::write(&path, b"\xff\xfe").unwrap();
    assert!(matches!(Settings::load(&path), LoadOutcome::Unreadable(ConfigError::Parse(_))));
    // Ordner statt Datei
    assert!(matches!(Settings::load(dir.path()), LoadOutcome::Unreadable(_)));
}

#[test]
fn no_secrets_in_schema() {
    // Strukturell: das Schema kennt kein Passwortfeld.
    let text = full_settings().to_toml().unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    fn walk(t: &toml::Table, path: &str, out: &mut BTreeMap<String, ()>) {
        for (k, v) in t {
            let p = format!("{path}.{k}");
            out.insert(p.clone(), ());
            if let toml::Value::Table(inner) = v {
                walk(inner, &p, out);
            }
        }
    }
    let mut keys = BTreeMap::new();
    walk(&table, "", &mut keys);
    // einziges „pass“-Feld ist die Speicherart, nicht ein Passwort
    assert!(keys.keys().all(|k| !k.contains("pass") || k == ".keystore.password_storage"), "{keys:?}");
    assert!(!text.contains("secret") && !text.contains("token"));
}
