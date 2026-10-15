//! Gemeinsame Hilfen für den Golden-Vergleich gegen PDF-AS 4.4.2 (`tests/golden_pdfas442.rs`):
//! Manifest-Strukturen (`testdata/golden/pdfas-4.4.2`, erzeugt von `tools/golden/run-pdfas.sh`),
//! Nachbau eines Falls mit `pdfover-sigblock` und ein Content-Stream-Parser mit demselben
//! Operationsmodell wie `tools/golden/extract.py`.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdfover_pdf_cos::Object;
use pdfover_sigblock::{
    Appearance, BuildOptions, CertFields, Emblem, FontProvider, PdfaMode, Profile, ProfileOverrides, ProfileStore,
    SigBlockInput,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Toleranz für Koordinaten (Plan §8.2: Rect/BBox exakt, Zahlen ±1e-3; Auftrag: ±0,01 pt).
pub const TOL: f32 = 0.01;

pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden/pdfas-4.4.2")
}

pub fn read(rel: &str) -> Vec<u8> {
    let p = golden_dir().join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

// ------------------------------------------------------------------------------------------------
// Manifest
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Index {
    pub cases: Vec<IndexCase>,
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
    #[serde(default)]
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexCase {
    pub id: String,
    pub profile: String,
    pub input: String,
    pub position_key: String,
    pub position: Option<String>,
    pub note_key: Option<String>,
    pub emblem: Option<String>,
    pub base_logo_mm: Option<f64>,
    #[serde(default)]
    pub placeholder: bool,
    pub status: String,
    pub pdf: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaseInfo {
    pub id: String,
    pub profile: String,
    pub input: String,
    pub position_key: String,
    pub position: Option<String>,
    pub note_key: Option<String>,
    pub note: Option<String>,
    pub emblem: Option<String>,
    pub base_logo_mm: Option<f64>,
    #[serde(default)]
    pub placeholder: bool,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub case: CaseInfo,
    pub file: Option<FileInfo>,
    pub page: Option<PageGeom>,
    pub widget: Option<Widget>,
    pub holder: Option<Holder>,
    pub inner: Option<Inner>,
    pub sig_date: Option<String>,
    pub pdfas_log: Option<PdfasLog>,
    pub oracles: Option<Oracles>,
    pub signature: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileInfo {
    pub pages: usize,
    pub input_pages: Option<usize>,
    pub new_page: Option<bool>,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageGeom {
    pub index: usize,
    pub media_box: Vec<f32>,
    pub crop_box: Vec<f32>,
    pub rotate: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Widget {
    pub rect: Vec<f32>,
    pub page_index: Option<usize>,
    pub tu: Option<String>,
    pub field_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Holder {
    pub bbox: Vec<f32>,
    pub matrix: Option<Vec<f32>>,
    pub content: String,
    pub xobjects: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Inner {
    pub bbox: Vec<f32>,
    pub content: String,
    pub ops: Vec<Op>,
    pub texts: Vec<TextBlock>,
    pub fonts: BTreeMap<String, FontInfo>,
    pub images: BTreeMap<String, ImageInfo>,
    #[serde(default)]
    pub color_ops: Vec<String>,
    #[serde(default)]
    pub proc_set: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Rect { x: f32, y: f32, w: f32, h: f32 },
    LineWidth { w: f32 },
    Line { x1: f32, y1: f32, x2: f32, y2: f32 },
    Image { name: String, matrix: Vec<f32> },
    Xobject { name: String, matrix: Vec<f32> },
    Text { index: usize },
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextBlock {
    pub font: Option<String>,
    pub res: Option<String>,
    pub size: Option<f32>,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Line {
    pub x: f32,
    pub y: f32,
    pub hex: String,
    pub text: Option<String>,
    pub width: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FontInfo {
    pub base_font: Option<String>,
    pub subtype: Option<String>,
    pub embedded: Option<String>,
    pub encoding: Option<String>,
    #[serde(default)]
    pub descriptor: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub w: BTreeMap<String, f32>,
    pub cid_set: Option<bool>,
    pub cid_to_gid_map: Option<String>,
    #[serde(default)]
    pub subset_prefix: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub bpc: u32,
    pub color_space: Option<String>,
    pub sha256: String,
    pub smask: Option<SMaskInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SMaskInfo {
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdfasLog {
    pub positioning: Option<Positioning>,
    pub fixed_width: Option<f32>,
    pub table_width: Option<f32>,
    pub crop_box: Option<Vec<f32>>,
    pub rotated_crop_box: Option<Vec<f32>>,
    pub rect: Option<Vec<f32>>,
    pub signature_positioning: Option<String>,
    pub request_positioning: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Positioning {
    pub page: usize,
    pub make_new_page: bool,
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Oracles {
    pub qpdf_check: Option<serde_json::Value>,
    pub pyhanko: Option<serde_json::Value>,
}

pub fn index() -> Index {
    serde_json::from_slice(&read("index.json")).expect("index.json gültig")
}

pub fn manifest(id: &str) -> Manifest {
    serde_json::from_slice(&read(&format!("cases/{id}/manifest.json")))
        .unwrap_or_else(|e| panic!("{id}: Manifest ungültig: {e}"))
}

/// Inhalt (in extract.py als Latin-1 dekodiert) zurück in Bytes.
pub fn latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| u8::try_from(u32::from(c)).expect("Latin-1")).collect()
}

pub fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// Drehung der Holder-`/Matrix` in Grad.
pub fn rotation_of(m: Option<&Vec<f32>>) -> i32 {
    let Some(m) = m else { return 0 };
    match (m[0].round() as i32, m[1].round() as i32) {
        (0, 1) => 90,
        (-1, 0) => 180,
        (0, -1) => 270,
        _ => 0,
    }
}

// ------------------------------------------------------------------------------------------------
// Nachbau mit pdfover-sigblock
// ------------------------------------------------------------------------------------------------

/// Zertifikatsfelder des Test-Zertifikats `testdata/keystores/test-rsa2048.crt.pem`
/// (Subject = Issuer, selbstsigniert, Seriennummer 0x0A0B0C0D01).
pub fn test_cert() -> CertFields {
    let dn = vec![
        ("C".to_string(), "AT".to_string()),
        ("O".to_string(), "PDF-Over NG Test (NUR TEST)".to_string()),
        ("CN".to_string(), "PDF-Over NG Test RSA (NUR TEST)".to_string()),
    ];
    CertFields {
        subject: dn.clone(),
        issuer: dn,
        serial_decimal: u64::from_str_radix("0A0B0C0D01", 16).unwrap().to_string(),
        subject_dn: "CN=PDF-Over NG Test RSA (NUR TEST),O=PDF-Over NG Test (NUR TEST),C=AT".into(),
        issuer_dn: "CN=PDF-Over NG Test RSA (NUR TEST),O=PDF-Over NG Test (NUR TEST),C=AT".into(),
    }
}

pub fn store() -> &'static ProfileStore {
    static STORE: std::sync::OnceLock<ProfileStore> = std::sync::OnceLock::new();
    STORE.get_or_init(|| ProfileStore::builtin().expect("eingebaute Profile"))
}

pub fn emblem(file: &str) -> Arc<Emblem> {
    // PDF-Over übergibt das (ggf. verkleinerte) Cache-Emblem; die Test-Embleme sind kleiner als
    // 480×600 und ohne EXIF, `decode` und `decode_original` liefern dasselbe.
    Arc::new(Emblem::decode(&read(&format!("inputs/{file}"))).unwrap_or_else(|e| panic!("{file}: {e}")))
}

/// Profil mit den PDF-Over-Laufzeit-Overrides des Falls (`PdfAs4Signer.prepare`).
pub fn profile_for(case: &CaseInfo) -> Profile {
    let base = store().profile(&case.profile).unwrap_or_else(|e| panic!("{}: {e}", case.profile));
    let em = case.emblem.as_deref().map(emblem);
    let overrides = if case.profile == "BASE_LOGO" {
        let px = em.as_ref().map(|e| (e.width, e.height)).unwrap_or((1, 1));
        ProfileOverrides::base_logo(em, px, case.base_logo_mm.unwrap_or(23.0))
    } else {
        ProfileOverrides { sig_label: em, sig_note: case.note.clone(), ..Default::default() }
    };
    base.with_overrides(&overrides)
}

/// PDF/A-Behandlung wie PDF-AS ohne XMP-Kennung: PDF/A-Profil ⇒ Teil 1 (Alpha entfernt).
pub fn pdfa_for(profile: &Profile) -> PdfaMode {
    if profile.is_pdfa() { PdfaMode::Part1 } else { PdfaMode::None }
}

/// `SIG_DATE` aus dem Block (Zeitstempel normalisiert: der Nachbau verwendet genau den Zeitpunkt,
/// den PDF-AS geschrieben hat); Blöcke ohne Datum (BASE_LOGO) bekommen einen festen Wert.
pub fn signing_time(m: &Manifest) -> OffsetDateTime {
    let s = m.sig_date.as_deref().unwrap_or("2026-01-01T00:00:00+01:00");
    OffsetDateTime::parse(s, &Rfc3339).unwrap_or_else(|e| panic!("{}: {s}: {e}", m.case.id))
}

/// Baut den Block des Falls mit der von PDF-AS berechneten Breite (`fixed_width`) und Rotation.
pub fn build(m: &Manifest, quirks: bool) -> Appearance {
    let profile = profile_for(&m.case);
    let mut input = SigBlockInput::new(test_cert(), signing_time(m));
    input.pdfas_quirks = quirks;
    input.pdfa = pdfa_for(&profile);
    let width = m.pdfas_log.as_ref().and_then(|l| l.fixed_width).unwrap_or_else(|| panic!("{}: Breite", m.case.id));
    let rotation = rotation_of(m.holder.as_ref().and_then(|h| h.matrix.as_ref()));
    let opts = BuildOptions { width, rotation, pdfa: input.pdfa, preview_only: false };
    let res = store().resources().clone();
    Appearance::build(&profile, &input, &opts, &FontProvider::from_resources(res.clone()), &*res)
        .unwrap_or_else(|e| panic!("{}: {e}", m.case.id))
}

// ------------------------------------------------------------------------------------------------
// Operationsmodell
// ------------------------------------------------------------------------------------------------

/// Normalisierte Zeichenoperation (Farben werden nicht verglichen).
#[derive(Debug, Clone, PartialEq)]
pub enum G {
    Rect([f32; 4]),
    Width(f32),
    Line([f32; 4]),
    Image(String, [f32; 6]),
    Text { font: String, size: f32, lines: Vec<(f32, f32, Vec<u8>)> },
}

impl G {
    pub fn kind(&self) -> &'static str {
        match self {
            G::Rect(_) => "rect",
            G::Width(_) => "line_width",
            G::Line(_) => "line",
            G::Image(..) => "image",
            G::Text { .. } => "text",
        }
    }
}

/// Operationen eines Manifests.
pub fn manifest_ops(inner: &Inner) -> Vec<G> {
    inner
        .ops
        .iter()
        .filter_map(|o| match o {
            Op::Rect { x, y, w, h } => Some(G::Rect([*x, *y, *w, *h])),
            Op::LineWidth { w } => Some(G::Width(*w)),
            Op::Line { x1, y1, x2, y2 } => Some(G::Line([*x1, *y1, *x2, *y2])),
            Op::Image { name, matrix } => Some(G::Image(name.clone(), arr6(matrix))),
            Op::Xobject { .. } => None,
            Op::Text { index } => {
                let t = &inner.texts[*index];
                Some(G::Text {
                    font: t.font.clone().unwrap_or_default(),
                    size: t.size.unwrap_or(0.0),
                    lines: t.lines.iter().map(|l| (l.x, l.y, hex(&l.hex))).collect(),
                })
            }
        })
        .collect()
}

fn arr6(v: &[f32]) -> [f32; 6] {
    let mut m = [0.0; 6];
    m.copy_from_slice(&v[..6]);
    m
}

pub fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOL
}

/// Maximale Zahlenabweichung zweier Operationen; `None`, wenn sie strukturell verschieden sind
/// (Art, Font, Größe, Zeilenzahl, Bytes).
pub fn deviation(a: &G, b: &G) -> Option<f32> {
    let max = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| (p - q).abs()).fold(0f32, f32::max);
    match (a, b) {
        (G::Rect(x), G::Rect(y)) | (G::Line(x), G::Line(y)) => Some(max(x, y)),
        (G::Width(x), G::Width(y)) => Some((x - y).abs()),
        (G::Image(_, x), G::Image(_, y)) => Some(max(x, y)),
        (G::Text { font: f1, size: s1, lines: l1 }, G::Text { font: f2, size: s2, lines: l2 }) => {
            if f1 != f2 || !close(*s1, *s2) || l1.len() != l2.len() || l1.iter().zip(l2).any(|(p, q)| p.2 != q.2) {
                return None;
            }
            Some(l1.iter().zip(l2).map(|(p, q)| (p.0 - q.0).abs().max((p.1 - q.1).abs())).fold(0f32, f32::max))
        }
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------
// Content-Stream des eigenen Blocks
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f32),
    Name(String),
    Str(Vec<u8>),
    Op(String),
}

fn tokenize(s: &[u8]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut i = 0;
    let delim = |b: u8| b.is_ascii_whitespace() || b"/[]<>()".contains(&b);
    while i < s.len() {
        let b = s[i];
        if b.is_ascii_whitespace() || b == b'[' || b == b']' {
            i += 1;
        } else if b == b'(' {
            let mut depth = 1;
            let mut j = i + 1;
            let mut v = Vec::new();
            while j < s.len() {
                let c = s[j];
                if c == b'\\' {
                    let n = s[j + 1];
                    if n.is_ascii_digit() {
                        let mut k = j + 1;
                        let mut val = 0u32;
                        while k < s.len() && k < j + 4 && (b'0'..=b'7').contains(&s[k]) {
                            val = val * 8 + u32::from(s[k] - b'0');
                            k += 1;
                        }
                        v.push(val as u8);
                        j = k;
                        continue;
                    }
                    v.push(match n {
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        b'b' => 8,
                        b'f' => 12,
                        other => other,
                    });
                    j += 2;
                    continue;
                }
                if c == b'(' {
                    depth += 1;
                }
                if c == b')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                v.push(c);
                j += 1;
            }
            out.push(Tok::Str(v));
            i = j + 1;
        } else if b == b'<' {
            let j = i + s[i..].iter().position(|&c| c == b'>').unwrap();
            let hx: Vec<u8> = s[i + 1..j].iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
            let bytes =
                hx.chunks(2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect();
            out.push(Tok::Str(bytes));
            i = j + 1;
        } else if b == b'/' {
            let mut j = i + 1;
            while j < s.len() && !delim(s[j]) {
                j += 1;
            }
            out.push(Tok::Name(String::from_utf8_lossy(&s[i + 1..j]).into()));
            i = j;
        } else {
            let mut j = i;
            while j < s.len() && !delim(s[j]) {
                j += 1;
            }
            let t = String::from_utf8_lossy(&s[i..j]).to_string();
            match t.parse::<f32>() {
                Ok(n) => out.push(Tok::Num(n)),
                Err(_) => out.push(Tok::Op(t)),
            }
            i = j.max(i + 1);
        }
    }
    out
}

fn mul(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// Textblock im Aufbau: Font, Größe, Zeilen (Start x, Start y, kodierte Bytes).
type TextAcc = (String, f32, Vec<(f32, f32, Vec<u8>)>);

/// Parst einen Content-Stream in das Operationsmodell von extract.py (Text: absolute Zeilenstarts).
pub fn parse_content(content: &[u8], fonts: &HashMap<String, String>) -> Vec<G> {
    let mut out = Vec::new();
    let mut nums: Vec<f32> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut strs: Vec<Vec<u8>> = Vec::new();
    let mut ctm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut stack = Vec::new();
    let mut path: Vec<G> = Vec::new();
    let mut cur = (0.0, 0.0);
    let mut text: Option<TextAcc> = None;
    let mut tlm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    for t in tokenize(content) {
        match t {
            Tok::Num(n) => nums.push(n),
            Tok::Name(n) => names.push(n),
            Tok::Str(s) => strs.push(s),
            Tok::Op(op) => {
                match op.as_str() {
                    "q" => stack.push(ctm),
                    "Q" => ctm = stack.pop().unwrap_or(ctm),
                    "cm" => ctm = mul([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]], ctm),
                    "w" => out.push(G::Width(nums[0])),
                    "re" => path.push(G::Rect([nums[0], nums[1], nums[2], nums[3]])),
                    "m" => cur = (nums[0], nums[1]),
                    "l" => {
                        path.push(G::Line([cur.0, cur.1, nums[0], nums[1]]));
                        cur = (nums[0], nums[1]);
                    }
                    "f" | "F" | "f*" | "S" | "s" | "B" | "B*" | "b" | "b*" | "n" => out.append(&mut path),
                    "Do" => out.push(G::Image(names[0].clone(), ctm)),
                    "BT" => {
                        text = Some((String::new(), 0.0, Vec::new()));
                        tlm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                    }
                    "Tf" => {
                        let t = text.as_mut().unwrap();
                        t.0 = fonts.get(&names[0]).cloned().unwrap_or_else(|| names[0].clone());
                        t.1 = nums[0];
                    }
                    "Td" => tlm = mul([1.0, 0.0, 0.0, 1.0, nums[0], nums[1]], tlm),
                    "Tj" => text.as_mut().unwrap().2.push((tlm[4], tlm[5], strs[0].clone())),
                    "ET" => {
                        let (font, size, lines) = text.take().unwrap();
                        if !lines.is_empty() {
                            out.push(G::Text { font, size, lines });
                        }
                    }
                    _ => {}
                }
                nums.clear();
                names.clear();
                strs.clear();
            }
        }
    }
    out
}

pub fn strip_subset(name: &str) -> String {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()) => rest.to_string(),
        _ => name.to_string(),
    }
}

/// Objekte des eigenen Blocks über lokale Referenzen.
pub fn local<'a>(app: &'a Appearance, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference((i, _)) => &app.objects[*i as usize],
        other => other,
    }
}

fn dict_of<'a>(app: &'a Appearance, o: &'a Object) -> Option<&'a pdfover_pdf_cos::Dictionary> {
    match local(app, o) {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

/// Eigenes Inner-Form: Inhalt, Fontnamen (Ressource → BaseFont), Fonts, Bilder.
pub struct OwnForm {
    pub content: Vec<u8>,
    pub font_names: HashMap<String, String>,
    pub fonts: BTreeMap<String, OwnFont>,
    pub images: BTreeMap<String, OwnImage>,
    pub bbox: Vec<f32>,
    pub color_space_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OwnFont {
    pub base_font: String,
    pub subtype: String,
    pub embedded: Option<String>,
    pub descent: Option<f32>,
    pub widths: BTreeMap<u32, f32>,
    pub cid_set: bool,
    pub encoding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OwnImage {
    pub width: u32,
    pub height: u32,
    pub color_space: String,
    pub sha256: String,
    pub smask: Option<String>,
}

fn name_of(o: &Object) -> Option<String> {
    o.as_name().ok().map(|n| String::from_utf8_lossy(n).to_string())
}

fn sha_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

fn decoded(app: &Appearance, o: &Object) -> Vec<u8> {
    match local(app, o) {
        Object::Stream(s) => s.decompressed_content().unwrap_or_else(|_| s.content.clone()),
        _ => Vec::new(),
    }
}

fn parse_w(app: &Appearance, w: &Object) -> BTreeMap<u32, f32> {
    let mut out = BTreeMap::new();
    let Ok(a) = local(app, w).as_array() else { return out };
    let mut k = 0;
    while k < a.len() {
        let first = a[k].as_i64().unwrap_or(0) as u32;
        match local(app, &a[k + 1]) {
            Object::Array(ws) => {
                for (j, v) in ws.iter().enumerate() {
                    out.insert(first + j as u32, v.as_float().unwrap_or(0.0));
                }
                k += 2;
            }
            last => {
                let last = last.as_i64().unwrap_or(0) as u32;
                let v = a[k + 2].as_float().unwrap_or(0.0);
                for c in first..=last {
                    out.insert(c, v);
                }
                k += 3;
            }
        }
    }
    out
}

pub fn own_form(app: &Appearance) -> OwnForm {
    let Object::Stream(s) = &app.objects[app.inner] else { panic!("Inner-Form fehlt") };
    let res = s.dict.get(b"Resources").unwrap().as_dict().unwrap();
    let mut font_names = HashMap::new();
    let mut fonts = BTreeMap::new();
    if let Ok(fd) = res.get(b"Font").and_then(|f| f.as_dict()) {
        for (k, v) in fd.iter() {
            let d = dict_of(app, v).unwrap();
            let base = strip_subset(&name_of(d.get(b"BaseFont").unwrap()).unwrap());
            let subtype = name_of(d.get(b"Subtype").unwrap()).unwrap_or_default();
            let encoding = d.get(b"Encoding").ok().and_then(name_of);
            let (desc_holder, widths) = if subtype == "Type0" {
                let df = d.get(b"DescendantFonts").unwrap();
                let arr = local(app, df).as_array().unwrap();
                let cid = dict_of(app, &arr[0]).unwrap();
                let w = cid.get(b"W").map(|w| parse_w(app, w)).unwrap_or_default();
                (cid, w)
            } else {
                (d, BTreeMap::new())
            };
            let fd = desc_holder.get(b"FontDescriptor").ok().and_then(|o| dict_of(app, o));
            let descent = fd.and_then(|f| f.get(b"Descent").ok()).and_then(|o| o.as_float().ok());
            let embedded = fd.and_then(|f| {
                ["FontFile", "FontFile2", "FontFile3"]
                    .iter()
                    .find(|k| f.get(k.as_bytes()).is_ok())
                    .map(|k| k.to_string())
            });
            let cid_set = fd.is_some_and(|f| f.get(b"CIDSet").is_ok());
            let key = String::from_utf8_lossy(k).to_string();
            font_names.insert(key.clone(), base.clone());
            fonts.insert(key, OwnFont { base_font: base, subtype, embedded, descent, widths, cid_set, encoding });
        }
    }
    let mut images = BTreeMap::new();
    if let Ok(xd) = res.get(b"XObject").and_then(|f| f.as_dict()) {
        for (k, v) in xd.iter() {
            let d = dict_of(app, v).unwrap();
            let cs = match d.get(b"ColorSpace").map(|o| local(app, o)) {
                Ok(Object::Name(n)) => String::from_utf8_lossy(n).to_string(),
                Ok(Object::Array(a)) => a.first().and_then(name_of).unwrap_or_default(),
                _ => String::new(),
            };
            let smask = d.get(b"SMask").ok().map(|o| sha_hex(&decoded(app, o)));
            images.insert(
                String::from_utf8_lossy(k).to_string(),
                OwnImage {
                    width: d.get(b"Width").unwrap().as_i64().unwrap() as u32,
                    height: d.get(b"Height").unwrap().as_i64().unwrap() as u32,
                    color_space: cs,
                    sha256: sha_hex(&decoded(app, v)),
                    smask,
                },
            );
        }
    }
    let color_space_names = res
        .get(b"ColorSpace")
        .and_then(|c| c.as_dict())
        .map(|d| d.iter().map(|(k, _)| String::from_utf8_lossy(k).to_string()).collect())
        .unwrap_or_default();
    let bbox = s.dict.get(b"BBox").unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap_or(0.0)).collect();
    OwnForm { content: s.content.clone(), font_names, fonts, images, bbox, color_space_names }
}
