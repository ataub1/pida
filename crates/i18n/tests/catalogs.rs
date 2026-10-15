//! Integrationstests der Kataloge: Konvertierung aus den PDF-Over-Quellen (Kopie unter
//! `testdata/i18n/`), Vollständigkeit DE/EN, semantischer Rundlauf jeder Nachricht, Java-Orakel.
//!
//! Neu erzeugen der `locales/<lang>/pdfover.ftl`: `PIDA_I18N_BLESS=1 cargo test -p pida-i18n --test catalogs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use pida_i18n::convert::{self, Placeholder};
use pida_i18n::{ArgValue, Catalog, Lang, fluent_id_for_java_key, markup};
use pida_java_properties::Properties;

const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/i18n");
const LOCALES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/locales");

fn source_path(lang: Lang) -> PathBuf {
    Path::new(TESTDATA).join(match lang {
        Lang::De => "messages_de.properties",
        Lang::En => "messages.properties",
    })
}

fn source(lang: Lang) -> Vec<u8> {
    std::fs::read(source_path(lang)).expect("testdata/i18n vorhanden")
}

fn locale_file(lang: Lang, name: &str) -> PathBuf {
    Path::new(LOCALES).join(lang.tag()).join(name)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn generated_catalogs_are_up_to_date() {
    let bless = std::env::var_os("PIDA_I18N_BLESS").is_some();
    for lang in Lang::ALL {
        let conv = convert::convert_pdfover_messages(&source(lang), lang).expect("Konvertierung");
        assert!(conv.warnings.is_empty(), "{lang:?}: {:?}", conv.warnings);
        assert_eq!(conv.messages, 273, "{lang:?}: Anzahl Nachrichten nach Korrektur/Ergänzung");
        let path = locale_file(lang, "pdfover.ftl");
        if bless {
            std::fs::write(&path, &conv.ftl).expect("schreiben");
        } else {
            assert!(
                read(&path) == conv.ftl,
                "{} ist veraltet: PIDA_I18N_BLESS=1 cargo test -p pida-i18n --test catalogs",
                path.display()
            );
        }
    }
}

#[test]
fn source_counts_match_analysis() {
    // docs/analyse/01 §6.2: EN 272 eindeutige Keys, DE 268 (+1 Duplikat).
    let en = convert::raw_entries(&convert::decode_resource_bundle(&source(Lang::En))).unwrap();
    let de = convert::raw_entries(&convert::decode_resource_bundle(&source(Lang::De))).unwrap();
    let uniq =
        |e: &[(String, String)]| e.iter().map(|(k, _)| k.as_str()).collect::<std::collections::BTreeSet<_>>().len();
    assert_eq!((en.len(), uniq(&en)), (272, 272));
    assert_eq!((de.len(), uniq(&de)), (269, 268));
    let with = |e: &[(String, String)], pred: &dyn Fn(&str) -> bool| e.iter().filter(|(_, v)| pred(v)).count();
    assert_eq!(with(&en, &|v| v.contains("%s") || v.contains("%d")), 23);
    assert_eq!(with(&de, &|v| v.contains("%s") || v.contains("%d")), 23);
    assert_eq!(with(&en, &|v| v.contains("<a")), 11);
    assert_eq!(with(&de, &|v| v.contains("<a")), 10);
}

#[test]
fn de_and_en_are_complete_with_same_variables() {
    let de = convert::prepare_pdfover_messages(&source(Lang::De), Lang::De).unwrap().props;
    let en = convert::prepare_pdfover_messages(&source(Lang::En), Lang::En).unwrap().props;
    assert_eq!(convert::compare_catalogs(&en, &de), Vec::<String>::new());
    for file in ["pdfover.ftl", "pida.ftl"] {
        let problems = convert::compare_ftl(&read(&locale_file(Lang::En, file)), &read(&locale_file(Lang::De, file)))
            .expect("gültiges FTL");
        assert_eq!(problems, Vec::<String>::new(), "{file}");
    }
    let catalog = Catalog::builtin().unwrap();
    let de_ids: Vec<&str> = catalog.message_ids(Lang::De).collect();
    let en_ids: Vec<&str> = catalog.message_ids(Lang::En).collect();
    assert_eq!(de_ids, en_ids);
    assert!(de_ids.iter().filter(|id| id.starts_with("ng-")).count() >= 30);
}

/// Unabhängige Referenz: Java-Wert → erwarteter Text (Mnemonics entfernt, `%s`/`%d` → `<n>`).
fn expected_text(java: &str) -> (String, Option<char>, usize) {
    let mut out = String::new();
    let mut key = None;
    let mut n = 0usize;
    let mut it = java.chars().peekable();
    while let Some(c) = it.next() {
        match (c, it.peek().copied()) {
            ('&', Some('&')) => {
                out.push('&');
                it.next();
            }
            ('&', Some(x)) if x.is_alphanumeric() => {
                key.get_or_insert(x);
            }
            ('%', Some('%')) => {
                out.push('%');
                it.next();
            }
            ('%', Some('s' | 'd')) => {
                n += 1;
                out.push_str(&format!("<{n}>"));
                it.next();
            }
            (c, _) => out.push(c),
        }
    }
    (out, key, n)
}

#[test]
fn every_message_roundtrips_semantically() {
    let catalog = Catalog::builtin().unwrap();
    for lang in Lang::ALL {
        let props = convert::prepare_pdfover_messages(&source(lang), lang).unwrap().props;
        for (key, value) in props.iter() {
            let id = fluent_id_for_java_key(key);
            assert!(catalog.has(lang, &id), "{lang:?}: {id} fehlt");
            let (expected, access_key, n) = expected_text(value);
            let args: Vec<(String, ArgValue)> =
                (1..=n).map(|i| (format!("arg{i}"), ArgValue::Str(format!("<{i}>")))).collect();
            let args_ref: Vec<(&str, ArgValue)> = args.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            let actual = catalog.format(lang, &id, &args_ref);
            assert_eq!(actual, expected, "{lang:?} {key}");
            assert_eq!(catalog.attribute(lang, &id, "accesskey"), access_key.map(|c| c.to_string()), "{lang:?} {key}");
        }
    }
}

#[test]
fn numbers_format_like_java() {
    let c = Catalog::builtin().unwrap();
    let s = c.format(Lang::De, "error-SLError", &[("arg1", 6002u32.into()), ("arg2", "Abbruch".into())]);
    assert_eq!(s, "Vorgang vom Signaturserver abgebrochen (6002):\nAbbruch");
    let s = c.format(
        Lang::En,
        "exception-InvalidPort",
        &[("arg1", "x".into()), ("arg2", 1u32.into()), ("arg3", 65535u32.into())],
    );
    assert_eq!(s, "x is invalid: has to be a number between 1 and 65535");
}

#[test]
fn bug_fixes_are_applied() {
    let c = Catalog::builtin().unwrap();
    // B-09
    assert!(c.format(Lang::De, "error-Initialization", &[]).starts_with("Initialisierung fehlgeschlagen."));
    let cfg = c.format(Lang::De, "error-ConfigInitialization", &[]);
    assert!(cfg.starts_with("Der Einstellungsbildschirm konnte nicht geladen werden."), "{cfg}");
    assert!(cfg.contains("Wir bitten um Verzeihung"));
    // B-08 (nur EN war vertauscht)
    assert!(c.format(Lang::En, "argument-help-keystorestorepass", &[]).ends_with("-kspass <password>"));
    assert!(c.format(Lang::En, "argument-help-keystorekeypass", &[]).ends_with("-kskeypass <password>"));
    assert!(c.format(Lang::De, "argument-help-keystorestorepass", &[]).ends_with("-kspass <Passwort>"));
    // B-07 Hilfetext
    assert!(c.format(Lang::De, "argument-help-vis", &[]).contains("nein"));
    assert!(c.format(Lang::En, "tanEnter-APPTAN", &[]).contains("ID Austria"));
    // Kommentare im erzeugten Katalog
    let de = read(&locale_file(Lang::De, "pdfover.ftl"));
    assert!(de.contains("# korrigiert (B-09)"));
    assert!(de.contains("# unused (in PDF-Over 4.4.x nicht referenziert)\ntanEnter-tries ="));
}

#[test]
fn special_values_survive() {
    let c = Catalog::builtin().unwrap();
    assert_eq!(c.format(Lang::De, "simple_config-Note_Default_Empty", &[]), "");
    let fido = c.format(Lang::En, "config-fido2-UnsupportedTryLibFido2", &[]);
    assert!(fido.contains(":\n ● <a>LibFido2</a>\n\n"), "{fido:?}");
    let about = c.format(Lang::De, "config-AboutText", &[]);
    assert_eq!(markup::parse(&about).into_iter().filter(|s| matches!(s, markup::Segment::Link { .. })).count(), 2);
    assert!(markup::to_plain(&about).contains("Technischer Support unter software@egiz.gv.at."));
    let note = c.format(Lang::De, "simple_config-Note_Default_Standard", &[]);
    assert!(note.contains("(\"eIDAS-VO\")"));
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
                out.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

/// Orakel: Java (`PropertyResourceBundle`, wie PDF-Over) liest die Quellen identisch zu unserem
/// Parser (inklusive „letzter Wert gewinnt“ beim DE-Duplikat), und `String.format` liefert – nach
/// Entfernen der Mnemonics – denselben Text wie der Fluent-Katalog. Ohne `java` übersprungen.
#[test]
fn java_oracle_reads_and_formats_identically() {
    if !java_available() {
        eprintln!("übersprungen: kein java im PATH");
        return;
    }
    let catalog = Catalog::builtin().unwrap();
    let oracle = Path::new(TESTDATA).join("oracle/DumpBundle.java");
    for lang in Lang::ALL {
        let out = Command::new("java").arg(&oracle).arg(source_path(lang)).output().expect("java");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = String::from_utf8(out.stdout).unwrap();
        let mut java = Properties::new();
        let mut formatted = Properties::new();
        for line in text.lines() {
            let mut parts = line.splitn(3, '\t');
            let kind = parts.next().unwrap();
            let (Some(k), v) = (parts.next(), parts.next().unwrap_or("")) else { continue };
            match kind {
                "E" => {
                    java.insert(unescape(k), unescape(v));
                }
                "F" => {
                    formatted.insert(unescape(k), unescape(v));
                }
                _ => {}
            }
        }
        let ours = Properties::load_str(&convert::decode_resource_bundle(&source(lang))).unwrap();
        assert_eq!(java.len(), ours.len(), "{lang:?}");
        for (k, v) in ours.iter() {
            assert_eq!(java.get(k), Some(v), "{lang:?} {k}");
        }
        let prepared = convert::prepare_pdfover_messages(&source(lang), lang).unwrap().props;
        let mut compared = 0;
        for (k, v) in ours.iter() {
            if prepared.get(k) != Some(v) {
                continue; // korrigierter Wert (B-08/B-09/Tippfehler)
            }
            let ph = convert::java_placeholders(v);
            let args: Vec<(String, ArgValue)> = ph
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let n = i + 1;
                    let value = match p {
                        Placeholder::Str => ArgValue::Str(format!("<{n}>")),
                        Placeholder::Int => ArgValue::Num((1000 + n) as f64),
                    };
                    (format!("arg{n}"), value)
                })
                .collect();
            let args_ref: Vec<(&str, ArgValue)> = args.iter().map(|(a, b)| (a.as_str(), b.clone())).collect();
            let (expected, ..) = expected_text(formatted.get(k).expect("F-Zeile"));
            assert_eq!(catalog.format(lang, &fluent_id_for_java_key(k), &args_ref), expected, "{lang:?} {k}");
            compared += 1;
        }
        assert!(compared > 260, "{lang:?}: nur {compared} verglichen");
    }
}
