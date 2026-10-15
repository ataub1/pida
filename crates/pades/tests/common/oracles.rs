//! Orakel für eine signierte Ausgabe: qpdf, openssl, PDFium-Pixel, pyHanko-Bewertung,
//! Ergebnisdatei.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use pdfover_pades::SignedPdf;
use pdfover_render::{PdfiumBackend, RenderBackend};

use super::{PyhankoSig, modification_rank, openssl_verify, pdfium, pyhanko_python, qpdf_check, testdata, which};

/// Verfügbare externe Werkzeuge.
pub struct Oracles {
    pub qpdf: Option<PathBuf>,
    pub openssl: Option<PathBuf>,
    pub python: Option<PathBuf>,
    pub pdfium: Option<PdfiumBackend>,
}

impl Oracles {
    pub fn detect() -> Oracles {
        let o = Oracles { qpdf: which("qpdf"), openssl: which("openssl"), python: pyhanko_python(), pdfium: pdfium() };
        for (n, ok) in [
            ("qpdf", o.qpdf.is_some()),
            ("openssl", o.openssl.is_some()),
            ("pyHanko", o.python.is_some()),
            ("PDFium", o.pdfium.is_some()),
        ] {
            if !ok {
                eprintln!("HINWEIS: {n} nicht verfügbar – Prüfung übersprungen");
            }
        }
        o
    }

    pub fn renderer(&self) -> Option<&dyn RenderBackend> {
        self.pdfium.as_ref().map(|b| b as &dyn RenderBackend)
    }

    /// Werkzeugversionen für RESULTS.md.
    pub fn versions(&self) -> String {
        let first_line = |cmd: &Path, args: &[&str]| {
            Command::new(cmd)
                .args(args)
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string())
                .unwrap_or_default()
        };
        let mut v = Vec::new();
        if let Some(o) = &self.openssl {
            v.push(first_line(o, &["version"]));
        }
        if let Some(q) = &self.qpdf {
            v.push(first_line(q, &["--version"]));
        }
        if let Some(p) = &self.python {
            v.push(format!(
                "pyHanko {}",
                first_line(p, &["-I", "-c", "import importlib.metadata as m; print(m.version('pyHanko'))"])
            ));
        }
        if let Some(b) = &self.pdfium {
            // ohne lokalen Bibliothekspfad
            let d = b.describe();
            v.push(d.split(" (").next().unwrap_or(&d).to_string());
        }
        v.join("; ")
    }
}

/// Erwartetes Bild.
#[derive(Debug, Clone, Copy)]
pub enum Look {
    /// Block im sichtbaren Rechteck (PAGE_ABS, pt) der Seite `page`; `new_page` = Seite neu.
    Visible { page: usize, rect: pdfover_pdf_cos::Rect, new_page: bool },
    /// keine sichtbare Änderung auf `page`
    Invisible { page: usize },
}

fn dark(p: &[u8]) -> bool {
    p[3] > 0 && p[0].min(p[1]).min(p[2]) < 128
}

/// PDFium-Render (Maßstab 2) vorher/nachher: ein sichtbarer Block erzeugt Pixel im Rechteck und
/// ändert außerhalb (2 pt Rand) nichts; eine unsichtbare Signatur ändert nichts.
pub fn pixel_check(backend: &PdfiumBackend, before: &[u8], after: &[u8], look: Look) -> Result<String, String> {
    const S: f32 = 2.0;
    let a = backend.open(Arc::from(after.to_vec()), None).map_err(|e| e.to_string())?;
    let b = backend.open(Arc::from(before.to_vec()), None).map_err(|e| e.to_string())?;
    let page = match look {
        Look::Visible { page, .. } | Look::Invisible { page } => page,
    };
    let img_a = a.render_page(page, S).map_err(|e| e.to_string())?;
    let img_b = if page < b.page_count() { Some(b.render_page(page, S).map_err(|e| e.to_string())?) } else { None };
    if let Some(ib) = &img_b
        && (ib.width != img_a.width || ib.height != img_a.height)
    {
        return Err("Seitengröße geändert".into());
    }
    let h = a.page_size(page).map_err(|e| e.to_string())?.height;
    match look {
        Look::Invisible { .. } => {
            let ib = img_b.ok_or("Seite fehlt im Original")?;
            let changed = img_a.data.chunks(4).zip(ib.data.chunks(4)).filter(|(p, q)| p != q).count();
            if changed > 0 {
                return Err(format!("unsichtbare Signatur ändert {changed} Pixel"));
            }
            Ok("unverändert".into())
        }
        Look::Visible { rect, new_page, .. } => {
            let r = rect.normalized();
            let (x0, x1) = ((r.llx * S).floor() as i64, (r.urx * S).ceil() as i64);
            let (y0, y1) = (((h - r.ury) * S).floor() as i64, ((h - r.lly) * S).ceil() as i64);
            let m = (2.0 * S) as i64;
            let (mut ink, mut inside_changed, mut outside_changed, mut area) = (0usize, 0usize, 0usize, 0usize);
            let w = img_a.width as i64;
            for y in 0..img_a.height as i64 {
                for x in 0..w {
                    let i = ((y * w + x) * 4) as usize;
                    let pa = &img_a.data[i..i + 4];
                    let changed = img_b.as_ref().is_some_and(|ib| &ib.data[i..i + 4] != pa);
                    let inside = x >= x0 && x < x1 && y >= y0 && y < y1;
                    let near = x >= x0 - m && x < x1 + m && y >= y0 - m && y < y1 + m;
                    if inside {
                        area += 1;
                        if dark(pa) {
                            ink += 1;
                        }
                        if changed {
                            inside_changed += 1;
                        }
                    } else if !near && changed {
                        outside_changed += 1;
                    }
                }
            }
            if area == 0 {
                return Err(format!("Rechteck {r:?} außerhalb der Seite"));
            }
            if ink < 100 {
                return Err(format!("zu wenig Inhalt im Block ({ink} dunkle Pixel von {area})"));
            }
            if !new_page && inside_changed < 100 {
                return Err(format!("Block ändert die Seite kaum ({inside_changed} Pixel)"));
            }
            if outside_changed > 0 {
                return Err(format!("{outside_changed} Pixel außerhalb des Blocks geändert"));
            }
            Ok(format!("Block sichtbar ({ink} px)"))
        }
    }
}

/// qpdf, openssl und PDFium-Pixel für eine Ausgabe; liefert (qpdf, openssl, Bild) als Text und
/// hängt Probleme an.
#[allow(clippy::too_many_arguments)]
pub fn check_output(
    or: &Oracles,
    original_path: &Path,
    original: &[u8],
    signed: &SignedPdf,
    out_path: &Path,
    look: Option<Look>,
    work: &Path,
    problems: &mut Vec<String>,
) -> (String, String, String) {
    let qpdf = match &or.qpdf {
        Some(q) => {
            let b = qpdf_check(q, original_path);
            let a = qpdf_check(q, out_path);
            let new: Vec<_> = a.messages.iter().filter(|m| !b.messages.contains(m)).cloned().collect();
            if !(a.code == 0 || a.code == b.code) || !new.is_empty() {
                problems.push(format!("qpdf Code {} neue Befunde {new:?}", a.code));
            }
            if a.code == 0 { "ok".to_string() } else { format!("wie Original ({})", a.code) }
        }
        None => "–".into(),
    };
    let openssl = match &or.openssl {
        Some(o) => {
            if openssl_verify(o, work, signed) {
                "ok".to_string()
            } else {
                problems.push("openssl cms -verify fehlgeschlagen".into());
                "FEHLER".into()
            }
        }
        None => "–".into(),
    };
    let pixel = match (&or.pdfium, look) {
        (Some(b), Some(l)) => match pixel_check(b, original, &signed.bytes(), l) {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!("PDFium: {e}"));
                "FEHLER".into()
            }
        },
        (None, _) => "–".into(),
        (_, None) => "nicht geprüft".into(),
    };
    (qpdf, openssl, pixel)
}

/// Bewertet die pyHanko-Ergebnisse einer Ausgabe: neue Signatur intakt/gültig/vertraut/ganze
/// Datei; Vorsignaturen behalten Integrität und Gültigkeit, die zuvor vollständige erreicht
/// höchstens `FORM_FILLING`, ältere werden nicht schlechter als vorher (bzw. `FORM_FILLING`).
pub fn evaluate_pyhanko(
    before: &[PyhankoSig],
    after: &[PyhankoSig],
    new_field: &str,
    problems: &mut Vec<String>,
) -> (String, String) {
    let new_desc = match after.iter().find(|s| s.field.as_deref() == Some(new_field)) {
        Some(s) => {
            if !(s.intact && s.valid && s.trusted) {
                problems.push(format!("pyHanko: neue Signatur nicht intakt/gültig/vertraut: {s:?}"));
            }
            if s.coverage.as_deref() != Some("ENTIRE_FILE") {
                problems.push(format!("pyHanko: neue Signatur deckt nicht die ganze Datei ab: {:?}", s.coverage));
            }
            if s.error.is_some() {
                problems.push(format!("pyHanko-Fehler: {:?}", s.error));
            }
            format!(
                "{} / {} / {}",
                if s.intact { "INTACT" } else { "BROKEN" },
                if s.valid { "VALID" } else { "INVALID" },
                s.coverage.as_deref().unwrap_or("-")
            )
        }
        None => {
            problems.push(format!("pyHanko: neue Signatur nicht gefunden ({} Signaturen)", after.len()));
            "fehlt".into()
        }
    };
    let mut prior = Vec::new();
    for b in before {
        let Some(a) = after.iter().find(|s| s.field == b.field) else {
            problems.push("pyHanko: Vorsignatur fehlt".into());
            continue;
        };
        if a.intact != b.intact || a.valid != b.valid {
            problems.push(format!("pyHanko: Status einer Vorsignatur geändert: {b:?} → {a:?}"));
        }
        let rb = modification_rank(b.modification_level.as_deref());
        let ra = modification_rank(a.modification_level.as_deref());
        let limit = if b.coverage.as_deref() == Some("ENTIRE_FILE") { 2 } else { rb.max(2) };
        if ra > limit {
            problems.push(format!(
                "pyHanko: Änderungsstufe einer Vorsignatur {:?} → {:?}",
                b.modification_level, a.modification_level
            ));
        }
        prior.push(format!(
            "{} → {}",
            b.modification_level.as_deref().unwrap_or("-"),
            a.modification_level.as_deref().unwrap_or("-")
        ));
    }
    (new_desc, if prior.is_empty() { "–".into() } else { prior.join("; ") })
}

/// Schreibt einen Abschnitt nach `$CARGO_TARGET_TMPDIR/pades-results-<abschnitt>.md`; mit
/// `PDFOVER_UPDATE_RESULTS=1` wird der Abschnitt in `testdata/pades/RESULTS.md` ersetzt.
pub fn write_results_section(section: &str, body: &str) {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("pades-results-{section}.md"));
    let _ = std::fs::write(&tmp, body);
    eprintln!("Ergebnis: {}", tmp.display());
    if std::env::var_os("PDFOVER_UPDATE_RESULTS").is_none() {
        return;
    }
    let path = testdata().join("pades/RESULTS.md");
    let start = format!("<!-- abschnitt:{section} -->");
    let end = format!("<!-- /abschnitt:{section} -->");
    let block = format!("{start}\n{body}\n{end}");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = match (current.find(&start), current.find(&end)) {
        (Some(s), Some(e)) if e > s => format!("{}{}{}", &current[..s], block, &current[e + end.len()..]),
        _ => format!("{}\n\n{}\n", current.trim_end(), block),
    };
    std::fs::write(&path, updated).unwrap();
}
