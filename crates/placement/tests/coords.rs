//! Koordinatenräume PAGE_ABS/PAGE_REL/IMAGE/SCREEN (PDF-Over `SignaturePanel`).

use pida_pdf_cos::{PageInfo, Rect, Rotation};
use pida_placement::{
    CoordSpace, KEYBOARD_COARSE_STEP_PT, KEYBOARD_FINE_STEP_PT, KEYBOARD_STEP_PT, KeyStep, PageTransform, Point,
    ScreenGeometry,
};
use proptest::prelude::*;

fn page(rotation: Rotation, crop: Option<Rect>) -> PageInfo {
    let media = Rect::new(0.0, 0.0, 595.0, 842.0);
    PageInfo {
        index: 0,
        object_id: (1, 0),
        media_box: media,
        crop_box: crop.unwrap_or(media),
        rotation,
        user_unit: 1.0,
        annots_indirect: None,
    }
}

fn transform() -> PageTransform {
    PageTransform::new(
        &page(Rotation::R0, None),
        ScreenGeometry { page_to_image: 2.0, image_to_screen: 0.5, offset: Point { x: 10.0, y: 20.0 } },
    )
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

#[test]
fn page_size_is_visible_cropbox() {
    let t = PageTransform::new(&page(Rotation::R90, None), ScreenGeometry::identity());
    assert_eq!((t.page_width, t.page_height), (842.0, 595.0));
    let t =
        PageTransform::new(&page(Rotation::R0, Some(Rect::new(50.0, 50.0, 545.0, 792.0))), ScreenGeometry::identity());
    assert_eq!((t.page_width, t.page_height), (495.0, 742.0));
}

#[test]
fn convert_chain_like_signature_panel() {
    let t = transform();
    let abs = Point { x: 100.0, y: 742.0 };
    let rel = t.convert(abs, CoordSpace::PageAbs, CoordSpace::PageRel);
    assert_eq!(rel, Point { x: 100.0, y: 100.0 });
    assert_eq!(t.convert(abs, CoordSpace::PageAbs, CoordSpace::Image), Point { x: 200.0, y: 200.0 });
    assert_eq!(t.convert(abs, CoordSpace::PageAbs, CoordSpace::ScreenRel), Point { x: 100.0, y: 100.0 });
    assert_eq!(t.convert(abs, CoordSpace::PageAbs, CoordSpace::ScreenAbs), Point { x: 110.0, y: 120.0 });
    // zurück
    assert_eq!(t.convert(Point { x: 110.0, y: 120.0 }, CoordSpace::ScreenAbs, CoordSpace::PageAbs), abs);
    assert_eq!(t.convert(Point { x: 200.0, y: 200.0 }, CoordSpace::Image, CoordSpace::PageRel), rel);
    assert_eq!(t.convert(abs, CoordSpace::PageAbs, CoordSpace::PageAbs), abs);
}

const SPACES: [CoordSpace; 5] =
    [CoordSpace::PageAbs, CoordSpace::PageRel, CoordSpace::Image, CoordSpace::ScreenRel, CoordSpace::ScreenAbs];

proptest! {
    #[test]
    fn convert_roundtrips(x in -1000.0f32..2000.0, y in -1000.0f32..2000.0, a in 0usize..5, b in 0usize..5,
                          k1 in 0.1f32..8.0, k2 in 0.1f32..4.0, ox in -50.0f32..50.0, oy in -50.0f32..50.0) {
        let t = PageTransform::new(&page(Rotation::R0, None),
            ScreenGeometry { page_to_image: k1, image_to_screen: k2, offset: Point { x: ox, y: oy } });
        let p = Point { x, y };
        let there = t.convert(p, SPACES[a], SPACES[b]);
        let back = t.convert(there, SPACES[b], SPACES[a]);
        prop_assert!((back.x - p.x).abs() < 0.05 && (back.y - p.y).abs() < 0.05, "{p:?} → {there:?} → {back:?}");
        // Umweg über PAGE_REL ergibt dasselbe
        let via = t.convert(t.convert(p, SPACES[a], CoordSpace::PageRel), CoordSpace::PageRel, SPACES[b]);
        prop_assert!((via.x - there.x).abs() < 0.05 && (via.y - there.y).abs() < 0.05);
    }
}

#[test]
fn convert_size_scales_without_offset() {
    let t = transform();
    let s = Point { x: 230.0, y: 49.0 };
    assert_eq!(t.convert_size(s, CoordSpace::PageRel, CoordSpace::ScreenRel), s);
    assert_eq!(t.convert_size(s, CoordSpace::PageAbs, CoordSpace::Image), Point { x: 460.0, y: 98.0 });
    assert!(close(t.convert_size(Point { x: 460.0, y: 98.0 }, CoordSpace::Image, CoordSpace::PageAbs), s));
    assert_eq!(t.convert_size(s, CoordSpace::ScreenAbs, CoordSpace::ScreenRel), s);
}

#[test]
fn clamp_block_like_f35() {
    let t = transform();
    assert_eq!(t.clamp_block(Point { x: 100.0, y: 500.0 }, 230.0, 49.0), Point { x: 100.0, y: 500.0 });
    assert_eq!(t.clamp_block(Point { x: -5.0, y: 900.0 }, 230.0, 49.0), Point { x: 0.0, y: 842.0 });
    assert_eq!(t.clamp_block(Point { x: 500.0, y: 10.0 }, 230.0, 49.0), Point { x: 365.0, y: 49.0 });
    // größer als die Seite: links bzw. oben anliegend
    assert_eq!(t.clamp_block(Point { x: 50.0, y: 50.0 }, 700.0, 900.0), Point { x: 0.0, y: 842.0 });
}

#[test]
fn clamp_never_panics_on_nan_or_infinite_geometry() {
    // f32::clamp bricht bei NaN-Grenzen ab; eine feindliche CropBox [inf 0 inf 842] ergibt NaN-Breite.
    let hostile = page(Rotation::R0, Some(Rect::new(f32::INFINITY, 0.0, f32::INFINITY, 842.0)));
    let t = PageTransform::new(&hostile, ScreenGeometry::identity());
    assert!(t.page_width.is_nan());
    let p = t.clamp_block(Point { x: 10.0, y: 500.0 }, 230.0, 49.0);
    assert_eq!(p, Point { x: 0.0, y: 500.0 });
    let t = transform();
    let nan = f32::NAN;
    assert_eq!(t.clamp_block(Point { x: 10.0, y: 500.0 }, nan, 49.0), Point { x: 0.0, y: 500.0 });
    assert_eq!(t.clamp_block(Point { x: 10.0, y: 500.0 }, 230.0, nan), Point { x: 10.0, y: 842.0 });
    assert_eq!(t.clamp_block(Point { x: nan, y: nan }, 230.0, 49.0), Point { x: 0.0, y: 49.0 });
    assert_eq!(t.clamp_block(Point { x: 10.0, y: 500.0 }, f32::INFINITY, f32::INFINITY), Point { x: 0.0, y: 842.0 });
    assert_eq!(t.nudge(Point { x: 10.0, y: 500.0 }, nan, f32::INFINITY, 230.0, 49.0), Point { x: 0.0, y: 842.0 });
}

proptest! {
    #[test]
    fn clamp_is_total_and_inside_the_page(
        x in prop_oneof![Just(f32::NAN), Just(f32::INFINITY), Just(f32::NEG_INFINITY), -2000.0f32..3000.0],
        y in prop_oneof![Just(f32::NAN), Just(f32::INFINITY), -2000.0f32..3000.0],
        w in prop_oneof![Just(f32::NAN), 0.0f32..1000.0],
        h in prop_oneof![Just(f32::NAN), 0.0f32..1000.0],
    ) {
        let t = transform();
        let p = t.clamp_block(Point { x, y }, w, h);
        prop_assert!((0.0..=595.0).contains(&p.x), "{p:?}");
        prop_assert!((0.0..=842.0).contains(&p.y), "{p:?}");
        if w <= 595.0 {
            prop_assert!(p.x + w <= 595.0 + 1e-3);
        }
        if h <= 842.0 {
            prop_assert!(p.y - h >= -1e-3);
        }
    }
}

#[test]
fn keyboard_steps_and_nudge() {
    assert_eq!(KeyStep::Normal.pt(), KEYBOARD_STEP_PT);
    assert_eq!(KeyStep::Fine.pt(), KEYBOARD_FINE_STEP_PT);
    assert_eq!(KeyStep::Coarse.pt(), KEYBOARD_COARSE_STEP_PT);
    assert_eq!((KEYBOARD_STEP_PT, KEYBOARD_FINE_STEP_PT, KEYBOARD_COARSE_STEP_PT), (15.0, 1.0, 50.0));
    let t = transform();
    let start = Point { x: 100.0, y: 500.0 };
    // Pfeil hoch = y größer
    assert_eq!(t.nudge(start, 0.0, KeyStep::Normal.pt(), 230.0, 49.0), Point { x: 100.0, y: 515.0 });
    assert_eq!(t.nudge(start, -KeyStep::Fine.pt(), 0.0, 230.0, 49.0), Point { x: 99.0, y: 500.0 });
    assert_eq!(t.nudge(start, 1000.0, 0.0, 230.0, 49.0).x, 365.0);
    assert_eq!(
        t.nudge(Point { x: 0.0, y: 49.0 }, -KeyStep::Coarse.pt(), -KeyStep::Coarse.pt(), 230.0, 49.0),
        Point { x: 0.0, y: 49.0 }
    );
}

#[test]
fn fit_centers_like_paint() {
    // Bild 600×800 px in Panel 300×300: Skala 0,375 ⇒ 225×300, Versatz (37, 0)
    let g = ScreenGeometry::fit(600, 800, 2.0, 300, 300);
    assert_eq!(g.image_to_screen, 0.375);
    assert_eq!(g.offset, Point { x: 37.0, y: 0.0 });
    assert_eq!(g.page_to_image, 2.0);
    let g = ScreenGeometry::fit(800, 600, 1.0, 400, 600);
    assert_eq!((g.image_to_screen, g.offset), (0.5, Point { x: 0.0, y: 150.0 }));
    let g = ScreenGeometry::fit(0, 0, 1.0, 400, 600);
    assert_eq!(g.image_to_screen, 1.0);
    // Blockvorschau auf dem Bildschirm
    let t = PageTransform::new(&page(Rotation::R0, None), ScreenGeometry::fit(1190, 1684, 2.0, 595, 842));
    let top_left = t.convert(Point { x: 0.0, y: 842.0 }, CoordSpace::PageAbs, CoordSpace::ScreenAbs);
    assert!(close(top_left, Point { x: 0.0, y: 0.0 }), "{top_left:?}");
}
