//! Java-Semantik der Lese-Regeln von PDF-Over (`String.trim`, `equalsIgnoreCase`,
//! `Integer.parseInt`, `Double.parseDouble`, `Locale.toString`). Geprüft gegen das Java-Orakel
//! `testdata/config/oracle/LegacyOracle.java`.

/// `String.trim()`: entfernt führende und nachgestellte Zeichen ≤ U+0020 (nicht Unicode-Leerraum).
pub(crate) fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `s == null || s.trim().isEmpty()`
pub(crate) fn is_blank(s: Option<&str>) -> bool {
    s.is_none_or(|s| trim(s).is_empty())
}

/// Einfaches Case-Mapping einer UTF-16-Einheit wie `Character.toUpperCase(char)`.
fn upper(c: u16) -> u16 {
    map_unit(c, |ch| {
        let mut it = ch.to_uppercase();
        match (it.next(), it.next()) {
            (Some(u), None) => u,
            _ => ch,
        }
    })
}

fn lower(c: u16) -> u16 {
    map_unit(c, |ch| {
        let mut it = ch.to_lowercase();
        match (it.next(), it.next()) {
            (Some(l), None) => l,
            _ => ch,
        }
    })
}

fn map_unit(c: u16, f: impl Fn(char) -> char) -> u16 {
    match char::from_u32(u32::from(c)) {
        Some(ch) => {
            let m = f(ch);
            // Nur Abbildungen innerhalb der BMP (wie `char` in Java).
            u16::try_from(u32::from(m)).unwrap_or(c)
        }
        None => c,
    }
}

/// `a.equalsIgnoreCase(b)` (JDK `String.regionMatches(true, …)`): gleiche Länge in UTF-16-Einheiten,
/// je Einheit gleich nach `toUpperCase` oder – danach – `toLowerCase`.
pub(crate) fn equals_ignore_case(a: &str, b: &str) -> bool {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(&b).all(|(&x, &y)| {
        if x == y {
            return true;
        }
        let (ux, uy) = (upper(x), upper(y));
        ux == uy || lower(ux) == lower(uy)
    })
}

/// `Integer.parseInt(s)` für Dezimalziffern (ASCII; Java akzeptiert zusätzlich Unicode-Ziffern
/// anderer Schriften, die in Konfigurationsdateien praktisch nicht vorkommen). Kein Trimmen.
pub(crate) fn parse_int(s: &str) -> Option<i32> {
    let (neg, digits) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut v: i64 = 0;
    for b in digits.bytes() {
        v = v * 10 + i64::from(b - b'0');
        if v > i64::from(i32::MAX) + 1 {
            return None;
        }
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).ok()
}

/// `Double.parseDouble(s)`: trimmt (≤ U+0020), optionales Vorzeichen, `NaN`, `Infinity`,
/// Dezimalzahl mit optionalem Exponenten bzw. Hex-Gleitkommazahl (`0x1.8p1`), jeweils mit optionalem
/// Suffix `f`/`F`/`d`/`D`.
pub(crate) fn parse_double(s: &str) -> Option<f64> {
    let s = trim(s);
    let (neg, body) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    let sign = if neg { -1.0 } else { 1.0 };
    if body == "NaN" {
        return Some(f64::NAN);
    }
    if body == "Infinity" {
        return Some(sign * f64::INFINITY);
    }
    let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
    if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        return parse_hex_float(hex).map(|v| sign * v);
    }
    // Dezimal: digits [. digits] [(e|E) [+-] digits], mindestens eine Ziffer in der Mantisse.
    let bytes = body.as_bytes();
    let mut i = 0usize;
    let mut mantissa_digits = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        mantissa_digits += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            mantissa_digits += 1;
        }
    }
    if mantissa_digits == 0 {
        return None;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i != bytes.len() {
        return None;
    }
    body.parse::<f64>().ok().map(|v| sign * v)
}

fn parse_hex_float(hex: &str) -> Option<f64> {
    // hexdigits [. hexdigits] (p|P) [+-] digits – der binäre Exponent ist Pflicht.
    let (mant, exp) = hex.split_once(['p', 'P'])?;
    let (int_part, frac_part) = mant.split_once('.').unwrap_or((mant, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().chain(frac_part.chars()).all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let exp: i32 = parse_int(exp)?;
    let mut value = 0f64;
    for c in int_part.chars() {
        value = value * 16.0 + f64::from(c.to_digit(16)?);
    }
    let mut scale = 1.0 / 16.0;
    for c in frac_part.chars() {
        value += f64::from(c.to_digit(16)?) * scale;
        scale /= 16.0;
    }
    Some(value * 2f64.powi(exp))
}

/// `Locale.getAvailableLocales()` von JDK 21 (PDF-Over 4.4.x bündelt Temurin 21) als
/// `Locale.toString()`, sortiert; erzeugt mit dem Java-Orakel (`LegacyOracle.java --locales`).
const JAVA21_LOCALES: &str = include_str!("../assets/java21-available-locales.txt");

fn java_locales() -> &'static [&'static str] {
    static LIST: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        let mut v: Vec<&str> = JAVA21_LOCALES.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        v.sort_unstable();
        v
    })
}

/// `LocaleSerializer.parseFromString`: exakter Vergleich mit `Locale.toString()` aller verfügbaren
/// Locales (JDK 21). Liefert die Sprache (`de_AT` → `de`); `de-AT`, `DE` oder `""` → `None`.
pub(crate) fn java_locale_language(s: &str) -> Option<&str> {
    if s.is_empty() || java_locales().binary_search(&s).is_err() {
        return None;
    }
    s.split('_').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_like_java() {
        assert_eq!(trim("\u{1}  a b \t\n"), "a b");
        assert_eq!(trim("\u{a0}a\u{a0}"), "\u{a0}a\u{a0}");
        assert!(is_blank(Some(" \t ")));
        assert!(!is_blank(Some("\u{a0}")));
        assert!(is_blank(None));
    }

    #[test]
    fn equals_ignore_case_like_java() {
        assert!(equals_ignore_case("FALSE", "false"));
        assert!(equals_ignore_case("False", "false"));
        assert!(!equals_ignore_case("false ", "false"));
        // U+017F (langes s) wird in Java zu 'S' großgeschrieben.
        assert!(equals_ignore_case("fal\u{17f}e", "false"));
        // U+0130 (İ) → toLowerCase ergibt 'i' (Einzelzeichen-Abbildung in Java: 'i').
        assert!(!equals_ignore_case("tr\u{fc}e", "true"));
    }

    #[test]
    fn parse_int_like_java() {
        assert_eq!(parse_int("8080"), Some(8080));
        assert_eq!(parse_int("+8080"), Some(8080));
        assert_eq!(parse_int("-1"), Some(-1));
        assert_eq!(parse_int("08080"), Some(8080));
        assert_eq!(parse_int(" 8080"), None);
        assert_eq!(parse_int("8080 "), None);
        assert_eq!(parse_int("2147483647"), Some(i32::MAX));
        assert_eq!(parse_int("2147483648"), None);
        assert_eq!(parse_int("-2147483648"), Some(i32::MIN));
        assert_eq!(parse_int(""), None);
        assert_eq!(parse_int("+"), None);
        assert_eq!(parse_int("80a"), None);
    }

    #[test]
    fn parse_double_like_java() {
        assert_eq!(parse_double("23"), Some(23.0));
        assert_eq!(parse_double(" 23.5 "), Some(23.5));
        assert_eq!(parse_double("23.0d"), Some(23.0));
        assert_eq!(parse_double("1e1"), Some(10.0));
        assert_eq!(parse_double(".5"), Some(0.5));
        assert_eq!(parse_double("5."), Some(5.0));
        assert_eq!(parse_double("-2.5E-1f"), Some(-0.25));
        assert_eq!(parse_double("0x1.8p1"), Some(3.0));
        assert!(parse_double("NaN").unwrap().is_nan());
        assert_eq!(parse_double("-Infinity"), Some(f64::NEG_INFINITY));
        assert_eq!(parse_double("inf"), None);
        assert_eq!(parse_double("23,5"), None);
        assert_eq!(parse_double("e5"), None);
        assert_eq!(parse_double("."), None);
        assert_eq!(parse_double("1e"), None);
        assert_eq!(parse_double(""), None);
        assert_eq!(parse_double("0x10"), None);
    }

    #[test]
    fn java_locales() {
        assert_eq!(java_locale_language("de"), Some("de"));
        assert_eq!(java_locale_language("de_AT"), Some("de"));
        assert_eq!(java_locale_language("es_419"), Some("es"));
        assert_eq!(java_locale_language("sr_RS_#Latn"), Some("sr"));
        assert_eq!(java_locale_language("fr"), Some("fr"));
        assert_eq!(java_locale_language("xx"), None);
        assert_eq!(java_locale_language("de_XX"), None);
        assert_eq!(java_locale_language("de-AT"), None);
        assert_eq!(java_locale_language("DE"), None);
        assert_eq!(java_locale_language("de_at"), None);
        assert_eq!(java_locale_language(""), None);
        assert_eq!(java_locale_language("german"), None);
    }
}
