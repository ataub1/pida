//! Eigenschaftstests für Umbruch und Ausdrücke (proptest).

use std::sync::Arc;

use pdfover_sigblock::layout::break_string;
use pdfover_sigblock::{CertFields, FontProvider, FontSpec, LoadedFont, SigBlockInput, Std14, expr};
use proptest::prelude::*;

fn width(font: &LoadedFont, s: &str, size: f32) -> f32 {
    font.string_width(s).unwrap() / 1000.0 * size
}

fn fonts() -> Vec<(Arc<LoadedFont>, &'static str)> {
    let p = FontProvider::builtin();
    vec![
        (Arc::new(LoadedFont::standard(Std14::HelveticaBold)), "Helvetica-Bold"),
        (Arc::new(LoadedFont::standard(Std14::Courier)), "Courier"),
        (p.load(&FontSpec::parse("TTF:DejaVuSansMono.ttf,5")).unwrap(), "DejaVuSansMono"),
        (p.load(&FontSpec::parse("TTF:DejaVuSansCondensed-Bold.ttf,5")).unwrap(), "DejaVuSansCondensed-Bold"),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Gieriger Umbruch: nichts geht verloren, Zeilen mit mehreren Wörtern passen, und das erste
    /// Wort der Folgezeile hätte nicht mehr gepasst (strikt größer).
    #[test]
    fn greedy_break_invariants(words in prop::collection::vec("[a-zA-Z0-9äöü]{1,14}", 1..30), max in 10.0f32..200.0) {
        let text = words.join(" ");
        for (font, name) in fonts() {
            let lines = break_string(&text, max, &font, 5.0).unwrap();
            prop_assert_eq!(lines.join(" "), text.clone(), "{}", name);
            for (i, l) in lines.iter().enumerate() {
                if l.contains(' ') {
                    prop_assert!(width(&font, l, 5.0) <= max, "{}: {:?} zu breit", name, l);
                }
                if let Some(next) = lines.get(i + 1) {
                    let first = next.split(' ').next().unwrap();
                    let joined = format!("{l} {first}");
                    prop_assert!(width(&font, &joined, 5.0) > max, "{}: {:?} hätte gepasst", name, joined);
                }
            }
        }
    }

    /// Zeilenumbrüche im Text bleiben als Zeilengrenzen erhalten.
    #[test]
    fn hard_breaks_are_kept(parts in prop::collection::vec("[a-z]{1,8}", 1..6)) {
        let text = parts.join("\n");
        let (font, _) = fonts().remove(1);
        let lines = break_string(&text, 1000.0, &font, 5.0).unwrap();
        prop_assert_eq!(lines, parts);
    }

    /// Texte ohne `${` bleiben unverändert; Sonderzeichen stören nicht.
    #[test]
    fn plain_text_passes_through(s in "[^$]{0,60}") {
        let input = SigBlockInput::new(CertFields::example(), time::OffsetDateTime::UNIX_EPOCH);
        let ctx = expr::ExprContext::from_input(&input);
        prop_assert_eq!(expr::resolve(&s, &ctx).unwrap(), s);
    }

    /// Das Default-Datumsmuster ergibt immer 25 Zeichen (bzw. 20 bei UTC mit `Z`).
    #[test]
    fn default_date_has_fixed_length(secs in 0i64..4_000_000_000, off in -50_400i32..50_400) {
        let off = time::UtcOffset::from_whole_seconds(off / 60 * 60).unwrap();
        let t = time::OffsetDateTime::from_unix_timestamp(secs).unwrap().to_offset(off);
        let s = expr::format_sig_date(None, t, None).unwrap();
        prop_assert_eq!(s.len(), if off.is_utc() { 20 } else { 25 }, "{}", s);
    }
}
