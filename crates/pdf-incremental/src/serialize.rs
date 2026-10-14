//! Serialisierung von COS-Objekten (eigener Writer; lopdf schreibt nie Originalbytes neu).
//!
//! Format wie PDFBox `COSWriter` (docs/analyse/03 Phase E Schritt 7): Top-Level-Dictionaries
//! zeilenweise `<<\n/Key Wert\n>>`, verschachtelte Objekte einzeilig, Strings als Literal nur bei
//! druckbarem ASCII, sonst Hex in Großbuchstaben, Streams `stream\r\n…\r\nendstream`.

use std::io::Write as _;

use pdfover_pdf_cos::{CryptTarget, Dictionary, Encryptor, Object, ObjectId, PathSegment, RealLiterals, StringFormat};

use crate::{IncError, Result};

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Maximale Verschachtelungstiefe beim Schreiben.
const MAX_DEPTH: usize = 128;

#[inline]
fn is_delimiter(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

pub(crate) fn write_name(out: &mut Vec<u8>, name: &[u8]) {
    out.push(b'/');
    for &b in name {
        if !(0x21..=0x7E).contains(&b) || is_delimiter(b) || b == b'#' {
            out.push(b'#');
            out.push(HEX[usize::from(b >> 4)]);
            out.push(HEX[usize::from(b & 0x0F)]);
        } else {
            out.push(b);
        }
    }
}

pub(crate) fn write_hex(out: &mut Vec<u8>, bytes: &[u8]) {
    out.push(b'<');
    for &b in bytes {
        out.push(HEX[usize::from(b >> 4)]);
        out.push(HEX[usize::from(b & 0x0F)]);
    }
    out.push(b'>');
}

pub(crate) fn write_string(out: &mut Vec<u8>, bytes: &[u8], format: StringFormat) {
    let printable = bytes.iter().all(|b| (0x20..=0x7E).contains(b));
    if format == StringFormat::Literal && printable {
        out.push(b'(');
        for &b in bytes {
            if matches!(b, b'(' | b')' | b'\\') {
                out.push(b'\\');
            }
            out.push(b);
        }
        out.push(b')');
    } else {
        write_hex(out, bytes);
    }
}

pub(crate) fn write_real(out: &mut Vec<u8>, v: f32) {
    if !v.is_finite() {
        out.push(b'0');
        return;
    }
    let mut s = format!("{:.6}", v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" || s.is_empty() {
        s = "0".into();
    }
    out.extend_from_slice(s.as_bytes());
}

/// Kontext beim Schreiben: Pfad im Objekt und Originalschreibweise der Reals (fortgeschriebene
/// Objekte behalten so z. B. `595.303937007874` statt der `f32`-Rundung).
pub(crate) struct Ctx<'a> {
    lits: Option<&'a RealLiterals>,
    path: Vec<PathSegment>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(lits: Option<&'a RealLiterals>) -> Self {
        Self { lits, path: Vec::new() }
    }

    fn real(&self, out: &mut Vec<u8>, v: f32) {
        match self.lits.and_then(|l| l.get(&self.path, v)) {
            Some(text) => out.extend_from_slice(text.as_bytes()),
            None => write_real(out, v),
        }
    }
}

/// Schreibt ein direktes Objekt (Streams sind nur als indirekte Objekte zulässig).
pub(crate) fn write_direct(out: &mut Vec<u8>, obj: &Object, depth: usize) -> Result<()> {
    write_value(out, obj, depth, &mut Ctx::new(None))
}

fn write_value(out: &mut Vec<u8>, obj: &Object, depth: usize, ctx: &mut Ctx<'_>) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(IncError::Serialize("Objekt zu tief verschachtelt".into()));
    }
    match obj {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Boolean(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Object::Integer(i) => {
            let _ = write!(out, "{i}");
        }
        Object::Real(r) => ctx.real(out, *r),
        Object::Name(n) => write_name(out, n),
        Object::String(s, f) => write_string(out, s, *f),
        Object::Array(a) => {
            out.push(b'[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                ctx.path.push(PathSegment::Index(i));
                let r = write_value(out, item, depth + 1, ctx);
                ctx.path.pop();
                r?;
            }
            out.push(b']');
        }
        Object::Dictionary(d) => {
            out.extend_from_slice(b"<<");
            for (k, v) in d.iter() {
                out.push(b' ');
                write_name(out, k);
                out.push(b' ');
                ctx.path.push(PathSegment::Key(k.clone()));
                let r = write_value(out, v, depth + 1, ctx);
                ctx.path.pop();
                r?;
            }
            out.extend_from_slice(b" >>");
        }
        Object::Reference((n, g)) => {
            let _ = write!(out, "{n} {g} R");
        }
        Object::Stream(_) => return Err(IncError::Serialize("Stream als direktes Objekt".into())),
    }
    Ok(())
}

/// Top-Level-Dictionary zeilenweise (PDFBox-Stil).
pub(crate) fn write_dict_block(out: &mut Vec<u8>, d: &Dictionary) -> Result<()> {
    write_dict_block_ctx(out, d, &mut Ctx::new(None))
}

fn write_dict_block_ctx(out: &mut Vec<u8>, d: &Dictionary, ctx: &mut Ctx<'_>) -> Result<()> {
    out.extend_from_slice(b"<<\n");
    for (k, v) in d.iter() {
        write_name(out, k);
        out.push(b' ');
        ctx.path.push(PathSegment::Key(k.clone()));
        let r = write_value(out, v, 1, ctx);
        ctx.path.pop();
        r?;
        out.push(b'\n');
    }
    out.extend_from_slice(b">>");
    Ok(())
}

/// Schreibt `n g obj … endobj`; `lits` erhält die Originalschreibweise unveränderter Reals.
pub(crate) fn write_indirect(out: &mut Vec<u8>, id: ObjectId, obj: &Object, lits: Option<&RealLiterals>) -> Result<()> {
    let mut ctx = Ctx::new(lits);
    let _ = writeln!(out, "{} {} obj", id.0, id.1);
    match obj {
        Object::Stream(s) => {
            let mut dict = s.dict.clone();
            dict.set("Length", Object::Integer(s.content.len() as i64));
            write_dict_block_ctx(out, &dict, &mut ctx)?;
            out.extend_from_slice(b"\nstream\r\n");
            out.extend_from_slice(&s.content);
            out.extend_from_slice(b"\r\nendstream\nendobj\n");
        }
        Object::Dictionary(d) => {
            write_dict_block_ctx(out, d, &mut ctx)?;
            out.extend_from_slice(b"\nendobj\n");
        }
        other => {
            write_value(out, other, 0, &mut ctx)?;
            out.extend_from_slice(b"\nendobj\n");
        }
    }
    Ok(())
}

/// `true` für Signatur-/Zeitstempel-Dictionaries (`/ByteRange` + String-`/Contents`).
pub(crate) fn is_signature_dict(d: &Dictionary) -> bool {
    d.has(b"ByteRange") && matches!(d.get(b"Contents"), Ok(Object::String(..)))
}

/// Verschlüsselt alle Strings und Streaminhalte eines Objekts mit dem Schlüssel von `id`.
/// `/Contents` von Signaturdictionaries bleibt unverschlüsselt (ISO 32000-2 §7.6.1).
pub(crate) fn encrypt_object(obj: &mut Object, id: ObjectId, enc: &Encryptor, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(IncError::Serialize("Objekt zu tief verschachtelt".into()));
    }
    let map = |e: pdfover_pdf_cos::CosError| IncError::Encryption(e.to_string());
    match obj {
        Object::String(bytes, format) => {
            *bytes = enc.encrypt(id, bytes, CryptTarget::String).map_err(map)?;
            *format = StringFormat::Hexadecimal;
        }
        Object::Array(a) => {
            for item in a.iter_mut() {
                encrypt_object(item, id, enc, depth + 1)?;
            }
        }
        Object::Dictionary(d) => encrypt_dict(d, id, enc, depth)?,
        Object::Stream(s) => {
            encrypt_dict(&mut s.dict, id, enc, depth)?;
            s.content = enc.encrypt(id, &s.content, CryptTarget::Stream).map_err(map)?;
        }
        _ => {}
    }
    Ok(())
}

fn encrypt_dict(d: &mut Dictionary, id: ObjectId, enc: &Encryptor, depth: usize) -> Result<()> {
    let sig = is_signature_dict(d);
    for (k, v) in d.iter_mut() {
        if sig && k.as_slice() == b"Contents" {
            continue;
        }
        encrypt_object(v, id, enc, depth + 1)?;
    }
    Ok(())
}

/// Komprimiert Daten mit zlib (Flate).
pub(crate) fn deflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).map_err(|err| IncError::Serialize(err.to_string()))?;
    e.finish().map_err(|err| IncError::Serialize(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ser(o: &Object) -> String {
        let mut v = Vec::new();
        write_direct(&mut v, o, 0).unwrap();
        String::from_utf8(v).unwrap()
    }

    #[test]
    fn primitives() {
        assert_eq!(ser(&Object::Real(1.5)), "1.5");
        assert_eq!(ser(&Object::Real(2.0)), "2");
        assert_eq!(ser(&Object::Real(-0.0)), "0");
        assert_eq!(ser(&Object::Real(0.000001)), "0.000001");
        assert_eq!(ser(&Object::Name(b"A B#".to_vec())), "/A#20B#23");
        assert_eq!(ser(&Object::String(b"a(b)\\".to_vec(), StringFormat::Literal)), "(a\\(b\\)\\\\)");
        assert_eq!(ser(&Object::String(b"\n\xff".to_vec(), StringFormat::Literal)), "<0AFF>");
        assert_eq!(ser(&Object::Reference((3, 1))), "3 1 R");
        let mut d = Dictionary::new();
        d.set("A", Object::Array(vec![1.into(), Object::Null, Object::Boolean(true)]));
        assert_eq!(ser(&Object::Dictionary(d)), "<< /A [1 null true] >>");
    }

    #[test]
    fn stream_must_be_indirect() {
        let s = Object::Stream(pdfover_pdf_cos::Stream::new(Dictionary::new(), vec![]));
        assert!(write_direct(&mut Vec::new(), &Object::Array(vec![s]), 0).is_err());
    }
}
