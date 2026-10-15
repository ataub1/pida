//! Fonts und Metriken (docs/analyse/04 §3.6).
//!
//! - Standard-14: AFM-Breiten (Adobe Core14, Lizenzhinweis in `assets/licenses/`), WinAnsi, nicht
//!   eingebettet; Helvetica(-Bold) descender −207, Courier −157/600.
//! - TTF: Breite = `round(advance · 1000 / upem)` (so steht sie im `/W`-Array), Descent =
//!   `hhea.descender · 1000 / upem` (float); DejaVuSansMono 602, descent −235.84.
//! - Einbettung als Type0/CIDFontType2-Subset (Identity-H, `/W`, ToUnicode, CIDToGIDMap, CIDSet außer
//!   bei PDF/A-3) über `skrifa` + `subsetter` (Objekte baut [`crate::Appearance::build`]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use skrifa::MetadataProvider;
use skrifa::raw::types::GlyphId;
use skrifa::raw::{FontRef, TableProvider};

use crate::profile::{BuiltinResources, Resources};
use crate::{Result, SigBlockError, afm, java};

/// Standard-14-Varianten, die PDF-AS akzeptiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Std14 {
    /// Helvetica
    Helvetica,
    /// Helvetica-Bold
    HelveticaBold,
    /// Courier
    Courier,
    /// Courier-Bold
    CourierBold,
    /// Times-Roman
    TimesRoman,
    /// Times-Bold
    TimesBold,
    /// Times-Italic
    TimesItalic,
}

impl Std14 {
    /// PostScript-Name (`/BaseFont`).
    pub fn base_font(self) -> &'static str {
        match self {
            Std14::Helvetica => "Helvetica",
            Std14::HelveticaBold => "Helvetica-Bold",
            Std14::Courier => "Courier",
            Std14::CourierBold => "Courier-Bold",
            Std14::TimesRoman => "Times-Roman",
            Std14::TimesBold => "Times-Bold",
            Std14::TimesItalic => "Times-Italic",
        }
    }

    /// Schlüssel `FACE:STYLE` aus `PDFAsFontCache` (Groß-/Kleinschreibung wie in Java relevant).
    pub fn from_face_style(face: &str, style: &str) -> Option<Std14> {
        Some(match (face, style) {
            ("HELVETICA", "NORMAL") => Std14::Helvetica,
            ("HELVETICA", "BOLD") => Std14::HelveticaBold,
            ("COURIER", "NORMAL") => Std14::Courier,
            ("COURIER", "BOLD") => Std14::CourierBold,
            ("TIMES_ROMAN", "NORMAL") => Std14::TimesRoman,
            ("TIMES_ROMAN", "BOLD") => Std14::TimesBold,
            ("TIMES_ROMAN", "ITALIC") => Std14::TimesItalic,
            _ => return None,
        })
    }

    /// Eingebetteter Ersatz für PDF/A (Plan §6.9, Register): DejaVu-Schnitte aus `assets/fonts`.
    pub fn pdfa_substitute(self) -> &'static str {
        match self {
            Std14::HelveticaBold | Std14::TimesBold => "DejaVuSansCondensed-Bold.ttf",
            Std14::Courier | Std14::CourierBold => "DejaVuSansMono.ttf",
            Std14::Helvetica | Std14::TimesRoman | Std14::TimesItalic => "DejaVuSansCondensed.ttf",
        }
    }
}

/// Default-Font von PDF-AS bei ungültiger Beschreibung (`PDFBoxFont.defaultFont`, Größe 8).
const DEFAULT_FONT: FontSpec = FontSpec::Standard { face: Std14::Helvetica, size: 8.0 };

/// Fontbeschreibung `FACE,SIZE,STYLE` bzw. `TTF:<datei>,SIZE` (ungültig ⇒ Fallback Helvetica 8).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FontSpec {
    /// Standard-14
    Standard {
        /// Face
        face: Std14,
        /// Größe in pt
        size: f32,
    },
    /// TrueType aus `fonts/`
    TrueType {
        /// Dateiname (z. B. `DejaVuSansMono.ttf`)
        file: String,
        /// Größe in pt
        size: f32,
    },
}

impl FontSpec {
    /// Parst die PDF-AS-Syntax mit Fallback `HELVETICA,8` – auch dort, wo PDF-AS mit einem Fehler
    /// abbricht (unbekannter Face/Style, ungültige Größe); streng ist [`FontSpec::try_parse`].
    pub fn parse(desc: &str) -> FontSpec {
        FontSpec::try_parse(desc).unwrap_or(DEFAULT_FONT)
    }

    /// Wie `PDFBoxFont.setFont`: 3 Teile `FACE,SIZE,STYLE` (bei `TTF:` wird STYLE ignoriert),
    /// 2 Teile nur mit `TTF:`; jede andere Teilanzahl ergibt Helvetica 8. Ein unbekannter
    /// `FACE:STYLE` (`Invalid font descriptor`) oder eine ungültige Größe (`NumberFormatException`)
    /// ist ein Fehler.
    pub fn try_parse(desc: &str) -> Result<FontSpec> {
        let parts = java::split(desc, ',');
        let invalid = || SigBlockError::Font(format!("ungültige Fontbeschreibung {desc:?}"));
        let ttf = |face: &str, size: &str| -> Result<FontSpec> {
            let size = java::parse_float(size).ok_or_else(invalid)?;
            // `fonttype.replaceFirst("TTF:", "")`
            Ok(FontSpec::TrueType { file: face.replacen("TTF:", "", 1), size })
        };
        match parts.as_slice() {
            [face, size, style] => {
                if face.starts_with("TTF:") {
                    ttf(face, size)
                } else {
                    let face = Std14::from_face_style(face, style).ok_or_else(invalid)?;
                    let size = java::parse_float(size).ok_or_else(invalid)?;
                    Ok(FontSpec::Standard { face, size })
                }
            }
            [face, size] if face.starts_with("TTF:") => ttf(face, size),
            _ => Ok(DEFAULT_FONT),
        }
    }

    /// Schriftgröße.
    pub fn size(&self) -> f32 {
        match self {
            FontSpec::Standard { size, .. } | FontSpec::TrueType { size, .. } => *size,
        }
    }

    /// Cache-Schlüssel wie `PDFAsFontCache` (Fontpfad bzw. `FACE:STYLE`), ohne Größe.
    pub fn cache_key(&self) -> String {
        match self {
            FontSpec::Standard { face, .. } => format!("std:{}", face.base_font()),
            FontSpec::TrueType { file, .. } => format!("ttf:{file}"),
        }
    }

    /// Dieselbe Schrift mit eingebettetem Ersatz für PDF/A (Standard-14 ⇒ DejaVu, Plan §6.9).
    pub fn embeddable(&self) -> FontSpec {
        match self {
            FontSpec::Standard { face, size } => {
                FontSpec::TrueType { file: face.pdfa_substitute().into(), size: *size }
            }
            FontSpec::TrueType { .. } => self.clone(),
        }
    }
}

/// Metriken eines TrueType-Fonts, wie `TrueTypeEmbedder.createFontDescriptor` sie bildet.
#[derive(Clone)]
pub(crate) struct TtfMetrics {
    /// Dateiname (`DejaVuSansMono.ttf`)
    pub file: String,
    /// PostScript-Name (name ID 6)
    pub ps_name: String,
    /// Originaldaten
    pub data: Arc<[u8]>,
    /// `1000f / unitsPerEm`
    pub scaling: f32,
    /// Unicode → GID
    pub cmap: HashMap<u32, u16>,
    /// gerundete Breite je GID (1/1000 em)
    pub widths: Vec<u16>,
    /// `hhea.ascender · scaling`
    pub ascent: f32,
    /// `hhea.descender · scaling`
    pub descent: f32,
    /// `head`-BBox · scaling
    pub bbox: [f32; 4],
    /// CapHeight
    pub cap_height: f32,
    /// XHeight
    pub x_height: f32,
    /// `post.italicAngle`
    pub italic_angle: f32,
    /// `OS/2.usWeightClass`
    pub weight: f32,
    /// FontDescriptor-Flags
    pub flags: i64,
    /// `bbox.width · 0.13`
    pub stem_v: f32,
}

#[derive(Clone)]
pub(crate) enum FontKind {
    Std14(Std14),
    TrueType(Box<TtfMetrics>),
}

/// Geladener Font mit Metriken.
pub struct LoadedFont {
    pub(crate) kind: FontKind,
}

impl std::fmt::Debug for LoadedFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FontKind::Std14(face) => f.debug_struct("LoadedFont").field("std14", &face.base_font()).finish(),
            FontKind::TrueType(t) => f
                .debug_struct("LoadedFont")
                .field("ttf", &t.file)
                .field("glyphs", &t.widths.len())
                .finish_non_exhaustive(),
        }
    }
}

impl LoadedFont {
    /// Standard-14-Font.
    pub fn standard(face: Std14) -> LoadedFont {
        LoadedFont { kind: FontKind::Std14(face) }
    }

    /// TrueType-Font aus Dateidaten.
    pub fn truetype(file: &str, data: Arc<[u8]>) -> Result<LoadedFont> {
        Ok(LoadedFont { kind: FontKind::TrueType(Box::new(load_ttf(file, data)?)) })
    }

    /// Breite in 1/1000 em ohne Kerning (`getStringWidth`); Fehler bei nicht kodierbaren Zeichen.
    pub fn string_width(&self, text: &str) -> Result<f32> {
        let mut width = 0f32;
        for c in text.chars() {
            width += f32::from(self.char_width(c).ok_or(SigBlockError::MissingGlyph(c))?);
        }
        Ok(width)
    }

    /// Breite eines Zeichens (1/1000 em), `None` wenn nicht kodierbar.
    pub fn char_width(&self, c: char) -> Option<u16> {
        match &self.kind {
            FontKind::Std14(face) => {
                let code = afm::winansi_code(c)?;
                let w = afm::tables(*face).0[code as usize];
                (w != 0).then_some(w)
            }
            FontKind::TrueType(t) => t.gid(c).map(|g| t.widths.get(g as usize).copied().unwrap_or(1000)),
        }
    }

    /// Descent in 1/1000 em (negativ).
    pub fn descent(&self) -> f32 {
        match &self.kind {
            FontKind::Std14(face) => f32::from(afm::tables(*face).1.descender),
            FontKind::TrueType(t) => t.descent,
        }
    }

    /// `true`, wenn alle Zeichen kodierbar sind.
    pub fn can_encode(&self, text: &str) -> bool {
        text.chars().all(|c| self.char_width(c).is_some())
    }

    /// `true`, wenn das Zeichen kodierbar ist.
    pub fn can_encode_char(&self, c: char) -> bool {
        self.char_width(c).is_some()
    }

    /// Wird eingebettet (TTF) oder nicht (Standard-14).
    pub fn is_embedded(&self) -> bool {
        matches!(self.kind, FontKind::TrueType(_))
    }

    /// PostScript-Name ohne Subset-Präfix.
    pub fn base_font(&self) -> &str {
        match &self.kind {
            FontKind::Std14(face) => face.base_font(),
            FontKind::TrueType(t) => &t.ps_name,
        }
    }

    /// Kodierung für den Content-Stream: WinAnsi (1 Byte) bzw. Identity-H (2 Byte, CID = GID des
    /// Originalfonts wie bei PDFBox).
    pub fn encode(&self, text: &str) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(text.len() * 2);
        match &self.kind {
            FontKind::Std14(_) => {
                for c in text.chars() {
                    self.char_width(c).ok_or(SigBlockError::MissingGlyph(c))?;
                    out.push(afm::winansi_code(c).ok_or(SigBlockError::MissingGlyph(c))?);
                }
            }
            FontKind::TrueType(t) => {
                for c in text.chars() {
                    let gid = t.gid(c).ok_or(SigBlockError::MissingGlyph(c))?;
                    out.extend_from_slice(&gid.to_be_bytes());
                }
            }
        }
        Ok(out)
    }

    pub(crate) fn ttf(&self) -> Option<&TtfMetrics> {
        match &self.kind {
            FontKind::TrueType(t) => Some(t),
            FontKind::Std14(_) => None,
        }
    }
}

impl TtfMetrics {
    pub(crate) fn gid(&self, c: char) -> Option<u16> {
        self.cmap.get(&(c as u32)).copied().filter(|g| *g != 0)
    }
}

fn font_err(file: &str, e: impl std::fmt::Display) -> SigBlockError {
    SigBlockError::Font(format!("{file}: {e}"))
}

fn load_ttf(file: &str, data: Arc<[u8]>) -> Result<TtfMetrics> {
    let font = FontRef::new(&data).map_err(|e| font_err(file, e))?;
    let head = font.head().map_err(|e| font_err(file, e))?;
    let hhea = font.hhea().map_err(|e| font_err(file, e))?;
    let hmtx = font.hmtx().map_err(|e| font_err(file, e))?;
    let maxp = font.maxp().map_err(|e| font_err(file, e))?;
    let post = font.post().ok();
    let os2 = font.os2().ok();
    if font.glyf().is_err() {
        return Err(font_err(file, "nur TrueType-Outlines (glyf) werden unterstützt"));
    }
    let upem = head.units_per_em();
    if upem == 0 {
        return Err(font_err(file, "unitsPerEm = 0"));
    }
    let scaling = 1000f32 / f32::from(upem);
    let num_glyphs = maxp.num_glyphs();
    let widths = (0..num_glyphs)
        .map(|g| {
            let adv = hmtx.advance(GlyphId::new(u32::from(g))).unwrap_or(0);
            // `Math.round(advance * scaling)` (float)
            (f32::from(adv) * scaling + 0.5).floor() as u16
        })
        .collect();
    let mut cmap = HashMap::new();
    for (cp, gid) in font.charmap().mappings() {
        if let Ok(g) = u16::try_from(gid.to_u32()) {
            cmap.entry(cp).or_insert(g);
        }
    }
    let ps_name = font
        .localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME)
        .english_or_first()
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect::<String>())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| file.trim_end_matches(".ttf").replace(' ', ""));
    let bbox = [
        f32::from(head.x_min()) * scaling,
        f32::from(head.y_min()) * scaling,
        f32::from(head.x_max()) * scaling,
        f32::from(head.y_max()) * scaling,
    ];
    let glyph_top = |c: char| -> Option<f32> {
        let gid = font.charmap().map(c)?;
        let loca = font.loca(None).ok()?;
        let glyf = font.glyf().ok()?;
        let glyph = loca.get_glyf(gid, &glyf).ok()??;
        Some(f32::from(glyph.y_max()))
    };
    let os2_version = os2.as_ref().map(|o| o.version()).unwrap_or(0);
    let (cap_height, x_height) = match os2.as_ref() {
        Some(o) if os2_version >= 2 => {
            (f32::from(o.s_cap_height().unwrap_or(0)) * scaling, f32::from(o.sx_height().unwrap_or(0)) * scaling)
        }
        _ => {
            let (typo_asc, typo_desc) =
                os2.as_ref().map(|o| (o.s_typo_ascender(), o.s_typo_descender())).unwrap_or((0, 0));
            let cap = glyph_top('H')
                .map(|y| y.round() * scaling)
                .unwrap_or((f32::from(typo_asc) + f32::from(typo_desc)) * scaling);
            let x = glyph_top('x').map(|y| y.round() * scaling).unwrap_or(f32::from(typo_asc) / 2.0 * scaling);
            (cap, x)
        }
    };
    let fixed = post.as_ref().is_some_and(|p| p.is_fixed_pitch() > 0) || hhea.number_of_h_metrics() == 1;
    let mut flags = 4i64; // Symbolic (PDFBox setzt immer symbolic)
    if fixed {
        flags |= 1;
    }
    if let Some(o) = os2.as_ref() {
        match o.s_family_class() >> 8 {
            1 | 2 | 3 | 4 | 5 | 7 => flags |= 2,
            10 => flags |= 8,
            _ => {}
        }
        if o.fs_selection().bits() & (0x0001 | 0x0200) != 0 {
            flags |= 64;
        }
    }
    Ok(TtfMetrics {
        file: file.to_string(),
        ps_name,
        scaling,
        cmap,
        widths,
        ascent: f32::from(hhea.ascender().to_i16()) * scaling,
        descent: f32::from(hhea.descender().to_i16()) * scaling,
        bbox,
        cap_height,
        x_height,
        italic_angle: post.as_ref().map(|p| p.italic_angle().to_f32()).unwrap_or(0.0),
        weight: os2.as_ref().map(|o| f32::from(o.us_weight_class())).unwrap_or(400.0),
        flags,
        stem_v: (bbox[2] - bbox[0]) * 0.13,
        data,
    })
}

/// Fontquelle mit Cache je Dokument (Schlüssel Fontpfad bzw. `FACE:STYLE`).
pub struct FontProvider {
    resources: Arc<dyn Resources>,
    cache: Mutex<HashMap<String, Arc<LoadedFont>>>,
}

impl std::fmt::Debug for FontProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.cache.lock().map(|c| c.len()).unwrap_or(0);
        f.debug_struct("FontProvider").field("resources", &self.resources).field("cached", &n).finish()
    }
}

impl Default for FontProvider {
    fn default() -> Self {
        FontProvider::builtin()
    }
}

impl FontProvider {
    /// Nur eingebettete Fonts (DejaVu aus `assets/fonts`, AFM-Daten).
    pub fn builtin() -> FontProvider {
        FontProvider::from_resources(Arc::new(BuiltinResources::default()))
    }

    /// Fonts aus einer Ressourcenquelle (eigene Profile mit `TTF:<datei>` aus `<workdir>/fonts`).
    pub fn from_resources(resources: Arc<dyn crate::profile::Resources>) -> FontProvider {
        FontProvider { resources, cache: Mutex::new(HashMap::new()) }
    }

    /// Lädt bzw. liefert aus dem Cache.
    pub fn load(&self, spec: &FontSpec) -> Result<Arc<LoadedFont>> {
        let key = spec.cache_key();
        if let Some(f) = self.cache.lock().map_err(|_| SigBlockError::Font("Cache vergiftet".into()))?.get(&key) {
            return Ok(f.clone());
        }
        let font = Arc::new(match spec {
            FontSpec::Standard { face, .. } => LoadedFont::standard(*face),
            FontSpec::TrueType { file, .. } => LoadedFont::truetype(file, self.resources.font(file)?)?,
        });
        self.cache.lock().map_err(|_| SigBlockError::Font("Cache vergiftet".into()))?.insert(key, font.clone());
        Ok(font)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_like_pdfbox_font() {
        assert_eq!(FontSpec::parse("HELVETICA,5,BOLD"), FontSpec::Standard { face: Std14::HelveticaBold, size: 5.0 });
        assert_eq!(
            FontSpec::parse("TTF:DejaVuSansMono.ttf,5,NORMAL"),
            FontSpec::TrueType { file: "DejaVuSansMono.ttf".into(), size: 5.0 }
        );
        assert_eq!(
            FontSpec::parse("TTF:DejaVuSansCondensed-Bold.ttf,5"),
            FontSpec::TrueType { file: "DejaVuSansCondensed-Bold.ttf".into(), size: 5.0 }
        );
        // falsche Teilanzahl ⇒ Helvetica 8
        assert_eq!(FontSpec::parse("HELVETICA,5"), DEFAULT_FONT);
        assert_eq!(FontSpec::parse("HELVETICA,5,"), DEFAULT_FONT);
        assert_eq!(FontSpec::parse("garbage"), DEFAULT_FONT);
        // unbekannter Face/Style: PDF-AS bricht ab
        assert!(FontSpec::try_parse("HELVETICA,5,ITALIC").is_err());
        assert!(FontSpec::try_parse("helvetica,5,BOLD").is_err());
        assert!(FontSpec::try_parse("HELVETICA,x,BOLD").is_err());
        assert_eq!(FontSpec::parse("HELVETICA,x,BOLD"), DEFAULT_FONT);
        assert_eq!(FontSpec::parse("COURIER, 7 ,NORMAL"), FontSpec::Standard { face: Std14::Courier, size: 7.0 });
    }

    #[test]
    fn dejavu_metrics() {
        let p = FontProvider::builtin();
        let mono = p.load(&FontSpec::parse("TTF:DejaVuSansMono.ttf,5")).unwrap();
        assert_eq!(mono.string_width("Max Mustermann").unwrap(), 14.0 * 602.0);
        assert_eq!(mono.descent(), -235.839_84);
        assert!(mono.is_embedded());
        assert!(!mono.can_encode("a\nb"));
        let t = mono.ttf().unwrap();
        assert_eq!(t.ps_name, "DejaVuSansMono");
        assert_eq!(t.flags, 5);
        assert_eq!(t.ascent, 928.222_66);
        assert_eq!(t.cap_height, 729.003_9);
        assert_eq!(t.x_height, 546.875);
        assert_eq!(t.bbox, [-558.105_47, -374.511_72, 717.773_44, 1_041.503_9]);
        assert_eq!(t.stem_v, 165.864_26);
        assert_eq!(mono.encode("S").unwrap(), vec![0, 0x36]);
        let bold = p.load(&FontSpec::parse("TTF:DejaVuSansCondensed-Bold.ttf,5")).unwrap();
        assert_eq!(bold.descent(), -235.839_84);
        assert_eq!(bold.ttf().unwrap().flags, 4);
        // Cache liefert dieselbe Instanz
        let again = p.load(&FontSpec::parse("TTF:DejaVuSansMono.ttf,8")).unwrap();
        assert!(Arc::ptr_eq(&mono, &again));
    }

    #[test]
    fn std14_metrics() {
        let f = LoadedFont::standard(Std14::HelveticaBold);
        assert_eq!(f.descent(), -207.0);
        assert_eq!(f.string_width("Hinweis").unwrap(), 3779.0);
        assert!(!f.is_embedded());
        assert!(matches!(f.string_width("Ł"), Err(SigBlockError::MissingGlyph('Ł'))));
        assert_eq!(f.encode("Prü").unwrap(), b"Pr\xfc".to_vec());
        assert!(!f.can_encode("\u{a0}"));
    }

    #[test]
    fn missing_font_file() {
        let p = FontProvider::builtin();
        assert!(matches!(p.load(&FontSpec::parse("TTF:arial.ttf,5")), Err(SigBlockError::MissingResource(_))));
    }
}
