//! PDF/A-Kennung aus XMP (`pdfaid:part`, `pdfaid:conformance`; Plan §6.9).
//!
//! Kein vollständiger RDF-Parser: gesucht werden die Präfixe, die an den Namensraum
//! `http://www.aiim.org/pdfa/ns/id/` gebunden sind, und dann Element- (`<p:part>2</p:part>`) oder
//! Attributform (`p:part="2"`).

/// PDF/A-Kennung.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PdfAIdentification {
    /// `pdfaid:part` (1, 2, 3, 4)
    pub part: u8,
    /// `pdfaid:conformance` (A, B, U, …), falls vorhanden
    pub conformance: Option<String>,
}

const PDFAID_NS: &str = "http://www.aiim.org/pdfa/ns/id/";

fn prefixes(xmp: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xmp;
    while let Some(i) = rest.find("xmlns:") {
        let after = &rest[i + 6..];
        let Some(eq) = after.find('=') else { break };
        let prefix = after[..eq].trim();
        let val = after[eq + 1..].trim_start();
        let quote = val.chars().next();
        if let Some(q @ ('"' | '\'')) = quote
            && let Some(end) = val[1..].find(q)
        {
            let uri = &val[1..1 + end];
            if uri.trim() == PDFAID_NS && !prefix.is_empty() && !out.iter().any(|p| p == prefix) {
                out.push(prefix.to_string());
            }
        }
        rest = &after[eq..];
    }
    if out.is_empty() {
        out.push("pdfaid".into());
    }
    out
}

fn property(xmp: &str, prefix: &str, name: &str) -> Option<String> {
    let qname = format!("{prefix}:{name}");
    // Elementform
    let open = format!("<{qname}>");
    if let Some(i) = xmp.find(&open) {
        let start = i + open.len();
        if let Some(end) = xmp[start..].find('<') {
            let v = xmp[start..start + end].trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    // Attributform
    let mut rest = xmp;
    while let Some(i) = rest.find(&qname) {
        let before_ok = i == 0 || rest[..i].ends_with(|c: char| c.is_whitespace());
        let after = rest[i + qname.len()..].trim_start();
        if before_ok && let Some(val) = after.strip_prefix('=') {
            let val = val.trim_start();
            if let Some(q @ ('"' | '\'')) = val.chars().next()
                && let Some(end) = val[1..].find(q)
            {
                return Some(val[1..1 + end].trim().to_string());
            }
        }
        rest = &rest[i + qname.len()..];
    }
    None
}

/// Liest die PDF/A-Kennung aus einem XMP-Paket.
pub fn parse_pdfa_identification(xmp: &[u8]) -> Option<PdfAIdentification> {
    let text = String::from_utf8_lossy(xmp);
    for prefix in prefixes(&text) {
        if let Some(part) = property(&text, &prefix, "part") {
            let part: u8 = part.parse().ok()?;
            let conformance = property(&text, &prefix, "conformance");
            return Some(PdfAIdentification { part, conformance });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_and_attribute_forms() {
        let el = br#"<x:xmpmeta><rdf:RDF><rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
            <pdfaid:part>2</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        assert_eq!(parse_pdfa_identification(el), Some(PdfAIdentification { part: 2, conformance: Some("B".into()) }));
        let attr = br#"<rdf:Description xmlns:id='http://www.aiim.org/pdfa/ns/id/' id:part='1' id:conformance="A"/>"#;
        assert_eq!(
            parse_pdfa_identification(attr),
            Some(PdfAIdentification { part: 1, conformance: Some("A".into()) })
        );
        assert_eq!(parse_pdfa_identification(b"<x:xmpmeta/>"), None);
        let no_conf = br#"<rdf:Description xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/" pdfaid:part="3"/>"#;
        assert_eq!(parse_pdfa_identification(no_conf), Some(PdfAIdentification { part: 3, conformance: None }));
    }
}
