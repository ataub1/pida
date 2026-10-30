//! „QR-Platzhalter erstellen“ (Analyse 06 E-1, ADR 0004 Nachtrag 24): Bild eines Signaturplatzhalters wie der
//! PDF-AS-Generator (`pdf-as-web/placeholder`, PDF-AS-WAI) – QR-Code links, rechts „Platzhalter für die
//! elektronische Signatur Nr. …“, Format 5 : 1 –, offline aus dem Kern.
//!
//! - **Inhalt** wie PDF-AS: `PDF-AS-POS;id=<Nummer>[;profile=<Profil-ID>]` ([`pida_placement::qr_placeholder_text`]);
//!   PIDA, PDF-Over, pdf-as-web und PDF-AS-WAI erkennen ihn.
//! - **Größe:** 240 × 48 pt (8,5 × 1,7 cm). Die Breite des eingefügten Bildes bestimmt beim Signieren die Breite des
//!   Signaturblocks (PDF-AS: Platzhalter-`TablePos` mit `w`), deshalb etwa so breit wie der Standardblock.
//! - **PNG** über ein Mini-PDF und PDFium (gleiches Bild auf allen Plattformen, auch im Browser), 360 dpi mit
//!   `pHYs`, damit Textverarbeitungen es in der vorgesehenen Größe einfügen; **SVG** direkt.
//! - **Speichern:** mit Pfad (Speichern-Dialog am Desktop) dorthin, sonst in den Temp-Ordner und teilen
//!   (Mobile) bzw. öffnen (Web: Download).

use std::collections::BTreeMap;
use std::path::PathBuf;

use pida_i18n::Lang;
use pida_placement::QrMatrix;

/// Breite des Bildes in pt.
pub const WIDTH_PT: f32 = 240.0;
/// Höhe des Bildes in pt.
pub const HEIGHT_PT: f32 = 48.0;
/// Pixel je pt im PNG (360 dpi).
pub const PNG_SCALE: f32 = 5.0;
/// Höchstlänge der Nummer (wie PDF-AS-WAI, `maxlength=10`).
pub const MAX_ID_LEN: usize = 10;

/// Meldungs-ID: Nummer ungültig (nur Ziffern, 1 bis 10 Stellen).
pub const MSG_INVALID_ID: &str = "ng-qr-invalid-id";
/// Meldungs-ID: gespeichert (`name` = Dateiname).
pub const MSG_SAVED: &str = "ng-qr-saved";
/// Meldungs-ID: Erstellen bzw. Speichern fehlgeschlagen.
pub const MSG_FAILED: &str = "ng-qr-failed";

/// Signaturbild, das den Platzhalter beim Signieren ersetzt.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QrPlaceholderProfile {
    /// Standard (`SIGNATURBLOCK_SMALL_<SPRACHE>`)
    Standard,
    /// Amtssignatur (`AMTSSIGNATURBLOCK_<SPRACHE>_RECOMMENDED`)
    Amtssignatur,
    /// ohne Profilangabe: das der unterzeichnenden Person (PDF-AS-WAI „PDF Over“)
    Signer,
}

/// Dateiformat.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QrPlaceholderFormat {
    /// PNG (360 dpi)
    Png,
    /// SVG
    Svg,
}

/// Was der Platzhalter enthält.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct QrPlaceholderSpec {
    /// Nummer (nur Ziffern, 1 bis 10 Stellen; „0001“ und „1“ gelten beim Signieren als gleich)
    pub id: String,
    /// Signaturbild
    pub profile: QrPlaceholderProfile,
    /// Sprache des Signaturblocks und der Beschriftung
    pub language: Lang,
}

impl QrPlaceholderSpec {
    /// Prüft die Nummer; `Err` = Meldungs-ID.
    pub fn validate(&self) -> Result<(), &'static str> {
        let ok = !self.id.is_empty() && self.id.len() <= MAX_ID_LEN && self.id.bytes().all(|b| b.is_ascii_digit());
        if ok { Ok(()) } else { Err(MSG_INVALID_ID) }
    }

    /// Profil-ID im QR-Code (`None` = das der unterzeichnenden Person).
    pub fn profile_id(&self) -> Option<String> {
        let lang = match self.language {
            Lang::En => "EN",
            _ => "DE",
        };
        match self.profile {
            QrPlaceholderProfile::Standard => Some(format!("SIGNATURBLOCK_SMALL_{lang}")),
            QrPlaceholderProfile::Amtssignatur => Some(format!("AMTSSIGNATURBLOCK_{lang}_RECOMMENDED")),
            QrPlaceholderProfile::Signer => None,
        }
    }

    /// Inhalt des QR-Codes.
    pub fn payload(&self) -> String {
        pida_placement::qr_placeholder_text(&self.id, self.profile_id().as_deref())
    }

    /// Vorgeschlagener Dateiname, z. B. `qr-platzhalter-0001.png`.
    pub fn file_name(&self, format: QrPlaceholderFormat) -> String {
        let base = match self.language {
            Lang::En => "qr-placeholder",
            _ => "qr-platzhalter",
        };
        let ext = match format {
            QrPlaceholderFormat::Png => "png",
            QrPlaceholderFormat::Svg => "svg",
        };
        format!("{base}-{}.{ext}", self.id)
    }

    /// Beschriftung (drei Zeilen) in der Sprache des Platzhalters.
    fn caption(&self) -> [String; 3] {
        let t = |id: &str| pida_i18n::translate(self.language, id, &[]);
        [t("ng-qr-caption-1"), t("ng-qr-caption-2"), format!("{} {}", t("ng-qr-caption-number"), self.id)]
    }
}

/// Lage der Elemente (pt, Ursprung oben links).
struct Layout {
    qr: QrMatrix,
    /// Kantenlänge eines Moduls
    module: f32,
    /// linke obere Ecke des QR-Codes (ohne Ruhezone)
    origin: (f32, f32),
    caption: [String; 3],
}

const TEXT_X: f32 = HEIGHT_PT + 6.0;
const TEXT_SIZE: f32 = 10.0;
const TEXT_BASELINES: [f32; 3] = [15.0, 28.0, 41.0];
const BORDER: f32 = 0.5;

fn layout(spec: &QrPlaceholderSpec) -> Result<Layout, &'static str> {
    spec.validate()?;
    let qr = pida_placement::encode_qr(&spec.payload()).map_err(|_| MSG_FAILED)?;
    // QR im linken Quadrat (Höhe × Höhe) mit 4 Modulen Ruhezone
    let module = HEIGHT_PT / (qr.size as f32 + 8.0);
    let origin = (4.0 * module, 4.0 * module);
    Ok(Layout { qr, module, origin, caption: spec.caption() })
}

/// Rechtecke der dunklen Module, zeilenweise zusammengefasst: (Spalte, Zeile, Länge).
fn runs(qr: &QrMatrix) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    for y in 0..qr.size {
        let mut x = 0;
        while x < qr.size {
            if qr.get(x, y) {
                let start = x;
                while x < qr.size && qr.get(x, y) {
                    x += 1;
                }
                out.push((start, y, x - start));
            } else {
                x += 1;
            }
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Zahl ohne überflüssige Nachkommastellen (SVG, PDF).
fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".to_owned() } else { s.to_owned() }
}

/// SVG des Platzhalters.
pub fn svg(spec: &QrPlaceholderSpec) -> Result<Vec<u8>, &'static str> {
    let l = layout(spec)?;
    let label = xml_escape(&l.caption.join(" "));
    let mut path = String::new();
    for (x, y, len) in runs(&l.qr) {
        let px = l.origin.0 + x as f32 * l.module;
        let py = l.origin.1 + y as f32 * l.module;
        path.push_str(&format!(
            "M{} {}h{}v{}h-{}z",
            num(px),
            num(py),
            num(len as f32 * l.module),
            num(l.module),
            num(len as f32 * l.module)
        ));
    }
    let mut text = String::new();
    for (line, y) in l.caption.iter().zip(TEXT_BASELINES) {
        text.push_str(&format!("<text x=\"{}\" y=\"{}\">{}</text>", num(TEXT_X), num(y), xml_escape(line)));
    }
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}pt\" height=\"{h}pt\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"{label}\">\
         <title>{label}</title>\
         <rect width=\"{w}\" height=\"{h}\" fill=\"#fff\"/>\
         <rect x=\"{b2}\" y=\"{b2}\" width=\"{bw}\" height=\"{bh}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{b}\"/>\
         <path d=\"{path}\" fill=\"#000\" shape-rendering=\"crispEdges\"/>\
         <g font-family=\"Helvetica, Arial, sans-serif\" font-weight=\"bold\" font-size=\"{fs}\" fill=\"#000\">{text}</g>\
         </svg>\n",
        w = num(WIDTH_PT),
        h = num(HEIGHT_PT),
        b = num(BORDER),
        b2 = num(BORDER / 2.0),
        bw = num(WIDTH_PT - BORDER),
        bh = num(HEIGHT_PT - BORDER),
        fs = num(TEXT_SIZE),
    );
    Ok(svg.into_bytes())
}

/// Text als PDF-String in WinAnsi (Latin-1-Teilmenge; anderes wird `?`).
fn pdf_string(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            c if (0xA0..=0xFF).contains(&(c as u32)) => out.push_str(&format!("\\{:03o}", c as u32)),
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

/// Einseitiges PDF des Platzhalters (Vorlage für das PNG).
pub fn pdf(spec: &QrPlaceholderSpec) -> Result<Vec<u8>, &'static str> {
    let l = layout(spec)?;
    let mut content = String::new();
    content.push_str(&format!("1 g 0 0 {} {} re f\n", num(WIDTH_PT), num(HEIGHT_PT)));
    content.push_str(&format!(
        "0 G {} w {} {} {} {} re S\n",
        num(BORDER),
        num(BORDER / 2.0),
        num(BORDER / 2.0),
        num(WIDTH_PT - BORDER),
        num(HEIGHT_PT - BORDER)
    ));
    content.push_str("0 g\n");
    for (x, y, len) in runs(&l.qr) {
        let px = l.origin.0 + x as f32 * l.module;
        let top = l.origin.1 + y as f32 * l.module;
        content.push_str(&format!(
            "{} {} {} {} re\n",
            num(px),
            num(HEIGHT_PT - top - l.module),
            num(len as f32 * l.module),
            num(l.module)
        ));
    }
    content.push_str("f\nBT\n");
    content.push_str(&format!("/F1 {} Tf\n", num(TEXT_SIZE)));
    for (line, y) in l.caption.iter().zip(TEXT_BASELINES) {
        content.push_str(&format!("1 0 0 1 {} {} Tm {} Tj\n", num(TEXT_X), num(HEIGHT_PT - y), pdf_string(line)));
    }
    content.push_str("ET\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
            num(WIDTH_PT),
            num(HEIGHT_PT)
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
    ];
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
    );
    Ok(out)
}

/// PNG des Platzhalters (PDFium, [`PNG_SCALE`]) mit Auflösung (`pHYs`).
pub fn png(spec: &QrPlaceholderSpec, render: &dyn pida_render::RenderBackend) -> Result<Vec<u8>, &'static str> {
    let pdf = pdf(spec)?;
    let renderer = render.open(std::sync::Arc::from(pdf), None).map_err(|_| MSG_FAILED)?;
    let image = renderer.render_page(0, PNG_SCALE).map_err(|_| MSG_FAILED)?;
    encode_png(&image).ok_or(MSG_FAILED)
}

/// PNG (RGB, ohne Alpha) mit 360 dpi.
fn encode_png(image: &pida_render::RgbaImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, image.width, image.height);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let ppm = (PNG_SCALE * 72.0 / 0.0254).round() as u32;
        enc.set_pixel_dims(Some(png::PixelDimensions { xppu: ppm, yppu: ppm, unit: png::Unit::Meter }));
        let mut writer = enc.write_header().ok()?;
        let rgb: Vec<u8> = image.data.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        writer.write_image_data(&rgb).ok()?;
    }
    Some(out)
}

/// Bild im gewünschten Format.
pub fn image(
    spec: &QrPlaceholderSpec,
    format: QrPlaceholderFormat,
    render: Option<&dyn pida_render::RenderBackend>,
) -> Result<Vec<u8>, &'static str> {
    match format {
        QrPlaceholderFormat::Svg => svg(spec),
        QrPlaceholderFormat::Png => png(spec, render.ok_or(MSG_FAILED)?),
    }
}

/// Hinweis nach dem Speichern bzw. bei einem Fehler.
pub(crate) fn notice(result: Result<String, &'static str>) -> (String, BTreeMap<String, String>) {
    match result {
        Ok(name) => (MSG_SAVED.to_owned(), BTreeMap::from([("name".to_owned(), name)])),
        Err(code) => (code.to_owned(), BTreeMap::new()),
    }
}

/// Ziel ohne Pfad (Mobile, Web): Temp-Ordner des Kerns.
pub(crate) fn temp_target(
    temp_dir: &std::path::Path,
    spec: &QrPlaceholderSpec,
    format: QrPlaceholderFormat,
) -> PathBuf {
    temp_dir.join("qr").join(spec.file_name(format))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, profile: QrPlaceholderProfile, language: Lang) -> QrPlaceholderSpec {
        QrPlaceholderSpec { id: id.to_owned(), profile, language }
    }

    #[test]
    fn payload_profile_and_file_name_like_pdf_as() {
        let s = spec("0001", QrPlaceholderProfile::Standard, Lang::De);
        assert_eq!(s.payload(), "PDF-AS-POS;id=0001;profile=SIGNATURBLOCK_SMALL_DE");
        assert_eq!(s.file_name(QrPlaceholderFormat::Png), "qr-platzhalter-0001.png");
        let a = spec("2", QrPlaceholderProfile::Amtssignatur, Lang::En);
        assert_eq!(a.payload(), "PDF-AS-POS;id=2;profile=AMTSSIGNATURBLOCK_EN_RECOMMENDED");
        assert_eq!(a.file_name(QrPlaceholderFormat::Svg), "qr-placeholder-2.svg");
        assert_eq!(spec("7", QrPlaceholderProfile::Signer, Lang::De).payload(), "PDF-AS-POS;id=7");
        // jedes Profil gibt es im eingebauten Profilspeicher
        for p in [a.profile_id(), s.profile_id()].into_iter().flatten() {
            assert!(pida_sigblock::PDFOVER_PROFILE_IDS.contains(&p.as_str()), "{p}");
        }
    }

    #[test]
    fn number_must_be_one_to_ten_digits() {
        for bad in ["", "abc", "1a", "12345678901", "-1", " 1"] {
            assert_eq!(spec(bad, QrPlaceholderProfile::Standard, Lang::De).validate(), Err(MSG_INVALID_ID), "{bad:?}");
            assert_eq!(svg(&spec(bad, QrPlaceholderProfile::Standard, Lang::De)).unwrap_err(), MSG_INVALID_ID);
        }
        assert!(spec("0123456789", QrPlaceholderProfile::Standard, Lang::De).validate().is_ok());
    }

    #[test]
    fn svg_and_pdf_carry_caption_and_size() {
        let s = spec("0002", QrPlaceholderProfile::Standard, Lang::De);
        let svg = String::from_utf8(svg(&s).unwrap()).unwrap();
        assert!(svg.contains("viewBox=\"0 0 240 48\"") && svg.contains("width=\"240pt\""), "{svg}");
        assert!(svg.contains(">Platzhalter für die<") && svg.contains(">Nr. 0002<"), "{svg}");
        let pdf = pdf(&s).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(
            text.starts_with("%PDF-1.4")
                && text.contains("/MediaBox [0 0 240 48]")
                && text.trim_end().ends_with("%%EOF")
        );
        // ü in WinAnsi (oktal 374)
        assert!(text.contains("(Platzhalter f\\374r die)"), "{text}");
        // xref-Offsets stimmen
        let start: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(pdf[start..].starts_with(b"xref"));
    }
}
