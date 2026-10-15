//! QR-Inhalt, Auswahlmodi, leere Felder und Platzhalterposition – ohne PDFium.

use pdfover_pdf_cos::{PageInfo, Rect, Rotation, SigField, SigWidget};
use pdfover_placement::*;

fn qr(name: &str, id: Option<&str>) -> QrPlaceholder {
    QrPlaceholder {
        page_index: 0,
        resource_name: name.to_owned(),
        payload: QrPayload { id: id.map(str::to_owned), raw: "PDF-AS-POS".into(), ..Default::default() },
        pos: TablePos::manual(1, 10.0, 20.0),
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    }
}

fn query(id: Option<&str>, mode: PlaceholderMode, used: &[&str]) -> PlaceholderQuery {
    PlaceholderQuery { id: id.map(str::to_owned), mode, used_names: used.iter().map(|s| (*s).to_owned()).collect() }
}

fn pick<'a>(c: &'a [QrPlaceholder], q: &PlaceholderQuery) -> Option<&'a str> {
    select_placeholder(c, q).unwrap().map(|p| p.resource_name.as_str())
}

#[test]
fn payload_parsing_like_check_image() {
    let p = parse_qr_payload("PDF-AS-POS").unwrap();
    assert_eq!((p.id, p.profile, p.kind, p.key), (None, None, None, None));
    assert_eq!(p.raw, "PDF-AS-POS");
    let p = parse_qr_payload("PDF-AS-POS;ID=2;Profile=SIGNATURBLOCK_SMALL_DE;type=x;KEY=k").unwrap();
    assert_eq!(p.id.as_deref(), Some("2"));
    assert_eq!(p.profile.as_deref(), Some("SIGNATURBLOCK_SMALL_DE"));
    assert_eq!(p.kind.as_deref(), Some("x"));
    assert_eq!(p.key.as_deref(), Some("k"));
    // ungültige Paare ignoriert, letzter Wert gewinnt, Präfix nur per startsWith
    let p = parse_qr_payload("PDF-AS-POSX;id=;foo;=1;id==3;id=4;id=5;").unwrap();
    assert_eq!(p.id.as_deref(), Some("5"));
    assert_eq!(parse_qr_payload("pdf-as-pos;id=1"), None, "startsWith ist case-sensitiv");
    assert_eq!(parse_qr_payload("XPDF-AS-POS;id=1"), None);
    assert_eq!(parse_qr_payload("https://example.invalid"), None);
    let p = parse_qr_payload("PDF-AS-POS;;id=7").unwrap();
    assert_eq!(p.id.as_deref(), Some("7"));
}

#[test]
fn payload_debug_redacts_key_and_raw_text() {
    let p = parse_qr_payload("PDF-AS-POS;id=1;key=geheim-123").unwrap();
    let d = format!("{p:?}");
    assert!(!d.contains("geheim"), "{d}");
    assert!(d.contains("***") && d.contains("raw_len"), "{d}");
    assert!(d.contains("Some(\"1\")"), "{d}");
}

#[test]
fn selection_modes() {
    let c = [qr("A", None), qr("B", Some("2")), qr("C", Some("1")), qr("D", Some("abc")), qr("E", Some("200"))];
    // PDF-Over beim Öffnen: id 1, SORTED
    assert_eq!(pick(&c, &query(Some("1"), PlaceholderMode::Sorted, &[])), Some("C"));
    // SORTED ohne id: kleinste id (numerisch; "abc" > "1" lexikografisch)
    assert_eq!(pick(&c, &query(None, PlaceholderMode::Sorted, &[])), Some("C"));
    // MODERATE ohne id: erster ohne id (Seitenebene)
    assert_eq!(pick(&c, &query(None, PlaceholderMode::Moderate, &[])), Some("A"));
    // MODERATE mit unbekannter id: erster ohne id (Dokumentebene)
    assert_eq!(pick(&c, &query(Some("7"), PlaceholderMode::Moderate, &[])), Some("A"));
    assert_eq!(pick(&c[1..], &query(Some("7"), PlaceholderMode::Moderate, &[])), None);
    // LENIENT: sonst erster überhaupt
    assert_eq!(pick(&c[1..], &query(Some("7"), PlaceholderMode::Lenient, &[])), Some("B"));
    // STRICT
    assert_eq!(pick(&c, &query(Some("2"), PlaceholderMode::Strict, &[])), Some("B"));
    assert!(matches!(
        select_placeholder(&c, &query(Some("7"), PlaceholderMode::Strict, &[])),
        Err(PlacementError::PlaceholderNotFound)
    ));
    assert!(select_placeholder(&[], &query(None, PlaceholderMode::Strict, &[])).is_err());
    assert_eq!(pick(&[], &query(None, PlaceholderMode::Lenient, &[])), None);
    // id-Vergleich ohne Groß-/Kleinschreibung, numerisch mit führenden Nullen
    assert_eq!(pick(&c, &query(Some("ABC"), PlaceholderMode::Moderate, &[])), Some("D"));
    assert_eq!(pick(&c, &query(Some("002"), PlaceholderMode::Moderate, &[])), Some("B"));
    // leere gesuchte id = keine id
    assert_eq!(pick(&c, &query(Some(""), PlaceholderMode::Moderate, &[])), Some("A"));
}

#[test]
fn ids_above_127() {
    // KR-F03: Java vergleicht Integer-Objekte mit == (nur −128…127 aus dem Cache gleich)
    let c = [qr("X", Some("128")), qr("Y", Some("200"))];
    assert_eq!(pick(&c, &query(Some("200"), PlaceholderMode::Strict, &[])), Some("Y"));
    assert_eq!(pick(&c, &query(Some("128"), PlaceholderMode::Moderate, &[])), Some("X"));
}

#[test]
fn used_placeholders_are_skipped() {
    let c = [qr("A", None), qr("B", Some("2")), qr("C", Some("1"))];
    assert_eq!(pick(&c, &query(Some("1"), PlaceholderMode::Sorted, &["C"])), Some("B"));
    assert_eq!(pick(&c, &query(Some("1"), PlaceholderMode::Sorted, &["C", "B"])), Some("A"));
    assert_eq!(pick(&c, &query(Some("1"), PlaceholderMode::Sorted, &["A", "B", "C"])), None);
    assert!(select_placeholder(&c, &query(Some("1"), PlaceholderMode::Strict, &["A", "B", "C"])).is_err());
    // Signieren (MODERATE) mit gespeicherter id nach dem Öffnen
    let opened = select_placeholder(&c, &query(Some("1"), PlaceholderMode::Sorted, &[])).unwrap().unwrap();
    let stored = opened.payload.id.clone();
    let signing =
        select_placeholder(&c, &PlaceholderQuery { id: stored, mode: PlaceholderMode::Moderate, used_names: vec![] })
            .unwrap()
            .unwrap();
    assert_eq!(signing.resource_name, "C");
}

#[test]
fn sorted_mode_with_mixed_ids_folds_like_java() {
    // Reihenfolge der paarweisen Vergleiche wie placeHolderIdMatcher
    let c = [qr("A", Some("b")), qr("B", Some("10")), qr("C", Some("9")), qr("D", Some("a"))];
    // A(b) vs B(10): String "10" < "b" ⇒ B; B(10) vs C(9): 9 < 10 ⇒ C; C(9) vs D(a): "a" > "9" ⇒ C
    assert_eq!(pick(&c, &query(None, PlaceholderMode::Sorted, &[])), Some("C"));
    // Gleichstand: der erste bleibt
    let c = [qr("A", Some("5")), qr("B", Some("05"))];
    assert_eq!(pick(&c, &query(None, PlaceholderMode::Sorted, &[])), Some("A"));
}

#[test]
fn mode_from_config() {
    assert_eq!(PlaceholderMode::from_config(0), Some(PlaceholderMode::Strict));
    assert_eq!(PlaceholderMode::from_config(1), Some(PlaceholderMode::Moderate));
    assert_eq!(PlaceholderMode::from_config(2), Some(PlaceholderMode::Lenient));
    assert_eq!(PlaceholderMode::from_config(3), Some(PlaceholderMode::Sorted));
    assert_eq!(PlaceholderMode::from_config(4), None);
}

#[test]
fn min_width_correction() {
    let mut p = qr("A", None);
    p.pos = TablePos::parse("p:1;x:72;y:700;w:100").unwrap();
    assert_eq!(p.table_pos_with_min_width(Some(230.0)).w, Some(Coord::Value(230.0)));
    assert_eq!(p.table_pos_with_min_width(Some(50.0)).w, Some(Coord::Value(100.0)));
    assert_eq!(p.table_pos_with_min_width(None), p.pos);
    assert_eq!(p.table_pos_with_min_width(Some(-1.0)), p.pos);
}

fn field(name: &str, partial: Option<&str>, signed: bool, ff: u32, widgets: Vec<SigWidget>) -> SigField {
    SigField {
        qualified_name: name.into(),
        partial_name: partial.map(str::to_owned),
        field_id: (name.len() as u32, 0),
        depth: name.matches('.').count(),
        widgets,
        signed,
        value_id: None,
        has_lock: false,
        has_seed_value: false,
        field_flags: ff,
    }
}

fn widget(id: u32, flags: u32) -> SigWidget {
    SigWidget { id: (id, 0), page_index: Some(0), rect: Rect::new(10.0, 10.0, 110.0, 60.0), annot_flags: flags }
}

#[test]
fn empty_fields_filtering() {
    let fields = vec![
        field("Sig1", Some("Sig1"), false, 0, vec![widget(10, 4)]),
        field("Signed", Some("Signed"), true, 0, vec![widget(11, 4)]),
        field("NoName", None, false, 0, vec![widget(12, 4)]),
        field("ReadOnly", Some("ReadOnly"), false, 1, vec![widget(13, 4)]),
        field("Locked", Some("Locked"), false, 0, vec![widget(14, 4 | 512)]),
        field("RoWidget", Some("RoWidget"), false, 0, vec![widget(15, 64)]),
        field("NoWidget", Some("NoWidget"), false, 0, vec![]),
        field("form.part.Sig2", Some("Sig2"), false, 0, vec![widget(16, 4), widget(17, 4)]),
        field("Hidden", Some("Hidden"), false, 0, vec![widget(18, 2)]),
    ];
    let c = find_empty_sig_fields(&fields);
    let names: Vec<&str> = c.iter().map(|c| c.qualified_name.as_str()).collect();
    assert_eq!(names, ["Sig1", "form.part.Sig2", "Hidden"]);
    assert_eq!(c[1].widget_id, (16, 0), "erstes Widget");
    assert_eq!(c[0].rect, Rect::new(10.0, 10.0, 110.0, 60.0));
    assert_eq!(c[0].page_index, Some(0));
    assert_eq!(c[0].field_id, fields[0].field_id);
    assert!(find_empty_sig_fields(&[]).is_empty());
}

fn page(rotation: Rotation, media: [f32; 4], crop: Option<[f32; 4]>) -> PageInfo {
    let m = Rect::new(media[0], media[1], media[2], media[3]);
    PageInfo {
        index: 0,
        object_id: (1, 0),
        media_box: m,
        crop_box: crop.map(|c| Rect::new(c[0], c[1], c[2], c[3])).unwrap_or(m),
        rotation,
        user_unit: 1.0,
        annots_indirect: None,
    }
}

#[test]
fn placeholder_position_from_ctm_for_rotations() {
    let a4 = [0.0, 0.0, 595.0, 842.0];
    let s = 100.0;
    // aufrecht: CTM = [s 0 0 s e f] ⇒ x = e, Oberkante = f + s
    assert_eq!(
        placeholder_position(&page(Rotation::R0, a4, None), &[s, 0.0, 0.0, s, 72.0, 600.0]),
        (72.0, 700.0, 100.0)
    );
    // /Rotate 90: im Benutzerraum um 90° gegen den Uhrzeigersinn gedreht, damit sichtbar aufrecht
    let (x, y, w) = placeholder_position(&page(Rotation::R90, a4, None), &[0.0, s, -s, 0.0, 300.0, 200.0]);
    // Bild belegt X ∈ [200, 300], Y ∈ [200, 300] ⇒ sichtbar links = 200, oben = 595 − 200 = 395
    assert_eq!((x, y, w), (200.0, 395.0, 100.0));
    let (x, y, w) = placeholder_position(&page(Rotation::R180, a4, None), &[-s, 0.0, 0.0, -s, 300.0, 400.0]);
    // X ∈ [200, 300], Y ∈ [300, 400] ⇒ links = 595 − 300 = 295, oben = 842 − 300 = 542
    assert_eq!((x, y, w), (295.0, 542.0, 100.0));
    let (x, y, w) = placeholder_position(&page(Rotation::R270, a4, None), &[0.0, -s, s, 0.0, 100.0, 400.0]);
    // X ∈ [100, 200], Y ∈ [300, 400] ⇒ links = 842 − 400 = 442, oben = 200
    assert_eq!((x, y, w), (442.0, 200.0, 100.0));
    // CropBox-Ursprung wird abgezogen (KR-F02)
    let (x, y, _) =
        placeholder_position(&page(Rotation::R0, a4, Some([20.0, 30.0, 575.0, 812.0])), &[s, 0.0, 0.0, s, 72.0, 600.0]);
    assert_eq!((x, y), (52.0, 670.0));
}

#[test]
fn decode_qr_rejects_small_and_broken_images() {
    assert!(decode_qr(&pdfover_render::RgbaImage::filled(9, 100, [0, 0, 0, 255])).is_empty());
    assert!(decode_qr(&pdfover_render::RgbaImage::filled(100, 9, [0, 0, 0, 255])).is_empty());
    assert!(decode_qr(&pdfover_render::RgbaImage::filled(64, 64, [255, 255, 255, 255])).is_empty());
    assert!(decode_qr(&pdfover_render::RgbaImage::empty()).is_empty());
    let broken = pdfover_render::RgbaImage { width: 50, height: 50, data: vec![0; 17] };
    assert!(decode_qr(&broken).is_empty());
}
