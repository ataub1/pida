//! Seitenlayout (ADR 0004 §3.3) mit echtem PDFium: Belegungsraster aus dem Forced-Color-Rendering,
//! Inhaltsrahmen, freie Flächen und Überdeckung auf Korpusseiten (`testdata/pdf/**`,
//! `testdata/placement/**`) sowie auf erzeugten Seiten mit `/Rotate` und CropBox ≠ MediaBox. Misst die
//! Laufzeit (Raster je Seite, Layout, Prüfung). Ohne `PIDA_PDFIUM_DIR` übersprungen (ADR 0001 Regel 6).

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::*;
use pida_pdf_cos::Rect;
use pida_placement::{BlockGeometry, LayoutOptions, OccupancyGrid, SCAN_TOLERANCE, page_layout, user_rect_to_visible};
use pida_render::{ForcedColorOptions, Renderer};

const BLOCK: BlockGeometry = BlockGeometry { width: 230.0, height: 49.0 };

fn pdfs_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            pdfs_under(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            out.push(p);
        }
    }
}

/// Raster einer Seite wie im Kern (72 dpi, Toleranz des Rasterscans) und die Zeit dafür.
fn grid_of(r: &dyn Renderer, page: usize) -> Option<(OccupancyGrid, Duration)> {
    let t = Instant::now();
    let size = r.page_size(page).ok()?;
    let opts = ForcedColorOptions::default();
    let img = r.render_forced_color(page, &opts).ok()?;
    let grid =
        OccupancyGrid::from_forced_color(&img, size.width, size.height, opts.dpi, opts.background, SCAN_TOLERANCE);
    Some((grid, t.elapsed()))
}

fn within_page(r: &Rect, w: f32, h: f32) -> bool {
    r.llx >= -0.01 && r.lly >= -0.01 && r.urx <= w + 0.01 && r.ury <= h + 0.01 && r.llx < r.urx && r.lly < r.ury
}

/// Invarianten auf allen Korpusseiten (höchstens drei je Dokument): Inhalt und freie Flächen liegen auf
/// der Seite, freie Flächen sind unbelegt, halten 6 pt Abstand zum Inhalt und fassen den Block; der
/// Rahmen enthält jede belegte Zelle. Die Laufzeiten stehen im Testprotokoll (`--nocapture`).
#[test]
fn layout_on_corpus_pages() {
    let backend = require_pdfium!();
    let mut files = Vec::new();
    pdfs_under(&testdata("pdf"), &mut files);
    pdfs_under(&testdata("placement"), &mut files);
    assert!(files.len() > 20, "Korpus fehlt: {}", files.len());
    let (mut pages, mut raster, mut layout_max, mut check_max) = (0usize, Vec::new(), Duration::ZERO, Duration::ZERO);
    for file in &files {
        let Ok(bytes) = std::fs::read(file) else { continue };
        // verschlüsselte bzw. kaputte Dateien des Korpus: nicht Gegenstand dieses Tests
        let Ok(r) = pida_render::RenderBackend::open(&backend, std::sync::Arc::from(bytes.as_slice()), None) else {
            continue;
        };
        for page in 0..r.page_count().min(3) {
            let Some((grid, t_raster)) = grid_of(r.as_ref(), page) else { continue };
            pages += 1;
            raster.push(t_raster);
            let (w, h) = grid.page_size();
            let t = Instant::now();
            let l = page_layout(&grid, page, BLOCK, &LayoutOptions::default());
            layout_max = layout_max.max(t.elapsed());
            let name = file.strip_prefix(testdata("")).unwrap_or(file).display().to_string();
            if let Some(c) = l.content {
                assert!(within_page(&c, w, h), "{name} S. {page}: Inhalt {c:?}");
                // jede belegte Zelle liegt im Rahmen
                let (cols, rows) = grid.dimensions();
                for row in (0..rows).step_by(7) {
                    for col in (0..cols).step_by(7) {
                        if grid.get(col, row) {
                            let x = col as f32 * 3.0 + 1.5;
                            let y = h - row as f32 * 3.0 - 1.5;
                            assert!(x >= c.llx && x <= c.urx && y >= c.lly && y <= c.ury, "{name} S. {page}");
                        }
                    }
                }
            } else {
                assert_eq!(grid.count_occupied(), 0, "{name} S. {page}");
            }
            assert!(l.free.len() <= 16, "{name}");
            for f in &l.free {
                assert!(within_page(f, w, h), "{name} S. {page}: frei {f:?}");
                assert!(f.width() + 0.01 >= BLOCK.width && f.height() + 0.01 >= BLOCK.height, "{name}: {f:?}");
                let t = Instant::now();
                let occupied = grid.occupied(*f);
                check_max = check_max.max(t.elapsed());
                assert!(!occupied, "{name} S. {page}: freie Fläche belegt {f:?}");
                // 6 pt Abstand: um knapp 6 pt vergrößert immer noch unbelegt (bis auf den Seitenrand)
                let grown = Rect { llx: f.llx - 5.5, lly: f.lly - 5.5, urx: f.urx + 5.5, ury: f.ury + 5.5 };
                assert!(!grid.occupied(grown), "{name} S. {page}: Abstand zum Inhalt {f:?}");
            }
        }
    }
    assert!(pages > 20, "zu wenige Seiten geprüft: {pages}");
    raster.sort();
    eprintln!(
        "Seitenlayout auf {pages} Korpusseiten: Raster Median {:?}, Maximum {:?}; Layout max. {layout_max:?}; \
         Prüfung max. {check_max:?} (Debug-Build: Zeitziele siehe `layout_time_budget`)",
        raster[raster.len() / 2],
        raster[raster.len() - 1]
    );
}

/// Erzeugte Seiten mit gefülltem Rechteck an bekannter Stelle: gedreht (0/90/180/270°) und mit
/// CropBox ≠ MediaBox. Der Inhaltsrahmen aus dem Raster liegt dort, wo die Umrechnung
/// Benutzerraum → sichtbare Seite (`user_rect_to_visible`) das Rechteck erwartet (± eine Zelle).
#[test]
fn layout_on_rotated_and_cropped_pages_matches_the_content() {
    let backend = require_pdfium!();
    let content = "0 0 0 rg 100 600 200 100 re f";
    let user = Rect { llx: 100.0, lly: 600.0, urx: 300.0, ury: 700.0 };
    for (rotate, crop) in [
        (0, None),
        (90, None),
        (180, None),
        (270, None),
        (0, Some([20.0, 30.0, 575.0, 812.0])),
        (90, Some([20.0, 30.0, 575.0, 812.0])),
    ] {
        let mut spec = PageSpec::a4(content);
        spec.rotate = rotate;
        spec.crop_box = crop;
        let bytes = build_doc(PdfBuilder::new(), vec![spec], "");
        let pages = common::pages(&load(&bytes));
        let expected = user_rect_to_visible(&pages[0], user);
        let r = open(&backend, &bytes);
        let (grid, _) = grid_of(r.as_ref(), 0).expect("Raster");
        let (w, h) = grid.page_size();
        let (vw, vh) = pages[0].visible_size();
        assert!((w - vw).abs() < 0.01 && (h - vh).abs() < 0.01, "/Rotate {rotate} {crop:?}: {w}×{h}");
        let l = page_layout(&grid, 0, BLOCK, &LayoutOptions::default());
        let c = l.content.unwrap_or_else(|| panic!("/Rotate {rotate} {crop:?}: kein Inhalt"));
        for (got, want) in [(c.llx, expected.llx), (c.lly, expected.lly), (c.urx, expected.urx), (c.ury, expected.ury)]
        {
            assert!((got - want).abs() <= 3.5, "/Rotate {rotate} {crop:?}: {c:?} statt {expected:?}");
        }
        assert!(grid.occupied(expected));
        assert!(!l.free.is_empty(), "/Rotate {rotate} {crop:?}");
    }
}

/// Zeitbudget ADR 0004 §3.3 (Release-Build, Referenzrechner): Raster ≤ 150 ms je A4-Textseite, Prüfung
/// ≤ 2 ms. Als ignorierter Benchmark: `cargo test --release -p pida-placement --test layout_corpus --
/// --ignored --nocapture`.
#[test]
#[ignore = "Benchmark (Release-Build, Referenzrechner)"]
fn layout_time_budget() {
    let backend = require_pdfium!();
    let bytes = read("pdf/pdfas/pdf-as-legacy/simple.pdf");
    let r = open(&backend, &bytes);
    let mut worst = Duration::ZERO;
    let mut check = Duration::ZERO;
    for _ in 0..10 {
        let (grid, t) = grid_of(r.as_ref(), 0).expect("Raster");
        worst = worst.max(t);
        let t = Instant::now();
        let l = page_layout(&grid, 0, BLOCK, &LayoutOptions::default());
        let _ = grid.occupied(Rect { llx: 300.0, lly: 100.0, urx: 530.0, ury: 149.0 });
        check = check.max(t.elapsed());
        assert!(l.content.is_some());
    }
    eprintln!("Raster max. {worst:?}, Layout + Prüfung max. {check:?}");
    assert!(worst <= Duration::from_millis(150), "Raster {worst:?}");
    assert!(check <= Duration::from_millis(2), "Prüfung {check:?}");
}
