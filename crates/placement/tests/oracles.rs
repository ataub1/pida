//! Externe Orakel: `qpdf --check` für die selbst erzeugten Testdokumente (ohne `qpdf`
//! übersprungen) und PDFium-Seitengrößen gegen `PageInfo::visible_size` (die Positionierung
//! rechnet mit `PageInfo`, der Rasterscan mit PDFium – beide müssen übereinstimmen).

mod common;

use std::process::Command;

use common::{PageSpec, PdfBuilder, build_doc, qr_image};

fn qpdf_available() -> bool {
    Command::new("qpdf").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn sample_documents() -> Vec<(&'static str, Vec<u8>)> {
    let mut b = PdfBuilder::new();
    let im = b.add_gray_image(&qr_image("PDF-AS-POS;id=1", 3));
    let form = b.add_stream(
        &format!("/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Matrix [1 0 0 1 5 5] /Resources << /XObject << /Im {im} 0 R >> >>"),
        b"q 50 0 0 50 0 0 cm /Im Do Q",
    );
    let qr = build_doc(
        b,
        vec![
            PageSpec {
                resources: format!("/XObject << /Im {im} 0 R /Fm {form} 0 R >>"),
                ..PageSpec::a4("q 100 0 0 100 50 600 cm /Im Do Q /Fm Do")
            },
            PageSpec { rotate: 90, crop_box: Some([10.0, 20.0, 580.0, 820.0]), ..PageSpec::a4("0 g 0 0 100 50 re f") },
        ],
        "",
    );
    let mut b = PdfBuilder::new();
    let ap = b.add_stream("/Type /XObject /Subtype /Form /BBox [0 0 100 50]", b"0 g 0 0 100 50 re f");
    let widget = b.reserve();
    b.set(
        widget,
        format!(
            "<< /Type /Annot /Subtype /Widget /FT /Sig /T (Sig1) /F 4 /Rect [400 100 500 150] /AP << /N {ap} 0 R >> >>"
        ),
    );
    let fields = build_doc(
        b,
        vec![PageSpec { extra: format!("/Annots [{widget} 0 R]"), ..PageSpec::a4("") }],
        &format!("/AcroForm << /Fields [{widget} 0 R] /SigFlags 3 >>"),
    );
    vec![("qr", qr), ("fields", fields)]
}

#[test]
fn generated_test_documents_pass_qpdf_check() {
    if !qpdf_available() {
        eprintln!("qpdf nicht gefunden – Test übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in sample_documents() {
        let path = dir.path().join(format!("{name}.pdf"));
        std::fs::write(&path, &bytes).unwrap();
        let out = Command::new("qpdf").arg("--check").arg(&path).output().unwrap();
        // 0 = in Ordnung, 3 = nur Warnungen
        assert!(
            matches!(out.status.code(), Some(0) | Some(3)),
            "{name}: qpdf --check {:?}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout)
        );
    }
}

#[test]
fn lopdf_helper_reads_the_generated_structures() {
    let docs = sample_documents();
    let qr = common::load(&docs[0].1);
    let pages = common::pages(&qr);
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[1].rotation, pdfover_pdf_cos::Rotation::R90);
    assert_eq!(pages[1].visible_size(), (800.0, 570.0));
    let flat = common::image_invocations(&qr, 0, false);
    assert_eq!(flat.len(), 1);
    let deep = common::image_invocations(&qr, 0, true);
    assert_eq!(deep.len(), 2);
    assert!(deep[1].inside_form_xobject);
    assert_eq!(deep[1].ctm, [50.0, 0.0, 0.0, 50.0, 5.0, 5.0]);
    let fields = common::sig_fields(&common::load(&docs[1].1));
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].qualified_name, "Sig1");
    assert_eq!(fields[0].widgets[0].page_index, Some(0));
    assert!(!fields[0].signed);
}

#[test]
fn pdfium_page_sizes_match_page_info() {
    let backend = require_pdfium!();
    let mut files: Vec<(String, Vec<u8>)> = sample_documents().into_iter().map(|(n, b)| (n.to_owned(), b)).collect();
    for rel in [
        "pdf/pdfas/pdf-as-cli/rotate_090.pdf",
        "pdf/pdfas/pdf-as-cli/rotate_180.pdf",
        "pdf/pdfas/pdf-as-cli/rotate_270.pdf",
        "pdf/pdfas/pdf-as-cli/simple_rotated_90.pdf",
        "pdf/pdfas/pdf-as-cli/simple_rotated_270.pdf",
        "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf",
        "pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf",
        "placement/pdfas-pos/POS_AUTO/example.pdf",
        "placement/pdfas-pos/POS_LAST_ALREADY_SIGNED/example.pdf",
        "placement/pdfas-pos/POS_SIGNATURBLOCK_DE/example.pdf",
    ] {
        files.push((rel.to_owned(), common::read(rel)));
    }
    for (name, bytes) in files {
        let pages = common::pages(&common::load(&bytes));
        let renderer = common::open(&backend, &bytes);
        assert_eq!(renderer.page_count(), pages.len(), "{name}");
        for p in &pages {
            let s = renderer.page_size(p.index).unwrap();
            let (w, h) = p.visible_size();
            assert!(
                (s.width - w).abs() < 0.01 && (s.height - h).abs() < 0.01,
                "{name} Seite {}: PDFium {s:?} ≠ ({w}, {h})",
                p.index
            );
        }
    }
}
