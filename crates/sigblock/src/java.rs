//! Java-Semantik, die das Layout bit-genau braucht: `String.split`/`trim`, `Float.parseFloat`,
//! `Double.toString` und die Zahlenausgabe von PDFBox (`NumberFormatUtil.formatFloatFast`).

/// `String.split(<ein Zeichen>)` wie Java: Treffer am Anfang ergeben ein führendes `""`,
/// **nachfolgende** leere Teile werden entfernt; ohne Treffer kommt die Eingabe unverändert zurück
/// (auch `""` → `[""]`).
pub(crate) fn split(s: &str, sep: char) -> Vec<&str> {
    if !s.contains(sep) {
        return vec![s];
    }
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `String.trim()`: entfernt alle Zeichen `<= U+0020` an beiden Enden.
pub(crate) fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `Float.parseFloat` (Teilmenge): Leerraum `<= U+0020` an den Enden, optionales Vorzeichen,
/// Dezimalzahl mit Exponent, optionaler Suffix `f`/`F`/`d`/`D`, `NaN`, `Infinity`.
/// Hexadezimale Gleitkommazahlen werden nicht unterstützt (`None`).
pub(crate) fn parse_float(s: &str) -> Option<f32> {
    let t = trim(s);
    let (sign, body) = match t.as_bytes().first() {
        Some(b'+') => ("", &t[1..]),
        Some(b'-') => ("-", &t[1..]),
        _ => ("", t),
    };
    if body == "NaN" {
        return Some(f32::NAN);
    }
    if body == "Infinity" {
        return Some(if sign == "-" { f32::NEG_INFINITY } else { f32::INFINITY });
    }
    let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-')) {
        return None;
    }
    // Java verlangt mindestens eine Ziffer in Mantisse; Rust akzeptiert die gleiche Grammatik
    // (ohne `inf`/`nan`, die oben schon ausgeschlossen sind).
    let mantissa_end = body.find(['e', 'E']).unwrap_or(body.len());
    if !body[..mantissa_end].bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    format!("{sign}{body}").parse::<f32>().ok()
}

/// `Integer.parseInt`: optionales Vorzeichen, nur ASCII-Ziffern, kein Leerraum.
pub(crate) fn parse_int(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<i32>().ok()
}

/// `Double.toString(d)` (JDK ≥ 19: kürzeste eindeutige Darstellung).
pub(crate) fn double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if d == 0.0 {
        return if d.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let abs = d.abs();
    if (1e-3..1e7).contains(&abs) {
        let s = format!("{d}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        // Computerized scientific notation: d.dddE±n
        let s = format!("{d:e}"); // z. B. "1.5e7", "1e-5"
        let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
        let mant = if mant.contains('.') { mant.to_string() } else { format!("{mant}.0") };
        format!("{mant}E{exp}")
    }
}

/// Zahlenausgabe von PDFBox `PDPageContentStream.writeOperand(float)`
/// (`NumberFormatUtil.formatFloatFast(value, 5, …)`): höchstens 5 Nachkommastellen, kaufmännisch
/// gerundet über `double`, ohne nachfolgende Nullen, ohne Dezimalpunkt bei ganzen Zahlen.
pub(crate) fn pdf_number(value: f32) -> String {
    if !value.is_finite() || value as f64 > i64::MAX as f64 || value as f64 <= i64::MIN as f64 {
        // PDFBox fällt dann auf DecimalFormat zurück; im Block kommt das nicht vor.
        return format!("{value}");
    }
    const POW: i64 = 100_000;
    let mut out = String::new();
    let mut integer_part = value as i64; // Java (long)value: Richtung 0
    if value < 0.0 {
        out.push('-');
        integer_part = -integer_part;
    }
    let mut fraction = (((value as f64).abs() - integer_part as f64) * POW as f64 + 0.5) as i64;
    if fraction >= POW {
        integer_part += 1;
        fraction -= POW;
    }
    out.push_str(&integer_part.to_string());
    if fraction > 0 {
        out.push('.');
        let digits = format!("{fraction:05}");
        out.push_str(digits.trim_end_matches('0'));
    }
    out
}

/// `URLEncoder.encode(s, "UTF-8")` (Quirk Q11).
pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'*' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Java `String.hashCode()` über UTF-16-Einheiten (für deterministische Subset-Präfixe).
pub(crate) fn string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(i32::from(u)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_like_java() {
        assert_eq!(split("a b", ' '), vec!["a", "b"]);
        assert_eq!(split("a  b", ' '), vec!["a", "", "b"]);
        assert_eq!(split(" a", ' '), vec!["", "a"]);
        assert_eq!(split("a ", ' '), vec!["a"]);
        assert_eq!(split("a  ", ' '), vec!["a"]);
        assert_eq!(split("", ' '), vec![""]);
        assert!(split(" ", ' ').is_empty());
        assert!(split("\n\n", '\n').is_empty());
        assert_eq!(split("x\n\ny", '\n'), vec!["x", "", "y"]);
        assert_eq!(split("x\n", '\n'), vec!["x"]);
    }

    #[test]
    fn trim_like_java() {
        assert_eq!(trim("\t a b \u{1}"), "a b");
        assert_eq!(trim("\u{a0}a\u{a0}"), "\u{a0}a\u{a0}");
    }

    #[test]
    fn parse_float_like_java() {
        assert_eq!(parse_float("4"), Some(4.0));
        assert_eq!(parse_float(" 0.1 "), Some(0.1));
        assert_eq!(parse_float("4.5f"), Some(4.5));
        assert_eq!(parse_float("1e2"), Some(100.0));
        assert_eq!(parse_float("-2"), Some(-2.0));
        assert_eq!(parse_float(""), None);
        assert_eq!(parse_float("abc"), None);
        assert_eq!(parse_float("inf"), None);
        assert_eq!(parse_float("."), None);
        assert!(parse_float("NaN").unwrap().is_nan());
        assert_eq!(parse_float("65.16666666666667"), Some(65.166_664));
    }

    #[test]
    fn parse_int_like_java() {
        assert_eq!(parse_int("255"), Some(255));
        assert_eq!(parse_int("-1"), Some(-1));
        assert_eq!(parse_int(" 1"), None);
        assert_eq!(parse_int(""), None);
        assert_eq!(parse_int("1.0"), None);
    }

    #[test]
    fn double_to_string_like_java() {
        assert_eq!(double_to_string(65.16666666666667), "65.16666666666667");
        assert_eq!(double_to_string(65.0), "65.0");
        assert_eq!(double_to_string(230.0), "230.0");
        assert_eq!(double_to_string(1.0e7), "1.0E7");
        assert_eq!(double_to_string(1.5e-5), "1.5E-5");
        assert_eq!(double_to_string(0.001), "0.001");
        assert_eq!(double_to_string(0.0), "0.0");
        assert_eq!(double_to_string(23.0 * (595.0 / 210.0)), "65.16666666666667");
    }

    #[test]
    fn pdf_number_like_pdfbox() {
        assert_eq!(pdf_number(1.0), "1");
        assert_eq!(pdf_number(0.1), "0.1");
        assert_eq!(pdf_number(41.818_18), "41.81818");
        assert_eq!(pdf_number(42.035), "42.035");
        assert_eq!(pdf_number(-5.0), "-5");
        assert_eq!(pdf_number(0.0), "0");
        assert_eq!(pdf_number(229.999_98), "229.99998");
        assert_eq!(pdf_number(42.179_2), "42.1792");
        assert_eq!(pdf_number(-0.5), "-0.5");
        assert_eq!(pdf_number(0.999_999_9), "1");
    }

    #[test]
    fn url_encode_like_java() {
        assert_eq!(url_encode("a b+ü"), "a+b%2B%C3%BC");
        assert_eq!(url_encode("x.y-z*_"), "x.y-z*_");
    }
}
