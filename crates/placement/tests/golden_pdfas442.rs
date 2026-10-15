//! Positions-Golden gegen einen frischen PDF-AS-4.4.2-Lauf (Plan §8.2; `tools/golden/run-pdfas.sh`,
//! Referenzen unter `testdata/golden/pdfas-4.4.2/`, Bericht `docs/integration/golden-report.md`).
//!
//! Je Fall (12 Profil-IDs × Eingaben × Positionen) wird die PDF-AS-Positionierung aus dem
//! Manifest (DEBUG-Log: `PositioningInstruction`, feste Tabellenbreite; Widget-`/Rect`, Seite,
//! Holder-`/Matrix`) mit `resolve_width`/`resolve_position`/`widget_rect_with` nachgerechnet:
//!
//! - manuelle Position, `p:new`, `p:last`, Platzhalter: Breite, x, y und Widget-Rect ±0,01 pt,
//! - Auto-Position (Rasterscan mit PDFium, nur mit `PDFOVER_PDFIUM_DIR`): gleiche Seite, ±1 pt,
//! - neue Seite (Seitenindex, MediaBox und `/Rotate` der neuen Seite), Rotation der Erscheinung.
//!
//! Gerechnet wird in beiden Modi: [`CompatMode::PdfasQuirks`] muss PDF-AS exakt treffen,
//! [`CompatMode::Corrected`] darf nur in den dokumentierten Fällen des Kompatibilitätsregisters
//! abweichen ([`known_deviation`]: KR-F01 AMTS-Breite, KR-F02 CropBox-Ursprung und neue Seite mit
//! MediaBox, KR-F04 Rasterscan gedrehter Seiten). Seiten, Felder und Bildaufrufe liest
//! `pdfover-pdf-cos` (Gegentest zum lopdf-Mini-COS der übrigen Placement-Tests).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdfover_pdf_cos::{Limits, PageInfo, Rect, SourcePdf};
use pdfover_placement::{
    CompatMode, FreeSpaceScanner, PlaceholderMode, PlaceholderQuery, PositionContext, RasterScanner, ResolvedPosition,
    TablePos, appearance_rotation, find_qr_placeholders, new_page_geometry, resolve_position, resolve_width,
    select_placeholder, widget_rect_with,
};
use pdfover_render::{ForcedColorOptions, RenderBackend, Renderer};
use serde::Deserialize;

/// Toleranz manuelle Position (Auftrag: ±0,01 pt) bzw. Auto-Position (Plan §8.2: ±1 pt).
const TOL_EXACT: f32 = 0.01;
const TOL_AUTO: f32 = 1.0;

fn golden(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden/pdfas-4.4.2").join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    let p = golden(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

// ------------------------------------------------------------------------------------------------
// Manifest (Auszug)
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Index {
    cases: Vec<IndexCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct IndexCase {
    id: String,
    profile: String,
    input: String,
    position_key: String,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    case: CaseInfo,
    file: FileInfo,
    page: Option<PageGeom>,
    widget: Widget,
    holder: Option<Holder>,
    inner: Option<Inner>,
    pdfas_log: Option<PdfasLog>,
}

#[derive(Debug, Clone, Deserialize)]
struct CaseInfo {
    id: String,
    profile: String,
    input: String,
    position_key: String,
    position: Option<String>,
    #[serde(default)]
    placeholder: bool,
    placeholder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileInfo {
    pages: usize,
    input_pages: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct PageGeom {
    index: usize,
    media_box: Vec<f32>,
    rotate: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct Widget {
    rect: Vec<f32>,
    page_index: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct Holder {
    matrix: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Deserialize)]
struct Inner {
    bbox: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct PdfasLog {
    positioning: Option<Positioning>,
    fixed_width: Option<f32>,
    signature_positioning: Option<String>,
    request_positioning: Option<String>,
    placeholder: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Positioning {
    page: usize,
    make_new_page: bool,
    x: f32,
    y: f32,
    r: f32,
}

fn index() -> Index {
    serde_json::from_slice(&read("index.json")).expect("index.json gültig")
}

fn manifest(id: &str) -> Manifest {
    serde_json::from_slice(&read(&format!("cases/{id}/manifest.json"))).unwrap_or_else(|e| panic!("{id}: {e}"))
}

// ------------------------------------------------------------------------------------------------
// Eingaben über pdf-cos
// ------------------------------------------------------------------------------------------------

struct Input {
    bytes: Arc<[u8]>,
    pdf: SourcePdf,
}

impl Input {
    fn load(name: &str) -> Input {
        let bytes: Arc<[u8]> = Arc::from(read(&format!("inputs/{name}.pdf")));
        let pdf = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        Input { bytes, pdf }
    }

    fn pages(&self) -> &[PageInfo] {
        self.pdf.pages()
    }

    fn already_signed(&self) -> bool {
        self.pdf.signature_fields().expect("Felder").iter().any(|f| f.signed)
    }
}

/// Freiraumsuche: PDFium-Rasterscan, falls verfügbar; merkt sich, ob ein Scan nötig war.
struct Probe<'a> {
    inner: Option<RasterScanner<'a, dyn Renderer>>,
    scanned: bool,
}

impl FreeSpaceScanner for Probe<'_> {
    fn lowest_content(&mut self, page_index: usize, footer: f32) -> pdfover_placement::Result<f32> {
        self.scanned = true;
        match &mut self.inner {
            Some(s) => s.lowest_content(page_index, footer),
            None => Ok(0.0),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Erwartung aus dem Manifest
// ------------------------------------------------------------------------------------------------

/// Drehung der Holder-`/Matrix` in Grad.
fn rotation_of(m: Option<&Vec<f32>>) -> i32 {
    let Some(m) = m else { return 0 };
    match (m[0].round() as i32, m[1].round() as i32) {
        (0, 1) => 90,
        (-1, 0) => 180,
        (0, -1) => 270,
        _ => 0,
    }
}

fn normalized(r: &[f32]) -> Rect {
    Rect::new(r[0], r[1], r[2], r[3]).normalized()
}

/// Bewusste Abweichungen des korrigierten Modus (Kompatibilitätsregister in der Crate-Doku).
/// Im Modus `PdfasQuirks` gibt es nur KR-F03 (die Platzhalterauswahl hat keinen Quirk-Modus).
fn known_deviation(m: &Manifest, compat: CompatMode, pos: &TablePos) -> Option<&'static str> {
    if m.case.input == "qr-id200" {
        return Some("KR-F03 Platzhalter-IDs > 127 werden gefunden (PDF-AS: Integer-==)");
    }
    if compat == CompatMode::PdfasQuirks {
        return None;
    }
    let profile_has_w = m
        .pdfas_log
        .as_ref()
        .and_then(|l| l.signature_positioning.as_deref())
        .is_some_and(|p| p.split(';').any(|kv| kv.starts_with("w:")));
    let manual_x = matches!(pos.x, Some(pdfover_placement::Coord::Value(_)));
    if !profile_has_w && manual_x && !matches!(pos.w, Some(pdfover_placement::Coord::Value(_))) {
        return Some("KR-F01 AMTS-Breite unabhängig vom manuellen x");
    }
    if m.case.input == "a4-cropbox" {
        return Some("KR-F02 Rect relativ zur CropBox, neue Seite mit MediaBox-Geometrie");
    }
    if matches!(m.case.input.as_str(), "a4-rot90" | "a4-rot270")
        && matches!(m.case.position_key.as_str(), "auto" | "none" | "footer0" | "footer300")
    {
        return Some("KR-F04 Rasterscan im gedrehten (sichtbaren) Raum");
    }
    None
}

#[derive(Debug, Default, Clone, serde::Serialize)]
struct Outcome {
    id: String,
    compat: String,
    scanned: bool,
    skipped: Option<String>,
    dx: f32,
    dy: f32,
    dw: f32,
    drect: f32,
    errors: Vec<String>,
    documented: Option<String>,
    /// eigenes Ergebnis (für die Prüfung dokumentierter Abweichungen)
    width: f32,
    page_index: usize,
    new_page: bool,
    y: f32,
    rect: [f32; 4],
}

fn position_of(m: &Manifest, inp: &Input, backend: Option<&pdfover_render::PdfiumBackend>) -> Result<TablePos, String> {
    let log = m.pdfas_log.as_ref().ok_or("Log fehlt")?;
    if m.case.placeholder {
        // PDF-Over: Platzhaltersuche mit id 1, Modus MODERATE; die Platzhalter-Position ersetzt
        // die Position vollständig (minWidth gibt es in den PDF-Over-Profilen nicht).
        let backend = backend.ok_or("QR-Platzhalter braucht PDFium")?;
        let renderer = backend.open(inp.bytes.clone(), None).map_err(|e| e.to_string())?;
        let invocations: Vec<_> =
            (0..inp.pages().len()).flat_map(|i| inp.pdf.image_invocations(i, false).expect("Bildaufrufe")).collect();
        let found = find_qr_placeholders(renderer.as_ref(), inp.pages(), &invocations).map_err(|e| e.to_string())?;
        let id = m.case.placeholder_id.clone().unwrap_or_else(|| "1".into());
        let query = PlaceholderQuery { id: Some(id), mode: PlaceholderMode::Moderate, used_names: Vec::new() };
        let p = select_placeholder(&found, &query).map_err(|e| e.to_string())?.ok_or("kein Platzhalter")?;
        let pos = p.table_pos_with_min_width(None);
        if let Some(expected) = &log.placeholder {
            let e = TablePos::parse(expected).map_err(|e| e.to_string())?;
            if e != pos {
                return Err(format!("Platzhalter-Position {pos} statt {expected}"));
            }
        }
        return Ok(pos);
    }
    let request = match log.request_positioning.as_deref().or(m.case.position.as_deref()) {
        Some(r) => TablePos::parse(r).map_err(|e| e.to_string())?,
        None => TablePos::default(),
    };
    let profile =
        TablePos::parse(log.signature_positioning.as_deref().ok_or("Profil-pos fehlt")?).map_err(|e| e.to_string())?;
    Ok(TablePos::merge(&request, &profile))
}

fn check(m: &Manifest, inp: &Input, compat: CompatMode, backend: Option<&pdfover_render::PdfiumBackend>) -> Outcome {
    let mut o = Outcome { id: m.case.id.clone(), compat: format!("{compat:?}"), ..Default::default() };
    let log = m.pdfas_log.as_ref().expect("Log");
    let Some(expected) = &log.positioning else {
        o.skipped = Some("keine PositioningInstruction".into());
        return o;
    };
    let pos = match position_of(m, inp, backend) {
        Ok(p) => p,
        Err(e) if e.contains("braucht PDFium") => {
            o.skipped = Some(e);
            return o;
        }
        Err(e) => {
            o.errors.push(e);
            return o;
        }
    };
    o.documented = known_deviation(m, compat, &pos).map(str::to_string);
    let ctx = PositionContext {
        pages: inp.pages(),
        already_signed: inp.already_signed(),
        signed_newpage_force: false,
        less_space_failing: false,
        compat,
    };
    let width = match resolve_width(&pos, &ctx) {
        Ok(w) => w,
        Err(e) => {
            o.errors.push(format!("resolve_width: {e}"));
            return o;
        }
    };
    let fixed = log.fixed_width.expect("feste Breite");
    o.width = width.width;
    o.dw = width.width - fixed;
    if (width.width - fixed).abs() > TOL_EXACT {
        o.errors.push(format!("Breite {} statt {fixed}", width.width));
    }
    // Blockhöhe aus der PDF-AS-Ausgabe (BBox = H + 1)
    let h = m.inner.as_ref().map(|i| i.bbox[1].abs() - 1.0).expect("Inner-BBox");
    let renderer = backend.map(|b| b.open(inp.bytes.clone(), None).expect("PDFium öffnet Eingabe"));
    let mut probe = Probe {
        inner: renderer.as_deref().map(|r| RasterScanner { renderer: r, options: ForcedColorOptions::default() }),
        scanned: false,
    };
    let got: ResolvedPosition = match resolve_position(&pos, &ctx, width, h, &mut probe) {
        Ok(g) => g,
        Err(e) => {
            o.errors.push(format!("resolve_position: {e}"));
            return o;
        }
    };
    o.scanned = probe.scanned;
    (o.page_index, o.new_page, o.y) = (got.page_index, got.new_page, got.y);
    if probe.scanned && probe.inner.is_none() {
        o.skipped = Some("Rasterscan braucht PDFium".into());
        o.errors.clear();
        return o;
    }
    let tol = if probe.scanned { TOL_AUTO } else { TOL_EXACT };
    // Seite (PDF-AS: 1-basiert, bei neuer Seite = Seitenzahl + 1)
    if got.page_index + 1 != expected.page || got.new_page != expected.make_new_page {
        o.errors.push(format!(
            "Seite {}{} statt {}{}",
            got.page_index + 1,
            if got.new_page { " (neu)" } else { "" },
            expected.page,
            if expected.make_new_page { " (neu)" } else { "" }
        ));
    }
    o.dx = got.x - expected.x;
    o.dy = got.y - expected.y;
    if o.dx.abs() > tol || o.dy.abs() > tol {
        o.errors.push(format!("Position ({}, {}) statt ({}, {})", got.x, got.y, expected.x, expected.y));
    }
    if got.rotation != expected.r as i32 {
        o.errors.push(format!("Blockrotation {} statt {}", got.rotation, expected.r));
    }
    // Widget-Rect auf der Zielseite (neue Seite: Geometrie der letzten vorhandenen)
    let n = inp.pages().len();
    let page = &inp.pages()[got.page_index.min(n - 1)];
    let r = widget_rect_with(&got, page, compat);
    o.rect = [r.llx, r.lly, r.urx, r.ury];
    let e = normalized(&m.widget.rect);
    o.drect =
        [(r.llx - e.llx), (r.lly - e.lly), (r.urx - e.urx), (r.ury - e.ury)].iter().fold(0f32, |a, d| a.max(d.abs()));
    if o.drect > tol {
        o.errors.push(format!("Rect {:?} statt {:?}", [r.llx, r.lly, r.urx, r.ury], [e.llx, e.lly, e.urx, e.ury]));
    }
    // Seite des Widgets, neue Seite: MediaBox und /Rotate wie die letzte Seite
    if m.widget.page_index != Some(got.page_index) {
        o.errors.push(format!("Widget-Seite {:?} statt {}", m.widget.page_index, got.page_index));
    }
    if got.new_page {
        let g = new_page_geometry(&inp.pages()[n - 1]);
        let mp = m.page.as_ref().expect("Zielseite");
        let mb = g.media_box.normalized();
        let em = normalized(&mp.media_box);
        if mp.index != n || (mb.width() - em.width()).abs() > TOL_EXACT || (mb.height() - em.height()).abs() > TOL_EXACT
        {
            o.errors.push(format!("neue Seite {mp:?} statt MediaBox {mb:?}"));
        }
        if i32::from(g.rotation.degrees()) != mp.rotate {
            o.errors.push(format!("neue Seite /Rotate {} statt {}", g.rotation.degrees(), mp.rotate));
        }
        if m.file.pages != n + 1 {
            o.errors.push(format!("Seitenzahl {} statt {}", m.file.pages, n + 1));
        }
    } else if m.file.input_pages.is_some_and(|ip| ip != m.file.pages) {
        o.errors.push("PDF-AS hat eine Seite angehängt".into());
    }
    // Rotation der Erscheinung (Holder-/Matrix = Blockrotation + Seitenrotation)
    let rot = appearance_rotation(got.rotation, page);
    let erot = rotation_of(m.holder.as_ref().and_then(|h| h.matrix.as_ref()));
    if rot != erot {
        o.errors.push(format!("Erscheinungsrotation {rot} statt {erot}"));
    }
    o
}

fn write_report(name: &str, value: &impl serde::Serialize) {
    if let Some(dir) = std::env::var_os("PDFOVER_GOLDEN_REPORT_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
}

fn run(compat: CompatMode) -> Vec<Outcome> {
    let backend = common::backend();
    if backend.is_none() {
        eprintln!("PDFOVER_PDFIUM_DIR nicht gesetzt – Auto-Positionen und QR-Platzhalter übersprungen");
    }
    let cases: Vec<IndexCase> =
        index().cases.into_iter().filter(|c| c.status == "ok" && c.profile != "INVISIBLE").collect();
    let mut inputs: BTreeMap<String, Input> = BTreeMap::new();
    let mut out = Vec::new();
    for c in &cases {
        let inp = inputs.entry(c.input.clone()).or_insert_with(|| Input::load(&c.input));
        out.push(check(&manifest(&c.id), inp, compat, backend.as_ref()));
    }
    out
}

fn summarize(outcomes: &[Outcome]) -> String {
    let mut by_input: BTreeMap<String, (usize, usize, usize, usize, f32, f32)> = BTreeMap::new();
    for o in outcomes {
        let input = o.id.split("__").nth(1).unwrap_or("").to_string();
        let e = by_input.entry(input).or_default();
        e.0 += 1;
        if o.skipped.is_some() {
            e.3 += 1;
        } else if o.errors.is_empty() {
            e.1 += 1;
        } else if o.documented.is_some() {
            e.2 += 1;
        }
        if o.skipped.is_none() && o.errors.is_empty() {
            e.4 = e.4.max(o.dy.abs().max(o.dx.abs()));
            e.5 = e.5.max(o.drect);
        }
    }
    let mut s = String::from(
        "Eingabe | Fälle | gleich | dokumentiert abweichend | übersprungen | max. |Δx|,|Δy| | max. ΔRect\n",
    );
    for (k, (n, same, doc, skip, d, r)) in by_input {
        s.push_str(&format!("{k} | {n} | {same} | {doc} | {skip} | {d:.3} | {r:.3}\n"));
    }
    s
}

/// Die dokumentierten Abweichungen haben genau die erwartete (korrigierte) Form.
fn verify_documented(outcomes: &[Outcome]) {
    let mut wrong = Vec::new();
    for o in outcomes.iter().filter(|o| o.documented.is_some() && o.skipped.is_none()) {
        let m = manifest(&o.id);
        let inp = Input::load(&m.case.input);
        let last = &inp.pages()[inp.pages().len() - 1];
        let doc = o.documented.as_deref().unwrap();
        let ok = if doc.starts_with("KR-F03") {
            // Platzhalter id=200 gefunden: Position und Breite aus dem Platzhalter
            let e = TablePos::parse(m.pdfas_log.as_ref().and_then(|l| l.placeholder.as_deref()).unwrap()).unwrap();
            let val = |c: Option<pdfover_placement::Coord>| c.and_then(|c| c.value()).unwrap();
            (o.width - val(e.w)).abs() <= TOL_EXACT && (o.y - val(e.y)).abs() <= TOL_EXACT && o.page_index == 0
        } else if doc.starts_with("KR-F01") {
            // Breite wie bei automatischer Position: sichtbare Seitenbreite − 2 · 50
            let pw = if o.new_page {
                new_page_geometry(last).visible_size().0
            } else {
                inp.pages()[o.page_index].visible_size().0
            };
            (o.width - (pw - 100.0)).abs() <= TOL_EXACT
        } else if doc.starts_with("KR-F02") {
            let crop = inp.pages()[0].crop_box.normalized();
            if o.new_page {
                // neue Seite mit MediaBox-Geometrie: Oberkante MediaBox-Höhe − 20
                let mb = last.media_box.normalized();
                (o.y - (mb.height() - 20.0)).abs() <= TOL_EXACT
            } else {
                // Rect wie PDF-AS, verschoben um den CropBox-Ursprung
                let e = normalized(&m.widget.rect);
                let tol = if o.scanned { TOL_AUTO } else { TOL_EXACT };
                [
                    (o.rect[0], e.llx + crop.llx),
                    (o.rect[1], e.lly + crop.lly),
                    (o.rect[2], e.urx + crop.llx),
                    (o.rect[3], e.ury + crop.lly),
                ]
                .iter()
                .all(|(a, b)| (a - b).abs() <= tol)
            }
        } else {
            // KR-F04: Freiraum im sichtbaren Raum gefunden, keine neue Seite
            !o.new_page && o.page_index == 0
        };
        if !ok {
            wrong.push(format!(
                "{} [{doc}]: Breite {}, Seite {} neu={}, y {}, Rect {:?}",
                o.id, o.width, o.page_index, o.new_page, o.y, o.rect
            ));
        }
    }
    assert!(wrong.is_empty(), "dokumentierte Abweichungen mit unerwarteter Form:\n{}", wrong.join("\n"));
}

/// `CompatMode::PdfasQuirks` rechnet wie PDF-AS 4.4.2: Breite, Seite, Position und Rect exakt
/// (Auto-Position ±1 pt).
#[test]
fn pdfas_quirks_mode_matches_pdfas_442() {
    let outcomes = run(CompatMode::PdfasQuirks);
    println!("{}", summarize(&outcomes));
    write_report("placement-quirks.json", &outcomes);
    let failures: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.errors.is_empty() && o.documented.is_none())
        .map(|o| format!("{}: {}", o.id, o.errors.join("; ")))
        .collect();
    assert!(failures.is_empty(), "{} Abweichungen:\n{}", failures.len(), failures.join("\n"));
    verify_documented(&outcomes);
}

/// `CompatMode::Corrected` (Standard) weicht nur in den Fällen des Kompatibilitätsregisters ab.
#[test]
fn corrected_mode_deviates_only_where_documented() {
    let outcomes = run(CompatMode::Corrected);
    println!("{}", summarize(&outcomes));
    write_report("placement-corrected.json", &outcomes);
    let failures: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.errors.is_empty() && o.documented.is_none())
        .map(|o| format!("{}: {}", o.id, o.errors.join("; ")))
        .collect();
    let documented: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.errors.is_empty() && o.documented.is_some())
        .map(|o| format!("{} [{}]: {}", o.id, o.documented.as_deref().unwrap_or(""), o.errors.join("; ")))
        .collect();
    println!("dokumentierte Abweichungen ({}):\n{}", documented.len(), documented.join("\n"));
    assert!(failures.is_empty(), "{} undokumentierte Abweichungen:\n{}", failures.len(), failures.join("\n"));
    verify_documented(&outcomes);
}

/// Unsichtbare Signatur: Widget-Rect [0 0 0 0] auf der letzten bzw. gewählten Seite, keine neue Seite.
#[test]
fn invisible_profile_has_empty_rect() {
    for c in index().cases.iter().filter(|c| c.profile == "INVISIBLE" && c.status == "ok") {
        let m = manifest(&c.id);
        assert_eq!(normalized(&m.widget.rect), Rect::new(0.0, 0.0, 0.0, 0.0), "{}", c.id);
        assert_eq!(m.file.input_pages, Some(m.file.pages), "{}: keine neue Seite", c.id);
        assert!(m.pdfas_log.as_ref().is_none_or(|l| l.positioning.is_none()), "{}", c.id);
        let _ = (&c.input, &c.position_key);
    }
}
