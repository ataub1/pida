//! Appearance-XObjects als lopdf-Objektgraph (docs/analyse/03 §4.7, docs/analyse/04 §3.7).
//!
//! Der Graph verwendet **lokale Referenzen** `Object::Reference((i, LOCAL_GENERATION))` mit
//! `i` = Index in [`Appearance::objects`]. [`Appearance::materialize`] ersetzt sie durch echte IDs
//! des `IncrementBuilder` (`pdfover-pdf-incremental`).
//!
//! Aufbau wie PDF-AS (`PDFAsVisualSignatureBuilder`/`PDFAsTemplateCreator`):
//! - Holder-Form (`/AP /N`): `/BBox [W+1 H+1 0 0]`, `/Matrix` = Rotation, Inhalt
//!   `q 1.0 0.0 0.0 1.0 0.0 0.0 cm /FRM1 Do Q`, Ressourcen `/XObject /FRM1` + `/ProcSet`.
//! - Inner-Form `/FRM1`: gleiche BBox, Tabelle bei (0, 1) gezeichnet (Hintergrund, Rahmenlinien
//!   einzeln, Inhalt), unkomprimiert, Ressourcen `/Font /F1…`, `/XObject /Im1…`, `/ProcSet`.
//! - Zahlen im Content-Stream wie PDFBox (`formatFloatFast`, 5 Nachkommastellen), Strings wie
//!   `COSWriter.writeString` (Literal bei ASCII ohne CR/LF, sonst Hex).
//! - Farben: im Standard DeviceGray für Grautöne (`g`, Plan §6.5), mit `pdfas_quirks` exakt wie
//!   PDF-AS (`/DeviceRGB cs … sc`).
//! - Bildmarken und farbige Hintergründe: DeviceRGB wie PDF-AS; bei PDF/A (ohne `pdfas_quirks`)
//!   `ICCBased` mit eingebettetem sRGB-Profil (Plan §6.5), damit der Block unabhängig vom
//!   OutputIntent des Dokuments (z. B. CMYK) konform bleibt.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write as _;
use std::sync::Arc;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use pdfover_pdf_cos::{Dictionary, Object, ObjectId, Stream, lopdf};

use crate::Result;
use crate::SigBlockError;
use crate::emblem::Emblem;
use crate::fonts::{FontProvider, LoadedFont, TtfMetrics};
use crate::input::{PdfaMode, SigBlockInput};
use crate::java::pdf_number;
use crate::layout::{self, Layout, Sink, TextOp, TextPlacement};
use crate::profile::{Profile, Resources};

/// Generationsnummer lokaler Referenzen (kommt in neuen Objekten nie vor).
pub const LOCAL_GENERATION: u16 = u16::MAX;

/// Optionen für [`Appearance::build`].
#[derive(Debug, Clone, PartialEq)]
pub struct BuildOptions {
    /// Blockbreite aus der Positionierung.
    pub width: f32,
    /// Rotation des Inhalts: Blockrotation + Seitenrotation (Grad, Vielfaches von 90) → `/Matrix`.
    pub rotation: i32,
    /// PDF/A-Behandlung (`None` ⇒ [`SigBlockInput::pdfa`]). Achtung: Wer die Blockhöhe für die
    /// Positionierung mit [`crate::layout::layout`] misst, muss dort denselben Modus über
    /// `SigBlockInput::pdfa` setzen – bei PDF/A ersetzt DejaVu die Standard-14-Captions und kann den
    /// Umbruch ändern. Einfacher: die Maße aus [`Appearance::layout`] verwenden.
    pub pdfa: PdfaMode,
    /// Nur Vorschau: Holder ohne `Do` (wie `buildPreviewOnly`), Tabelle als Seiteninhalt im Mini-PDF.
    pub preview_only: bool,
}

/// Fertiges Erscheinungsbild. `Debug` zeigt keine Inhalte (ADR 0001 Regel 9).
#[derive(Clone, PartialEq)]
pub struct Appearance {
    /// Breite W in pt.
    pub width: f32,
    /// Höhe H in pt.
    pub height: f32,
    /// Layout (für Vorschau, `/TU`, Tests).
    pub layout: Option<Layout>,
    /// Objekte (Holder, Inner, Fonts, Deskriptoren, FontFile2, ToUnicode, Bilder, SMasks …).
    pub objects: Vec<Object>,
    /// Index des Holder-Form-XObjects (Ziel von `/AP /N`).
    pub holder: usize,
    /// Index des Inner-Form-XObjects (`/DR /XObject /FRM1`). Bei [`Appearance::invisible`] gleich
    /// `holder` (es gibt kein Inner-Form, `inner_name` ist leer).
    pub inner: usize,
    /// Ressourcenname des Inner-Forms (`FRM1`).
    pub inner_name: String,
    /// `/TU`-Text.
    pub tu_text: String,
}

/// Appearance mit echten Objekt-IDs. `Debug` zeigt keine Inhalte (ADR 0001 Regel 9).
#[derive(Clone, PartialEq)]
pub struct MaterializedAppearance {
    /// ID des Holder-Forms.
    pub holder_id: ObjectId,
    /// ID des Inner-Forms.
    pub inner_id: ObjectId,
    /// Ressourcenname des Inner-Forms.
    pub inner_name: String,
    /// Alle Objekte mit ihren IDs (in [`Appearance::objects`]-Reihenfolge).
    pub objects: Vec<(ObjectId, Object)>,
}

impl std::fmt::Debug for Appearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Appearance")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("layout", &self.layout)
            .field("objects", &self.objects.len())
            .field("holder", &self.holder)
            .field("inner", &self.inner)
            .field("inner_name", &self.inner_name)
            .field("tu_text_len", &self.tu_text.chars().count())
            .finish()
    }
}

impl std::fmt::Debug for MaterializedAppearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterializedAppearance")
            .field("holder_id", &self.holder_id)
            .field("inner_id", &self.inner_id)
            .field("inner_name", &self.inner_name)
            .field("objects", &self.objects.len())
            .finish()
    }
}

const PROC_SET: [&str; 5] = ["PDF", "Text", "ImageC", "ImageB", "ImageI"];
const INNER_NAME: &str = "FRM1";

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn local(i: usize) -> Object {
    Object::Reference((i as u32, LOCAL_GENERATION))
}

fn proc_set() -> Object {
    Object::Array(PROC_SET.iter().map(|n| name(n)).collect())
}

fn plain_stream(dict: Dictionary, content: Vec<u8>) -> Object {
    Object::Stream(Stream::new(dict, content).with_compression(false))
}

fn flate(data: &[u8]) -> Result<Vec<u8>> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data)?;
    Ok(enc.finish()?)
}

fn flate_stream(mut dict: Dictionary, data: &[u8]) -> Result<Object> {
    dict.set("Filter", name("FlateDecode"));
    Ok(plain_stream(dict, flate(data)?))
}

// ------------------------------------------------------------------------------------------------
// Content-Stream wie PDPageContentStream
// ------------------------------------------------------------------------------------------------

/// Ressourcenname des sRGB-`ICCBased`-Farbraums (PDF/A).
const ICC_NAME: &str = "CS1";

struct Content {
    out: Vec<u8>,
    quirks: bool,
    fill_rgb_space: bool,
    /// Nicht-graue Füllfarben über den sRGB-`ICCBased`-Farbraum (PDF/A ohne Quirks).
    icc: bool,
    /// Der `ICCBased`-Farbraum ist gerade als Füllfarbraum gesetzt.
    icc_active: bool,
    /// Der `ICCBased`-Farbraum wurde verwendet (Ressource nötig).
    icc_used: bool,
    /// Eine Zahl war NaN/unendlich (PDFBox: `IllegalArgumentException`); Ergebnis wird verworfen.
    non_finite: bool,
}

impl Content {
    fn new(quirks: bool) -> Content {
        Content {
            out: Vec::new(),
            quirks,
            fill_rgb_space: false,
            icc: false,
            icc_active: false,
            icc_used: false,
            non_finite: false,
        }
    }

    fn num(&mut self, v: f32) {
        if !v.is_finite() {
            self.non_finite = true;
        }
        self.out.extend_from_slice(pdf_number(v).as_bytes());
        self.out.push(b' ');
    }

    fn op(&mut self, op: &str) {
        self.out.extend_from_slice(op.as_bytes());
        self.out.push(b'\n');
    }

    fn name(&mut self, n: &str) {
        self.out.push(b'/');
        self.out.extend_from_slice(n.as_bytes());
        self.out.push(b' ');
    }

    /// `setNonStrokingColor(java.awt.Color)`.
    fn fill_color(&mut self, rgb: [u8; 3]) {
        let comps = rgb.map(|c| f32::from(c) / 255.0);
        if !self.quirks && rgb[0] == rgb[1] && rgb[1] == rgb[2] {
            self.num(comps[0]);
            self.op("g");
            self.icc_active = false;
            return;
        }
        if self.icc && !self.quirks {
            if !self.icc_active {
                self.name(ICC_NAME);
                self.op("cs");
                self.icc_active = true;
                self.icc_used = true;
            }
            for c in comps {
                self.num(c);
            }
            self.op("sc");
            return;
        }
        if self.quirks {
            if !self.fill_rgb_space {
                self.name("DeviceRGB");
                self.op("cs");
                self.fill_rgb_space = true;
            }
            for c in comps {
                self.num(c);
            }
            self.op("sc");
        } else {
            for c in comps {
                self.num(c);
            }
            self.op("rg");
        }
    }

    fn rect_fill(&mut self, x: f32, y: f32, w: f32, h: f32) {
        for v in [x, y, w, h] {
            self.num(v);
        }
        self.op("re");
        self.op("f");
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.num(x1);
        self.num(y1);
        self.op("m");
        self.num(x2);
        self.num(y2);
        self.op("l");
        self.op("S");
    }

    /// `COSWriter.writeString`
    fn string(&mut self, bytes: &[u8]) {
        let ascii = bytes.iter().all(|&b| b < 0x80 && b != b'\r' && b != b'\n');
        if ascii {
            self.out.push(b'(');
            for &b in bytes {
                if matches!(b, b'(' | b')' | b'\\') {
                    self.out.push(b'\\');
                }
                self.out.push(b);
            }
            self.out.push(b')');
        } else {
            self.out.push(b'<');
            for b in bytes {
                self.out.extend_from_slice(format!("{b:02X}").as_bytes());
            }
            self.out.push(b'>');
        }
        self.out.push(b' ');
    }
}

/// Verwendete Fonts in Ressourcenreihenfolge (`F1`, `F2` …) und benutzte Glyphen.
#[derive(Default)]
struct FontUse {
    order: Vec<String>,
    fonts: HashMap<String, Arc<LoadedFont>>,
    /// GID → erstes dafür verwendetes Zeichen
    glyphs: HashMap<String, BTreeMap<u16, char>>,
}

impl FontUse {
    /// `PDResources.add(font)`: gleicher Font ⇒ gleicher Name.
    fn resource(&mut self, key: &str, font: &Arc<LoadedFont>) -> String {
        if !self.fonts.contains_key(key) {
            self.order.push(key.to_string());
            self.fonts.insert(key.to_string(), font.clone());
        }
        let idx = self.order.iter().position(|k| k == key).unwrap_or(0);
        format!("F{}", idx + 1)
    }

    fn record(&mut self, key: &str, font: &LoadedFont, text: &str) {
        if let Some(t) = font.ttf() {
            let g = self.glyphs.entry(key.to_string()).or_default();
            for c in text.chars() {
                if let Some(gid) = t.gid(c) {
                    g.entry(gid).or_insert(c);
                }
            }
        }
    }
}

/// Schreibt die Zeichenoperationen von [`layout::Prepared::draw`] wie `PDPageContentStream`.
struct ContentSink {
    cs: Content,
    fonts: FontUse,
}

impl Sink for ContentSink {
    fn background(&mut self, rgb: [u8; 3], x: f32, y: f32, w: f32, h: f32) {
        self.cs.fill_color(rgb);
        self.cs.rect_fill(x, y, w, h);
        self.cs.fill_color([0, 0, 0]);
    }

    fn line_width(&mut self, w: f32) {
        self.cs.num(w);
        self.cs.op("w");
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.cs.line(x1, y1, x2, y2);
    }

    fn text(&mut self, op: TextOp<'_>) -> Result<()> {
        let res = self.fonts.resource(&op.font_key, op.font);
        for l in op.lines {
            self.fonts.record(&op.font_key, op.font, l);
        }
        self.cs.op("BT");
        self.cs.name(&res);
        self.cs.num(op.size);
        self.cs.op("Tf");
        match op.placement {
            TextPlacement::Block { tx, ty } => {
                self.cs.num(tx);
                self.cs.num(ty);
                self.cs.op("Td");
                for (k, l) in op.lines.iter().enumerate() {
                    self.cs.string(&op.font.encode(l)?);
                    self.cs.op("Tj");
                    if k + 1 < op.lines.len() {
                        self.cs.num(0.0);
                        self.cs.num(-op.size);
                        self.cs.op("Td");
                    }
                }
            }
            TextPlacement::LineCenter { y0, offsets } => {
                for (k, l) in op.lines.iter().enumerate() {
                    let off = offsets[k];
                    self.cs.num(off);
                    self.cs.num(if k == 0 { y0 } else { -op.size });
                    self.cs.op("Td");
                    self.cs.string(&op.font.encode(l)?);
                    self.cs.op("Tj");
                    self.cs.num(-off);
                    self.cs.num(0.0);
                    self.cs.op("Td");
                }
            }
        }
        self.cs.op("ET");
        Ok(())
    }

    fn image(&mut self, index: usize, x: f32, y: f32, w: f32, h: f32) {
        self.cs.op("q");
        for v in [w, 0.0, 0.0, h, x, y] {
            self.cs.num(v);
        }
        self.cs.op("cm");
        self.cs.name(&format!("Im{}", index + 1));
        self.cs.op("Do");
        self.cs.op("Q");
    }
}

// ------------------------------------------------------------------------------------------------
// Fonts (Type1 / Type0-CIDFontType2-Subset)
// ------------------------------------------------------------------------------------------------

/// Composite-Komponenten hinzufügen (wie `TTFSubsetter.addCompoundReferences`).
fn glyph_closure(t: &TtfMetrics, gids: &mut BTreeSet<u16>) {
    use skrifa::raw::tables::glyf::Glyph;
    use skrifa::raw::types::GlyphId;
    use skrifa::raw::{FontRef, TableProvider};
    let Ok(font) = FontRef::new(&t.data) else { return };
    let (Ok(loca), Ok(glyf)) = (font.loca(None), font.glyf()) else { return };
    let mut stack: Vec<u16> = gids.iter().copied().collect();
    while let Some(g) = stack.pop() {
        if let Ok(Some(Glyph::Composite(c))) = loca.get_glyf(GlyphId::new(u32::from(g)), &glyf) {
            for comp in c.components() {
                let cg = comp.glyph.to_u16();
                if gids.insert(cg) {
                    stack.push(cg);
                }
            }
        }
    }
}

/// Deterministisches Subset-Präfix (6 Großbuchstaben + `+`, Alphabet wie PDFBox).
fn subset_tag(gids: &BTreeSet<u16>) -> String {
    const BASE25: &[u8] = b"BCDEFGHIJKLMNOPQRSTUVWXYZ";
    let key: String = gids.iter().map(|g| format!("{g},")).collect();
    let mut num = i64::from(crate::java::string_hash(&key)).unsigned_abs();
    let mut s = Vec::new();
    loop {
        s.push(BASE25[(num % 25) as usize]);
        num /= 25;
        if num == 0 || s.len() >= 6 {
            break;
        }
    }
    while s.len() < 6 {
        s.insert(0, b'A');
    }
    format!("{}+", String::from_utf8_lossy(&s))
}

fn utf16_hex(c: char) -> String {
    let mut buf = [0u16; 2];
    c.encode_utf16(&mut buf).iter().map(|u| format!("{u:04X}")).collect()
}

/// ToUnicode-CMap wie `ToUnicodeWriter` (Bereiche konsekutiver CIDs/Unicodes, je Block ≤ 100).
fn to_unicode_cmap(map: &BTreeMap<u16, char>) -> Vec<u8> {
    let mut ranges: Vec<(u16, u16, char)> = Vec::new();
    for (&cid, &c) in map {
        if let Some(last) = ranges.last_mut() {
            let (start, end, first) = *last;
            let next_uni = char::from_u32(first as u32 + u32::from(end - start) + 1);
            if cid == end.wrapping_add(1)
                && cid & 0xFF != 0
                && (c as u32) <= 0xFFFF
                && next_uni == Some(c)
                && (c as u32) & 0xFF != 0
            {
                last.1 = cid;
                continue;
            }
        }
        ranges.push((cid, cid, c));
    }
    let mut s = String::new();
    s.push_str("/CIDInit /ProcSet findresource begin\n12 dict begin\n\nbegincmap\n/CIDSystemInfo\n<< /Registry (Adobe)\n/Ordering (UCS)\n/Supplement 0\n>> def\n\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    for chunk in ranges.chunks(100) {
        s.push_str(&format!("\n{} beginbfrange\n", chunk.len()));
        for (a, b, c) in chunk {
            s.push_str(&format!("<{a:04X}> <{b:04X}> <{}>\n", utf16_hex(*c)));
        }
        s.push_str("endbfrange\n");
    }
    s.push_str("\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s.into_bytes()
}

fn real_array(v: &[f32]) -> Object {
    Object::Array(v.iter().map(|x| Object::Real(*x)).collect())
}

/// Baut die Objekte eines Fonts; liefert den Index des Font-Dictionarys.
fn push_font(
    objects: &mut Vec<Object>,
    font: &LoadedFont,
    used: Option<&BTreeMap<u16, char>>,
    pdfa: PdfaMode,
) -> Result<usize> {
    let Some(t) = font.ttf() else {
        let mut d = Dictionary::new();
        d.set("Type", name("Font"));
        d.set("Subtype", name("Type1"));
        d.set("BaseFont", name(font.base_font()));
        d.set("Encoding", name("WinAnsiEncoding"));
        objects.push(Object::Dictionary(d));
        return Ok(objects.len() - 1);
    };
    let empty = BTreeMap::new();
    let used = used.unwrap_or(&empty);
    let mut gids: BTreeSet<u16> = used.keys().copied().collect();
    gids.insert(0);
    glyph_closure(t, &mut gids);
    let mut remapper = subsetter::GlyphRemapper::new();
    for g in &gids {
        remapper.remap(*g);
    }
    let subset = subsetter::subset(&t.data, 0, &remapper)
        .map_err(|e| SigBlockError::Font(format!("{}: Subset: {e}", t.file)))?;
    let tag = subset_tag(&gids);
    let base = format!("{tag}{}", t.ps_name);

    // ToUnicode: verwendete Zeichen, sonst (Komponenten) das kleinste Zeichen aus der cmap
    let mut reverse: HashMap<u16, u32> = HashMap::new();
    for (&cp, &g) in &t.cmap {
        reverse.entry(g).and_modify(|c| *c = (*c).min(cp)).or_insert(cp);
    }
    let mut uni: BTreeMap<u16, char> = BTreeMap::new();
    for &g in &gids {
        if g == 0 {
            continue;
        }
        if let Some(c) = used.get(&g).copied().or_else(|| reverse.get(&g).and_then(|cp| char::from_u32(*cp))) {
            uni.insert(g, c);
        }
    }

    // /W: c [w …] für konsekutive CIDs, Breite 1000 (= DW) wird ausgelassen
    let mut w_arr: Vec<Object> = Vec::new();
    let mut prev: Option<u16> = None;
    for &g in &gids {
        let w = t.widths.get(g as usize).copied().unwrap_or(1000);
        if w == 1000 {
            continue;
        }
        if prev.is_none_or(|p| p + 1 != g) {
            w_arr.push(Object::Integer(i64::from(g)));
            w_arr.push(Object::Array(Vec::new()));
        }
        if let Some(Object::Array(a)) = w_arr.last_mut() {
            a.push(Object::Integer(i64::from(w)));
        }
        prev = Some(g);
    }
    let max_cid = *gids.iter().next_back().unwrap_or(&0);
    let mut cid_to_gid = Vec::with_capacity((usize::from(max_cid) + 1) * 2);
    for cid in 0..=max_cid {
        let ng = if gids.contains(&cid) { remapper.get(cid).unwrap_or(0) } else { 0 };
        cid_to_gid.extend_from_slice(&ng.to_be_bytes());
    }
    let mut cid_set = vec![0u8; usize::from(max_cid) / 8 + 1];
    for &g in &gids {
        cid_set[usize::from(g) / 8] |= 1 << (7 - g % 8);
    }

    let type0 = objects.len();
    let cid_font = type0 + 1;
    let descriptor = type0 + 2;
    let font_file = type0 + 3;
    let to_unicode = type0 + 4;
    let cid_map = type0 + 5;
    let cid_set_idx = type0 + 6;

    let mut d0 = Dictionary::new();
    d0.set("Type", name("Font"));
    d0.set("Subtype", name("Type0"));
    d0.set("BaseFont", name(&base));
    d0.set("Encoding", name("Identity-H"));
    d0.set("DescendantFonts", Object::Array(vec![local(cid_font)]));
    d0.set("ToUnicode", local(to_unicode));
    objects.push(Object::Dictionary(d0));

    let mut d1 = Dictionary::new();
    d1.set("Type", name("Font"));
    d1.set("Subtype", name("CIDFontType2"));
    d1.set("BaseFont", name(&base));
    let mut info = Dictionary::new();
    info.set("Registry", Object::string_literal("Adobe"));
    info.set("Ordering", Object::string_literal("Identity"));
    info.set("Supplement", Object::Integer(0));
    d1.set("CIDSystemInfo", Object::Dictionary(info));
    d1.set("FontDescriptor", local(descriptor));
    d1.set("W", Object::Array(w_arr));
    d1.set("CIDToGIDMap", local(cid_map));
    objects.push(Object::Dictionary(d1));

    let mut fd = Dictionary::new();
    fd.set("Type", name("FontDescriptor"));
    fd.set("FontName", name(&base));
    fd.set("Flags", Object::Integer(t.flags));
    fd.set("FontBBox", real_array(&t.bbox));
    fd.set("ItalicAngle", Object::Real(t.italic_angle));
    fd.set("Ascent", Object::Real(t.ascent));
    fd.set("Descent", Object::Real(t.descent));
    fd.set("CapHeight", Object::Real(t.cap_height));
    fd.set("XHeight", Object::Real(t.x_height));
    fd.set("StemV", Object::Real(t.stem_v));
    fd.set("FontWeight", Object::Real(t.weight));
    fd.set("FontFile2", local(font_file));
    // PDF-AS entfernt CIDSet nur bei PDF/A-3 (PDFAsTemplateCreator.removeCidSet)
    if pdfa != PdfaMode::Part3 {
        fd.set("CIDSet", local(cid_set_idx));
    }
    objects.push(Object::Dictionary(fd));

    let mut ff = Dictionary::new();
    ff.set("Length1", Object::Integer(subset.len() as i64));
    objects.push(flate_stream(ff, &subset)?);
    objects.push(flate_stream(Dictionary::new(), &to_unicode_cmap(&uni))?);
    objects.push(flate_stream(Dictionary::new(), &cid_to_gid)?);
    if pdfa != PdfaMode::Part3 {
        objects.push(flate_stream(Dictionary::new(), &cid_set)?);
    }
    Ok(type0)
}

// ------------------------------------------------------------------------------------------------
// Bilder (LosslessFactory: Flate + PNG-Predictor 15, Alpha als SMask)
// ------------------------------------------------------------------------------------------------

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = ((p - i16::from(a)).abs(), (p - i16::from(b)).abs(), (p - i16::from(c)).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// PNG-Prädiktor je Zeile (Filter mit kleinster Summe der Beträge, wie PDFBox `PredictorEncoder`).
fn predictor_encode(data: &[u8], width: usize, bpp: usize) -> Vec<u8> {
    let stride = width * bpp;
    let rows = data.len().checked_div(stride).unwrap_or(0);
    let mut out = Vec::with_capacity(data.len() + rows);
    let zero = vec![0u8; stride];
    let mut cand = vec![vec![0u8; stride]; 5];
    for r in 0..rows {
        let cur = &data[r * stride..(r + 1) * stride];
        let prev = if r == 0 { &zero[..] } else { &data[(r - 1) * stride..r * stride] };
        for i in 0..stride {
            let a = if i >= bpp { cur[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            cand[0][i] = cur[i];
            cand[1][i] = cur[i].wrapping_sub(a);
            cand[2][i] = cur[i].wrapping_sub(b);
            cand[3][i] = cur[i].wrapping_sub(((u16::from(a) + u16::from(b)) / 2) as u8);
            cand[4][i] = cur[i].wrapping_sub(paeth(a, b, c));
        }
        let score = |v: &[u8]| v.iter().map(|&x| u64::from((x as i8).unsigned_abs())).sum::<u64>();
        let best = (0..5).min_by_key(|&f| score(&cand[f])).unwrap_or(0);
        out.push(best as u8);
        out.extend_from_slice(&cand[best]);
    }
    out
}

fn image_dict(e: &Emblem, colors: i64, space: Object) -> Dictionary {
    let mut d = Dictionary::new();
    d.set("Type", name("XObject"));
    d.set("Subtype", name("Image"));
    d.set("Width", Object::Integer(i64::from(e.width)));
    d.set("Height", Object::Integer(i64::from(e.height)));
    d.set("BitsPerComponent", Object::Integer(8));
    d.set("ColorSpace", space);
    let mut parms = Dictionary::new();
    parms.set("Predictor", Object::Integer(15));
    parms.set("Colors", Object::Integer(colors));
    parms.set("BitsPerComponent", Object::Integer(8));
    parms.set("Columns", Object::Integer(i64::from(e.width)));
    d.set("DecodeParms", Object::Dictionary(parms));
    d
}

/// `[/ICCBased <sRGB>]` bzw. `/DeviceRGB`.
fn rgb_space(icc: Option<usize>) -> Object {
    match icc {
        Some(i) => Object::Array(vec![name("ICCBased"), local(i)]),
        None => name("DeviceRGB"),
    }
}

/// sRGB-Profilstream (`/N 3`, Alternate DeviceRGB).
fn push_icc(objects: &mut Vec<Object>) -> Result<usize> {
    let mut d = Dictionary::new();
    d.set("N", Object::Integer(3));
    d.set("Alternate", name("DeviceRGB"));
    objects.push(flate_stream(d, crate::icc::srgb_profile())?);
    Ok(objects.len() - 1)
}

fn push_image(objects: &mut Vec<Object>, emblem: &Emblem, pdfa: PdfaMode, icc: Option<usize>) -> Result<usize> {
    emblem.check()?;
    let flattened;
    let e = if emblem.has_alpha && pdfa == PdfaMode::Part1 {
        flattened = emblem.flattened_on_white();
        &flattened
    } else {
        emblem
    };
    let idx = objects.len();
    let mut d = image_dict(e, 3, rgb_space(icc));
    let rgb = predictor_encode(&e.rgb(), e.width as usize, 3);
    if e.has_alpha {
        d.set("SMask", local(idx + 1));
        objects.push(flate_stream(d, &rgb)?);
        let a = predictor_encode(&e.alpha(), e.width as usize, 1);
        // SMask: immer DeviceGray (ISO 32000-1 §11.6.5.3)
        objects.push(flate_stream(image_dict(e, 1, name("DeviceGray")), &a)?);
    } else {
        objects.push(flate_stream(d, &rgb)?);
    }
    Ok(idx)
}

/// `/Matrix` für eine Drehung in Grad (`AffineTransform.rotate`, exakt bei Vielfachen von 90°).
fn rotation_matrix(deg: i32) -> [f32; 6] {
    match deg.rem_euclid(360) {
        0 => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        90 => [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        180 => [-1.0, 0.0, 0.0, -1.0, 0.0, 0.0],
        270 => [0.0, -1.0, 1.0, 0.0, 0.0, 0.0],
        d => {
            let r = f64::from(d).to_radians();
            let (s, c) = (r.sin() as f32, r.cos() as f32);
            [c, s, -s, c, 0.0, 0.0]
        }
    }
}

impl Appearance {
    /// Baut das Erscheinungsbild eines sichtbaren Profils (unsichtbares Profil ⇒
    /// [`Appearance::invisible`]). PDF/A: `opts.pdfa`, ersatzweise `input.pdfa`.
    pub fn build(
        profile: &Profile,
        input: &SigBlockInput,
        opts: &BuildOptions,
        fonts: &FontProvider,
        resources: &dyn Resources,
    ) -> Result<Appearance> {
        if !profile.visible {
            return Ok(Appearance::invisible());
        }
        let pdfa = if opts.pdfa != PdfaMode::None { opts.pdfa } else { input.pdfa };
        let prepared = layout::prepare(profile, input, opts.width, pdfa, fonts, resources)?;
        // PDF/A: sRGB als ICCBased statt DeviceRGB (Plan §6.5); mit Quirks wie PDF-AS
        let icc_mode = pdfa != PdfaMode::None && !prepared.quirks;
        let mut cs = Content::new(prepared.quirks);
        cs.icc = icc_mode;
        let mut sink = ContentSink { cs, fonts: FontUse::default() };
        prepared.draw(&mut sink)?;
        if sink.cs.non_finite {
            return Err(SigBlockError::Pdf("Content-Stream enthielte eine nicht endliche Zahl".into()));
        }
        let lay = &prepared.layout;
        let content = std::mem::take(&mut sink.cs.out);

        let bbox = [lay.width + 1.0, lay.height + 1.0, 0.0, 0.0];
        let mut objects: Vec<Object> = vec![Object::Null, Object::Null];
        let icc = if icc_mode && (sink.cs.icc_used || !prepared.images.is_empty()) {
            Some(push_icc(&mut objects)?)
        } else {
            None
        };
        // Bilder (readTableResources-Reihenfolge: Im1, Im2 …)
        let mut xobjects = Dictionary::new();
        for (i, img) in prepared.images.iter().enumerate() {
            let idx = push_image(&mut objects, &img.emblem, pdfa, icc)?;
            xobjects.set(format!("Im{}", i + 1), local(idx));
        }
        // Fonts (Reihenfolge der ersten Verwendung: F1, F2 …)
        let mut font_dict = Dictionary::new();
        for (i, key) in sink.fonts.order.iter().enumerate() {
            let f = &sink.fonts.fonts[key];
            let idx = push_font(&mut objects, f, sink.fonts.glyphs.get(key), pdfa)?;
            font_dict.set(format!("F{}", i + 1), local(idx));
        }

        let mut inner_res = Dictionary::new();
        if !font_dict.is_empty() {
            inner_res.set("Font", Object::Dictionary(font_dict));
        }
        if !xobjects.is_empty() {
            inner_res.set("XObject", Object::Dictionary(xobjects));
        }
        if sink.cs.icc_used {
            let mut spaces = Dictionary::new();
            spaces.set(ICC_NAME, rgb_space(icc));
            inner_res.set("ColorSpace", Object::Dictionary(spaces));
        }
        inner_res.set("ProcSet", proc_set());
        let mut inner = Dictionary::new();
        inner.set("Type", name("XObject"));
        inner.set("Subtype", name("Form"));
        inner.set("BBox", real_array(&bbox));
        inner.set("FormType", Object::Integer(1));
        inner.set("Resources", Object::Dictionary(inner_res));
        objects[1] = plain_stream(inner, content);

        let mut holder_xobj = Dictionary::new();
        holder_xobj.set(INNER_NAME, local(1));
        let mut holder_res = Dictionary::new();
        holder_res.set("XObject", Object::Dictionary(holder_xobj));
        holder_res.set("ProcSet", proc_set());
        let mut holder = Dictionary::new();
        holder.set("Type", name("XObject"));
        holder.set("Subtype", name("Form"));
        holder.set("BBox", real_array(&bbox));
        holder.set("FormType", Object::Integer(1));
        holder.set("Matrix", real_array(&rotation_matrix(opts.rotation)));
        holder.set("Resources", Object::Dictionary(holder_res));
        let holder_content = if opts.preview_only {
            Vec::new()
        } else {
            format!("q 1.0 0.0 0.0 1.0 0.0 0.0 cm /{INNER_NAME} Do Q").into_bytes()
        };
        objects[0] = plain_stream(holder, holder_content);

        Ok(Appearance {
            width: lay.width,
            height: lay.height,
            tu_text: lay.tu_text.clone(),
            layout: Some(prepared.layout.clone()),
            objects,
            holder: 0,
            inner: 1,
            inner_name: INNER_NAME.to_string(),
        })
    }

    /// Unsichtbare Signatur: leeres Form-XObject mit `/BBox [0 0 0 0]` (docs/analyse/03 §4.6).
    pub fn invisible() -> Appearance {
        let mut d = Dictionary::new();
        d.set("Type", name("XObject"));
        d.set("Subtype", name("Form"));
        d.set("BBox", Object::Array(vec![Object::Integer(0); 4]));
        Appearance {
            width: 0.0,
            height: 0.0,
            layout: None,
            objects: vec![plain_stream(d, Vec::new())],
            holder: 0,
            inner: 0,
            inner_name: String::new(),
            tu_text: String::new(),
        }
    }

    /// Ersetzt lokale Referenzen; `alloc` liefert neue IDs in Objektreihenfolge.
    pub fn materialize(&self, alloc: &mut dyn FnMut() -> ObjectId) -> MaterializedAppearance {
        let ids: Vec<ObjectId> = self.objects.iter().map(|_| alloc()).collect();
        let objects = self.objects.iter().zip(&ids).map(|(o, id)| (*id, relink(o, &ids))).collect();
        MaterializedAppearance {
            holder_id: ids[self.holder],
            inner_id: ids[self.inner],
            inner_name: self.inner_name.clone(),
            objects,
        }
    }

    /// Eigenständiges Mini-PDF mit genau diesem Block (Blockvorschau über denselben Renderer,
    /// Plan §6.6); Seitengröße `W × H` (bei 90°/270° `H × W`). Die Seite zeigt das Inner-Form mit
    /// derselben Abbildung `BBox → Rect`, die ein Viewer auf das Widget anwendet (inklusive der
    /// Stauchung durch die `+1`-BBox, Q6).
    pub fn to_standalone_pdf(&self) -> Result<Vec<u8>> {
        if self.inner_name.is_empty() || self.width <= 0.0 || self.height <= 0.0 {
            return Err(SigBlockError::Pdf("unsichtbare Signatur hat kein Erscheinungsbild".into()));
        }
        let mut doc = lopdf::Document::with_version("1.7");
        let mut next = 1u32;
        let mat = self.materialize(&mut || {
            let id = (next, 0);
            next += 1;
            id
        });
        doc.max_id = next - 1;
        let holder_matrix = match &self.objects[self.holder] {
            Object::Stream(s) => s.dict.get(b"Matrix").ok().and_then(|m| m.as_array().ok()).map(|a| {
                let mut m = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
                for (i, v) in a.iter().take(6).enumerate() {
                    m[i] = v.as_float().unwrap_or(m[i]);
                }
                m
            }),
            _ => None,
        }
        .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        for (id, obj) in mat.objects {
            doc.objects.insert(id, obj);
        }
        // transformierte BBox [0 0 W+1 H+1]
        let (bw, bh) = (self.width + 1.0, self.height + 1.0);
        let m = holder_matrix;
        let pts = [(0.0f32, 0.0f32), (bw, 0.0), (0.0, bh), (bw, bh)];
        let tp: Vec<(f32, f32)> =
            pts.iter().map(|&(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])).collect();
        let x0 = tp.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let x1 = tp.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let y0 = tp.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let y1 = tp.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        let sideways = (m[0].abs() < 0.5) && (m[1].abs() > 0.5);
        let (pw, ph) = if sideways { (self.height, self.width) } else { (self.width, self.height) };
        let sx = pw / (x1 - x0);
        let sy = ph / (y1 - y0);
        let content = format!(
            "q {} 0 0 {} {} {} cm {} {} {} {} {} {} cm /{} Do Q\n",
            pdf_number(sx),
            pdf_number(sy),
            pdf_number(-x0 * sx),
            pdf_number(-y0 * sy),
            pdf_number(m[0]),
            pdf_number(m[1]),
            pdf_number(m[2]),
            pdf_number(m[3]),
            pdf_number(m[4]),
            pdf_number(m[5]),
            self.inner_name
        );
        let content_id = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content.into_bytes())));
        let mut xobj = Dictionary::new();
        xobj.set(self.inner_name.as_str(), Object::Reference(mat.inner_id));
        let mut res = Dictionary::new();
        res.set("XObject", Object::Dictionary(xobj));
        res.set("ProcSet", proc_set());
        let pages_id = doc.new_object_id();
        let mut page = Dictionary::new();
        page.set("Type", name("Page"));
        page.set("Parent", Object::Reference(pages_id));
        page.set("MediaBox", Object::Array(vec![0.into(), 0.into(), Object::Real(pw), Object::Real(ph)]));
        page.set("Resources", Object::Dictionary(res));
        page.set("Contents", Object::Reference(content_id));
        let page_id = doc.add_object(Object::Dictionary(page));
        let mut pages = Dictionary::new();
        pages.set("Type", name("Pages"));
        pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", name("Catalog"));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(Object::Dictionary(catalog));
        doc.trailer.set("Root", Object::Reference(catalog_id));
        let mut out = Vec::new();
        doc.save_to(&mut out).map_err(|e| SigBlockError::Pdf(e.to_string()))?;
        Ok(out)
    }
}

fn relink(o: &Object, ids: &[ObjectId]) -> Object {
    match o {
        Object::Reference((i, g)) if *g == LOCAL_GENERATION => {
            ids.get(*i as usize).map(|id| Object::Reference(*id)).unwrap_or(Object::Null)
        }
        Object::Array(a) => Object::Array(a.iter().map(|x| relink(x, ids)).collect()),
        Object::Dictionary(d) => Object::Dictionary(relink_dict(d, ids)),
        Object::Stream(s) => {
            let mut s2 = s.clone();
            s2.dict = relink_dict(&s.dict, ids);
            Object::Stream(s2)
        }
        other => other.clone(),
    }
}

fn relink_dict(d: &Dictionary, ids: &[ObjectId]) -> Dictionary {
    let mut out = Dictionary::new();
    for (k, v) in d.iter() {
        out.set(k.clone(), relink(v, ids));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_encoding_like_cos_writer() {
        let mut c = Content::new(true);
        c.string(b"a(b)\\");
        c.string(&[0, 0x36]);
        c.string(&[0xfc]);
        c.string(b"a\nb");
        assert_eq!(c.out, b"(a\\(b\\)\\\\) (\x00\x36) <FC> <610A62> ".to_vec());
    }

    #[test]
    fn colors() {
        let mut q = Content::new(true);
        q.fill_color([255, 255, 255]);
        q.fill_color([0, 0, 0]);
        assert_eq!(String::from_utf8(q.out).unwrap(), "/DeviceRGB cs\n1 1 1 sc\n0 0 0 sc\n");
        let mut n = Content::new(false);
        n.fill_color([255, 255, 255]);
        n.fill_color([255, 0, 0]);
        n.fill_color([0, 0, 0]);
        assert_eq!(String::from_utf8(n.out).unwrap(), "1 g\n1 0 0 rg\n0 g\n");
        // PDF/A: farbige Füllungen im sRGB-ICCBased-Raum, nach `g` erneut `cs`
        let mut p = Content::new(false);
        p.icc = true;
        p.fill_color([255, 0, 0]);
        p.fill_color([0, 0, 255]);
        p.fill_color([0, 0, 0]);
        p.fill_color([0, 255, 0]);
        assert!(p.icc_used);
        assert_eq!(String::from_utf8(p.out).unwrap(), "/CS1 cs\n1 0 0 sc\n0 0 1 sc\n0 g\n/CS1 cs\n0 1 0 sc\n");
    }

    #[test]
    fn rotation() {
        assert_eq!(rotation_matrix(0), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(rotation_matrix(450), [0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
        assert_eq!(rotation_matrix(-90), [0.0, -1.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(rotation_matrix(180), [-1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    }

    #[test]
    fn predictor_roundtrip() {
        let data: Vec<u8> = (0..60u8).collect();
        let enc = predictor_encode(&data, 5, 3);
        assert_eq!(enc.len(), data.len() + 4);
        // dekodieren
        let stride = 15;
        let mut out = vec![0u8; data.len()];
        for r in 0..4 {
            let f = enc[r * (stride + 1)];
            for i in 0..stride {
                let x = enc[r * (stride + 1) + 1 + i];
                let a = if i >= 3 { out[r * stride + i - 3] } else { 0 };
                let b = if r > 0 { out[(r - 1) * stride + i] } else { 0 };
                let c = if r > 0 && i >= 3 { out[(r - 1) * stride + i - 3] } else { 0 };
                out[r * stride + i] = match f {
                    0 => x,
                    1 => x.wrapping_add(a),
                    2 => x.wrapping_add(b),
                    3 => x.wrapping_add(((u16::from(a) + u16::from(b)) / 2) as u8),
                    _ => x.wrapping_add(paeth(a, b, c)),
                };
            }
        }
        assert_eq!(out, data);
    }

    #[test]
    fn tounicode_ranges() {
        let mut m = BTreeMap::new();
        m.insert(3u16, ' ');
        m.insert(0x36, 'S');
        m.insert(0x37, 'T');
        m.insert(0xBE, 'ü');
        let s = String::from_utf8(to_unicode_cmap(&m)).unwrap();
        assert!(
            s.contains("3 beginbfrange\n<0003> <0003> <0020>\n<0036> <0037> <0053>\n<00BE> <00BE> <00FC>\nendbfrange")
        );
    }

    #[test]
    fn invisible_is_empty_form() {
        let a = Appearance::invisible();
        assert_eq!(a.objects.len(), 1);
        assert!(a.inner_name.is_empty());
        assert!(a.to_standalone_pdf().is_err());
        let mut n = 10;
        let m = a.materialize(&mut || {
            n += 1;
            (n, 0)
        });
        assert_eq!(m.holder_id, (11, 0));
        assert_eq!(m.inner_id, (11, 0));
    }

    #[test]
    fn tag_is_six_letters() {
        let t = subset_tag(&[0u16, 3, 54].into_iter().collect());
        assert_eq!(t.len(), 7);
        assert!(t.ends_with('+'));
        assert!(t[..6].bytes().all(|b| b.is_ascii_uppercase()));
    }
}
