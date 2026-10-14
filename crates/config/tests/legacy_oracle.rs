//! Lese-Semantik von `PDF-Over.config` gegen das Java-Orakel (`testdata/config/oracle/LegacyOracle.java`,
//! Nachbau von `ConfigurationManager.loadFromDisk` mit den echten JDK-Funktionen). Ohne `java` im
//! `PATH` werden die Orakel-Tests übersprungen; die Erwartungen ohne Orakel stehen in `migration.rs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use pdfover_config::{BackendChoice, LegacyConfig, LegacyNote, PasswordStorage};
use pdfover_i18n::Lang;
use pdfover_java_properties::{Encoding, Properties, StoreOptions};

const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/config");

/// Fixtures und Lesemodus des Orakels (UTF-8 = so, wie die Datei gemeint war).
const FIXTURES: &[(&str, bool)] = &[
    ("pdf-over-generated-de.config", false),
    ("pdf-over-generated-en-keystore.config", false),
    ("special-cases.config", false),
    ("note-missing-amts-en.config", false),
    ("windows-crlf.config", false),
    ("latin1-raw.config", false),
    ("handedited-utf8.config", true),
    ("handedited-utf8-bom-crlf.config", true),
    ("edge-values.config", false),
];

fn fixture(name: &str) -> PathBuf {
    Path::new(TESTDATA).join(name)
}

fn java_available() -> bool {
    Command::new("java").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('u') => {
                let hex: String = it.by_ref().take(4).collect();
                let unit = u16::from_str_radix(&hex, 16).unwrap();
                out.push_str(&String::from_utf16_lossy(&[unit]));
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

#[derive(Default, Debug)]
struct OracleResult {
    raw: Properties,
    val: BTreeMap<String, Option<String>>,
}

fn run_oracle() -> BTreeMap<String, OracleResult> {
    let mut cmd = Command::new("java");
    cmd.arg(Path::new(TESTDATA).join("oracle/LegacyOracle.java"));
    for (name, utf8) in FIXTURES {
        cmd.arg(if *utf8 { "--utf8" } else { "--latin1" });
        cmd.arg(fixture(name));
    }
    let out = cmd.output().expect("java");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    let mut results: BTreeMap<String, OracleResult> = BTreeMap::new();
    let mut current = String::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        match parts[0] {
            "FILE" => {
                current = Path::new(parts[1]).file_name().unwrap().to_string_lossy().into_owned();
                results.entry(current.clone()).or_default();
            }
            "RAW" => {
                results.get_mut(&current).unwrap().raw.insert(unescape(parts[1]), unescape(parts[2]));
            }
            "VAL" => {
                let v = if parts[2] == "<null>" { None } else { Some(unescape(parts[2])) };
                results.get_mut(&current).unwrap().val.insert(parts[1].to_owned(), v);
            }
            _ => {}
        }
    }
    results
}

fn b(x: bool) -> Option<String> {
    Some(x.to_string())
}

fn s<T: AsRef<str>>(x: Option<T>) -> Option<String> {
    x.map(|v| v.as_ref().to_owned())
}

fn p(x: &Option<PathBuf>) -> Option<String> {
    x.as_ref().map(|v| v.to_string_lossy().into_owned())
}

#[test]
fn values_match_java_oracle() {
    if !java_available() {
        eprintln!("übersprungen: kein java im PATH");
        return;
    }
    let oracle = run_oracle();
    assert_eq!(oracle.len(), FIXTURES.len());
    for (name, utf8) in FIXTURES {
        let o = &oracle[*name];
        let cfg = LegacyConfig::load(&fixture(name)).unwrap();
        assert_eq!(cfg.encoding == Encoding::Utf8, *utf8, "{name}: Kodierungsheuristik");
        // Rohwerte identisch (Properties.load bzw. UTF-8-Reader)
        assert_eq!(cfg.props.len(), o.raw.len(), "{name}");
        for (k, v) in o.raw.iter() {
            assert_eq!(cfg.props.get(k), Some(v), "{name}: RAW {k}");
        }
        let (v, _warnings) = cfg.values();
        let get = |k: &str| o.val.get(k).unwrap_or_else(|| panic!("{name}: {k} fehlt im Orakel")).clone();
        let check = |k: &str, ours: Option<String>| assert_eq!(ours, get(k), "{name}: {k}");

        check("BKU", Some(v.bku.map(BackendChoice::legacy_name).unwrap_or("NONE").to_owned()));
        check("SIGNATURE_POSITION", b(v.auto_position));
        check("USE_MARKER", b(v.use_marker));
        check("USE_FIELDS", b(v.use_fields));
        check("USE_PLACEHOLDER_SEARCH", b(v.use_placeholder_search));
        check("MOBILE_NUMBER", v.mobile_number.clone());
        check("MOBILE_REMEMBER_PASSWORD", b(v.mobile_remember_password));
        check(
            "SIGNATURE_NOTE",
            Some(match &v.signature_note {
                LegacyNote::Missing => "<MISSING>".to_owned(),
                LegacyNote::Empty => "<EMPTY>".to_owned(),
                LegacyNote::Text(t) => t.clone(),
            }),
        );
        check("SIGNATURE_PDFA_COMPAT", b(v.pdfa_compat));
        check("SIGNATURE_PROFILE", Some(v.profile.legacy_name().to_owned()));
        check("EMBLEM", p(&v.emblem));
        check("UPDATE_CHECK", b(v.update_check));
        check("PROXY_HOST", v.proxy_host.clone());
        check("PROXY_USER", v.proxy_user.clone());
        check("PROXY_PASS", v.proxy_pass.clone());
        check("OUTPUT_FOLDER", p(&v.output_folder));
        check("SAVE_FILE_POSTFIX", Some(v.save_file_postfix.clone()));
        check("KEYSTORE_ENABLED", b(v.keystore_enabled));
        check("KEYSTORE_FILE", Some(p(&v.keystore_file).unwrap_or_default()));
        check("KEYSTORE_TYPE", Some(v.keystore_type.clone().unwrap_or_default()));
        check("KEYSTORE_ALIAS", Some(v.keystore_alias.clone().unwrap_or_default()));
        check("KEYSTORE_STOREPASS", v.keystore_storepass.clone());
        check("KEYSTORE_KEYPASS", v.keystore_keypass.clone());
        check(
            "KEYSTORE_PASS_STORE_TYPE",
            Some(
                match v.keystore_pass_store_type {
                    PasswordStorage::Keychain => "DISK",
                    PasswordStorage::Memory => "MEMORY",
                    PasswordStorage::None => "NONE",
                }
                .to_owned(),
            ),
        );
        check("FIDO2_DEFAULT", b(v.fido2_default));

        // LAST_SOURCE_DIRECTORY: leer ist bei uns „kein Ordner“.
        let java_last = get("LAST_SOURCE_DIRECTORY").filter(|x| !x.is_empty());
        assert_eq!(p(&v.last_source_directory), java_last, "{name}: LAST_SOURCE_DIRECTORY");

        // PROXY_PORT: Java −1 = keiner; <ERROR>/<RANGE> = bei uns None + Warnung.
        let java_port = get("PROXY_PORT").unwrap();
        let expected_port = java_port.parse::<i64>().ok().filter(|x| *x > 0).map(|x| x as u16);
        assert_eq!(v.proxy_port, expected_port, "{name}: PROXY_PORT ({java_port})");

        // LOGO_ONLY_SIZE: gleich, außer NaN/∞/≤ 0 (bei uns Standard 23).
        let java_logo: f64 = match get("LOGO_ONLY_SIZE").unwrap().as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            x => x.parse().unwrap(),
        };
        let expected_logo = if java_logo.is_finite() && java_logo > 0.0 { java_logo } else { 23.0 };
        assert_eq!(v.logo_only_size_mm, expected_logo, "{name}: LOGO_ONLY_SIZE");

        // MAINWINDOW_SIZE: Java liefert immer einen Wert; negative Werte verwerfen wir.
        let java_size = get("MAINWINDOW_SIZE").unwrap();
        let (jw, jh) = java_size.split_once(',').unwrap();
        let (jw, jh): (i64, i64) = (jw.parse().unwrap(), jh.parse().unwrap());
        let ours = v.mainwindow_size.unwrap_or((600, 780));
        if jw >= 0 && jh >= 0 {
            assert_eq!((i64::from(ours.0), i64::from(ours.1)), (jw, jh), "{name}: MAINWINDOW_SIZE");
        } else {
            assert_eq!(v.mainwindow_size, None, "{name}: MAINWINDOW_SIZE negativ");
        }

        // Locales: Sprache der gefundenen Java-Locale.
        let java_sign = get("SIGNLOCALE");
        let expected_sign = java_sign.as_deref().map(|l| if l == "en" { Lang::En } else { Lang::De });
        assert_eq!(v.sign_locale, expected_sign, "{name}: SIGNLOCALE");
        let java_locale = get("LOCALE");
        let expected_locale = java_locale.as_deref().map(|l| if l == "de" { Lang::De } else { Lang::En });
        assert_eq!(v.locale, expected_locale, "{name}: LOCALE");
    }
}

/// Die eingebettete Locale-Liste entspricht `Locale.getAvailableLocales()` des JDK 21.
#[test]
fn embedded_locale_list_matches_jdk21() {
    if !java_available() {
        eprintln!("übersprungen: kein java im PATH");
        return;
    }
    let version = Command::new("java").args(["-XshowSettings:properties", "-version"]).output().unwrap();
    let props = String::from_utf8_lossy(&version.stderr);
    if !props.contains("java.specification.version = 21") {
        eprintln!("übersprungen: kein JDK 21");
        return;
    }
    let out = Command::new("java")
        .arg(Path::new(TESTDATA).join("oracle/LegacyOracle.java"))
        .arg("--locales")
        .output()
        .unwrap();
    let java: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(str::to_owned).collect();
    let asset = include_str!("../assets/java21-available-locales.txt");
    let ours: Vec<String> = asset.lines().map(str::to_owned).collect();
    assert_eq!(java, ours);
}

/// `Properties.store` von Java (PDF-Over) und `store_latin1` sind bytegleich.
#[test]
fn java_store_format_is_reproduced() {
    for name in ["pdf-over-generated-de.config", "pdf-over-generated-en-keystore.config"] {
        let bytes = std::fs::read(fixture(name)).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let date = text.lines().nth(1).unwrap().trim_start_matches('#').to_owned();
        let cfg = LegacyConfig::parse(&bytes).unwrap();
        let ours = cfg.props.store_latin1(&StoreOptions {
            comment: Some("Configuration file was generated!".into()),
            date_line: Some(date),
            ..Default::default()
        });
        assert_eq!(String::from_utf8_lossy(&ours), text, "{name}");
    }
}
