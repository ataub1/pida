//! `pdfover-i18n` – eine Textquelle für UI, CLI und Fehlermeldungen (Plan §2.1, §7.5).
//!
//! - Kataloge in Fluent, im Binary eingebettet; die UI nutzt dieselben `.ftl`-Dateien über
//!   `@fluent/bundle`:
//!   - `locales/{de,en}/pdfover.ftl`: **erzeugt** aus `messages*.properties` von PDF-Over
//!     ([`convert::convert_pdfover_messages`], nie von Hand bearbeiten),
//!   - `locales/{de,en}/pdfover-ng.ftl`: eigene Texte von PDF-Over NG (Präfix `ng-`; neue Meldungs-IDs
//!     anderer Pakete trägt die Integration hier ein, ADR 0001 Regel 11).
//! - Sprachen DE und EN; Fallback-Kette UI-Sprache → System ([`Lang::system`]) → DE (wie PDF-Over,
//!   docs/analyse/01 §6.1). Fehlt eine Nachricht in der gewählten Sprache, gilt DE, danach EN, danach
//!   `!id!` (wie `Messages.getString`). Die Sprache des Signaturblocks (`SIGNLOCALE`) ist davon getrennt.
//! - Der Kern liefert Fehlercodes mit Parametern; übersetzt wird mit [`t!`] bzw. [`Catalog::format`].
//!   Platzhalter aus PDF-Over heißen `$arg1`, `$arg2` … in der Reihenfolge der Java-`%s`/`%d`.
//! - [`convert`]: `messages*.properties` → Fluent (EN 272, DE 268 Keys; Escapes, `&`-Mnemonics,
//!   `%s`/`%d`, `<a>`-Markup; B-09 behoben), als Werkzeug und Test.
//! - [`markup`]: Link-Markup `<a>…</a>` in Segmente bzw. Klartext (CLI).
//!
//! Ownership: Paket **E** (zusammen mit `pdfover-config` und `pdfover-java-properties`).

pub mod convert;
pub mod markup;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Unterstützte Sprachen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Deutsch (Fallback, wie PDF-Over).
    #[default]
    De,
    /// Englisch.
    En,
}

static SYSTEM_OVERRIDE: RwLock<Option<Lang>> = RwLock::new(None);

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
        let primary = tag.split(['-', '_', '.', '@']).next().unwrap_or("").to_ascii_lowercase();
        match primary.as_str() {
            "de" => Some(Lang::De),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// Systemsprache, auf DE/EN abgebildet, sonst DE (docs/analyse/01 §6.1 „Default-Sprache“).
    ///
    /// Reihenfolge: von der Shell gesetzte Sprache ([`Lang::set_system`], z. B. aus
    /// `GetUserDefaultLocaleName`/`CFLocale`/Android-Locale), sonst die POSIX-Variablen `LC_ALL`,
    /// `LC_MESSAGES`, `LANG` (wie die JVM), sonst DE.
    pub fn system() -> Lang {
        if let Some(lang) = SYSTEM_OVERRIDE.read().ok().and_then(|g| *g) {
            return lang;
        }
        Lang::system_from_env(|name| std::env::var(name).ok())
    }

    /// Setzt (bzw. löscht mit `None`) die Systemsprache, die die Shell aus dem Betriebssystem ermittelt
    /// hat (unter Windows/macOS-GUI fehlen die POSIX-Variablen meist).
    pub fn set_system(lang: Option<Lang>) {
        if let Ok(mut g) = SYSTEM_OVERRIDE.write() {
            *g = lang;
        }
    }

    /// Systemsprache aus Umgebungsvariablen (testbar): erster nicht leerer Wert von `LC_ALL`,
    /// `LC_MESSAGES`, `LANG`. `C`/`POSIX` (auch `C.UTF-8`) ergeben EN – die JVM unter Linux bildet sie
    /// auf `en_US` ab, PDF-Over zeigt dort also Englisch; unbekannte Sprachen und keine Variable ergeben
    /// DE (`Messages.getDefaultLocale`).
    pub fn system_from_env<F: Fn(&str) -> Option<String>>(get: F) -> Lang {
        let Some(value) =
            ["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|name| get(name)).find(|v| !v.trim().is_empty())
        else {
            return Lang::De;
        };
        let value = value.trim();
        let base = value.split(['.', '@']).next().unwrap_or("");
        if base == "C" || base == "POSIX" {
            return Lang::En;
        }
        Lang::from_tag(value).unwrap_or(Lang::De)
    }

    /// Effektive UI-Sprache: Einstellung, sonst System (Fallback DE).
    pub fn effective(setting: Option<Lang>) -> Lang {
        setting.unwrap_or_else(Lang::system)
    }

    fn langid(self) -> LanguageIdentifier {
        LanguageIdentifier::from_str(self.tag()).unwrap_or_default()
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

/// Eingebettete Quellen je Sprache (Reihenfolge: erzeugter PDF-Over-Katalog, eigene Texte).
pub const BUILTIN_SOURCES: &[(Lang, &str, &str)] = &[
    (Lang::De, "de/pdfover.ftl", include_str!("../locales/de/pdfover.ftl")),
    (Lang::De, "de/pdfover-ng.ftl", include_str!("../locales/de/pdfover-ng.ftl")),
    (Lang::En, "en/pdfover.ftl", include_str!("../locales/en/pdfover.ftl")),
    (Lang::En, "en/pdfover-ng.ftl", include_str!("../locales/en/pdfover-ng.ftl")),
];

/// Übersetzungskatalog (thread-sicher, `concurrent::FluentBundle` je Sprache).
pub struct Catalog {
    bundles: BTreeMap<Lang, FluentBundle<FluentResource>>,
    ids: BTreeMap<Lang, BTreeSet<String>>,
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let counts: BTreeMap<&str, usize> = self.ids.iter().map(|(l, ids)| (l.tag(), ids.len())).collect();
        f.debug_struct("Catalog").field("messages", &counts).finish()
    }
}

impl Catalog {
    /// Eingebettete Kataloge `locales/<lang>/pdfover.ftl` und `locales/<lang>/pdfover-ng.ftl`.
    pub fn builtin() -> Result<Catalog, I18nError> {
        let sources: Vec<(Lang, &str)> = BUILTIN_SOURCES.iter().map(|(l, _, s)| (*l, *s)).collect();
        Catalog::from_sources(&sources)
    }

    /// Katalog aus eigenen FTL-Quellen (Tests, Overlays); mehrere Quellen je Sprache sind erlaubt,
    /// dieselbe Nachricht zweimal ist ein Fehler.
    pub fn from_sources(sources: &[(Lang, &str)]) -> Result<Catalog, I18nError> {
        let mut bundles: BTreeMap<Lang, FluentBundle<FluentResource>> = BTreeMap::new();
        let mut ids: BTreeMap<Lang, BTreeSet<String>> = BTreeMap::new();
        for lang in Lang::ALL {
            let mut bundle = FluentBundle::new_concurrent(vec![lang.langid()]);
            // Keine Unicode-Isolationszeichen (FSI/PDI) um Platzhalter: CLI-Ausgabe, Logs, Dateinamen.
            bundle.set_use_isolating(false);
            bundles.insert(lang, bundle);
            ids.insert(lang, BTreeSet::new());
        }
        for (lang, source) in sources {
            let resource = FluentResource::try_new((*source).to_owned()).map_err(|(_, errors)| I18nError::Syntax {
                lang: lang.tag(),
                detail: errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "),
            })?;
            let mut new_ids = Vec::new();
            for entry in resource.entries() {
                if let fluent_syntax::ast::Entry::Message(m) = entry {
                    new_ids.push(m.id.name.to_owned());
                }
            }
            let bundle = bundles.get_mut(lang).expect("alle Sprachen angelegt");
            if let Err(errors) = bundle.add_resource(resource) {
                let detail = errors
                    .iter()
                    .map(|e| match e {
                        fluent_bundle::FluentError::Overriding { id, .. } => id.clone(),
                        other => other.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(I18nError::Duplicate(detail));
            }
            ids.get_mut(lang).expect("alle Sprachen angelegt").extend(new_ids);
        }
        Ok(Catalog { bundles, ids })
    }

    /// Leerer Katalog (jede Anfrage liefert `!id!`).
    pub fn empty() -> Catalog {
        Catalog::from_sources(&[]).unwrap_or_else(|_| unreachable!("leere Quellen sind gültig"))
    }

    /// Prozessweiter Standardkatalog (lazy, [`Catalog::builtin`]).
    pub fn global() -> &'static Catalog {
        static GLOBAL: OnceLock<Catalog> = OnceLock::new();
        // Die eingebetteten Kataloge werden in den Tests geprüft; ein Fehler darf die App nicht beenden.
        GLOBAL.get_or_init(|| Catalog::builtin().unwrap_or_else(|_| Catalog::empty()))
    }

    /// `true`, wenn `id` in `lang` (ohne Fallback) existiert.
    pub fn has(&self, lang: Lang, id: &str) -> bool {
        self.ids.get(&lang).is_some_and(|s| s.contains(id))
    }

    /// Alle Nachrichten-IDs einer Sprache (sortiert).
    pub fn message_ids(&self, lang: Lang) -> impl Iterator<Item = &str> + '_ {
        self.ids.get(&lang).into_iter().flatten().map(String::as_str)
    }

    fn chain(lang: Lang) -> impl Iterator<Item = Lang> {
        let mut v = vec![lang];
        for l in [Lang::De, Lang::En] {
            if !v.contains(&l) {
                v.push(l);
            }
        }
        v.into_iter()
    }

    /// Formatiert `id` mit Argumenten; Fallback auf DE (dann EN); fehlt die Nachricht ganz, wird
    /// `!id!` geliefert (wie PDF-Over `Messages.getString`). Fehlende Argumente erscheinen als
    /// `{$name}` (Fluent-Verhalten).
    pub fn format(&self, lang: Lang, id: &str, args: &[(&str, ArgValue)]) -> Cow<'static, str> {
        let fargs = to_fluent_args(args);
        for l in Catalog::chain(lang) {
            let Some(bundle) = self.bundles.get(&l) else { continue };
            let Some(message) = bundle.get_message(id) else { continue };
            let Some(pattern) = message.value() else { continue };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, Some(&fargs), &mut errors);
            return Cow::Owned(text.into_owned());
        }
        Cow::Owned(format!("!{id}!"))
    }

    /// Attribut einer Nachricht (z. B. `.accesskey` aus den `&`-Mnemonics); ohne Sprach-Fallback, weil
    /// Zugriffstasten zum Text der jeweiligen Sprache gehören.
    pub fn attribute(&self, lang: Lang, id: &str, attribute: &str) -> Option<String> {
        let bundle = self.bundles.get(&lang)?;
        let message = bundle.get_message(id)?;
        let attr = message.get_attribute(attribute)?;
        let mut errors = Vec::new();
        Some(bundle.format_pattern(attr.value(), None, &mut errors).into_owned())
    }
}

fn to_fluent_args<'a>(args: &'a [(&'a str, ArgValue)]) -> FluentArgs<'a> {
    let mut fargs = FluentArgs::with_capacity(args.len());
    for (name, value) in args {
        match value {
            ArgValue::Str(s) => fargs.set(*name, FluentValue::from(s.as_str())),
            ArgValue::Num(n) => fargs.set(*name, FluentValue::from(*n)),
        }
    }
    fargs
}

/// Übersetzt mit dem globalen Katalog.
pub fn translate(lang: Lang, id: &str, args: &[(&str, ArgValue)]) -> String {
    Catalog::global().format(lang, id, args).into_owned()
}

/// Übersetzungsmakro über den globalen Katalog.
///
/// ```
/// use pdfover_i18n::{Lang, t};
/// assert_eq!(t!(Lang::De, "output-success_message"), "Signatur war erfolgreich");
/// assert_eq!(
///     t!(Lang::En, "error-SLError", arg1 = 6002u32, arg2 = "Abbruch"),
///     "Operation aborted by signature server (6002):\nAbbruch"
/// );
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
        assert_eq!(Lang::from_tag("de_AT.UTF-8"), Some(Lang::De));
        assert_eq!(Lang::from_tag("fr"), None);
    }

    #[test]
    fn java_key_mapping() {
        assert_eq!(fluent_id_for_java_key("error.Initialization"), "error-Initialization");
        assert_eq!(
            fluent_id_for_java_key("PlaceholderSelectionGui.lblNewLabel.text"),
            "PlaceholderSelectionGui-lblNewLabel-text"
        );
        assert_eq!(fluent_id_for_java_key("1x y"), "k1x_y");
    }

    #[test]
    fn system_language_from_env() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| (*v).to_owned())
        };
        assert_eq!(Lang::system_from_env(env(&[("LANG", "en_US.UTF-8")])), Lang::En);
        assert_eq!(Lang::system_from_env(env(&[("LANG", "en_US.UTF-8"), ("LC_ALL", "de_AT.UTF-8")])), Lang::De);
        assert_eq!(Lang::system_from_env(env(&[("LC_MESSAGES", "en_GB"), ("LANG", "de_DE")])), Lang::En);
        assert_eq!(Lang::system_from_env(env(&[("LANG", "fr_FR.UTF-8")])), Lang::De);
        assert_eq!(Lang::system_from_env(env(&[("LANG", "C")])), Lang::En);
        assert_eq!(Lang::system_from_env(env(&[("LANG", "POSIX")])), Lang::En);
        assert_eq!(Lang::system_from_env(env(&[("LC_ALL", "C.UTF-8"), ("LANG", "de_AT.UTF-8")])), Lang::En);
        assert_eq!(Lang::system_from_env(env(&[("LANG", "Cy_GB")])), Lang::De);
        assert_eq!(Lang::system_from_env(env(&[("LC_ALL", ""), ("LANG", "en")])), Lang::En);
        assert_eq!(Lang::system_from_env(env(&[])), Lang::De);
    }

    #[test]
    fn catalog_fallback_and_missing() {
        let c = Catalog::from_sources(&[
            (Lang::De, "a = A-de\nb = B-de { $arg1 }\n"),
            (Lang::En, "a = A-en\nonly-en = nur EN\n    .accesskey = n\n"),
        ])
        .unwrap();
        assert_eq!(c.format(Lang::En, "a", &[]), "A-en");
        assert_eq!(c.format(Lang::En, "b", &[("arg1", ArgValue::Num(3.0))]), "B-de 3");
        assert_eq!(c.format(Lang::De, "only-en", &[]), "nur EN");
        assert_eq!(c.format(Lang::De, "nope", &[]), "!nope!");
        assert!(c.has(Lang::En, "a") && !c.has(Lang::En, "b"));
        assert_eq!(c.attribute(Lang::En, "only-en", "accesskey").as_deref(), Some("n"));
        assert_eq!(c.attribute(Lang::De, "only-en", "accesskey"), None);
        assert!(format!("{c:?}").contains("messages"));
    }

    #[test]
    fn catalog_rejects_duplicates_and_syntax_errors() {
        assert!(matches!(
            Catalog::from_sources(&[(Lang::De, "a = 1\n"), (Lang::De, "a = 2\n")]),
            Err(I18nError::Duplicate(_))
        ));
        assert!(matches!(Catalog::from_sources(&[(Lang::En, "a = {\n")]), Err(I18nError::Syntax { .. })));
    }

    #[test]
    fn builtin_catalog_loads() {
        let c = Catalog::builtin().expect("eingebettete Kataloge gültig");
        assert!(c.message_ids(Lang::De).count() > 270);
        assert_eq!(c.format(Lang::De, "common-Cancel", &[]), "Abbrechen");
        assert_eq!(c.attribute(Lang::De, "common-Cancel", "accesskey").as_deref(), Some("A"));
        assert_eq!(t!(Lang::En, "positioning-page", arg1 = 1u32, arg2 = 3u32), "Page 1 of 3");
    }
}
