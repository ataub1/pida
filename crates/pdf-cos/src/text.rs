//! Textstrings (ISO 32000-1 §7.9.2.2): PDFDocEncoding, UTF-16BE mit BOM, UTF-8 mit BOM (PDF 2.0).

use lopdf::{Object, StringFormat};

/// PDFDocEncoding 0x18–0x1F und 0x80–0xA0 (ISO 32000-1 Anhang D.2); übrige Codes wie Latin-1.
const SPECIAL: [(u8, char); 40] = [
    (0x18, '\u{02D8}'),
    (0x19, '\u{02C7}'),
    (0x1A, '\u{02C6}'),
    (0x1B, '\u{02D9}'),
    (0x1C, '\u{02DD}'),
    (0x1D, '\u{02DB}'),
    (0x1E, '\u{02DA}'),
    (0x1F, '\u{02DC}'),
    (0x80, '\u{2022}'),
    (0x81, '\u{2020}'),
    (0x82, '\u{2021}'),
    (0x83, '\u{2026}'),
    (0x84, '\u{2014}'),
    (0x85, '\u{2013}'),
    (0x86, '\u{0192}'),
    (0x87, '\u{2044}'),
    (0x88, '\u{2039}'),
    (0x89, '\u{203A}'),
    (0x8A, '\u{2212}'),
    (0x8B, '\u{2030}'),
    (0x8C, '\u{201E}'),
    (0x8D, '\u{201C}'),
    (0x8E, '\u{201D}'),
    (0x8F, '\u{2018}'),
    (0x90, '\u{2019}'),
    (0x91, '\u{201A}'),
    (0x92, '\u{2122}'),
    (0x93, '\u{FB01}'),
    (0x94, '\u{FB02}'),
    (0x95, '\u{0141}'),
    (0x96, '\u{0152}'),
    (0x97, '\u{0160}'),
    (0x98, '\u{0178}'),
    (0x99, '\u{017D}'),
    (0x9A, '\u{0131}'),
    (0x9B, '\u{0142}'),
    (0x9C, '\u{0153}'),
    (0x9D, '\u{0161}'),
    (0x9E, '\u{017E}'),
    (0xA0, '\u{20AC}'),
];

fn pdfdoc_decode(b: u8) -> char {
    match SPECIAL.iter().find(|(c, _)| *c == b) {
        Some((_, ch)) => *ch,
        None => char::from(b),
    }
}

/// Byte für `c` in PDFDocEncoding, falls darstellbar (ohne undefinierte Codes).
fn pdfdoc_encode(c: char) -> Option<u8> {
    if let Some((b, _)) = SPECIAL.iter().find(|(_, ch)| *ch == c) {
        return Some(*b);
    }
    match c {
        '\t' | '\n' | '\r' | '\u{20}'..='\u{7E}' | '\u{A1}'..='\u{AC}' | '\u{AE}'..='\u{FF}' => Some(c as u32 as u8),
        _ => None,
    }
}

/// Dekodiert einen Textstring (`/T`, `/Name`, `/Reason` …); `None` bei Nicht-Strings.
pub fn decode_text_string(obj: &Object) -> Option<String> {
    let bytes = obj.as_str().ok()?;
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return Some(String::from_utf8_lossy(rest).into_owned());
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        let units: Vec<u16> = rest
            .chunks(2)
            .map(|c| if c.len() == 2 { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], 0]) })
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    Some(bytes.iter().map(|&b| pdfdoc_decode(b)).collect())
}

pub(crate) fn decode_text(obj: &Object) -> Option<String> {
    decode_text_string(obj)
}

/// Kodiert einen Textstring wie PDFBox `COSString(String)`: druckbares ASCII als Literal, sonst
/// PDFDocEncoding (falls möglich) bzw. UTF-16BE mit BOM, jeweils als Hex-String.
pub fn encode_text_string(text: &str) -> Object {
    if text.chars().all(|c| matches!(c, '\u{20}'..='\u{7E}')) {
        return Object::String(text.as_bytes().to_vec(), StringFormat::Literal);
    }
    if let Some(bytes) = text.chars().map(pdfdoc_encode).collect::<Option<Vec<u8>>>() {
        return Object::String(bytes, StringFormat::Hexadecimal);
    }
    let mut out = vec![0xFE, 0xFF];
    for u in text.encode_utf16() {
        out.extend_from_slice(&u.to_be_bytes());
    }
    Object::String(out, StringFormat::Hexadecimal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for s in ["PDF-AS Signatur1", "Prüfung äöü ß", "Ελληνικά € 中文", "a\nb", "", "„Zitat“ – € •", "ﬁ Ł Ÿ"]
        {
            let o = encode_text_string(s);
            assert_eq!(decode_text_string(&o).unwrap(), s, "{s}");
        }
        assert!(matches!(encode_text_string("abc"), Object::String(_, StringFormat::Literal)));
        assert!(matches!(encode_text_string("ä"), Object::String(ref b, StringFormat::Hexadecimal) if b == &[0xE4]));
        assert!(matches!(encode_text_string("€"), Object::String(ref b, _) if b == &[0xA0]));
        assert!(matches!(encode_text_string("中"), Object::String(ref b, _) if b.starts_with(&[0xFE, 0xFF])));
        let utf8 = Object::String(b"\xEF\xBB\xBFx\xC3\xA4".to_vec(), StringFormat::Literal);
        assert_eq!(decode_text_string(&utf8).unwrap(), "xä");
        // Beispiel aus ISO 32000-1: 0x8B = ‰
        let o = Object::String(b"text\x8B".to_vec(), StringFormat::Literal);
        assert_eq!(decode_text_string(&o).unwrap(), "text‰");
        assert_eq!(decode_text_string(&Object::Integer(1)), None);
    }
}
