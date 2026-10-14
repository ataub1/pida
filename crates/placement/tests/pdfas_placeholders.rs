//! Golden-Tests gegen die PDF-AS-Testdokumente (pdf-as-pdfbox-2 `SignatureFieldsAndPlaceHolderExtractorTest`,
//! `PDFBoxPlaceholderExtractorTest`) und gegen die Widget-Rects der von PDF-AS signierten Dokumente.

mod common;

use pdfover_pdf_cos::Rect;
use pdfover_placement::{
    Coord, PlaceholderMode, PlaceholderQuery, QrPlaceholder, ResolvedPosition, TablePos, find_empty_sig_fields,
    find_qr_placeholders, select_placeholder, widget_rect,
};

const DIR: &str = "pdf/pdfas/pdf-as-pdfbox-2";

struct Doc {
    bytes: Vec<u8>,
    lopdf: pdfover_pdf_cos::lopdf::Document,
}

fn doc(rel: &str) -> Doc {
    let bytes = common::read(rel);
    let lopdf = common::load(&bytes);
    Doc { bytes, lopdf }
}

fn placeholders(backend: &pdfover_render::PdfiumBackend, d: &Doc) -> Vec<QrPlaceholder> {
    let renderer = common::open(backend, &d.bytes);
    let pages = common::pages(&d.lopdf);
    find_qr_placeholders(renderer.as_ref(), &pages, &common::all_invocations(&d.lopdf, false)).unwrap()
}

/// `SignatureFieldsAndPlaceHolderExtractor.getNextUnusedSignaturePlaceHolder`: id 1, SORTED.
fn next_unused(backend: &pdfover_render::PdfiumBackend, rel: &str) -> Option<String> {
    let d = doc(rel);
    let found = placeholders(backend, &d);
    let query = PlaceholderQuery {
        id: Some("1".into()),
        mode: PlaceholderMode::Sorted,
        used_names: common::signature_locations(&d.lopdf),
    };
    select_placeholder(&found, &query).unwrap().map(|p| p.resource_name.clone())
}

#[test]
fn next_unused_placeholder_like_pdfas() {
    let backend = require_pdfium!();
    assert_eq!(next_unused(&backend, &format!("{DIR}/new_qr_2-2.pdf")).as_deref(), Some("Image5"));
    assert_eq!(next_unused(&backend, &format!("{DIR}/new_qr_2_signed.pdf")).as_deref(), Some("Image8"));
    assert_eq!(next_unused(&backend, &format!("{DIR}/new_qr_2_signed_signed.pdf")), None);
    assert_eq!(next_unused(&backend, &format!("{DIR}/new_qr_2_signed_signed_signed.pdf")), None);
    assert_eq!(next_unused(&backend, &format!("{DIR}/manySignFields.pdf")), None);
    assert_eq!(next_unused(&backend, &format!("{DIR}/data/platzhalter_en_de_test.pdf")).as_deref(), Some("Im48"));
    // wiederholte Aufrufe sind zustandslos (PDF-AS `subsequentCalls`)
    for _ in 0..2 {
        assert_eq!(next_unused(&backend, &format!("{DIR}/new_qr_2_signed.pdf")).as_deref(), Some("Image8"));
        assert_eq!(next_unused(&backend, &format!("{DIR}/new_qr_2-2.pdf")).as_deref(), Some("Image5"));
    }
}

#[test]
fn locations_of_existing_signatures() {
    let signed2 = doc(&format!("{DIR}/new_qr_2_signed_signed.pdf"));
    assert_eq!(common::signature_locations(&signed2.lopdf), ["Image5", "Image8"]);
    let signed1 = doc(&format!("{DIR}/new_qr_2_signed.pdf"));
    assert_eq!(common::signature_locations(&signed1.lopdf), ["Image5"]);
}

fn empty_field_names(rel: &str) -> Vec<String> {
    let d = doc(rel);
    find_empty_sig_fields(&common::sig_fields(&d.lopdf)).into_iter().map(|c| c.qualified_name).collect()
}

#[test]
fn empty_fields_like_pdfas() {
    assert!(empty_field_names(&format!("{DIR}/new_qr_2-2.pdf")).is_empty());
    let all: Vec<String> = (0..8).map(|i| format!("Signature_{i}")).collect();
    assert_eq!(empty_field_names(&format!("{DIR}/manySignFields.pdf")), all);
    let without4: Vec<String> = all.iter().filter(|n| *n != "Signature_4").cloned().collect();
    assert_eq!(empty_field_names(&format!("{DIR}/manySignFields_signed4.pdf")), without4);
    // Kandidaten tragen Widget, Seite und Rect
    let d = doc(&format!("{DIR}/manySignFields.pdf"));
    let c = find_empty_sig_fields(&common::sig_fields(&d.lopdf));
    assert!(c.iter().all(|c| c.page_index.is_some() && !c.rect.is_empty()), "{c:?}");
}

#[test]
fn web_sample_with_field_and_qr() {
    let backend = require_pdfium!();
    let rel = "pdf/pdfas/pdf-as-web/data/placeholder_sigfield_and_qr.pdf";
    let d = doc(rel);
    assert!(!find_empty_sig_fields(&common::sig_fields(&d.lopdf)).is_empty(), "leeres Signaturfeld erwartet");
    assert!(!placeholders(&backend, &d).is_empty(), "QR-Platzhalter erwartet");
}

/// Widget-Rect der Signatur mit `/Location == name` im signierten Dokument.
fn signed_widget_rect(d: &Doc, location: &str) -> Rect {
    use pdfover_pdf_cos::lopdf::Object;
    for f in common::sig_fields(&d.lopdf).iter().filter(|f| f.signed) {
        let field = d.lopdf.get_dictionary(f.field_id).unwrap();
        let v = match field.get(b"V").unwrap() {
            Object::Reference(id) => d.lopdf.get_dictionary(*id).unwrap(),
            Object::Dictionary(dict) => dict,
            other => panic!("unerwartetes /V: {other:?}"),
        };
        let loc =
            v.get(b"Location").ok().and_then(|o| o.as_str().ok()).map(|b| String::from_utf8_lossy(b).into_owned());
        if loc.as_deref() == Some(location) {
            return f.widgets[0].rect.normalized();
        }
    }
    panic!("keine Signatur mit /Location {location}");
}

#[test]
fn placeholder_position_matches_pdfas_widget_rects() {
    let backend = require_pdfium!();
    let unsigned = doc(&format!("{DIR}/new_qr_2-2.pdf"));
    let found = placeholders(&backend, &unsigned);
    let pages = common::pages(&unsigned.lopdf);
    let signed = doc(&format!("{DIR}/new_qr_2_signed_signed.pdf"));
    for name in ["Image5", "Image8"] {
        let ph = found.iter().find(|p| p.resource_name == name).unwrap_or_else(|| panic!("{name} nicht gefunden"));
        let expected = signed_widget_rect(&signed, name);
        let (Some(Coord::Value(x)), Some(Coord::Value(y)), Some(Coord::Value(w))) = (ph.pos.x, ph.pos.y, ph.pos.w)
        else {
            panic!("Platzhalterposition unvollständig: {}", ph.pos);
        };
        // Platzhalter-TablePos ersetzt die Position komplett; die Höhe liefert das Layout
        let h = expected.height();
        let resolved = ResolvedPosition {
            page_index: ph.page_index,
            new_page: false,
            x,
            y,
            width: w,
            height: h,
            rotation: 0,
            notices: vec![],
        };
        let r = widget_rect(&resolved, &pages[ph.page_index]);
        eprintln!("{name}: unser {r:?} PDF-AS {expected:?} ({})", ph.pos);
        for (k, a, b) in [("llx", r.llx, expected.llx), ("ury", r.ury, expected.ury), ("urx", r.urx, expected.urx)] {
            assert!((a - b).abs() <= 1.0, "{name}.{k}: {a} statt {b}");
        }
        assert_eq!(TablePos::parse(&ph.pos.to_string()).unwrap(), ph.pos);
    }
}
