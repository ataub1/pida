//! `multipart/form-data` (RFC 7578) in fester Teil-Reihenfolge.
//!
//! Selbst kodiert statt `reqwest::multipart`, damit Reihenfolge, Header je Teil und
//! `Content-Length` exakt feststehen (A-Trust: `fileupload` zuerst, lokale BKU: `XMLRequest` zuerst,
//! docs/analyse/02 §3.3/§4.3).

use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::FormPart;

/// Kodiert die Teile; liefert den `Content-Type` (mit Boundary) und den Körper.
pub(crate) fn encode(parts: &[FormPart]) -> (String, Vec<u8>) {
    let boundary = boundary_for(parts);
    let mut out = Vec::with_capacity(parts.iter().map(|p| p.data.len() + 160).sum::<usize>() + 64);
    for part in parts {
        out.extend_from_slice(b"--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"\r\nContent-Disposition: form-data; name=\"");
        out.extend_from_slice(escape_quoted(&part.name).as_bytes());
        out.push(b'"');
        if let Some(filename) = &part.filename {
            out.extend_from_slice(b"; filename=\"");
            out.extend_from_slice(escape_quoted(filename).as_bytes());
            out.push(b'"');
        }
        out.extend_from_slice(b"\r\n");
        if let Some(content_type) = &part.content_type {
            out.extend_from_slice(b"Content-Type: ");
            out.extend_from_slice(strip_crlf(content_type).as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&part.data);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"--\r\n");
    (format!("multipart/form-data; boundary={boundary}"), out)
}

/// Feld- und Dateinamen in Anführungszeichen (WHATWG: `"` → `%22`, CR → `%0D`, LF → `%0A`).
fn escape_quoted(value: &str) -> String {
    value.replace('"', "%22").replace('\r', "%0D").replace('\n', "%0A")
}

fn strip_crlf(value: &str) -> String {
    value.chars().filter(|c| *c != '\r' && *c != '\n').collect()
}

/// Zufällige Boundary, die in keinem Teil vorkommt.
fn boundary_for(parts: &[FormPart]) -> String {
    loop {
        let candidate = random_boundary();
        let needle = candidate.as_bytes();
        let clash =
            parts.iter().any(|p| p.data.windows(needle.len()).any(|w| w == needle) || p.name.contains(&candidate));
        if !clash {
            return candidate;
        }
    }
}

fn random_boundary() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = std::collections::hash_map::RandomState::new();
    let mut words = [0u64; 2];
    for (i, word) in words.iter_mut().enumerate() {
        let mut hasher = state.build_hasher();
        hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        hasher.write_usize(i);
        if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            hasher.write_u128(d.as_nanos());
        }
        *word = hasher.finish();
    }
    format!("----PDFOverNG{:016x}{:016x}", words[0], words[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_layout_in_given_order() {
        let parts = vec![
            FormPart::file("fileupload", "sign.pdf", "application/pdf", b"%PDF\x00\x00".to_vec()),
            FormPart::text("XMLRequest", "<x/>"),
        ];
        let (ct, body) = encode(&parts);
        let boundary = ct.strip_prefix("multipart/form-data; boundary=").unwrap();
        let expected = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"fileupload\"; filename=\"sign.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF\0\0\r\n--{b}\r\nContent-Disposition: form-data; name=\"XMLRequest\"\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n<x/>\r\n--{b}--\r\n",
            b = boundary
        );
        assert_eq!(body, expected.as_bytes());
    }

    #[test]
    fn names_are_escaped_and_boundaries_unique() {
        let part = FormPart { name: "a\"b\r\nc".into(), filename: None, content_type: None, data: vec![] };
        let (_, body) = encode(std::slice::from_ref(&part));
        let text = String::from_utf8(body).unwrap();
        assert!(text.contains("name=\"a%22b%0D%0Ac\"\r\n\r\n"), "{text}");
        assert_ne!(random_boundary(), random_boundary());
    }

    #[test]
    fn empty_form() {
        let (ct, body) = encode(&[]);
        let b = ct.split("boundary=").nth(1).unwrap();
        assert_eq!(body, format!("--{b}--\r\n").as_bytes());
    }
}
