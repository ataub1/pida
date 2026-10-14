//! Robustheit des Konverters über die PDF-Over-Kataloge hinaus: zufällige Java-Werte aus einem
//! Alphabet mit allen Sonderfällen (Leerraum am Zeilenrand, `{`/`}`, `[`/`*`/`.` am Zeilenanfang,
//! Steuerzeichen, `&`-Mnemonics, `%s`/`%d`/`%%`, Anführungszeichen, Backslashes, Unicode-Trenner)
//! werden konvertiert, als Fluent-Katalog geladen und formatiert. Erwartet wird genau der Java-Text
//! (Mnemonics entfernt, Platzhalter ersetzt). Deterministischer Generator (xorshift), kein Netz.

use pdfover_i18n::convert::{self, ConvertOptions};
use pdfover_i18n::{ArgValue, Catalog, Lang};
use pdfover_java_properties::Properties;

/// Unabhängige Referenz: Java-Wert → erwarteter Text und Anzahl Platzhalter.
fn expected_text(java: &str) -> (String, usize) {
    let mut out = String::new();
    let mut n = 0usize;
    let mut it = java.chars().peekable();
    while let Some(c) = it.next() {
        match (c, it.peek().copied()) {
            ('&', Some('&')) => {
                out.push('&');
                it.next();
            }
            ('&', Some(x)) if x.is_alphanumeric() => {}
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
    (out, n)
}

#[test]
fn random_values_roundtrip_through_fluent() {
    let alphabet = [
        ' ', ' ', '\n', '\t', '\r', '{', '}', '[', '*', '.', '"', '\\', '&', '%', 's', 'd', 'a', 'B', '#', '-', '=',
        ':', '$', 'ü', '\u{1}', '\u{a0}', '\u{2028}', '<', '>', '\u{feff}', '\u{7f}', '\u{85}',
    ];
    let mut seed: u64 = 0x1234_5678_9abc_def0;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for case in 0..20_000 {
        let len = (next() % 25) as usize;
        let value: String = (0..len).map(|_| alphabet[(next() % alphabet.len() as u64) as usize]).collect();
        let props: Properties = [("k.x", value.as_str())].into_iter().collect();
        let conv = convert::properties_to_ftl(&props, &ConvertOptions::default())
            .unwrap_or_else(|e| panic!("Fall {case}: {value:?}: {e}"));
        let catalog = Catalog::from_sources(&[(Lang::De, &conv.ftl)])
            .unwrap_or_else(|e| panic!("Fall {case}: {value:?}: {e}\n{}", conv.ftl));
        let (expected, n) = expected_text(&value);
        let args: Vec<(String, ArgValue)> =
            (1..=n).map(|i| (format!("arg{i}"), ArgValue::Str(format!("<{i}>")))).collect();
        let args_ref: Vec<(&str, ArgValue)> = args.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        assert_eq!(catalog.format(Lang::De, "k-x", &args_ref), expected, "Fall {case}: {value:?}\n{}", conv.ftl);
    }
}

/// Lange Werte und viele Nachrichten bleiben handhabbar.
#[test]
fn large_inputs() {
    let long = "Lorem ipsum dolor sit amet ".repeat(20_000);
    let props: Properties = (0..2_000)
        .map(|i| (format!("k.{i}"), format!("Wert {i} %s")))
        .chain([("long".to_owned(), long.clone())])
        .collect();
    let conv = convert::properties_to_ftl(&props, &ConvertOptions::default()).unwrap();
    assert_eq!(conv.messages, 2_001);
    assert!(conv.warnings.is_empty());
    let catalog = Catalog::from_sources(&[(Lang::En, &conv.ftl)]).unwrap();
    assert_eq!(catalog.format(Lang::En, "long", &[]), long, "nachgestellte Leerzeichen bleiben erhalten");
}

/// `fluent-bundle` löst höchstens 100 Placeables je Nachricht auf und schneidet den Rest ab; der
/// Konverter fasst benachbarte maskierte Zeichen zusammen und warnt, wenn die Grenze überschritten wird.
#[test]
fn placeable_limit_is_reported() {
    let check = |value: &str| {
        let props: Properties = [("k", value)].into_iter().collect();
        let conv = convert::properties_to_ftl(&props, &ConvertOptions::default()).unwrap();
        let catalog = Catalog::from_sources(&[(Lang::De, &conv.ftl)]).unwrap();
        (conv.warnings, catalog.format(Lang::De, "k", &[]) == value)
    };
    // 1000 Klammern hintereinander = ein Literal
    assert_eq!(check(&"{}".repeat(500)), (vec![], true));
    // 100 getrennte Klammern: genau an der Grenze, noch vollständig
    assert_eq!(check(&"x{".repeat(100)), (vec![], true));
    let (warnings, complete) = check(&"x{".repeat(101));
    assert!(!complete);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("101 Placeables"), "{warnings:?}");
}
