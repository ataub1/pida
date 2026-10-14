//! Stream-Filter mit Grenzen gegen Dekompressionsbomben (Plan §7.7).
//!
//! Die allgemeinen Filter (Flate/LZW mit Predictor, ASCII85, ASCIIHex, RunLength) dekodiert lopdf,
//! jeweils mit einer Obergrenze je Filterstufe. Bildfilter (DCT, JPX, JBIG2, CCITT) bleiben – je
//! nach Aufrufer – als „Rest“ stehen und werden in `image.rs` behandelt bzw. abgelehnt.

use lopdf::{Dictionary, Document, Object, Stream};

use crate::{CosError, Limits, Result};

/// Unterhalb dieser dekodierten Größe greift die Verhältnisgrenze nicht (gleichmäßige Bilder
/// komprimieren legitim um Faktor > 1000).
pub(crate) const RATIO_FLOOR: u64 = 16 * 1024 * 1024;

/// Bildfilter, der nach den allgemeinen Filtern übrig bleibt.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ImageFilter {
    pub name: Vec<u8>,
    pub params: Option<Dictionary>,
}

/// Ergebnis der Dekodierung.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Decoded {
    pub data: Vec<u8>,
    pub image_filter: Option<ImageFilter>,
}

fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    let mut o = obj;
    for _ in 0..32 {
        match o {
            Object::Reference(id) => match doc.get_object(*id) {
                Ok(next) => o = next,
                Err(_) => return &Object::Null,
            },
            _ => return o,
        }
    }
    &Object::Null
}

fn normalize_filter(name: &[u8]) -> &[u8] {
    match name {
        b"AHx" => b"ASCIIHexDecode",
        b"A85" => b"ASCII85Decode",
        b"LZW" => b"LZWDecode",
        b"Fl" => b"FlateDecode",
        b"RL" => b"RunLengthDecode",
        b"CCF" => b"CCITTFaxDecode",
        b"DCT" => b"DCTDecode",
        other => other,
    }
}

/// Grenze für eine Filterstufe mit Eingabelänge `input_len`.
pub(crate) fn layer_limit(limits: &Limits, input_len: usize) -> u64 {
    let ratio_bound = (input_len as u64).saturating_mul(u64::from(limits.max_decompression_ratio)).max(RATIO_FLOOR);
    limits.max_decoded_stream.min(ratio_bound)
}

/// Filterkette eines Streams: `(Name, DecodeParms)` in Dekodierreihenfolge.
pub(crate) fn filter_chain(doc: &Document, dict: &Dictionary) -> Result<Vec<(Vec<u8>, Option<Dictionary>)>> {
    let filters: Vec<Vec<u8>> = match dict.get(b"Filter").map(|o| resolve(doc, o)) {
        Err(_) | Ok(Object::Null) => Vec::new(),
        Ok(Object::Name(n)) => vec![n.clone()],
        Ok(Object::Array(a)) => a
            .iter()
            .map(|o| match resolve(doc, o) {
                Object::Name(n) => Ok(n.clone()),
                _ => Err(CosError::InvalidStructure("ungültiger /Filter-Eintrag".into())),
            })
            .collect::<Result<_>>()?,
        Ok(_) => return Err(CosError::InvalidStructure("ungültiger /Filter-Eintrag".into())),
    };
    let params_obj = dict.get(b"DecodeParms").or_else(|_| dict.get(b"DP")).map(|o| resolve(doc, o)).ok();
    let params: Vec<Option<Dictionary>> = match params_obj {
        // Ein einzelnes Dictionary gilt (wie bei lopdf) für jede Stufe; nur Flate/LZW/DCT werten es aus.
        Some(Object::Dictionary(d)) => vec![Some(d.clone()); filters.len()],
        Some(Object::Array(a)) => {
            let mut v: Vec<Option<Dictionary>> = a
                .iter()
                .map(|o| match resolve(doc, o) {
                    Object::Dictionary(d) => Some(d.clone()),
                    _ => None,
                })
                .collect();
            v.resize(filters.len(), None);
            v
        }
        _ => vec![None; filters.len()],
    };
    Ok(filters.into_iter().zip(params).map(|(f, p)| (normalize_filter(&f).to_vec(), p)).collect())
}

/// Dekodiert einen Stream. Mit `allow_image_filter` darf als **letzter** Filter ein Bildfilter
/// übrig bleiben, sonst ist das ein Fehler.
pub(crate) fn decode_stream(
    doc: &Document,
    stream: &Stream,
    limits: &Limits,
    allow_image_filter: bool,
) -> Result<Decoded> {
    let chain = filter_chain(doc, &stream.dict)?;
    let mut data: Vec<u8> = stream.content.clone();
    let n = chain.len();
    for (i, (name, params)) in chain.into_iter().enumerate() {
        match name.as_slice() {
            b"FlateDecode" | b"LZWDecode" | b"ASCII85Decode" | b"ASCIIHexDecode" | b"RunLengthDecode" => {
                let limit = layer_limit(limits, data.len());
                let mut d = Dictionary::new();
                d.set("Filter", Object::Name(name.clone()));
                if let Some(p) = params {
                    d.set("DecodeParms", Object::Dictionary(p));
                }
                let tmp = Stream::new(d, std::mem::take(&mut data));
                let cap = usize::try_from(limit).unwrap_or(usize::MAX);
                data = match tmp.decompressed_content_with_limit(cap) {
                    Ok(v) => v,
                    Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
                        return Err(CosError::LimitExceeded { what: "dekomprimierte Stream-Größe", limit });
                    }
                    Err(e) => {
                        return Err(CosError::Parse(format!("Stream-Filter {}: {e}", String::from_utf8_lossy(&name))));
                    }
                };
                if data.len() as u64 > limit {
                    return Err(CosError::LimitExceeded { what: "dekomprimierte Stream-Größe", limit });
                }
            }
            b"Crypt" => {
                // Bereits beim Laden entschlüsselt (lopdf); nur `/Identity` bzw. Standard zulässig.
            }
            b"DCTDecode" | b"JPXDecode" | b"JBIG2Decode" | b"CCITTFaxDecode" if allow_image_filter && i + 1 == n => {
                return Ok(Decoded { data, image_filter: Some(ImageFilter { name, params }) });
            }
            other => {
                return Err(CosError::Parse(format!(
                    "nicht unterstützter Stream-Filter {}",
                    String::from_utf8_lossy(other)
                )));
            }
        }
    }
    if data.len() as u64 > limits.max_decoded_stream {
        return Err(CosError::LimitExceeded {
            what: "dekomprimierte Stream-Größe", limit: limits.max_decoded_stream
        });
    }
    Ok(Decoded { data, image_filter: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn flate(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn flate_roundtrip_and_bomb_limit() {
        let doc = Document::new();
        let raw = vec![b'A'; 100_000];
        let mut d = Dictionary::new();
        d.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        let s = Stream::new(d.clone(), flate(&raw));
        let out = decode_stream(&doc, &s, &Limits::default(), false).unwrap();
        assert_eq!(out.data, raw);

        // Bombe: winziger Stream, große Ausgabe, Grenze 50 000 Bytes
        let limits = Limits { max_decoded_stream: 50_000, ..Limits::default() };
        let err = decode_stream(&doc, &s, &limits, false).unwrap_err();
        assert!(matches!(err, CosError::LimitExceeded { .. }), "{err:?}");
    }

    #[test]
    fn ratio_limit_applies_above_floor() {
        let doc = Document::new();
        let size = (RATIO_FLOOR as usize) + 1024 * 1024;
        // Mäßig komprimierbar (Pseudozufall, 4 Bit je Byte): Verhältnis ≈ 2.
        let mut x: u32 = 12345;
        let raw: Vec<u8> = (0..size)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                ((x >> 16) & 0x0F) as u8
            })
            .collect();
        let mut d = Dictionary::new();
        d.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        let s = Stream::new(d.clone(), flate(&raw));
        // Standardgrenzen (Verhältnis 1000): unkritisch
        assert_eq!(decode_stream(&doc, &s, &Limits::default(), false).unwrap().data.len(), raw.len());
        // Verhältnis 1: oberhalb der Untergrenze abgelehnt
        let limits = Limits { max_decompression_ratio: 1, ..Limits::default() };
        assert!(matches!(decode_stream(&doc, &s, &limits, false), Err(CosError::LimitExceeded { .. })));
        // Nullen komprimieren ≈ 1030:1 – oberhalb der Untergrenze auch mit Standardgrenzen eine Bombe
        let zeros = Stream::new(d, flate(&vec![0u8; size]));
        assert!(matches!(decode_stream(&doc, &zeros, &Limits::default(), false), Err(CosError::LimitExceeded { .. })));
        // Unterhalb der Untergrenze greift das Verhältnis nie.
        let small = {
            let mut dd = Dictionary::new();
            dd.set("Filter", Object::Name(b"FlateDecode".to_vec()));
            Stream::new(dd, flate(&vec![0u8; 4 * 1024 * 1024]))
        };
        assert!(decode_stream(&doc, &small, &limits, false).is_ok());
    }

    #[test]
    fn filter_chain_and_image_rest() {
        let doc = Document::new();
        let mut d = Dictionary::new();
        d.set("Filter", Object::Array(vec![Object::Name(b"AHx".to_vec()), Object::Name(b"DCT".to_vec())]));
        let s = Stream::new(d, b"414243>".to_vec());
        let out = decode_stream(&doc, &s, &Limits::default(), true).unwrap();
        assert_eq!(out.data, b"ABC");
        assert_eq!(out.image_filter.unwrap().name, b"DCTDecode");
        assert!(decode_stream(&doc, &s, &Limits::default(), false).is_err());
    }
}
