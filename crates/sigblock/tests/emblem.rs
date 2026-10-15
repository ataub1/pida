//! Bildmarke: Formate, EXIF (nur JPEG wie PDF-Over), Verkleinerung, Transparenz
//! (docs/analyse/04 §7). Fixtures: `testdata/sigblock/images/` (`tools/make_image_fixtures.py`).

use std::path::Path;

use pida_sigblock::{Emblem, MAX_EMBLEM_HEIGHT, MAX_EMBLEM_WIDTH};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/sigblock/images").join(name)).unwrap()
}

fn px(e: &Emblem, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * e.width + x) * 4) as usize;
    [e.rgba[i], e.rgba[i + 1], e.rgba[i + 2], e.rgba[i + 3]]
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60
}

#[test]
fn jpeg_exif_orientation_is_applied() {
    let plain = Emblem::decode(&fixture("plain.jpg")).unwrap();
    assert_eq!((plain.width, plain.height), (40, 20));
    assert!(is_red(px(&plain, 2, 10)) && is_blue(px(&plain, 37, 10)));
    let rotated = Emblem::decode(&fixture("exif6.jpg")).unwrap();
    // Orientierung 6: 90° im Uhrzeigersinn ⇒ links (rot) wird oben
    assert_eq!((rotated.width, rotated.height), (20, 40));
    assert!(is_red(px(&rotated, 10, 2)), "{:?}", px(&rotated, 10, 2));
    assert!(is_blue(px(&rotated, 10, 37)));
    assert!(!rotated.has_alpha);
    // Profilbilder (PDF-AS ImageIO.read) ohne EXIF
    let original = Emblem::decode_original(&fixture("exif6.jpg")).unwrap();
    assert_eq!((original.width, original.height), (40, 20));
}

#[test]
fn png_exif_is_ignored_like_pdf_over() {
    let e = Emblem::decode(&fixture("exif6.png")).unwrap();
    assert_eq!((e.width, e.height), (40, 20));
}

#[test]
fn gif_transparency() {
    let e = Emblem::decode(&fixture("transparent.gif")).unwrap();
    assert_eq!((e.width, e.height), (10, 10));
    assert!(e.has_alpha);
    assert_eq!(px(&e, 0, 0)[3], 0);
    assert_eq!(px(&e, 9, 9), [0, 128, 0, 255]);
    let flat = e.flattened_on_white();
    assert_eq!(px(&flat, 0, 0), [255, 255, 255, 255]);
    assert!(!flat.has_alpha);
}

#[test]
fn large_images_are_reduced_and_cached_as_png() {
    let e = Emblem::decode(&fixture("large.png")).unwrap();
    assert!(e.width <= MAX_EMBLEM_WIDTH && e.height <= MAX_EMBLEM_HEIGHT);
    assert_eq!((e.width, e.height), (400, 600));
    let png = e.to_png().unwrap();
    let back = Emblem::decode(&png).unwrap();
    assert_eq!((back.width, back.height), (400, 600));
    assert_eq!(back.rgba, e.rgba);
    // Cache-Schlüssel ist der Hash der Originaldatei
    assert_ne!(back.source_sha256, e.source_sha256);
}

#[test]
fn debug_output_has_no_pixels() {
    let e = Emblem::decode(&fixture("plain.jpg")).unwrap();
    let d = format!("{e:?}");
    assert!(d.contains("width: 40") && !d.contains("rgba"), "{d}");
}
