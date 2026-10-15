//! Positionsstring, Merge, `resolve_width`/`resolve_position` (mit Test-Scanner), Widget-Rect und
//! Rasterscan-Hilfen – ohne PDFium.

use pida_pdf_cos::{PageInfo, Rect, Rotation, SigField, SigWidget};
use pida_placement::*;
use pida_render::{ForcedColorOptions, ImageObject, PageSize, RenderError, Renderer, RgbaImage};
use proptest::prelude::*;

fn page(index: usize, media: [f32; 4], crop: Option<[f32; 4]>, rotation: Rotation) -> PageInfo {
    let m = Rect::new(media[0], media[1], media[2], media[3]);
    PageInfo {
        index,
        object_id: (index as u32 + 1, 0),
        media_box: m,
        crop_box: crop.map(|c| Rect::new(c[0], c[1], c[2], c[3])).unwrap_or(m),
        rotation,
        user_unit: 1.0,
        annots_indirect: None,
    }
}

fn a4(index: usize) -> PageInfo {
    page(index, [0.0, 0.0, 595.0, 842.0], None, Rotation::R0)
}

fn p(s: &str) -> TablePos {
    TablePos::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
}

// ------------------------------------------------------------------------------------------------
// Parser / Formatter
// ------------------------------------------------------------------------------------------------

#[test]
fn parse_full_and_auto() {
    assert_eq!(p("p:auto;x:auto;y:auto"), TablePos::auto());
    let t = p("p:3;x:72.5;y:800;w:230;f:80;r:90");
    assert_eq!(t.page, Some(PageSelector::Exact(3)));
    assert_eq!(t.x, Some(Coord::Value(72.5)));
    assert_eq!(t.y, Some(Coord::Value(800.0)));
    assert_eq!(t.w, Some(Coord::Value(230.0)));
    assert_eq!(t.footer, Some(80.0));
    assert_eq!(t.rotation, Some(90));
    assert_eq!(p("p:NEW").page, Some(PageSelector::New));
    assert_eq!(p("p:Last").page, Some(PageSelector::Last));
    assert_eq!(p("x:AUTO").x, Some(Coord::Auto));
    assert_eq!("p:2".parse::<TablePos>().unwrap().page, Some(PageSelector::Exact(2)));
}

#[test]
fn parse_java_split_semantics() {
    // leere Teile am Ende entfallen
    assert_eq!(p("p:1;"), TablePos { page: Some(PageSelector::Exact(1)), ..Default::default() });
    assert_eq!(p(";;"), TablePos::default());
    assert_eq!(p("x:5:"), TablePos { x: Some(Coord::Value(5.0)), ..Default::default() });
    for bad in ["", ";p:1", "p:1;;x:2", "x:", "x:1:2", "xx:1", "q:1", ":5", "P:1", "x"] {
        assert!(TablePos::parse(bad).is_err(), "{bad:?} muss ungültig sein");
    }
    // spätere Schlüssel gewinnen
    assert_eq!(p("x:1;x:2").x, Some(Coord::Value(2.0)));
}

#[test]
fn parse_value_ranges_like_pdfas() {
    for bad in ["x:-1", "y:-0.5", "w:0.5", "w:0", "f:-1", "p:0", "p:-1", "p:1.5", "p: 1", "r:45", "r:90.5"] {
        assert!(TablePos::parse(bad).is_err(), "{bad:?} muss ungültig sein");
    }
    assert_eq!(p("w:1").w, Some(Coord::Value(1.0)));
    assert_eq!(p("x: 5 ").x, Some(Coord::Value(5.0)), "Float.parseFloat trimmt");
    assert_eq!(p("x:5f").x, Some(Coord::Value(5.0)));
    assert_eq!(p("x:5.0d").x, Some(Coord::Value(5.0)));
    assert_eq!(p("x:1e2").x, Some(Coord::Value(100.0)));
    assert_eq!(p("x:.5").x, Some(Coord::Value(0.5)));
    assert_eq!(p("p:+2").page, Some(PageSelector::Exact(2)));
    assert_eq!(p("p:007").page, Some(PageSelector::Exact(7)));
    assert_eq!(p("r:-90").rotation, Some(-90), "Java-% behält das Vorzeichen");
    assert_eq!(p("r:450").rotation, Some(90));
    assert_eq!(p("r:90.0").rotation, Some(90));
    assert_eq!(p("r:360").rotation, Some(0));
}

#[test]
fn parse_rejects_non_finite_numbers() {
    // Abweichung KR-F09: PDF-AS akzeptiert NaN/Infinity
    for bad in ["x:NaN", "y:Infinity", "w:1e40", "f:inf", "x:0x1p3", "r:NaN"] {
        assert!(TablePos::parse(bad).is_err(), "{bad:?} muss ungültig sein");
    }
    let e = TablePos::parse("x:-1").unwrap_err();
    assert_eq!(e.code(), "placement.invalid_position");
    assert!(e.to_string().contains("x:-1"));
}

#[test]
fn display_formats_pdfas_syntax() {
    assert_eq!(TablePos::auto().to_string(), "p:auto;x:auto;y:auto");
    assert_eq!(TablePos::manual(2, 100.5, 700.25).to_string(), "p:2;x:100.5;y:700.25");
    assert_eq!(TablePos::default().to_string(), "");
    assert_eq!(p("f:80;w:230").to_string(), "w:230;f:80");
    assert_eq!(p("r:-90;p:new;w:auto").to_string(), "p:new;w:auto;r:-90");
    assert_eq!(p("p:last;y:200").to_string(), "p:last;y:200");
    let big = TablePos::manual(1, 1e20, 0.1);
    assert_eq!(TablePos::parse(&big.to_string()).unwrap(), big);
}

fn arb_coord(min: f32) -> impl Strategy<Value = Option<Coord>> {
    prop_oneof![Just(None), Just(Some(Coord::Auto)), (min..100_000.0f32).prop_map(|v| Some(Coord::Value(v)))]
}

fn arb_table_pos() -> impl Strategy<Value = TablePos> {
    (
        prop_oneof![
            Just(None),
            Just(Some(PageSelector::Auto)),
            Just(Some(PageSelector::New)),
            Just(Some(PageSelector::Last)),
            (1u32..100_000).prop_map(|n| Some(PageSelector::Exact(n)))
        ],
        arb_coord(0.0),
        arb_coord(0.0),
        arb_coord(1.0),
        prop_oneof![Just(None), (0.0..10_000.0f32).prop_map(Some)],
        prop_oneof![Just(None), (-3i32..=3).prop_map(|k| Some(k * 90))],
    )
        .prop_map(|(page, x, y, w, footer, rotation)| TablePos { page, x, y, w, footer, rotation })
}

proptest! {
    #[test]
    fn display_parse_roundtrip(t in arb_table_pos()) {
        let s = t.to_string();
        if s.is_empty() {
            prop_assert_eq!(t, TablePos::default());
        } else {
            prop_assert_eq!(TablePos::parse(&s).unwrap(), t);
        }
    }

    #[test]
    fn parse_is_total_on_arbitrary_input(s in "\\PC{0,80}") {
        let _ = TablePos::parse(&s);
        let _ = parse_qr_payload(&s);
        let _ = parse_qr_payload(&format!("PDF-AS-POS;{s}"));
    }

    #[test]
    fn parsed_positions_roundtrip_through_display(s in "([pxywfr]:([-+0-9.eE]{1,8}|auto|new|last|AUTO);?){1,6}") {
        if let Ok(t) = TablePos::parse(&s) {
            prop_assert_eq!(TablePos::parse(&t.to_string()).unwrap(), t);
        }
    }

    #[test]
    fn merge_with_empty_request_is_identity(t in arb_table_pos()) {
        let merged = TablePos::merge(&TablePos::default(), &t);
        prop_assert_eq!(merged.page, t.page);
        prop_assert_eq!(merged.x, t.x);
        prop_assert_eq!(merged.w, t.w);
    }
}

// ------------------------------------------------------------------------------------------------
// Merge
// ------------------------------------------------------------------------------------------------

#[test]
fn merge_like_tablepos_constructor() {
    let profile = p("f:80;w:230");
    let m = TablePos::merge(&TablePos::auto(), &profile);
    assert_eq!(m.page, Some(PageSelector::Auto));
    assert_eq!(m.w, Some(Coord::Value(230.0)));
    assert_eq!(m.footer, Some(80.0));
    assert_eq!(m.x, Some(Coord::Auto));
    // Request überschreibt nur genannte Schlüssel
    let m = TablePos::merge(&p("p:2;x:10;y:20"), &profile);
    assert_eq!(m.to_string(), "p:2;x:10;y:20;w:230;f:80");
    // `x:auto` setzt in PDF-AS nichts: fester Profilwert bleibt (justizsignatur: x:110)
    let justiz = p("x:110;y:auto;w:375;p:auto;f:60");
    let m = TablePos::merge(&TablePos::auto(), &justiz);
    assert_eq!(m.x, Some(Coord::Value(110.0)));
    assert_eq!(m.w, Some(Coord::Value(375.0)));
    assert_eq!(m.footer, Some(60.0));
    // `p:auto` setzt den Seitenmodus dagegen zurück
    assert_eq!(TablePos::merge(&p("p:auto"), &p("p:new")).page, Some(PageSelector::Auto));
    // Fußzeile des Profils geht bei festem y bzw. p:new verloren (getFooterLine)
    assert_eq!(TablePos::merge(&p("y:auto"), &p("y:100;f:80")).footer, Some(0.0));
    assert_eq!(TablePos::merge(&p("p:auto"), &p("p:new;f:80")).footer, Some(0.0));
    assert_eq!(TablePos::merge(&p("f:30"), &p("p:new;f:80")).footer, Some(30.0));
}

#[test]
fn effective_accessors() {
    assert_eq!(TablePos::default().page_mode(), PageSelector::Auto);
    assert_eq!(p("f:80").footer_line(), 80.0);
    assert_eq!(p("f:80;y:100").footer_line(), 0.0);
    assert_eq!(p("f:80;p:new").footer_line(), 0.0);
    assert_eq!(p("f:80;y:auto;p:last").footer_line(), 80.0);
    assert_eq!(p("r:180").block_rotation(), 180);
    assert_eq!(TablePos::default().block_rotation(), 0);
    let pl = Placement { page_index: 1, x: 10.0, y: 20.0, width: 5.0, height: 5.0 };
    assert_eq!(pl.to_table_pos().to_string(), "p:2;x:10;y:20");
}

// ------------------------------------------------------------------------------------------------
// resolve_width
// ------------------------------------------------------------------------------------------------

fn ctx(pages: &[PageInfo]) -> PositionContext<'_> {
    PositionContext::new(pages)
}

#[test]
fn width_auto_centering_and_margins() {
    let pages = [a4(0)];
    let w = resolve_width(&TablePos::auto(), &ctx(&pages)).unwrap();
    assert_eq!((w.page_index, w.x, w.width), (0, 50.0, 495.0));
    let w = resolve_width(&TablePos::merge(&TablePos::auto(), &p("w:230;f:80")), &ctx(&pages)).unwrap();
    assert_eq!((w.x, w.width), (182.5, 230.0));
    let w = resolve_width(&p("x:100;w:230"), &ctx(&pages)).unwrap();
    assert_eq!((w.x, w.width), (100.0, 230.0));
}

#[test]
fn amts_width_corrected_and_quirk() {
    let pages = [a4(0)];
    let pos = p("p:1;x:150;y:700");
    let corrected = resolve_width(&pos, &ctx(&pages)).unwrap();
    assert_eq!((corrected.x, corrected.width), (150.0, 495.0), "KR-F01: Breite unabhängig von x");
    let quirk = resolve_width(&pos, &PositionContext { compat: CompatMode::PdfasQuirks, ..ctx(&pages) }).unwrap();
    assert_eq!(quirk.width, 595.0 - 300.0, "PDF-AS: pw − 2x");
    let far = p("p:1;x:400;y:700");
    assert_eq!(
        resolve_width(&far, &PositionContext { compat: CompatMode::PdfasQuirks, ..ctx(&pages) }).unwrap().width,
        -205.0
    );
    assert_eq!(resolve_width(&far, &ctx(&pages)).unwrap().width, 495.0);
    // Mindestbreite 1 pt bei sehr schmalen Seiten
    let narrow = [page(0, [0.0, 0.0, 80.0, 200.0], None, Rotation::R0)];
    assert_eq!(resolve_width(&TablePos::auto(), &ctx(&narrow)).unwrap().width, 1.0);
}

#[test]
fn width_uses_rotated_cropbox() {
    let rotated = [page(0, [0.0, 0.0, 595.0, 842.0], None, Rotation::R90)];
    assert_eq!(resolve_width(&TablePos::auto(), &ctx(&rotated)).unwrap().width, 742.0);
    let cropped = [page(0, [0.0, 0.0, 595.0, 842.0], Some([50.0, 50.0, 545.0, 792.0]), Rotation::R0)];
    assert_eq!(resolve_width(&TablePos::auto(), &ctx(&cropped)).unwrap().width, 395.0);
    // p:2 nimmt die Geometrie von Seite 2
    let mixed = [a4(0), page(1, [0.0, 0.0, 842.0, 595.0], None, Rotation::R0), a4(2)];
    let w = resolve_width(&p("p:2"), &ctx(&mixed)).unwrap();
    assert_eq!((w.page_index, w.width), (1, 742.0));
}

#[test]
fn width_for_new_page_uses_media_box_when_corrected() {
    let cropped = [page(0, [0.0, 0.0, 595.0, 842.0], Some([50.0, 50.0, 545.0, 792.0]), Rotation::R0)];
    let corrected = resolve_width(&p("p:new"), &ctx(&cropped)).unwrap();
    assert_eq!((corrected.page_index, corrected.width), (0, 495.0));
    let quirk =
        resolve_width(&p("p:new"), &PositionContext { compat: CompatMode::PdfasQuirks, ..ctx(&cropped) }).unwrap();
    assert_eq!(quirk.width, 395.0);
    let w = resolve_width(&p("p:5"), &ctx(&[a4(0), a4(1)])).unwrap();
    assert_eq!(w.page_index, 1);
}

#[test]
fn width_errors() {
    assert!(matches!(resolve_width(&TablePos::auto(), &ctx(&[])), Err(PlacementError::NoSuchPage(0))));
    let pos = TablePos { page: Some(PageSelector::Exact(0)), ..Default::default() };
    assert!(matches!(resolve_width(&pos, &ctx(&[a4(0)])), Err(PlacementError::InvalidPosition(_))));
    let pages = [a4(0)];
    let failing = PositionContext { already_signed: true, less_space_failing: true, ..ctx(&pages) };
    let e = resolve_width(&p("p:new"), &failing).unwrap_err();
    assert!(matches!(e, PlacementError::NotEnoughSpace));
    assert_eq!(e.code(), "placement.not_enough_space");
}

// ------------------------------------------------------------------------------------------------
// resolve_position
// ------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeScanner {
    lowest: f32,
    fail: bool,
    occupied: Option<bool>,
    scans: Vec<(usize, f32)>,
    regions: Vec<(usize, Rect)>,
}

impl FreeSpaceScanner for FakeScanner {
    fn lowest_content(&mut self, page_index: usize, footer: f32) -> Result<f32> {
        self.scans.push((page_index, footer));
        if self.fail {
            return Err(PlacementError::Render(RenderError::Render("Test".into())));
        }
        Ok(self.lowest)
    }

    fn region_occupied(&mut self, page_index: usize, area: Rect) -> Result<Option<bool>> {
        self.regions.push((page_index, area));
        Ok(self.occupied)
    }
}

fn resolve(pos: &TablePos, c: &PositionContext<'_>, h: f32, scanner: &mut FakeScanner) -> Result<ResolvedPosition> {
    let w = resolve_width(pos, c)?;
    resolve_position(pos, c, w, h, scanner)
}

fn small_auto() -> TablePos {
    TablePos::merge(&TablePos::auto(), &p("f:80;w:230"))
}

#[test]
fn auto_on_empty_and_filled_pages() {
    let pages = [a4(0), a4(1)];
    let mut s = FakeScanner::default();
    let r = resolve(&small_auto(), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.x, r.y, r.width, r.height), (1, false, 182.5, 822.0, 230.0, 49.0));
    assert_eq!(s.scans, vec![(1, 80.0)], "letzte Seite, Fußzeile 80");
    assert!(s.regions.is_empty(), "Freiraumposition braucht keine Überdeckungsprüfung");
    let mut s = FakeScanner { lowest: 300.0, ..Default::default() };
    let r = resolve(&small_auto(), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!(r.y, 842.0 - 300.0 - 20.0);
    assert!(r.notices.is_empty());
}

#[test]
fn auto_no_space_adds_new_page() {
    let pages = [a4(0)];
    // y = 842 − 700 − 20 = 122; 122 − 80 = 42 ≤ 49 ⇒ neue Seite
    let mut s = FakeScanner { lowest: 700.0, ..Default::default() };
    let r = resolve(&small_auto(), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (1, true, 822.0));
    // Grenze: y − f == h passt nicht, knapp darüber passt
    let mut s = FakeScanner { lowest: 842.0 - 20.0 - 80.0 - 49.0, ..Default::default() };
    assert!(resolve(&small_auto(), &ctx(&pages), 49.0, &mut s).unwrap().new_page);
    let mut s = FakeScanner { lowest: 842.0 - 20.0 - 80.0 - 49.5, ..Default::default() };
    assert!(!resolve(&small_auto(), &ctx(&pages), 49.0, &mut s).unwrap().new_page);
}

#[test]
fn signed_documents_get_no_new_page() {
    let pages = [a4(0)];
    let signed = PositionContext { already_signed: true, ..ctx(&pages) };
    let mut s = FakeScanner { lowest: 700.0, ..Default::default() };
    let r = resolve(&small_auto(), &signed, 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (0, false, 822.0));
    assert_eq!(r.notices, vec![PlacementNotice::NewPageNotAllowed]);
    // erzwungen (SIGNED_NEWPAGE_FORCE)
    let forced = PositionContext { signed_newpage_force: true, ..signed };
    let r = resolve(&small_auto(), &forced, 49.0, &mut FakeScanner { lowest: 700.0, ..Default::default() }).unwrap();
    assert!(r.new_page);
    // Fehler statt Ausweichen
    let failing = PositionContext { less_space_failing: true, ..signed };
    let e =
        resolve(&small_auto(), &failing, 49.0, &mut FakeScanner { lowest: 700.0, ..Default::default() }).unwrap_err();
    assert!(matches!(e, PlacementError::NotEnoughSpace));
    // p:new auf signiertem Dokument ⇒ letzte Seite mit Freiraumsuche
    let mut s = FakeScanner { lowest: 100.0, ..Default::default() };
    let r = resolve(&p("p:new;f:80"), &signed, 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (0, false, 722.0));
    assert_eq!(r.notices, vec![PlacementNotice::NewPageNotAllowed]);
    assert_eq!(s.scans, vec![(0, 0.0)], "p:new ⇒ Fußzeile 0");
}

#[test]
fn exact_page_that_does_not_fit_moves_to_last_page() {
    // PDF-AS-Eigenheit: p:<n> ohne Platz ⇒ letzte Seite, ph − 20
    let pages = [a4(0), a4(1), a4(2)];
    let pos = TablePos::merge(&p("p:2"), &p("f:80"));
    let mut s = FakeScanner { lowest: 800.0, ..Default::default() };
    let r = resolve(&pos, &ctx(&pages), 50.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (2, false, 822.0));
    assert_eq!(s.scans, vec![(1, 80.0)]);
    assert_eq!(s.regions.len(), 1, "Ausweichposition wird auf Überdeckung geprüft");
    // mit Platz bleibt Seite 2
    let mut s = FakeScanner { lowest: 100.0, ..Default::default() };
    let r = resolve(&pos, &ctx(&pages), 50.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.y), (1, 722.0));
}

#[test]
fn last_fixed_and_new_page_modes_skip_scan() {
    let pages = [a4(0), a4(1)];
    let mut s = FakeScanner::default();
    let r = resolve(&p("p:last;f:80"), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (1, false, 822.0));
    let r = resolve(&p("p:last;y:300"), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!(r.y, 300.0);
    let r = resolve(&p("p:1;x:10;y:400"), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.x, r.y), (0, 10.0, 400.0));
    let r = resolve(&p("p:new"), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (2, true, 822.0));
    let r = resolve(&p("p:99;y:auto"), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (2, true, 822.0));
    let r = resolve(&p("p:99;x:20;y:300"), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y), (2, true, 300.0));
    assert!(s.scans.is_empty(), "{:?}", s.scans);
}

#[test]
fn invisible_block_height_zero() {
    let pages = [a4(0)];
    let mut s = FakeScanner::default();
    let r = resolve(&TablePos::auto(), &ctx(&pages), 0.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page, r.y, r.height), (0, false, 0.0, 0.0));
    let r = resolve(&p("p:new"), &ctx(&pages), 0.0, &mut s).unwrap();
    assert_eq!((r.page_index, r.new_page), (1, true));
    assert!(s.scans.is_empty() && s.regions.is_empty());
}

#[test]
fn scan_failure_is_treated_as_empty_page() {
    let pages = [a4(0)];
    let mut s = FakeScanner { fail: true, ..Default::default() };
    let r = resolve(&small_auto(), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!(r.y, 822.0);
    assert_eq!(r.notices, vec![PlacementNotice::ScanFailed]);
}

#[test]
fn overlap_notices_for_manual_positions() {
    let pages = [a4(0)];
    let mut s = FakeScanner { occupied: Some(true), ..Default::default() };
    let r = resolve(&p("p:1;x:100;y:500;w:200"), &ctx(&pages), 50.0, &mut s).unwrap();
    assert_eq!(r.notices, vec![PlacementNotice::OverlapsContent]);
    assert_eq!(s.regions, vec![(0, Rect::new(100.0, 450.0, 300.0, 500.0))]);
    let mut s = FakeScanner { occupied: Some(false), ..Default::default() };
    assert!(resolve(&p("p:1;x:100;y:500;w:200"), &ctx(&pages), 50.0, &mut s).unwrap().notices.is_empty());
    // neue Seite: keine Prüfung
    let mut s = FakeScanner { occupied: Some(true), ..Default::default() };
    assert!(resolve(&p("p:new;x:100;y:500"), &ctx(&pages), 50.0, &mut s).unwrap().notices.is_empty());
    assert!(s.regions.is_empty());
}

#[test]
fn rotation_and_footer_pass_through() {
    let pages = [a4(0)];
    let mut s = FakeScanner::default();
    let r = resolve(&TablePos::merge(&p("r:90"), &p("f:80;w:230")), &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!(r.rotation, 90);
    assert_eq!(s.scans, vec![(0, 80.0)]);
}

#[test]
fn quirk_mode_keeps_height_of_checked_page() {
    // p:1 auf kleiner Seite 1 passt nicht ⇒ letzte (große) Seite; PDF-AS nimmt ph von Seite 1.
    let pages = [page(0, [0.0, 0.0, 300.0, 300.0], None, Rotation::R0), a4(1)];
    let pos = TablePos::merge(&p("p:1"), &p("f:80;w:230"));
    let mut s = FakeScanner { lowest: 250.0, ..Default::default() };
    let corrected = resolve(&pos, &ctx(&pages), 49.0, &mut s).unwrap();
    assert_eq!((corrected.page_index, corrected.y), (1, 822.0));
    let quirk = PositionContext { compat: CompatMode::PdfasQuirks, ..ctx(&pages) };
    let r = resolve(&pos, &quirk, 49.0, &mut FakeScanner { lowest: 250.0, ..Default::default() }).unwrap();
    assert_eq!((r.page_index, r.y), (1, 280.0));
}

// ------------------------------------------------------------------------------------------------
// Widget-Rect und Rotation
// ------------------------------------------------------------------------------------------------

fn resolved(x: f32, y: f32, w: f32, h: f32, rotation: i32) -> ResolvedPosition {
    ResolvedPosition { page_index: 0, new_page: false, x, y, width: w, height: h, rotation, notices: vec![] }
}

/// Direkte Portierung von `PDFAsVisualSignatureBuilder.createSignatureRectangle` (f64, wie Java
/// `Point2D.Float` + `AffineTransform`) als Orakel; liefert das normalisierte Rect.
fn pdfas_rect(pos: &ResolvedPosition, media_w: f64, media_h: f64, page_rot: i32) -> Rect {
    let (x, y, w, h) = (pos.x as f64, pos.y as f64, pos.width as f64, pos.height as f64);
    let (pw, ph) = if page_rot == 90 || page_rot == 270 { (media_h, media_w) } else { (media_w, media_h) };
    let y_axis = ph - y;
    let up = (x + w, ph - y_axis);
    let ll = (x, ph - y_axis - h);
    let degrees = (pos.rotation + page_rot) as f64;
    let rot = |p: (f64, f64)| -> (f64, f64) {
        if degrees % 360.0 == 0.0 {
            return p;
        }
        let t = degrees.to_radians();
        let (s, c) = (t.sin().round(), t.cos().round());
        let (dx, dy) = (p.0 - ll.0, p.1 - ll.1);
        (ll.0 + dx * c - dy * s, ll.1 + dx * s + dy * c)
    };
    let (mut up, mut lo) = (rot(up), rot(ll));
    let (tx, ty) = match page_rot {
        90 => (ph - (ph - y_axis) - x + h, x + h - (ph - y_axis)),
        180 => (pw - 2.0 * x, ph - 2.0 * (y - h)),
        270 => (-h + y - x, pw - (y - h) - x),
        _ => (0.0, 0.0),
    };
    up = (up.0 + tx, up.1 + ty);
    lo = (lo.0 + tx, lo.1 + ty);
    Rect::new(lo.0 as f32, lo.1 as f32, up.0 as f32, up.1 as f32).normalized()
}

fn rect_close(a: Rect, b: Rect) -> bool {
    [(a.llx, b.llx), (a.lly, b.lly), (a.urx, b.urx), (a.ury, b.ury)].iter().all(|(u, v)| (u - v).abs() < 1e-3)
}

#[test]
fn widget_rect_unrotated() {
    let r = widget_rect(&resolved(182.5, 822.0, 230.0, 49.0, 0), &a4(0));
    assert_eq!(r, Rect::new(182.5, 773.0, 412.5, 822.0));
}

#[test]
fn widget_rect_page_rotations_match_analysis_formulas() {
    let (w0, h0) = (595.0f32, 842.0f32);
    let pos = resolved(100.0, 500.0, 200.0, 50.0, 0);
    let (x, y, w, h) = (100.0, 500.0, 200.0, 50.0);
    let r90 = widget_rect(&pos, &page(0, [0.0, 0.0, w0, h0], None, Rotation::R90));
    assert_eq!(r90, Rect::new(w0 - y, x, w0 - y + h, x + w));
    let r180 = widget_rect(&pos, &page(0, [0.0, 0.0, w0, h0], None, Rotation::R180));
    assert_eq!(r180, Rect::new(w0 - x - w, h0 - y, w0 - x, h0 - y + h));
    let r270 = widget_rect(&pos, &page(0, [0.0, 0.0, w0, h0], None, Rotation::R270));
    assert_eq!(r270, Rect::new(y - h, h0 - x - w, y, h0 - x));
}

#[test]
fn widget_rect_matches_pdfas_port_for_all_rotations() {
    for page_rot in [0, 90, 180, 270] {
        for block_rot in [0, 90, 180, 270, -90] {
            for (x, y, w, h) in [(50.0, 822.0, 495.0, 120.0), (182.5, 400.0, 230.0, 49.0), (0.0, 49.0, 230.0, 49.0)] {
                let rotation = Rotation::from_degrees(page_rot).unwrap();
                let pg = page(0, [0.0, 0.0, 612.0, 792.0], None, rotation);
                let pos = resolved(x, y, w, h, block_rot);
                let ours = widget_rect_with(&pos, &pg, CompatMode::PdfasQuirks);
                let theirs = pdfas_rect(&pos, 612.0, 792.0, page_rot as i32);
                assert!(rect_close(ours, theirs), "Seite {page_rot}°, Block {block_rot}°: {ours:?} ≠ {theirs:?}");
                // ohne CropBox-Versatz ist korrigiert = Original
                assert!(rect_close(widget_rect(&pos, &pg), theirs));
            }
        }
    }
}

#[test]
fn block_rotation_turns_around_lower_left_corner() {
    let r = widget_rect(&resolved(100.0, 500.0, 200.0, 50.0, 90), &a4(0));
    assert_eq!(r, Rect::new(50.0, 450.0, 100.0, 650.0));
    let r = widget_rect(&resolved(100.0, 500.0, 200.0, 50.0, 180), &a4(0));
    assert_eq!(r, Rect::new(-100.0, 400.0, 100.0, 450.0));
    assert_eq!(appearance_rotation(90, &page(0, [0.0, 0.0, 1.0, 1.0], None, Rotation::R270)), 0);
    assert_eq!(appearance_rotation(-90, &a4(0)), 270);
    assert_eq!(appearance_rotation(0, &page(0, [0.0, 0.0, 1.0, 1.0], None, Rotation::R90)), 90);
}

#[test]
fn cropbox_origin_is_honoured_when_corrected() {
    let crop = [36.0, 72.0, 576.0, 756.0];
    let pos = resolved(100.0, 500.0, 200.0, 50.0, 0);
    let pg = page(0, [0.0, 0.0, 612.0, 792.0], Some(crop), Rotation::R0);
    assert_eq!(widget_rect(&pos, &pg), Rect::new(136.0, 522.0, 336.0, 572.0));
    assert_eq!(widget_rect_with(&pos, &pg, CompatMode::PdfasQuirks), Rect::new(100.0, 450.0, 300.0, 500.0));
    // gedreht: Bezug ist die CropBox (Breite 540, Höhe 684)
    let pg90 = page(0, [0.0, 0.0, 612.0, 792.0], Some(crop), Rotation::R90);
    assert_eq!(
        widget_rect(&pos, &pg90),
        Rect::new(36.0 + 540.0 - 500.0, 72.0 + 100.0, 36.0 + 540.0 - 450.0, 72.0 + 300.0)
    );
    // neue Seite: MediaBox der letzten Seite, ohne CropBox
    let new = ResolvedPosition { new_page: true, page_index: 1, ..pos.clone() };
    assert_eq!(widget_rect(&new, &pg), Rect::new(100.0, 450.0, 300.0, 500.0));
    let g = new_page_geometry(&pg);
    assert_eq!((g.index, g.crop_box, g.rotation), (1, pg.media_box, Rotation::R0));
}

#[test]
fn visible_rect_and_widget_overlap() {
    let pos = resolved(100.0, 500.0, 200.0, 50.0, 0);
    assert_eq!(visible_block_rect(&pos), Rect::new(100.0, 450.0, 300.0, 500.0));
    let field = |page: Option<usize>, rect: Rect| SigField {
        qualified_name: "S".into(),
        partial_name: Some("S".into()),
        field_id: (9, 0),
        depth: 0,
        widgets: vec![SigWidget { id: (9, 0), page_index: page, rect, annot_flags: 4 }],
        signed: true,
        value_id: None,
        has_lock: false,
        has_seed_value: false,
        field_flags: 0,
    };
    let pg = a4(0);
    assert!(widget_overlaps(&pos, &pg, &[field(Some(0), Rect::new(250.0, 480.0, 400.0, 600.0))]));
    assert!(!widget_overlaps(&pos, &pg, &[field(Some(1), Rect::new(250.0, 480.0, 400.0, 600.0))]));
    assert!(!widget_overlaps(&pos, &pg, &[field(Some(0), Rect::new(300.0, 480.0, 400.0, 600.0))]), "Kante berührt nur");
    assert!(!widget_overlaps(&pos, &pg, &[field(Some(0), Rect::new(0.0, 0.0, 0.0, 0.0))]));
    let new = ResolvedPosition { new_page: true, ..pos };
    assert!(!widget_overlaps(&new, &pg, &[field(Some(0), Rect::new(250.0, 480.0, 400.0, 600.0))]));
}

#[test]
fn start_position_is_clamped() {
    let pg = a4(0);
    let s = default_start_position(&pg, BlockGeometry { width: 230.0, height: 49.0 });
    assert_eq!((s.x, s.y), (297.5, 210.5));
    let s = default_start_position(&pg, BlockGeometry { width: 495.0, height: 300.0 });
    assert_eq!((s.x, s.y), (100.0, 300.0));
    let s = default_start_position(&pg, BlockGeometry { width: 700.0, height: 900.0 });
    assert_eq!((s.x, s.y), (0.0, 842.0));
    let rot = page(0, [0.0, 0.0, 595.0, 842.0], None, Rotation::R90);
    let s = default_start_position(&rot, BlockGeometry { width: 100.0, height: 50.0 });
    assert_eq!((s.x, s.y), (421.0, 148.75));
    // NaN-Blockmaße (fehlerhaftes Layout) führen nicht zur Panik
    let s = default_start_position(&pg, BlockGeometry { width: f32::NAN, height: f32::NAN });
    assert!(s.x.is_finite() && s.y.is_finite(), "{s:?}");
}

#[test]
fn start_placement_uses_last_page() {
    let block = BlockGeometry { width: 230.0, height: 49.0 };
    assert_eq!(default_start_placement(&[], block), None);
    let pages = [a4(0), page(1, [0.0, 0.0, 595.0, 842.0], None, Rotation::R90)];
    let p = default_start_placement(&pages, block).unwrap();
    assert_eq!((p.page_index, p.x, p.y, p.width, p.height), (1, 421.0, 148.75, 230.0, 49.0));
    assert_eq!(p.to_table_pos().to_string(), "p:2;x:421;y:148.75");
    // Seitenindex am Rand des Wertebereichs: kein Überlauf
    let far = Placement { page_index: usize::MAX, ..p };
    assert_eq!(far.to_table_pos().page, Some(PageSelector::Exact(u32::MAX)));
}

// ------------------------------------------------------------------------------------------------
// Rasterscan
// ------------------------------------------------------------------------------------------------

const MAGIC: [u8; 4] = [152, 254, 52, 255];

#[test]
fn lowest_row_like_pdfutilities() {
    let mut img = RgbaImage::filled(20, 100, MAGIC);
    assert_eq!(lowest_content_row(&img, 0, MAGIC), 0);
    img.set_pixel(5, 30, [0, 0, 0, 255]);
    assert_eq!(lowest_content_row(&img, 0, MAGIC), 30);
    img.set_pixel(5, 90, [0, 0, 0, 255]);
    assert_eq!(lowest_content_row(&img, 0, MAGIC), 90);
    // Fußzeile: Zeilen ab 100 − 1 − 20 = 79 aufwärts
    assert_eq!(lowest_content_row(&img, 20, MAGIC), 30);
    assert_eq!(lowest_content_row(&img, 9, MAGIC), 90);
    assert_eq!(lowest_content_row(&img, 10, MAGIC), 30);
    // Zeile 0 wird nie gezählt
    let mut top = RgbaImage::filled(20, 100, MAGIC);
    top.set_pixel(0, 0, [0, 0, 0, 255]);
    assert_eq!(lowest_content_row(&top, 0, MAGIC), 0);
    // Fußzeile höher als das Bild: negativer Startwert wie PDF-AS
    assert_eq!(lowest_content_row(&img, 150, MAGIC), -51);
    // extreme Fußzeilen ohne Überlauf (negative Fußzeile = ganze Seite)
    assert_eq!(lowest_content_row(&img, i64::MIN, MAGIC), 90);
    assert_eq!(lowest_content_row(&img, i64::MAX, MAGIC), -i64::MAX + 99);
    assert_eq!(lowest_content_row(&RgbaImage::empty(), 0, MAGIC), -1);
    // Alpha zählt mit (exakter ARGB-Vergleich)
    let mut alpha = RgbaImage::filled(4, 10, MAGIC);
    alpha.set_pixel(1, 5, [152, 254, 52, 254]);
    assert_eq!(lowest_content_row(&alpha, 0, MAGIC), 5);
}

#[test]
fn tolerance_ignores_faint_antialiasing() {
    let mut img = RgbaImage::filled(4, 10, MAGIC);
    img.set_pixel(1, 4, [0, 0, 0, 255]);
    img.set_pixel(1, 6, [153, 254, 52, 255]); // Abweichung 1
    assert_eq!(lowest_content_row(&img, 0, MAGIC), 6);
    assert_eq!(lowest_content_row_with_tolerance(&img, 0, MAGIC, SCAN_TOLERANCE), 4);
    img.set_pixel(2, 7, [158, 254, 52, 255]); // Abweichung 6
    assert_eq!(lowest_content_row_with_tolerance(&img, 0, MAGIC, SCAN_TOLERANCE), 7);
}

#[test]
fn background_detection_from_corners() {
    let mut img = RgbaImage::filled(20, 20, [255, 255, 255, 255]);
    assert_eq!(detect_background(&img), Some([255, 255, 255, 255]));
    img.set_pixel(5, 5, [1, 2, 3, 255]);
    img.set_pixel(15, 5, [1, 2, 3, 255]);
    img.set_pixel(5, 15, [1, 2, 3, 255]);
    assert_eq!(detect_background(&img), Some([1, 2, 3, 255]));
    assert_eq!(detect_background(&RgbaImage::filled(5, 5, MAGIC)), None);
}

/// Renderer mit fester Forced-Color-Rasterung (für `RasterScanner`).
struct FakeRenderer {
    raster: RgbaImage,
    size: PageSize,
}

impl Renderer for FakeRenderer {
    fn page_count(&self) -> usize {
        1
    }
    fn page_size(&self, page: usize) -> pida_render::Result<PageSize> {
        if page == 0 { Ok(self.size) } else { Err(RenderError::PageOutOfRange { page, count: 1 }) }
    }
    fn render_page(&self, _page: usize, _scale: f32) -> pida_render::Result<RgbaImage> {
        Err(RenderError::Render("nicht benötigt".into()))
    }
    fn render_forced_color(&self, page: usize, _opts: &ForcedColorOptions) -> pida_render::Result<RgbaImage> {
        self.page_size(page)?;
        Ok(self.raster.clone())
    }
    fn image_objects(&self, _page: usize) -> pida_render::Result<Vec<ImageObject>> {
        Ok(vec![])
    }
}

#[test]
fn raster_scanner_converts_pixels_to_points() {
    // 144 dpi: Bild 2× so groß wie die Seite
    let mut raster = RgbaImage::filled(200, 400, MAGIC);
    raster.set_pixel(10, 101, [0, 0, 0, 255]);
    raster.set_pixel(10, 390, [0, 0, 0, 255]); // in der Fußzeile (f = 10 pt = 20 px)
    let r = FakeRenderer { raster, size: PageSize { width: 100.0, height: 200.0 } };
    let mut scanner = RasterScanner { renderer: &r, options: ForcedColorOptions { dpi: 144.0, ..Default::default() } };
    assert_eq!(scanner.lowest_content(0, 10.0).unwrap(), 50.5);
    assert_eq!(scanner.lowest_content(0, 0.0).unwrap(), 195.0);
    assert!(matches!(scanner.lowest_content(1, 0.0), Err(PlacementError::Render(_))));
    // Überdeckung im sichtbaren Rechteck (PAGE_ABS): Pixel (10, 101) ⇒ (5 pt, 149,5 pt von unten)
    assert_eq!(scanner.region_occupied(0, Rect::new(0.0, 140.0, 20.0, 160.0)).unwrap(), Some(true));
    assert_eq!(scanner.region_occupied(0, Rect::new(20.0, 140.0, 40.0, 160.0)).unwrap(), Some(false));
}

#[test]
fn auto_place_uses_raster_scanner() {
    let mut raster = RgbaImage::filled(595, 842, MAGIC);
    for x in 50..545 {
        raster.set_pixel(x, 300, [0, 0, 0, 255]);
    }
    let r = FakeRenderer { raster, size: PageSize { width: 595.0, height: 842.0 } };
    let pages = [a4(0)];
    let c = ctx(&pages);
    let pos = small_auto();
    let w = resolve_width(&pos, &c).unwrap();
    let res = auto_place(&r, &pos, &c, w, 49.0).unwrap();
    assert_eq!(res.y, 842.0 - 300.0 - 20.0);
}

#[test]
fn error_and_notice_codes() {
    assert_eq!(PlacementError::NoSuchPage(1).code(), "placement.no_such_page");
    assert_eq!(PlacementError::PlaceholderNotFound.code(), "placement.placeholder_not_found");
    assert_eq!(PlacementError::Render(RenderError::PasswordRequired).code(), "render.password_required");
    for (n, c) in [
        (PlacementNotice::NewPageNotAllowed, "placement.notice.new_page_not_allowed"),
        (PlacementNotice::OverlapsContent, "placement.notice.overlaps_content"),
        (PlacementNotice::OverlapsWidget, "placement.notice.overlaps_widget"),
        (PlacementNotice::Clamped, "placement.notice.clamped"),
        (PlacementNotice::ScanFailed, "placement.notice.scan_failed"),
    ] {
        assert_eq!(n.code(), c);
    }
}
