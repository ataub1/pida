//! Minimaler, eigener DER-Kodierer und BER/DER-TLV-Leser.
//!
//! Warum eigen statt `cms`/`der`-Typen: Die SignedAttributes müssen **byte-genau** so signiert
//! werden, wie sie im CMS stehen (docs/analyse/03 §5.1). Beim Prüfen fremder CMS (Modell B) braucht
//! der Verifier die **Originalbytes** der Attribute; ein Dekodieren und Neukodieren (wie bei `cms`
//! 0.2, das ein `SET OF` sortiert neu kodiert) würde nicht-kanonische, aber gültig signierte
//! Kodierungen verfälschen. Der Leser akzeptiert deshalb auch BER (unbestimmte Länge, nicht minimale
//! Längenkodierung); der Kodierer erzeugt strikt DER (X.690 §10, SET OF sortiert).

use crate::{CmsError, Result};

/// Universal-Tags (eine Oktette, Klasse universal).
pub(crate) mod tag {
    pub const BOOLEAN: u8 = 0x01;
    pub const INTEGER: u8 = 0x02;
    pub const BIT_STRING: u8 = 0x03;
    pub const OCTET_STRING: u8 = 0x04;
    pub const NULL: u8 = 0x05;
    pub const OID: u8 = 0x06;
    pub const UTF8_STRING: u8 = 0x0C;
    pub const UTC_TIME: u8 = 0x17;
    pub const GENERALIZED_TIME: u8 = 0x18;
    pub const SEQUENCE: u8 = 0x30;
    pub const SET: u8 = 0x31;
}

/// Maximale Verschachtelungstiefe beim Lesen (Schutz vor Stapelüberlauf durch bösartige Eingaben).
const MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------------------------------
// Kodierer
// ---------------------------------------------------------------------------------------------

/// Hängt die DER-Längenkodierung (kürzeste Form) an.
pub(crate) fn push_len(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes = len.to_be_bytes();
        let skip = bytes.iter().take_while(|&&b| b == 0).count();
        let n = bytes.len() - skip;
        out.push(0x80 | n as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
}

/// Länge des Headers (Tag + Länge) für einen Inhalt der Länge `len`.
pub(crate) fn header_len(len: usize) -> usize {
    if len < 0x80 {
        2
    } else {
        let bits = usize::BITS - len.leading_zeros();
        2 + bits.div_ceil(8) as usize
    }
}

/// `tag || len || content`.
pub(crate) fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + header_len(content.len()));
    out.push(tag);
    push_len(&mut out, content.len());
    out.extend_from_slice(content);
    out
}

/// Konstruierter Wert aus bereits kodierten Teilen.
pub(crate) fn constructed(tag: u8, parts: &[&[u8]]) -> Vec<u8> {
    let len: usize = parts.iter().map(|p| p.len()).sum();
    let mut out = Vec::with_capacity(len + header_len(len));
    out.push(tag);
    push_len(&mut out, len);
    for p in parts {
        out.extend_from_slice(p);
    }
    out
}

/// `SEQUENCE { parts… }`.
pub(crate) fn seq(parts: &[&[u8]]) -> Vec<u8> {
    constructed(tag::SEQUENCE, parts)
}

/// Sortiert kodierte Elemente nach X.690 §11.6 (DER „SET OF“): aufsteigend als Oktettfolgen
/// verglichen, kürzere am Ende mit `0x00` aufgefüllt.
pub(crate) fn der_sort(items: &mut [Vec<u8>]) {
    items.sort_by(|a, b| {
        let n = a.len().max(b.len());
        for i in 0..n {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            match x.cmp(&y) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        std::cmp::Ordering::Equal
    });
}

/// `SET OF` mit DER-Sortierung.
pub(crate) fn set_of(mut items: Vec<Vec<u8>>) -> Vec<u8> {
    der_sort(&mut items);
    let refs: Vec<&[u8]> = items.iter().map(Vec::as_slice).collect();
    constructed(tag::SET, &refs)
}

/// OID aus Punktnotation (Konstanten dieses Crates; ungültige Eingabe ist ein Programmierfehler).
pub(crate) fn oid(dotted: &str) -> Vec<u8> {
    let oid = const_oid::ObjectIdentifier::new(dotted).unwrap_or_else(|e| panic!("ungültige OID {dotted}: {e}"));
    tlv(tag::OID, oid.as_bytes())
}

/// `NULL`.
pub(crate) fn null() -> Vec<u8> {
    vec![tag::NULL, 0x00]
}

/// `INTEGER` aus einer kleinen nichtnegativen Zahl.
pub(crate) fn small_int(v: u32) -> Vec<u8> {
    integer_unsigned(&v.to_be_bytes())
}

/// `INTEGER` aus einem nichtnegativen Betrag (big-endian, führende Nullen erlaubt).
pub(crate) fn integer_unsigned(magnitude: &[u8]) -> Vec<u8> {
    let skip = magnitude.iter().take_while(|&&b| b == 0).count();
    let m = &magnitude[skip..];
    let mut content = Vec::with_capacity(m.len() + 1);
    if m.is_empty() || m[0] & 0x80 != 0 {
        content.push(0);
    }
    content.extend_from_slice(m);
    tlv(tag::INTEGER, &content)
}

/// `OCTET STRING`.
pub(crate) fn octet_string(b: &[u8]) -> Vec<u8> {
    tlv(tag::OCTET_STRING, b)
}

/// `AlgorithmIdentifier { oid, NULL? }`.
pub(crate) fn alg_id(dotted: &str, null_params: bool) -> Vec<u8> {
    if null_params { seq(&[&oid(dotted), &null()]) } else { seq(&[&oid(dotted)]) }
}

/// Kontextspezifisch, konstruiert (`[n]`, EXPLICIT bzw. IMPLICIT über konstruierten Typ).
pub(crate) fn context(n: u8, parts: &[&[u8]]) -> Vec<u8> {
    constructed(0xA0 | n, parts)
}

// ---------------------------------------------------------------------------------------------
// Leser
// ---------------------------------------------------------------------------------------------

/// Ein gelesenes TLV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tlv<'a> {
    /// Tag-Oktett.
    pub tag: u8,
    /// Inhalt (bei unbestimmter Länge ohne End-of-Contents).
    pub content: &'a [u8],
    /// Vollständige Kodierung (Header + Inhalt + ggf. End-of-Contents).
    pub raw: &'a [u8],
    /// BER: unbestimmte Länge.
    pub indefinite: bool,
}

impl<'a> Tlv<'a> {
    /// Konstruierter Typ?
    pub fn is_constructed(&self) -> bool {
        self.tag & 0x20 != 0
    }

    /// Kinder lesen (nur für konstruierte Typen).
    pub fn reader(&self) -> Reader<'a> {
        Reader::new(self.content)
    }

    /// Erwartet ein bestimmtes Tag.
    pub fn expect(self, tag: u8, what: &str) -> Result<Self> {
        if self.tag == tag { Ok(self) } else { Err(der_err(format!("{what}: Tag {:#04x} statt {tag:#04x}", self.tag))) }
    }

    /// OID-Inhalt als Punktnotation.
    pub fn oid_string(&self) -> Result<String> {
        if self.tag != tag::OID {
            return Err(der_err(format!("OID erwartet, Tag {:#04x}", self.tag)));
        }
        oid_to_string(self.content)
    }
}

pub(crate) fn der_err(msg: impl Into<String>) -> CmsError {
    CmsError::Der(msg.into())
}

/// OID-Inhaltsbytes → Punktnotation.
pub(crate) fn oid_to_string(content: &[u8]) -> Result<String> {
    const_oid::ObjectIdentifier::from_bytes(content)
        .map(|o| o.to_string())
        .map_err(|e| der_err(format!("ungültige OID: {e}")))
}

/// Liest ein TLV am Anfang von `data` (BER-tolerant). Liefert das TLV.
pub(crate) fn read_tlv(data: &[u8]) -> Result<Tlv<'_>> {
    read_tlv_depth(data, 0)
}

fn read_tlv_depth(data: &[u8], depth: usize) -> Result<Tlv<'_>> {
    if depth > MAX_DEPTH {
        return Err(der_err("Verschachtelung zu tief"));
    }
    let tag = *data.first().ok_or_else(|| der_err("unerwartetes Ende (Tag)"))?;
    if tag & 0x1F == 0x1F {
        return Err(der_err("Tags mit hoher Nummer werden nicht unterstützt"));
    }
    let l0 = *data.get(1).ok_or_else(|| der_err("unerwartetes Ende (Länge)"))?;
    if l0 == 0x80 {
        // BER: unbestimmte Länge – nur für konstruierte Typen zulässig.
        if tag & 0x20 == 0 {
            return Err(der_err("unbestimmte Länge bei primitivem Typ"));
        }
        let mut pos = 2;
        loop {
            let rest = &data[pos..];
            if rest.len() >= 2 && rest[0] == 0 && rest[1] == 0 {
                let content = &data[2..pos];
                return Ok(Tlv { tag, content, raw: &data[..pos + 2], indefinite: true });
            }
            let child = read_tlv_depth(rest, depth + 1)?;
            pos += child.raw.len();
        }
    }
    let (len, hl) = if l0 < 0x80 {
        (l0 as usize, 2)
    } else {
        let n = (l0 & 0x7F) as usize;
        if n == 0x7F || n > std::mem::size_of::<usize>() || n > 8 {
            return Err(der_err("Längenfeld zu groß"));
        }
        let bytes = data.get(2..2 + n).ok_or_else(|| der_err("unerwartetes Ende (Länge)"))?;
        let mut len: usize = 0;
        for &b in bytes {
            len =
                len.checked_mul(256).and_then(|l| l.checked_add(b as usize)).ok_or_else(|| der_err("Länge zu groß"))?;
        }
        (len, 2 + n)
    };
    let end = hl.checked_add(len).ok_or_else(|| der_err("Länge zu groß"))?;
    if end > data.len() {
        return Err(der_err(format!("Länge {len} überschreitet die Daten ({} Bytes verfügbar)", data.len() - hl)));
    }
    Ok(Tlv { tag, content: &data[hl..end], raw: &data[..end], indefinite: false })
}

/// Sequentieller Leser über die Kinder eines konstruierten Werts.
#[derive(Debug, Clone)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Tag des nächsten Elements.
    pub fn peek_tag(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Nächstes Element.
    pub fn read(&mut self) -> Result<Tlv<'a>> {
        let t = read_tlv(&self.data[self.pos..])?;
        self.pos += t.raw.len();
        Ok(t)
    }

    /// Nächstes Element mit erwartetem Tag.
    pub fn read_expect(&mut self, tag: u8, what: &str) -> Result<Tlv<'a>> {
        self.read()?.expect(tag, what)
    }

    /// Nächstes Element, wenn es das Tag hat.
    pub fn read_optional(&mut self, tag: u8) -> Result<Option<Tlv<'a>>> {
        if self.peek_tag() == Some(tag) { self.read().map(Some) } else { Ok(None) }
    }

    /// Alle restlichen Elemente.
    pub fn read_all(&mut self) -> Result<Vec<Tlv<'a>>> {
        let mut out = Vec::new();
        while !self.is_empty() {
            out.push(self.read()?);
        }
        Ok(out)
    }
}

/// INTEGER-Inhalt → `u32` (nur nichtnegative, kleine Werte, z. B. Versionen).
pub(crate) fn small_uint(t: &Tlv<'_>) -> Result<u32> {
    if t.tag != tag::INTEGER || t.content.is_empty() {
        return Err(der_err("INTEGER erwartet"));
    }
    if t.content[0] & 0x80 != 0 {
        return Err(der_err("negative Zahl"));
    }
    let skip = t.content.iter().take_while(|&&b| b == 0).count();
    let m = &t.content[skip..];
    if m.len() > 4 {
        return Err(der_err("Zahl zu groß"));
    }
    Ok(m.iter().fold(0u32, |acc, &b| (acc << 8) | b as u32))
}

/// Betrag eines INTEGER (führende `0x00` entfernt) – für Seriennummern-Vergleiche.
pub(crate) fn integer_magnitude(content: &[u8]) -> &[u8] {
    let skip = content.iter().take_while(|&&b| b == 0).count();
    let m = &content[skip..];
    if m.is_empty() && !content.is_empty() { &content[content.len() - 1..] } else { m }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_encoding() {
        let mut v = Vec::new();
        push_len(&mut v, 0x7F);
        assert_eq!(v, [0x7F]);
        v.clear();
        push_len(&mut v, 0x80);
        assert_eq!(v, [0x81, 0x80]);
        v.clear();
        push_len(&mut v, 0x1234);
        assert_eq!(v, [0x82, 0x12, 0x34]);
        for len in [0usize, 1, 127, 128, 255, 256, 65535, 65536, 1 << 24] {
            let mut v = vec![0u8];
            push_len(&mut v, len);
            assert_eq!(v.len(), header_len(len), "{len}");
        }
    }

    #[test]
    fn integers() {
        assert_eq!(small_int(1), [0x02, 0x01, 0x01]);
        assert_eq!(small_int(0), [0x02, 0x01, 0x00]);
        assert_eq!(integer_unsigned(&[0x80]), [0x02, 0x02, 0x00, 0x80]);
        assert_eq!(integer_unsigned(&[0x00, 0x00, 0x7F]), [0x02, 0x01, 0x7F]);
        assert_eq!(integer_magnitude(&[0x00, 0x80]), &[0x80]);
        assert_eq!(integer_magnitude(&[0x00]), &[0x00]);
    }

    #[test]
    fn set_of_sorting_matches_x690() {
        // kürzere Kodierung wird mit 0x00 aufgefüllt verglichen
        let mut items = vec![vec![0x30, 0x03, 0x02, 0x01, 0x05], vec![0x30, 0x02], vec![0x04, 0x00]];
        der_sort(&mut items);
        assert_eq!(items[0], [0x04, 0x00]);
        assert_eq!(items[1], [0x30, 0x02]);
    }

    #[test]
    fn reader_handles_ber_indefinite_length() {
        // SEQUENCE (unbestimmt) { INTEGER 5, OCTET STRING "ab" } EOC
        let data = [0x30, 0x80, 0x02, 0x01, 0x05, 0x04, 0x02, b'a', b'b', 0x00, 0x00, 0xFF];
        let t = read_tlv(&data).unwrap();
        assert!(t.indefinite);
        assert_eq!(t.raw.len(), 11);
        let mut r = t.reader();
        assert_eq!(small_uint(&r.read().unwrap()).unwrap(), 5);
        assert_eq!(r.read().unwrap().content, b"ab");
        assert!(r.is_empty());
    }

    #[test]
    fn reader_rejects_garbage() {
        assert!(read_tlv(&[]).is_err());
        assert!(read_tlv(&[0x30]).is_err());
        assert!(read_tlv(&[0x30, 0x05, 0x00]).is_err());
        assert!(read_tlv(&[0x04, 0x80, 0x00, 0x00]).is_err());
        assert!(read_tlv(&[0x1F, 0x01, 0x00]).is_err());
        assert!(read_tlv(&[0x30, 0x89, 1, 2, 3, 4, 5, 6, 7, 8, 9]).is_err());
        // tiefe Verschachtelung mit unbestimmter Länge
        let mut deep = Vec::new();
        for _ in 0..200 {
            deep.extend_from_slice(&[0x30, 0x80]);
        }
        assert!(read_tlv(&deep).is_err());
    }

    #[test]
    fn oid_roundtrip() {
        let enc = oid("1.2.840.113549.1.7.2");
        let t = read_tlv(&enc).unwrap();
        assert_eq!(t.oid_string().unwrap(), "1.2.840.113549.1.7.2");
    }
}
