//! Vertrag Kern ↔ Oberfläche: Die TypeScript-Typen in `apps/app/src/core/generated.ts` entsprechen
//! genau den Rust-Typen der Bridge (`pdfover_core_api::ts::typescript_bindings`, ts-rs).
//!
//! Weicht die Datei ab (Rust-Typ geändert, Datei von Hand bearbeitet), schlägt der Test fehl.
//! Neu erzeugen: `PDFOVER_BLESS=1 cargo test -p pdfover-core-api --test ts_bindings`.

use std::collections::BTreeSet;

const GENERATED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/app/src/core/generated.ts");

#[test]
fn generated_typescript_matches_rust_types() {
    let expected = pdfover_core_api::ts::typescript_bindings();
    if std::env::var_os("PDFOVER_BLESS").is_some() {
        std::fs::write(GENERATED, &expected).expect("generated.ts schreiben");
        return;
    }
    let actual = std::fs::read_to_string(GENERATED).unwrap_or_default().replace("\r\n", "\n");
    if actual != expected {
        let first = actual
            .lines()
            .zip(expected.lines())
            .position(|(a, e)| a != e)
            .map_or_else(|| "Länge".to_owned(), |n| format!("Zeile {}", n + 1));
        panic!(
            "apps/app/src/core/generated.ts weicht von den Rust-Typen ab (erste Abweichung: {first}). \
             Neu erzeugen: PDFOVER_BLESS=1 cargo test -p pdfover-core-api --test ts_bindings"
        );
    }
}

/// Jeder Typname, den eine Deklaration verwendet, ist selbst deklariert (keine vergessenen Typen).
#[test]
fn every_referenced_type_is_declared() {
    let (text, names) = pdfover_core_api::ts::typescript_bindings_with_names();
    let declared: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(declared.len(), names.len(), "doppelte Typnamen: {names:?}");
    const BUILTIN: &[&str] = &["Array", "Record", "Partial"];
    // Kommentare (Kopf, JSDoc) entfernen; übrig bleibt der Typtext.
    let mut code = String::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find("/*") {
        code.push_str(&rest[..start]);
        rest = rest[start..].find("*/").map_or("", |end| &rest[start + end + 2..]);
    }
    code.push_str(rest);
    let code: String = code.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    let mut missing = BTreeSet::new();
    let mut in_str = false;
    let mut ident = String::new();
    let mut after_type_keyword = false;
    for c in code.chars().chain(std::iter::once(' ')) {
        if c == '"' {
            in_str = !in_str;
            ident.clear();
            continue;
        }
        if in_str {
            continue;
        }
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(c);
            continue;
        }
        if !ident.is_empty() {
            let is_type = ident.starts_with(|c: char| c.is_ascii_uppercase());
            if is_type
                && !after_type_keyword
                && !declared.contains(ident.as_str())
                && !BUILTIN.contains(&ident.as_str())
            {
                missing.insert(ident.clone());
            }
            after_type_keyword = ident == "type";
            ident.clear();
        }
    }
    assert!(missing.is_empty(), "nicht deklarierte Typen in generated.ts: {missing:?}");
}
