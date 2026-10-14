//! Dekodierung von Bild-XObjects in Luma bzw. RGBA (für die QR-Erkennung, docs/analyse/04 §5.1).
//!
//! Unterstützt: Flate/LZW (mit Predictor), ASCII85/ASCIIHex/RunLength, DCT (zune-jpeg);
//! Farbräume DeviceGray/RGB/CMYK, CalGray/CalRGB, ICCBased (über `/N`), Indexed, Lab (nur L*),
//! Separation/DeviceN (Tönung als Grau); 1/2/4/8/16 Bit je Komponente, `/Decode`, `/ImageMask`,
//! `/SMask` (als Alpha). JPX, JBIG2 und CCITT werden abgelehnt.
//!
//! `Debug` von [`DecodedImage`] zeigt nur Maße, nie Pixel (ADR 0001 Regel 9).

use lopdf::{Dictionary, Document, Object, Stream};

use crate::filters::{self, ImageFilter};
use crate::pages::{number, resolve};
use crate::{CosError, Limits, Result};

/// Pixelformat von [`DecodedImage::data`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PixelFormat {
    /// 1 Byte Helligkeit je Pixel
    Luma8,
    /// 4 Byte R, G, B, A je Pixel (A aus `/SMask`, sonst 255)
    Rgba8,
}

/// Dekodiertes Bild, zeilenweise von oben nach unten.
#[derive(Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Breite in Pixel
    pub width: u32,
    /// Höhe in Pixel
    pub height: u32,
    /// Pixelformat
    pub format: PixelFormat,
    /// Pixeldaten
    pub data: Vec<u8>,
}

impl std::fmt::Debug for DecodedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &self.format)
            .field("len", &self.data.len())
            .finish()
    }
}

#[inline]
fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) + 500) / 1000) as u8
}

impl DecodedImage {
    /// Helligkeit je Pixel; Transparenz wird über Weiß verrechnet (Papierhintergrund).
    pub fn to_luma(&self) -> Vec<u8> {
        match self.format {
            PixelFormat::Luma8 => self.data.clone(),
            PixelFormat::Rgba8 => self
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| {
                    let y = u32::from(luma(p[0], p[1], p[2]));
                    let a = u32::from(p[3]);
                    ((y * a + 255 * (255 - a) + 127) / 255) as u8
                })
                .collect(),
        }
    }

    /// RGBA je Pixel.
    pub fn to_rgba(&self) -> Vec<u8> {
        match self.format {
            PixelFormat::Rgba8 => self.data.clone(),
            PixelFormat::Luma8 => self.data.iter().flat_map(|&y| [y, y, y, 255]).collect(),
        }
    }
}

/// Farbraum eines Bildes.
#[derive(Debug, Clone, PartialEq)]
enum ColorSpace {
    Gray,
    Rgb,
    Cmyk,
    Lab,
    Indexed { base: Box<ColorSpace>, hival: u32, lookup: Vec<u8> },
    Tint(usize),
}

impl ColorSpace {
    fn components(&self) -> usize {
        match self {
            ColorSpace::Gray | ColorSpace::Indexed { .. } => 1,
            ColorSpace::Rgb | ColorSpace::Lab => 3,
            ColorSpace::Cmyk => 4,
            ColorSpace::Tint(n) => *n,
        }
    }

    fn is_gray_output(&self) -> bool {
        match self {
            ColorSpace::Gray | ColorSpace::Lab | ColorSpace::Tint(_) => true,
            ColorSpace::Indexed { base, .. } => base.is_gray_output(),
            _ => false,
        }
    }
}

fn unsupported(what: &str) -> CosError {
    CosError::Parse(format!("Bild nicht dekodierbar: {what}"))
}

fn parse_color_space(doc: &Document, obj: &Object, limits: &Limits, depth: u32) -> Result<ColorSpace> {
    if depth > 8 {
        return Err(unsupported("Farbraum zu tief verschachtelt"));
    }
    let obj = resolve(doc, obj, 8).ok_or_else(|| unsupported("Farbraum fehlt"))?;
    match obj {
        Object::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"G" | b"CalGray" => Ok(ColorSpace::Gray),
            b"DeviceRGB" | b"RGB" | b"CalRGB" => Ok(ColorSpace::Rgb),
            b"DeviceCMYK" | b"CMYK" => Ok(ColorSpace::Cmyk),
            other => Err(unsupported(&format!("Farbraum {}", String::from_utf8_lossy(other)))),
        },
        Object::Array(a) => {
            let family = a.first().and_then(|o| resolve(doc, o, 8)).and_then(|o| o.as_name().ok()).unwrap_or(b"");
            match family {
                b"CalGray" | b"DeviceGray" | b"G" => Ok(ColorSpace::Gray),
                b"CalRGB" | b"DeviceRGB" | b"RGB" => Ok(ColorSpace::Rgb),
                b"DeviceCMYK" | b"CMYK" => Ok(ColorSpace::Cmyk),
                b"Lab" => Ok(ColorSpace::Lab),
                b"ICCBased" => {
                    let Some(Object::Stream(s)) = a.get(1).and_then(|o| resolve(doc, o, 8)) else {
                        return Err(unsupported("ICCBased ohne Stream"));
                    };
                    match s.dict.get(b"N").ok().and_then(|o| number(doc, o)).map(|n| n as i64) {
                        Some(1) => Ok(ColorSpace::Gray),
                        Some(3) => Ok(ColorSpace::Rgb),
                        Some(4) => Ok(ColorSpace::Cmyk),
                        _ => match s.dict.get(b"Alternate") {
                            Ok(alt) => parse_color_space(doc, alt, limits, depth + 1),
                            Err(_) => Err(unsupported("ICCBased ohne /N")),
                        },
                    }
                }
                b"Indexed" | b"I" => {
                    let base = parse_color_space(doc, a.get(1).unwrap_or(&Object::Null), limits, depth + 1)?;
                    if matches!(base, ColorSpace::Indexed { .. }) {
                        return Err(unsupported("Indexed über Indexed"));
                    }
                    let hival = a.get(2).and_then(|o| number(doc, o)).unwrap_or(0.0).clamp(0.0, 255.0) as u32;
                    let lookup = match a.get(3).and_then(|o| resolve(doc, o, 8)) {
                        Some(Object::String(s, _)) => s.clone(),
                        Some(Object::Stream(s)) => filters::decode_stream(doc, s, limits, false)?.data,
                        _ => return Err(unsupported("Indexed ohne Farbtabelle")),
                    };
                    Ok(ColorSpace::Indexed { base: Box::new(base), hival, lookup })
                }
                b"Separation" => Ok(ColorSpace::Tint(1)),
                b"DeviceN" => {
                    let n = a
                        .get(1)
                        .and_then(|o| resolve(doc, o, 8))
                        .and_then(|o| o.as_array().ok())
                        .map_or(1, |v| v.len().max(1));
                    Ok(ColorSpace::Tint(n))
                }
                other => Err(unsupported(&format!("Farbraum {}", String::from_utf8_lossy(other)))),
            }
        }
        _ => Err(unsupported("Farbraum ungültig")),
    }
}

fn int_of(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<i64> {
    d.get(key).ok().and_then(|o| number(doc, o)).map(|v| v as i64)
}

fn decode_array(doc: &Document, d: &Dictionary) -> Option<Vec<f32>> {
    let a = resolve(doc, d.get(b"Decode").ok()?, 8)?.as_array().ok()?;
    a.iter().map(|o| number(doc, o)).collect()
}

/// Zieht `bpc`-Bit-Samples zeilenweise auseinander (Zeilen auf Byte-Grenzen aufgefüllt).
fn unpack_samples(data: &[u8], width: usize, height: usize, comps: usize, bpc: usize) -> Vec<u16> {
    let per_row = width * comps;
    let row_bytes = (per_row * bpc).div_ceil(8);
    let mut out = Vec::with_capacity(per_row * height);
    for y in 0..height {
        let row_start = y * row_bytes;
        for i in 0..per_row {
            let v = match bpc {
                8 => u16::from(*data.get(row_start + i).unwrap_or(&0)),
                16 => {
                    let p = row_start + 2 * i;
                    u16::from_be_bytes([*data.get(p).unwrap_or(&0), *data.get(p + 1).unwrap_or(&0)])
                }
                1 | 2 | 4 => {
                    let bit = i * bpc;
                    let byte = *data.get(row_start + bit / 8).unwrap_or(&0);
                    let shift = 8 - bpc - (bit % 8);
                    u16::from((byte >> shift) & ((1u8 << bpc) - 1))
                }
                _ => 0,
            };
            out.push(v);
        }
    }
    out
}

fn cmyk_to_rgb(c: f32, m: f32, y: f32, k: f32) -> [u8; 3] {
    let f = |x: f32| ((1.0 - x.clamp(0.0, 1.0)) * (1.0 - k.clamp(0.0, 1.0)) * 255.0 + 0.5) as u8;
    [f(c), f(m), f(y)]
}

/// Wandelt normalisierte Komponenten (0..1) eines Pixels in RGB.
fn to_rgb(cs: &ColorSpace, v: &[f32]) -> [u8; 3] {
    let u = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    match cs {
        ColorSpace::Gray => {
            let g = u(v[0]);
            [g, g, g]
        }
        ColorSpace::Rgb => [u(v[0]), u(v[1]), u(v[2])],
        ColorSpace::Cmyk => cmyk_to_rgb(v[0], v[1], v[2], v[3]),
        ColorSpace::Lab => {
            let g = u(v[0]);
            [g, g, g]
        }
        ColorSpace::Tint(_) => {
            let t: f32 = v.iter().copied().fold(0.0, f32::max);
            let g = u(1.0 - t);
            [g, g, g]
        }
        ColorSpace::Indexed { .. } => [0, 0, 0], // wird vorher aufgelöst
    }
}

pub(crate) fn decode_image(doc: &Document, stream: &Stream, limits: &Limits, with_smask: bool) -> Result<DecodedImage> {
    let d = &stream.dict;
    let image_mask =
        d.get(b"ImageMask").ok().and_then(|o| resolve(doc, o, 8)).and_then(|o| o.as_bool().ok()).unwrap_or(false);
    let decoded = filters::decode_stream(doc, stream, limits, true)?;

    let (width, height, format, data) = match decoded.image_filter {
        Some(ImageFilter { name, .. }) if name == b"DCTDecode" => decode_jpeg(doc, d, &decoded.data, limits)?,
        Some(ImageFilter { name, .. }) => {
            return Err(unsupported(&format!("Filter {}", String::from_utf8_lossy(&name))));
        }
        None => decode_raw(doc, d, &decoded.data, image_mask, limits)?,
    };
    // Länge passt zu den Maßen (Voraussetzung für die SMask-Verrechnung und für Aufrufer).
    let bpp: u64 = match format {
        PixelFormat::Luma8 => 1,
        PixelFormat::Rgba8 => 4,
    };
    if u64::from(width).saturating_mul(u64::from(height)).saturating_mul(bpp) != data.len() as u64 {
        return Err(unsupported("Pixelzahl passt nicht zu /Width und /Height"));
    }
    let mut img = DecodedImage { width, height, format, data };

    if with_smask
        && let Some(Object::Stream(sm)) = d.get(b"SMask").ok().and_then(|o| resolve(doc, o, 8))
        && let Ok(mask) = decode_image(doc, sm, limits, false)
    {
        let alpha = mask.to_luma();
        let rgba = img.to_rgba();
        let (w, h) = (img.width as usize, img.height as usize);
        let (mw, mh) = (mask.width as usize, mask.height as usize);
        let mut out = rgba;
        if mw > 0 && mh > 0 {
            for y in 0..h {
                let my = (y * mh / h.max(1)).min(mh - 1);
                for x in 0..w {
                    let mx = (x * mw / w.max(1)).min(mw - 1);
                    out[(y * w + x) * 4 + 3] = alpha[my * mw + mx];
                }
            }
        }
        img = DecodedImage { width: img.width, height: img.height, format: PixelFormat::Rgba8, data: out };
    }
    Ok(img)
}

fn check_size(width: usize, height: usize, comps: usize, limits: &Limits) -> Result<()> {
    let bytes = (width as u64).saturating_mul(height as u64).saturating_mul(comps.max(4) as u64);
    if width == 0 || height == 0 {
        return Err(unsupported("Breite oder Höhe 0"));
    }
    if bytes > limits.max_decoded_stream {
        return Err(CosError::LimitExceeded { what: "Bildgröße", limit: limits.max_decoded_stream });
    }
    Ok(())
}

fn decode_raw(
    doc: &Document,
    d: &Dictionary,
    data: &[u8],
    image_mask: bool,
    limits: &Limits,
) -> Result<(u32, u32, PixelFormat, Vec<u8>)> {
    let width = int_of(doc, d, b"Width").unwrap_or(0).max(0) as usize;
    let height = int_of(doc, d, b"Height").unwrap_or(0).max(0) as usize;
    let (cs, bpc) = if image_mask {
        (ColorSpace::Gray, 1)
    } else {
        let cs_obj = d.get(b"ColorSpace").map_err(|_| unsupported("/ColorSpace fehlt"))?;
        let cs = parse_color_space(doc, cs_obj, limits, 0)?;
        let bpc = int_of(doc, d, b"BitsPerComponent").unwrap_or(8) as usize;
        (cs, bpc)
    };
    if !matches!(bpc, 1 | 2 | 4 | 8 | 16) {
        return Err(unsupported(&format!("BitsPerComponent {bpc}")));
    }
    let comps = cs.components();
    check_size(width, height, comps, limits)?;
    let samples = unpack_samples(data, width, height, comps, bpc);
    let max = ((1u32 << bpc) - 1) as f32;
    let decode = decode_array(doc, d);
    let n = width * height;

    if image_mask {
        // Stencil: Sample 0 = gemalt (schwarz angenommen), 1 = nicht gemalt (weiß); /Decode [1 0] kehrt um.
        let inverted = decode.as_ref().is_some_and(|v| v.len() >= 2 && v[0] > v[1]);
        let data = samples.iter().map(|&s| if (s != 0) != inverted { 255 } else { 0 }).collect();
        return Ok((width as u32, height as u32, PixelFormat::Luma8, data));
    }

    if let ColorSpace::Indexed { base, hival, lookup } = &cs {
        let (dmin, dmax) = match decode.as_deref() {
            Some([a, b, ..]) => (*a, *b),
            _ => (0.0, max),
        };
        let bc = base.components();
        let gray = base.is_gray_output();
        let mut out = Vec::with_capacity(n * if gray { 1 } else { 4 });
        let mut comp = vec![0f32; bc];
        for &s in &samples {
            let idx = (dmin + f32::from(s) * (dmax - dmin) / max).round().clamp(0.0, *hival as f32) as usize;
            for (c, slot) in comp.iter_mut().enumerate() {
                *slot = f32::from(*lookup.get(idx * bc + c).unwrap_or(&0)) / 255.0;
            }
            let rgb = to_rgb(base, &comp);
            if gray {
                out.push(rgb[0]);
            } else {
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        let fmt = if gray { PixelFormat::Luma8 } else { PixelFormat::Rgba8 };
        return Ok((width as u32, height as u32, fmt, out));
    }

    let ranges: Vec<(f32, f32)> = (0..comps)
        .map(|c| match decode.as_deref() {
            Some(v) if v.len() >= 2 * (c + 1) => (v[2 * c], v[2 * c + 1]),
            _ if cs == ColorSpace::Lab && c == 0 => (0.0, 100.0),
            _ => (0.0, 1.0),
        })
        .collect();
    let gray = cs.is_gray_output();
    let mut out = Vec::with_capacity(n * if gray { 1 } else { 4 });
    let mut comp = vec![0f32; comps];
    for px in samples.chunks_exact(comps) {
        for c in 0..comps {
            let (lo, hi) = ranges[c];
            let mut v = lo + f32::from(px[c]) * (hi - lo) / max;
            if cs == ColorSpace::Lab && c == 0 {
                v /= 100.0;
            }
            comp[c] = v;
        }
        let rgb = to_rgb(&cs, &comp);
        if gray {
            out.push(rgb[0]);
        } else {
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    Ok((width as u32, height as u32, if gray { PixelFormat::Luma8 } else { PixelFormat::Rgba8 }, out))
}

fn decode_jpeg(
    doc: &Document,
    d: &Dictionary,
    data: &[u8],
    limits: &Limits,
) -> Result<(u32, u32, PixelFormat, Vec<u8>)> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace as ZCs;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let mut probe = JpegDecoder::new(ZCursor::new(data));
    probe.decode_headers().map_err(|e| unsupported(&format!("JPEG: {e:?}")))?;
    let info = probe.info().ok_or_else(|| unsupported("JPEG ohne Kopf"))?;
    let input = probe.input_colorspace().unwrap_or(ZCs::YCbCr);
    let out_cs = match input {
        ZCs::Luma | ZCs::LumaA => ZCs::Luma,
        ZCs::CMYK | ZCs::YCCK => ZCs::CMYK,
        _ => ZCs::RGB,
    };
    let comps = out_cs.num_components();
    check_size(usize::from(info.width), usize::from(info.height), comps, limits)?;
    let options = DecoderOptions::default().jpeg_set_out_colorspace(out_cs).set_strict_mode(false);
    let mut dec = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let pixels = dec.decode().map_err(|e| unsupported(&format!("JPEG: {e:?}")))?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));

    let decode = decode_array(doc, d);
    let map = |c: usize, v: u8| -> f32 {
        let x = f32::from(v) / 255.0;
        match decode.as_deref() {
            Some(a) if a.len() >= 2 * (c + 1) => a[2 * c] + x * (a[2 * c + 1] - a[2 * c]),
            _ => x,
        }
    };
    match comps {
        1 => {
            let data = if decode.is_some() {
                pixels.iter().map(|&v| (map(0, v).clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect()
            } else {
                pixels
            };
            Ok((w, h, PixelFormat::Luma8, data))
        }
        3 => {
            let mut out = Vec::with_capacity(pixels.len() / 3 * 4);
            for p in pixels.as_chunks::<3>().0 {
                let rgb = to_rgb(&ColorSpace::Rgb, &[map(0, p[0]), map(1, p[1]), map(2, p[2])]);
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
            Ok((w, h, PixelFormat::Rgba8, out))
        }
        _ => {
            let mut out = Vec::with_capacity(pixels.len());
            for p in pixels.as_chunks::<4>().0 {
                let rgb = cmyk_to_rgb(map(0, p[0]), map(1, p[1]), map(2, p[2]), map(3, p[3]));
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
            Ok((w, h, PixelFormat::Rgba8, out))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(dict: Vec<(&str, Object)>, data: Vec<u8>) -> Stream {
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"XObject".to_vec()));
        d.set("Subtype", Object::Name(b"Image".to_vec()));
        for (k, v) in dict {
            d.set(k.as_bytes().to_vec(), v);
        }
        Stream::new(d, data)
    }

    fn name(s: &str) -> Object {
        Object::Name(s.as_bytes().to_vec())
    }

    #[test]
    fn gray_rgb_cmyk_bits() {
        let doc = Document::new();
        let l = Limits::default();
        let s = img(
            vec![
                ("Width", 2.into()),
                ("Height", 1.into()),
                ("ColorSpace", name("DeviceGray")),
                ("BitsPerComponent", 8.into()),
            ],
            vec![0, 255],
        );
        let i = decode_image(&doc, &s, &l, true).unwrap();
        assert_eq!((i.format, i.data.clone()), (PixelFormat::Luma8, vec![0, 255]));

        let s = img(
            vec![
                ("Width", 1.into()),
                ("Height", 1.into()),
                ("ColorSpace", name("DeviceRGB")),
                ("BitsPerComponent", 8.into()),
            ],
            vec![255, 0, 0],
        );
        let i = decode_image(&doc, &s, &l, true).unwrap();
        assert_eq!(i.data, vec![255, 0, 0, 255]);
        assert_eq!(i.to_luma(), vec![76]);

        let s = img(
            vec![
                ("Width", 1.into()),
                ("Height", 1.into()),
                ("ColorSpace", name("DeviceCMYK")),
                ("BitsPerComponent", 8.into()),
            ],
            vec![0, 0, 0, 255],
        );
        assert_eq!(decode_image(&doc, &s, &l, true).unwrap().data, vec![0, 0, 0, 255]);

        // 1 Bit, 10 Pixel breit → 2 Byte je Zeile
        let s = img(
            vec![
                ("Width", 10.into()),
                ("Height", 1.into()),
                ("ColorSpace", name("DeviceGray")),
                ("BitsPerComponent", 1.into()),
            ],
            vec![0b1010_0000, 0b0100_0000],
        );
        let i = decode_image(&doc, &s, &l, true).unwrap();
        assert_eq!(i.data, vec![255, 0, 255, 0, 0, 0, 0, 0, 0, 255]);

        // Decode [1 0] kehrt um
        let s = img(
            vec![
                ("Width", 2.into()),
                ("Height", 1.into()),
                ("ColorSpace", name("DeviceGray")),
                ("BitsPerComponent", 8.into()),
                ("Decode", Object::Array(vec![1.into(), 0.into()])),
            ],
            vec![0, 255],
        );
        assert_eq!(decode_image(&doc, &s, &l, true).unwrap().data, vec![255, 0]);
    }

    #[test]
    fn indexed_and_mask() {
        let doc = Document::new();
        let l = Limits::default();
        let cs = Object::Array(vec![
            name("Indexed"),
            name("DeviceRGB"),
            1.into(),
            Object::String(vec![0, 0, 0, 255, 255, 255], lopdf::StringFormat::Hexadecimal),
        ]);
        let s = img(
            vec![("Width", 2.into()), ("Height", 1.into()), ("ColorSpace", cs), ("BitsPerComponent", 8.into())],
            vec![1, 0],
        );
        let i = decode_image(&doc, &s, &l, true).unwrap();
        assert_eq!(i.data, vec![255, 255, 255, 255, 0, 0, 0, 255]);

        let s = img(
            vec![("Width", 8.into()), ("Height", 1.into()), ("ImageMask", Object::Boolean(true))],
            vec![0b0000_1111],
        );
        let i = decode_image(&doc, &s, &l, true).unwrap();
        assert_eq!(i.data, vec![0, 0, 0, 0, 255, 255, 255, 255]);
    }

    #[test]
    fn size_limits_and_unsupported() {
        let doc = Document::new();
        let l = Limits { max_decoded_stream: 1000, ..Limits::default() };
        let s = img(vec![("Width", 100.into()), ("Height", 100.into()), ("ColorSpace", name("DeviceRGB"))], vec![]);
        assert!(matches!(decode_image(&doc, &s, &l, true), Err(CosError::LimitExceeded { .. })));
        let s = img(
            vec![
                ("Width", 1.into()),
                ("Height", 1.into()),
                ("ColorSpace", name("DeviceRGB")),
                ("Filter", name("JPXDecode")),
            ],
            vec![],
        );
        assert!(decode_image(&doc, &s, &Limits::default(), true).is_err());
        let s = img(vec![("Width", 1.into()), ("Height", 1.into()), ("ColorSpace", name("Pattern"))], vec![0]);
        assert!(decode_image(&doc, &s, &Limits::default(), true).is_err());
    }

    #[test]
    fn debug_is_redacted() {
        let i = DecodedImage { width: 1, height: 1, format: PixelFormat::Luma8, data: vec![42] };
        let s = format!("{i:?}");
        assert!(s.contains("len: 1") && !s.contains("42"), "{s}");
    }
}
