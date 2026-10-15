//! Blockmaße gegen die Rechenbeispiele aus docs/analyse/04 §3.8 (Toleranz ±0,01 pt) und Randfälle
//! des Layouts (§3.2–§3.5, Quirks Q1–Q12).

use std::sync::Arc;

use pida_sigblock::layout::{CellContent, LaidOutCell, LaidOutTable, TextRole};
use pida_sigblock::{
    CertFields, Emblem, FontProvider, HAlign, Layout, PdfaMode, Profile, ProfileOverrides, ProfileStore, SigBlockError,
    SigBlockInput, VAlign, fonts::FontSpec, layout,
};
use time::macros::datetime;

const TOL: f32 = 0.01;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOL
}

#[track_caller]
fn assert_close(a: f32, b: f32, what: &str) {
    assert!(close(a, b), "{what}: {a} != {b}");
}

fn input() -> SigBlockInput {
    let mut cert = CertFields::example();
    cert.subject = vec![("C".into(), "AT".into()), ("CN".into(), "Max Mustermann".into())];
    SigBlockInput::new(cert, datetime!(2026-10-14 12:34:56 +2))
}

fn store() -> ProfileStore {
    ProfileStore::builtin().unwrap()
}

fn lay(profile: &Profile, input: &SigBlockInput, width: f32) -> Layout {
    let s = store();
    layout::layout(profile, input, width, &FontProvider::builtin(), &**s.resources()).unwrap()
}

fn lay_id(id: &str, width: f32) -> Layout {
    let s = store();
    lay(s.get(id).unwrap(), &input(), width)
}

fn sub(cell: &LaidOutCell) -> &LaidOutTable {
    match &cell.content {
        CellContent::SubTable(t) => t,
        c => panic!("keine Untertabelle: {c:?}"),
    }
}

fn image(cell: &LaidOutCell) -> (f32, f32) {
    match &cell.content {
        CellContent::Image { draw_width, draw_height, .. } => (*draw_width, *draw_height),
        c => panic!("kein Bild: {c:?}"),
    }
}

#[track_caller]
fn check_small(id: &str, h: f32, main_cols: [f32; 2], info_cols: [f32; 2], main_rows: &[f32], logo: (f32, f32)) {
    let l = lay_id(id, 230.0);
    assert_close(l.width, 230.0, &format!("{id} W"));
    assert_close(l.height, h, &format!("{id} H"));
    let t = &l.table;
    assert_close(t.col_widths[0], main_cols[0], &format!("{id} main col 0"));
    assert_close(t.col_widths[1], main_cols[1], &format!("{id} main col 1"));
    assert_eq!(t.row_heights, main_rows, "{id}");
    let info = sub(&t.cells[1]);
    assert_close(info.col_widths[0], info_cols[0], &format!("{id} info col 0"));
    assert_close(info.col_widths[1], info_cols[1], &format!("{id} info col 1"));
    assert_eq!(info.row_heights, vec![13.0, 13.0, 23.0], "{id}");
    assert_eq!(image(&t.cells[0]), logo, "{id}");
}

#[test]
fn small_profiles_match_analysis() {
    let d = [41.8182, 188.1818];
    let i = [50.8599, 137.3219];
    check_small("SIGNATURBLOCK_SMALL_DE", 49.0, d, i, &[49.0], (33.0, 34.0));
    check_small("SIGNATURBLOCK_SMALL_DE_NOTE", 82.0, d, i, &[49.0, 33.0], (33.0, 34.0));
    check_small("SIGNATURBLOCK_SMALL_EN", 49.0, d, i, &[49.0], (33.0, 34.0));
    check_small("SIGNATURBLOCK_SMALL_EN_NOTE", 82.0, d, [44.8052, 143.3766], &[49.0, 33.0], (33.0, 34.0));
    check_small("SIGNATURBLOCK_SMALL_DE_PDFA", 49.0, d, i, &[49.0], (33.0, 34.0));
    check_small("SIGNATURBLOCK_SMALL_DE_NOTE_PDFA", 82.0, d, i, &[49.0, 33.0], (33.0, 34.0));
    check_small("SIGNATURBLOCK_SMALL_EN_PDFA", 49.0, [35.3846, 194.6154], [35.3846, 159.2308], &[49.0], (27.0, 28.0));
    check_small("SIGNATURBLOCK_SMALL_EN_NOTE_PDFA", 82.0, d, i, &[49.0, 33.0], (33.0, 34.0));
}

/// Detail `SIGNATURBLOCK_SMALL_DE` (analyse/04 §3.8): Baselines = Startpunkt des `Td`.
#[test]
fn small_de_detail_positions() {
    use pida_pdf_cos::Object;
    let s = store();
    let p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap();
    let opts = pida_sigblock::BuildOptions { width: 230.0, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let app = pida_sigblock::Appearance::build(p, &input(), &opts, &FontProvider::builtin(), &**s.resources()).unwrap();
    let Object::Stream(inner) = &app.objects[app.inner] else { panic!() };
    let text = String::from_utf8_lossy(&inner.content).to_string();
    let tds: Vec<(f32, f32)> = text
        .lines()
        .filter(|l| l.ends_with(" Td") && !l.starts_with("0 -"))
        .map(|l| {
            let v: Vec<f32> = l.split(' ').take(2).map(|n| n.parse().unwrap()).collect();
            (v[0], v[1])
        })
        .collect();
    let expected =
        [(55.338, 42.035), (96.678, 42.179), (50.903, 29.035), (96.678, 29.179), (51.178, 16.035), (96.678, 16.179)];
    assert_eq!(tds.len(), expected.len(), "{text}");
    for (got, want) in tds.iter().zip(expected) {
        assert!((got.0 - want.0).abs() < 0.001 && (got.1 - want.1).abs() < 0.001, "{got:?} != {want:?}");
    }
    assert!(text.contains("q\n33 0 0 34 4.40909 8.5 cm\n/Im1 Do\nQ\n"), "{text}");
    // Wertezeilen: 3 Zeilen SIG_META, Abstand 5
    assert_eq!(text.matches("0 -5 Td").count(), 2);
}

#[test]
fn amts_on_a4() {
    for id in ["AMTSSIGNATURBLOCK_DE_RECOMMENDED", "AMTSSIGNATURBLOCK_EN_RECOMMENDED"] {
        let l = lay_id(id, 495.0);
        assert_close(l.width, 495.0, id);
        assert_close(l.height, 120.0, id);
        let t = &l.table;
        assert_close(t.col_widths[0], 70.7143, id);
        assert_close(t.col_widths[1], 424.2857, id);
        assert_eq!(t.row_heights, vec![96.0, 24.0], "{id}");
        let info = sub(&t.cells[1]);
        assert_close(info.col_widths[0], 84.8571, id);
        assert_close(info.col_widths[1], 339.4286, id);
        assert_eq!(info.row_heights, vec![24.0, 16.0, 56.0], "{id}");
        // float32: floor(f32(62/236)·236) = 61, nicht 62
        assert_eq!(image(&t.cells[0]), (61.0, 61.0), "{id}");
        // Caption mit echtem Zeilenumbruch, zentriert als Block (Q2)
        match &info.cells[0].content {
            CellContent::Text { lines, role, halign, valign, .. } => {
                assert_eq!(lines.len(), 2);
                assert_eq!(*role, TextRole::Caption);
                assert_eq!(*halign, Some(HAlign::Center));
                assert_eq!(*valign, Some(VAlign::Top));
            }
            c => panic!("{c:?}"),
        }
    }
}

#[test]
fn base_logo_23mm() {
    let s = store();
    let base = s.get("BASE_LOGO").unwrap();
    // Standardlogo DE (326×342 px)
    let ov = ProfileOverrides::base_logo(None, (326, 342), 23.0);
    let p = base.with_overrides(&ov);
    let w: f32 = p.pos.as_deref().unwrap().trim_start_matches("w:").split(';').next().unwrap().parse().unwrap();
    let l = lay(&p, &input(), w);
    assert_close(l.width, 62.118, "BASE_LOGO W");
    assert_close(l.height, 65.167, "BASE_LOGO H");
    assert_eq!(l.table.col_widths.len(), 1);
    assert_eq!(l.table.border, 0.0);
    assert_eq!(l.table.bgcolor, None);
    assert_eq!(image(&l.table.cells[0]), (62.0, 65.0));
    // Amts-Demo-Logo als Emblem (236×235 px)
    let demo = Arc::new(demo_logo());
    let ov2 = ProfileOverrides::base_logo(Some(demo.clone()), (demo.width, demo.height), 23.0);
    let p2 = base.with_overrides(&ov2);
    let w2: f32 = p2.pos.as_deref().unwrap().trim_start_matches("w:").split(';').next().unwrap().parse().unwrap();
    let l2 = lay(&p2, &input(), w2);
    assert_close(l2.width, 65.167, "BASE_LOGO demo W");
    assert_close(l2.height, 64.891, "BASE_LOGO demo H");
    assert_eq!(image(&l2.table.cells[0]), (65.0, 64.0));
}

fn demo_logo() -> Emblem {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/images/amtsignatur-logo_demo.png")).unwrap();
    Emblem::decode(&bytes).unwrap()
}

fn solid_emblem(w: u32, h: u32) -> Arc<Emblem> {
    Arc::new(Emblem::from_rgba(w, h, vec![200; (w * h * 4) as usize], [1; 32]).unwrap())
}

#[test]
fn custom_emblem_stretches_info_table() {
    let s = store();
    let p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap();
    let mut inp = input();
    // hohes Emblem 100×300 px: Zellhöhe floor((41.818−8)/100·300)+8 = 109
    inp.emblem = Some(solid_emblem(100, 300));
    let l = lay(p, &inp, 230.0);
    assert_eq!(l.table.row_heights, vec![109.0]);
    assert_close(l.height, 109.0, "H");
    let info = sub(&l.table.cells[1]);
    // Streckung über die letzte Zeile (SIG_META)
    assert_eq!(info.row_heights, vec![13.0, 13.0, 83.0]);
    // Bildbox floor(41.818)−8 × floor(109+8)−8 = 33 × 109 ⇒ 33 × 99
    assert_eq!(image(&l.table.cells[0]), (33.0, 99.0));
    // Profil-Overlay statt Eingabe wirkt genauso
    let ov = ProfileOverrides { sig_label: Some(solid_emblem(100, 300)), ..Default::default() };
    let l2 = lay(&p.with_overrides(&ov), &input(), 230.0);
    assert_eq!(l2.table.row_heights, vec![109.0]);
}

#[test]
fn long_subject_wraps_and_grows() {
    let s = store();
    let p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap();
    let mut inp = input();
    // 42 Zeichen passen, 43 brechen um (analyse/04 §3.8)
    inp.cert.subject = vec![("CN".into(), "A".repeat(42))];
    assert_close(lay(p, &inp, 230.0).height, 49.0, "42 Zeichen");
    inp.cert.subject = vec![("CN".into(), format!("{} {}", "A".repeat(30), "B".repeat(12)))];
    let l = lay(p, &inp, 230.0);
    let info = sub(&l.table.cells[1]);
    assert_eq!(info.row_heights[0], 18.0);
    // Info-Tabelle wächst ⇒ Blockhöhe 54
    assert_close(l.height, 54.0, "43 Zeichen");
    // Titel aus dem Zertifikat
    inp.cert.subject = vec![("CN".into(), "Max Mustermann".into()), ("title".into(), "Dr.".into())];
    let l = lay(p, &inp, 230.0);
    match &sub(&l.table.cells[1]).cells[1].content {
        CellContent::Text { lines, .. } => assert_eq!(lines, &vec!["Dr. Max Mustermann".to_string()]),
        c => panic!("{c:?}"),
    }
}

#[test]
fn note_override_and_tu_text() {
    let s = store();
    let p = s.get("SIGNATURBLOCK_SMALL_DE_NOTE").unwrap();
    let mut inp = input();
    inp.note = Some("Kurzer Hinweis".into());
    let l = lay(p, &inp, 230.0);
    assert_eq!(l.table.row_heights, vec![49.0, 13.0]);
    // Register: /TU enthält alle Texte; Quirk Q10: nur die äußerste Tabelle
    assert!(
        l.tu_text.contains("Max Mustermann") && l.tu_text.ends_with("Hinweis\nKurzer Hinweis\n"),
        "{:?}",
        l.tu_text
    );
    inp.pdfas_quirks = true;
    let q = lay(p, &inp, 230.0);
    assert_eq!(q.tu_text, "Hinweis\nKurzer Hinweis\n");
    let small = lay(s.get("SIGNATURBLOCK_SMALL_DE").unwrap(), &inp, 230.0);
    assert_eq!(small.tu_text, "", "Q10: SMALL ohne Hinweis ⇒ /TU leer");
}

#[test]
fn unencodable_characters() {
    let s = store();
    let p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap();
    let mut inp = input();
    // U+4E2D hat in DejaVuSansMono keinen Glyph
    inp.cert.subject = vec![("CN".into(), "Max \u{4e2d} Mustermann\tX".into())];
    let l = lay(p, &inp, 230.0);
    assert_eq!(l.replaced_chars, vec!['\u{4e2d}']);
    match &sub(&l.table.cells[1]).cells[1].content {
        CellContent::Text { lines, .. } => assert_eq!(lines, &vec!["Max ? Mustermann X".to_string()]),
        c => panic!("{c:?}"),
    }
    // Quirk Q11: nicht in CP1252 darstellbar ⇒ URL-Kodierung
    inp.pdfas_quirks = true;
    let q = lay(p, &inp, 230.0);
    match &sub(&q.table.cells[1]).cells[1].content {
        CellContent::Text { lines, .. } => assert_eq!(lines, &vec!["Max+%E4%B8%AD+Mustermann%09X".to_string()]),
        c => panic!("{c:?}"),
    }
    assert!(q.replaced_chars.is_empty());
}

#[test]
fn pdfa_substitutes_standard14_captions() {
    let s = store();
    let p = s.get("AMTSSIGNATURBLOCK_DE_RECOMMENDED").unwrap();
    let mut inp = input();
    inp.pdfa = PdfaMode::Part2;
    let l = lay(p, &inp, 495.0);
    match &l.table.cells[2].content {
        CellContent::Text { font, .. } => {
            assert_eq!(font, &FontSpec::TrueType { file: "DejaVuSansCondensed-Bold.ttf".into(), size: 8.0 })
        }
        c => panic!("{c:?}"),
    }
    // im Quirk-Modus bleibt Helvetica (wie PDF-AS)
    inp.pdfas_quirks = true;
    let q = lay(p, &inp, 495.0);
    match &q.table.cells[2].content {
        CellContent::Text { font, .. } => assert!(matches!(font, FontSpec::Standard { .. })),
        c => panic!("{c:?}"),
    }
}

#[test]
fn width_errors_and_invalid_profiles() {
    let s = store();
    let p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap();
    let fonts = FontProvider::builtin();
    assert!(layout::layout(p, &input(), 0.0, &fonts, &**s.resources()).is_err());
    assert!(layout::layout(p, &input(), f32::NAN, &fonts, &**s.resources()).is_err());
    let inv = s.get("INVISIBLE").unwrap();
    assert!(matches!(layout::layout(inv, &input(), 230.0, &fonts, &**s.resources()), Err(SigBlockError::Profile(_))));
    // unbekannter Ausdruck wird abgelehnt
    let mut bad = p.clone();
    bad.values.insert("SIG_SUBJECT".into(), "${subject.CN.toUpperCase()}".into());
    assert!(matches!(
        layout::layout(&bad, &input(), 230.0, &fonts, &**s.resources()),
        Err(SigBlockError::UnsupportedExpression(_))
    ));
    // Datumsmuster außerhalb der Teilmenge
    let mut bad_date = p.clone();
    bad_date.values.insert("SIG_DATE".into(), "EEEE".into());
    assert!(matches!(
        layout::layout(&bad_date, &input(), 230.0, &fonts, &**s.resources()),
        Err(SigBlockError::UnsupportedDatePattern(_))
    ));
    // Selbstreferenz einer Untertabelle (PDF-AS: StackOverflow)
    let mut cyc = p.clone();
    cyc.tables
        .get_mut("info")
        .unwrap()
        .rows
        .push(pida_sigblock::Row(vec![pida_sigblock::CellSpec::SubTable("info".into())]));
    assert!(matches!(layout::layout(&cyc, &input(), 230.0, &fonts, &**s.resources()), Err(SigBlockError::Profile(_))));
    // ColsWidth mit Summe 0
    let mut zero = p.clone();
    zero.tables.get_mut("main").unwrap().cols_width = Some(vec![0.0, 0.0]);
    assert!(matches!(layout::layout(&zero, &input(), 230.0, &fonts, &**s.resources()), Err(SigBlockError::Profile(_))));
    // fehlende Caption
    let mut no_cap = p.clone();
    no_cap.captions.remove("SIG_META");
    assert!(matches!(
        layout::layout(&no_cap, &input(), 230.0, &fonts, &**s.resources()),
        Err(SigBlockError::Profile(_))
    ));
    // fehlendes Bild
    let mut no_img = p.clone();
    no_img.values.insert("SIG_LABEL".into(), "./images/gibt-es-nicht.png".into());
    assert!(layout::layout(&no_img, &input(), 230.0, &fonts, &**s.resources()).is_err());
    // kein Wert ⇒ „IMG MISSING“ als Textzelle
    let mut missing = p.clone();
    missing.values.remove("SIG_LABEL");
    let l = layout::layout(&missing, &input(), 230.0, &fonts, &**s.resources()).unwrap();
    match &l.table.cells[0].content {
        CellContent::Text { lines, role, .. } => {
            assert_eq!(lines, &vec!["IMG MISSING".to_string()]);
            assert_eq!(*role, TextRole::Value);
        }
        c => panic!("{c:?}"),
    }
}

/// Eigenes Profil mit allen Ausrichtungen, Colspan-Auffüllung und Untertabelle mit eigenem Stil
/// (border 0, Rest geerbt); Erwartungswerte von Hand nach analyse/04 §3.4/§3.5 berechnet.
#[test]
fn custom_profile_alignments_and_colspan() {
    use pida_java_properties::Properties;
    use pida_pdf_cos::Object;
    let mut props = Properties::new();
    for (k, v) in [
        ("sig_obj.types.T", "on"),
        ("sig_obj.T.table.main.1", "SIG_LABEL-i|TABLE-info"),
        ("sig_obj.T.table.main.2", "SIG_NOTE-v"),
        ("sig_obj.T.table.main.ColsWidth", "1 3"),
        ("sig_obj.T.table.main.Style.font", "HELVETICA,6,BOLD"),
        ("sig_obj.T.table.main.Style.valuefont", "COURIER,6,NORMAL"),
        ("sig_obj.T.table.main.Style.padding", "2"),
        ("sig_obj.T.table.main.Style.halign", "right"),
        ("sig_obj.T.table.main.Style.valign", "bottom"),
        ("sig_obj.T.table.main.Style.valuehalign", "lineCenter"),
        ("sig_obj.T.table.main.Style.valuevalign", "middle"),
        ("sig_obj.T.table.main.Style.imagehalign", "right"),
        ("sig_obj.T.table.main.Style.imagevalign", "bottom"),
        ("sig_obj.T.table.info.1", "SIG_DATE-cv"),
        ("sig_obj.T.table.info.Style.border", "0"),
        ("sig_obj.T.key.SIG_DATE", "Datum"),
        ("sig_obj.T.value.SIG_DATE", "dd.MM.yyyy"),
        ("sig_obj.T.value.SIG_LABEL", "./images/signatur-logo_de.png"),
        ("sig_obj.T.value.SIG_NOTE", "Erste Zeile\nZweite längere Zeile"),
    ] {
        props.insert(k, v);
    }
    let s = ProfileStore::from_properties(&props, Arc::new(pida_sigblock::BuiltinResources::default())).unwrap();
    let p = s.get("T").unwrap();
    let l = lay(p, &input(), 200.0);
    assert_eq!((l.width, l.height), (200.0, 68.0));
    let t = &l.table;
    assert_eq!(t.col_widths, vec![50.0, 150.0]);
    assert_eq!(t.row_heights, vec![52.0, 16.0]);
    assert_eq!(t.border, 1.0, "border nicht gesetzt ⇒ 1");
    // Zeile 2: eine Zelle mit Colspan 2 ⇒ volle Breite
    assert_eq!(t.cells[2].width, 200.0);
    let info = sub(&t.cells[1]);
    assert_eq!(info.border, 0.0);
    assert_eq!(info.padding, 2.0, "padding geerbt");
    assert_eq!(info.row_heights, vec![52.0], "auf Zeilenhöhe gestreckt");

    let opts = pida_sigblock::BuildOptions { width: 200.0, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let app = pida_sigblock::Appearance::build(p, &input(), &opts, &FontProvider::builtin(), &**s.resources()).unwrap();
    let Object::Stream(inner) = &app.objects[app.inner] else { panic!() };
    let text = String::from_utf8_lossy(&inner.content).to_string();
    let nums = |l: &str| -> Vec<f32> { l.split(' ').filter_map(|n| n.parse().ok()).collect() };
    let tds: Vec<Vec<f32>> = text.lines().filter(|l| l.ends_with(" Td")).map(nums).collect();
    let expected = [
        [104.334, 20.242], // Caption rechts/unten (Helvetica-Bold, Descent −1,242)
        [144.5, 40.942],   // Wert lineCenter einzeilig = zentriert, middle (Courier, Descent −0,942)
        [80.2, 9.942],     // lineCenter mehrzeilig: je Zeile zentriert
        [-80.2, 0.0],
        [64.0, -6.0],
        [-64.0, 0.0],
    ];
    assert_eq!(tds.len(), expected.len(), "{text}");
    for (got, want) in tds.iter().zip(expected) {
        assert!((got[0] - want[0]).abs() < 1e-3 && (got[1] - want[1]).abs() < 1e-3, "{got:?} != {want:?}\n{text}");
    }
    // kein Hintergrund (bgcolor fehlt), Hauptrahmen 1 pt, Untertabelle ohne Linien
    assert!(!text.contains(" re\n"));
    assert_eq!(text.matches(" w\n").count(), 1);
    assert!(text.starts_with("1 w\n0 69 m\n200 69 l\nS\n0 17 m\n200 17 l\nS\n0 1 m\n200 1 l\nS\n0 69 m\n0 17 l\nS\n50 69 m\n50 17 l\nS\n200 69 m\n200 17 l\nS\n0 17 m\n0 1 l\nS\n200 17 m\n200 1 l\nS\n"), "{text}");
    // Bild rechts/unten: x = 50 − iw, y = Zeilenunterkante 17
    let (iw, ih) = image(&t.cells[0]);
    let cm: Vec<f32> = text.lines().find(|l| l.ends_with(" cm")).map(nums).unwrap();
    assert_eq!(cm, vec![iw, 0.0, 0.0, ih, 50.0 - iw, 17.0]);
}

#[test]
fn timezone_from_profile() {
    let s = store();
    let mut p = s.get("SIGNATURBLOCK_SMALL_DE").unwrap().clone();
    p.timezone = Some("UTC".into());
    let l = lay(&p, &input(), 230.0);
    match &sub(&l.table.cells[1]).cells[3].content {
        CellContent::Text { lines, .. } => assert_eq!(lines, &vec!["2026-10-14T10:34:56Z".to_string()]),
        c => panic!("{c:?}"),
    }
}
