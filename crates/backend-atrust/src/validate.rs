//! Eingabeprüfungen der Masken (docs/analyse/01 §4.6.1/§4.6.2). Die UI ruft sie über den Kern auf
//! (keine Fachlogik in TypeScript, Plan §2.2).

use pida_backend_api::FormProblem;

/// Mindestlänge des Passworts (`MobileBKUValidator.java:88`).
pub const PASSWORD_MIN_LEN: usize = 5;
/// Höchstlänge des Passworts (`MobileBKUValidator.java:90`).
pub const PASSWORD_MAX_LEN: usize = 200;
/// Höchstlänge der TAN (`MobileBKUEnterTANComposite.java:66`).
pub const TAN_MAX_LEN: usize = 6;

/// Java `String.length()` (UTF-16-Codeeinheiten).
fn java_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Passwortlänge 5–200 (`MobileBKUValidator`).
pub fn validate_password(password: &str) -> Result<(), FormProblem> {
    let len = java_len(password);
    if len < PASSWORD_MIN_LEN {
        return Err(FormProblem::PasswordTooShort);
    }
    if len > PASSWORD_MAX_LEN {
        return Err(FormProblem::PasswordTooLong);
    }
    Ok(())
}

/// TAN: getrimmt nicht leer; darf kein Präfix des Vergleichswerts sein; höchstens 6 Zeichen.
pub fn validate_tan(tan: &str, reference_value: &str) -> Result<(), FormProblem> {
    let tan = crate::page::java_trim(tan);
    if tan.is_empty() {
        return Err(FormProblem::NoTan);
    }
    if reference_value.starts_with(tan) {
        return Err(FormProblem::EnteredReferenceValue);
    }
    if java_len(tan) > TAN_MAX_LEN {
        return Err(FormProblem::TanTooLong);
    }
    Ok(())
}

/// Live-Warnung ab mehr als 3 Zeichen, wenn der Vergleichswert mit der Eingabe beginnt
/// (`text.length() > 3 && refVal.startsWith(text.trim())`, ungetrimmte Länge).
pub fn tan_live_warning(tan: &str, reference_value: &str) -> Option<FormProblem> {
    (java_len(tan) > 3 && reference_value.starts_with(crate::page::java_trim(tan)))
        .then_some(FormProblem::EnteredReferenceValue)
}

/// Java-`\s` (`[ \t\n\x0B\f\r]`).
fn is_java_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

/// Normalisiert eine Telefonnummer wie `MobileBKUValidator.normalizeMobileNumber` (beim Speichern
/// von `MOBILE_NUMBER` und bei `-n`; gesendet wird der Benutzername **unverändert**).
///
/// Nach `trim()` und Entfernen aller Whitespaces muss `^((\+\d{2})|(00\d{2})|(0)|(10301))([1-9]\d+)$`
/// passen: `00xx…` → `+xx…`, `0…` → `+43…`, `10301…` (A-Trust-Testnummer) bleibt (ohne
/// Whitespace) unverändert. Passt der Ausdruck nicht, kommt der getrimmte Wert zurück
/// (ID-Austria-Benutzername). Wie in Java ersetzt `replace("00", "+")` jedes Vorkommen im
/// Ländercode (`0000…` → `++…`).
pub fn normalize_phone_number(number: &str) -> String {
    let trimmed = crate::page::java_trim(number);
    let compact: String = trimmed.chars().filter(|c| !is_java_whitespace(*c)).collect();
    let rest_ok = |rest: &str| {
        let mut chars = rest.chars();
        matches!(chars.next(), Some('1'..='9')) && {
            let tail = chars.as_str();
            !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit())
        }
    };
    let two_digits = |s: &str| s.len() >= 2 && s.as_bytes()[..2].iter().all(u8::is_ascii_digit);

    // Alternativen in Regex-Reihenfolge (feste Längen, Backtracking über die Alternativen)
    if let Some(after) = compact.strip_prefix('+')
        && two_digits(after)
        && rest_ok(&after[2..])
    {
        let country = compact[..3].replace("00", "+");
        return format!("{country}{}", &compact[3..]);
    }
    if let Some(after) = compact.strip_prefix("00")
        && two_digits(after)
        && rest_ok(&after[2..])
    {
        let country = compact[..4].replace("00", "+");
        return format!("{country}{}", &compact[4..]);
    }
    if let Some(after) = compact.strip_prefix('0')
        && rest_ok(after)
    {
        return format!("+43{after}");
    }
    if let Some(after) = compact.strip_prefix("10301")
        && rest_ok(after)
    {
        return compact;
    }
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_lengths() {
        assert_eq!(validate_password("1234"), Err(FormProblem::PasswordTooShort));
        assert_eq!(validate_password(""), Err(FormProblem::PasswordTooShort));
        assert_eq!(validate_password("12345"), Ok(()));
        assert_eq!(validate_password(&"x".repeat(200)), Ok(()));
        assert_eq!(validate_password(&"x".repeat(201)), Err(FormProblem::PasswordTooLong));
        // Java zählt UTF-16-Einheiten: 3 Emoji = 6 Einheiten
        assert_eq!(validate_password("😀😀😀"), Ok(()));
        assert_eq!(validate_password("äöüß"), Err(FormProblem::PasswordTooShort));
    }

    #[test]
    fn tan_rules() {
        assert_eq!(validate_tan("   ", "AB12"), Err(FormProblem::NoTan));
        assert_eq!(validate_tan("AB", "AB12"), Err(FormProblem::EnteredReferenceValue));
        assert_eq!(validate_tan(" AB12 ", "AB12"), Err(FormProblem::EnteredReferenceValue));
        assert_eq!(validate_tan("1234567", "AB12"), Err(FormProblem::TanTooLong));
        assert_eq!(validate_tan(" 123456 ", "AB12"), Ok(()));
        assert_eq!(validate_tan("x7k2", "AB12"), Ok(()));
        // Reihenfolge wie Java: Präfix-Prüfung vor der Länge
        assert_eq!(validate_tan("AB12xyz", "AB12xyzw"), Err(FormProblem::EnteredReferenceValue));
    }

    #[test]
    fn tan_warning() {
        assert_eq!(tan_live_warning("AB1", "AB12"), None, "erst ab mehr als 3 Zeichen");
        assert_eq!(tan_live_warning("AB12", "AB12"), Some(FormProblem::EnteredReferenceValue));
        assert_eq!(tan_live_warning("AB1 ", "AB12"), Some(FormProblem::EnteredReferenceValue));
        assert_eq!(tan_live_warning("1234", "AB12"), None);
    }

    #[test]
    fn phone_normalization() {
        assert_eq!(normalize_phone_number("0664 1234567"), "+436641234567");
        assert_eq!(normalize_phone_number(" +43 664 123 "), "+43664123");
        assert_eq!(normalize_phone_number("0043664123"), "+43664123");
        assert_eq!(normalize_phone_number("004912345"), "+4912345");
        assert_eq!(normalize_phone_number("10301 12345"), "1030112345");
        assert_eq!(normalize_phone_number("max.mustermann"), "max.mustermann");
        assert_eq!(normalize_phone_number("  benutzer name "), "benutzer name");
        assert_eq!(normalize_phone_number("+4301234"), "+4301234", "Rest muss mit 1–9 beginnen");
        assert_eq!(normalize_phone_number("06"), "06", "mindestens zwei Ziffern nach dem Präfix");
        assert_eq!(normalize_phone_number("0000123"), "++123", "Java replace(\"00\",\"+\") auf den Ländercode");
        assert_eq!(normalize_phone_number("00012"), "00012");
        assert_eq!(normalize_phone_number("+0012345"), "++12345", "auch bei +00");
        assert_eq!(normalize_phone_number("+4366412a"), "+4366412a");
        assert_eq!(normalize_phone_number(""), "");
    }
}
