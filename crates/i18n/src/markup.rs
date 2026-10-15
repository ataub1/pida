//! Link-Markup in Katalogtexten (Konvention aus [`crate::convert`], Regel 4).
//!
//! Die Texte aus PDF-Over enthalten SWT-Link-Markup:
//! - `<a>Text</a>` – Ziel ist der Text selbst (URL) oder ein vom Bildschirm festgelegtes Ziel
//!   (z. B. `config-fido2-InfoURL`, `config-DataProtectionURL`, „Jetzt prüfen“ im Update-Status),
//! - `<a href="Ziel">Text</a>` – explizites Ziel (z. B. `mailto:`).
//!
//! Die UI rendert [`Segment::Link`] als Link-Komponente; die CLI gibt [`to_plain`] aus. Andere
//! spitze Klammern bleiben Text.

/// Teil eines Textes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Normaler Text.
    Text(String),
    /// Link.
    Link {
        /// sichtbarer Text
        text: String,
        /// explizites Ziel (`href`), sonst `None`
        href: Option<String>,
    },
}

/// Zerlegt einen Text in Text- und Link-Segmente. Unvollständiges Markup bleibt Text.
pub fn parse(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<a") {
        let after = &rest[start + 2..];
        let Some(open_end) = after.find('>') else { break };
        let attrs = &after[..open_end];
        if !(attrs.is_empty() || attrs.starts_with(char::is_whitespace)) {
            // z. B. "<abbr>": kein Link
            plain.push_str(&rest[..start + 2]);
            rest = after;
            continue;
        }
        let body = &after[open_end + 1..];
        let Some(close) = body.find("</a>") else { break };
        plain.push_str(&rest[..start]);
        if !plain.is_empty() {
            out.push(Segment::Text(std::mem::take(&mut plain)));
        }
        out.push(Segment::Link { text: body[..close].to_owned(), href: parse_href(attrs) });
        rest = &body[close + 4..];
    }
    plain.push_str(rest);
    if !plain.is_empty() {
        out.push(Segment::Text(plain));
    }
    out
}

fn parse_href(attrs: &str) -> Option<String> {
    let i = attrs.find("href")?;
    let v = attrs[i + 4..].trim_start().strip_prefix('=')?.trim_start();
    let quote = v.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let v = &v[1..];
    let end = v.find(quote)?;
    Some(v[..end].to_owned())
}

/// Klartext für Terminal und Logs: Links werden zu ihrem Text; weicht ein explizites Ziel ab
/// (außer `mailto:` + Adresse = Text), folgt es in spitzen Klammern.
pub fn to_plain(text: &str) -> String {
    let mut out = String::new();
    for seg in parse(text) {
        match seg {
            Segment::Text(t) => out.push_str(&t),
            Segment::Link { text, href } => {
                out.push_str(&text);
                if let Some(h) = href
                    && h != text
                    && h.strip_prefix("mailto:") != Some(text.as_str())
                {
                    out.push_str(" <");
                    out.push_str(&h);
                    out.push('>');
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        let s = "Lizenz unter <a>https://x.example/</a>. Support: <a href=\"mailto:a@b.example\">a@b.example</a>.";
        assert_eq!(
            parse(s),
            vec![
                Segment::Text("Lizenz unter ".into()),
                Segment::Link { text: "https://x.example/".into(), href: None },
                Segment::Text(". Support: ".into()),
                Segment::Link { text: "a@b.example".into(), href: Some("mailto:a@b.example".into()) },
                Segment::Text(".".into()),
            ]
        );
        assert_eq!(to_plain(s), "Lizenz unter https://x.example/. Support: a@b.example.");
        assert_eq!(to_plain("<a href='https://y.example'>hier</a>"), "hier <https://y.example>");
    }

    #[test]
    fn non_links_stay_text() {
        assert_eq!(to_plain("a < b > c"), "a < b > c");
        assert_eq!(to_plain("<abbr>x</abbr>"), "<abbr>x</abbr>");
        assert_eq!(to_plain("offen <a>ohne Ende"), "offen <a>ohne Ende");
        assert_eq!(parse(""), vec![]);
    }
}
