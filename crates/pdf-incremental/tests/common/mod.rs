//! Gemeinsame Test-Hilfen für `pdfover-pdf-incremental`: Korpus, externe Orakel (qpdf, openssl,
//! pyHanko) und ein Dummy-Signaturdictionary mit leerem Widget (Struktur wie PDF-AS, analyse/03 §4.4).
//!
//! Externe Werkzeuge sind optional: fehlen sie, überspringen die Tests die jeweilige Prüfung und
//! melden das auf stderr. Kein Netzverkehr (pyHanko ohne Fetcher, siehe `pyhanko_report.py`).
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use pdfover_pdf_cos::{Dictionary, Object, ObjectId, SigField, SourcePdf, Stream, StringFormat};
use pdfover_pdf_incremental::{
    Increment, IncrementBuilder, IncrementOptions, Result, SignatureDictSpec, format_pdf_date,
};

/// Wurzel der gemeinsamen Testdaten.
pub fn testdata() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata"))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            out.push(p);
        }
    }
}

/// Alle PDFs des gemeinsamen Korpus (`testdata/pdf/`) und der Ordner von Paket A.
pub fn corpus() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["pdf", "pdf-cos", "pdf-incremental"] {
        collect(&testdata().join(sub), &mut out);
    }
    out
}

/// Relativer Name für Meldungen.
pub fn rel(p: &Path) -> String {
    p.strip_prefix(testdata()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Dateien, die erwartbar nicht geöffnet werden können (K5).
pub fn expected_open_error(name: &str) -> bool {
    name.ends_with("simple-aes256-userpw.pdf")
}

// ------------------------------------------------------------------------------------------------
// Externe Werkzeuge
// ------------------------------------------------------------------------------------------------

/// Sucht ein Programm im `PATH`.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in [cmd.to_string(), format!("{cmd}.exe")] {
            let p = dir.join(&name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Python mit pyHanko: `PDFOVER_PYTHON`, sonst `python3` neben `pyhanko`, sonst `python3` im PATH.
pub fn pyhanko_python() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(p) = std::env::var_os("PDFOVER_PYTHON") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(ph) = which("pyhanko")
        && let Some(dir) = ph.parent()
    {
        candidates.push(dir.join("python3"));
        candidates.push(dir.join("python.exe"));
    }
    if let Some(p) = which("python3") {
        candidates.push(p);
    }
    candidates.into_iter().find(|p| {
        p.is_file()
            && Command::new(p)
                .args(["-I", "-c", "import pyhanko.sign.validation"])
                .output()
                .is_ok_and(|o| o.status.success())
    })
}

/// Ergebnis von `qpdf --check`.
#[derive(Debug, Clone)]
pub struct QpdfResult {
    pub code: i32,
    /// Warnungen/Fehler ohne Dateinamen
    pub messages: Vec<String>,
}

pub fn qpdf_check(qpdf: &Path, file: &Path) -> QpdfResult {
    let out = Command::new(qpdf).arg("--check").arg(file).output().expect("qpdf startet");
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let fname = file.to_string_lossy().to_string();
    let messages = text
        .lines()
        .filter(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("warning") || l.contains("error")
        })
        .filter(|l| !l.contains("No syntax or stream encoding errors") && !l.contains("errors that qpdf cannot detect"))
        .map(|l| l.replace(&fname, "<datei>"))
        .map(|l| {
            // Offsets/Objektnummern in Meldungen sind dateispezifisch – nur der Text zählt.
            l.split(": ").last().unwrap_or(&l).to_string()
        })
        .collect();
    QpdfResult { code: out.status.code().unwrap_or(-1), messages }
}

/// Ein Eintrag aus `pyhanko_report.py`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct PyhankoSig {
    pub field: String,
    pub intact: bool,
    pub valid: bool,
    pub trusted: bool,
    pub coverage: Option<String>,
    pub modification_level: Option<String>,
    pub docmdp_ok: Option<bool>,
    pub error: Option<String>,
}

pub fn pyhanko_report(python: &Path, file: &Path, trust: &[&Path]) -> std::result::Result<Vec<PyhankoSig>, String> {
    let script = testdata().join("pdf-incremental/pyhanko_report.py");
    let mut cmd = Command::new(python);
    cmd.arg("-I").arg(&script).arg(file);
    for t in trust {
        cmd.arg(t);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("pyHanko-Skript fehlgeschlagen: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().rev().find(|l| l.trim_start().starts_with('[')).ok_or("keine JSON-Ausgabe")?;
    serde_json::from_str(line).map_err(|e| e.to_string())
}

/// Rangfolge der pyHanko-Änderungsstufen (`ModificationLevel`).
pub fn modification_rank(level: Option<&str>) -> u8 {
    match level {
        Some("NONE") => 0,
        Some("LTA_UPDATES") => 1,
        Some("FORM_FILLING") => 2,
        Some("ANNOTATIONS") => 3,
        _ => 4, // OTHER oder unbekannt
    }
}

// ------------------------------------------------------------------------------------------------
// Dummy-Signatur
// ------------------------------------------------------------------------------------------------

/// Fester Zeitpunkt für `/M`.
pub fn signing_time() -> String {
    format_pdf_date(time::macros::datetime!(2026-10-14 12:00:00 +02:00))
}

pub const TEST_NAME: &str = "PDF-Over NG Test (NUR TEST)";

/// Nächste Nummer für „PDF-AS Signatur<n>“ (oberste Ebene, Präfix-Zählung wie PDF-AS `countSignatures`).
pub fn next_field_number(fields: &[SigField]) -> u32 {
    fields
        .iter()
        .filter(|f| f.depth == 0)
        .filter_map(|f| f.partial_name.as_deref()?.strip_prefix("PDF-AS Signatur")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn rect0() -> Object {
    Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()])
}

fn empty_appearance() -> Object {
    let mut d = Dictionary::new();
    d.set("Type", name("XObject"));
    d.set("Subtype", name("Form"));
    d.set("BBox", rect0());
    d.set("Resources", Object::Dictionary(Dictionary::new()));
    Object::Stream(Stream::new(d, Vec::new()))
}

/// Signaturdictionary-Spezifikation der Tests.
pub fn test_spec(reserve: usize) -> SignatureDictSpec {
    SignatureDictSpec {
        signing_time: Some(signing_time()),
        name: Some(TEST_NAME.into()),
        reason: Some("PAdES Signature".into()),
        contents_reserve: reserve,
        ..SignatureDictSpec::default()
    }
}

/// Hängt `widget` an `/Fields` des AcroForms an (neues AcroForm direkt im Katalog wie PDFBox).
fn register_field(b: &mut IncrementBuilder<'_>, src: &SourcePdf, widget: ObjectId) -> Result<()> {
    let catalog_id = src.catalog_id()?;
    let mut catalog = src.catalog()?.clone();
    let push_fields = |b: &mut IncrementBuilder<'_>, acro: &mut Dictionary| -> Result<()> {
        match acro.get(b"Fields") {
            Ok(Object::Reference(fid)) => {
                let fid = *fid;
                let mut arr = src.get_object(fid)?.as_array().cloned().unwrap_or_default();
                arr.push(Object::Reference(widget));
                b.update_object(fid, Object::Array(arr))?;
            }
            Ok(Object::Array(a)) => {
                let mut a = a.clone();
                a.push(Object::Reference(widget));
                acro.set("Fields", Object::Array(a));
            }
            _ => acro.set("Fields", Object::Array(vec![Object::Reference(widget)])),
        }
        let flags = acro.get(b"SigFlags").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
        acro.set("SigFlags", Object::Integer(flags | 3));
        Ok(())
    };
    match catalog.get(b"AcroForm").cloned() {
        Ok(Object::Reference(aid)) => {
            let mut acro = src.get_object(aid)?.as_dict().cloned().unwrap_or_default();
            push_fields(b, &mut acro)?;
            b.update_object(aid, Object::Dictionary(acro))?;
        }
        Ok(Object::Dictionary(d)) => {
            let mut acro = d.clone();
            push_fields(b, &mut acro)?;
            catalog.set("AcroForm", Object::Dictionary(acro));
            b.update_object(catalog_id, Object::Dictionary(catalog))?;
        }
        _ => {
            let mut acro = Dictionary::new();
            push_fields(b, &mut acro)?;
            catalog.set("AcroForm", Object::Dictionary(acro));
            b.update_object(catalog_id, Object::Dictionary(catalog))?;
        }
    }
    Ok(())
}

/// Hängt `widget` an `/Annots` der Seite an (indirektes Array → Array-Objekt fortschreiben).
fn register_annot(b: &mut IncrementBuilder<'_>, src: &SourcePdf, page_index: usize, widget: ObjectId) -> Result<()> {
    let page = &src.pages()[page_index];
    if let Some(arr_id) = page.annots_indirect {
        let mut arr = src.get_object(arr_id)?.as_array().cloned().unwrap_or_default();
        arr.push(Object::Reference(widget));
        b.update_object(arr_id, Object::Array(arr))?;
    } else {
        let mut pd = src.get_object(page.object_id)?.as_dict().cloned().unwrap_or_default();
        let mut annots = match pd.get(b"Annots") {
            Ok(o) => src.resolve(o).ok().and_then(|o| o.as_array().ok()).cloned().unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        annots.push(Object::Reference(widget));
        pd.set("Annots", Object::Array(annots));
        b.update_object(page.object_id, Object::Dictionary(pd))?;
    }
    Ok(())
}

/// Increment mit Dummy-Signaturdictionary und leerem, unsichtbarem Widget auf Seite 1
/// (Feld `PDF-AS Signatur<n>`, `/F 132`, `/Rect [0 0 0 0]`, leeres `/AP`; analyse/03 §4.4/§4.6).
pub fn add_dummy_signature(src: &SourcePdf, opts: IncrementOptions, reserve: usize) -> Result<Increment> {
    let n = next_field_number(&src.signature_fields()?);
    let mut b = IncrementBuilder::new(src, opts);
    let sig_id = b.add_signature_dictionary(test_spec(reserve))?;
    let ap_id = b.add_object(empty_appearance());
    let widget_id = b.allocate_id();
    let page = &src.pages()[0];
    let mut w = Dictionary::new();
    w.set("FT", name("Sig"));
    w.set("Type", name("Annot"));
    w.set("Subtype", name("Widget"));
    w.set("F", Object::Integer(132));
    w.set("T", Object::String(format!("PDF-AS Signatur{n}").into_bytes(), StringFormat::Literal));
    w.set("V", Object::Reference(sig_id));
    w.set("P", Object::Reference(page.object_id));
    w.set("Rect", rect0());
    let mut ap = Dictionary::new();
    ap.set("N", Object::Reference(ap_id));
    w.set("AP", Object::Dictionary(ap));
    b.insert_new(widget_id, Object::Dictionary(w))?;
    register_field(&mut b, src, widget_id)?;
    register_annot(&mut b, src, 0, widget_id)?;
    b.build()
}

/// Befüllt ein vorhandenes leeres Signaturfeld (analyse/03 §4.5): `/V` am Feld, Widget behält
/// `/Rect`, bekommt leeres `/AP` und Print-Flag; Widgets in `/Kids` werden selbst fortgeschrieben.
pub fn fill_empty_field(src: &SourcePdf, field: &SigField, reserve: usize) -> Result<Increment> {
    let mut b = IncrementBuilder::new(src, IncrementOptions::default());
    let sig_id = b.add_signature_dictionary(test_spec(reserve))?;
    let ap_id = b.add_object(empty_appearance());
    let mut fd = src.get_object(field.field_id)?.as_dict().cloned().unwrap_or_default();
    fd.set("V", Object::Reference(sig_id));
    let mut ap = Dictionary::new();
    ap.set("N", Object::Reference(ap_id));
    let merged = field.widgets.iter().any(|w| w.id == field.field_id);
    if merged {
        fd.set("AP", Object::Dictionary(ap.clone()));
        let f = fd.get(b"F").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
        fd.set("F", Object::Integer(f | 4));
    }
    b.update_object(field.field_id, Object::Dictionary(fd))?;
    for w in field.widgets.iter().filter(|w| w.id != field.field_id) {
        let mut wd = src.get_object(w.id)?.as_dict().cloned().unwrap_or_default();
        wd.set("AP", Object::Dictionary(ap.clone()));
        wd.set("F", Object::Integer(i64::from(w.annot_flags | 4)));
        b.update_object(w.id, Object::Dictionary(wd))?;
    }
    // /SigFlags 3 am AcroForm
    let catalog_id = src.catalog_id()?;
    let mut catalog = src.catalog()?.clone();
    match catalog.get(b"AcroForm").cloned() {
        Ok(Object::Reference(aid)) => {
            let mut acro = src.get_object(aid)?.as_dict().cloned().unwrap_or_default();
            let flags = acro.get(b"SigFlags").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
            acro.set("SigFlags", Object::Integer(flags | 3));
            b.update_object(aid, Object::Dictionary(acro))?;
        }
        Ok(Object::Dictionary(mut acro)) => {
            let flags = acro.get(b"SigFlags").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
            acro.set("SigFlags", Object::Integer(flags | 3));
            catalog.set("AcroForm", Object::Dictionary(acro));
            b.update_object(catalog_id, Object::Dictionary(catalog))?;
        }
        _ => {}
    }
    b.build()
}

/// Liest das 36-Zeichen-ByteRange-Feld der fertigen Datei.
pub fn byte_range_text(out: &[u8], inc: &Increment) -> String {
    String::from_utf8_lossy(&out[inc.byte_range_field.start as usize..inc.byte_range_field.end as usize]).into_owned()
}
