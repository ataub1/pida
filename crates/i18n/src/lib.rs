//! `pdfover-i18n` – eine Textquelle für UI, CLI und Fehlermeldungen (Plan §2.1, §7.5).
//!
//! - Kataloge in Fluent (`locales/{de,en}/pdfover.ftl`), im Binary eingebettet; die UI nutzt
//!   dieselben `.ftl`-Dateien über `@fluent/bundle`.
//! - Sprachen DE und EN; Fallback-Kette UI-Sprache → System → DE (wie PDF-Over,
//!   docs/analyse/01 §6.1). Die Sprache des Signaturblocks (`SIGNLOCALE`) ist davon getrennt.
//! - Der Kern liefert Fehlercodes mit Parametern; übersetzt wird mit [`t!`] bzw. [`Catalog::format`].
//! - [`convert`]: `messages*.properties` → Fluent (EN 272, DE 268 Keys; Escapes, `&`-Mnemonics,
//!   `%s`/`%d`, `<a>`-Markup; B-09 behoben), als Werkzeug und Test.
//!
//! Ownership: Paket **E** (zusammen mit `pdfover-config` und `pdfover-java-properties`).

pub mod convert;

use std::borrow::Cow;

/// Unterstützte Sprachen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Deutsch (Fallback, wie PDF-Over).
    #[default]
    De,
    /// Englisch.
    En,
}

impl Lang {
    /// Alle Sprachen.
    pub const ALL: [Lang; 2] = [Lang::De, Lang::En];

    /// BCP-47-Tag (`de`, `en`).
    pub fn tag(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }

    /// Aus einem Tag oder Java-Locale-String (`de`, `de_AT`, `en-US` …); unbekannt → `None`.
    pub fn from_tag(tag: &str) -> Option<Lang> {
        let primary = tag.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
        match primary.as_str() {
            "de" => Some(Lang::De),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// Systemsprache, auf DE/EN abgebildet, sonst DE (docs/analyse/01 §6.1 „Default-Sprache“).
    pub fn system() -> Lang {
        todo!("Paket E: Locale des Betriebssystems ermitteln (LANG/LC_ALL, GetUserDefaultLocaleName, CFLocale)")
    }
}

/// Wert eines Platzhalters.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    /// Text
    Str(String),
    /// Zahl
    Num(f64),
}

impl From<&str> for ArgValue {
    fn from(v: &str) -> Self {
        ArgValue::Str(v.to_owned())
    }
}
impl From<String> for ArgValue {
    fn from(v: String) -> Self {
        ArgValue::Str(v)
    }
}
impl From<&String> for ArgValue {
    fn from(v: &String) -> Self {
        ArgValue::Str(v.clone())
    }
}
impl From<i64> for ArgValue {
    fn from(v: i64) -> Self {
        ArgValue::Num(v as f64)
    }
}
impl From<u32> for ArgValue {
    fn from(v: u32) -> Self {
        ArgValue::Num(f64::from(v))
    }
}
impl From<usize> for ArgValue {
    fn from(v: usize) -> Self {
        ArgValue::Num(v as f64)
    }
}
impl From<f64> for ArgValue {
    fn from(v: f64) -> Self {
        ArgValue::Num(v)
    }
}

/// Fehler beim Laden eines Katalogs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum I18nError {
    /// Fluent-Syntaxfehler.
    #[error("Fluent-Syntaxfehler in {lang}: {detail}")]
    Syntax {
        /// Sprache
        lang: &'static str,
        /// Details
        detail: String,
    },
    /// Doppelte Nachricht.
    #[error("doppelte Nachricht {0}")]
    Duplicate(String),
}

/// Übersetzungskatalog (thread-sicher, `concurrent::FluentBundle` je Sprache).
#[derive(Debug)]
pub struct Catalog {
    _private: (),
}

impl Catalog {
    /// Eingebettete Kataloge `locales/de/pdfover.ftl` und `locales/en/pdfover.ftl`.
    pub fn builtin() -> Result<Catalog, I18nError> {
        todo!("Paket E: include_str!(\"../locales/<lang>/pdfover.ftl\")")
    }

    /// Katalog aus eigenen FTL-Quellen (Tests, Overlays).
    pub fn from_sources(sources: &[(Lang, &str)]) -> Result<Catalog, I18nError> {
        todo!("Paket E")
    }

    /// Prozessweiter Standardkatalog (lazy, [`Catalog::builtin`]).
    pub fn global() -> &'static Catalog {
        todo!("Paket E: OnceLock")
    }

    /// `true`, wenn `id` in `lang` (ohne Fallback) existiert.
    pub fn has(&self, lang: Lang, id: &str) -> bool {
        todo!("Paket E")
    }

    /// Formatiert `id` mit Argumenten; Fallback auf DE; fehlt die Nachricht ganz, wird `!id!`
    /// geliefert (wie PDF-Over `Messages.getString`).
    pub fn format(&self, lang: Lang, id: &str, args: &[(&str, ArgValue)]) -> Cow<'static, str> {
        todo!("Paket E")
    }

    /// Attribut einer Nachricht (z. B. `.accesskey` aus den `&`-Mnemonics).
    pub fn attribute(&self, lang: Lang, id: &str, attribute: &str) -> Option<String> {
        todo!("Paket E")
    }
}

/// Übersetzt mit dem globalen Katalog.
pub fn translate(lang: Lang, id: &str, args: &[(&str, ArgValue)]) -> String {
    Catalog::global().format(lang, id, args).into_owned()
}

/// Übersetzungsmakro über den globalen Katalog.
///
/// ```ignore
/// let s = pdfover_i18n::t!(Lang::De, "output-success_message");
/// let s = pdfover_i18n::t!(Lang::En, "error-SLError", code = 6002u32, info = "…");
/// ```
#[macro_export]
macro_rules! t {
    ($lang:expr, $id:expr $(,)?) => {
        $crate::translate($lang, $id, &[])
    };
    ($lang:expr, $id:expr, $($key:ident = $value:expr),+ $(,)?) => {
        $crate::translate($lang, $id, &[$((stringify!($key), $crate::ArgValue::from($value))),+])
    };
}

/// Fluent-ID zu einem Java-Key aus `messages*.properties`: `.` → `-`, andere unzulässige Zeichen
/// → `_` (Fluent erlaubt `[a-zA-Z][a-zA-Z0-9_-]*`). Die Key-Namen bleiben damit nachverfolgbar.
pub fn fluent_id_for_java_key(java_key: &str) -> String {
    let mut out = String::with_capacity(java_key.len());
    for (i, c) in java_key.chars().enumerate() {
        let ok = c.is_ascii_alphanumeric() || (i > 0 && (c == '-' || c == '_'));
        out.push(if c == '.' {
            '-'
        } else if ok {
            c
        } else {
            '_'
        });
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out.insert(0, 'k');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_from_tag() {
        assert_eq!(Lang::from_tag("de_AT"), Some(Lang::De));
        assert_eq!(Lang::from_tag("en-US"), Some(Lang::En));
        assert_eq!(Lang::from_tag("fr"), None);
    }

    #[test]
    fn java_key_mapping() {
        assert_eq!(fluent_id_for_java_key("error.Initialization"), "error-Initialization");
        assert_eq!(
            fluent_id_for_java_key("PlaceholderSelectionGui.lblNewLabel.text"),
            "PlaceholderSelectionGui-lblNewLabel-text"
        );
    }
}
