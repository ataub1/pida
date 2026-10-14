//! Ausdrücke `${…}` (OGNL-Teilmenge) und `SIG_DATE` (docs/analyse/04 §2.5).
//!
//! Unterstützt genau die belegte Teilmenge: `subject.X`, `issuer.X`, `sn`, `sbp.X`, `iui.pdfVersion`,
//! Ternary `a != null ? (b + " ") : ""`, Null-Vergleich, String-Konkatenation, Map-Zugriff und den
//! Listen-Literal-Trick (`{expr}` → `[wert]` → Klammern abschneiden). Alles andere ergibt
//! [`crate::SigBlockError::UnsupportedExpression`].
//!
//! Nachgebildet sind `ValueResolver.resolve` (Regex `\$(\{[^\$]*\})`, `latin1_encoding`,
//! `SIG_DATE` = `SimpleDateFormat`-Muster), `OgnlUtils.resolvsOgnlExpression` und
//! `DNUtils.dnToMap`.

use std::collections::BTreeMap;

use time::{OffsetDateTime, UtcOffset};

use crate::input::SigBlockInput;
use crate::profile::Profile;
use crate::{Result, SigBlockError};

/// Default-Muster von `SIG_DATE` (`ValueResolver.defaultDateFormat`).
pub const DEFAULT_DATE_PATTERN: &str = "yyyy-MM-dd'T'HH:mm:ssXXX";

/// Variablen für die Auflösung. `Debug` zeigt nur die Attributnamen (ADR 0001 Regel 9).
#[derive(Clone, PartialEq, Eq, Default)]
pub struct ExprContext {
    /// `subject.*` (inkl. Aliasse `title`/`T`, `DN`)
    pub subject: BTreeMap<String, String>,
    /// `issuer.*`
    pub issuer: BTreeMap<String, String>,
    /// `sn`
    pub sn: String,
    /// `sbp.*`
    pub sbp: BTreeMap<String, String>,
    /// `iui.pdfVersion`
    pub pdf_version: String,
}

impl std::fmt::Debug for ExprContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprContext")
            .field("subject", &self.subject.keys().collect::<Vec<_>>())
            .field("issuer", &self.issuer.keys().collect::<Vec<_>>())
            .field("sbp", &self.sbp.keys().collect::<Vec<_>>())
            .field("pdf_version", &self.pdf_version)
            .finish_non_exhaustive()
    }
}

/// OID des Titel-Attributs (`title`).
const OID_TITLE: &str = "2.5.4.12";

fn dn_map(avas: &[(String, String)], dn: &str, quirks: bool) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    // linkester (zuerst kodierter) Eintrag gewinnt (Plan §6.5 Register)
    for (k, v) in avas {
        let is_title = k == "title" || k == "T" || k == OID_TITLE;
        if quirks {
            // DNUtils: nur `title` wird zusätzlich als `T` abgelegt
            map.entry(k.clone()).or_insert_with(|| v.clone());
            if k == "title" {
                map.entry("T".to_string()).or_insert_with(|| v.clone());
            }
        } else if is_title {
            map.entry("title".to_string()).or_insert_with(|| v.clone());
            map.entry("T".to_string()).or_insert_with(|| v.clone());
            if k == OID_TITLE {
                map.entry(k.clone()).or_insert_with(|| v.clone());
            }
        } else {
            map.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    map.insert("DN".to_string(), dn.to_string());
    map
}

impl ExprContext {
    /// Kontext aus der Blockeingabe mit den Alias-Regeln aus [`crate::CertFields`]
    /// (Titel-RDN unter `title` **und** `T`, linkester Eintrag gewinnt, `DN` = gesamter DN).
    pub fn from_input(input: &SigBlockInput) -> ExprContext {
        ExprContext {
            subject: dn_map(&input.cert.subject, &input.cert.subject_dn, input.pdfas_quirks),
            issuer: dn_map(&input.cert.issuer, &input.cert.issuer_dn, input.pdfas_quirks),
            sn: input.cert.serial_decimal.clone(),
            sbp: input.sbp.clone(),
            pdf_version: input.pdf_version.clone(),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// OGNL-Teilmenge
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Map(&'static str),
    List(Vec<Value>),
}

impl Value {
    /// `String.valueOf` bzw. `AbstractCollection.toString`.
    fn to_java_string(&self, ctx: &ExprContext) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Str(s) => s.clone(),
            Value::Map(name) => {
                let m = map_of(ctx, name);
                let inner: Vec<String> =
                    m.map(|m| m.iter().map(|(k, v)| format!("{k}={v}")).collect()).unwrap_or_default();
                format!("{{{}}}", inner.join(", "))
            }
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_java_string(ctx)).collect();
                format!("[{}]", inner.join(", "))
            }
        }
    }

    /// `OgnlOps.booleanValue`.
    fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            _ => true,
        }
    }
}

fn map_of<'a>(ctx: &'a ExprContext, name: &str) -> Option<&'a BTreeMap<String, String>> {
    match name {
        "subject" => Some(&ctx.subject),
        "issuer" => Some(&ctx.issuer),
        "sbp" => Some(&ctx.sbp),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Int(i64),
    Punct(&'static str),
}

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Tok>,
    pos: usize,
    ctx: &'a ExprContext,
}

fn unsupported(src: &str) -> SigBlockError {
    SigBlockError::UnsupportedExpression(src.to_string())
}

fn tokenize(src: &str) -> Result<Vec<Tok>> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '.' || chars[i].is_ascii_alphabetic()) {
                return Err(unsupported(src)); // Gleitkomma/Suffixe nicht unterstützt
            }
            let s: String = chars[start..i].iter().collect();
            out.push(Tok::Int(s.parse().map_err(|_| unsupported(src))?));
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            loop {
                let Some(&ch) = chars.get(i) else { return Err(unsupported(src)) };
                i += 1;
                if ch == quote {
                    break;
                }
                if ch == '\\' {
                    let Some(&esc) = chars.get(i) else { return Err(unsupported(src)) };
                    i += 1;
                    match esc {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        'b' => s.push('\u{8}'),
                        'f' => s.push('\u{c}'),
                        '\\' | '\'' | '"' => s.push(esc),
                        'u' => {
                            let hex: String = chars.get(i..i + 4).ok_or_else(|| unsupported(src))?.iter().collect();
                            let cp = u32::from_str_radix(&hex, 16).map_err(|_| unsupported(src))?;
                            s.push(char::from_u32(cp).ok_or_else(|| unsupported(src))?);
                            i += 4;
                        }
                        _ => return Err(unsupported(src)),
                    }
                } else {
                    s.push(ch);
                }
            }
            out.push(Tok::Str(s));
            continue;
        }
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let p2 = ["==", "!=", "&&", "||"].into_iter().find(|p| *p == two);
        if let Some(p) = p2 {
            out.push(Tok::Punct(p));
            i += 2;
            continue;
        }
        let p1 = [".", "[", "]", "(", ")", "{", "}", ",", "?", ":", "+", "!"].into_iter().find(|p| p.starts_with(c));
        match p1 {
            Some(p) => {
                out.push(Tok::Punct(p));
                i += 1;
            }
            None => return Err(unsupported(src)),
        }
    }
    Ok(out)
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn eat(&mut self, p: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Punct(q)) if *q == p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(q)) if q == w) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, p: &str) -> Result<()> {
        if self.eat(p) { Ok(()) } else { Err(unsupported(self.src)) }
    }

    fn sequence(&mut self) -> Result<Value> {
        // Komma auf oberster Ebene: OGNL-Sequenz, Ergebnis ist der letzte Wert
        let mut v = self.ternary()?;
        while self.eat(",") {
            v = self.ternary()?;
        }
        Ok(v)
    }

    fn ternary(&mut self) -> Result<Value> {
        let cond = self.or()?;
        if self.eat("?") {
            let a = self.ternary()?;
            self.expect(":")?;
            let b = self.ternary()?;
            return Ok(if cond.truthy() { a } else { b });
        }
        Ok(cond)
    }

    fn or(&mut self) -> Result<Value> {
        let mut v = self.and()?;
        while self.eat("||") || self.eat_word("or") {
            let r = self.and()?;
            v = Value::Bool(v.truthy() || r.truthy());
        }
        Ok(v)
    }

    fn and(&mut self) -> Result<Value> {
        let mut v = self.equality()?;
        while self.eat("&&") || self.eat_word("and") {
            let r = self.equality()?;
            v = Value::Bool(v.truthy() && r.truthy());
        }
        Ok(v)
    }

    fn equality(&mut self) -> Result<Value> {
        let mut v = self.additive()?;
        loop {
            let eq = if self.eat("==") || self.eat_word("eq") {
                true
            } else if self.eat("!=") || self.eat_word("neq") {
                false
            } else {
                return Ok(v);
            };
            let r = self.additive()?;
            let same = match (&v, &r) {
                (Value::Null, Value::Null) => true,
                (Value::Null, _) | (_, Value::Null) => false,
                (Value::Str(a), Value::Str(b)) => a == b,
                (Value::Int(a), Value::Int(b)) => a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                _ => return Err(unsupported(self.src)),
            };
            v = Value::Bool(same == eq);
        }
    }

    fn additive(&mut self) -> Result<Value> {
        let mut v = self.unary()?;
        while self.eat("+") {
            let r = self.unary()?;
            v = match (&v, &r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a.checked_add(*b).ok_or_else(|| unsupported(self.src))?),
                (Value::Null, Value::Null) => return Err(unsupported(self.src)),
                (Value::Str(_), _) | (_, Value::Str(_)) => {
                    Value::Str(format!("{}{}", v.to_java_string(self.ctx), r.to_java_string(self.ctx)))
                }
                _ => return Err(unsupported(self.src)),
            };
        }
        Ok(v)
    }

    fn unary(&mut self) -> Result<Value> {
        if self.eat("!") || self.eat_word("not") {
            let v = self.unary()?;
            return Ok(Value::Bool(!v.truthy()));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Value> {
        let mut v = self.primary()?;
        loop {
            if self.eat(".") {
                let Some(Tok::Ident(name)) = self.peek().cloned() else { return Err(unsupported(self.src)) };
                self.pos += 1;
                v = self.property(&v, &name)?;
            } else if self.eat("[") {
                let key = self.sequence()?;
                self.expect("]")?;
                let Value::Str(k) = key else { return Err(unsupported(self.src)) };
                v = self.property(&v, &k)?;
            } else {
                return Ok(v);
            }
        }
    }

    fn property(&self, target: &Value, name: &str) -> Result<Value> {
        match target {
            Value::Map("iui") => {
                if name == "pdfVersion" {
                    Ok(Value::Str(self.ctx.pdf_version.clone()))
                } else {
                    Ok(Value::Null)
                }
            }
            Value::Map(m) => {
                let map = map_of(self.ctx, m).ok_or_else(|| unsupported(self.src))?;
                Ok(map.get(name).map(|s| Value::Str(s.clone())).unwrap_or(Value::Null))
            }
            _ => Err(unsupported(self.src)),
        }
    }

    fn primary(&mut self) -> Result<Value> {
        let Some(tok) = self.peek().cloned() else { return Err(unsupported(self.src)) };
        self.pos += 1;
        match tok {
            Tok::Str(s) => Ok(Value::Str(s)),
            Tok::Int(i) => Ok(Value::Int(i)),
            Tok::Ident(id) => match id.as_str() {
                "null" => Ok(Value::Null),
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                "subject" => Ok(Value::Map("subject")),
                "issuer" => Ok(Value::Map("issuer")),
                "sbp" => Ok(Value::Map("sbp")),
                "iui" => Ok(Value::Map("iui")),
                "sn" => Ok(Value::Str(self.ctx.sn.clone())),
                _ => Err(unsupported(self.src)),
            },
            Tok::Punct("(") => {
                let v = self.sequence()?;
                self.expect(")")?;
                Ok(v)
            }
            Tok::Punct("{") => {
                let mut items = Vec::new();
                if !self.eat("}") {
                    loop {
                        items.push(self.ternary()?);
                        if self.eat("}") {
                            break;
                        }
                        self.expect(",")?;
                    }
                }
                Ok(Value::List(items))
            }
            _ => Err(unsupported(self.src)),
        }
    }
}

/// Wertet einen OGNL-Ausdruck (inklusive Klammern `{…}`) aus und entpackt das Ergebnis wie
/// `OgnlUtils.resolvsOgnlExpression` (führendes `[` und schließendes `]` abschneiden).
fn eval_group(group: &str, ctx: &ExprContext) -> Result<String> {
    let toks = tokenize(group)?;
    let mut p = Parser { src: group, toks, pos: 0, ctx };
    let v = p.sequence()?;
    if p.pos != p.toks.len() {
        return Err(unsupported(group));
    }
    let mut s = v.to_java_string(ctx);
    if s.starts_with('[') {
        s.remove(0);
    }
    if s.ends_with(']') {
        s.pop();
    }
    Ok(s)
}

/// Löst alle `${…}` in `template` auf (Regex `\$(\{[^\$]*\})` wie PDF-AS: ein Treffer reicht bis
/// zur letzten `}` vor dem nächsten `$`).
pub fn resolve(template: &str, ctx: &ExprContext) -> Result<String> {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len());
    let mut cur = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
            let seg_end = template[i + 1..].find('$').map(|p| i + 1 + p).unwrap_or(template.len());
            if let Some(close) = template[i + 1..seg_end].rfind('}') {
                let close = i + 1 + close;
                out.push_str(&template[cur..i]);
                out.push_str(&eval_group(&template[i + 1..=close], ctx)?);
                cur = close + 1;
                i = close + 1;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&template[cur..]);
    Ok(out)
}

/// `new String(value.getBytes(ISO-8859-1), UTF-8)` (`latin1_encoding`).
fn latin1_repair(value: &str) -> String {
    let bytes: Vec<u8> = value.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `ValueResolver.resolve(key, value, profile)`: `latin1_encoding`, `SIG_DATE` (Wert = Datumsmuster,
/// Zeitpunkt = `input.signing_time`, Zeitzone = Profil-`timezone`) und `${…}`-Ausdrücke.
/// `None` bleibt `None` (außer bei `SIG_DATE`, dort gilt das Default-Muster).
pub fn resolve_value(
    key: &str,
    value: Option<&str>,
    profile: &Profile,
    input: &SigBlockInput,
    ctx: &ExprContext,
) -> Result<Option<String>> {
    let repaired;
    let mut value = value;
    if profile.flags.latin1_encoding
        && let Some(v) = value
    {
        repaired = latin1_repair(v);
        value = Some(&repaired);
    }
    if key == "SIG_DATE" {
        return format_sig_date(value, input.signing_time, profile.timezone.as_deref()).map(Some);
    }
    value.map(|v| resolve(v, ctx)).transpose()
}

/// `/Name` des Signaturdictionarys wie PDF-AS: aufgelöster Wert `value.SIG_SUBJECT` des Profils
/// (inkl. `default.*`-Fallback und Laufzeit-Overlays); `None`, wenn das Profil keinen Wert hat.
/// Die Integration (`pdfover-pades::SignPlan::name`) nutzt diese Funktion statt die Alias-Regeln
/// nachzubauen.
pub fn signer_name(profile: &Profile, input: &SigBlockInput) -> Result<Option<String>> {
    let ctx = ExprContext::from_input(input);
    resolve_value("SIG_SUBJECT", profile.values.get("SIG_SUBJECT").map(String::as_str), profile, input, &ctx)
}

// ------------------------------------------------------------------------------------------------
// SimpleDateFormat
// ------------------------------------------------------------------------------------------------

/// `TimeZone.getTimeZone(id)` → Offset zum Zeitpunkt `t` (unbekannt ⇒ `GMT` wie Java).
fn java_zone_offset(id: &str, t: OffsetDateTime) -> UtcOffset {
    if let Some(off) = custom_gmt_offset(id) {
        return off;
    }
    let ts = jiff::Timestamp::new(t.unix_timestamp(), t.nanosecond() as i32);
    match (jiff::tz::TimeZone::get(id), ts) {
        (Ok(tz), Ok(ts)) => UtcOffset::from_whole_seconds(tz.to_offset(ts).seconds()).unwrap_or(UtcOffset::UTC),
        _ => UtcOffset::UTC,
    }
}

/// Java-Custom-IDs `GMT±h`, `GMT±hh`, `GMT±hhmm`, `GMT±h:mm`, `GMT±hh:mm`; `GMT`/`UTC` ⇒ 0.
fn custom_gmt_offset(id: &str) -> Option<UtcOffset> {
    if id == "GMT" || id == "UTC" {
        return Some(UtcOffset::UTC);
    }
    let rest = id.strip_prefix("GMT")?;
    let (sign, rest) = match rest.as_bytes().first()? {
        b'+' => (1i32, &rest[1..]),
        b'-' => (-1i32, &rest[1..]),
        _ => return None,
    };
    let digits_only = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (h, m) = if let Some((h, m)) = rest.split_once(':') {
        if !(1..=2).contains(&h.len()) || m.len() != 2 || !digits_only(h) || !digits_only(m) {
            return None;
        }
        (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?)
    } else {
        if !digits_only(rest) {
            return None;
        }
        match rest.len() {
            1 | 2 => (rest.parse().ok()?, 0),
            3 => (rest[..1].parse().ok()?, rest[1..].parse().ok()?),
            4 => (rest[..2].parse().ok()?, rest[2..].parse().ok()?),
            _ => return None,
        }
    };
    if h > 23 || m > 59 {
        return None;
    }
    UtcOffset::from_whole_seconds(sign * (h * 3600 + m * 60)).ok()
}

fn pad(out: &mut String, v: i64, width: usize) {
    if v < 0 {
        out.push('-');
    }
    let s = v.unsigned_abs().to_string();
    for _ in s.len()..width {
        out.push('0');
    }
    out.push_str(&s);
}

fn offset_text(out: &mut String, off: UtcOffset, count: usize, iso: bool) -> std::result::Result<(), ()> {
    let total = off.whole_seconds();
    if iso && total == 0 {
        out.push('Z');
        return Ok(());
    }
    let sign = if total < 0 { '-' } else { '+' };
    let mins = total.abs() / 60;
    let (h, m) = (mins / 60, mins % 60);
    out.push(sign);
    pad(out, i64::from(h), 2);
    if iso {
        match count {
            1 => {}
            2 => pad(out, i64::from(m), 2),
            3 => {
                out.push(':');
                pad(out, i64::from(m), 2);
            }
            _ => return Err(()),
        }
    } else {
        pad(out, i64::from(m), 2);
    }
    Ok(())
}

/// Formatiert `SIG_DATE` mit einem `SimpleDateFormat`-Muster (Default
/// `yyyy-MM-dd'T'HH:mm:ssXXX`, Quirk Q9, Register).
///
/// Zeitzone: `timezone = None` ⇒ der Offset von `time` (die Integration übergibt die lokale Zeit
/// inklusive Offset, ermittelt über `jiff`, ADR 0001 §8); `Some(id)` ⇒ Java-`TimeZone`-ID
/// (IANA-Name wie `Europe/Vienna`, `UTC`, `GMT+01:00`) über `jiff` (Workspace-Dependency);
/// unbekannte IDs ergeben wie in Java `GMT`.
///
/// Unterstützte Musterbuchstaben: `y M L d D F u H k K h m s S X Z` (Zahlen, sprachunabhängig);
/// Texte wie `MMM`, `E`, `a`, `z`, `G` sowie `Y w W` (gebietsschemaabhängig) werden abgelehnt.
pub fn format_sig_date(pattern: Option<&str>, time: OffsetDateTime, timezone: Option<&str>) -> Result<String> {
    let pattern = pattern.unwrap_or(DEFAULT_DATE_PATTERN);
    let offset = match timezone {
        Some(id) => java_zone_offset(id, time),
        None => time.offset(),
    };
    let t = time.to_offset(offset);
    let bad = || SigBlockError::UnsupportedDatePattern(pattern.to_string());
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // '' ⇒ ', sonst Literal bis zum nächsten '
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(bad()), // Unterminated quote
                    Some('\'') if chars.get(i + 1) == Some(&'\'') => {
                        out.push('\'');
                        i += 2;
                    }
                    Some('\'') => {
                        i += 1;
                        break;
                    }
                    Some(ch) => {
                        out.push(*ch);
                        i += 1;
                    }
                }
            }
            continue;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i] == c {
            i += 1;
        }
        let n = i - start;
        let hour = i64::from(t.hour());
        match c {
            'y' => {
                let year = i64::from(t.year());
                if n == 2 {
                    pad(&mut out, year.rem_euclid(100), 2);
                } else {
                    pad(&mut out, year, n);
                }
            }
            'M' | 'L' if n <= 2 => pad(&mut out, i64::from(u8::from(t.month())), n),
            'd' => pad(&mut out, i64::from(t.day()), n),
            'D' => pad(&mut out, i64::from(t.ordinal()), n),
            'F' => pad(&mut out, i64::from((t.day() - 1) / 7 + 1), n),
            'u' => pad(&mut out, i64::from(t.weekday().number_from_monday()), n),
            'H' => pad(&mut out, hour, n),
            'k' => pad(&mut out, if hour == 0 { 24 } else { hour }, n),
            'K' => pad(&mut out, hour % 12, n),
            'h' => pad(&mut out, if hour % 12 == 0 { 12 } else { hour % 12 }, n),
            'm' => pad(&mut out, i64::from(t.minute()), n),
            's' => pad(&mut out, i64::from(t.second()), n),
            'S' => pad(&mut out, i64::from(t.millisecond()), n),
            'X' => offset_text(&mut out, offset, n, true).map_err(|_| bad())?,
            'Z' => offset_text(&mut out, offset, n, false).map_err(|_| bad())?,
            _ => return Err(bad()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::CertFields;
    use time::macros::datetime;

    fn ctx() -> ExprContext {
        let mut input = SigBlockInput::new(CertFields::example(), datetime!(2026-10-14 12:34:56 +2));
        input.cert.subject.push(("title".into(), "Dipl.-Ing.".into()));
        input.sbp.insert("Abteilung".into(), "IT".into());
        ExprContext::from_input(&input)
    }

    const SIG_SUBJECT: &str = "${subject.title != null ? (subject.title + \" \") : \"\"}${subject.CN}";

    #[test]
    fn sig_subject_with_and_without_title() {
        assert_eq!(resolve(SIG_SUBJECT, &ctx()).unwrap(), "Dipl.-Ing. Max Mustermann");
        let plain = ExprContext::from_input(&SigBlockInput::new(CertFields::example(), OffsetDateTime::UNIX_EPOCH));
        assert_eq!(resolve(SIG_SUBJECT, &plain).unwrap(), "Max Mustermann");
        // PDF-AS-Default mit `T`
        let t = "${subject.T != null ? (subject.T + \" \") : \"\"}${subject.CN}";
        assert_eq!(resolve(t, &ctx()).unwrap(), "Dipl.-Ing. Max Mustermann");
    }

    #[test]
    fn variables_and_maps() {
        let c = ctx();
        assert_eq!(resolve("${sn}", &c).unwrap(), "1234567890");
        assert_eq!(resolve("${issuer.CN}", &c).unwrap(), "Beispiel-Zertifikat (nur Vorschau)");
        assert_eq!(resolve("${subject.DN}", &c).unwrap(), "CN=Max Mustermann,C=AT");
        assert_eq!(resolve("${sbp.Abteilung}", &c).unwrap(), "IT");
        assert_eq!(resolve("${sbp['Abteilung']}", &c).unwrap(), "IT");
        assert_eq!(resolve("${subject[\"CN\"]}", &c).unwrap(), "Max Mustermann");
        assert_eq!(resolve("${iui.pdfVersion}", &c).unwrap(), "1.7");
        // fehlender Key ⇒ "null" (Java String.valueOf(null))
        assert_eq!(resolve("${subject.O}", &c).unwrap(), "null");
        assert_eq!(resolve("a ${sn} b ${sn}", &c).unwrap(), "a 1234567890 b 1234567890");
        assert_eq!(resolve("ohne Ausdruck", &c).unwrap(), "ohne Ausdruck");
        assert_eq!(resolve("$ {sn} ${", &c).unwrap(), "$ {sn} ${");
    }

    #[test]
    fn list_literal_trick() {
        let c = ctx();
        // Komma auf oberster Ebene: zwei Listenelemente
        assert_eq!(resolve("${'a', 'b'}", &c).unwrap(), "a, b");
        // Sequenz in Klammern: letzter Wert
        assert_eq!(eval_group("{('a', 'b')}", &c).unwrap(), "b");
        assert_eq!(eval_group("{}", &c).unwrap(), "");
        assert_eq!(eval_group("{'[x]'}", &c).unwrap(), "[x]");
        assert_eq!(eval_group("{1 + 2}", &c).unwrap(), "3");
        assert_eq!(eval_group("{sn == '1234567890' ? 'ja' : 'nein'}", &c).unwrap(), "ja");
        assert_eq!(eval_group("{!(subject.O != null) && true}", &c).unwrap(), "true");
    }

    #[test]
    fn unsupported_expressions_are_rejected() {
        let c = ctx();
        for e in
            ["${subject.CN.length()}", "${foo}", "${@java.lang.System@exit(0)}", "${1.5}", "${subject.}", "${'x' == 1}"]
        {
            assert!(matches!(resolve(e, &c), Err(SigBlockError::UnsupportedExpression(_))), "{e}");
        }
    }

    #[test]
    fn regex_semantics_last_brace_before_dollar() {
        let c = ctx();
        // `[^$]*` reicht bis zur letzten `}` vor dem nächsten `$`
        assert!(matches!(resolve("${sn}}x", &c), Err(SigBlockError::UnsupportedExpression(e)) if e == "{sn}}"));
        assert_eq!(resolve("${sn} und ${issuer.C}", &c).unwrap(), "1234567890 und AT");
    }

    #[test]
    fn leftmost_wins_and_title_alias() {
        let mut input = SigBlockInput::new(CertFields::example(), OffsetDateTime::UNIX_EPOCH);
        input.cert.subject =
            vec![("CN".into(), "Erster".into()), ("CN".into(), "Zweiter".into()), ("2.5.4.12".into(), "Mag.".into())];
        let c = ExprContext::from_input(&input);
        assert_eq!(c.subject["CN"], "Erster");
        assert_eq!(c.subject["title"], "Mag.");
        assert_eq!(c.subject["T"], "Mag.");
        input.pdfas_quirks = true;
        input.cert.subject.push(("title".into(), "Dr.".into()));
        let q = ExprContext::from_input(&input);
        assert_eq!(q.subject["T"], "Dr.");
        assert!(q.subject.contains_key("2.5.4.12"));
    }

    #[test]
    fn sig_date_default_pattern() {
        let t = datetime!(2026-10-14 12:34:56.789 +2);
        assert_eq!(format_sig_date(None, t, None).unwrap(), "2026-10-14T12:34:56+02:00");
        assert_eq!(format_sig_date(None, t, Some("UTC")).unwrap(), "2026-10-14T10:34:56Z");
        assert_eq!(format_sig_date(None, t, Some("GMT-07:30")).unwrap(), "2026-10-14T03:04:56-07:30");
        assert_eq!(format_sig_date(None, t, Some("Europe/Vienna")).unwrap(), "2026-10-14T12:34:56+02:00");
        let winter = datetime!(2026-01-14 12:34:56 UTC);
        assert_eq!(format_sig_date(None, winter, Some("Europe/Vienna")).unwrap(), "2026-01-14T13:34:56+01:00");
        // unbekannt ⇒ GMT
        assert_eq!(format_sig_date(None, t, Some("Mars/Olympus")).unwrap(), "2026-10-14T10:34:56Z");
    }

    #[test]
    fn sig_date_patterns() {
        let t = datetime!(2026-03-05 07:08:09.012 +1);
        let f = |p: &str| format_sig_date(Some(p), t, None).unwrap();
        assert_eq!(f("dd.MM.yyyy HH:mm:ss"), "05.03.2026 07:08:09");
        assert_eq!(f("d.M.yy"), "5.3.26");
        assert_eq!(f("y"), "2026");
        assert_eq!(f("hh:mm K k"), "07:08 7 7");
        assert_eq!(f("SSS S"), "012 12");
        assert_eq!(f("X XX XXX Z"), "+01 +0100 +01:00 +0100");
        assert_eq!(f("'Datum: 'dd'' ''"), "Datum: 05' '");
        assert_eq!(f("D u F"), "64 4 1");
        assert_eq!(f("yyyy-MM-dd'T'HH:mm:ssXXX"), "2026-03-05T07:08:09+01:00");
        for p in ["MMM", "EEE", "a", "z", "G", "YYYY", "w", "XXXX", "'offen", "q"] {
            assert!(matches!(format_sig_date(Some(p), t, None), Err(SigBlockError::UnsupportedDatePattern(_))), "{p}");
        }
        let midnight = datetime!(2026-03-05 00:30:00 UTC);
        assert_eq!(format_sig_date(Some("k h K H"), midnight, None).unwrap(), "24 12 0 0");
    }

    #[test]
    fn custom_ids() {
        assert_eq!(custom_gmt_offset("GMT+1").unwrap().whole_seconds(), 3600);
        assert_eq!(custom_gmt_offset("GMT+0130").unwrap().whole_seconds(), 5400);
        assert_eq!(custom_gmt_offset("GMT-130").unwrap().whole_seconds(), -5400);
        assert!(custom_gmt_offset("GMT+24").is_none());
        assert!(custom_gmt_offset("UTC+1").is_none());
    }

    #[test]
    fn latin1_flag() {
        assert_eq!(latin1_repair("PrÃ¼fung"), "Prüfung");
    }
}
