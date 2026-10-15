//! Konverter `messages*.properties` → Fluent (Plan §7.5, docs/analyse/01 §6.3).
//!
//! Regeln:
//! 1. Java-Properties-Syntax über `pida-java-properties` (Escapes `\:`, `\!`, `\n`, `\\`, `\uXXXX`).
//!    Die Bytes werden wie von `PropertyResourceBundle` (Java ≥ 9) gelesen: UTF-8, bei ungültigem UTF-8
//!    ISO-8859-1.
//! 2. `&`-Mnemonic entfernen und als Attribut `.accesskey` ablegen (`&&` = literales `&`; ein `&` vor
//!    einem Nicht-Buchstaben/-Ziffer bleibt literal und erzeugt eine Warnung).
//! 3. `%s`/`%d` in Reihenfolge zu benannten Argumenten `{ $arg1 }`, `{ $arg2 }` … (EN und DE haben
//!    dieselbe Reihenfolge, [`compare_catalogs`]); `%%` wird zu `%`.
//! 4. `<a>…</a>` und `<a href="…">…</a>` bleiben als Markup-Token im Text; die UI rendert sie als Link,
//!    die CLI über [`crate::markup::to_plain`] als Klartext. Konvention:
//!    - `<a>Text</a>`: Ziel ist der Text selbst (URL) bzw. ein vom Bildschirm festgelegtes Ziel
//!      (z. B. `config-fido2-InfoURL`, `config-DataProtectionURL`, Update-Aktion).
//!    - `<a href="Ziel">Text</a>`: explizites Ziel (z. B. `mailto:`).
//! 5. Mehrzeilige Werte (`\n`) als Fluent-Mehrzeiler (Blockform, 4 Leerzeichen Einzug). Führende bzw.
//!    nachgestellte Leerzeichen einer Zeile, leere erste/letzte Zeilen, `{`/`}`, Steuerzeichen und die
//!    in Blockzeilen reservierten Anfangszeichen `[`, `*`, `.` werden als String-Literal `{"…"}`
//!    geschrieben (benachbarte Zeichen in **einem** Literal); ein leerer Wert wird zu `{ "" }`.
//!    `fluent-bundle` löst je Nachricht höchstens [`FLUENT_MAX_PLACEABLES`] Placeables auf; braucht ein
//!    Wert mehr, meldet der Konverter eine Warnung (der Katalog-Test verlangt null Warnungen).
//! 6. B-09: das DE-Duplikat `error.Initialization` wird zu `error.ConfigInitialization` korrigiert;
//!    fehlende Keys werden ergänzt, ungenutzte markiert ([`convert_pdfover_messages`]).
//!
//! Die Fluent-ID ist der Java-Key nach [`crate::fluent_id_for_java_key`] (`.` → `-`); ist die
//! Abbildung nicht umkehrbar, steht der Java-Key als Kommentar über der Nachricht.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use fluent_syntax::ast;
use pida_java_properties::Properties;

use crate::{Lang, fluent_id_for_java_key};

/// Ergebnis einer Konvertierung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Conversion {
    /// Fertiger FTL-Text.
    pub ftl: String,
    /// Anzahl Nachrichten.
    pub messages: usize,
    /// Warnungen (z. B. nicht konvertierbare Platzhalter).
    pub warnings: Vec<String>,
}

/// Optionen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    /// Keys, die als ungenutzt markiert werden (Kommentar `# unused`).
    pub unused_keys: Vec<String>,
    /// Kopfkommentar (Lizenz/Herkunft: EUPL, PDF-Over), als Ressourcenkommentar `###`.
    pub header: Option<String>,
    /// Zusätzliche Kommentare je Java-Key (z. B. Herkunft einer Korrektur); mehrere je Key möglich.
    pub key_comments: Vec<(String, String)>,
}

/// Fehler.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// Properties nicht lesbar.
    #[error("Properties nicht lesbar: {0}")]
    Properties(String),
    /// Erzeugtes FTL ist ungültig.
    #[error("erzeugtes FTL ungültig: {0}")]
    InvalidFtl(String),
    /// Zwei Java-Keys ergeben dieselbe Fluent-ID.
    #[error("Fluent-ID {0} entsteht aus mehreren Java-Keys")]
    DuplicateId(String),
}

/// Konvertiert eine Properties-Map nach Fluent (Reihenfolge = Einfügereihenfolge der Map).
pub fn properties_to_ftl(props: &Properties, opts: &ConvertOptions) -> Result<Conversion, ConvertError> {
    let mut out = String::new();
    let mut warnings = Vec::new();
    if let Some(header) = &opts.header {
        for line in header.lines() {
            if line.is_empty() {
                out.push_str("###\n");
            } else {
                let _ = writeln!(out, "### {line}");
            }
        }
        out.push('\n');
    }
    let unused: BTreeSet<&str> = opts.unused_keys.iter().map(String::as_str).collect();
    let mut comments: HashMap<&str, Vec<&str>> = HashMap::new();
    for (k, c) in &opts.key_comments {
        comments.entry(k.as_str()).or_default().push(c.as_str());
    }
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut messages = 0usize;
    for (key, value) in props.iter() {
        let id = fluent_id_for_java_key(key);
        if let Some(previous) = ids.insert(id.clone(), key.to_owned()) {
            return Err(ConvertError::DuplicateId(format!("{id} ({previous}, {key})")));
        }
        if id.replace('-', ".") != key {
            let _ = writeln!(out, "# Java-Key: {}", single_line(key));
        }
        if unused.contains(key) {
            out.push_str("# unused (in PDF-Over 4.4.x nicht referenziert)\n");
        }
        for c in comments.get(key).into_iter().flatten() {
            for line in c.lines() {
                let _ = writeln!(out, "# {line}");
            }
        }
        let converted = convert_value_detailed(value);
        for w in converted.warnings {
            warnings.push(format!("{key}: {w}"));
        }
        let lines: Vec<&str> = converted.pattern.split('\n').collect();
        if lines.len() == 1 {
            let _ = writeln!(out, "{id} = {}", lines[0]);
        } else {
            let _ = writeln!(out, "{id} =");
            for line in lines {
                if line.is_empty() {
                    out.push('\n');
                } else {
                    let _ = writeln!(out, "    {line}");
                }
            }
        }
        if let Some(c) = converted.access_key {
            let _ = writeln!(out, "    .accesskey = {c}");
        }
        messages += 1;
    }
    if let Err((_, errors)) = fluent_syntax::parser::parse(out.as_str()) {
        let detail: Vec<String> = errors.iter().map(|e| format!("{:?}", e.kind)).collect();
        return Err(ConvertError::InvalidFtl(detail.join("; ")));
    }
    Ok(Conversion { ftl: out, messages, warnings })
}

fn single_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

/// Ein einzelner Wert: liefert (Fluent-Pattern-Quelltext ohne Einrückung, mehrere Zeilen durch `\n`
/// getrennt; Access-Key).
pub fn convert_value(java_value: &str) -> (String, Option<char>) {
    let c = convert_value_detailed(java_value);
    (c.pattern, c.access_key)
}

/// Platzhalterart eines Java-Werts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Placeholder {
    /// `%s`
    Str,
    /// `%d`
    Int,
}

struct ConvertedValue {
    pattern: String,
    access_key: Option<char>,
    warnings: Vec<String>,
}

enum Piece {
    Text(String),
    Var(usize),
}

/// Zerlegt einen Java-Wert in Text und Platzhalter (Mnemonics entfernt).
fn split_java_value(java_value: &str) -> (Vec<Piece>, Option<char>, Vec<Placeholder>, Vec<String>) {
    let chars: Vec<char> = java_value.chars().collect();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut text = String::new();
    let mut access_key = None;
    let mut placeholders = Vec::new();
    let mut warnings = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '&' => match chars.get(i + 1) {
                Some('&') => {
                    text.push('&');
                    i += 2;
                    continue;
                }
                Some(&n) if n.is_alphanumeric() => {
                    if access_key.is_none() {
                        access_key = Some(n);
                    } else {
                        warnings.push(format!("weitere Mnemonic vor '{n}' entfernt"));
                    }
                    i += 1;
                    continue;
                }
                _ => {
                    warnings.push("'&' ohne Buchstaben bleibt literal".to_owned());
                    text.push('&');
                }
            },
            '%' => match chars.get(i + 1) {
                Some('%') => {
                    text.push('%');
                    i += 2;
                    continue;
                }
                Some(&kind @ ('s' | 'd')) => {
                    if !text.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut text)));
                    }
                    placeholders.push(if kind == 's' { Placeholder::Str } else { Placeholder::Int });
                    pieces.push(Piece::Var(placeholders.len()));
                    i += 2;
                    continue;
                }
                other => {
                    warnings.push(format!("Format-Angabe '%{}' bleibt literal", other.copied().unwrap_or(' ')));
                    text.push('%');
                }
            },
            _ => text.push(c),
        }
        i += 1;
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    (pieces, access_key, placeholders, warnings)
}

fn convert_value_detailed(java_value: &str) -> ConvertedValue {
    let (pieces, access_key, _, warnings) = split_java_value(java_value);
    if pieces.is_empty() {
        return ConvertedValue { pattern: "{ \"\" }".to_owned(), access_key, warnings };
    }
    // In Zeilen aufteilen: jede Zeile ist eine Folge aus Text und Variablen.
    let mut lines: Vec<Vec<Piece>> = vec![Vec::new()];
    for piece in pieces {
        match piece {
            Piece::Var(n) => lines.last_mut().expect("mindestens eine Zeile").push(Piece::Var(n)),
            Piece::Text(t) => {
                let mut first = true;
                for part in t.split('\n') {
                    if !first {
                        lines.push(Vec::new());
                    }
                    first = false;
                    if !part.is_empty() {
                        lines.last_mut().expect("mindestens eine Zeile").push(Piece::Text(part.to_owned()));
                    }
                }
            }
        }
    }
    let block = lines.len() > 1;
    let last = lines.len() - 1;
    let mut rendered = Vec::with_capacity(lines.len());
    let mut placeables = 0usize;
    for (idx, line) in lines.iter().enumerate() {
        if line.is_empty() {
            // Leere erste/letzte Zeilen würde Fluent abschneiden; mittlere bleiben erhalten.
            if idx == 0 || idx == last {
                rendered.push("{ \"\" }".to_owned());
                placeables += 1;
            } else {
                rendered.push(String::new());
            }
            continue;
        }
        let (text, n) = render_line(line, block);
        rendered.push(text);
        placeables += n;
    }
    let mut warnings = warnings;
    if placeables > FLUENT_MAX_PLACEABLES {
        warnings.push(format!(
            "{placeables} Placeables (Variablen, maskierte Zeichen) – Fluent gibt nur die ersten {FLUENT_MAX_PLACEABLES} aus"
        ));
    }
    ConvertedValue { pattern: rendered.join("\n"), access_key, warnings }
}

/// Höchstzahl der Placeables, die `fluent-bundle` je Nachricht auflöst (`MAX_PLACEABLES`); darüber wird
/// der Text abgeschnitten.
pub const FLUENT_MAX_PLACEABLES: usize = 100;

/// Rendert eine Zeile; liefert den Quelltext und die Anzahl der Placeables (Variablen und
/// String-Literale). Aufeinanderfolgende Zeichen, die ein Literal brauchen, teilen sich eines.
fn render_line(line: &[Piece], block: bool) -> (String, usize) {
    let mut out = String::new();
    let mut placeables = 0usize;
    let mut literal = String::new();
    let flush = |literal: &mut String, out: &mut String, placeables: &mut usize| {
        if !literal.is_empty() {
            out.push_str(&string_literal(literal));
            literal.clear();
            *placeables += 1;
        }
    };
    let n = line.len();
    for (idx, piece) in line.iter().enumerate() {
        match piece {
            Piece::Var(v) => {
                flush(&mut literal, &mut out, &mut placeables);
                let _ = write!(out, "{{ $arg{v} }}");
                placeables += 1;
            }
            Piece::Text(t) => {
                let mut t: &str = t;
                // führende Leerzeichen der Zeile
                if idx == 0 {
                    let trimmed = t.trim_start_matches(' ');
                    if trimmed.len() != t.len() {
                        literal.push_str(&t[..t.len() - trimmed.len()]);
                        t = trimmed;
                    } else if block && t.starts_with(['[', '*', '.']) {
                        literal.push_str(&t[..1]);
                        t = &t[1..];
                    }
                }
                // nachgestellte Leerzeichen der Zeile
                let mut trailing = "";
                if idx == n - 1 {
                    let trimmed = t.trim_end_matches(' ');
                    trailing = &t[trimmed.len()..];
                    t = trimmed;
                }
                for c in t.chars() {
                    match c {
                        '{' | '}' => literal.push(c),
                        '\t' => {
                            flush(&mut literal, &mut out, &mut placeables);
                            out.push('\t');
                        }
                        c if c.is_control() => literal.push(c),
                        c => {
                            flush(&mut literal, &mut out, &mut placeables);
                            out.push(c);
                        }
                    }
                }
                literal.push_str(trailing);
            }
        }
    }
    flush(&mut literal, &mut out, &mut placeables);
    (out, placeables)
}

/// Fluent-String-Literal als Placeable (`{"…"}`).
fn string_literal(s: &str) -> String {
    let mut out = String::from("{\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push_str("\"}");
    out
}

/// Platzhalter eines Java-Werts in Reihenfolge (`%s`/`%d`).
pub fn java_placeholders(java_value: &str) -> Vec<Placeholder> {
    split_java_value(java_value).2
}

/// Vergleicht zwei Sprachstände (fehlende/überzählige Keys, Platzhalter-Konsistenz).
/// Leere Liste = konsistent.
pub fn compare_catalogs(base: &Properties, other: &Properties) -> Vec<String> {
    let mut problems = Vec::new();
    for (key, value) in base.iter() {
        match other.get(key) {
            None => problems.push(format!("fehlt: {key}")),
            Some(o) => {
                let a = java_placeholders(value);
                let b = java_placeholders(o);
                if a != b {
                    problems.push(format!("Platzhalter verschieden: {key} ({a:?} ≠ {b:?})"));
                }
            }
        }
    }
    for key in other.keys() {
        if !base.contains_key(key) {
            problems.push(format!("überzählig: {key}"));
        }
    }
    problems.sort();
    problems
}

/// Variablen (`$name`) und Attribute je Nachricht eines FTL-Texts.
fn ftl_signatures(ftl: &str) -> Result<BTreeMap<String, BTreeSet<String>>, ConvertError> {
    let resource = fluent_syntax::parser::parse(ftl).map_err(|(_, errors)| {
        ConvertError::InvalidFtl(errors.iter().map(|e| format!("{:?}", e.kind)).collect::<Vec<_>>().join("; "))
    })?;
    let mut out = BTreeMap::new();
    for entry in &resource.body {
        if let ast::Entry::Message(m) = entry {
            let mut vars = BTreeSet::new();
            if let Some(p) = &m.value {
                collect_pattern_vars(p, &mut vars);
            }
            for a in &m.attributes {
                collect_pattern_vars(&a.value, &mut vars);
            }
            if out.insert(m.id.name.to_owned(), vars).is_some() {
                return Err(ConvertError::DuplicateId(m.id.name.to_owned()));
            }
        }
    }
    Ok(out)
}

pub(crate) fn collect_pattern_vars(p: &ast::Pattern<&str>, vars: &mut BTreeSet<String>) {
    for el in &p.elements {
        if let ast::PatternElement::Placeable { expression } = el {
            collect_expression_vars(expression, vars);
        }
    }
}

fn collect_expression_vars(e: &ast::Expression<&str>, vars: &mut BTreeSet<String>) {
    match e {
        ast::Expression::Inline(i) => collect_inline_vars(i, vars),
        ast::Expression::Select { selector, variants } => {
            collect_inline_vars(selector, vars);
            for v in variants {
                collect_pattern_vars(&v.value, vars);
            }
        }
    }
}

fn collect_inline_vars(e: &ast::InlineExpression<&str>, vars: &mut BTreeSet<String>) {
    match e {
        ast::InlineExpression::VariableReference { id } => {
            vars.insert(id.name.to_owned());
        }
        ast::InlineExpression::Placeable { expression } => collect_expression_vars(expression, vars),
        ast::InlineExpression::FunctionReference { arguments, .. }
        | ast::InlineExpression::TermReference { arguments: Some(arguments), .. } => {
            for a in &arguments.positional {
                collect_inline_vars(a, vars);
            }
            for a in &arguments.named {
                collect_inline_vars(&a.value, vars);
            }
        }
        _ => {}
    }
}

/// Vergleicht zwei FTL-Kataloge: fehlende/überzählige Nachrichten und abweichende Variablen.
/// Attribute (`.accesskey`) dürfen sich unterscheiden (nicht jede Übersetzung hat eine Mnemonic).
pub fn compare_ftl(base: &str, other: &str) -> Result<Vec<String>, ConvertError> {
    let a = ftl_signatures(base)?;
    let b = ftl_signatures(other)?;
    let mut problems = Vec::new();
    for (id, vars) in &a {
        match b.get(id) {
            None => problems.push(format!("fehlt: {id}")),
            Some(o) if o != vars => problems.push(format!("Variablen verschieden: {id} ({vars:?} ≠ {o:?})")),
            Some(_) => {}
        }
    }
    for id in b.keys() {
        if !a.contains_key(id) {
            problems.push(format!("überzählig: {id}"));
        }
    }
    Ok(problems)
}

// --- PDF-Over-spezifische Aufbereitung -------------------------------------------------------------

/// Keys, die PDF-Over 4.4.x nicht referenziert (Literal-Suche im Java-Code plus dynamisch gebildete
/// Keys `BKU.*`, `simple_config.<Profil>`, `config.UpdateStatus.*`; docs/analyse/01 §6.2). Sie bleiben im
/// Katalog (PIDA nutzt einige davon, z. B. `tanEnter.tries`), werden aber markiert.
pub const PDFOVER_UNUSED_KEYS: &[&str] = &[
    "BKUSelectionComposite.btnMobile.text",
    "PlaceholderSelectionGui.lblNewLabel.text",
    "PlaceholderSelectionGui.shlInfo.text",
    "TrustedSocketFactory.FailedToCreateSecureConnection",
    "advanced_config.SigPHTransparency",
    "advanced_config.SigPHTransparencyMax",
    "advanced_config.SigPHTransparencyMin",
    "advanced_config.SigPHTransparencyTooltip",
    "advanced_config.UpdateCheck_Title",
    "error.Details",
    "error.FailedToGetSignedDocument",
    "error.InvalidLocale",
    "error.InvalidSettings",
    "error.PDFProtected",
    "error.Retry",
    "error.SWTLib",
    "error.SaveOutputFolder",
    "error.TitleFatal",
    "keystore_config.KeystoreFile.Dialog",
    "keystore_config.KeystoreFile.Dialog_Title",
    "mobileBKU.notice",
    "mobileBKU.tan_tries_exceeded",
    "mobileBKU.wrong_tan",
    "positioning.signature",
    "simple_config.Note_Default_Empty",
    "tanEnter.tries",
    "tanEnter.try",
    "waiting_for_app.message",
    "tanEnter.APPTAN",
];

/// Doppelte Keys, deren **zweites** Vorkommen umbenannt wird, wenn der Ziel-Key fehlt (B-09).
const DUPLICATE_RENAMES: &[(&str, &str, &str)] = &[(
    "error.Initialization",
    "error.ConfigInitialization",
    "korrigiert (B-09): zweites Vorkommen von error.Initialization in messages_de.properties",
)];

/// Korrekturen an vorhandenen Werten.
enum Fix {
    /// Werte zweier Keys tauschen, wenn `detect(a_value)` zutrifft.
    Swap { a: &'static str, b: &'static str, detect: &'static str },
    /// Teilstring ersetzen.
    Replace { key: &'static str, from: &'static str, to: &'static str },
    /// Wert ersetzen.
    Set { key: &'static str, value: &'static str },
}

struct Correction {
    lang: Lang,
    fix: Fix,
    reason: &'static str,
}

const CORRECTIONS: &[Correction] = &[
    Correction {
        lang: Lang::En,
        fix: Fix::Swap {
            a: "argument.help.keystorestorepass",
            b: "argument.help.keystorekeypass",
            detect: "-kskeypass",
        },
        reason: "korrigiert (B-08): Hilfetexte -kspass/-kskeypass waren vertauscht",
    },
    Correction {
        lang: Lang::En,
        fix: Fix::Set {
            key: "argument.help.vis",
            value: "Makes the signature invisible (the values false, 0, no, nein or off keep it visible). Example: -v true",
        },
        reason: "korrigiert (B-07): -v wertet seinen Wert aus (ADR 0001 §8.9)",
    },
    Correction {
        lang: Lang::De,
        fix: Fix::Replace {
            key: "error.Initialization",
            from: "Wir bittem un Verzeihung",
            to: "Wir bitten um Verzeihung",
        },
        reason: "korrigiert: Tippfehler „bittem un“ (docs/analyse/01 §6.2)",
    },
    Correction {
        lang: Lang::De,
        fix: Fix::Replace {
            key: "error.ConfigInitialization",
            from: "Wir bittem un Verzeihung",
            to: "Wir bitten um Verzeihung",
        },
        reason: "korrigiert: Tippfehler „bittem un“ (docs/analyse/01 §6.2)",
    },
    // Terminologie der Oberfläche von PIDA: „Chipkarte“ und „Chipkarten-Software“ statt „Bürgerkarte“ und
    // „Bürgerkartenumgebung“ (EN „smart card“, „smart card software“); URLs wie www.buergerkarte.at bleiben.
    Correction {
        lang: Lang::De,
        fix: Fix::Replace { key: "BKU.LOCAL", from: "Lokale Bürgerkarte", to: "Lokale Chipkarte" },
        reason: TERM_DE,
    },
    Correction {
        lang: Lang::De,
        fix: Fix::Replace {
            key: "config.DataProtectionStatement",
            from: "mit der Bürgerkarte",
            to: "mit der Chipkarte",
        },
        reason: TERM_DE,
    },
    Correction {
        lang: Lang::De,
        fix: Fix::Replace {
            key: "error.LocalBKU",
            from: "lokale Bürgerkartenumgebung (BKU) läuft\n\nSie benötigen eine BKU, um auf Ihre Bürgerkarte zuzugreifen.",
            to: "lokale Chipkarten-Software (BKU) läuft\n\nSie benötigen eine BKU, um auf Ihre Chipkarte zuzugreifen.",
        },
        reason: TERM_DE,
    },
    Correction {
        lang: Lang::En,
        fix: Fix::Replace { key: "BKU.LOCAL", from: "Local Citizen Card", to: "Local Smart Card" },
        reason: TERM_EN,
    },
    Correction {
        lang: Lang::En,
        fix: Fix::Replace {
            key: "config.DataProtectionStatement",
            from: "with the citizen card",
            to: "with the smart card",
        },
        reason: TERM_EN,
    },
    Correction {
        lang: Lang::En,
        fix: Fix::Replace {
            key: "error.LocalBKU",
            from: "a local CCE (citizen card environment) is running\n\nYou need a CCE to access your citizen card.",
            to: "a local CCE (smart card software) is running\n\nYou need a CCE to access your smart card.",
        },
        reason: TERM_EN,
    },
];

/// Kommentar der Terminologie-Korrekturen im erzeugten Katalog (DE; ohne die alten Begriffe, damit eine
/// Suche nach ihnen in den Katalogen leer bleibt).
const TERM_DE: &str = "angepasst (PIDA): Terminologie der Oberfläche („Chipkarte“, „Chipkarten-Software“)";
/// Kommentar der Terminologie-Korrekturen im erzeugten Katalog (EN).
const TERM_EN: &str = "angepasst (PIDA): Terminologie der Oberfläche („smart card“, „smart card software“)";

/// Ergänzungen fehlender Keys (Sprachstände vollständig, docs/analyse/01 §6.2 „nur in dieser Datei“).
const SUPPLEMENTS: &[(Lang, &str, &str)] = &[
    (Lang::En, "tanEnter.APPTAN", "Please open the ID Austria app\nTAN:"),
    (Lang::De, "PlaceholderSelectionGui.lblNewLabel.text", "Neue Beschriftung"),
    (Lang::De, "PlaceholderSelectionGui.shlInfo.text", "Info"),
    (
        Lang::De,
        "argument.help.vis",
        "Macht die Signatur unsichtbar (die Werte false, 0, no, nein oder off lassen sie sichtbar). Bsp.: -v true",
    ),
    (Lang::De, "argument.invalid.vis", "Sichtbarkeits-Argument ungültig! Verwendung:"),
];

/// Dekodiert eine `.properties`-Datei wie `PropertyResourceBundle` (Java ≥ 9): UTF-8, sonst ISO-8859-1.
pub fn decode_resource_bundle(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// Physische Zeilen samt Zeilenende; Zeilenenden wie `Properties.LineReader`: `\n`, `\r\n` **und**
/// ein einzelnes `\r`.
fn physical_lines(text: &str) -> impl Iterator<Item = &str> {
    let bytes = text.as_bytes();
    let mut start = 0usize;
    std::iter::from_fn(move || {
        if start >= bytes.len() {
            return None;
        }
        let mut i = start;
        while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }
        if i < bytes.len() {
            i += if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
        }
        // Grenzen liegen auf ASCII-Bytes (`\r`/`\n`) bzw. am Ende, also auf Zeichengrenzen.
        let line = &text[start..i];
        start = i;
        Some(line)
    })
}

/// Alle Einträge in Dateireihenfolge **einschließlich Duplikaten** (Java selbst behält nur den
/// letzten Wert). Jede logische Zeile wird mit dem Java-kompatiblen Parser gelesen.
pub fn raw_entries(text: &str) -> Result<Vec<(String, String)>, ConvertError> {
    let mut out = Vec::new();
    let mut logical = String::new();
    let mut continuing = false;
    for line in physical_lines(text) {
        let content = line.trim_end_matches(['\n', '\r']);
        if !continuing {
            let start = content.trim_start_matches([' ', '\t', '\u{0C}']);
            if start.is_empty() || start.starts_with(['#', '!']) {
                continue;
            }
        }
        logical.push_str(line);
        let backslashes = content.chars().rev().take_while(|&c| c == '\\').count();
        continuing = backslashes % 2 == 1;
        if !continuing {
            push_logical(&mut logical, &mut out)?;
        }
    }
    if !logical.is_empty() {
        push_logical(&mut logical, &mut out)?;
    }
    Ok(out)
}

fn push_logical(logical: &mut String, out: &mut Vec<(String, String)>) -> Result<(), ConvertError> {
    let props = Properties::load_str(logical).map_err(|e| ConvertError::Properties(e.to_string()))?;
    out.extend(props.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
    logical.clear();
    Ok(())
}

/// Aufbereitete Nachrichten einer Sprache ([`prepare_pdfover_messages`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedMessages {
    /// Nachrichten nach Korrektur und Ergänzung (Reihenfolge der Quelldatei, Ergänzungen am Ende).
    pub props: Properties,
    /// Kommentare je Java-Key (Herkunft von Korrekturen und Ergänzungen).
    pub key_comments: Vec<(String, String)>,
    /// Warnungen (z. B. doppelte Keys ohne Korrekturregel).
    pub warnings: Vec<String>,
}

/// Bereitet `messages*.properties` von PDF-Over für eine Sprache auf (Duplikate, Korrekturen,
/// Ergänzungen) und liefert die Properties samt Kommentaren je Key.
pub fn prepare_pdfover_messages(bytes: &[u8], lang: Lang) -> Result<PreparedMessages, ConvertError> {
    let text = decode_resource_bundle(bytes);
    let entries = raw_entries(&text)?;
    let all_keys: BTreeSet<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    let mut props = Properties::new();
    let mut comments: Vec<(String, String)> = Vec::new();
    let mut warnings = Vec::new();
    for (key, value) in &entries {
        if props.contains_key(key) {
            if let Some((_, target, reason)) =
                DUPLICATE_RENAMES.iter().find(|(dup, target, _)| *dup == key && !all_keys.contains(target))
                && !props.contains_key(target)
            {
                props.insert(*target, value.as_str());
                comments.push(((*target).to_owned(), (*reason).to_owned()));
                continue;
            }
            warnings.push(format!("doppelter Key {key}: letzter Wert gilt (Java-Semantik)"));
        }
        props.insert(key.as_str(), value.as_str());
    }
    for c in CORRECTIONS.iter().filter(|c| c.lang == lang) {
        let applied = match &c.fix {
            Fix::Swap { a, b, detect } => match (props.get(a), props.get(b)) {
                (Some(va), Some(vb)) if va.contains(detect) => {
                    let (va, vb) = (va.to_owned(), vb.to_owned());
                    props.insert(*a, vb);
                    props.insert(*b, va);
                    comments.push(((*b).to_owned(), c.reason.to_owned()));
                    true
                }
                _ => false,
            },
            Fix::Replace { key, from, to } => match props.get(key) {
                Some(v) if v.contains(from) => {
                    let nv = v.replace(from, to);
                    props.insert(*key, nv);
                    true
                }
                _ => false,
            },
            Fix::Set { key, value } => match props.get(key) {
                Some(v) if v != *value => {
                    props.insert(*key, *value);
                    true
                }
                _ => false,
            },
        };
        let key = match &c.fix {
            Fix::Swap { a, .. } => a,
            Fix::Replace { key, .. } | Fix::Set { key, .. } => key,
        };
        if applied {
            comments.push(((*key).to_owned(), c.reason.to_owned()));
        }
    }
    for (l, key, value) in SUPPLEMENTS {
        if *l == lang && !props.contains_key(key) {
            props.insert(*key, *value);
            comments.push(((*key).to_owned(), "ergänzt (PIDA): fehlte in dieser Sprache".to_owned()));
        }
    }
    Ok(PreparedMessages { props, key_comments: comments, warnings })
}

/// Kopfkommentar der erzeugten Kataloge.
pub fn pdfover_header(lang: Lang) -> String {
    let (name, file) = match lang {
        Lang::De => ("deutscher", "messages_de.properties"),
        Lang::En => ("englischer", "messages.properties"),
    };
    format!(
        "PIDA – {name} Katalog (EUPL-1.2)\n\
         \n\
         Erzeugt mit `pida_i18n::convert::convert_pdfover_messages` aus `{file}` von PDF-Over\n\
         (A-SIT, EUPL). NICHT von Hand bearbeiten: Korrekturen gehören in die Tabellen des Konverters,\n\
         eigene Texte von PIDA in `pida.ftl`. Neu erzeugen:\n\
         PIDA_I18N_BLESS=1 cargo test -p pida-i18n --test catalogs\n\
         \n\
         Konventionen: Fluent-ID = Java-Key mit „-“ statt „.“; `&`-Mnemonic → `.accesskey`;\n\
         `%s`/`%d` → `{{ $arg1 }}`, `{{ $arg2 }}` … in Reihenfolge; `<a>…</a>` bzw. `<a href=\"…\">…</a>`\n\
         bleiben als Link-Markup im Text (`pida_i18n::markup`)."
    )
}

/// Vollständige Konvertierung einer PDF-Over-Nachrichtendatei (Bytes wie im Repository).
pub fn convert_pdfover_messages(bytes: &[u8], lang: Lang) -> Result<Conversion, ConvertError> {
    let PreparedMessages { props, key_comments, mut warnings } = prepare_pdfover_messages(bytes, lang)?;
    let opts = ConvertOptions {
        unused_keys: PDFOVER_UNUSED_KEYS.iter().map(|s| (*s).to_owned()).collect(),
        header: Some(pdfover_header(lang)),
        key_comments,
    };
    let mut conversion = properties_to_ftl(&props, &opts)?;
    warnings.append(&mut conversion.warnings);
    conversion.warnings = warnings;
    Ok(conversion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics() {
        assert_eq!(convert_value("C&ancel"), ("Cancel".into(), Some('a')));
        assert_eq!(convert_value("O&K"), ("OK".into(), Some('K')));
        assert_eq!(convert_value("Save && Exit"), ("Save & Exit".into(), None));
        assert_eq!(convert_value("A & B"), ("A & B".into(), None));
        assert_eq!(convert_value("Ü&ber %s"), ("Über { $arg1 }".into(), Some('b')));
    }

    #[test]
    fn placeholders_in_order() {
        assert_eq!(
            convert_value("%s is invalid: has to be a number between %d and %d").0,
            "{ $arg1 } is invalid: has to be a number between { $arg2 } and { $arg3 }"
        );
        assert_eq!(convert_value("100%% sicher").0, "100% sicher");
        assert_eq!(java_placeholders("Operation aborted (%d):\n%s"), vec![Placeholder::Int, Placeholder::Str]);
    }

    #[test]
    fn multiline_and_escapes() {
        assert_eq!(convert_value("a\n\nb").0, "a\n\nb");
        assert_eq!(convert_value("a\n ● b").0, "a\n{\" \"}● b");
        assert_eq!(convert_value("x {y}").0, "x {\"{\"}y{\"}\"}");
        assert_eq!(convert_value("").0, "{ \"\" }");
        assert_eq!(convert_value("a\n").0, "a\n{ \"\" }");
        assert_eq!(convert_value("a\n.b").0, "a\n{\".\"}b");
        assert_eq!(convert_value(".b").0, ".b");
        assert_eq!(convert_value("a  ").0, "a{\"  \"}");
    }

    #[test]
    fn raw_entries_keep_duplicates() {
        let e = raw_entries("# c\na=1\nb=x\\\n  y\na=2\n").unwrap();
        assert_eq!(e, vec![("a".into(), "1".into()), ("b".into(), "xy".into()), ("a".into(), "2".into())]);
    }

    #[test]
    fn raw_entries_accept_all_java_line_endings() {
        // Nur `\r` (alte Mac-Dateien): ein Kommentar am Anfang darf nicht die ganze Datei schlucken.
        let expected: Vec<(String, String)> =
            vec![("a".into(), "1".into()), ("b".into(), "xy".into()), ("a".into(), "2".into())];
        for text in ["# c\ra=1\rb=x\\\r  y\ra=2\r", "# c\r\na=1\r\nb=x\\\r\n  y\r\na=2", "# c\n\ra=1\rb=x\\\n  y\na=2"]
        {
            assert_eq!(raw_entries(text).unwrap(), expected, "{text:?}");
            let java = Properties::load_str(text).unwrap();
            assert_eq!(java.get("a"), Some("2"));
            assert_eq!(java.get("b"), Some("xy"));
        }
        assert_eq!(raw_entries("").unwrap(), vec![]);
        assert_eq!(raw_entries("ä=ö\rü=ß").unwrap(), vec![("ä".into(), "ö".into()), ("ü".into(), "ß".into())]);
    }

    #[test]
    fn b09_duplicate_is_renamed() {
        let src = "error.Initialization=Erster\\nText\nerror.Initialization=Zweiter\n";
        let PreparedMessages { props, key_comments: comments, warnings } =
            prepare_pdfover_messages(src.as_bytes(), Lang::De).unwrap();
        assert_eq!(props.get("error.Initialization"), Some("Erster\nText"));
        assert_eq!(props.get("error.ConfigInitialization"), Some("Zweiter"));
        assert!(warnings.is_empty());
        assert!(comments.iter().any(|(k, c)| k == "error.ConfigInitialization" && c.contains("B-09")));
    }

    #[test]
    fn plain_duplicates_follow_java() {
        let PreparedMessages { props, warnings, .. } = prepare_pdfover_messages(b"a=1\na=2\n", Lang::En).unwrap();
        assert_eq!(props.get("a"), Some("2"));
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn compare_detects_differences() {
        let a: Properties = [("x", "%s und %d"), ("y", "a")].into_iter().collect();
        let b: Properties = [("x", "%d und %s"), ("z", "b")].into_iter().collect();
        let p = compare_catalogs(&a, &b);
        assert_eq!(p.len(), 3, "{p:?}");
    }

    #[test]
    fn generated_ftl_is_valid_and_annotated() {
        let props: Properties = [("a.b", "x"), ("c.d_e", "&Save"), ("multi", "1\n2")].into_iter().collect();
        let conv = properties_to_ftl(
            &props,
            &ConvertOptions { unused_keys: vec!["a.b".into()], header: Some("H".into()), ..Default::default() },
        )
        .unwrap();
        assert_eq!(conv.messages, 3);
        assert!(conv.ftl.starts_with("### H\n\n"));
        assert!(conv.ftl.contains("# unused"));
        assert!(conv.ftl.contains("c-d_e = Save\n    .accesskey = S\n"));
        assert!(conv.ftl.contains("multi =\n    1\n    2\n"));
    }

    #[test]
    fn id_collision_is_an_error() {
        let props: Properties = [("a.b", "x"), ("a-b", "y")].into_iter().collect();
        assert!(matches!(properties_to_ftl(&props, &ConvertOptions::default()), Err(ConvertError::DuplicateId(_))));
    }
}
