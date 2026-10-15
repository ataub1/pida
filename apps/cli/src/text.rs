//! Texte der Kommandozeile (DE/EN) – ausschließlich aus den Fluent-Katalogen
//! (`crates/i18n/locales/{de,en}/*.ftl`, eine Quelle für Kern, CLI und Oberfläche; keine
//! Doppelpflege). Neue IDs trägt die Integration dort ein (ADR 0001 Regel 11).

use std::collections::BTreeMap;

use pdfover_i18n::{ArgValue, Catalog, Lang, markup};

/// Text zu einer Meldungs-ID aus dem Katalog (Codes mit Punkt wie Java-Keys: `.` → `-`), sonst
/// `None`.
pub fn lookup(lang: Lang, id: &str, args: &BTreeMap<String, String>) -> Option<String> {
    let catalog = Catalog::global();
    let fluent_id = pdfover_i18n::fluent_id_for_java_key(id);
    for candidate in [id, fluent_id.as_str()] {
        if catalog.has(lang, candidate) || catalog.has(Lang::De, candidate) {
            let fargs: Vec<(&str, ArgValue)> =
                args.iter().map(|(k, v)| (k.as_str(), ArgValue::Str(v.clone()))).collect();
            return Some(markup::to_plain(&catalog.format(lang, candidate, &fargs)));
        }
    }
    None
}

/// Text zu einer ID; unbekannte IDs erscheinen als `id (detail)`.
pub fn tr(lang: Lang, id: &str, args: &[(&str, String)]) -> String {
    let map: BTreeMap<String, String> = args.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
    tr_map(lang, id, &map)
}

/// Wie [`tr`] mit Parameter-Map (Fehler des Kerns).
pub fn tr_map(lang: Lang, id: &str, args: &BTreeMap<String, String>) -> String {
    match lookup(lang, id, args) {
        Some(t) => t,
        None => match args.get("detail").filter(|d| !d.is_empty()) {
            Some(d) => format!("{id} ({d})"),
            None => id.to_owned(),
        },
    }
}

/// Antwort „ja“ (j/ja/y/yes, ohne Groß-/Kleinschreibung).
pub fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "j" | "ja" | "y" | "yes")
}

/// Zweisprachige Hilfe (`-h`, `-?`, `--help`; F-108), in der Systemsprache.
pub fn help(lang: Lang) -> String {
    let t = |id: &str| tr(lang, id, &[]);
    let legacy: &[(&str, &str)] = &[
        ("-h, -?, --help", "argument-help-help"),
        ("-i <datei>", "argument-help-input"),
        ("-o <ordner|datei>", "argument-help-output"),
        ("-b <LOCAL|MOBILE|KS|NONE>", "argument-help-bku"),
        ("-n <nummer>", "argument-help-number"),
        ("-p <passwort>", "argument-help-password"),
        ("-ks <datei>", "argument-help-keystorefile"),
        ("-kstype <typ>", "argument-help-keystoretype"),
        ("-kspass <passwort>", "argument-help-keystorestorepass"),
        ("-ksalias <alias>", "argument-help-keystorealias"),
        ("-kskeypass <passwort>", "argument-help-keystorekeypass"),
        ("-proxy <host>", "argument-help-proxyhost"),
        ("-proxyport <port>", "argument-help-proxyport"),
        ("-proxyuser <benutzer>", "argument-help-proxyuser"),
        ("-proxypass <passwort>", "argument-help-proxypass"),
        ("-e <bild>", "argument-help-emblem"),
        ("-a", "argument-help-autopos"),
        ("-s", "argument-help-skipfinish"),
        ("-c <datei>", "argument-help-config"),
        ("-v <wert>", "argument-help-vis"),
    ];
    let modern: &[(&str, &str)] = &[
        ("--headless", "ng-cli-help-headless"),
        ("--json", "ng-cli-help-json"),
        ("--kspass-stdin", "ng-cli-help-kspass-stdin"),
        ("--kspass-env <variable>", "ng-cli-help-kspass-env"),
        ("--log-level <stufe>", "ng-cli-help-log-level"),
        ("--version", "ng-cli-help-version"),
    ];
    let commands: &[(&str, &str)] = &[
        ("sign <eingaben…>", "ng-cli-help-cmd-sign"),
        ("verify <datei>", "ng-cli-help-cmd-verify"),
        ("profiles", "ng-cli-help-cmd-profiles"),
        ("migrate", "ng-cli-help-cmd-migrate"),
        ("render-block", "ng-cli-help-cmd-render-block"),
    ];
    let width = legacy.iter().chain(modern).chain(commands).map(|(o, _)| o.chars().count()).max().unwrap_or(0) + 2;
    let mut out = String::new();
    let mut section = |title: &str, rows: &[(&str, &str)]| {
        out.push_str(title);
        out.push('\n');
        for (opt, id) in rows {
            let pad = width - opt.chars().count();
            out.push_str(&format!("  {opt}{}{}\n", " ".repeat(pad), t(id).replace('\n', " ")));
        }
        out.push('\n');
    };
    let header = format!("{}\n{}\n\n", t("ng-cli-help-title"), t("ng-cli-help-usage"));
    section(&format!("{header}{}", t("ng-cli-help-options")), legacy);
    section(&t("ng-cli-help-new-options"), modern);
    section(&t("ng-cli-help-commands"), commands);
    out.push_str(&t("ng-cli-help-exit-codes"));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_in_both_languages() {
        let de = help(Lang::De);
        let en = help(Lang::En);
        assert!(de.contains("Optionen"), "{de}");
        assert!(en.contains("Options"), "{en}");
        for opt in ["-kskeypass", "-proxyport", "--kspass-env", "render-block", "-v <wert>"] {
            assert!(de.contains(opt) && en.contains(opt), "{opt}");
        }
        // B-08: Hilfetexte von -kspass/-kskeypass richtig zugeordnet
        let line = |text: &str, opt: &str| text.lines().find(|l| l.trim_start().starts_with(opt)).unwrap().to_owned();
        assert!(line(&en, "-kskeypass").to_lowercase().contains("key password"), "{}", line(&en, "-kskeypass"));
        assert!(!line(&en, "-kspass").to_lowercase().contains("key password"), "{}", line(&en, "-kspass"));
        assert!(!de.contains("!argument"), "fehlende Katalogeinträge");
    }

    /// Eingabeaufforderungen behalten das Leerzeichen am Ende (`{ " " }` im Katalog).
    #[test]
    fn prompts_keep_trailing_space() {
        assert_eq!(tr(Lang::De, "ng-cli-password", &[]), "Passwort: ");
        assert_eq!(tr(Lang::En, "ng-cli-retry", &[]), "Retry? [y/N] ");
        assert_eq!(tr(Lang::De, "ng-cli-overwrite", &[("name", "a.pdf".into())]), "Datei a.pdf existiert bereits. Überschreiben? [j/N] ");
    }

    #[test]
    fn translation_fallbacks() {
        assert_eq!(
            tr(Lang::En, "ng-cli-kspass-env-missing", &[("name", "X".into())]),
            "The environment variable X is not set."
        );
        assert!(tr(Lang::De, "error-FileNotExist", &[("arg1", "/a".into())]).contains("/a"));
        let mut m = BTreeMap::new();
        m.insert("detail".to_owned(), "x".to_owned());
        assert_eq!(tr_map(Lang::De, "gibt-es-nicht", &m), "gibt-es-nicht (x)");
        assert!(is_yes(" J ") && is_yes("yes") && !is_yes("n") && !is_yes(""));
    }
}
