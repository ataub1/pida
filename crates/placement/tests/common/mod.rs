//! Test-Hilfen für `pdfover-placement`.
//!
//! - Mini-COS über `lopdf` (Re-Export aus `pdfover-pdf-cos`): `PageInfo`, `ImageInvocation`,
//!   `SigField`, `/Location` – nur für Tests, weil `pdf-cos` (Paket A) parallel entsteht
//!   (ADR 0001 §5.3: Tests bauen die Datenstrukturen selbst).
//! - Minimaler PDF-Schreiber, QR-Bilder mit dem `rxing`-Encoder.
//! - PDFium über `PDFOVER_PDFIUM_DIR` (sonst werden PDFium-Tests übersprungen).
//! - Erkennung umrandeter Signaturblöcke in den Referenzbildern der PDF-AS-Testsuiten.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdfover_pdf_cos::lopdf::content::Content;
use pdfover_pdf_cos::lopdf::{Dictionary, Document, Object, ObjectId};
use pdfover_pdf_cos::{ImageInvocation, PageInfo, Rect, Rotation, SigField, SigWidget};
use pdfover_render::{ENV_PDFIUM_DIR, PdfiumBackend, RenderBackend, Renderer, RgbaImage, concat_matrix};

// ------------------------------------------------------------------------------------------------
// Pfade, PDFium
// ------------------------------------------------------------------------------------------------

pub fn testdata(rel: &str) -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel)
}

pub fn read(rel: &str) -> Vec<u8> {
    std::fs::read(testdata(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

pub fn backend() -> Option<PdfiumBackend> {
    let dir = std::env::var_os(ENV_PDFIUM_DIR)?;
    if dir.is_empty() {
        return None;
    }
    Some(PdfiumBackend::bind(Some(Path::new(&dir))).expect("PDFium aus PDFOVER_PDFIUM_DIR bindbar"))
}

#[macro_export]
macro_rules! require_pdfium {
    () => {
        match common::backend() {
            Some(b) => b,
            None => {
                eprintln!("PDFOVER_PDFIUM_DIR nicht gesetzt – Test übersprungen");
                return;
            }
        }
    };
}

pub fn open(backend: &PdfiumBackend, bytes: &[u8]) -> Box<dyn Renderer> {
    backend.open(Arc::from(bytes), None).expect("PDF mit PDFium lesbar")
}

// ------------------------------------------------------------------------------------------------
// Mini-COS über lopdf
// ------------------------------------------------------------------------------------------------

pub fn load(bytes: &[u8]) -> Document {
    Document::load_mem(bytes).expect("PDF mit lopdf lesbar")
}

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    doc.dereference(obj).map(|(_, o)| o).unwrap_or(obj)
}

fn inherited<'a>(doc: &'a Document, page: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut id = page;
    for _ in 0..64 {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(deref(doc, v));
        }
        id = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn num(doc: &Document, o: &Object) -> f32 {
    deref(doc, o).as_float().unwrap_or(0.0)
}

fn rect(doc: &Document, o: &Object) -> Option<Rect> {
    let a = deref(doc, o).as_array().ok()?;
    if a.len() != 4 {
        return None;
    }
    Some(Rect::new(num(doc, &a[0]), num(doc, &a[1]), num(doc, &a[2]), num(doc, &a[3])))
}

/// Seiten mit geerbter MediaBox/CropBox/Rotate.
pub fn pages(doc: &Document) -> Vec<PageInfo> {
    doc.get_pages()
        .into_values()
        .enumerate()
        .map(|(index, id)| {
            let media_box =
                inherited(doc, id, b"MediaBox").and_then(|o| rect(doc, o)).unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
            let crop_box = inherited(doc, id, b"CropBox").and_then(|o| rect(doc, o)).unwrap_or(media_box);
            let rotation = inherited(doc, id, b"Rotate")
                .and_then(|o| o.as_i64().ok())
                .and_then(Rotation::from_degrees)
                .unwrap_or_default();
            PageInfo { index, object_id: id, media_box, crop_box, rotation, user_unit: 1.0, annots_indirect: None }
        })
        .collect()
}

fn matrix(doc: &Document, o: &Object) -> Option<[f32; 6]> {
    let a = deref(doc, o).as_array().ok()?;
    if a.len() != 6 {
        return None;
    }
    let mut m = [0.0; 6];
    for (i, v) in a.iter().enumerate() {
        m[i] = num(doc, v);
    }
    Some(m)
}

fn stream_content(doc: &Document, id: ObjectId) -> Option<(Dictionary, Vec<u8>)> {
    let s = doc.get_object(id).ok()?.as_stream().ok()?;
    let data = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
    Some((s.dict.clone(), data))
}

#[allow(clippy::too_many_arguments)]
fn walk(
    doc: &Document,
    page_index: usize,
    content: &[u8],
    resources: Option<&Dictionary>,
    base: [f32; 6],
    inside_form: bool,
    descend_forms: bool,
    out: &mut Vec<ImageInvocation>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    let Ok(ops) = Content::decode(content) else { return };
    let mut ctm = base;
    let mut stack = Vec::new();
    for op in ops.operations {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or(base),
            "cm" if op.operands.len() == 6 => {
                let mut m = [0.0; 6];
                for (i, v) in op.operands.iter().enumerate() {
                    m[i] = num(doc, v);
                }
                ctm = concat_matrix(m, ctm);
            }
            "Do" => {
                let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) else { continue };
                let Some(xobjects) =
                    resources.and_then(|r| r.get(b"XObject").ok()).and_then(|o| deref(doc, o).as_dict().ok())
                else {
                    continue;
                };
                let Ok(reference) = xobjects.get(name) else { continue };
                let Ok(id) = reference.as_reference() else { continue };
                let Some((dict, data)) = stream_content(doc, id) else { continue };
                let subtype = dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()).unwrap_or(b"");
                if subtype == b"Image" {
                    out.push(ImageInvocation {
                        page_index,
                        order: out.len(),
                        resource_name: String::from_utf8_lossy(name).into_owned(),
                        xobject_id: Some(id),
                        ctm,
                        pixel_width: dict.get(b"Width").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32,
                        pixel_height: dict.get(b"Height").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32,
                        inside_form_xobject: inside_form,
                    });
                } else if subtype == b"Form" && descend_forms {
                    let fm =
                        dict.get(b"Matrix").ok().and_then(|o| matrix(doc, o)).unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                    let form_res = dict.get(b"Resources").ok().and_then(|o| deref(doc, o).as_dict().ok()).or(resources);
                    walk(doc, page_index, &data, form_res, concat_matrix(fm, ctm), true, true, out, depth + 1);
                }
            }
            _ => {}
        }
    }
}

/// `Do`-Aufrufe von Bild-XObjects mit CTM (wie `SourcePdf::image_invocations`).
pub fn image_invocations(doc: &Document, page_index: usize, descend_forms: bool) -> Vec<ImageInvocation> {
    let Some(&page_id) = doc.get_pages().values().nth(page_index) else { return Vec::new() };
    let content = doc.get_page_content(page_id);
    let resources = inherited(doc, page_id, b"Resources").and_then(|o| o.as_dict().ok());
    let mut out = Vec::new();
    walk(doc, page_index, &content, resources, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], false, descend_forms, &mut out, 0);
    out
}

pub fn all_invocations(doc: &Document, descend_forms: bool) -> Vec<ImageInvocation> {
    (0..doc.get_pages().len()).flat_map(|i| image_invocations(doc, i, descend_forms)).collect()
}

fn text(doc: &Document, o: &Object) -> Option<String> {
    let bytes = deref(doc, o).as_str().ok()?;
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        return Some(String::from_utf16_lossy(&units));
    }
    Some(bytes.iter().map(|&b| b as char).collect())
}

fn page_of_widget(doc: &Document, widget: ObjectId, dict: &Dictionary, page_ids: &[ObjectId]) -> Option<usize> {
    if let Ok(p) = dict.get(b"P").and_then(|o| o.as_reference())
        && let Some(i) = page_ids.iter().position(|id| *id == p)
    {
        return Some(i);
    }
    page_ids.iter().position(|pid| {
        doc.get_dictionary(*pid)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
            .and_then(|a| deref(doc, a).as_array().ok())
            .is_some_and(|a| a.iter().any(|o| o.as_reference().ok() == Some(widget)))
    })
}

fn collect_fields(
    doc: &Document,
    id: ObjectId,
    parent_name: Option<String>,
    inherited_ft: Option<Vec<u8>>,
    depth: usize,
    page_ids: &[ObjectId],
    out: &mut Vec<SigField>,
) {
    if depth > 32 {
        return;
    }
    let Ok(dict) = doc.get_dictionary(id) else { return };
    let partial = dict.get(b"T").ok().and_then(|o| text(doc, o));
    let name = match (&parent_name, &partial) {
        (Some(p), Some(t)) => Some(format!("{p}.{t}")),
        (None, Some(t)) => Some(t.clone()),
        (p, None) => p.clone(),
    };
    let ft = dict.get(b"FT").ok().and_then(|o| o.as_name().ok()).map(<[u8]>::to_vec).or(inherited_ft);
    let kids: Vec<ObjectId> = dict
        .get(b"Kids")
        .ok()
        .and_then(|o| deref(doc, o).as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
        .unwrap_or_default();
    let (field_kids, widget_kids): (Vec<ObjectId>, Vec<ObjectId>) =
        kids.into_iter().partition(|k| doc.get_dictionary(*k).is_ok_and(|d| d.has(b"T")));
    for k in field_kids {
        collect_fields(doc, k, name.clone(), ft.clone(), depth + 1, page_ids, out);
    }
    if ft.as_deref() != Some(b"Sig") {
        return;
    }
    let mut widgets = Vec::new();
    let widget_ids: Vec<ObjectId> = if dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Widget") {
        vec![id]
    } else {
        widget_kids
    };
    for w in widget_ids {
        let Ok(wd) = doc.get_dictionary(w) else { continue };
        widgets.push(SigWidget {
            id: w,
            page_index: page_of_widget(doc, w, wd, page_ids),
            rect: wd.get(b"Rect").ok().and_then(|o| rect(doc, o)).unwrap_or_default(),
            annot_flags: wd.get(b"F").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32,
        });
    }
    if partial.is_none() && widgets.is_empty() {
        return;
    }
    let value = dict.get(b"V").ok();
    let signed = value.is_some_and(|v| !matches!(deref(doc, v), Object::Null));
    out.push(SigField {
        qualified_name: name.unwrap_or_default(),
        partial_name: partial,
        field_id: id,
        depth,
        widgets,
        signed,
        value_id: value.and_then(|v| v.as_reference().ok()),
        has_lock: dict.has(b"Lock"),
        has_seed_value: dict.has(b"SV"),
        field_flags: dict.get(b"Ff").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32,
    });
}

/// Signaturfelder des AcroForm-Baums (wie `SourcePdf::signature_fields`).
pub fn sig_fields(doc: &Document) -> Vec<SigField> {
    let page_ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let Ok(catalog) = doc.catalog() else { return Vec::new() };
    let Some(acro) = catalog.get(b"AcroForm").ok().and_then(|o| deref(doc, o).as_dict().ok()) else {
        return Vec::new();
    };
    let Some(fields) = acro.get(b"Fields").ok().and_then(|o| deref(doc, o).as_array().ok()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in fields {
        if let Ok(id) = f.as_reference() {
            collect_fields(doc, id, None, None, 0, &page_ids, &mut out);
        }
    }
    out
}

/// `/Location` aller Signaturen (benutzte Platzhalter).
pub fn signature_locations(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    for f in sig_fields(doc).iter().filter(|f| f.signed) {
        let Ok(field) = doc.get_dictionary(f.field_id) else { continue };
        let Some(v) = field.get(b"V").ok().map(|o| deref(doc, o)) else { continue };
        if let Some(loc) = v.as_dict().ok().and_then(|d| d.get(b"Location").ok()).and_then(|o| text(doc, o)) {
            out.push(loc);
        }
    }
    out
}

pub fn already_signed(doc: &Document) -> bool {
    sig_fields(doc).iter().any(|f| f.signed)
}

// ------------------------------------------------------------------------------------------------
// PDF-Schreiber und QR-Bilder
// ------------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct PdfBuilder {
    objects: Vec<Vec<u8>>,
}

impl PdfBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reserve(&mut self) -> u32 {
        self.objects.push(Vec::new());
        self.objects.len() as u32
    }
    pub fn set(&mut self, id: u32, body: impl Into<Vec<u8>>) {
        self.objects[id as usize - 1] = body.into();
    }
    pub fn add(&mut self, body: impl Into<Vec<u8>>) -> u32 {
        let id = self.reserve();
        self.set(id, body);
        id
    }
    /// Stream ohne Filter; `dict` ohne `/Length` und ohne `<< >>`.
    pub fn add_stream(&mut self, dict: &str, data: &[u8]) -> u32 {
        let mut body = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }
    /// Graustufenbild (8 bpc) als XObject.
    pub fn add_gray_image(&mut self, img: &GrayImage) -> u32 {
        self.add_stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8",
                img.width, img.height
            ),
            &img.data,
        )
    }
    pub fn finish(self, root: u32) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1).as_bytes());
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {} /Root {root} 0 R >>\nstartxref\n{xref}\n%%EOF\n", self.objects.len() + 1)
                .as_bytes(),
        );
        out
    }
}

pub struct PageSpec {
    pub media_box: [f32; 4],
    pub crop_box: Option<[f32; 4]>,
    pub rotate: i32,
    pub content: Vec<u8>,
    pub resources: String,
    pub extra: String,
}

impl PageSpec {
    pub fn new(media_box: [f32; 4], content: &str) -> Self {
        Self {
            media_box,
            crop_box: None,
            rotate: 0,
            content: content.as_bytes().to_vec(),
            resources: String::new(),
            extra: String::new(),
        }
    }
    pub fn a4(content: &str) -> Self {
        Self::new([0.0, 0.0, 595.0, 842.0], content)
    }
}

fn fmt_box(b: [f32; 4]) -> String {
    format!("[{} {} {} {}]", b[0], b[1], b[2], b[3])
}

pub fn build_doc(mut b: PdfBuilder, pages: Vec<PageSpec>, catalog_extra: &str) -> Vec<u8> {
    let pages_id = b.reserve();
    let mut kids = Vec::new();
    for p in pages {
        let content = b.add_stream("", &p.content);
        let crop = p.crop_box.map(|c| format!("/CropBox {}", fmt_box(c))).unwrap_or_default();
        let page = b.add(format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox {} {crop} /Rotate {} /Contents {content} 0 R \
             /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> {} >> {} >>",
            fmt_box(p.media_box),
            p.rotate,
            p.resources,
            p.extra
        ));
        kids.push(format!("{page} 0 R"));
    }
    b.set(pages_id, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), kids.len()));
    let catalog = b.add(format!("<< /Type /Catalog /Pages {pages_id} 0 R {catalog_extra} >>"));
    b.finish(catalog)
}

/// 8-Bit-Graustufenbild.
#[derive(Clone)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// QR-Code mit dem `rxing`-Encoder, `module_px` Pixel je Modul, Ruhezone 4 Module.
pub fn qr_image(text: &str, module_px: u32) -> GrayImage {
    use rxing::Writer;
    let m = rxing::qrcode::QRCodeWriter.encode(text, &rxing::BarcodeFormat::QR_CODE, 0, 0).expect("QR kodierbar");
    let (w, h) = (m.width() * module_px, m.height() * module_px);
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            data.push(if m.get(x / module_px, y / module_px) { 0 } else { 255 });
        }
    }
    GrayImage { width: w, height: h, data }
}

/// Platzhalterbild wie der PDF-AS-Generator: QR links, rechts weiße Fläche mit Rand.
pub fn generator_like_image(text: &str) -> GrayImage {
    let qr = qr_image(text, 3);
    let width = qr.width * 5;
    let mut data = vec![255u8; (width * qr.height) as usize];
    for y in 0..qr.height {
        for x in 0..qr.width {
            data[(y * width + x) as usize] = qr.data[(y * qr.width + x) as usize];
        }
        data[(y * width + width - 1) as usize] = 0;
    }
    GrayImage { width, height: qr.height, data }
}

pub fn to_rgba(img: &GrayImage) -> RgbaImage {
    let mut data = Vec::with_capacity(img.data.len() * 4);
    for &g in &img.data {
        data.extend_from_slice(&[g, g, g, 255]);
    }
    RgbaImage::from_raw(img.width, img.height, data).unwrap()
}

// ------------------------------------------------------------------------------------------------
// Referenzbilder (PDF-AS-Testsuiten, 288 dpi = Faktor 4)
// ------------------------------------------------------------------------------------------------

/// Umrandeter Block im Referenzbild (Pixel, inklusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockPx {
    pub x0: u32,
    pub x1: u32,
    pub y0: u32,
    pub y1: u32,
}

/// Graustufen (Luma) eines PNG.
pub fn load_png_luma(path: &Path) -> (u32, u32, Vec<u8>) {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("PNG lesbar");
    let mut buf = vec![0; reader.output_buffer_size().expect("PNG-Größe")];
    let info = reader.next_frame(&mut buf).expect("PNG-Frame");
    let (w, h) = (info.width, info.height);
    let channels = info.color_type.samples();
    let luma = buf[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|p| match channels {
            1 | 2 => p[0],
            _ => ((299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2])) / 1000) as u8,
        })
        .collect();
    (w, h, luma)
}

/// Alle umrandeten Rechtecke (Tabellenrahmen 0,1 pt ⇒ 1 px dunkel bei 288 dpi): Zeilen mit
/// langem dunklem Lauf, gruppiert nach gleicher x-Ausdehnung, verbunden durch dunkle Spalten an
/// beiden Enden. Größte Rechtecke zuerst.
pub fn detect_blocks(w: u32, h: u32, luma: &[u8]) -> Vec<BlockPx> {
    let dark = |x: u32, y: u32| luma[(y * w + x) as usize] < 200;
    let mut rows: Vec<(u32, u32, u32)> = Vec::new();
    for y in 0..h {
        let mut best = (0u32, 0u32, 0u32);
        let mut start: Option<u32> = None;
        for x in 0..=w {
            let d = x < w && dark(x, y);
            match (d, start) {
                (true, None) => start = Some(x),
                (false, Some(s)) => {
                    if x - s > best.0 {
                        best = (x - s, s, x - 1);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        if best.0 >= 150 {
            rows.push((y, best.1, best.2));
        }
    }
    let mut groups: Vec<(u32, u32, Vec<u32>)> = Vec::new();
    for (y, s, e) in rows {
        match groups.iter_mut().find(|g| g.0.abs_diff(s) <= 2 && g.1.abs_diff(e) <= 2) {
            Some(g) => g.2.push(y),
            None => groups.push((s, e, vec![y])),
        }
    }
    let mut out = Vec::new();
    for (s, e, ys) in groups {
        let column_dark = |x: u32, top: u32, bot: u32| {
            let n = (top..=bot).filter(|&y| dark(x, y)).count();
            n as f32 / (bot - top + 1) as f32 >= 0.9
        };
        // größtes verbundenes Zeilenpaar dieser Gruppe (getrennte Blöcke gleicher Breite)
        let mut used = vec![false; ys.len()];
        for i in 0..ys.len() {
            if used[i] {
                continue;
            }
            let mut best: Option<usize> = None;
            for j in (i + 1)..ys.len() {
                if ys[j] - ys[i] >= 20 && column_dark(s, ys[i], ys[j]) && column_dark(e, ys[i], ys[j]) {
                    best = Some(j);
                }
            }
            if let Some(j) = best {
                for u in used.iter_mut().take(j + 1).skip(i) {
                    *u = true;
                }
                out.push(BlockPx { x0: s, x1: e, y0: ys[i], y1: ys[j] });
            }
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse((b.x1 - b.x0) * (b.y1 - b.y0)));
    out
}

/// Umkehrung der BBox-Stauchung `v = W²/(W+1)` (docs/analyse/04 §3.7).
pub fn unsqueeze(visible: f32) -> f32 {
    (visible + (visible * visible + 4.0 * visible).sqrt()) / 2.0
}

/// Erwartete Blockgeometrie (pt) aus Pixeln: x links, Oberkante y (von unten), Breite, Höhe.
pub fn block_pt(b: BlockPx, page_height: f32) -> (f32, f32, f32, f32) {
    let x = b.x0 as f32 / 4.0;
    let y = page_height - b.y0 as f32 / 4.0;
    let w = unsqueeze((b.x1 - b.x0) as f32 / 4.0);
    let h = unsqueeze((b.y1 - b.y0) as f32 / 4.0);
    (x, y, w, h)
}

pub fn map_pages(pages: &[PageInfo]) -> BTreeMap<usize, PageInfo> {
    pages.iter().map(|p| (p.index, p.clone())).collect()
}
