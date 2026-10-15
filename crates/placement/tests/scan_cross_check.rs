//! Gegenprobe der Freiraumsuche (Plan §6.7): unterste belegte Rasterzeile des Forced-Color-Scans
//! gegen die Hüllrechtecke der PDFium-Seitenobjekte (`PdfiumRenderer::lowest_object_edge`).
//!
//! Hüllrechtecke enthalten die gezeichneten Pixel, können aber größer sein (Glyphenhüllen mit
//! Unterlänge): Die Objektkante liegt daher höchstens 1 pt über (Rasterrundung) und nie viel tiefer
//! als die Scanzeile.

mod common;

use std::sync::Arc;

use common::{PageSpec, PdfBuilder, build_doc};
use pida_placement::{DEFAULT_FOOTER, FreeSpaceScanner, RasterScanner};
use pida_render::{ForcedColorOptions, PdfiumBackend, Renderer};

struct Row {
    page: usize,
    footer: f32,
    scan: f32,
    edge: Option<f32>,
}

/// Scan und Objektkante (pt von oben) für jede Seite, Fußzeile 0 und 80.
fn compare(backend: &PdfiumBackend, bytes: &[u8]) -> Vec<Row> {
    let r = backend.open_document(Arc::from(bytes), None).expect("PDF lesbar");
    let mut out = Vec::new();
    for page in 0..r.page_count() {
        for footer in [0.0, DEFAULT_FOOTER] {
            let mut scanner = RasterScanner { renderer: &r, options: ForcedColorOptions::default() };
            let scan = scanner.lowest_content(page, footer).unwrap();
            let edge = r.lowest_object_edge(page, footer).unwrap();
            out.push(Row { page, footer, scan, edge });
        }
    }
    out
}

fn check(name: &str, rows: &[Row], max_slack: f32, failures: &mut Vec<String>) {
    for row in rows {
        let ok = match row.edge {
            None => row.scan == 0.0,
            Some(edge) => {
                let d = edge - row.scan;
                (-1.0..=max_slack).contains(&d)
            }
        };
        if !ok {
            failures.push(format!(
                "{name} Seite {} f{}: Scan {} / Objektkante {:?}",
                row.page, row.footer, row.scan, row.edge
            ));
        }
    }
}

#[test]
fn raster_scan_agrees_with_object_bounds_on_pdfas_suites() {
    let backend = require_pdfium!();
    let dir = common::testdata("placement/pdfas-pos");
    let mut suites: Vec<_> =
        std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    suites.sort();
    assert_eq!(suites.len(), 26);
    let mut failures = Vec::new();
    for suite in &suites {
        let rows = compare(&backend, &std::fs::read(suite.join("example.pdf")).unwrap());
        let name = suite.file_name().unwrap().to_string_lossy().into_owned();
        // oberhalb der Fußzeile f:80 (dort entscheidet die Positionierung): höchstens 1 pt
        let (with_footer, without): (Vec<Row>, Vec<Row>) = rows.into_iter().partition(|r| r.footer > 0.0);
        check(&name, &with_footer, 1.0, &mut failures);
        // ohne Fußzeile auch Fußzeilentext (Glyphenhüllen mit Unterlänge)
        check(&name, &without, 10.0, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn raster_scan_agrees_with_object_bounds_on_generated_pages() {
    let backend = require_pdfium!();
    let pages = vec![
        PageSpec::a4(""),
        // Rechteck Y 300..400 ⇒ Unterkante 842 − 300 = 542 pt von oben
        PageSpec::a4("1 g 100 300 200 100 re f"),
        // weißer Text zählt ebenfalls (Farbe wird ersetzt)
        PageSpec::a4("1 g BT /F1 30 Tf 100 500 Td (Hallo) Tj ET"),
        // gedrehte Seite: Rechteck X 0..100 ⇒ sichtbar oben (Zeilen 0..100)
        PageSpec { rotate: 90, ..PageSpec::a4("0 g 0 0 100 50 re f") },
        // Inhalt nur in der Fußzeile (Y 10..40)
        PageSpec::a4("0 g 100 10 300 30 re f"),
    ];
    let bytes = build_doc(PdfBuilder::new(), pages, "");
    let rows = compare(&backend, &bytes);
    let mut failures = Vec::new();
    check("generiert", &rows, 1.0, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let at = |page: usize, footer: f32| rows.iter().find(|r| r.page == page && r.footer == footer).unwrap();
    assert_eq!((at(0, 0.0).scan, at(0, 0.0).edge), (0.0, None));
    assert!((at(1, 0.0).scan - 541.0).abs() <= 1.0, "{}", at(1, 0.0).scan);
    assert!(at(2, 0.0).scan > 300.0);
    assert!((at(3, 0.0).scan - 99.0).abs() <= 1.0, "{}", at(3, 0.0).scan);
    // Fußzeile 80 blendet das Rechteck in der Fußzeile aus
    assert_eq!((at(4, DEFAULT_FOOTER).scan, at(4, DEFAULT_FOOTER).edge), (0.0, None));
    assert!(at(4, 0.0).scan > 800.0);
}
