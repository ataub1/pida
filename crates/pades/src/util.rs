//! Kleine Hilfen für Objekte und Redaktion.

use pdfover_pdf_cos::{Object, ObjectId, SourcePdf, StringFormat};

/// Name-Objekt.
pub(crate) fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

/// Literal-String (ASCII).
pub(crate) fn literal(s: &str) -> Object {
    Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
}

/// Feldnamen u. ä. in Meldungen nur als Länge (ADR 0001 Regel 9: Namen sind Dokumentinhalt).
pub(crate) fn redacted_name(s: &str) -> String {
    format!("<{} Zeichen>", s.chars().count())
}

/// Ganzzahl (direkt oder über Referenz).
pub(crate) fn int_of(src: &SourcePdf, o: &Object) -> Option<i64> {
    match src.resolve(o).ok()? {
        Object::Integer(i) => Some(*i),
        Object::Real(r) if r.is_finite() && r.fract() == 0.0 => Some(*r as i64),
        _ => None,
    }
}

/// Semantischer Vergleich zweier Objekte: Strings nach Bytes (nicht nach Schreibweise), Streams
/// nach Dictionary und Inhalt (ohne Dateiposition), Dictionaries ohne Reihenfolge, Reals mit
/// `f32`-Gleichheit.
pub(crate) fn same(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::String(x, _), Object::String(y, _)) => x == y,
        (Object::Array(x), Object::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Object::Dictionary(x), Object::Dictionary(y)) => same_dict(x, y),
        (Object::Stream(x), Object::Stream(y)) => same_dict(&x.dict, &y.dict) && x.content == y.content,
        (Object::Integer(x), Object::Real(y)) | (Object::Real(y), Object::Integer(x)) => (*x as f32) == *y,
        (Object::Real(x), Object::Real(y)) => x.to_bits() == y.to_bits() || x == y,
        _ => a == b,
    }
}

/// Dictionaries ohne Reihenfolge vergleichen ([`same`]).
pub(crate) fn same_dict(a: &pdfover_pdf_cos::Dictionary, b: &pdfover_pdf_cos::Dictionary) -> bool {
    a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_ok_and(|w| same(v, w)))
}

/// Schlüssel, deren Werte sich zwischen `old` und `new` unterscheiden (inkl. hinzugekommener
/// und entfernter Schlüssel), als Text für Diagnosen (nur Schlüsselnamen, Regel 9).
pub(crate) fn changed_keys(old: &pdfover_pdf_cos::Dictionary, new: &pdfover_pdf_cos::Dictionary) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (k, v) in old.iter() {
        match new.get(k) {
            Ok(w) if same(v, w) => {}
            _ => out.push(k.clone()),
        }
    }
    for (k, _) in new.iter() {
        if !old.has(k) {
            out.push(k.clone());
        }
    }
    out
}

/// `true`, wenn `old` ein Präfix von `new` ist und `new` länger ist oder gleich (nur angehängt).
pub(crate) fn append_only(old: &[Object], new: &[Object]) -> bool {
    new.len() >= old.len() && old.iter().zip(new).all(|(a, b)| same(a, b))
}

/// Referenz-ID, falls `o` eine Referenz ist.
pub(crate) fn as_ref_id(o: &Object) -> Option<ObjectId> {
    match o {
        Object::Reference(id) => Some(*id),
        _ => None,
    }
}

/// Schlüsselname für Diagnosen.
pub(crate) fn key_text(k: &[u8]) -> String {
    format!("/{}", String::from_utf8_lossy(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfover_pdf_cos::{Dictionary, Stream};

    #[test]
    fn semantic_equality() {
        assert!(same(
            &Object::String(b"ab".to_vec(), StringFormat::Literal),
            &Object::String(b"ab".to_vec(), StringFormat::Hexadecimal)
        ));
        let mut s1 = Stream::new(Dictionary::new(), b"x".to_vec());
        let mut s2 = s1.clone();
        s1.start_position = Some(10);
        s2.start_position = Some(99);
        assert!(same(&Object::Stream(s1), &Object::Stream(s2)));
        let mut d1 = Dictionary::new();
        d1.set("A", 1);
        d1.set("B", 2);
        let mut d2 = Dictionary::new();
        d2.set("B", 2);
        d2.set("A", 1);
        assert!(same_dict(&d1, &d2));
        d2.set("C", 3);
        assert_eq!(changed_keys(&d1, &d2), vec![b"C".to_vec()]);
        assert!(same(&Object::Integer(3), &Object::Real(3.0)));
        assert!(append_only(&[1.into()], &[1.into(), 2.into()]));
        assert!(!append_only(&[1.into(), 2.into()], &[2.into(), 1.into()]));
        assert_eq!(redacted_name("Max"), "<3 Zeichen>");
    }
}
