//! Rotation und CropBox gegen PDFium als unabhängiges Orakel: Was `widget_rect` im Benutzerraum
//! liefert, muss PDFium (das `/Rotate` und die CropBox selbst anwendet) an der sichtbaren
//! Position zeichnen; Auto-Positionierung und QR-Positionen auf gedrehten Seiten ebenso.

mod common;

use common::{PageSpec, PdfBuilder, build_doc, qr_image};
use pida_pdf_cos::{PageInfo, Rect};
use pida_placement::{
    CompatMode, PositionContext, ResolvedPosition, TablePos, auto_place, find_qr_placeholders, resolve_width,
    visible_block_rect, widget_rect,
};
use pida_render::RgbaImage;

/// Hülle der dunklen Pixel (x0, y0, x1, y1), exklusiv rechts/unten.
fn dark_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..img.height {
        for x in 0..img.width {
            let p = img.pixel(x, y);
            if p[0] < 128 && p[1] < 128 && p[2] < 128 {
                b = Some(match b {
                    None => (x, y, x + 1, y + 1),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)),
                });
            }
        }
    }
    b
}

fn fill(r: Rect) -> String {
    let r = r.normalized();
    format!("0 g {} {} {} {} re f ", r.llx, r.lly, r.width(), r.height())
}

fn page_info(bytes: &[u8]) -> PageInfo {
    common::pages(&common::load(bytes)).remove(0)
}

fn resolved(x: f32, y: f32, w: f32, h: f32, rotation: i32) -> ResolvedPosition {
    ResolvedPosition { page_index: 0, new_page: false, x, y, width: w, height: h, rotation, notices: vec![] }
}

const CASES: [(i32, Option<[f32; 4]>); 8] = [
    (0, None),
    (90, None),
    (180, None),
    (270, None),
    (0, Some([36.0, 72.0, 576.0, 756.0])),
    (90, Some([36.0, 72.0, 576.0, 756.0])),
    (180, Some([36.0, 72.0, 576.0, 756.0])),
    (270, Some([36.0, 72.0, 576.0, 756.0])),
];

#[test]
fn widget_rect_lands_where_pdfium_shows_it() {
    let backend = require_pdfium!();
    for (rotate, crop) in CASES {
        for block_rotation in [0, 90, 180, 270] {
            // Geometrie der Seite bestimmen, Rect berechnen, Rechteck im Benutzerraum füllen
            let spec =
                |content: &str| PageSpec { rotate, crop_box: crop, ..PageSpec::new([0.0, 0.0, 612.0, 792.0], content) };
            let probe = build_doc(PdfBuilder::new(), vec![spec("")], "");
            let page = page_info(&probe);
            let (vw, vh) = page.visible_size();
            let pos = resolved(300.0, 380.0, 200.0, 60.0, block_rotation);
            let rect = widget_rect(&pos, &page);
            let bytes = build_doc(PdfBuilder::new(), vec![spec(&fill(rect))], "");
            let img = common::open(&backend, &bytes).render_page(0, 1.0).unwrap();
            assert_eq!((img.width, img.height), (vw as u32, vh as u32));
            let (x0, y0, x1, y1) = dark_bbox(&img).unwrap_or_else(|| panic!("{rotate}° {crop:?}: nichts sichtbar"));
            let v = visible_block_rect(&pos);
            let expected = (v.llx, vh - v.ury, v.urx, vh - v.lly);
            let got = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
            let ok = [(got.0, expected.0), (got.1, expected.1), (got.2, expected.2), (got.3, expected.3)]
                .iter()
                .all(|(a, b)| (a - b).abs() <= 1.0);
            assert!(
                ok,
                "Seite {rotate}° CropBox {crop:?} Block {block_rotation}°: sichtbar {got:?}, erwartet {expected:?}"
            );
        }
    }
}

#[test]
fn quirk_rect_ignores_cropbox_origin() {
    // Gegenprobe: das PDF-AS-Original landet bei CropBox-Versatz daneben (KR-F02).
    let backend = require_pdfium!();
    let spec = |content: &str| PageSpec {
        crop_box: Some([36.0, 72.0, 576.0, 756.0]),
        ..PageSpec::new([0.0, 0.0, 612.0, 792.0], content)
    };
    let page = page_info(&build_doc(PdfBuilder::new(), vec![spec("")], ""));
    let pos = resolved(150.0, 400.0, 200.0, 60.0, 0);
    let rect = pida_placement::widget_rect_with(&pos, &page, CompatMode::PdfasQuirks);
    let bytes = build_doc(PdfBuilder::new(), vec![spec(&fill(rect))], "");
    let img = common::open(&backend, &bytes).render_page(0, 1.0).unwrap();
    let (x0, y0, _, _) = dark_bbox(&img).unwrap();
    assert_eq!((x0, y0), (150 - 36, (684 - 400) + 72), "um den CropBox-Ursprung versetzt");
}

#[test]
fn auto_position_on_rotated_pages_scans_visible_space() {
    let backend = require_pdfium!();
    for (rotate, crop) in CASES {
        let spec =
            |content: &str| PageSpec { rotate, crop_box: crop, ..PageSpec::new([0.0, 0.0, 612.0, 792.0], content) };
        let page = page_info(&build_doc(PdfBuilder::new(), vec![spec("")], ""));
        let (vw, vh) = page.visible_size();
        // Inhalt: sichtbar oben, 300 pt hoch, über fast die ganze Breite
        let content = resolved(20.0, vh - 10.0, vw - 40.0, 290.0, 0);
        let bytes = build_doc(PdfBuilder::new(), vec![spec(&fill(widget_rect(&content, &page)))], "");
        let pages = vec![page.clone()];
        let ctx = PositionContext::new(&pages);
        let pos = TablePos::merge(&TablePos::auto(), &TablePos::parse("f:80;w:230").unwrap());
        let width = resolve_width(&pos, &ctx).unwrap();
        let renderer = common::open(&backend, &bytes);
        let r = auto_place(renderer.as_ref(), &pos, &ctx, width, 49.0).unwrap();
        // unterste belegte Zeile ≈ 300 ⇒ y = vh − 300 − 20 (±1 Kantenpixel)
        assert!(!r.new_page, "{rotate}°: neue Seite unerwartet");
        assert!((r.y - (vh - 320.0)).abs() <= 1.0, "{rotate}° {crop:?}: y = {} statt {}", r.y, vh - 320.0);
        assert_eq!(r.x, (vw - 230.0) / 2.0);
        assert!(r.notices.is_empty(), "{:?}", r.notices);
        // Block gezeichnet: liegt sichtbar unter dem Inhalt, nicht darauf
        let with_block = build_doc(
            PdfBuilder::new(),
            vec![spec(&format!("{}{}", fill(widget_rect(&content, &page)), fill(widget_rect(&r, &page))))],
            "",
        );
        let img = common::open(&backend, &with_block).render_page(0, 1.0).unwrap();
        let gap_row = (vh - r.y - 10.0) as u32; // zwischen Inhalt (bis 300) und Block (ab 320)
        assert!(
            (0..img.width).all(|x| img.pixel(x, gap_row)[0] > 128),
            "{rotate}°: Lücke zwischen Inhalt und Block erwartet"
        );
    }
}

#[test]
fn qr_position_on_rotated_pages_matches_rendering() {
    let backend = require_pdfium!();
    for (rotate, crop) in CASES {
        let mut b = PdfBuilder::new();
        let im = b.add_gray_image(&qr_image("PDF-AS-POS;id=1", 4));
        let spec_page = PageSpec { rotate, crop_box: crop, ..PageSpec::new([0.0, 0.0, 612.0, 792.0], "") };
        let page = page_info(&build_doc(PdfBuilder::new(), vec![spec_page], ""));
        // QR sichtbar aufrecht 120 × 120 pt mit linker oberer Ecke (100.3, 500.6): CTM = Einheitsquadrat
        // → sichtbares Rechteck → Benutzerraum (über widget_rect der Ecken)
        let target = resolved(100.3, 500.6, 120.0, 120.0, 0);
        let user = widget_rect(&target, &page);
        let (a, b_, c, d) = match rotate {
            90 => (0.0, 120.0, -120.0, 0.0),
            180 => (-120.0, 0.0, 0.0, -120.0),
            270 => (0.0, -120.0, 120.0, 0.0),
            _ => (120.0, 0.0, 0.0, 120.0),
        };
        // Ursprung des Bildes (Einheitsquadrat (0,0)) = sichtbar links unten
        let (e, f) = match rotate {
            90 => (user.urx, user.lly),
            180 => (user.urx, user.ury),
            270 => (user.llx, user.ury),
            _ => (user.llx, user.lly),
        };
        let content = format!("q {a} {b_} {c} {d} {e} {f} cm /Q1 Do Q");
        let spec = PageSpec {
            rotate,
            crop_box: crop,
            resources: format!("/XObject << /Q1 {im} 0 R >>"),
            ..PageSpec::new([0.0, 0.0, 612.0, 792.0], &content)
        };
        let bytes = build_doc(b, vec![spec], "");
        let doc = common::load(&bytes);
        let renderer = common::open(&backend, &bytes);
        let found =
            find_qr_placeholders(renderer.as_ref(), &common::pages(&doc), &common::all_invocations(&doc, false))
                .unwrap();
        assert_eq!(found.len(), 1, "{rotate}° {crop:?}: QR nicht gefunden (Bild aufrecht?)");
        // Bild 100,3 … 220,3: Breite 121 statt PDF-AS 120, damit die rechte Kante bedeckt ist (KR-F14)
        assert_eq!(found[0].pos.to_string(), "p:1;x:100;y:501;w:121", "{rotate}° {crop:?}");
        // Gegenprobe: PDFium zeigt das Bild sichtbar an dieser Stelle (QR-Ruhezone ist weiß,
        // daher die dunklen Module etwas innerhalb)
        let img = renderer.render_page(0, 1.0).unwrap();
        let (x0, y0, x1, y1) = dark_bbox(&img).unwrap();
        let vh = page.visible_size().1;
        assert!(x0 as f32 >= 100.0 && (x1 as f32) <= 221.0, "{rotate}°: x {x0}..{x1}");
        assert!(y0 as f32 >= vh - 501.0 && (y1 as f32) <= vh - 380.0, "{rotate}°: y {y0}..{y1}");
    }
}
