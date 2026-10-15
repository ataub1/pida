//! Vorhandene Signaturen: Signaturdictionary lesen, ByteRange-Daten streamen, Status ermitteln
//! (Invariante I4, Plan §3.6; `cades::verify_detached` mit `AlgorithmPolicy::PriorSignature`).

use std::io::Read;

use pdfover_cades::{AlgorithmPolicy, CmsError, Expect, ExpectedSigner, verify_detached};
use pdfover_pdf_cos::{Dictionary, Object, ObjectId, SigField, SourcePdf, decode_text_string};
use pdfover_pdf_incremental::ByteRange;

use crate::signed::PriorStatus;

/// Inhalt eines Signaturdictionarys (ohne Personendaten im `Debug`).
#[derive(Clone)]
pub(crate) struct SigDictInfo {
    /// ID des Signaturdictionarys (falls indirekt)
    pub id: Option<ObjectId>,
    /// `/Type` (`Sig`, `DocTimeStamp`)
    pub kind: Option<Vec<u8>>,
    /// `/SubFilter`
    pub sub_filter: Option<String>,
    /// `/ByteRange` (alle Zahlen)
    pub byte_range_raw: Option<Vec<i64>>,
    /// `/Contents` (roh, mit Padding)
    pub contents: Vec<u8>,
    /// `/Location`
    pub location: Option<String>,
}

impl std::fmt::Debug for SigDictInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigDictInfo")
            .field("id", &self.id)
            .field("sub_filter", &self.sub_filter)
            .field("byte_range", &self.byte_range_raw)
            .field("contents_len", &self.contents.len())
            .finish_non_exhaustive()
    }
}

impl SigDictInfo {
    /// ByteRange mit genau vier nichtnegativen Zahlen.
    pub fn byte_range(&self) -> Option<ByteRange> {
        let v = self.byte_range_raw.as_ref()?;
        if v.len() != 4 || v.iter().any(|x| *x < 0) {
            return None;
        }
        Some(ByteRange([v[0] as u64, v[1] as u64, v[2] as u64, v[3] as u64]))
    }

    /// `/Contents` ohne `0`-Padding (DER), falls trennbar.
    pub fn der(&self) -> Vec<u8> {
        pdfover_cades::trim_contents_padding(&self.contents).map(<[u8]>::to_vec).unwrap_or_default()
    }

    /// Ende der abgedeckten Bytes (`max(off + len)` über alle Paare).
    pub fn covered_end(&self) -> Option<u64> {
        let v = self.byte_range_raw.as_ref()?;
        if v.is_empty() || !v.len().is_multiple_of(2) || v.iter().any(|x| *x < 0) {
            return None;
        }
        v.chunks(2).map(|p| (p[0] as u64).checked_add(p[1] as u64)).collect::<Option<Vec<_>>>()?.into_iter().max()
    }
}

/// Signaturdictionary eines (signierten) Felds.
pub(crate) fn read_sig_dict(src: &SourcePdf, field: &SigField) -> Option<SigDictInfo> {
    let (id, dict) = match field.value_id {
        Some(id) => (Some(id), src.get_object(id).ok()?.as_dict().ok()?.clone()),
        None => {
            let fd = src.get_object(field.field_id).ok()?.as_dict().ok()?;
            let v = src.resolve(fd.get(b"V").ok()?).ok()?;
            (None, v.as_dict().ok()?.clone())
        }
    };
    Some(sig_info_from_dict(src, id, &dict))
}

pub(crate) fn sig_info_from_dict(src: &SourcePdf, id: Option<ObjectId>, dict: &Dictionary) -> SigDictInfo {
    let name = |k: &[u8]| {
        dict.get(k).ok().and_then(|o| src.resolve(o).ok()).and_then(|o| o.as_name().ok()).map(<[u8]>::to_vec)
    };
    let byte_range_raw = dict.get(b"ByteRange").ok().and_then(|o| src.resolve(o).ok()).and_then(|o| match o {
        Object::Array(a) => a.iter().map(|x| crate::util::int_of(src, x)).collect::<Option<Vec<i64>>>(),
        _ => None,
    });
    let contents = match dict.get(b"Contents").ok().and_then(|o| src.resolve(o).ok()) {
        Some(Object::String(b, _)) => b.clone(),
        _ => Vec::new(),
    };
    SigDictInfo {
        id,
        kind: name(b"Type"),
        sub_filter: name(b"SubFilter").map(|n| String::from_utf8_lossy(&n).into_owned()),
        byte_range_raw,
        contents,
        location: dict.get(b"Location").ok().and_then(|o| src.resolve(o).ok()).and_then(decode_text_string),
    }
}

/// Liest die ByteRange-Abschnitte von `data` nacheinander (gestreamt, ohne Kopie).
pub(crate) struct RangeReader<'a> {
    data: &'a [u8],
    ranges: Vec<(usize, usize)>,
    idx: usize,
    pos: usize,
}

impl<'a> RangeReader<'a> {
    /// `pairs` = `[off0, len0, off1, len1, …]`; `None`, wenn ein Abschnitt außerhalb liegt.
    pub fn new(data: &'a [u8], pairs: &[i64]) -> Option<Self> {
        if pairs.is_empty() || !pairs.len().is_multiple_of(2) {
            return None;
        }
        let mut ranges = Vec::with_capacity(pairs.len() / 2);
        for p in pairs.chunks(2) {
            let off = usize::try_from(p[0]).ok()?;
            let len = usize::try_from(p[1]).ok()?;
            if off.checked_add(len)? > data.len() {
                return None;
            }
            ranges.push((off, len));
        }
        Some(Self { data, ranges, idx: 0, pos: 0 })
    }
}

impl Read for RangeReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while let Some(&(off, len)) = self.ranges.get(self.idx) {
            if self.pos >= len {
                self.idx += 1;
                self.pos = 0;
                continue;
            }
            let n = (len - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.data[off + self.pos..off + self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        Ok(0)
    }
}

/// Verfahren, die der eigene Verifier prüfen kann.
fn verifiable_sub_filter(sf: Option<&str>) -> bool {
    matches!(sf, Some("ETSI.CAdES.detached" | "adbe.pkcs7.detached"))
}

/// Status einer Signatur über `data` (ganze Datei): gültig, ungültig oder nicht prüfbar; dazu
/// der Fehlercode bei ungültigen bzw. nicht prüfbaren Signaturen.
pub(crate) fn signature_status(data: &[u8], info: &SigDictInfo) -> (PriorStatus, Option<String>) {
    if info.kind.as_deref() == Some(b"DocTimeStamp") || !verifiable_sub_filter(info.sub_filter.as_deref()) {
        return (PriorStatus::NotVerifiable, Some("pdf-signature-subfilter".into()));
    }
    let Some(pairs) = info.byte_range_raw.as_deref() else {
        return (PriorStatus::Invalid, Some("pdf-signature-byterange".into()));
    };
    let Some(mut reader) = RangeReader::new(data, pairs) else {
        return (PriorStatus::Invalid, Some("pdf-signature-byterange".into()));
    };
    let expect = Expect {
        signer: ExpectedSigner::Any,
        require_signing_certificate: false,
        max_len: None,
        policy: AlgorithmPolicy::PriorSignature,
    };
    match verify_detached(&info.contents, &mut reader, &expect) {
        Ok(_) => (PriorStatus::Valid, None),
        Err(e @ CmsError::UnsupportedAlgorithm(_)) => (PriorStatus::NotVerifiable, Some(e.code().into())),
        Err(e) => (PriorStatus::Invalid, Some(e.code().into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_reader_streams_pairs() {
        let data = b"0123456789";
        let mut r = RangeReader::new(data, &[0, 3, 6, 4]).unwrap();
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"0126789");
        assert!(RangeReader::new(data, &[0, 3, 8, 4]).is_none());
        assert!(RangeReader::new(data, &[0, 3, 1]).is_none());
        assert!(RangeReader::new(data, &[-1, 3]).is_none());
    }

    #[test]
    fn covered_end_and_byte_range() {
        let info = SigDictInfo {
            id: None,
            kind: Some(b"Sig".to_vec()),
            sub_filter: None,
            byte_range_raw: Some(vec![0, 10, 20, 5]),
            contents: vec![],
            location: None,
        };
        assert_eq!(info.covered_end(), Some(25));
        assert_eq!(info.byte_range(), Some(ByteRange([0, 10, 20, 5])));
        assert_eq!(signature_status(b"", &info).0, PriorStatus::NotVerifiable);
        let bad = SigDictInfo { sub_filter: Some("ETSI.CAdES.detached".into()), ..info };
        assert_eq!(signature_status(b"short", &bad).0, PriorStatus::Invalid);
    }
}
