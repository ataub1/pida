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
use crate::emblem::{Emblem, SourceColor, java_linear_gray_to_srgb};
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
/// Ressourcenname des Holder-Forms im Mini-PDF ([`Appearance::to_standalone_pdf`]).
const HOLDER_NAME: &str = "FRM0";

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

/// PDF/A-1 ohne Alpha: Standard = Komposition auf Weiß (Register); mit `pdfas_quirks` wie PDF-AS
/// (`removeAlphaChannel`, Golden PDF-AS 4.4.2): der Alphakanal entfällt, die gespeicherten
/// RGB-Werte bleiben – voll transparente Pixel zeigen ihre gespeicherte Farbe (meist Schwarz).
fn without_alpha(emblem: &Emblem, quirks: bool) -> Emblem {
    if quirks {
        let rgba = emblem.rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        Emblem { rgba, has_alpha: false, ..emblem.clone() }
    } else {
        emblem.flattened_on_white()
    }
}

/// Einbettung einer Bildmarke außerhalb von PDF/A mit `pdfas_quirks` wie PDF-AS 4.4.2 (Register
/// SB-G2 … SB-G4; Farbtyp aus [`Emblem::source_color`]).
///
/// PDF-AS liest die Datei mit `ImageIO.read`, verwirft eine Transparenz ohne Alpha-Raster
/// (`removeAlphaChannel`) und bettet mit PDFBox `LosslessFactory.createFromImage` ein. Gegen
/// genau diese Bibliotheken (PDF-AS 4.4.2 mit PDFBox 2.0.34, Java 21) geprüft:
///
/// | Quelle | PDF-AS | hier |
/// |---|---|---|
/// | RGB, Palette ohne `tRNS` | DeviceRGB | gleich |
/// | RGBA (auch voll deckend), RGB mit `tRNS` | DeviceRGB + SMask (SB-G2) | gleich |
/// | Palette mit `tRNS`, GIF mit Transparenz | Alpha verworfen, Palettenfarbe (SB-G3) | gleich |
/// | Graustufen 8 Bit | DeviceGray, `getRGB` linear → sRGB (SB-G4) | gleich |
/// | Graustufen 16 Bit | DeviceRGB, aufgehellt | DeviceGray, aufgehellt (gleiches Bild) |
/// | Graustufen mit `tRNS`, Graustufen + Alpha (auch voll deckend) | ICCBased (lineares `CS_GRAY`) + SMask | DeviceGray aufgehellt + SMask (gleiches Bild, ohne das Java-ICC-Profil) |
/// | Graustufen 1/2/4 Bit (Java: Palettenbild) | DeviceRGB, nicht aufgehellt | wie Palette |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuirkImage {
    /// wie im Standardmodus
    AsIs,
    /// SB-G2: Alphakanal der Datei voll deckend ⇒ trotzdem eine SMask (lauter 255)
    OpaqueSmask,
    /// SB-G3: Palettentransparenz verworfen (gespeicherte Palettenfarbe, keine SMask)
    DropAlpha,
    /// SB-G4: DeviceGray, Samples linear → sRGB aufgehellt; `smask`: mit SMask aus dem Alpha
    /// (Graustufen mit Alphakanal bzw. `tRNS`, auch voll deckend)
    BrightenedGray { smask: bool },
}

fn quirk_image(emblem: &Emblem, pdfa: PdfaMode, quirks: bool) -> QuirkImage {
    if !quirks || pdfa != PdfaMode::None {
        return QuirkImage::AsIs;
    }
    match emblem.source_color {
        SourceColor::RgbAlpha if !emblem.has_alpha => QuirkImage::OpaqueSmask,
        SourceColor::Indexed if emblem.has_alpha => QuirkImage::DropAlpha,
        SourceColor::Gray => QuirkImage::BrightenedGray { smask: emblem.has_alpha },
        SourceColor::GrayAlpha => QuirkImage::BrightenedGray { smask: true },
        _ => QuirkImage::AsIs,
    }
}

fn push_image(
    objects: &mut Vec<Object>,
    emblem: &Emblem,
    pdfa: PdfaMode,
    icc: Option<usize>,
    quirks: bool,
) -> Result<usize> {
    emblem.check()?;
    let quirk = quirk_image(emblem, pdfa, quirks);
    let flattened;
    let e = if emblem.has_alpha && (pdfa == PdfaMode::Part1 || quirk == QuirkImage::DropAlpha) {
        flattened = without_alpha(emblem, quirks);
        &flattened
    } else {
        emblem
    };
    let idx = objects.len();
    let alpha_mask = |objects: &mut Vec<Object>| -> Result<()> {
        let a = predictor_encode(&e.alpha(), e.width as usize, 1);
        // SMask: immer DeviceGray (ISO 32000-1 §11.6.5.3)
        objects.push(flate_stream(image_dict(e, 1, name("DeviceGray")), &a)?);
        Ok(())
    };
    if let QuirkImage::BrightenedGray { smask } = quirk {
        // SB-G4: PDFBox `createFromGrayImage` liest `getRGB` des linearen CS_GRAY (aufgehellt)
        let gray: Vec<u8> = e.rgba.as_chunks::<4>().0.iter().map(|p| java_linear_gray_to_srgb(p[0])).collect();
        let mut d = image_dict(e, 1, name("DeviceGray"));
        if smask {
            d.set("SMask", local(idx + 1));
        }
        objects.push(flate_stream(d, &predictor_encode(&gray, e.width as usize, 1))?);
        if smask {
            alpha_mask(objects)?;
        }
        return Ok(idx);
    }
    let mut d = image_dict(e, 3, rgb_space(icc));
    let rgb = predictor_encode(&e.rgb(), e.width as usize, 3);
    if e.has_alpha || quirk == QuirkImage::OpaqueSmask {
        d.set("SMask", local(idx + 1));
        objects.push(flate_stream(d, &rgb)?);
        alpha_mask(objects)?;
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
            let idx = push_image(&mut objects, &img.emblem, pdfa, icc, prepared.quirks)?;
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
    /// Plan §6.6); Seitengröße `W × H` (bei 90°/270° `H × W`). Die Seite zeigt das Holder-Form
    /// (bzw. bei `preview_only` das Inner-Form mit der Holder-Matrix) mit derselben Abbildung
    /// `BBox → Rect`, die ein Viewer auf das Widget anwendet (inklusive der Stauchung durch die
    /// `+1`-BBox, Q6) – mit PDFium pixelgleich zum Widget in der PDF-AS-Ausgabe
    /// (`tests/golden_pdfas442.rs`).
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
        // Wie im Viewer über das Holder-Form (Seite → Holder mit `/Matrix` → Inner-Form): PDFium
        // rastert Bilder eine Formularebene tiefer mit anderer Rundung, nur so ist die Vorschau
        // pixelgleich zum signierten Widget (Golden PDF-AS 4.4.2). Ein Holder ohne Inhalt
        // (`preview_only`) wird übersprungen: Inner-Form direkt mit der Holder-Matrix.
        let via_holder = matches!(&self.objects[self.holder], Object::Stream(s) if !s.content.is_empty());
        let (xname, target) =
            if via_holder { (HOLDER_NAME, mat.holder_id) } else { (self.inner_name.as_str(), mat.inner_id) };
        // Abbildung mit voller f32-Genauigkeit (kürzeste Darstellung, nie Exponent): fünf
        // Nachkommastellen (pdf_number) verschieben die Bildrasterung um Bruchteile eines Pixels.
        let exact = |v: f32| if v.is_finite() { format!("{v}") } else { "0".to_string() };
        let mut content = format!("q {} 0 0 {} {} {} cm ", exact(sx), exact(sy), exact(-x0 * sx), exact(-y0 * sy));
        if !via_holder {
            content.push_str(&format!(
                "{} {} {} {} {} {} cm ",
                pdf_number(m[0]),
                pdf_number(m[1]),
                pdf_number(m[2]),
                pdf_number(m[3]),
                pdf_number(m[4]),
                pdf_number(m[5])
            ));
        }
        content.push_str(&format!("/{xname} Do Q\n"));
        let content_id = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content.into_bytes())));
        let mut xobj = Dictionary::new();
        xobj.set(xname, Object::Reference(target));
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

    /// PDF/A-1 ohne Alpha (SB-G6): Standard auf Weiß komponiert, mit Quirks wie PDF-AS
    /// (`removeAlphaChannel` behält die gespeicherten RGB-Werte, Golden PDF-AS 4.4.2).
    #[test]
    fn pdfa1_alpha_removal_default_and_quirk() {
        let e = Emblem {
            width: 2,
            height: 1,
            rgba: vec![0, 0, 0, 0, 200, 10, 20, 255],
            has_alpha: true,
            source_sha256: [0; 32],
            source_color: SourceColor::RgbAlpha,
        };
        let white = without_alpha(&e, false);
        assert_eq!(white.rgba, vec![255, 255, 255, 255, 200, 10, 20, 255]);
        assert!(!white.has_alpha);
        let quirk = without_alpha(&e, true);
        assert_eq!(quirk.rgba, vec![0, 0, 0, 255, 200, 10, 20, 255]);
        assert!(!quirk.has_alpha);
        // eingebettet: kein SMask, RGB-Samples wie oben
        for (quirks, rgb) in [(false, [255u8, 255, 255, 200, 10, 20]), (true, [0, 0, 0, 200, 10, 20])] {
            let mut objects = Vec::new();
            let idx = push_image(&mut objects, &e, PdfaMode::Part1, None, quirks).unwrap();
            assert_eq!(objects.len(), 1, "kein SMask");
            let Object::Stream(s) = &objects[idx] else { panic!("Bildstream") };
            assert!(s.dict.get(b"SMask").is_err());
            assert_eq!(s.decompressed_content().unwrap(), rgb.to_vec());
        }
    }

    /// Mit `pdfas_quirks` (außerhalb von PDF/A) wie PDF-AS: SMask für jeden Alphakanal (SB-G2),
    /// Palettentransparenz verworfen (SB-G3), Graustufen als DeviceGray aufgehellt (SB-G4); ohne
    /// Quirks nach Inhalt.
    #[test]
    fn quirk_image_embedding_by_source_color() {
        let emblem = |rgba: Vec<u8>, has_alpha, source_color| Emblem {
            width: 2,
            height: 1,
            rgba,
            has_alpha,
            source_sha256: [0; 32],
            source_color,
        };
        let embed = |e: &Emblem, quirks| {
            let mut objects = Vec::new();
            let idx = push_image(&mut objects, e, PdfaMode::None, None, quirks).unwrap();
            let Object::Stream(s) = &objects[idx] else { panic!("Bildstream") };
            let cs = s.dict.get(b"ColorSpace").unwrap().as_name().unwrap().to_vec();
            let samples = s.decompressed_content().unwrap();
            let smask = (objects.len() > 1).then(|| match &objects[idx + 1] {
                Object::Stream(m) => m.decompressed_content().unwrap(),
                _ => panic!("SMask"),
            });
            (cs, samples, smask)
        };
        // SB-G2: voll deckender Alphakanal
        let opaque = emblem(vec![1, 2, 3, 255, 4, 5, 6, 255], false, SourceColor::RgbAlpha);
        assert_eq!(embed(&opaque, false).2, None);
        assert_eq!(embed(&opaque, true).2, Some(vec![255, 255]));
        // SB-G3: Palette mit Transparenz
        let indexed = emblem(vec![255, 0, 0, 0, 0, 0, 255, 255], true, SourceColor::Indexed);
        assert_eq!(embed(&indexed, false).2, Some(vec![0, 255]));
        let (cs, samples, smask) = embed(&indexed, true);
        assert_eq!((cs.as_slice(), samples, smask), (&b"DeviceRGB"[..], vec![255, 0, 0, 0, 0, 255], None));
        // SB-G4: Graustufen
        let gray = emblem(vec![20, 20, 20, 255, 200, 200, 200, 255], false, SourceColor::Gray);
        let (cs, samples, _) = embed(&gray, false);
        assert_eq!((cs.as_slice(), samples), (&b"DeviceRGB"[..], vec![20, 20, 20, 200, 200, 200]));
        let (cs, samples, smask) = embed(&gray, true);
        assert_eq!(
            (cs.as_slice(), samples, smask),
            (&b"DeviceGray"[..], vec![79, java_linear_gray_to_srgb(200)], None)
        );
        // Graustufen mit tRNS bzw. Alphakanal (PDF-AS: ICCBased-Grau + SMask, auch voll deckend):
        // Transparenz bleibt, Grau aufgehellt (PDFBox 2.0.34 / Java 21 geprüft: 20 → 0x4f)
        let gray_trns = emblem(vec![20, 20, 20, 0, 200, 200, 200, 255], true, SourceColor::Gray);
        let (cs, samples, smask) = embed(&gray_trns, true);
        assert_eq!((cs.as_slice(), samples, smask), (&b"DeviceGray"[..], vec![79, 229], Some(vec![0, 255])));
        let gray_alpha_opaque = emblem(vec![20, 20, 20, 255, 200, 200, 200, 255], false, SourceColor::GrayAlpha);
        let (cs, samples, smask) = embed(&gray_alpha_opaque, true);
        assert_eq!((cs.as_slice(), samples, smask), (&b"DeviceGray"[..], vec![79, 229], Some(vec![255, 255])));
        assert_eq!(embed(&gray_alpha_opaque, false).2, None, "Standardmodus: keine SMask ohne Transparenz");
        // echte Transparenz bleibt in beiden Modi
        let alpha = emblem(vec![1, 2, 3, 0, 4, 5, 6, 255], true, SourceColor::RgbAlpha);
        assert_eq!(embed(&alpha, true).2, Some(vec![0, 255]));
        // PDF/A: unverändert (Part 1 ohne Alpha, andere wie bisher)
        let mut objects = Vec::new();
        push_image(&mut objects, &gray, PdfaMode::Part1, None, true).unwrap();
        let Object::Stream(s) = &objects[0] else { panic!() };
        assert_eq!(s.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceRGB");
    }

    /// PNG-/GIF-Bildmarken aller Farbtypen für das PDF-AS-Orakel (Name, Datei).
    fn oracle_fixtures() -> Vec<(&'static str, Vec<u8>)> {
        let png = |w: u32, color, depth, palette: Option<Vec<u8>>, trns: Option<Vec<u8>>, data: &[u8]| {
            let mut out = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut out, w, 1);
                enc.set_color(color);
                enc.set_depth(depth);
                if let Some(p) = palette {
                    enc.set_palette(p);
                }
                if let Some(t) = trns {
                    enc.set_trns(t);
                }
                let mut wr = enc.write_header().unwrap();
                wr.write_image_data(data).unwrap();
            }
            out
        };
        use png::{BitDepth as B, ColorType as C};
        // GIF mit `colors` Paletteneinträgen (Java: ≤ 16 gepackt), Index 0 transparent
        let gif = |colors: usize| {
            let mut palette = vec![255u8, 0, 0, 0, 0, 255, 20, 20, 20];
            palette.resize(colors * 3, 7);
            let mut out = Vec::new();
            {
                let mut enc = gif::Encoder::new(&mut out, 3, 1, &palette).unwrap();
                let mut frame =
                    gif::Frame { width: 3, height: 1, buffer: vec![0u8, 1, 2].into(), ..Default::default() };
                frame.transparent = Some(0);
                enc.write_frame(&frame).unwrap();
            }
            out
        };
        vec![
            ("rgb.png", png(2, C::Rgb, B::Eight, None, None, &[1, 2, 3, 200, 100, 50])),
            ("rgba.png", png(2, C::Rgba, B::Eight, None, None, &[1, 2, 3, 0, 200, 100, 50, 128])),
            ("rgba-opaque.png", png(2, C::Rgba, B::Eight, None, None, &[1, 2, 3, 255, 200, 100, 50, 255])),
            ("rgb-trns.png", png(2, C::Rgb, B::Eight, None, Some(vec![0, 1, 0, 2, 0, 3]), &[1, 2, 3, 200, 100, 50])),
            ("gray8.png", png(3, C::Grayscale, B::Eight, None, None, &[0, 20, 200])),
            ("gray8-trns.png", png(3, C::Grayscale, B::Eight, None, Some(vec![0, 20]), &[0, 20, 200])),
            ("gray16.png", png(2, C::Grayscale, B::Sixteen, None, None, &[20, 0, 200, 0])),
            ("gray-alpha.png", png(2, C::GrayscaleAlpha, B::Eight, None, None, &[20, 0, 200, 128])),
            ("gray-alpha-opaque.png", png(2, C::GrayscaleAlpha, B::Eight, None, None, &[20, 255, 200, 255])),
            ("gray1.png", png(8, C::Grayscale, B::One, None, None, &[0b1010_0000])),
            ("gray2.png", png(4, C::Grayscale, B::Two, None, None, &[0b0001_1011])),
            ("gray4.png", png(2, C::Grayscale, B::Four, None, None, &[0x1e])),
            ("indexed.png", png(2, C::Indexed, B::Eight, Some(vec![255, 0, 0, 20, 20, 20]), None, &[0, 1])),
            (
                "indexed-trns.png",
                png(2, C::Indexed, B::Eight, Some(vec![255, 0, 0, 0, 0, 255]), Some(vec![0]), &[0, 1]),
            ),
            (
                "indexed2-trns.png",
                png(
                    3,
                    C::Indexed,
                    B::Two,
                    Some(vec![200, 100, 50, 0, 0, 255, 9, 9, 9]),
                    Some(vec![0, 255, 255]),
                    &[0b0001_1000],
                ),
            ),
            ("indexed.gif", gif(3)),
            ("indexed8.gif", gif(17)),
        ]
    }

    /// Ein eingebettetes Bild als sichtbare sRGB-Pixel (Farbraum wie ihn ein Viewer zeigt) und SMask.
    fn shown(
        cs: &str,
        bpc: u32,
        w: usize,
        h: usize,
        samples: &[u8],
        smask: Option<Vec<u8>>,
    ) -> (Vec<[u8; 3]>, Option<Vec<u8>>) {
        let n = w * h;
        let px: Vec<[u8; 3]> = match (cs, bpc) {
            ("DeviceRGB", 8) => samples.as_chunks::<3>().0.to_vec(),
            ("DeviceGray", 8) => samples.iter().map(|&v| [v, v, v]).collect(),
            ("DeviceGray", 1) => {
                (0..n).map(|i| if samples[i / 8] >> (7 - i % 8) & 1 == 1 { [255; 3] } else { [0; 3] }).collect()
            }
            // ICCBased mit dem linearen Java-`CS_GRAY`: sichtbar wie `getRGB` (aufgehellt)
            ("ICCBased1", 8) => samples.iter().map(|&v| [java_linear_gray_to_srgb(v); 3]).collect(),
            ("ICCBased1", 16) => samples.chunks(2).map(|v| [java_linear_gray_to_srgb(v[0]); 3]).collect(),
            other => panic!("Farbraum {other:?} im Orakel nicht vorgesehen"),
        };
        // 16-Bit-SMask auf 8 Bit (höherwertiges Byte)
        let smask = smask.map(|m| if m.len() == 2 * n { m.chunks(2).map(|v| v[0]).collect() } else { m });
        (px, smask)
    }

    /// PDF-AS-Orakel (nur mit `PDFOVER_PDFAS_LIB` = Ordner mit den Bibliotheken von PDF-AS 4.4.2,
    /// u. a. `pdfbox-2.0.*.jar` und `pdf-as-common-4.4.2.jar`, und Java ≥ 11 in `PDFOVER_JAVA` bzw.
    /// `java`): jede Bildmarke wird wie bei PDF-AS eingebettet (`testdata/golden/emblem-oracle/
    /// PdfAsEmblem.java`) und mit `pdfas_quirks` hier. Erwartet: gleiche sichtbare Pixel und gleiche
    /// SMask; Farbraum und Samples exakt außer bei den in `QuirkImage` genannten Fällen (ICCBased
    /// des Java-`CS_GRAY` bzw. 16-Bit-Grau als DeviceRGB ⇒ hier DeviceGray aufgehellt; 1-Bit-Grau).
    #[test]
    fn pdfas_lossless_factory_oracle() {
        let Some(lib) = std::env::var_os("PDFOVER_PDFAS_LIB").map(std::path::PathBuf::from) else {
            eprintln!("PDFOVER_PDFAS_LIB nicht gesetzt – PDF-AS-Bildmarken-Orakel übersprungen");
            return;
        };
        let java = std::env::var_os("PDFOVER_JAVA").unwrap_or_else(|| "java".into());
        let dir = std::env::temp_dir().join(format!("pdfover-emblem-oracle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fixtures = oracle_fixtures();
        let mut files = Vec::new();
        for (name, bytes) in &fixtures {
            let p = dir.join(name);
            std::fs::write(&p, bytes).unwrap();
            files.push(p);
        }
        let program = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/golden/emblem-oracle/PdfAsEmblem.java");
        let out = std::process::Command::new(&java)
            .arg("-cp")
            .arg(format!("{}/*", lib.display()))
            .arg(&program)
            .args(&files)
            .output()
            .expect("Java startet");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "Orakel: {}", String::from_utf8_lossy(&out.stderr));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let unhex = |h: &str| -> Option<Vec<u8>> {
            (h != "-").then(|| (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect())
        };
        let mut checked = 0;
        for line in stdout.lines().filter(|l| l.contains('|')) {
            let f: Vec<&str> = line.split('|').collect();
            let (name, mut cs, n, bpc, w, h) = (f[0], f[1].to_owned(), f[2], f[3], f[4], f[5]);
            if cs == "ICCBased" {
                cs = format!("ICCBased{n}");
            }
            let (bpc, w, h): (u32, usize, usize) = (bpc.parse().unwrap(), w.parse().unwrap(), h.parse().unwrap());
            let (want_px, want_mask) = shown(&cs, bpc, w, h, &unhex(f[6]).unwrap(), unhex(f[7]));
            let bytes = &fixtures.iter().find(|(n, _)| *n == name).unwrap().1;
            let emblem = Emblem::decode(bytes).unwrap();
            let mut objects = Vec::new();
            let idx = push_image(&mut objects, &emblem, PdfaMode::None, None, true).unwrap();
            let Object::Stream(st) = &objects[idx] else { panic!("Bildstream") };
            let our_cs = String::from_utf8(st.dict.get(b"ColorSpace").unwrap().as_name().unwrap().to_vec()).unwrap();
            let our_bpc = st.dict.get(b"BitsPerComponent").unwrap().as_i64().unwrap() as u32;
            let our_mask = st.dict.has(b"SMask").then(|| match &objects[idx + 1] {
                Object::Stream(m) => m.decompressed_content().unwrap(),
                _ => panic!("SMask"),
            });
            let (got_px, got_mask) = shown(&our_cs, our_bpc, w, h, &st.decompressed_content().unwrap(), our_mask);
            let close = got_px.iter().zip(&want_px).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 1));
            assert!(
                close && got_px.len() == want_px.len(),
                "{name}: Pixel {got_px:?} vs PDF-AS {want_px:?} ({our_cs} vs {cs})"
            );
            assert_eq!(got_mask, want_mask, "{name}: SMask ({our_cs} vs {cs})");
            let exact_expected = !cs.starts_with("ICCBased") && name != "gray16.png" && name != "gray1.png";
            if exact_expected {
                assert_eq!(our_cs, cs, "{name}: Farbraum");
            }
            checked += 1;
        }
        assert_eq!(checked, fixtures.len(), "Orakel-Ausgabe unvollständig:\n{stdout}");
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
