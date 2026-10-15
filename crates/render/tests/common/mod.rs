//! Test-Hilfen für `pida-render`: minimaler PDF-Schreiber (ohne Abhängigkeiten) und
//! PDFium-Bindung über `PIDA_PDFIUM_DIR` (ohne Variable werden PDFium-Tests übersprungen).
#![allow(dead_code)]

use std::sync::Arc;

use pida_render::{ENV_PDFIUM_DIR, PdfiumBackend};

/// Bindet PDFium oder liefert `None` (Test überspringen).
pub fn backend() -> Option<PdfiumBackend> {
    let dir = std::env::var_os(ENV_PDFIUM_DIR)?;
    if dir.is_empty() {
        return None;
    }
    Some(PdfiumBackend::bind(Some(std::path::Path::new(&dir))).expect("PDFium aus PIDA_PDFIUM_DIR bindbar"))
}

/// Überspringt den Test ohne PDFium.
#[macro_export]
macro_rules! require_pdfium {
    () => {
        match common::backend() {
            Some(b) => b,
            None => {
                eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – Test übersprungen");
                return;
            }
        }
    };
}

/// Pfad in `testdata/`.
pub fn testdata(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel)
}

pub fn read(rel: &str) -> Arc<[u8]> {
    std::fs::read(testdata(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")).into()
}

/// Sehr kleiner PDF-Schreiber mit korrekter xref-Tabelle.
#[derive(Default)]
pub struct PdfBuilder {
    objects: Vec<Vec<u8>>,
}

impl PdfBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserviert eine Objektnummer (Inhalt später mit [`PdfBuilder::set`]).
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

    /// Graustufenbild `w × h` (8 Bit) mit konstantem Wert, FlateDecode-komprimiert – klein in der
    /// Datei, groß dekodiert (Dekompressionsbomben-Tests).
    pub fn add_flate_gray_image(&mut self, w: u32, h: u32, value: u8) -> u32 {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        let row = vec![value; w as usize];
        for _ in 0..h {
            enc.write_all(&row).expect("zlib");
        }
        let z = enc.finish().expect("zlib");
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
            z.len()
        )
        .into_bytes();
        body.extend_from_slice(&z);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
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

/// Beschreibung einer Testseite.
pub struct PageSpec {
    pub media_box: [f32; 4],
    pub crop_box: Option<[f32; 4]>,
    pub rotate: i32,
    pub content: String,
    /// Zusätzliche Ressourcen (Inhalt des `/Resources`-Dictionarys ohne `<< >>`).
    pub resources: String,
    /// Zusätzliche Einträge im Seitendictionary.
    pub extra: String,
}

impl PageSpec {
    pub fn a4(content: &str) -> Self {
        Self {
            media_box: [0.0, 0.0, 595.0, 842.0],
            crop_box: None,
            rotate: 0,
            content: content.to_owned(),
            resources: String::new(),
            extra: String::new(),
        }
    }
}

fn fmt_box(b: [f32; 4]) -> String {
    format!("[{} {} {} {}]", b[0], b[1], b[2], b[3])
}

/// Baut ein Dokument aus Seiten; `objects` erlaubt zusätzliche Objekte (z. B. Bilder), die vor den
/// Seiten angelegt werden und deren Nummern die Ressourcen referenzieren können.
pub fn build_doc(mut b: PdfBuilder, pages: Vec<PageSpec>, catalog_extra: &str) -> Vec<u8> {
    let pages_id = b.reserve();
    let mut kids = Vec::new();
    for p in pages {
        let content = b.add_stream("", p.content.as_bytes());
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

/// Ein Dokument mit genau einer Seite.
pub fn single_page(spec: PageSpec) -> Arc<[u8]> {
    build_doc(PdfBuilder::new(), vec![spec], "").into()
}

/// Anzahl Pixel im Rechteck (x0..x1, y0..y1), die `pred` erfüllen.
pub fn count(
    img: &pida_render::RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    pred: impl Fn([u8; 4]) -> bool,
) -> usize {
    let mut n = 0;
    for y in y0..y1.min(img.height) {
        for x in x0..x1.min(img.width) {
            if pred(img.pixel(x, y)) {
                n += 1;
            }
        }
    }
    n
}
