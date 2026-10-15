//! Bit-kompatibler Leser und Schreiber für `java.util.Properties`.
//!
//! Gemeinsame Basis für drei Pakete, deshalb bereits im Fundament **vollständig implementiert**
//! (ADR 0001 §5.1):
//! - `pida-config`: `PDF-Over.config` (docs/analyse/05 §1.3, §1.9),
//! - `pida-sigblock`: PDF-AS-Profile `cfg/config.properties` + `profiles/*.properties`
//!   (docs/analyse/04 §2.1),
//! - `pida-i18n`: Konverter für `messages*.properties` (docs/analyse/01 §6.3).
//!
//! Semantik wie JDK 21:
//! - [`Properties::load_latin1`] entspricht `Properties.load(InputStream)` (jedes Byte ist ein Zeichen
//!   aus ISO-8859-1), [`Properties::load_str`] entspricht `Properties.load(Reader)`.
//! - Logische Zeilen, Fortsetzung mit ungerader Anzahl `\` am Zeilenende, Kommentare `#`/`!` nur am
//!   Beginn einer logischen Zeile, Trenner `=`, `:` oder Leerraum, Escapes `\t \n \r \f \uXXXX` und
//!   `\x` → `x`. Ein fehlerhaftes `\u` ist wie in Java ein Fehler.
//! - Doppelte Schlüssel: der spätere Wert gewinnt (Hashtable-`put`); die Position des ersten
//!   Vorkommens bleibt für die Iteration erhalten.
//! - [`Properties::store_latin1`] entspricht `Properties.store(OutputStream, comments)` inklusive
//!   `saveConvert`/`writeComments`; seit JDK 18 werden die Einträge nach Schlüssel sortiert geschrieben
//!   ([`StoreOrder::SortedByKey`], Default).

use std::collections::HashMap;
use std::fmt;

/// Fehler beim Lesen einer Properties-Datei.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PropertiesError {
    /// `\u` ohne vier gültige Hex-Ziffern (Java: `IllegalArgumentException("Malformed \\uxxxx encoding.")`).
    #[error("fehlerhafte \\uXXXX-Sequenz in logischer Zeile {line}")]
    MalformedUnicodeEscape {
        /// 1-basierte Nummer der logischen Zeile.
        line: usize,
    },
}

/// Erkannte bzw. verwendete Zeichenkodierung einer Datei.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// ISO-8859-1 (Java-Standard für `load(InputStream)`).
    Latin1,
    /// UTF-8 (von Hand bearbeitete Dateien, docs/analyse/05 §1.9 Punkt 2).
    Utf8,
}

/// Zeilenende beim Schreiben (`BufferedWriter.newLine()` ist plattformabhängig).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineEnding {
    /// `\n` (Linux, macOS).
    #[default]
    Lf,
    /// `\r\n` (Windows).
    CrLf,
}

impl LineEnding {
    /// Zeilenende der laufenden Plattform, wie Java `System.lineSeparator()`.
    pub fn platform() -> Self {
        if cfg!(windows) { LineEnding::CrLf } else { LineEnding::Lf }
    }

    fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }
}

/// Reihenfolge der Einträge beim Schreiben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StoreOrder {
    /// Natürliche Sortierung der Schlüssel (JDK ≥ 18, also PDF-Over 4.4.x mit Java 21).
    #[default]
    SortedByKey,
    /// Einfügereihenfolge.
    Insertion,
}

/// Optionen für [`Properties::store_latin1`] und [`Properties::store_string`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoreOptions {
    /// Kommentar (`comments`-Parameter von `store`), z. B. `"Configuration file was generated!"`.
    pub comment: Option<String>,
    /// Datumszeile ohne führendes `#` (Java: `new Date().toString()`); `None` lässt sie weg.
    pub date_line: Option<String>,
    /// Zeilenende.
    pub line_ending: LineEnding,
    /// Reihenfolge der Einträge.
    pub order: StoreOrder,
}

/// Geordnete Properties-Map mit Java-Semantik.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Properties {
    entries: Vec<(String, String)>,
    index: HashMap<String, usize>,
}

impl fmt::Debug for Properties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.entries.iter().map(|(k, v)| (k, v))).finish()
    }
}

impl Properties {
    /// Leere Map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wie `Properties.load(InputStream)`: jedes Byte ist ein ISO-8859-1-Zeichen.
    pub fn load_latin1(bytes: &[u8]) -> Result<Self, PropertiesError> {
        let units: Vec<u16> = bytes.iter().map(|&b| u16::from(b)).collect();
        Self::load_units(&units)
    }

    /// Wie `Properties.load(Reader)` auf einem bereits dekodierten Text.
    pub fn load_str(text: &str) -> Result<Self, PropertiesError> {
        let units: Vec<u16> = text.encode_utf16().collect();
        Self::load_units(&units)
    }

    /// Heuristik aus docs/analyse/05 §1.9 Punkt 2: Ist die Datei gültiges UTF-8 **und** enthält sie
    /// mindestens eine Mehrbyte-Sequenz, wurde sie wahrscheinlich von Hand als UTF-8 gespeichert;
    /// sonst gilt ISO-8859-1.
    pub fn load_auto(bytes: &[u8]) -> Result<(Self, Encoding), PropertiesError> {
        match std::str::from_utf8(bytes) {
            Ok(text) if bytes.iter().any(|&b| b >= 0x80) => Ok((Self::load_str(text)?, Encoding::Utf8)),
            _ => Ok((Self::load_latin1(bytes)?, Encoding::Latin1)),
        }
    }

    fn load_units(units: &[u16]) -> Result<Self, PropertiesError> {
        let mut props = Properties::new();
        let mut reader = LineReader::new(units);
        let mut line_no = 0usize;
        while let Some(line) = reader.read_line() {
            line_no += 1;
            let limit = line.len();
            let mut key_len = 0usize;
            let mut value_start = limit;
            let mut has_sep = false;
            let mut preceding_backslash = false;
            while key_len < limit {
                let c = line[key_len];
                if (c == u16::from(b'=') || c == u16::from(b':')) && !preceding_backslash {
                    value_start = key_len + 1;
                    has_sep = true;
                    break;
                } else if is_ws(c) && !preceding_backslash {
                    value_start = key_len + 1;
                    break;
                }
                if c == u16::from(b'\\') {
                    preceding_backslash = !preceding_backslash;
                } else {
                    preceding_backslash = false;
                }
                key_len += 1;
            }
            while value_start < limit {
                let c = line[value_start];
                if !is_ws(c) {
                    if !has_sep && (c == u16::from(b'=') || c == u16::from(b':')) {
                        has_sep = true;
                    } else {
                        break;
                    }
                }
                value_start += 1;
            }
            let key = load_convert(&line[..key_len], line_no)?;
            let value = load_convert(&line[value_start..limit], line_no)?;
            props.insert(key, value);
        }
        Ok(props)
    }

    /// Wert zu `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.index.get(key).map(|&i| self.entries[i].1.as_str())
    }

    /// `true`, wenn `key` vorhanden ist (auch mit leerem Wert – wichtig für `SIGNATURE_NOTE`,
    /// docs/analyse/05 §1.5).
    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// Setzt einen Wert (Hashtable-`put`); liefert den alten Wert.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        let key = key.into();
        let value = value.into();
        match self.index.get(&key) {
            Some(&i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, value));
                None
            }
        }
    }

    /// Entfernt einen Schlüssel.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let i = self.index.remove(key)?;
        let (_, value) = self.entries.remove(i);
        for idx in self.index.values_mut() {
            if *idx > i {
                *idx -= 1;
            }
        }
        Some(value)
    }

    /// Anzahl der Einträge.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true`, wenn leer.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Einträge in Einfügereihenfolge (Position des ersten Vorkommens).
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Schlüssel in Einfügereihenfolge.
    pub fn keys(&self) -> impl Iterator<Item = &str> + '_ {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    /// Alle Einträge, deren Schlüssel mit `prefix` beginnt (z. B. `include`, `sig_obj.types.`).
    pub fn with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.iter().filter(move |(k, _)| k.starts_with(prefix))
    }

    /// `putAll`: spätere Werte überschreiben gleiche Schlüssel (docs/analyse/04 §2.1 Punkt 2).
    pub fn extend_from(&mut self, other: &Properties) {
        for (k, v) in other.iter() {
            self.insert(k, v);
        }
    }

    /// Wie `Properties.store(OutputStream, comments)`: ISO-8859-1, alles außerhalb 0x20–0x7E als
    /// `\uXXXX` (Großbuchstaben-Hex).
    pub fn store_latin1(&self, opts: &StoreOptions) -> Vec<u8> {
        let text = self.store_impl(opts, true);
        // Nach saveConvert mit escUnicode=true enthält der Eintragsteil nur ASCII; Kommentare dürfen
        // Zeichen bis U+00FF roh enthalten (writeComments), größere sind bereits escaped.
        text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect()
    }

    /// Wie `Properties.store(Writer, comments)`: ohne `\u`-Escapes für Nicht-ASCII-Zeichen.
    pub fn store_string(&self, opts: &StoreOptions) -> String {
        self.store_impl(opts, false)
    }

    fn store_impl(&self, opts: &StoreOptions, escape_unicode: bool) -> String {
        let nl = opts.line_ending.as_str();
        let mut out = String::new();
        if let Some(comment) = &opts.comment {
            write_comments(&mut out, comment, nl);
        }
        if let Some(date) = &opts.date_line {
            out.push('#');
            out.push_str(date);
            out.push_str(nl);
        }
        let mut entries: Vec<&(String, String)> = self.entries.iter().collect();
        if opts.order == StoreOrder::SortedByKey {
            // Java String.compareTo vergleicht UTF-16-Code-Units.
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        }
        for (k, v) in entries {
            out.push_str(&save_convert(k, true, escape_unicode));
            out.push('=');
            out.push_str(&save_convert(v, false, escape_unicode));
            out.push_str(nl);
        }
        out
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for Properties {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut p = Properties::new();
        for (k, v) in iter {
            p.insert(k, v);
        }
        p
    }
}

fn is_ws(c: u16) -> bool {
    c == u16::from(b' ') || c == u16::from(b'\t') || c == 0x0C
}

/// Nachbau von `java.util.Properties.LineReader.readLine` (JDK 21).
struct LineReader<'a> {
    input: &'a [u16],
    off: usize,
}

impl<'a> LineReader<'a> {
    fn new(input: &'a [u16]) -> Self {
        Self { input, off: 0 }
    }

    fn read_line(&mut self) -> Option<Vec<u16>> {
        const CR: u16 = b'\r' as u16;
        const LF: u16 = b'\n' as u16;
        let mut buf: Vec<u16> = Vec::new();
        let mut skip_white_space = true;
        let mut appended_line_begin = false;
        let mut preceding_backslash = false;
        loop {
            if self.off >= self.input.len() {
                if buf.is_empty() {
                    return None;
                }
                if preceding_backslash {
                    buf.pop();
                }
                return Some(buf);
            }
            let c = self.input[self.off];
            self.off += 1;
            if skip_white_space {
                if is_ws(c) {
                    continue;
                }
                if !appended_line_begin && (c == CR || c == LF) {
                    continue;
                }
                skip_white_space = false;
                appended_line_begin = false;
            }
            if buf.is_empty() && (c == u16::from(b'#') || c == u16::from(b'!')) {
                // Kommentar bis zum Zeilenende überspringen.
                while self.off < self.input.len() {
                    let d = self.input[self.off];
                    if d == LF || d == CR {
                        break;
                    }
                    self.off += 1;
                }
                skip_white_space = true;
                continue;
            }
            if c != LF && c != CR {
                buf.push(c);
                if c == u16::from(b'\\') {
                    preceding_backslash = !preceding_backslash;
                } else {
                    preceding_backslash = false;
                }
            } else {
                if buf.is_empty() {
                    skip_white_space = true;
                    continue;
                }
                if self.off >= self.input.len() {
                    if preceding_backslash {
                        buf.pop();
                    }
                    return Some(buf);
                }
                if preceding_backslash {
                    buf.pop();
                    skip_white_space = true;
                    appended_line_begin = true;
                    preceding_backslash = false;
                    if c == CR && self.input.get(self.off) == Some(&LF) {
                        self.off += 1;
                    }
                } else {
                    return Some(buf);
                }
            }
        }
    }
}

/// Nachbau von `Properties.loadConvert`.
fn load_convert(input: &[u16], line: usize) -> Result<String, PropertiesError> {
    let mut out: Vec<u16> = Vec::with_capacity(input.len());
    let mut i = 0usize;
    while i < input.len() {
        let c = input[i];
        i += 1;
        if c == u16::from(b'\\') {
            let Some(&e) = input.get(i) else {
                // LineReader schließt unescapte `\` am Zeilenende aus; defensiv ignorieren.
                break;
            };
            i += 1;
            if e == u16::from(b'u') {
                if i + 4 > input.len() {
                    return Err(PropertiesError::MalformedUnicodeEscape { line });
                }
                let mut value: u16 = 0;
                for &h in &input[i..i + 4] {
                    let digit = char::from_u32(u32::from(h))
                        .and_then(|ch| ch.to_digit(16))
                        .ok_or(PropertiesError::MalformedUnicodeEscape { line })?;
                    value = (value << 4) | digit as u16;
                }
                i += 4;
                out.push(value);
            } else {
                let mapped = match e {
                    x if x == u16::from(b't') => u16::from(b'\t'),
                    x if x == u16::from(b'r') => u16::from(b'\r'),
                    x if x == u16::from(b'n') => u16::from(b'\n'),
                    x if x == u16::from(b'f') => 0x0C,
                    x => x,
                };
                out.push(mapped);
            }
        } else {
            out.push(c);
        }
    }
    Ok(String::from_utf16_lossy(&out))
}

/// Nachbau von `Properties.saveConvert`.
fn save_convert(s: &str, escape_space: bool, escape_unicode: bool) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for (x, unit) in s.encode_utf16().enumerate() {
        if unit > 61 && unit < 127 {
            if unit == u16::from(b'\\') {
                out.push_str("\\\\");
            } else {
                out.push(char::from(unit as u8));
            }
            continue;
        }
        match unit {
            0x20 => {
                if x == 0 || escape_space {
                    out.push('\\');
                }
                out.push(' ');
            }
            0x09 => out.push_str("\\t"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x0C => out.push_str("\\f"),
            0x3D | 0x3A | 0x23 | 0x21 => {
                out.push('\\');
                out.push(char::from(unit as u8));
            }
            _ => {
                if (!(0x0020..=0x007e).contains(&unit)) && escape_unicode {
                    out.push_str(&format!("\\u{unit:04X}"));
                } else {
                    // Ohne Unicode-Escape: Code-Unit unverändert; Surrogate werden unten zusammengesetzt.
                    push_unit(&mut out, unit);
                }
            }
        }
    }
    fixup_surrogates(out)
}

/// Schreibt eine UTF-16-Code-Unit; einzelne Surrogate werden als private Markierung gesammelt und
/// in [`fixup_surrogates`] zu einem Zeichen zusammengesetzt.
fn push_unit(out: &mut String, unit: u16) {
    match char::from_u32(u32::from(unit)) {
        Some(c) => out.push(c),
        None => {
            // Surrogat: als Platzhalter in der Private-Use-Area U+10FFxx kodieren (nur intern).
            out.push(char::from_u32(0x10_0000 | u32::from(unit)).unwrap_or('\u{FFFD}'));
        }
    }
}

fn fixup_surrogates(s: String) -> String {
    if !s.chars().any(|c| (0x10_D800..=0x10_DFFF).contains(&u32::from(c))) {
        return s;
    }
    let mut units: Vec<u16> = Vec::with_capacity(s.len());
    for c in s.chars() {
        let cp = u32::from(c);
        if (0x10_D800..=0x10_DFFF).contains(&cp) {
            units.push((cp & 0xFFFF) as u16);
        } else {
            let mut b = [0u16; 2];
            units.extend_from_slice(c.encode_utf16(&mut b));
        }
    }
    String::from_utf16_lossy(&units)
}

/// Nachbau von `Properties.writeComments`.
fn write_comments(out: &mut String, comments: &str, nl: &str) {
    out.push('#');
    let units: Vec<u16> = comments.encode_utf16().collect();
    let len = units.len();
    let mut current = 0usize;
    let mut last = 0usize;
    while current < len {
        let c = units[current];
        if c > 0x00FF || c == 0x0A || c == 0x0D {
            if last != current {
                out.push_str(&String::from_utf16_lossy(&units[last..current]));
            }
            if c > 0x00FF {
                out.push_str(&format!("\\u{c:04X}"));
            } else {
                out.push_str(nl);
                if c == 0x0D && current != len - 1 && units[current + 1] == 0x0A {
                    current += 1;
                }
                if current == len - 1
                    || (units[current + 1] != u16::from(b'#') && units[current + 1] != u16::from(b'!'))
                {
                    out.push('#');
                }
            }
            last = current + 1;
        }
        current += 1;
    }
    if last != current {
        out.push_str(&String::from_utf16_lossy(&units[last..current]));
    }
    out.push_str(nl);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_and_whitespace() {
        let p = Properties::load_str("a=1\nb : 2\nc 3\n  d=  4  \ne\nf=\n").unwrap();
        assert_eq!(p.get("a"), Some("1"));
        assert_eq!(p.get("b"), Some("2"));
        assert_eq!(p.get("c"), Some("3"));
        assert_eq!(p.get("d"), Some("4  "));
        assert_eq!(p.get("e"), Some(""));
        assert_eq!(p.get("f"), Some(""));
        assert!(p.contains_key("f"));
    }

    #[test]
    fn comments_only_at_line_start() {
        let p = Properties::load_str("# c\n! c2\nkey=value # no comment\n").unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p.get("key"), Some("value # no comment"));
    }

    #[test]
    fn continuation_lines() {
        let p = Properties::load_str("k = first \\\n    second \\\r\n\tthird\nx=\\\\\ny=a\\\nb\n").unwrap();
        assert_eq!(p.get("k"), Some("first second third"));
        assert_eq!(p.get("x"), Some("\\"));
        // führender Leerraum der Folgezeile entfällt, kein Leerzeichen wird eingefügt
        assert_eq!(p.get("y"), Some("ab"));
    }

    #[test]
    fn continuation_line_is_not_a_comment() {
        let p = Properties::load_str("k=a\\\n#b\n").unwrap();
        assert_eq!(p.get("k"), Some("a#b"));
    }

    #[test]
    fn escapes() {
        let p = Properties::load_str("path=C\\:\\\\Users\\\\x\nnl=a\\nb\nu=\\u00e4\\u00C4\nk\\=ey=v\n").unwrap();
        assert_eq!(p.get("path"), Some("C:\\Users\\x"));
        assert_eq!(p.get("nl"), Some("a\nb"));
        assert_eq!(p.get("u"), Some("äÄ"));
        assert_eq!(p.get("k=ey"), Some("v"));
    }

    #[test]
    fn malformed_unicode_is_error() {
        assert!(matches!(Properties::load_str("a=\\u12\n"), Err(PropertiesError::MalformedUnicodeEscape { line: 1 })));
    }

    #[test]
    fn latin1_bytes() {
        let bytes = b"note=Gr\xfc\xdfe\n";
        let p = Properties::load_latin1(bytes).unwrap();
        assert_eq!(p.get("note"), Some("Grüße"));
        let (p2, enc) = Properties::load_auto(bytes).unwrap();
        assert_eq!(enc, Encoding::Latin1);
        assert_eq!(p2.get("note"), Some("Grüße"));
        let (p3, enc3) = Properties::load_auto("note=Grüße\n".as_bytes()).unwrap();
        assert_eq!(enc3, Encoding::Utf8);
        assert_eq!(p3.get("note"), Some("Grüße"));
    }

    #[test]
    fn duplicate_key_last_wins() {
        let p = Properties::load_str("a=1\nb=2\na=3\n").unwrap();
        assert_eq!(p.get("a"), Some("3"));
        assert_eq!(p.keys().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn surrogate_pair_escape() {
        let p = Properties::load_str("e=\\uD83D\\uDE00\n").unwrap();
        assert_eq!(p.get("e"), Some("😀"));
    }

    #[test]
    fn store_roundtrip_and_format() {
        let mut p = Properties::new();
        p.insert("SIGNATURE_NOTE", "Grüße: a=b #1");
        p.insert("OUTPUT_FOLDER", "C:\\Users\\x");
        p.insert(" key", " lead");
        let opts = StoreOptions {
            comment: Some("Configuration file was generated!".into()),
            date_line: Some("Tue Oct 14 15:44:00 CEST 2026".into()),
            ..Default::default()
        };
        let bytes = p.store_latin1(&opts);
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.starts_with("#Configuration file was generated!\n#Tue Oct 14 15:44:00 CEST 2026\n"));
        assert!(text.contains("OUTPUT_FOLDER=C\\:\\\\Users\\\\x\n"));
        assert!(text.contains("SIGNATURE_NOTE=Gr\\u00FC\\u00DFe\\: a\\=b \\#1\n"));
        assert!(text.contains("\\ key=\\ lead\n"));
        let back = Properties::load_latin1(&bytes).unwrap();
        assert_eq!(back.get("SIGNATURE_NOTE"), Some("Grüße: a=b #1"));
        assert_eq!(back.get("OUTPUT_FOLDER"), Some("C:\\Users\\x"));
        assert_eq!(back.get(" key"), Some(" lead"));
    }

    #[test]
    fn store_sorted_by_key() {
        let p: Properties = [("b", "2"), ("a", "1")].into_iter().collect();
        let s = p.store_string(&StoreOptions::default());
        assert_eq!(s, "a=1\nb=2\n");
        let s2 = p.store_string(&StoreOptions { order: StoreOrder::Insertion, ..Default::default() });
        assert_eq!(s2, "b=2\na=1\n");
    }

    #[test]
    fn store_string_keeps_non_ascii() {
        let p: Properties = [("k", "ä😀")].into_iter().collect();
        assert_eq!(p.store_string(&StoreOptions::default()), "k=ä😀\n");
        let latin = p.store_latin1(&StoreOptions::default());
        assert_eq!(latin, b"k=\\u00E4\\uD83D\\uDE00\n");
    }

    #[test]
    fn multiline_comment() {
        let p = Properties::new();
        let s = p.store_string(&StoreOptions { comment: Some("a\nb\n#c".into()), ..Default::default() });
        assert_eq!(s, "#a\n#b\n#c\n");
    }

    #[test]
    fn remove_keeps_index_consistent() {
        let mut p: Properties = [("a", "1"), ("b", "2"), ("c", "3")].into_iter().collect();
        assert_eq!(p.remove("a"), Some("1".into()));
        assert_eq!(p.get("c"), Some("3"));
        assert_eq!(p.get("b"), Some("2"));
        assert_eq!(p.len(), 2);
    }
}
