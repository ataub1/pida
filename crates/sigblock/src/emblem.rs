//! Bildmarke (docs/analyse/04 §7; Plan §6.5 „Bilder“).
//!
//! Eingabeformate PNG, JPEG, GIF (erstes Bild); EXIF-Ausrichtung **nur bei JPEG** wie PDF-Over
//! (`ImageUtil.readImageWithEXIFRotation`); Verkleinerung auf ≤ 480×600 px (erst Höhe, dann Breite,
//! `(int)`-Trunkierung, nächster Nachbar wie `Graphics2D.drawImage`); Cache als PNG.
//! HEIC dekodiert die Shell über den Plattform-Decoder und übergibt RGBA ([`Emblem::from_rgba`]).
//! Profilbilder (`./images/…`) lädt PDF-AS ohne EXIF und ohne Verkleinerung
//! ([`Emblem::decode_original`]).

use std::io::Cursor;

use sha2::{Digest, Sha256};

use crate::{Result, SigBlockError};

/// Höchstbreite nach Verkleinerung.
pub const MAX_EMBLEM_WIDTH: u32 = 480;
/// Höchsthöhe nach Verkleinerung.
pub const MAX_EMBLEM_HEIGHT: u32 = 600;

/// Obergrenze für dekodierte Bilder (Schutz vor Dekompressionsbomben): 64 Megapixel.
const MAX_DECODED_PIXELS: u64 = 64 * 1024 * 1024;

/// Farbtyp der Quelldatei (additiv, Welle 3).
///
/// PDF-AS bettet Bildmarken je nach Java-Bildtyp verschieden ein (PDFBox `LosslessFactory`); mit
/// `SigBlockInput::pdfas_quirks` bildet die Appearance das nach (Register SB-G2 … SB-G4):
/// - [`SourceColor::RgbAlpha`]/[`SourceColor::GrayAlpha`]: SMask für jeden Alphakanal, auch voll
///   deckend (SB-G2),
/// - [`SourceColor::Indexed`] (PNG-Palette, GIF, Graustufen-PNG unter 8 Bit – Java liest sie als
///   Palettenbild): Transparenz verworfen, Palettenfarbe (SB-G3),
/// - [`SourceColor::Gray`]/[`SourceColor::GrayAlpha`] (PNG-Graustufen ab 8 Bit, Graustufen-JPEG):
///   Java-Umrechnung des linearen `CS_GRAY` nach sRGB (aufgehellt, SB-G4); mit Alphakanal bzw.
///   `tRNS` zusätzlich eine SMask (Einzelheiten `appearance::QuirkImage`).
///
/// Ohne Quirks (Standard) zählt nur der Inhalt: SMask nur bei echter Transparenz, Palettenalpha
/// bleibt erhalten, Graustufen unverändert als RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SourceColor {
    /// Truecolor ohne Alphakanal (PNG RGB, Farb-JPEG); auch der Default für Strukturliterale
    #[default]
    Rgb,
    /// Truecolor mit Alphakanal (PNG RGBA; RGBA aus der Shell, [`Emblem::from_rgba`])
    RgbAlpha,
    /// Graustufen ohne Alphakanal (PNG Typ 0 mit 8 oder 16 Bit, auch mit `tRNS`; Graustufen-JPEG)
    Gray,
    /// Graustufen mit Alphakanal (PNG Typ 4)
    GrayAlpha,
    /// Palettenbild (PNG Typ 3, auch mit `tRNS`; GIF; PNG-Graustufen mit 1, 2 oder 4 Bit)
    Indexed,
}

impl SourceColor {
    /// Die Datei hat einen eigenen Alphakanal (nicht nur Paletten- bzw. Farbschlüssel-Transparenz).
    pub fn has_alpha_channel(self) -> bool {
        matches!(self, SourceColor::RgbAlpha | SourceColor::GrayAlpha)
    }
}

/// Java `ColorModel.getRGB` für ein Byte des linearen `CS_GRAY` (TYPE_BYTE_GRAY): sRGB-Kodierung
/// (IEC 61966-2-1) des linearen Werts, gerundet – so schreibt PDFBox `createFromGrayImage` die
/// Samples (SB-G4, z. B. 20 → 79; Golden PDF-AS 4.4.2 für alle Stufen von `emblem-tall.png`).
pub fn java_linear_gray_to_srgb(v: u8) -> u8 {
    let l = f64::from(v) / 255.0;
    let s = if l <= 0.003_130_8 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Dekodierte Bildmarke (RGBA, 8 Bit).
#[derive(Clone, PartialEq, Eq)]
pub struct Emblem {
    /// Breite in Pixel.
    pub width: u32,
    /// Höhe in Pixel.
    pub height: u32,
    /// RGBA-Daten.
    pub rgba: Vec<u8>,
    /// Bild hat Transparenz.
    pub has_alpha: bool,
    /// SHA-256 der Originaldatei (Cache-Schlüssel).
    pub source_sha256: [u8; 32],
    /// Farbtyp der Quelldatei (additiv, Welle 3; [`SourceColor`]).
    pub source_color: SourceColor,
}

impl std::fmt::Debug for Emblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emblem")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("has_alpha", &self.has_alpha)
            .field("source_color", &self.source_color)
            .finish()
    }
}

/// Erkanntes Eingabeformat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Png,
    Jpeg,
    Gif,
}

fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Format::Gif)
    } else {
        None
    }
}

fn img_err(e: impl std::fmt::Display) -> SigBlockError {
    SigBlockError::Image(e.to_string())
}

fn check_size(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 {
        return Err(img_err("leeres Bild"));
    }
    if u64::from(w) * u64::from(h) > MAX_DECODED_PIXELS {
        return Err(img_err(format!("Bild zu groß ({w}×{h} px)")));
    }
    Ok(())
}

/// Dekodiertes Rohbild.
struct Raw {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    has_alpha: bool,
    source_color: SourceColor,
}

fn decode_png(bytes: &[u8]) -> Result<Raw> {
    let mut dec = png::Decoder::new(Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(img_err)?;
    let (w, h) = (reader.info().width, reader.info().height);
    let packed_palette = reader.info().color_type == png::ColorType::Indexed
        && matches!(reader.info().bit_depth, png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four);
    // Farbtyp laut IHDR (vor EXPAND), wie Java ImageIO ihn sieht: Graustufen unter 8 Bit liest
    // ImageIO als Palettenbild (`TYPE_BYTE_BINARY`, IndexColorModel ohne lineares CS_GRAY)
    let source_color = match (reader.info().color_type, reader.info().bit_depth) {
        (png::ColorType::Rgba, _) => SourceColor::RgbAlpha,
        (png::ColorType::Rgb, _) => SourceColor::Rgb,
        (png::ColorType::GrayscaleAlpha, _) => SourceColor::GrayAlpha,
        (png::ColorType::Grayscale, png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four) => {
            SourceColor::Indexed
        }
        (png::ColorType::Grayscale, _) => SourceColor::Gray,
        (png::ColorType::Indexed, _) => SourceColor::Indexed,
    };
    check_size(w, h)?;
    let size = reader.output_buffer_size().ok_or_else(|| img_err("PNG zu groß"))?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(img_err)?;
    buf.truncate(info.buffer_size());
    let px = (w as usize) * (h as usize);
    let (rgba, has_alpha) = match info.color_type {
        png::ColorType::Rgba => (buf, true),
        png::ColorType::Rgb => (buf.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(), false),
        png::ColorType::GrayscaleAlpha => {
            (buf.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(), true)
        }
        png::ColorType::Grayscale => (buf.iter().flat_map(|&g| [g, g, g, 255]).collect(), false),
        png::ColorType::Indexed => return Err(img_err("PNG-Palette nicht expandiert")),
    };
    if rgba.len() != px * 4 {
        return Err(img_err("PNG-Daten unvollständig"));
    }
    let mut rgba = rgba;
    if packed_palette {
        java_packed_palette_transparency(&mut rgba);
    }
    Ok(Raw { width: w, height: h, rgba, has_alpha, source_color })
}

/// Palettenbilder mit höchstens 16 Farben (PNG-Palette mit 1/2/4 Bit, GIF mit ≤ 16 Einträgen) liest
/// Java ImageIO gepackt (`TYPE_BYTE_BINARY`); wird ihre Transparenz verworfen (PDF-AS
/// `removeAlphaChannel`, SB-G3/SB-G6), zeigen voll transparente Pixel **Schwarz**, bei 8-Bit-
/// Paletten dagegen die gespeicherte Farbe (geprüft mit PDF-AS 4.4.2/Java 21). Die Farbe voll
/// transparenter Pixel ist sonst nirgends sichtbar (SMask bzw. Komposition auf Weiß); hier wird
/// sie deshalb gleich beim Dekodieren wie bei Java auf Schwarz gesetzt.
fn java_packed_palette_transparency(rgba: &mut [u8]) {
    for p in rgba.as_chunks_mut::<4>().0 {
        if p[3] == 0 {
            *p = [0, 0, 0, 0];
        }
    }
}

fn decode_jpeg(bytes: &[u8]) -> Result<Raw> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let opts = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_max_width(1 << 15)
        .set_max_height(1 << 15);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), opts);
    dec.decode_headers().map_err(img_err)?;
    let info = dec.info().ok_or_else(|| img_err("JPEG ohne Kopf"))?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    check_size(w, h)?;
    // Graustufen-JPEG liest ImageIO als TYPE_BYTE_GRAY (SB-G4)
    let source_color = if info.components == 1 { SourceColor::Gray } else { SourceColor::Rgb };
    let rgb = dec.decode().map_err(img_err)?;
    let px = (w as usize) * (h as usize);
    if rgb.len() != px * 3 {
        return Err(img_err("JPEG-Daten unvollständig"));
    }
    let rgba = rgb.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    Ok(Raw { width: w, height: h, rgba, has_alpha: false, source_color })
}

fn decode_gif(bytes: &[u8]) -> Result<Raw> {
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut dec = opts.read_info(Cursor::new(bytes)).map_err(img_err)?;
    let global_entries = dec.global_palette().map(|p| p.len() / 3);
    let frame = dec.read_next_frame().map_err(img_err)?.ok_or_else(|| img_err("GIF ohne Bild"))?;
    let (w, h) = (u32::from(frame.width), u32::from(frame.height));
    check_size(w, h)?;
    let mut rgba = frame.buffer.to_vec();
    // Java: höchstens 16 Paletteneinträge ⇒ gepackt (`java_packed_palette_transparency`)
    let entries = frame.palette.as_ref().map(|p| p.len() / 3).or(global_entries).unwrap_or(256);
    if entries <= 16 {
        java_packed_palette_transparency(&mut rgba);
    }
    if rgba.len() != (w as usize) * (h as usize) * 4 {
        return Err(img_err("GIF-Daten unvollständig"));
    }
    let has_alpha = frame.transparent.is_some() || rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255);
    Ok(Raw { width: w, height: h, rgba, has_alpha, source_color: SourceColor::Indexed })
}

fn decode_raw(bytes: &[u8]) -> Result<(Raw, Format)> {
    let format = sniff(bytes).ok_or_else(|| img_err("unbekanntes Bildformat (PNG, JPEG, GIF)"))?;
    let raw = match format {
        Format::Png => decode_png(bytes)?,
        Format::Jpeg => decode_jpeg(bytes)?,
        Format::Gif => decode_gif(bytes)?,
    };
    Ok((raw, format))
}

/// EXIF-Orientierung (1–8) einer JPEG-Datei; ungültig/fehlend ⇒ 1.
fn exif_orientation(bytes: &[u8]) -> u32 {
    let mut cursor = Cursor::new(bytes);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut cursor) else {
        return 1;
    };
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .filter(|o| (1..=8).contains(o))
        .unwrap_or(1)
}

/// Dreht im Uhrzeigersinn um `quarters` Vierteldrehungen.
fn rotate_cw(raw: Raw, quarters: u32) -> Raw {
    let (w, h) = (raw.width as usize, raw.height as usize);
    match quarters % 4 {
        0 => raw,
        2 => {
            let mut out = vec![0u8; raw.rgba.len()];
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = ((h - 1 - y) * w + (w - 1 - x)) * 4;
                    out[dst..dst + 4].copy_from_slice(&raw.rgba[src..src + 4]);
                }
            }
            Raw { rgba: out, ..raw }
        }
        q => {
            // neues Bild h×w; 90° im UZS: (x, y) → (h−1−y, x); 270°: (x, y) → (y, w−1−x)
            let mut out = vec![0u8; raw.rgba.len()];
            for y in 0..h {
                for x in 0..w {
                    let (nx, ny) = if q == 1 { (h - 1 - y, x) } else { (y, w - 1 - x) };
                    let src = (y * w + x) * 4;
                    let dst = (ny * h + nx) * 4;
                    out[dst..dst + 4].copy_from_slice(&raw.rgba[src..src + 4]);
                }
            }
            Raw { width: raw.height, height: raw.width, rgba: out, ..raw }
        }
    }
}

fn mirror_lr(mut raw: Raw) -> Raw {
    let w = raw.width as usize;
    for row in raw.rgba.chunks_exact_mut(w * 4) {
        for x in 0..w / 2 {
            let (a, b) = (x * 4, (w - 1 - x) * 4);
            for i in 0..4 {
                row.swap(a + i, b + i);
            }
        }
    }
    raw
}

/// `EXIFRotation` von PDF-Over: (Vierteldrehungen im UZS, danach links/rechts spiegeln).
fn apply_orientation(raw: Raw, orientation: u32) -> Raw {
    let (quarters, mirror) = match orientation {
        2 => (0, true),
        3 => (2, false),
        4 => (2, true),
        5 => (1, true),
        6 => (1, false),
        7 => (3, true),
        8 => (3, false),
        _ => (0, false),
    };
    let rotated = rotate_cw(raw, quarters);
    if mirror { mirror_lr(rotated) } else { rotated }
}

/// `Emblem.reduceImageSizeIfNecessary` (PDF-Over): erst Höhe, dann Breite begrenzen.
fn reduced_size(w: u32, h: u32) -> (u32, u32) {
    let ratio = f64::from(w) / f64::from(h);
    let (mut nw, mut nh) = (w, h);
    if nh > MAX_EMBLEM_HEIGHT {
        nh = MAX_EMBLEM_HEIGHT;
        nw = (f64::from(MAX_EMBLEM_HEIGHT) * ratio) as u32;
    }
    if nw > MAX_EMBLEM_WIDTH {
        nw = MAX_EMBLEM_WIDTH;
        nh = (f64::from(MAX_EMBLEM_WIDTH) / ratio) as u32;
    }
    (nw.max(1), nh.max(1))
}

fn resize_nearest(raw: Raw, nw: u32, nh: u32) -> Raw {
    if nw == raw.width && nh == raw.height {
        return raw;
    }
    let (sw, sh) = (raw.width as u64, raw.height as u64);
    let mut out = Vec::with_capacity((nw as usize) * (nh as usize) * 4);
    for y in 0..u64::from(nh) {
        // Pixelmitte: floor((y + 0.5) · sh / nh)
        let sy = ((2 * y + 1) * sh / (2 * u64::from(nh))).min(sh - 1);
        for x in 0..u64::from(nw) {
            let sx = ((2 * x + 1) * sw / (2 * u64::from(nw))).min(sw - 1);
            let i = ((sy * sw + sx) * 4) as usize;
            out.extend_from_slice(&raw.rgba[i..i + 4]);
        }
    }
    Raw { width: nw, height: nh, rgba: out, ..raw }
}

/// Palette (höchstens 256 RGBA-Farben, in Reihenfolge des ersten Auftretens) und Indizes.
fn palette_of(px: &[[u8; 4]]) -> Option<(Vec<[u8; 4]>, Vec<u8>)> {
    let mut colors: Vec<[u8; 4]> = Vec::new();
    let mut lookup = std::collections::HashMap::new();
    let mut indices = Vec::with_capacity(px.len());
    for p in px {
        let i = match lookup.get(p) {
            Some(&i) => i,
            None => {
                let i = u8::try_from(colors.len()).ok()?;
                colors.push(*p);
                lookup.insert(*p, i);
                i
            }
        };
        indices.push(i);
    }
    Some((colors, indices))
}

/// Farbschlüssel (`tRNS` bei RGB bzw. Graustufen): genau eine Farbe ist vollständig transparent,
/// alle anderen Pixel sind deckend und haben eine andere Farbe.
fn color_key(px: &[[u8; 4]]) -> Option<[u8; 3]> {
    let key = px.iter().find(|p| p[3] == 0).map(|p| [p[0], p[1], p[2]])?;
    px.iter().all(|p| if [p[0], p[1], p[2]] == key { p[3] == 0 } else { p[3] == 255 }).then_some(key)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

impl Emblem {
    /// Dekodiert PNG/JPEG/GIF inklusive EXIF-Ausrichtung (nur JPEG, wie PDF-Over) und
    /// Verkleinerung auf ≤ 480×600 px.
    pub fn decode(bytes: &[u8]) -> crate::Result<Emblem> {
        let (raw, format) = decode_raw(bytes)?;
        let raw = if format == Format::Jpeg { apply_orientation(raw, exif_orientation(bytes)) } else { raw };
        let (nw, nh) = reduced_size(raw.width, raw.height);
        let raw = resize_nearest(raw, nw, nh);
        Ok(Emblem::from_raw(raw, sha256(bytes)))
    }

    /// Dekodiert ein Profilbild wie PDF-AS (`ImageIO.read`): ohne EXIF, ohne Verkleinerung.
    pub fn decode_original(bytes: &[u8]) -> crate::Result<Emblem> {
        let (raw, _) = decode_raw(bytes)?;
        Ok(Emblem::from_raw(raw, sha256(bytes)))
    }

    /// Konsistenz der (öffentlichen) Felder: Maße > 0 und innerhalb der Pixelgrenze, RGBA-Puffer
    /// passend. Layout und Appearance prüfen das, bevor sie ein Emblem verwenden.
    pub(crate) fn check(&self) -> Result<()> {
        check_size(self.width, self.height)?;
        if self.rgba.len() as u64 != u64::from(self.width) * u64::from(self.height) * 4 {
            return Err(img_err(format!("RGBA-Puffer passt nicht zu {}×{}", self.width, self.height)));
        }
        Ok(())
    }

    fn from_raw(raw: Raw, source_sha256: [u8; 32]) -> Emblem {
        let has_alpha = raw.has_alpha && raw.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255);
        let source_color = raw.source_color;
        Emblem { width: raw.width, height: raw.height, rgba: raw.rgba, has_alpha, source_sha256, source_color }
    }

    /// Aus bereits dekodiertem RGBA (z. B. HEIC über die Shell); verkleinert wie [`Emblem::decode`].
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>, source_sha256: [u8; 32]) -> crate::Result<Emblem> {
        check_size(width, height)?;
        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(img_err(format!("RGBA-Puffer passt nicht zu {width}×{height}")));
        }
        let raw = Raw { width, height, rgba, has_alpha: true, source_color: SourceColor::RgbAlpha };
        let (nw, nh) = reduced_size(width, height);
        Ok(Emblem::from_raw(resize_nearest(raw, nw, nh), source_sha256))
    }

    /// Alphakanal entfernen (PDF/A-1): Komposition auf Weiß (Register-Eintrag).
    pub fn flattened_on_white(&self) -> Emblem {
        let rgba = self
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let a = u32::from(p[3]);
                let mix = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
                [mix(p[0]), mix(p[1]), mix(p[2]), 255]
            })
            .collect();
        Emblem { rgba, has_alpha: false, ..self.clone() }
    }

    /// PNG für den Cache im Konfigurationsordner (8 Bit). Der Farbtyp der Quelle
    /// ([`Emblem::source_color`]) bleibt beim erneuten Dekodieren erhalten, wie bei PDF-Over
    /// `ImageIO.write` – sonst bettete `pdfas_quirks` die Bildmarke aus dem Cache anders ein als
    /// aus der Originaldatei (SB-G2 … SB-G4): Palette (mit `tRNS`) bleibt Palette, Graustufen
    /// bleiben Graustufen, ein Alphakanal bleibt auch voll deckend erhalten, ein Farbschlüssel
    /// (`tRNS` bei RGB/Graustufen) bleibt Farbschlüssel. Lässt sich der Inhalt so nicht darstellen
    /// (z. B. mehr als 256 Farben nach dem Verkleinern), gilt RGB bzw. RGBA.
    pub fn to_png(&self) -> crate::Result<Vec<u8>> {
        let px = self.rgba.as_chunks::<4>().0;
        let gray = px.iter().all(|p| p[0] == p[1] && p[1] == p[2]);
        let palette = if self.source_color == SourceColor::Indexed { palette_of(px) } else { None };
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width, self.height);
            enc.set_depth(png::BitDepth::Eight);
            let alpha_channel = self.source_color.has_alpha_channel() || self.has_alpha;
            let key = if self.has_alpha && !self.source_color.has_alpha_channel() { color_key(px) } else { None };
            let data: Vec<u8> = match (self.source_color, palette) {
                (SourceColor::Indexed, Some((colors, indices))) => {
                    enc.set_color(png::ColorType::Indexed);
                    enc.set_palette(colors.iter().flat_map(|c| [c[0], c[1], c[2]]).collect::<Vec<u8>>());
                    if colors.iter().any(|c| c[3] != 255) {
                        enc.set_trns(colors.iter().map(|c| c[3]).collect::<Vec<u8>>());
                    }
                    indices
                }
                (SourceColor::Gray | SourceColor::GrayAlpha, _) if gray => match key {
                    Some(k) if self.source_color == SourceColor::Gray => {
                        enc.set_color(png::ColorType::Grayscale);
                        enc.set_trns(vec![0, k[0]]);
                        px.iter().map(|p| p[0]).collect()
                    }
                    _ if alpha_channel => {
                        enc.set_color(png::ColorType::GrayscaleAlpha);
                        px.iter().flat_map(|p| [p[0], p[3]]).collect()
                    }
                    _ => {
                        enc.set_color(png::ColorType::Grayscale);
                        px.iter().map(|p| p[0]).collect()
                    }
                },
                _ => match key {
                    Some(k) if self.source_color == SourceColor::Rgb => {
                        enc.set_color(png::ColorType::Rgb);
                        enc.set_trns(vec![0, k[0], 0, k[1], 0, k[2]]);
                        self.rgb()
                    }
                    _ if alpha_channel => {
                        enc.set_color(png::ColorType::Rgba);
                        self.rgba.clone()
                    }
                    _ => {
                        enc.set_color(png::ColorType::Rgb);
                        self.rgb()
                    }
                },
            };
            let mut w = enc.write_header().map_err(img_err)?;
            w.write_image_data(&data).map_err(img_err)?;
            w.finish().map_err(img_err)?;
        }
        Ok(out)
    }

    /// RGB-Ebene (ohne Alpha).
    pub fn rgb(&self) -> Vec<u8> {
        self.rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()
    }

    /// Alpha-Ebene.
    pub fn alpha(&self) -> Vec<u8> {
        self.rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(w: u32, h: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(color);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(data).unwrap();
        wr.finish().unwrap();
        out
    }

    #[test]
    fn reduce_like_pdf_over() {
        assert_eq!(reduced_size(326, 342), (326, 342));
        assert_eq!(reduced_size(1000, 1000), (480, 480));
        assert_eq!(reduced_size(400, 1200), (200, 600));
        assert_eq!(reduced_size(2000, 500), (480, 120));
        // erst Höhe: 700×800 → 525×600 → 480×548
        assert_eq!(reduced_size(700, 800), (480, 548));
    }

    #[test]
    fn decode_png_rgba_and_flatten() {
        let data = [255, 0, 0, 0, 0, 255, 0, 128, 0, 0, 255, 255, 10, 20, 30, 255];
        let bytes = png_bytes(2, 2, png::ColorType::Rgba, &data);
        let e = Emblem::decode(&bytes).unwrap();
        assert_eq!((e.width, e.height), (2, 2));
        assert!(e.has_alpha);
        assert_eq!(e.rgba, data);
        let f = e.flattened_on_white();
        assert!(!f.has_alpha);
        assert_eq!(&f.rgba[0..4], &[255, 255, 255, 255]);
        assert_eq!(&f.rgba[4..8], &[127, 255, 127, 255]);
        assert_eq!(&f.rgba[12..16], &[10, 20, 30, 255]);
        let png2 = f.to_png().unwrap();
        assert_eq!(Emblem::decode(&png2).unwrap().rgba, f.rgba);
    }

    #[test]
    fn decode_png_rgb_opaque() {
        let bytes = png_bytes(1, 1, png::ColorType::Rgb, &[1, 2, 3]);
        let e = Emblem::decode(&bytes).unwrap();
        assert!(!e.has_alpha);
        assert_eq!(e.rgba, vec![1, 2, 3, 255]);
        assert_eq!(e.source_sha256, sha256(&bytes));
    }

    #[test]
    fn opaque_rgba_is_not_alpha() {
        let bytes = png_bytes(1, 1, png::ColorType::Rgba, &[1, 2, 3, 255]);
        assert!(!Emblem::decode(&bytes).unwrap().has_alpha);
    }

    #[test]
    fn large_png_is_reduced() {
        let (w, h) = (100u32, 1200u32);
        let bytes = png_bytes(w, h, png::ColorType::Grayscale, &vec![200u8; (w * h) as usize]);
        let e = Emblem::decode(&bytes).unwrap();
        assert_eq!((e.width, e.height), (50, 600));
        let o = Emblem::decode_original(&bytes).unwrap();
        assert_eq!((o.width, o.height), (100, 1200));
    }

    #[test]
    fn orientation_mapping() {
        // 2×1: rot, grün
        let raw = || Raw {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
            has_alpha: false,
            source_color: SourceColor::Rgb,
        };
        let r6 = apply_orientation(raw(), 6);
        assert_eq!((r6.width, r6.height), (1, 2));
        assert_eq!(&r6.rgba[0..4], &[255, 0, 0, 255]); // 90° UZS: links wird oben
        let r8 = apply_orientation(raw(), 8);
        assert_eq!(&r8.rgba[0..4], &[0, 255, 0, 255]); // 270° UZS: rechts wird oben
        let r2 = apply_orientation(raw(), 2);
        assert_eq!(&r2.rgba[0..4], &[0, 255, 0, 255]);
        let r3 = apply_orientation(raw(), 3);
        assert_eq!(&r3.rgba[0..4], &[0, 255, 0, 255]);
        // 5 = transpose: (x, y) → (y, x)
        let r5 = apply_orientation(raw(), 5);
        assert_eq!((r5.width, r5.height), (1, 2));
        assert_eq!(&r5.rgba[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn rejects_garbage_and_bad_buffers() {
        assert!(Emblem::decode(b"not an image").is_err());
        assert!(Emblem::from_rgba(2, 2, vec![0; 3], [0; 32]).is_err());
        assert!(Emblem::from_rgba(0, 2, vec![], [0; 32]).is_err());
        let e = Emblem::from_rgba(1, 1, vec![9, 9, 9, 255], [7; 32]).unwrap();
        assert!(!e.has_alpha);
    }

    #[test]
    fn check_rejects_inconsistent_fields() {
        let ok = Emblem::from_rgba(2, 1, vec![0; 8], [0; 32]).unwrap();
        assert!(ok.check().is_ok());
        let short = Emblem { rgba: vec![0; 7], ..ok.clone() };
        assert!(short.check().is_err());
        let empty = Emblem { width: 0, rgba: Vec::new(), ..ok };
        assert!(empty.check().is_err());
    }

    #[test]
    fn source_color_from_the_file() {
        let png = |color, data: &[u8]| Emblem::decode(&png_bytes(1, 1, color, data)).unwrap().source_color;
        assert_eq!(png(png::ColorType::Rgb, &[1, 2, 3]), SourceColor::Rgb);
        assert_eq!(png(png::ColorType::Rgba, &[1, 2, 3, 255]), SourceColor::RgbAlpha);
        assert_eq!(png(png::ColorType::Grayscale, &[7]), SourceColor::Gray);
        // Graustufen unter 8 Bit: für Java ein Palettenbild (keine Aufhellung bei PDF-AS)
        for depth in [png::BitDepth::One, png::BitDepth::Two, png::BitDepth::Four] {
            let mut out = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut out, 1, 1);
                enc.set_color(png::ColorType::Grayscale);
                enc.set_depth(depth);
                let mut w = enc.write_header().unwrap();
                w.write_image_data(&[0x80]).unwrap();
            }
            assert_eq!(Emblem::decode(&out).unwrap().source_color, SourceColor::Indexed, "{depth:?}");
        }
        assert_eq!(png(png::ColorType::GrayscaleAlpha, &[7, 255]), SourceColor::GrayAlpha);
        assert!(SourceColor::RgbAlpha.has_alpha_channel() && SourceColor::GrayAlpha.has_alpha_channel());
        assert!(!SourceColor::Indexed.has_alpha_channel() && !SourceColor::Gray.has_alpha_channel());
        // Palette mit tRNS: Transparenz im Inhalt, kein eigener Alphakanal
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 1);
            enc.set_color(png::ColorType::Indexed);
            enc.set_depth(png::BitDepth::Eight);
            enc.set_palette(vec![255, 0, 0, 0, 0, 255]);
            enc.set_trns(vec![0, 255]);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[0, 1]).unwrap();
        }
        let e = Emblem::decode(&out).unwrap();
        assert_eq!(e.source_color, SourceColor::Indexed);
        assert!(e.has_alpha);
        assert_eq!(e.rgba, vec![255, 0, 0, 0, 0, 0, 255, 255]);
        // RGBA aus der Shell, Verkleinerung und Drehung erhalten den Farbtyp
        assert_eq!(Emblem::from_rgba(1, 1, vec![0, 0, 0, 255], [0; 32]).unwrap().source_color, SourceColor::RgbAlpha);
        let big = png_bytes(10, 1300, png::ColorType::Grayscale, &vec![9; 13000]);
        let g = Emblem::decode(&big).unwrap();
        assert_eq!(g.source_color, SourceColor::Gray);
        // Cache-PNG behält den Farbtyp
        let cached = Emblem::decode(&g.to_png().unwrap()).unwrap();
        assert_eq!((cached.source_color, cached.rgba.clone()), (SourceColor::Gray, g.rgba.clone()));
        assert!(format!("{e:?}").contains("Indexed"));
        assert_eq!(e.flattened_on_white().source_color, SourceColor::Indexed);
    }

    /// Der Cache (`to_png` → `decode`) erhält Farbtyp, Pixel und Transparenz jeder Quelle – sonst
    /// bettete `pdfas_quirks` eine Bildmarke aus dem Cache anders ein (SB-G2 … SB-G4).
    #[test]
    fn cache_png_keeps_the_source_color() {
        let encode = |w: u32, color, palette: Option<(Vec<u8>, Vec<u8>)>, trns: Option<Vec<u8>>, data: &[u8]| {
            let mut out = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut out, w, 1);
                enc.set_color(color);
                enc.set_depth(png::BitDepth::Eight);
                if let Some((p, t)) = palette {
                    enc.set_palette(p);
                    enc.set_trns(t);
                }
                if let Some(t) = trns {
                    enc.set_trns(t);
                }
                let mut wr = enc.write_header().unwrap();
                wr.write_image_data(data).unwrap();
            }
            out
        };
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("RGB", encode(2, png::ColorType::Rgb, None, None, &[1, 2, 3, 4, 5, 6])),
            ("RGBA deckend (SB-G2)", encode(2, png::ColorType::Rgba, None, None, &[1, 2, 3, 255, 4, 5, 6, 255])),
            ("RGBA", encode(2, png::ColorType::Rgba, None, None, &[1, 2, 3, 0, 4, 5, 6, 128])),
            ("RGB mit tRNS", encode(2, png::ColorType::Rgb, None, Some(vec![0, 1, 0, 2, 0, 3]), &[1, 2, 3, 4, 5, 6])),
            ("Grau", encode(2, png::ColorType::Grayscale, None, None, &[20, 200])),
            ("Grau mit tRNS", encode(2, png::ColorType::Grayscale, None, Some(vec![0, 20]), &[20, 200])),
            ("Grau+Alpha deckend", encode(2, png::ColorType::GrayscaleAlpha, None, None, &[20, 255, 200, 255])),
            ("Grau+Alpha", encode(2, png::ColorType::GrayscaleAlpha, None, None, &[20, 0, 200, 99])),
            (
                "Palette mit tRNS (SB-G3)",
                encode(3, png::ColorType::Indexed, Some((vec![255, 0, 0, 0, 0, 255], vec![0, 255])), None, &[0, 1, 1]),
            ),
            (
                "Palette",
                encode(2, png::ColorType::Indexed, Some((vec![9, 8, 7, 1, 2, 3], vec![255, 255])), None, &[1, 0]),
            ),
        ];
        for (label, bytes) in cases {
            let e = Emblem::decode(&bytes).unwrap();
            let back = Emblem::decode(&e.to_png().unwrap()).unwrap();
            assert_eq!(
                (back.source_color, back.has_alpha, &back.rgba),
                (e.source_color, e.has_alpha, &e.rgba),
                "{label}"
            );
        }
        // mehr als 256 Farben einer Palettenquelle (z. B. nach Bearbeitung): RGB(A) statt Palette
        let mut many = Emblem::decode(&encode(
            2,
            png::ColorType::Indexed,
            Some((vec![1, 1, 1, 2, 2, 2], vec![255, 255])),
            None,
            &[0, 1],
        ))
        .unwrap();
        many.width = 300;
        many.rgba = (0..300u32).flat_map(|i| [(i % 256) as u8, (i / 256) as u8, 0, 255]).collect();
        let back = Emblem::decode(&many.to_png().unwrap()).unwrap();
        assert_eq!((back.source_color, &back.rgba), (SourceColor::Rgb, &many.rgba));
    }

    #[test]
    fn java_gray_conversion_brightens_like_cs_gray() {
        // Stützwerte aus dem Golden-Lauf (PDF-AS 4.4.2, emblem-tall.png) bzw. sRGB-Definition
        assert_eq!(java_linear_gray_to_srgb(0), 0);
        assert_eq!(java_linear_gray_to_srgb(20), 79);
        assert_eq!(java_linear_gray_to_srgb(255), 255);
        assert_eq!(java_linear_gray_to_srgb(1), 13);
        let lut: Vec<u8> = (0..=255).map(java_linear_gray_to_srgb).collect();
        assert!(lut.windows(2).all(|w| w[0] <= w[1]), "monoton");
        assert!((1..255).all(|v| java_linear_gray_to_srgb(v) >= v), "hellt auf");
    }

    #[test]
    fn builtin_logo_dimensions() {
        let de = Emblem::decode_original(crate::assets::image("signatur-logo_de.png").unwrap()).unwrap();
        assert_eq!((de.width, de.height), (326, 342));
        let amts = Emblem::decode_original(crate::assets::image("amtsignatur-logo_demo.png").unwrap()).unwrap();
        assert_eq!((amts.width, amts.height), (236, 235));
    }
}
