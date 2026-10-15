//! Positions-Golden gegen die PDF-AS-Testsuiten `public_pdfbox2/POS_*` (Plan §8.2:
//! „Auto-Position: gleiche Seite, |Δy| ≤ 1 pt“).
//!
//! Erwartete Werte: Rahmen des Signaturblocks in den Referenzbildern (PDF-AS-Ausgabe, 288 dpi),
//! festgehalten in `testdata/placement/pdfas-pos/golden.json`; `manifest_matches_reference_images`
//! prüft das Manifest gegen die Bilder, `pdfas_positions_match_golden` die eigene Berechnung
//! (Profil-`pos` + Request-Position, `resolve_width`, Rasterscan mit PDFium) gegen das Manifest.

mod common;

use common::{BlockPx, block_pt, detect_blocks, load, load_png_luma, read, testdata};
use pida_pdf_cos::Rect;
use pida_placement::{
    CompatMode, FreeSpaceScanner, PositionContext, ResolvedPosition, TablePos, auto_place, resolve_position,
    resolve_width, widget_rect,
};

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    scale: u32,
    cases: Vec<Case>,
    skipped: Vec<Skipped>,
}

#[derive(Debug, serde::Deserialize)]
struct Case {
    suite: String,
    profile: String,
    profile_pos: String,
    request_pos: String,
    #[serde(default)]
    signed_newpage_force: bool,
    /// 1-basierte Seite des Blocks in der Ausgabe.
    page: usize,
    block_px: BlockPx,
}

#[derive(Debug, serde::Deserialize)]
struct Skipped {
    suite: String,
    reason: String,
}

fn manifest() -> Manifest {
    serde_json::from_slice(&read("placement/pdfas-pos/golden.json")).expect("golden.json gültig")
}

struct Input {
    pages: Vec<pida_pdf_cos::PageInfo>,
    signed: bool,
    bytes: Vec<u8>,
}

fn input(case: &Case) -> Input {
    let bytes = read(&format!("placement/pdfas-pos/{}/example.pdf", case.suite));
    let doc = load(&bytes);
    Input { pages: common::pages(&doc), signed: common::already_signed(&doc), bytes }
}

fn position(case: &Case) -> TablePos {
    let request = TablePos::parse(&case.request_pos).expect("Request-Position gültig");
    let profile = TablePos::parse(&case.profile_pos).expect("Profil-Position gültig");
    TablePos::merge(&request, &profile)
}

/// Sichtbare Höhe der Seite, auf der der Block landet (neue Seite = Maße der letzten).
fn target_height(case: &Case, pages: &[pida_pdf_cos::PageInfo]) -> f32 {
    let page = &pages[case.page.min(pages.len()) - 1];
    page.visible_size().1
}

fn context<'a>(case: &Case, inp: &'a Input, compat: CompatMode) -> PositionContext<'a> {
    PositionContext {
        pages: &inp.pages,
        already_signed: inp.signed,
        signed_newpage_force: case.signed_newpage_force,
        less_space_failing: false,
        compat,
    }
}

fn check(case: &Case, inp: &Input, got: &ResolvedPosition, failures: &mut Vec<String>) {
    let (ex, ey, ew, eh) = block_pt(case.block_px, target_height(case, &inp.pages));
    let n = inp.pages.len();
    let mut errs = Vec::new();
    if got.page_index != case.page - 1 {
        errs.push(format!("Seite {} statt {}", got.page_index + 1, case.page));
    }
    if got.new_page != (case.page > n) {
        errs.push(format!("new_page {} statt {}", got.new_page, case.page > n));
    }
    for (name, g, e) in [("x", got.x, ex), ("y", got.y, ey), ("w", got.width, ew)] {
        if (g - e).abs() > 1.0 {
            errs.push(format!("{name} = {g:.2} statt {e:.2}"));
        }
    }
    // Widget-Rect = [x, y−H, x+W, y] gegen die gemessenen Kanten
    let page = &inp.pages[got.page_index.min(n - 1)];
    let r = widget_rect(got, page);
    let expected = Rect::new(ex, ey - eh, ex + ew, ey);
    for (name, g, e) in [
        ("llx", r.llx, expected.llx),
        ("lly", r.lly, expected.lly),
        ("urx", r.urx, expected.urx),
        ("ury", r.ury, expected.ury),
    ] {
        if (g - e).abs() > 1.0 {
            errs.push(format!("Rect.{name} = {g:.2} statt {e:.2}"));
        }
    }
    if !errs.is_empty() {
        failures.push(format!("{} ({}): {}", case.suite, case.profile, errs.join(", ")));
    }
    eprintln!(
        "{:36} Seite {} new={} Δx={:+.2} Δy={:+.2} Δw={:+.2}",
        case.suite,
        got.page_index + 1,
        got.new_page,
        got.x - ex,
        got.y - ey,
        got.width - ew
    );
}

#[test]
fn manifest_is_complete() {
    let m = manifest();
    assert_eq!(m.scale, 4);
    assert_eq!(m.cases.len() + m.skipped.len(), 26, "alle POS_*-Suiten erfasst");
    for s in &m.skipped {
        assert!(!s.reason.is_empty(), "{}", s.suite);
    }
    for c in &m.cases {
        assert!(testdata(&format!("placement/pdfas-pos/{}/example.pdf", c.suite)).is_file(), "{}", c.suite);
        let cfg = String::from_utf8(read(&format!("placement/pdfas-pos/{}/config.properties", c.suite))).unwrap();
        assert!(
            cfg.contains(&format!("profile.id={}", c.profile)),
            "{}: Profil passt nicht zu config.properties",
            c.suite
        );
        let pos_line = cfg.lines().find_map(|l| l.strip_prefix("position.positioning_string="));
        assert_eq!(pos_line.unwrap_or("x:auto;y:auto"), c.request_pos, "{}", c.suite);
        let page_line = cfg.lines().find_map(|l| l.strip_prefix("position.page_number=")).unwrap_or("1");
        assert_eq!(page_line.trim().parse::<usize>().unwrap(), c.page, "{}", c.suite);
    }
}

#[test]
fn manifest_matches_reference_images() {
    let m = manifest();
    let results: Vec<Option<String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = m
            .cases
            .iter()
            .map(|case| {
                scope.spawn(move || {
                    let png = testdata(&format!("pdf/pdfas-suites/{}/example_ref.png", case.suite));
                    if !png.is_file() {
                        return Some(format!("{}: Referenzbild fehlt (übersprungen)", case.suite));
                    }
                    let (w, h, luma) = load_png_luma(&png);
                    let inp = input(case);
                    let ph = target_height(case, &inp.pages);
                    assert_eq!(h, (ph * 4.0) as u32, "{}: Bildhöhe passt nicht zur Seitenhöhe", case.suite);
                    let blocks = detect_blocks(w, h, &luma);
                    assert!(
                        blocks.contains(&case.block_px),
                        "{}: Block {:?} nicht im Referenzbild gefunden, erkannt: {:?}",
                        case.suite,
                        case.block_px,
                        blocks
                    );
                    None
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("Thread")).collect()
    });
    for note in results.into_iter().flatten() {
        eprintln!("{note}");
    }
}

#[test]
fn pdfas_positions_match_golden() {
    let backend = require_pdfium!();
    let m = manifest();
    let mut failures = Vec::new();
    for case in &m.cases {
        let inp = input(case);
        let pos = position(case);
        let renderer = common::open(&backend, &inp.bytes);
        let (_, _, _, eh) = block_pt(case.block_px, target_height(case, &inp.pages));
        for compat in [CompatMode::Corrected, CompatMode::PdfasQuirks] {
            let ctx = context(case, &inp, compat);
            let width = resolve_width(&pos, &ctx).unwrap();
            let got = auto_place(renderer.as_ref(), &pos, &ctx, width, eh).unwrap();
            check(case, &inp, &got, &mut failures);
        }
    }
    assert!(failures.is_empty(), "Abweichungen von PDF-AS:\n{}", failures.join("\n"));
}

/// Scanner, der nicht benutzt werden darf (Fälle ohne Freiraumsuche).
struct NoScan;

impl FreeSpaceScanner for NoScan {
    fn lowest_content(&mut self, page_index: usize, _footer: f32) -> pida_placement::Result<f32> {
        panic!("Freiraumsuche auf Seite {page_index} unerwartet");
    }
}

#[test]
fn golden_cases_without_scan_need_no_renderer() {
    let m = manifest();
    let mut failures = Vec::new();
    let mut checked = 0;
    for case in m.cases.iter().filter(|c| c.request_pos.contains("p:last") || c.request_pos == "p:99") {
        let inp = input(case);
        let pos = position(case);
        let (_, _, _, eh) = block_pt(case.block_px, target_height(case, &inp.pages));
        let ctx = context(case, &inp, CompatMode::Corrected);
        let width = resolve_width(&pos, &ctx).unwrap();
        let got = resolve_position(&pos, &ctx, width, eh, &mut NoScan).unwrap();
        check(case, &inp, &got, &mut failures);
        checked += 1;
    }
    assert_eq!(checked, 4);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
