//! Vollständigkeit der Kataloge für die stabilen Codes der Crates (ADR 0001 Regel 11, Plan §7.5).
//!
//! - Jeder Code, den der Quelltext eines Crates liefert, steht in `locales/de` **und** `locales/en`
//!   (`pida.ftl`) mit denselben Variablen. Gefunden werden die Codes über einen kleinen
//!   Rust-Lexer (Strings, Kommentare, Attribute, `#[cfg(test)]`-Module) in `crates/*/src` und
//!   `apps/*/src` bzw. `apps/*/src-tauri/src`:
//!   - alle String-Literale in `fn code(…)`/`fn message_id(…)` und in `const NOTICE_*: &str`,
//!   - jedes Literal, das wie ein Code einer bekannten Familie aussieht (`cms-…`, `backend-…`,
//!     `render.…` …, siehe [`is_family_code`]).
//! - Codes mit Punkt werden wie Java-Keys abgebildet (`fluent_id_for_java_key`: `.` → `-`).
//! - `NoticeCode::code`/`FormProblem::code` (backend-api) bilden jede Variante ohne Platzhalter-Arm
//!   (`_ =>`) ab; die gelieferten IDs prüft der Quelltext-Scanner wie alle anderen Codes.
//! - Die Welle-1-Liste `testdata/i18n/wave1-message-ids.json` (108 IDs, DE/EN) steht wortgleich im Katalog.
//! - Die Welle-2-Liste `docs/integration/wave2-message-ids.json` (274 IDs, DE/EN) steht im Katalog.
//! - IDs in `t!(…, "id")`/`translate(…, "id")` existieren.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fluent_syntax::ast;
use pida_i18n::{ArgValue, BUILTIN_SOURCES, Catalog, Lang, fluent_id_for_java_key};

const WORKSPACE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const WAVE1_IDS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/i18n/wave1-message-ids.json");
const WAVE2_IDS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/integration/wave2-message-ids.json");

/// Literale, die wie Codes aussehen, aber keine Meldungen sind (mit Fundstelle).
const NOT_MESSAGE_CODES: &[(&str, &str)] = &[
    ("atrust-native", "config::certcache – Backend-Kennung im Cache-Schlüssel"),
    ("pida", "config::paths – Ordnername unter Linux"),
];

// --- Quelltexte -----------------------------------------------------------------------------------

fn workspace() -> PathBuf {
    Path::new(WORKSPACE).canonicalize().expect("Workspace-Wurzel")
}

/// Alle `.rs`-Dateien unter `crates/*/src`, `apps/*/src` und `apps/*/src-tauri/src`.
fn source_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() {
                if !matches!(name, "target" | "node_modules" | "gen" | ".git") {
                    walk(&p, out);
                }
            } else if name.ends_with(".rs") {
                out.push(p);
            }
        }
    }
    let root = workspace();
    let mut out = Vec::new();
    for group in ["crates", "apps"] {
        let Ok(members) = std::fs::read_dir(root.join(group)) else { continue };
        let mut members: Vec<_> = members.flatten().map(|e| e.path()).collect();
        members.sort();
        for m in members {
            walk(&m.join("src"), &mut out);
            walk(&m.join("src-tauri").join("src"), &mut out);
        }
    }
    out
}

/// String-Literal im Quelltext.
#[derive(Debug)]
struct Literal {
    /// Byte-Offset des öffnenden Anführungszeichens
    pos: usize,
    value: String,
}

/// Lexer-Ergebnis: Literale und eine „Maske“ des Quelltexts, in der Kommentare und Literale durch
/// Leerzeichen ersetzt sind (Byte-Offsets bleiben gleich).
struct Lexed {
    literals: Vec<Literal>,
    mask: Vec<u8>,
}

fn lex(src: &str) -> Lexed {
    let b = src.as_bytes();
    let mut mask = b.to_vec();
    let mut literals = Vec::new();
    let mut i = 0;
    let blank = |mask: &mut Vec<u8>, from: usize, to: usize| {
        for m in &mut mask[from..to] {
            if *m != b'\n' {
                *m = b' ';
            }
        }
    };
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let end = src[i..].find('\n').map_or(b.len(), |n| i + n);
                blank(&mut mask, i, end);
                i = end;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let (mut depth, mut j) = (1, i + 2);
                while j < b.len() && depth > 0 {
                    if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut mask, i, j);
                i = j;
            }
            b'r' | b'b' | b'c'
                if !(i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'))
                    && raw_string_start(b, i).is_some() =>
            {
                let (quote, hashes) = raw_string_start(b, i).unwrap_or((i, 0));
                let close: Vec<u8> = std::iter::once(b'"').chain(std::iter::repeat_n(b'#', hashes)).collect();
                let body_start = quote + 1;
                let end = find_bytes(&b[body_start..], &close).map_or(b.len(), |n| body_start + n);
                literals.push(Literal { pos: quote, value: src[body_start..end].to_owned() });
                let stop = (end + close.len()).min(b.len());
                blank(&mut mask, i, stop);
                i = stop;
            }
            b'"' => {
                let mut j = i + 1;
                let mut value = Vec::new();
                while j < b.len() && b[j] != b'"' {
                    if b[j] == b'\\' && j + 1 < b.len() {
                        value.push(b[j]);
                        value.push(b[j + 1]);
                        j += 2;
                    } else {
                        value.push(b[j]);
                        j += 1;
                    }
                }
                literals.push(Literal { pos: i, value: String::from_utf8_lossy(&value).into_owned() });
                blank(&mut mask, i, (j + 1).min(b.len()));
                i = j + 1;
            }
            b'\'' => {
                // Zeichenliteral ('x', '\n', '\'', '"') oder Lebensdauer/Label ('a)
                if b.get(i + 1) == Some(&b'\\') {
                    // nach Backslash und maskiertem Zeichen bis zum schließenden '
                    let from = (i + 3).min(b.len());
                    let end = src[from..].find('\'').map_or(b.len(), |n| from + n + 1);
                    blank(&mut mask, i, end);
                    i = end;
                } else if let Some(c) = src[i + 1..].chars().next() {
                    let after = i + 1 + c.len_utf8();
                    if b.get(after) == Some(&b'\'') {
                        blank(&mut mask, i, after + 1);
                        i = after + 1;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    Lexed { literals, mask }
}

/// `r"…"`, `r#"…"#`, `br"…"`, `cr"…"`: Position des `"` und Zahl der `#`.
fn raw_string_start(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if b[j] == b'b' || b[j] == b'c' {
        j += 1;
    }
    if b.get(j) != Some(&b'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while b.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    (b.get(j) == Some(&b'"')).then_some((j, hashes))
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Passende schließende Klammer in der Maske.
fn matching(mask: &[u8], open: usize, (l, r): (u8, u8)) -> usize {
    let mut depth = 0usize;
    for (k, &c) in mask.iter().enumerate().skip(open) {
        if c == l {
            depth += 1;
        } else if c == r {
            depth -= 1;
            if depth == 0 {
                return k;
            }
        }
    }
    mask.len()
}

/// Bereiche ab jedem Vorkommen von `needle` bis zur passenden Klammer `r` der ersten folgenden
/// Klammer `l`. Mit `stop` endet die Suche nach `l` an diesem Zeichen (z. B. `;` einer
/// Trait-Deklaration ohne Rumpf), der Fund entfällt dann.
fn ranges_after(mask: &[u8], needle: &str, (l, r): (u8, u8), stop: Option<u8>) -> Vec<(usize, usize)> {
    let text = String::from_utf8_lossy(mask);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(n) = text[from..].find(needle) {
        let start = from + n;
        if let Some(k) = mask[start..].iter().position(|&c| c == l || Some(c) == stop)
            && mask[start + k] == l
        {
            out.push((start, matching(mask, start + k, (l, r))));
        }
        from = start + needle.len();
    }
    out
}

/// Bereiche von `#[cfg(test)] mod … { … }` und von Attributen `#[…]`/`#![…]`.
fn excluded_ranges(mask: &[u8]) -> Vec<(usize, usize)> {
    let text = String::from_utf8_lossy(mask);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(n) = text[from..].find("#[cfg(test)]") {
        let start = from + n;
        let rest = &text[start + "#[cfg(test)]".len()..];
        let trimmed = rest.trim_start();
        let is_mod =
            trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") || trimmed.starts_with("pub(crate) mod ");
        if is_mod && let Some(open) = mask[start..].iter().position(|&c| c == b'{') {
            out.push((start, matching(mask, start + open, (b'{', b'}'))));
        }
        from = start + 1;
    }
    for prefix in ["#[", "#!["] {
        out.extend(ranges_after(mask, prefix, (b'[', b']'), None));
    }
    out
}

/// Code einer bekannten Familie? (`ng-…`, `cms-…`, …; mit Punkt nur `render.`/`placement.`/künftige).
fn is_family_code(s: &str) -> bool {
    const HYPHEN: &[&str] =
        &["ng", "cms", "keystore", "backend", "fido2", "atrust", "sigblock", "pdf", "pades", "workflow", "core"];
    const DOT: &[&str] = &["render", "placement", "pades", "workflow", "core"];
    let kebab = |rest: &str, sep: char| {
        !rest.is_empty()
            && rest.split(sep).all(|part| {
                !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            })
            && !rest.starts_with('_')
    };
    HYPHEN.iter().any(|f| s.strip_prefix(f).and_then(|r| r.strip_prefix('-')).is_some_and(|r| kebab(r, '-')))
        || DOT.iter().any(|f| s.strip_prefix(f).and_then(|r| r.strip_prefix('.')).is_some_and(|r| kebab(r, '.')))
}

/// Sieht wie eine Meldungs-ID aus (für Literale in `fn code`)?
fn looks_like_id(s: &str) -> bool {
    s.len() > 2
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && (s.contains('-') || s.contains('.'))
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Gefundene Codes mit Fundstellen (`datei:zeile`).
fn codes_in_sources() -> BTreeMap<String, BTreeSet<String>> {
    let root = workspace();
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in source_files() {
        let src = std::fs::read_to_string(&file).expect("Quelltext lesbar");
        let lexed = lex(&src);
        let excluded = excluded_ranges(&lexed.mask);
        let code_fns: Vec<(usize, usize)> = ["fn code(", "fn message_id("]
            .iter()
            .flat_map(|needle| ranges_after(&lexed.mask, needle, (b'{', b'}'), Some(b';')))
            .collect();
        let mask_text = String::from_utf8_lossy(&lexed.mask).into_owned();
        let rel = file.strip_prefix(&root).unwrap_or(&file).display().to_string();
        for lit in &lexed.literals {
            if excluded.iter().any(|&(s, e)| lit.pos >= s && lit.pos <= e) {
                continue;
            }
            let v = lit.value.as_str();
            let in_code_fn = code_fns.iter().any(|&(s, e)| lit.pos >= s && lit.pos <= e) && looks_like_id(v);
            let before = mask_text[..lit.pos].trim_end();
            let notice_const = before.ends_with('=')
                && before.rfind("const NOTICE_").is_some_and(|k| !before[k..].contains(';'))
                && looks_like_id(v);
            if (in_code_fn || notice_const || is_family_code(v)) && !NOT_MESSAGE_CODES.iter().any(|(c, _)| *c == v) {
                let line = src[..lit.pos].matches('\n').count() + 1;
                out.entry(v.to_owned()).or_default().insert(format!("{rel}:{line}"));
            }
        }
    }
    out
}

// --- Katalog ---------------------------------------------------------------------------------------

fn ftl_variables(lang: Lang) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for (l, _, source) in BUILTIN_SOURCES {
        if *l != lang {
            continue;
        }
        let resource = fluent_syntax::parser::parse(*source).expect("gültiges FTL");
        for entry in &resource.body {
            if let ast::Entry::Message(m) = entry {
                let mut vars = BTreeSet::new();
                if let Some(p) = &m.value {
                    pattern_vars(p, &mut vars);
                }
                for a in &m.attributes {
                    pattern_vars(&a.value, &mut vars);
                }
                out.insert(m.id.name.to_owned(), vars);
            }
        }
    }
    out
}

fn pattern_vars(p: &ast::Pattern<&str>, vars: &mut BTreeSet<String>) {
    for el in &p.elements {
        if let ast::PatternElement::Placeable { expression } = el {
            expression_vars(expression, vars);
        }
    }
}

fn expression_vars(e: &ast::Expression<&str>, vars: &mut BTreeSet<String>) {
    match e {
        ast::Expression::Inline(i) => inline_vars(i, vars),
        ast::Expression::Select { selector, variants } => {
            inline_vars(selector, vars);
            for v in variants {
                pattern_vars(&v.value, vars);
            }
        }
    }
}

fn inline_vars(e: &ast::InlineExpression<&str>, vars: &mut BTreeSet<String>) {
    match e {
        ast::InlineExpression::VariableReference { id } => {
            vars.insert(id.name.to_owned());
        }
        ast::InlineExpression::Placeable { expression } => expression_vars(expression, vars),
        ast::InlineExpression::FunctionReference { arguments, .. }
        | ast::InlineExpression::TermReference { arguments: Some(arguments), .. } => {
            for a in &arguments.positional {
                inline_vars(a, vars);
            }
            for a in &arguments.named {
                inline_vars(&a.value, vars);
            }
        }
        _ => {}
    }
}

fn assert_in_both_languages(catalog: &Catalog, id: &str, context: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for lang in Lang::ALL {
        if !catalog.has(lang, id) {
            problems.push(format!("{id} fehlt in {} ({context})", lang.tag()));
        }
    }
    problems
}

// --- Tests -----------------------------------------------------------------------------------------

#[test]
fn lexer_handles_comments_raw_strings_and_char_literals() {
    let src = r####"
// "cms-in-comment"
/* "cms-block" /* nested "cms-nested" */ */
fn f<'a>(x: &'a str) -> char { let _q = '"'; let _e = '\''; '\\' }
const A: &str = r#"cms-raw "quoted""#;
const B: &[u8] = br"backend-bytes";
#[cfg(feature = "fido2-hid")]
fn g() { let _s = "https://example.invalid/x"; let _t = "cms-real"; }
#[cfg(test)]
mod tests { const T: &str = "cms-in-test"; }
"####;
    let lexed = lex(src);
    let values: Vec<&str> = lexed.literals.iter().map(|l| l.value.as_str()).collect();
    assert_eq!(
        values,
        ["cms-raw \"quoted\"", "backend-bytes", "fido2-hid", "https://example.invalid/x", "cms-real", "cms-in-test"]
    );
    let excluded = excluded_ranges(&lexed.mask);
    let kept: Vec<&str> = lexed
        .literals
        .iter()
        .filter(|l| !excluded.iter().any(|&(s, e)| l.pos >= s && l.pos <= e))
        .map(|l| l.value.as_str())
        .collect();
    assert_eq!(kept, ["cms-raw \"quoted\"", "backend-bytes", "https://example.invalid/x", "cms-real"]);
}

#[test]
fn family_codes_are_recognised() {
    for ok in ["cms-der", "backend-keystore-format", "render.library_not_found", "placement.notice.clamped", "ng-x"] {
        assert!(is_family_code(ok), "{ok}");
    }
    for no in ["cms", "cms-", "cms-Der", "render-block", "pdf.foo", "backend-api.rs", "ng--x", "https://cms-x"] {
        assert!(!is_family_code(no), "{no}");
    }
}

/// Kern des Pakets I4/5: jeder im Quelltext gelieferte Code hat DE- und EN-Text.
#[test]
fn every_code_in_sources_is_in_catalog_de_and_en() {
    let catalog = Catalog::builtin().expect("Kataloge gültig");
    let codes = codes_in_sources();
    // Plausibilität des Scanners: Stichproben aus allen Familien müssen gefunden werden.
    for sample in [
        "cms-der",
        "keystore-malformed",
        "backend-network",
        "backend-keystore-format",
        "fido2-unavailable",
        "sigblock-chars-replaced",
        "sigblock-unknown-profile",
        "render.failed",
        "placement.notice.clamped",
        "placement.invalid_position",
        "ng-config-error-io",
        "ng-config-legacy-invalid-bku",
    ] {
        assert!(codes.contains_key(sample), "Scanner findet {sample} nicht");
    }
    assert!(codes.len() >= 100, "nur {} Codes gefunden", codes.len());

    let mut problems = Vec::new();
    for (code, places) in &codes {
        let id = fluent_id_for_java_key(code);
        let context = places.iter().next().map(String::as_str).unwrap_or("?");
        problems.extend(assert_in_both_languages(&catalog, &id, context));
    }
    eprintln!("{} Codes aus dem Quelltext geprüft", codes.len());
    assert!(problems.is_empty(), "fehlende Katalogeinträge:\n{}", problems.join("\n"));
}

#[test]
fn de_and_en_use_the_same_variables() {
    let de = ftl_variables(Lang::De);
    let en = ftl_variables(Lang::En);
    let mut problems = Vec::new();
    for (id, vars) in &de {
        match en.get(id) {
            Some(other) if other != vars => problems.push(format!("{id}: DE {vars:?} ≠ EN {other:?}")),
            None => problems.push(format!("{id}: fehlt in EN")),
            _ => {}
        }
    }
    for id in en.keys().filter(|id| !de.contains_key(*id)) {
        problems.push(format!("{id}: fehlt in DE"));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn notice_and_form_variants_have_codes() {
    let src =
        std::fs::read_to_string(workspace().join("crates/backend-api/src/interaction.rs")).expect("interaction.rs");
    let mut problems = Vec::new();
    for enum_name in ["NoticeCode", "FormProblem"] {
        let variants = enum_variants(&src, enum_name);
        assert!(variants.len() >= 5, "{enum_name}: nur {variants:?}");
        let Some(body) = code_fn_body(&src, enum_name) else {
            problems.push(format!("impl {enum_name} {{ fn code(…) }} fehlt"));
            continue;
        };
        if body.contains("_ =>") {
            problems.push(format!("{enum_name}::code: Platzhalter-Arm `_ =>` – jede Variante einzeln abbilden"));
        }
        for v in &variants {
            if !body.contains(&format!("{enum_name}::{v}")) {
                problems.push(format!("{enum_name}::{v} fehlt in {enum_name}::code"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Rumpf von `fn code` im `impl <name> {`-Block (die Varianten stehen als `Name::Variante` darin).
fn code_fn_body(src: &str, name: &str) -> Option<String> {
    let lexed = lex(src);
    let mask = String::from_utf8_lossy(&lexed.mask).into_owned();
    let start = mask.find(&format!("impl {name} {{"))?;
    let impl_open = start + mask[start..].find('{')?;
    let impl_close = matching(&lexed.mask, impl_open, (b'{', b'}'));
    let fn_pos = impl_open + mask[impl_open..impl_close].find("fn code(")?;
    let body_open = fn_pos + mask[fn_pos..].find('{')?;
    let body_close = matching(&lexed.mask, body_open, (b'{', b'}'));
    Some(src[body_open..=body_close.min(src.len() - 1)].to_owned())
}

/// Varianten eines (rustfmt-formatierten) Enums: Bezeichner mit genau vier Leerzeichen Einzug.
fn enum_variants(src: &str, name: &str) -> Vec<String> {
    let start = src.find(&format!("pub enum {name} {{")).unwrap_or_else(|| panic!("enum {name} nicht gefunden"));
    let mut out = Vec::new();
    for line in src[start..].lines().skip(1) {
        if line.starts_with('}') {
            break;
        }
        let Some(rest) = line.strip_prefix("    ") else { continue };
        if rest.starts_with(' ') || rest.starts_with("//") || rest.starts_with('#') {
            continue;
        }
        let ident: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if ident.starts_with(|c: char| c.is_ascii_uppercase()) {
            out.push(ident);
        }
    }
    out
}

#[test]
fn wave1_message_ids_are_in_catalog_verbatim() {
    #[derive(serde::Deserialize)]
    struct Entry {
        id: String,
        de: String,
        en: String,
    }
    let entries: Vec<Entry> =
        serde_json::from_str(&std::fs::read_to_string(WAVE1_IDS).expect("wave1-message-ids.json")).expect("JSON");
    assert_eq!(entries.len(), 108, "Welle-1-Liste");
    let catalog = Catalog::builtin().unwrap();
    let mut problems = Vec::new();
    for e in &entries {
        let id = fluent_id_for_java_key(&e.id);
        for (lang, text) in [(Lang::De, &e.de), (Lang::En, &e.en)] {
            if !catalog.has(lang, &id) {
                problems.push(format!("{id} fehlt in {}", lang.tag()));
                continue;
            }
            // Platzhalter `{x}` bzw. `{ $x }` durch `<x>` ersetzen und Text vergleichen.
            let (expected, names) = substitute_placeholders(text);
            let args: Vec<(&str, ArgValue)> =
                names.iter().map(|n| (n.as_str(), ArgValue::Str(format!("<{n}>")))).collect();
            let actual = catalog.format(lang, &id, &args);
            if actual != expected {
                problems.push(format!("{id} ({}): {actual:?} ≠ {expected:?}", lang.tag()));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Die Kopie unter `testdata/i18n` entspricht dem Original `docs/integration/wave1-message-ids.json`,
/// solange es existiert (sonst prüft der Katalogtest eine veraltete Liste).
#[test]
fn wave1_copy_matches_integration_docs() {
    let original = workspace().join("docs/integration/wave1-message-ids.json");
    let Ok(original) = std::fs::read_to_string(&original) else {
        eprintln!("docs/integration/wave1-message-ids.json fehlt – Abgleich übersprungen");
        return;
    };
    let copy = std::fs::read_to_string(WAVE1_IDS).expect("testdata/i18n/wave1-message-ids.json");
    let original: serde_json::Value = serde_json::from_str(&original).expect("JSON (docs)");
    let copy: serde_json::Value = serde_json::from_str(&copy).expect("JSON (testdata)");
    assert!(
        original == copy,
        "testdata/i18n/wave1-message-ids.json weicht von docs/integration/wave1-message-ids.json ab – Kopie erneuern"
    );
}

fn substitute_placeholders(text: &str) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('}').map(|c| open + c).expect("geschlossener Platzhalter");
        let name = rest[open + 1..close].trim().trim_start_matches('$').trim().to_owned();
        out.push_str(&format!("<{name}>"));
        if !names.contains(&name) {
            names.push(name);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    (out, names)
}

/// Literale `id` aus Aufrufen `t!(<sprache>, "id" …)` und `translate(<sprache>, "id" …)`
/// (Position, ID). Erkannt wird ein Literal als zweites Argument direkt nach einem einfachen ersten
/// Argument.
fn translate_call_ids(src: &str) -> Vec<(usize, String)> {
    let lexed = lex(src);
    let mask = String::from_utf8_lossy(&lexed.mask).into_owned();
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = Vec::new();
    for lit in &lexed.literals {
        let Some(before) = mask[..lit.pos].trim_end().strip_suffix(',') else { continue };
        let Some(open) = before.rfind('(') else { continue };
        let callee = before[..open].trim_end();
        let is_call = ["t!", "translate"]
            .iter()
            .any(|name| callee.strip_suffix(name).is_some_and(|prefix| !prefix.ends_with(ident)));
        let simple_first_arg = !before[open + 1..].contains([',', '(', ')', ';', '{', '}']);
        if is_call && simple_first_arg {
            out.push((lit.pos, lit.value.clone()));
        }
    }
    out
}

#[test]
fn translate_call_matcher() {
    let src = r#"
        let a = t!(Lang::De, "output-success_message");
        let b = pida_i18n::translate(lang, "error-SLError", &[]);
        let c = pida_i18n::t!(lang, "ng-config-file-missing", path = p);
        assert!(x, "kein Aufruf");
        sprint!(lang, "auch kein Aufruf");
        let d = t!(f(lang), "verschachtelt-ignoriert");
    "#;
    let ids: Vec<String> = translate_call_ids(src).into_iter().map(|(_, id)| id).collect();
    assert_eq!(ids, ["output-success_message", "error-SLError", "ng-config-file-missing"]);
}

#[test]
fn translate_calls_use_existing_ids() {
    let catalog = Catalog::builtin().unwrap();
    let root = workspace();
    let mut problems = Vec::new();
    for file in source_files() {
        let src = std::fs::read_to_string(&file).unwrap();
        for (pos, id) in translate_call_ids(&src) {
            let rel = file.strip_prefix(&root).unwrap_or(&file).display().to_string();
            let line = src[..pos].matches('\n').count() + 1;
            problems.extend(assert_in_both_languages(&catalog, &id, &format!("{rel}:{line}")));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Später bewusst entfernte IDs der Welle-2-Liste (die Liste bleibt als Bericht unverändert), entfernt am
/// 15.10.2026: der Abschnitt „Position“ der Positionierung (manuelle Koordinaten in mm/pt, Seitenfeld) und
/// der blockierende Datenschutzdialog samt CLI-Rückfrage (ADR 0003); am 17.10.2026 der Wortlaut „gültig“/„ungültig“
/// von `pida verify` (jetzt `ng-cli-verify-*`, ADR 0004 Nachtrag 15). Sie dürfen nicht mehr im Katalog stehen.
const RETIRED_WAVE2_IDS: &[&str] = &[
    "ng-ui-unit",
    "ng-ui-position-coordinates",
    "ng-ui-position-x",
    "ng-ui-position-y",
    "ng-ui-position-page-field",
    "ng-ui-privacy-lead",
    "ng-ui-privacy-accept",
    "ng-ui-privacy-decline",
    "ng-cli-privacy",
    "ng-cli-valid",
    "ng-cli-invalid",
];

/// Welle 2 (Integration, 274 IDs): jede ID steht in DE und EN mit den Variablen des Berichts (die
/// Texte dürfen nach Review angepasst sein); ausgenommen [`RETIRED_WAVE2_IDS`].
#[test]
fn wave2_message_ids_are_in_catalog() {
    #[derive(serde::Deserialize)]
    struct Entry {
        id: String,
        de: String,
        en: String,
    }
    let Ok(text) = std::fs::read_to_string(WAVE2_IDS) else {
        eprintln!("docs/integration/wave2-message-ids.json fehlt – übersprungen");
        return;
    };
    let entries: Vec<Entry> = serde_json::from_str(&text).expect("JSON");
    assert!(entries.len() >= 274, "Welle-2-Liste: {}", entries.len());
    let catalog = Catalog::builtin().unwrap();
    let vars = [(Lang::De, ftl_variables(Lang::De)), (Lang::En, ftl_variables(Lang::En))];
    let mut problems = Vec::new();
    for e in &entries {
        let id = fluent_id_for_java_key(&e.id);
        if RETIRED_WAVE2_IDS.contains(&e.id.as_str()) {
            for lang in [Lang::De, Lang::En] {
                if catalog.has(lang, &id) {
                    problems.push(format!("{id} ist entfernt, steht aber noch in {}", lang.tag()));
                }
            }
            continue;
        }
        for ((lang, catalog_vars), text) in vars.iter().zip([&e.de, &e.en]) {
            if !catalog.has(*lang, &id) {
                problems.push(format!("{id} fehlt in {}", lang.tag()));
                continue;
            }
            let (_, names) = substitute_placeholders(text);
            let expected: BTreeSet<String> = names.into_iter().collect();
            let actual = catalog_vars.get(&id).cloned().unwrap_or_default();
            if expected != actual {
                problems.push(format!("{id} ({}): Variablen {actual:?} ≠ {expected:?}", lang.tag()));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
