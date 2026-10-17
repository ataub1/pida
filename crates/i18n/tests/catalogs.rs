//! Integrationstests der Kataloge: Konvertierung aus den PDF-Over-Quellen (Kopie unter
//! `testdata/i18n/`), Vollständigkeit DE/EN, semantischer Rundlauf jeder Nachricht, Java-Orakel.
//!
//! Neu erzeugen der `locales/<lang>/pdfover.ftl`: `PIDA_I18N_BLESS=1 cargo test -p pida-i18n --test catalogs`.

use std::collections::BTreeSet;
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
    for file in ["pdfover.ftl", "pida.ftl", "ui-workspace.ftl", "ui-tools.ftl"] {
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

/// Nachrichten-IDs eines FTL-Texts (ohne Terme).
fn message_ids(ftl: &str) -> BTreeSet<String> {
    let resource = fluent_syntax::parser::parse(ftl).unwrap_or_else(|(_, e)| panic!("ungültiges FTL: {e:?}"));
    resource
        .body
        .iter()
        .filter_map(|e| match e {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_owned()),
            _ => None,
        })
        .collect()
}

/// ADR 0004 §3.16/§4.5: Die UI-Kataloge der neuen Oberfläche – `ui-workspace.ftl` (Paket UI-A) und
/// `ui-tools.ftl` (UI-B) – bilden mit `pdfover.ftl` und `pida.ftl` **ein** Fluent-Bündel je Sprache
/// (`apps/app/src/i18n/catalog.ts`): Keine Datei teilt eine ID mit einer anderen (doppelte IDs wären ein
/// Ladefehler), jede `ui-*.ftl`-ID heißt `ng-ui-<bereich>-…` mit einem Bereich des Pakets, der in
/// `pida.ftl` nicht belegt ist; DE und EN mit denselben Variablen prüft
/// `de_and_en_are_complete_with_same_variables`.
#[test]
fn ui_catalogs_are_disjoint_and_use_their_own_areas() {
    const FILES: [&str; 4] = ["pdfover.ftl", "pida.ftl", "ui-workspace.ftl", "ui-tools.ftl"];
    const AREAS: [(&str, &[&str]); 2] = [
        ("ui-workspace.ftl", &["shell", "start", "ws", "insp", "sc", "fin", "batch"]),
        ("ui-tools.ftl", &["set", "pal", "sh", "ob", "recent"]),
    ];
    let area = |id: &str| id.strip_prefix("ng-ui-").and_then(|r| r.split('-').next()).map(str::to_owned);
    for lang in Lang::ALL {
        let ids: Vec<(&str, BTreeSet<String>)> =
            FILES.iter().map(|f| (*f, message_ids(&read(&locale_file(lang, f))))).collect();
        for (i, (a, ia)) in ids.iter().enumerate() {
            for (b, ib) in &ids[i + 1..] {
                let shared: Vec<&String> = ia.intersection(ib).collect();
                assert!(shared.is_empty(), "{lang:?}: {a} und {b} teilen IDs: {shared:?}");
            }
        }
        let pida_areas: BTreeSet<String> = ids[1].1.iter().filter_map(|id| area(id)).collect();
        for (file, areas) in AREAS {
            for a in areas {
                assert!(!pida_areas.contains(*a), "Bereich {a} ({file}) ist in pida.ftl schon belegt");
            }
            let (_, file_ids) = ids.iter().find(|(f, _)| *f == file).expect("Datei");
            for id in file_ids {
                let ok = area(id).is_some_and(|a| areas.contains(&a.as_str()));
                assert!(ok, "{lang:?} {file}: {id} liegt nicht in den Bereichen {areas:?}");
            }
        }
    }
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

/// Terminologie (auch in den aus PDF-Over erzeugten Texten, über die Korrekturen des Konverters):
/// „Chipkarte“/„Chipkarten-Software“ statt „Bürgerkarte“/„Bürgerkartenumgebung“, EN „smart card“ statt
/// „citizen card“. URLs wie www.buergerkarte.at bleiben.
#[test]
fn terminology_uses_smart_card() {
    let files: Vec<(Lang, String)> = Lang::ALL
        .into_iter()
        .flat_map(|lang| {
            ["pdfover.ftl", "pida.ftl", "ui-workspace.ftl", "ui-tools.ftl"].map(|f| (lang, read(&locale_file(lang, f))))
        })
        .collect();
    let sources: Vec<(Lang, &str)> = files.iter().map(|(l, s)| (*l, s.as_str())).collect();
    let c = Catalog::from_sources(&sources).unwrap();
    for lang in Lang::ALL {
        for id in c.message_ids(lang) {
            let text = c.format(lang, id, &[]).to_lowercase();
            for old in ["bürgerkarte", "citizen card", "citizen-card"] {
                assert!(!text.contains(old), "{lang:?} {id}: {text}");
            }
        }
    }
    assert_eq!(c.format(Lang::De, "BKU-LOCAL", &[]), "Lokale Chipkarte");
    assert_eq!(c.format(Lang::En, "BKU-LOCAL", &[]), "Local Smart Card");
    let de = c.format(Lang::De, "error-LocalBKU", &[]);
    assert!(de.contains("lokale Chipkarten-Software (BKU)") && de.contains("auf Ihre Chipkarte"), "{de}");
    assert!(de.contains("www.buergerkarte.at"), "{de}");
    let en = c.format(Lang::En, "error-LocalBKU", &[]);
    assert!(en.contains("(smart card software)") && en.contains("your smart card"), "{en}");
    assert!(c.format(Lang::De, "config-DataProtectionStatement", &[]).contains("mit der Chipkarte"));
    assert!(c.format(Lang::En, "config-DataProtectionStatement", &[]).contains("with the smart card"));
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

/// Texte einer Nachricht (Wert und Attribute, ohne Platzhalter) je ID.
fn message_texts(ftl: &str) -> Vec<(String, String)> {
    use fluent_syntax::ast::{Entry, Pattern, PatternElement};
    fn text(p: &Pattern<&str>, out: &mut String) {
        for e in &p.elements {
            match e {
                PatternElement::TextElement { value } => out.push_str(value),
                PatternElement::Placeable { expression } => {
                    // Varianten (Plural) mitnehmen
                    if let fluent_syntax::ast::Expression::Select { variants, .. } = expression {
                        for v in variants {
                            out.push(' ');
                            text(&v.value, out);
                        }
                    }
                    out.push(' ');
                }
            }
        }
    }
    let resource = fluent_syntax::parser::parse(ftl).unwrap_or_else(|(_, e)| panic!("ungültiges FTL: {e:?}"));
    resource
        .body
        .iter()
        .filter_map(|e| match e {
            Entry::Message(m) => {
                let mut out = String::new();
                if let Some(v) = &m.value {
                    text(v, &mut out);
                }
                for a in &m.attributes {
                    out.push(' ');
                    text(&a.value, &mut out);
                }
                Some((m.id.name.to_owned(), out))
            }
            _ => None,
        })
        .collect()
}

/// Wörter eines Texts (Buchstabenfolgen, klein).
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// ADR 0004 §8 F3: PIDA behauptet **nie** Gültigkeit oder Rechtsgültigkeit – die App prüft nur die eigene neue
/// Signatur (Selbstprüfung), einen Prüfmodus gibt es nicht mehr (Nachtrag 15), das Entwickler-Werkzeug `pida verify`
/// sagt „intakt“ bzw. „nicht intakt“ und nennt den Umfang (Vertrauenskette und Sperrstatus nicht geprüft). Geprüft
/// werden die Signaturtexte des Kerns (`ng-verify-*`, `ng-cli-verify-*`, die Codes der Integrität `cms-*`,
/// `pdf-signature-*`, die Übernahme-/Einrichtungstexte) ohne das Wort „gültig“ bzw. „valid“ (verneint als
/// „ungültig“/„invalid“ erlaubt); in allen Katalogen der Oberfläche nie „rechtsgültig“/„legally valid“.
#[test]
fn verify_texts_never_claim_validity() {
    let verify_ids = |id: &str| {
        [
            "ng-verify-",
            "ng-cli-verify-",
            "ng-cli-help-cmd-verify",
            "cms-",
            "pdf-signature-",
            "ng-firstrun-",
            "ng-migration-",
        ]
        .iter()
        .any(|p| id.starts_with(p))
    };
    for lang in Lang::ALL {
        let (claim, legal) = match lang {
            Lang::De => ("gültig", ["rechtsgültig", "rechtsgültige", "rechtsgültigen", "rechtsgültiger"].as_slice()),
            Lang::En => ("valid", ["legally"].as_slice()),
        };
        let pida = message_texts(&read(&locale_file(lang, "pida.ftl")));
        let checked: Vec<&(String, String)> = pida.iter().filter(|(id, _)| verify_ids(id)).collect();
        assert!(checked.len() >= 30, "{lang:?}: nur {} Signaturtexte", checked.len());
        for id in ["ng-cli-verify-intact", "ng-cli-verify-not-intact", "ng-cli-verify-scope", "ng-verify-byte-range"] {
            assert!(checked.iter().any(|(i, _)| i == id), "{lang:?}: {id}");
        }
        // Prüfmodus entfernt (Nachtrag 15): seine Texte stehen in keinem Katalog mehr
        for file in ["pida.ftl", "ui-workspace.ftl", "ui-tools.ftl"] {
            for (id, _) in message_texts(&read(&locale_file(lang, file))) {
                let removed = id.starts_with("ng-ui-vf-")
                    || id.starts_with("ng-ui-shell-verify")
                    || id.starts_with("ng-ui-start-verify")
                    || id == "ng-ui-shell-cmd-verify"
                    || ["ng-verify-open-failed", "ng-verify-busy", "ng-cli-valid", "ng-cli-invalid"]
                        .contains(&id.as_str())
                    || id.starts_with("ng-verify-warning-");
                assert!(!removed, "{lang:?} {file}: {id} gehört zum entfernten Prüfmodus");
            }
        }
        for (id, text) in checked {
            let w = words(text);
            assert!(!w.iter().any(|w| w == claim), "{lang:?} {id}: „{claim}“ in „{text}“");
            assert!(!w.iter().any(|w| legal.contains(&w.as_str())), "{lang:?} {id}: „{text}“");
        }
        for file in ["pida.ftl", "ui-workspace.ftl", "ui-tools.ftl"] {
            for (id, text) in message_texts(&read(&locale_file(lang, file))) {
                let w = words(&text);
                let legally_valid = match lang {
                    Lang::De => w.iter().any(|w| w.starts_with("rechtsgültig")),
                    Lang::En => w.windows(2).any(|p| p[0] == "legally" && p[1] == "valid"),
                };
                assert!(!legally_valid, "{lang:?} {file} {id}: „{text}“");
            }
        }
    }
}
