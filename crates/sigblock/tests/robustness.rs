//! Robustheit gegen fehlerhafte und bösartige Eingaben (Review Paket C): keine Panics, kein
//! Stapelüberlauf, keine ungültigen Zahlen im Content-Stream, begrenzte Include-Auflösung,
//! redigierende `Debug`-Ausgaben (ADR 0001 Regel 9).

use std::sync::Arc;
use std::time::{Duration, Instant};

use pdfover_java_properties::Properties;
use pdfover_pdf_cos::Object;
use pdfover_sigblock::{
    Appearance, BuildOptions, BuiltinResources, CertFields, Emblem, FontProvider, PdfaMode, ProfileOverrides,
    ProfileStore, SigBlockError, SigBlockInput, layout,
};
use time::macros::datetime;

fn input() -> SigBlockInput {
    SigBlockInput::new(CertFields::example(), datetime!(2026-10-14 12:34:56 +2))
}

/// Eigenes Minimalprofil `T` (eine Wertzeile) plus `extra`.
fn custom(extra: &[(&str, &str)]) -> ProfileStore {
    let mut props = Properties::new();
    for (k, v) in [
        ("sig_obj.types.T", "on"),
        ("sig_obj.T.table.main.1", "SIG_NOTE-v"),
        ("sig_obj.T.table.main.Style.font", "HELVETICA,6,BOLD"),
        ("sig_obj.T.table.main.Style.valuefont", "COURIER,6,NORMAL"),
        ("sig_obj.T.value.SIG_NOTE", "x"),
    ] {
        props.insert(k, v);
    }
    for (k, v) in extra {
        props.insert(*k, *v);
    }
    ProfileStore::from_properties(&props, Arc::new(BuiltinResources::default())).unwrap()
}

fn build(s: &ProfileStore, inp: &SigBlockInput, width: f32) -> pdfover_sigblock::Result<Appearance> {
    let opts = BuildOptions { width, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    Appearance::build(s.get("T").unwrap(), inp, &opts, &FontProvider::builtin(), &**s.resources())
}

fn lay(s: &ProfileStore, inp: &SigBlockInput, width: f32) -> pdfover_sigblock::Result<pdfover_sigblock::Layout> {
    layout::layout(s.get("T").unwrap(), inp, width, &FontProvider::builtin(), &**s.resources())
}

fn inner_content(a: &Appearance) -> String {
    let Object::Stream(s) = &a.objects[a.inner] else { panic!("Inner-Form fehlt") };
    String::from_utf8_lossy(&s.content).into_owned()
}

/// Der Hinweistext ist eine Nutzereingabe und läuft durch den Ausdrucks-Resolver (wie PDF-AS):
/// tiefe Klammerung darf den Prozess nicht per Stapelüberlauf abbrechen.
#[test]
fn deeply_nested_note_expression_is_rejected() {
    let s = ProfileStore::builtin().unwrap();
    let p = s.get("SIGNATURBLOCK_SMALL_DE_NOTE").unwrap();
    let mut inp = input();
    inp.note = Some(format!("${{{}'x'{}}}", "(".repeat(50_000), ")".repeat(50_000)));
    let r = layout::layout(p, &inp, 230.0, &FontProvider::builtin(), &**s.resources());
    match r {
        Err(SigBlockError::UnsupportedExpression(e)) => assert!(e.len() < 300, "Meldung gekürzt"),
        other => panic!("erwartet UnsupportedExpression: {other:?}"),
    }
    // flache Ausdrücke im Hinweis funktionieren weiterhin
    inp.note = Some("Signiert von ${subject.CN}".into());
    assert!(layout::layout(p, &inp, 230.0, &FontProvider::builtin(), &**s.resources()).is_ok());
}

/// NaN/unendliche Stilwerte ergäben ungültige Content-Streams (`NaN w`, `NaN 0 0 NaN … cm`);
/// PDFBox lehnt sie beim Schreiben ab – hier Layout und Appearance gleichermaßen.
#[test]
fn non_finite_style_values_are_errors() {
    for extra in [
        vec![
            ("sig_obj.T.table.main.1", "SIG_LABEL-i"),
            ("sig_obj.T.value.SIG_LABEL", "./images/signatur-logo_de.png"),
            ("sig_obj.T.table.main.Style.imagescaletofit", "NaN;10"),
        ],
        vec![
            ("sig_obj.T.table.main.1", "SIG_LABEL-i"),
            ("sig_obj.T.value.SIG_LABEL", "./images/signatur-logo_de.png"),
            ("sig_obj.T.table.main.Style.imagescaletofit", "10;Infinity"),
        ],
        vec![("sig_obj.T.table.main.Style.border", "NaN")],
        vec![("sig_obj.T.table.main.Style.border", "Infinity")],
        vec![("sig_obj.T.table.main.Style.padding", "NaN")],
        vec![("sig_obj.T.table.main.Style.valuefont", "COURIER,NaN,NORMAL")],
        vec![("sig_obj.T.table.main.Style.valuefont", "COURIER,1e39,NORMAL")],
        vec![("sig_obj.T.table.main.ColsWidth", "NaN")],
    ] {
        let s = custom(&extra);
        assert!(matches!(lay(&s, &input(), 100.0), Err(SigBlockError::Profile(_))), "{extra:?}: layout");
        match build(&s, &input(), 100.0) {
            Err(SigBlockError::Profile(_)) => {}
            Ok(a) => panic!("{extra:?}: Appearance gebaut: {}", inner_content(&a)),
            Err(e) => panic!("{extra:?}: {e}"),
        }
    }
    // `Infinity;10`: die Höhe begrenzt, die Bildgröße bleibt endlich (wie PDF-AS: min(wf, hf))
    let s = custom(&[
        ("sig_obj.T.table.main.1", "SIG_LABEL-i"),
        ("sig_obj.T.value.SIG_LABEL", "./images/signatur-logo_de.png"),
        ("sig_obj.T.table.main.Style.imagescaletofit", "Infinity;10"),
    ]);
    assert!(inner_content(&build(&s, &input(), 100.0).unwrap()).contains("9 0 0 10 "));
    // endliche Grenzwerte bleiben erlaubt (wie PDF-AS)
    let s = custom(&[("sig_obj.T.table.main.Style.border", "0")]);
    let a = build(&s, &input(), 100.0).unwrap();
    assert!(!inner_content(&a).contains("NaN") && !inner_content(&a).contains(" w\n"));
}

#[test]
fn malformed_tables_do_not_panic() {
    let cases: Vec<(Vec<(&str, &str)>, bool)> = vec![
        // unbekannter Typ ⇒ leere Zeile (PDF-AS ebenso): leerer Block, kein Absturz
        (vec![("sig_obj.T.table.main.1", "A-x")], true),
        // fehlende Untertabelle ⇒ Zelle entfällt
        (vec![("sig_obj.T.table.main.1", "TABLE-gibtsnicht")], true),
        // mehr Zellen als ColsWidth-Spalten ⇒ „Configuration is wrong“
        (vec![("sig_obj.T.table.main.1", "SIG_NOTE-v|SIG_NOTE-v"), ("sig_obj.T.table.main.ColsWidth", "1")], false),
        // Selbstreferenz (PDF-AS: StackOverflowError)
        (vec![("sig_obj.T.table.main.1", "TABLE-main")], false),
        // wechselseitige Referenz
        (
            vec![
                ("sig_obj.T.table.main.1", "TABLE-a"),
                ("sig_obj.T.table.a.1", "TABLE-b"),
                ("sig_obj.T.table.b.1", "TABLE-a"),
            ],
            false,
        ),
        // Base64-Wert, der kein Bild ist
        (vec![("sig_obj.T.table.main.1", "SIG_LABEL-i"), ("sig_obj.T.value.SIG_LABEL", "AAAA")], false),
        // Fontdatei außerhalb von `fonts/`
        (vec![("sig_obj.T.table.main.Style.font", "TTF:../../etc/passwd,5")], false),
        // negative Werte sind endlich und werden wie in PDF-AS übernommen
        (vec![("sig_obj.T.table.main.Style.padding", "-100")], true),
        (vec![("sig_obj.T.table.main.ColsWidth", "-1 2")], true),
        (vec![("sig_obj.T.timezone", "../../../../etc/passwd")], true),
    ];
    for (extra, ok) in cases {
        let s = custom(&extra);
        let r = build(&s, &input(), 100.0);
        assert_eq!(r.is_ok(), ok, "{extra:?}: {:?}", r.as_ref().err());
        if let Ok(a) = r {
            assert!(!inner_content(&a).contains("NaN"), "{extra:?}");
        }
    }
}

#[test]
fn inconsistent_emblems_are_rejected() {
    let s = ProfileStore::builtin().unwrap();
    let p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap();
    let opts = BuildOptions { width: 230.0, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    for e in [
        Emblem { width: 0, height: 5, rgba: vec![], has_alpha: false, source_sha256: [0; 32] },
        Emblem { width: 10, height: 10, rgba: vec![1; 7], has_alpha: true, source_sha256: [0; 32] },
    ] {
        let mut inp = input();
        inp.emblem = Some(Arc::new(e));
        let r = Appearance::build(p, &inp, &opts, &FontProvider::builtin(), &**s.resources());
        assert!(matches!(r, Err(SigBlockError::Image(_))), "{r:?}");
        // auch als Profil-Overlay
        let ov = ProfileOverrides { sig_label: inp.emblem.clone(), ..Default::default() };
        let r = layout::layout(&p.with_overrides(&ov), &input(), 230.0, &FontProvider::builtin(), &**s.resources());
        assert!(matches!(r, Err(SigBlockError::Image(_))), "{r:?}");
    }
    // extreme Seitenverhältnisse werden sauber verkleinert
    for (w, h, want) in [(1u32, 60_000u32, (1, 600)), (60_000, 1, (480, 1)), (1, 1, (1, 1))] {
        let e = Emblem::from_rgba(w, h, vec![10; (w * h * 4) as usize], [0; 32]).unwrap();
        assert_eq!((e.width, e.height), want);
    }
}

/// Mehrere Dateien mit `include = *.properties` würden ohne Zykluserkennung exponentiell oft
/// geladen (PDF-AS: StackOverflowError).
#[test]
fn include_cycles_terminate_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(cfg.join("config.properties"), "include.all = *.properties\nsig_obj.types.T = on\n").unwrap();
    for i in 0..4 {
        std::fs::write(cfg.join(format!("p{i}.properties")), format!("include.all = *.properties\nk{i} = v\n"))
            .unwrap();
    }
    let start = Instant::now();
    let store = ProfileStore::load_dir(&cfg).unwrap();
    assert!(start.elapsed() < Duration::from_secs(10), "Laden dauerte {:?}", start.elapsed());
    assert!(store.problems().iter().any(|p| p.contains("Include-Zyklus")), "{:?}", store.problems());
    assert_eq!(store.global("k3"), Some("v"));
    assert!(store.get("T").is_some());
}

#[test]
fn long_references_are_shortened_in_errors() {
    let blob = "QUJD".repeat(10_000);
    let s = custom(&[("sig_obj.T.table.main.1", "SIG_LABEL-i"), ("sig_obj.T.value.SIG_LABEL", &blob)]);
    let msg = build(&s, &input(), 100.0).unwrap_err().to_string();
    assert!(msg.len() < 300, "{} Zeichen", msg.len());
    assert!(msg.contains("40000 Zeichen"), "{msg}");
}

#[test]
fn profile_debug_redacts_note_override() {
    let s = ProfileStore::builtin().unwrap();
    let ov = ProfileOverrides { sig_note: Some("Geheimer Hinweis GZ-4711".into()), ..Default::default() };
    let p = s.get("SIGNATURBLOCK_SMALL_DE_NOTE").unwrap().with_overrides(&ov);
    let d = format!("{p:?} {s:?}");
    assert!(!d.contains("GZ-4711") && !d.contains("Geheimer"), "{d}");
    assert!(d.contains("SIG_NOTE"), "Schlüssel bleiben sichtbar");
    // der Wert selbst ist natürlich vorhanden
    assert_eq!(p.values["SIG_NOTE"], "Geheimer Hinweis GZ-4711");
}

/// Windows-Zeilenenden im Hinweistext (SWT liefert `\r\n`) werden wie `\n` behandelt statt als
/// nicht darstellbares Zeichen ersetzt (DejaVu hat keinen Glyph für U+000D).
#[test]
fn windows_line_endings_in_note() {
    use pdfover_sigblock::layout::CellContent;
    let s = ProfileStore::builtin().unwrap();
    let p = s.get("SIGNATURBLOCK_SMALL_DE_NOTE").unwrap();
    let mut inp = input();
    inp.note = Some("Zeile 1\r\nZeile 2\rZeile 3".into());
    let l = layout::layout(p, &inp, 230.0, &FontProvider::builtin(), &**s.resources()).unwrap();
    assert!(l.replaced_chars.is_empty(), "{:?}", l.replaced_chars);
    match &l.table.cells.last().unwrap().content {
        CellContent::Text { lines, .. } => assert_eq!(lines, &["Zeile 1", "Zeile 2", "Zeile 3"]),
        c => panic!("{c:?}"),
    }
    assert_eq!(l.table.row_heights[1], 5.0 * 3.0 + 8.0);
}
