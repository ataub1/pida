//! QR-Platzhalter: selbst erzeugte Testdokumente (QR mit dem `rxing`-Encoder als Bild-XObject),
//! Dekodierung über PDFium-Bildobjekte, Zuordnung zu den `Do`-Aufrufen, Position und Auswahl.

mod common;

use common::{GrayImage, PageSpec, PdfBuilder, build_doc, generator_like_image, qr_image, to_rgba};
use pdfover_placement::{
    Coord, PageSelector, PlaceholderMode, PlaceholderQuery, QrPlaceholder, decode_qr, find_qr_placeholders,
    select_placeholder,
};
use pdfover_render::RgbaImage;

#[test]
fn decode_generated_qr_codes() {
    let text = "PDF-AS-POS;id=1;profile=SIGNATURBLOCK_SMALL_DE";
    for module_px in 2..=6 {
        assert_eq!(decode_qr(&to_rgba(&qr_image(text, module_px))), vec![text.to_owned()], "{module_px} px je Modul");
    }
    // QR als Teil eines größeren Bildes (PDF-AS-Generator: QR links, Text rechts)
    assert_eq!(
        decode_qr(&to_rgba(&generator_like_image("PDF-AS-POS;id=12345"))),
        vec!["PDF-AS-POS;id=12345".to_owned()]
    );
    // transparente helle Module zählen als weiß (ZXing BufferedImageLuminanceSource)
    let g = qr_image("PDF-AS-POS;id=3", 3);
    let mut rgba = Vec::new();
    for v in &g.data {
        rgba.extend_from_slice(if *v == 0 { &[0, 0, 0, 255] } else { &[0, 0, 0, 0] });
    }
    let transparent = RgbaImage::from_raw(g.width, g.height, rgba).unwrap();
    assert_eq!(decode_qr(&transparent), vec!["PDF-AS-POS;id=3".to_owned()]);
    // Rauschen ist kein QR-Code
    let noise: Vec<u8> = (0..64u32 * 64)
        .flat_map(|i| {
            let v = if (i.wrapping_mul(2_654_435_761) >> 13) & 1 == 0 { 0 } else { 255 };
            [v, v, v, 255]
        })
        .collect();
    assert!(decode_qr(&RgbaImage::from_raw(64, 64, noise).unwrap()).is_empty());
}

fn gray_inline(img: &GrayImage) -> Vec<u8> {
    let mut v = format!("BI /W {} /H {} /CS /G /BPC 8 ID ", img.width, img.height).into_bytes();
    v.extend_from_slice(&img.data);
    v.extend_from_slice(b"\nEI");
    v
}

/// Testdokument mit allen Fällen (Seiten 1–6).
fn build_qr_doc() -> Vec<u8> {
    let mut b = PdfBuilder::new();
    let im1 = b.add_gray_image(&qr_image("PDF-AS-POS;id=2", 4));
    let im2 = b.add_gray_image(&qr_image("PDF-AS-POS;id=1;profile=SIGNATURBLOCK_SMALL_DE", 4));
    let im3 = b.add_gray_image(&qr_image("hello world", 4));
    let im4 = b.add_gray_image(&GrayImage { width: 8, height: 8, data: vec![0; 64] });
    let im5 = b.add_gray_image(&qr_image("PDF-AS-POS;id=5", 4));
    let im6 = b.add_gray_image(&qr_image("PDF-AS-POS;id=6", 4));
    let im7 = b.add_gray_image(&qr_image("PDF-AS-POS;id=7", 4));
    let im8 = b.add_gray_image(&qr_image("PDF-AS-POS;id=8", 4));
    let im10 = b.add_gray_image(&generator_like_image("PDF-AS-POS;id=10"));
    let form = b.add_stream(
        &format!("/Type /XObject /Subtype /Form /BBox [0 0 595 842] /Matrix [1 0 0 1 10 20] /Resources << /XObject << /Im5 {im5} 0 R >> >>"),
        b"q 100 0 0 100 50 50 cm /Im5 Do Q",
    );

    let mut p1 = b"q 100 0 0 100 50.4 600.6 cm /Im1 Do Q \
                   q 80 0 0 80 300 500 cm /Im2 Do Q \
                   q 90 0 0 90 400 300 cm /Im3 Do Q \
                   q 20 0 0 20 100 100 cm /Im4 Do Q \
                   q 60 0 0 60 200 150 cm "
        .to_vec();
    p1.extend_from_slice(&gray_inline(&qr_image("PDF-AS-POS;id=9", 2)));
    p1.extend_from_slice(b" Q q 50 0 0 50 450 100 cm /Im1 Do Q");
    let page1 = PageSpec {
        content: p1,
        resources: format!("/XObject << /Im1 {im1} 0 R /Im2 {im2} 0 R /Im3 {im3} 0 R /Im4 {im4} 0 R >>"),
        ..PageSpec::a4("")
    };
    let page2 =
        PageSpec { resources: format!("/XObject << /Fm1 {form} 0 R >>"), ..PageSpec::a4("q 2 0 0 2 0 0 cm /Fm1 Do Q") };
    // /Rotate 90: Bild im Benutzerraum gegen den Uhrzeigersinn gedreht ⇒ sichtbar aufrecht
    let page3 = PageSpec {
        rotate: 90,
        resources: format!("/XObject << /Im6 {im6} 0 R >>"),
        ..PageSpec::a4("q 0 100 -100 0 300 200 cm /Im6 Do Q")
    };
    let page4 = PageSpec {
        crop_box: Some([20.0, 30.0, 575.0, 812.0]),
        resources: format!("/XObject << /Im7 {im7} 0 R >>"),
        ..PageSpec::a4("q 100 0 0 100 72 600 cm /Im7 Do Q")
    };
    let page5 = PageSpec {
        resources: format!("/XObject << /Im8 {im8} 0 R >>"),
        ..PageSpec::a4("q 100 0 0 100 -30 600 cm /Im8 Do Q")
    };
    let page6 = PageSpec {
        resources: format!("/XObject << /Im10 {im10} 0 R >>"),
        ..PageSpec::a4("q 300 0 0 60 72 72 cm /Im10 Do Q")
    };
    build_doc(b, vec![page1, page2, page3, page4, page5, page6], "")
}

fn summary(p: &QrPlaceholder) -> (usize, String, Option<String>, String) {
    (p.page_index, p.resource_name.clone(), p.payload.id.clone(), p.pos.to_string())
}

#[test]
fn find_placeholders_in_generated_document() {
    let backend = require_pdfium!();
    let bytes = build_qr_doc();
    let doc = common::load(&bytes);
    let pages = common::pages(&doc);
    let renderer = common::open(&backend, &bytes);
    let invocations = common::all_invocations(&doc, false);
    let found = find_qr_placeholders(renderer.as_ref(), &pages, &invocations).unwrap();
    let got: Vec<_> = found.iter().map(summary).collect();
    let s = |page: usize, name: &str, id: &str, pos: &str| (page, name.to_owned(), Some(id.to_owned()), pos.to_owned());
    assert_eq!(
        got,
        vec![
            s(0, "Im1", "2", "p:1;x:50;y:701;w:100"),
            s(0, "Im2", "1", "p:1;x:300;y:580;w:80"),
            s(0, "Im1", "2", "p:1;x:450;y:150;w:50"),
            // Seite 2: Form-XObject ohne descend_forms ignoriert
            s(2, "Im6", "6", "p:3;x:200;y:395;w:100"),
            s(3, "Im7", "7", "p:4;x:52;y:670;w:100"),
            // Seite 5: x < 0 ⇒ übersprungen (KR-F05)
            s(5, "Im10", "10", "p:6;x:72;y:132;w:300"),
        ]
    );
    assert_eq!(found[1].payload.profile.as_deref(), Some("SIGNATURBLOCK_SMALL_DE"));
    assert_eq!(found[1].ctm, [80.0, 0.0, 0.0, 80.0, 300.0, 500.0]);
    assert!(matches!(found[0].pos.page, Some(PageSelector::Exact(1))));
    assert_eq!(found[0].pos.w, Some(Coord::Value(100.0)));

    // mit descend_forms: Bild im Form-XObject (CTM · /Matrix verkettet)
    let with_forms = common::all_invocations(&doc, true);
    let found_forms = find_qr_placeholders(renderer.as_ref(), &pages, &with_forms).unwrap();
    let form_hit = found_forms.iter().find(|p| p.page_index == 1).expect("Platzhalter im Form-XObject");
    // Bild: [100 0 0 100 50 50] · /Matrix [1 0 0 1 10 20] · CTM [2 0 0 2 0 0] = [200 0 0 200 120 140]
    assert_eq!(summary(form_hit), s(1, "Im5", "5", "p:2;x:120;y:340;w:200"));

    // Auswahl wie PDF-Over: id 1 SORTED ⇒ Im2; danach (Im2 benutzt) id 2
    let q = |id: Option<&str>, used: &[&str]| PlaceholderQuery {
        id: id.map(str::to_owned),
        mode: PlaceholderMode::Sorted,
        used_names: used.iter().map(|u| (*u).to_owned()).collect(),
    };
    assert_eq!(select_placeholder(&found, &q(Some("1"), &[])).unwrap().unwrap().resource_name, "Im2");
    let next = select_placeholder(&found, &q(Some("1"), &["Im2"])).unwrap().unwrap();
    assert_eq!((next.resource_name.as_str(), next.payload.id.as_deref()), ("Im1", Some("2")));
    // gleicher Ressourcenname auf mehreren Aufrufen: beide gelten als benutzt
    let rest = select_placeholder(&found, &q(Some("1"), &["Im2", "Im1"])).unwrap().unwrap();
    assert_eq!(rest.resource_name, "Im6");
}

#[test]
fn same_image_twice_at_same_matrix_is_assigned_once_each() {
    let backend = require_pdfium!();
    let mut b = PdfBuilder::new();
    let im = b.add_gray_image(&qr_image("PDF-AS-POS;id=1", 3));
    let page = PageSpec {
        resources: format!("/XObject << /A {im} 0 R /B {im} 0 R >>"),
        ..PageSpec::a4("q 100 0 0 100 50 600 cm /A Do Q q 100 0 0 100 50 600 cm /B Do Q")
    };
    let bytes = build_doc(b, vec![page], "");
    let doc = common::load(&bytes);
    let renderer = common::open(&backend, &bytes);
    let found =
        find_qr_placeholders(renderer.as_ref(), &common::pages(&doc), &common::all_invocations(&doc, false)).unwrap();
    let names: Vec<&str> = found.iter().map(|p| p.resource_name.as_str()).collect();
    assert_eq!(names, ["A", "B"], "Tie-Breaker über order");
}

#[test]
fn pages_without_invocations_are_not_rendered_and_errors_propagate() {
    let backend = require_pdfium!();
    let bytes = build_doc(PdfBuilder::new(), vec![PageSpec::a4("0 g 0 0 10 10 re f")], "");
    let doc = common::load(&bytes);
    let renderer = common::open(&backend, &bytes);
    assert!(find_qr_placeholders(renderer.as_ref(), &common::pages(&doc), &[]).unwrap().is_empty());
    // Aufruf für eine Seite, die der Renderer nicht kennt
    let bogus = pdfover_pdf_cos::ImageInvocation {
        page_index: 7,
        order: 0,
        resource_name: "X".into(),
        xobject_id: None,
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        pixel_width: 1,
        pixel_height: 1,
        inside_form_xobject: false,
    };
    let mut pages = common::pages(&doc);
    pages.push(pdfover_pdf_cos::PageInfo { index: 7, ..pages[0].clone() });
    assert!(find_qr_placeholders(renderer.as_ref(), &pages, &[bogus]).is_err());
}
