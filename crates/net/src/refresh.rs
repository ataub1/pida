//! `Refresh`-Header und `<meta http-equiv="refresh">` mit exakt der Semantik von PDF-Over.
//!
//! Java-Quelltext (`MobileBKUConnector.java:188`):
//! `"^\\s*[0-9\\.]+\\s*;\\s*(?:[uU][rR][lL]\s*=\s*)(.+)$"` – die beiden `\s` im URL-Teil haben nur
//! einen Backslash und sind ab Java 15 ein String-Escape für U+0020. Wirksam ist daher
//! `^\s*[0-9.]+\s*;\s*(?:[uU][rR][lL] *= *)(.+)$`, ausgewertet mit `Matcher.matches()`
//! (docs/analyse/02 §3.4). Java-`\s` = `[ \t\n\x0B\f\r]`; `.` passt auf kein Zeilenende
//! (`\n`, `\r`, U+0085, U+2028, U+2029), deshalb scheitert auch ein abschließendes Zeilenende.

/// Java-`\s` ohne `UNICODE_CHARACTER_CLASS`.
fn is_java_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

/// Zeilenenden, auf die Java-`.` nicht passt.
fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Wertet einen `Refresh`-Header bzw. `<meta http-equiv="refresh" content=…>` aus.
///
/// Wirksamer Java-Regex (docs/analyse/02 §3.4): `^\s*[0-9.]+\s*;\s*(?:[uU][rR][lL] *= *)(.+)$`;
/// `URL=` ist Pflicht, Anführungszeichen werden nicht entfernt. `None` bei Nichtübereinstimmung.
pub fn parse_refresh(value: &str) -> Option<String> {
    let mut rest = value.trim_start_matches(is_java_whitespace);
    let delay_len = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    if delay_len == 0 {
        return None;
    }
    rest = rest[delay_len..].trim_start_matches(is_java_whitespace);
    rest = rest.strip_prefix(';')?;
    rest = rest.trim_start_matches(is_java_whitespace);
    let mut chars = rest.chars();
    for expected in ['u', 'r', 'l'] {
        if !chars.next()?.eq_ignore_ascii_case(&expected) {
            return None;
        }
    }
    rest = chars.as_str().trim_start_matches(' ');
    rest = rest.strip_prefix('=')?;
    if rest.is_empty() || rest.contains(is_line_terminator) {
        return None;
    }
    // ` *` ist gierig, `(.+)` braucht mindestens ein Zeichen: besteht der Rest nur aus Leerzeichen,
    // bleibt das letzte Leerzeichen für die Gruppe.
    let group = rest.trim_start_matches(' ');
    Some(if group.is_empty() { " ".to_owned() } else { group.to_owned() })
}

#[cfg(test)]
mod tests {
    use super::parse_refresh;

    #[test]
    fn accepted_forms() {
        assert_eq!(parse_refresh("0; URL=https://x/y").as_deref(), Some("https://x/y"));
        assert_eq!(parse_refresh("0;url=/a.aspx").as_deref(), Some("/a.aspx"));
        assert_eq!(parse_refresh("  1.5 ;  uRl  =   rel?x=1 ").as_deref(), Some("rel?x=1 "));
        assert_eq!(parse_refresh("\n\t0\t;\x0bUrL=a").as_deref(), Some("a"));
        assert_eq!(parse_refresh("...;url=a").as_deref(), Some("a"));
        // Anführungszeichen bleiben (wie Java)
        assert_eq!(parse_refresh("0; url='x.aspx'").as_deref(), Some("'x.aspx'"));
        assert_eq!(parse_refresh("0; url=\"x\"").as_deref(), Some("\"x\""));
        // nur Leerzeichen nach `=`: die Gruppe ist das letzte Leerzeichen
        assert_eq!(parse_refresh("0;url=   ").as_deref(), Some(" "));
        // `=` in der URL
        assert_eq!(parse_refresh("0;url==a=b").as_deref(), Some("=a=b"));
    }

    #[test]
    fn rejected_forms() {
        assert_eq!(parse_refresh("0"), None, "URL= ist Pflicht");
        assert_eq!(parse_refresh("0; https://x"), None);
        assert_eq!(parse_refresh("url=x"), None, "Verzögerung fehlt");
        assert_eq!(parse_refresh("a; url=x"), None);
        assert_eq!(parse_refresh("0, url=x"), None);
        assert_eq!(parse_refresh("0; url\t=x"), None, "nur U+0020 vor `=`");
        assert_eq!(parse_refresh("0; url=\tx").as_deref(), Some("\tx"), "Tab nach `=` gehört zur Gruppe");
        assert_eq!(parse_refresh("0; url="), None);
        assert_eq!(parse_refresh("0; url=x\n"), None, "Zeilenende am Ende");
        assert_eq!(parse_refresh("0; url=x\ny"), None);
        assert_eq!(parse_refresh("0; url=x\u{2028}"), None);
        assert_eq!(parse_refresh("0; uri=x"), None);
        assert_eq!(parse_refresh(""), None);
        assert_eq!(parse_refresh("0; u"), None);
    }
}
