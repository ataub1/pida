//! Integrationstests für `PdfiumBackend`/`PdfiumRenderer` (nur mit `PDFOVER_PDFIUM_DIR`).

mod common;

use std::sync::Arc;

use common::{PageSpec, PdfBuilder, build_doc, count, read, single_page};
use pdfover_render::{ForcedColorOptions, RenderBackend, RenderError, Renderer, RgbaImage, encode_png};

const MAGIC: [u8; 4] = [152, 254, 52, 255];

fn approx(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= tol)
}

/// Vordergrund (α 211) über MAGIC, wie ihn PDFium zusammensetzt.
fn fg_over_magic() -> [u8; 4] {
    let a = 211u32;
    let mix = |s: u8, d: u8| ((u32::from(s) * a + u32::from(d) * (255 - a)) / 255) as u8;
    [mix(234, 152), mix(14, 254), mix(184, 52), 255]
}

/// 2×2-RGB-Bild: rot, grün / blau, weiß.
fn rgb_2x2(b: &mut PdfBuilder) -> u32 {
    let data = [255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
    b.add_stream("/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8", &data)
}

#[test]
fn bind_is_idempotent_and_describes_build() {
    let backend = require_pdfium!();
    let again = pdfover_render::PdfiumBackend::bind(None).expect("zweite Bindung liefert dieselbe");
    assert_eq!(backend.library_path(), again.library_path());
    let d = backend.describe();
    assert!(d.starts_with("PDFium"), "{d}");
    assert!(d.contains("8100"), "Build aus VERSION erwartet: {d}");
    // Auch ein falsches Verzeichnis liefert nach erfolgreicher Bindung die bestehende Bindung.
    let other = pdfover_render::PdfiumBackend::bind(Some(std::path::Path::new("/gibt/es/nicht"))).unwrap();
    assert_eq!(other.library_path(), backend.library_path());
}

#[test]
fn page_count_and_sizes_with_rotation_and_cropbox() {
    let backend = require_pdfium!();
    let mut b = PdfBuilder::new();
    let _ = &mut b;
    let pages = vec![
        PageSpec::a4(""),
        PageSpec { rotate: 90, ..PageSpec::a4("") },
        PageSpec { crop_box: Some([50.0, 50.0, 545.0, 792.0]), ..PageSpec::a4("") },
        PageSpec { crop_box: Some([50.0, 50.0, 545.0, 792.0]), rotate: 270, ..PageSpec::a4("") },
        PageSpec { rotate: 180, media_box: [0.0, 0.0, 612.0, 792.0], ..PageSpec::a4("") },
    ];
    let data: Arc<[u8]> = build_doc(b, pages, "").into();
    let r = backend.open(data, None).unwrap();
    assert_eq!(r.page_count(), 5);
    let s = |i| {
        let s = r.page_size(i).unwrap();
        (s.width, s.height)
    };
    assert_eq!(s(0), (595.0, 842.0));
    assert_eq!(s(1), (842.0, 595.0));
    assert_eq!(s(2), (495.0, 742.0));
    assert_eq!(s(3), (742.0, 495.0));
    assert_eq!(s(4), (612.0, 792.0));
    assert!(matches!(r.page_size(5), Err(RenderError::PageOutOfRange { page: 5, count: 5 })));
    assert!(matches!(r.render_page(9, 1.0), Err(RenderError::PageOutOfRange { .. })));
    assert!(matches!(
        r.render_forced_color(9, &ForcedColorOptions::default()),
        Err(RenderError::PageOutOfRange { .. })
    ));
    assert!(matches!(r.image_objects(9), Err(RenderError::PageOutOfRange { .. })));
}

#[test]
fn render_page_draws_content_and_applies_rotation() {
    let backend = require_pdfium!();
    // Schwarzes Rechteck links unten im Benutzerraum (X 0..100, Y 0..50).
    let content = "0 g 0 0 100 50 re f";
    let upright = backend.open(single_page(PageSpec::a4(content)), None).unwrap();
    let img = upright.render_page(0, 1.0).unwrap();
    assert_eq!((img.width, img.height), (595, 842));
    // unten links schwarz, oben rechts weiß
    assert!(approx(img.pixel(10, 830), [0, 0, 0, 255], 8));
    assert!(approx(img.pixel(500, 10), [255, 255, 255, 255], 0));

    // /Rotate 90: Unterkante wird linke Kante, linke Kante wird Oberkante.
    let rotated = backend.open(single_page(PageSpec { rotate: 90, ..PageSpec::a4(content) }), None).unwrap();
    let img = rotated.render_page(0, 1.0).unwrap();
    assert_eq!((img.width, img.height), (842, 595));
    assert_eq!(count(&img, 0, 0, 50, 100, |p| approx(p, [0, 0, 0, 255], 8)), 50 * 100);
    assert!(approx(img.pixel(60, 10), [255, 255, 255, 255], 0));
    assert!(approx(img.pixel(10, 110), [255, 255, 255, 255], 0));

    // Maßstab 2: doppelte Größe
    let img2 = upright.render_page(0, 2.0).unwrap();
    assert_eq!((img2.width, img2.height), (1190, 1684));
    assert!(matches!(upright.render_page(0, 0.0), Err(RenderError::Render(_))));
    assert!(matches!(upright.render_page(0, f32::NAN), Err(RenderError::Render(_))));
    let png = encode_png(&img2).unwrap();
    assert_eq!(&png[1..4], b"PNG");
}

fn forced_color_doc() -> Arc<[u8]> {
    let mut b = PdfBuilder::new();
    let im = rgb_2x2(&mut b);
    let content = "1 g 100 600 200 100 re f \
                   0 g BT /F1 24 Tf 100 400 Td (Hello) Tj ET \
                   q 100 0 0 50 300 100 cm /Im1 Do Q";
    let page = PageSpec { resources: format!("/XObject << /Im1 {im} 0 R >>"), ..PageSpec::a4(content) };
    build_doc(b, vec![page], "").into()
}

#[test]
fn forced_color_replaces_colors_and_keeps_images() {
    let backend = require_pdfium!();
    let r = backend.open(forced_color_doc(), None).unwrap();
    let opts = ForcedColorOptions::default();
    let img = r.render_forced_color(0, &opts).unwrap();
    assert_eq!((img.width, img.height), (595, 842));
    // Hintergrund exakt MAGIC
    assert_eq!(img.pixel(5, 5), MAGIC);
    assert_eq!(img.pixel(590, 837), MAGIC);
    // weißes Rechteck (Y 600..700 ⇒ Zeilen 142..242) wird zur Signalfarbe
    let fg = fg_over_magic();
    assert_eq!(count(&img, 102, 144, 298, 240, |p| approx(p, fg, 3)), 196 * 96);
    // Text wird ersetzt: belegt, aber nirgends schwarz
    let text = count(&img, 95, 410, 200, 445, |p| p != MAGIC);
    assert!(text > 100, "Text belegt nur {text} Pixel");
    assert_eq!(count(&img, 0, 0, 595, 842, |p| approx(p, [0, 0, 0, 255], 40)), 0, "schwarz übrig");
    // Bild unverändert (Quadrantenmitte: oben links rot, unten rechts weiß)
    assert!(approx(img.pixel(320, 705), [255, 0, 0, 255], 40), "{:?}", img.pixel(320, 705));
    assert!(approx(img.pixel(380, 735), [255, 255, 255, 255], 40), "{:?}", img.pixel(380, 735));

    // keep_images = false: Bildfläche in Signalfarbe
    let no_img = r.render_forced_color(0, &ForcedColorOptions { keep_images: false, ..opts }).unwrap();
    assert_eq!(count(&no_img, 302, 694, 398, 740, |p| approx(p, fg, 3)), 96 * 46);

    // doppelte Auflösung
    let hi = r.render_forced_color(0, &ForcedColorOptions { dpi: 144.0, ..opts }).unwrap();
    assert_eq!((hi.width, hi.height), (1190, 1684));
    assert!(approx(hi.pixel(400, 380), fg, 3));
    assert!(matches!(r.render_forced_color(0, &ForcedColorOptions { dpi: 0.0, ..opts }), Err(RenderError::Render(_))));
}

#[test]
fn forced_color_on_rotated_page_scans_visible_orientation() {
    let backend = require_pdfium!();
    let r = backend.open(single_page(PageSpec { rotate: 90, ..PageSpec::a4("0 g 0 0 100 50 re f") }), None).unwrap();
    let img = r.render_forced_color(0, &ForcedColorOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (842, 595));
    let fg = fg_over_magic();
    assert_eq!(count(&img, 0, 0, 50, 100, |p| approx(p, fg, 3)), 50 * 100);
    assert_eq!(img.pixel(400, 300), MAGIC);
}

#[test]
fn forced_color_includes_widgets_and_annotations() {
    let backend = require_pdfium!();
    let mut b = PdfBuilder::new();
    let ap = b.add_stream("/Type /XObject /Subtype /Form /BBox [0 0 100 50]", b"0 g 0 0 100 50 re f");
    let page_id_placeholder = "PAGE";
    let _ = page_id_placeholder;
    let widget = b.reserve();
    let square_ap = b.add_stream("/Type /XObject /Subtype /Form /BBox [0 0 40 40]", b"0 0 1 rg 0 0 40 40 re f");
    let square =
        b.add(format!("<< /Type /Annot /Subtype /Square /Rect [100 700 140 740] /F 4 /AP << /N {square_ap} 0 R >> >>"));
    b.set(
        widget,
        format!(
            "<< /Type /Annot /Subtype /Widget /FT /Sig /T (Sig1) /F 4 /Rect [400 100 500 150] /AP << /N {ap} 0 R >> >>"
        ),
    );
    let page = PageSpec { extra: format!("/Annots [{widget} 0 R {square} 0 R]"), ..PageSpec::a4("") };
    let data: Arc<[u8]> =
        build_doc(b, vec![page], &format!("/AcroForm << /Fields [{widget} 0 R] /SigFlags 3 >>")).into();
    let r = backend.open(data, None).unwrap();
    let img = r.render_forced_color(0, &ForcedColorOptions::default()).unwrap();
    // Widget Y 100..150 ⇒ Zeilen 692..742
    let widget_px = count(&img, 402, 694, 498, 740, |p| p != MAGIC);
    assert_eq!(widget_px, 96 * 46, "Widget muss als Inhalt zählen");
    // Square-Annotation Y 700..740 ⇒ Zeilen 102..142
    let square_px = count(&img, 102, 104, 138, 140, |p| p != MAGIC);
    assert_eq!(square_px, 36 * 36, "Annotation muss als Inhalt zählen");
    // normales Rendern zeigt beides ebenfalls
    let normal = r.render_page(0, 1.0).unwrap();
    assert!(approx(normal.pixel(450, 717), [0, 0, 0, 255], 8));
    assert!(approx(normal.pixel(120, 120), [0, 0, 255, 255], 8));
}

#[test]
fn image_objects_with_matrices_forms_and_inline_images() {
    let backend = require_pdfium!();
    let mut b = PdfBuilder::new();
    let im = rgb_2x2(&mut b);
    let gray = b.add_stream(
        "/Type /XObject /Subtype /Image /Width 3 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
        &[0, 128, 255],
    );
    let form = b.add_stream(
        &format!("/Type /XObject /Subtype /Form /BBox [0 0 500 500] /Matrix [1 0 0 1 10 20] /Resources << /XObject << /Im1 {im} 0 R >> >>"),
        b"q 30 0 0 40 5 6 cm /Im1 Do Q",
    );
    let content = "q 100 0 0 50 72 600 cm /Im1 Do Q \
                   q 2 0 0 2 100 100 cm /Fm1 Do Q \
                   q 20 0 0 20 300 300 cm BI /W 2 /H 1 /CS /G /BPC 8 ID \x10\x70\nEI Q \
                   q 0 30 -30 0 400 400 cm /Gr Do Q";
    let page = PageSpec {
        resources: format!("/XObject << /Im1 {im} 0 R /Fm1 {form} 0 R /Gr {gray} 0 R >>"),
        ..PageSpec::a4(content)
    };
    let data: Arc<[u8]> = build_doc(b, vec![page], "").into();
    let r = backend.open(data, None).unwrap();
    let objs = r.image_objects(0).unwrap();
    assert_eq!(objs.len(), 4, "{objs:?}");
    for (i, o) in objs.iter().enumerate() {
        assert_eq!(o.order, i);
    }
    let close = |m: [f32; 6], e: [f32; 6]| m.iter().zip(e.iter()).all(|(a, b)| (a - b).abs() <= 0.01);
    assert!(close(objs[0].matrix, [100.0, 0.0, 0.0, 50.0, 72.0, 600.0]), "{:?}", objs[0].matrix);
    // Form: Bild-CTM · /Matrix · CTM beim Form-Do
    assert!(close(objs[1].matrix, [60.0, 0.0, 0.0, 80.0, 130.0, 152.0]), "{:?}", objs[1].matrix);
    assert!(close(objs[2].matrix, [20.0, 0.0, 0.0, 20.0, 300.0, 300.0]), "{:?}", objs[2].matrix);
    assert!(close(objs[3].matrix, [0.0, 30.0, -30.0, 0.0, 400.0, 400.0]), "{:?}", objs[3].matrix);
    // Bitmaps dekodiert (Zeile 0 = oberste Bildzeile)
    let rgb = &objs[0].bitmap;
    assert_eq!((rgb.width, rgb.height), (2, 2));
    assert_eq!(rgb.pixel(0, 0), [255, 0, 0, 255]);
    assert_eq!(rgb.pixel(1, 0), [0, 255, 0, 255]);
    assert_eq!(rgb.pixel(0, 1), [0, 0, 255, 255]);
    assert_eq!(rgb.pixel(1, 1), [255, 255, 255, 255]);
    assert_eq!(objs[1].bitmap, objs[0].bitmap);
    let inline = &objs[2].bitmap;
    assert_eq!((inline.width, inline.height), (2, 1));
    assert_eq!(inline.pixel(0, 0), [0x10, 0x10, 0x10, 255]);
    let g = &objs[3].bitmap;
    assert_eq!((g.width, g.height), (3, 1));
    assert_eq!([g.pixel(0, 0)[0], g.pixel(1, 0)[0], g.pixel(2, 0)[0]], [0, 128, 255]);
}

#[test]
fn password_protected_documents() {
    let backend = require_pdfium!();
    let data = read("pdf/generated/simple-aes256-userpw.pdf");
    assert!(matches!(backend.open(Arc::clone(&data), None), Err(RenderError::PasswordRequired)));
    assert!(matches!(backend.open(Arc::clone(&data), Some("falsch")), Err(RenderError::PasswordRequired)));
    let r = backend.open_document(Arc::clone(&data), Some("test")).unwrap();
    assert_eq!(r.page_count(), 1);
    let debug = format!("{r:?}");
    assert!(!debug.contains("test\""), "Passwort im Debug: {debug}");
    assert!(debug.contains("***"));
    // Forced-Color öffnet die zweite Instanz mit demselben Passwort
    let img = r.render_forced_color(0, &ForcedColorOptions::default()).unwrap();
    assert!(count(&img, 0, 0, img.width, img.height, |p| p != MAGIC) > 0);
    // nur Owner-Passwort: ohne Passwort lesbar
    let owner = backend.open(read("pdf/generated/simple-aes256-owner.pdf"), None).unwrap();
    assert_eq!(owner.page_count(), 1);
}

#[test]
fn garbage_is_a_load_error() {
    let backend = require_pdfium!();
    let r = backend.open(Arc::from(&b"kein pdf"[..]), None);
    assert!(matches!(r, Err(RenderError::Load(_))), "{:?}", r.err());
    let r = backend.open(Arc::from(&b""[..]), None);
    assert!(matches!(r, Err(RenderError::Load(_))), "{:?}", r.err());
}

#[test]
fn backend_is_usable_from_many_threads() {
    let backend = require_pdfium!();
    let backend: Arc<dyn RenderBackend> = Arc::new(backend);
    let data = forced_color_doc();
    let baseline = {
        let r = backend.open(Arc::clone(&data), None).unwrap();
        (
            r.render_page(0, 0.5).unwrap(),
            r.render_forced_color(0, &ForcedColorOptions::default()).unwrap(),
            r.image_objects(0).unwrap(),
        )
    };
    let baseline = Arc::new(baseline);
    let mut handles = Vec::new();
    for t in 0..8 {
        let backend = Arc::clone(&backend);
        let data = Arc::clone(&data);
        let baseline = Arc::clone(&baseline);
        handles.push(std::thread::spawn(move || {
            // Renderer wandern zwischen Threads (Send)
            let r: Box<dyn Renderer> = backend.open(Arc::clone(&data), None).unwrap();
            for i in 0..6 {
                match (t + i) % 3 {
                    0 => assert_eq!(r.render_page(0, 0.5).unwrap(), baseline.0),
                    1 => assert_eq!(r.render_forced_color(0, &ForcedColorOptions::default()).unwrap(), baseline.1),
                    _ => assert_eq!(r.image_objects(0).unwrap(), baseline.2),
                }
            }
            r
        }));
    }
    let renderers: Vec<Box<dyn Renderer>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // Drop in einem anderen Thread als dem erzeugenden
    std::thread::spawn(move || drop(renderers)).join().unwrap();
}

#[test]
fn pdfas_suite_pages_render() {
    let backend = require_pdfium!();
    let r = backend.open(read("pdf/pdfas-suites/POS_AUTO/example.pdf"), None).unwrap();
    assert_eq!(r.page_count(), 1);
    let size = r.page_size(0).unwrap();
    assert!((size.width - 595.28).abs() < 0.1 && (size.height - 841.89).abs() < 0.1, "{size:?}");
    let img: RgbaImage = r.render_forced_color(0, &ForcedColorOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (595, 841));
    // Textblock in der Mitte belegt, unterer Rand frei, Fußzeile belegt
    assert!(count(&img, 0, 300, 595, 340, |p| p != MAGIC) > 500);
    assert_eq!(count(&img, 0, 600, 595, 740, |p| p != MAGIC), 0);
    assert!(count(&img, 0, 780, 595, 841, |p| p != MAGIC) > 100);
}
