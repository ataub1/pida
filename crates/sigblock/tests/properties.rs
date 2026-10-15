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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// Beliebige Ausdrücke (Hinweistexte sind Nutzereingaben) führen nie zu einer Panic.
    #[test]
    fn expressions_never_panic(body in r#"[a-zA-Z0-9 ${}()\[\]'"!?:,.+=&|\\_-]{0,80}"#) {
        let input = SigBlockInput::new(CertFields::example(), time::OffsetDateTime::UNIX_EPOCH);
        let ctx = expr::ExprContext::from_input(&input);
        let _ = expr::resolve(&body, &ctx);
        let _ = expr::resolve(&format!("${{{body}}}"), &ctx);
    }

    /// Beliebige `SimpleDateFormat`-Muster und Zeitzonen-IDs führen nie zu einer Panic.
    #[test]
    fn date_patterns_never_panic(
        pattern in "[a-zA-Z' :.,/+-]{0,40}",
        tz in prop::option::of("[A-Za-z/+:0-9_-]{0,20}"),
        secs in -62_000_000_000i64..253_000_000_000,
    ) {
        let t = time::OffsetDateTime::from_unix_timestamp(secs).unwrap();
        let _ = expr::format_sig_date(Some(&pattern), t, tz.as_deref());
    }

    /// Fontbeschreibungen: `parse` fällt immer auf einen gültigen Font zurück.
    #[test]
    fn font_specs_never_panic(desc in "[A-Za-z_:.,0-9 -]{0,40}") {
        let spec = FontSpec::parse(&desc);
        prop_assert!(spec.size().is_finite() || FontSpec::try_parse(&desc).is_ok());
    }

    /// Tabellenzeilen und Stile aus beliebigen Profilwerten: Laden und Layout ohne Panic.
    #[test]
    fn arbitrary_profiles_never_panic(
        row1 in r"[A-Z_]{1,8}-(i|v|c|cv|vc|req|x)(\|(TABLE-sub|[A-Z_]{1,8}-(v|c|cv)))?",
        row2 in r"(TABLE-sub|SIG_NOTE-cv|[A-Z_]{1,6}-[a-z]{1,3})",
        cols in r"(-?[0-9]{1,2}(\.[0-9])?)( -?[0-9]{1,2}(\.[0-9])?){0,3}",
        padding in r"-?[0-9]{1,3}(\.[0-9]{1,2})?",
        border in r"-?[0-9]{1,2}(\.[0-9])?",
        scale in prop::option::of(r"[0-9]{1,3};[0-9]{1,3}"),
        size in r"-?[0-9]{1,2}",
        width in -10.0f32..800.0,
    ) {
        use pdfover_java_properties::Properties;
        use pdfover_sigblock::{Appearance, BuildOptions, BuiltinResources, PdfaMode, ProfileStore};
        let mut props = Properties::new();
        let font = format!("HELVETICA,{size},BOLD");
        let mut entries = vec![
            ("sig_obj.types.T", "on".to_string()),
            ("sig_obj.T.table.main.1", row1.clone()),
            ("sig_obj.T.table.main.2", row2.clone()),
            ("sig_obj.T.table.main.ColsWidth", cols.clone()),
            ("sig_obj.T.table.main.Style.padding", padding.clone()),
            ("sig_obj.T.table.main.Style.border", border.clone()),
            ("sig_obj.T.table.main.Style.font", font),
            ("sig_obj.T.table.main.Style.valuefont", "TTF:DejaVuSansMono.ttf,5".to_string()),
            ("sig_obj.T.table.sub.1", "SIG_DATE-cv".to_string()),
            ("sig_obj.T.value.SIG_LABEL", "./images/signatur-logo_de.png".to_string()),
            ("sig_obj.T.value.SIG_NOTE", "Hinweis ${subject.CN}".to_string()),
        ];
        if let Some(s) = &scale {
            entries.push(("sig_obj.T.table.main.Style.imagescaletofit", s.clone()));
        }
        for (k, v) in &entries {
            props.insert(*k, v.as_str());
        }
        let store = ProfileStore::from_properties(&props, Arc::new(BuiltinResources::default())).unwrap();
        let input = SigBlockInput::new(CertFields::example(), time::OffsetDateTime::UNIX_EPOCH);
        let opts = BuildOptions { width, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
        let p = store.get("T").unwrap();
        if let Ok(a) = Appearance::build(p, &input, &opts, &FontProvider::builtin(), &**store.resources()) {
            let pdfover_pdf_cos::Object::Stream(inner) = &a.objects[a.inner] else { panic!() };
            let text = String::from_utf8_lossy(&inner.content);
            prop_assert!(!text.contains("NaN") && !text.contains("inf"), "{}", text);
            prop_assert!(a.width.is_finite() && a.height.is_finite());
        }
    }
}
