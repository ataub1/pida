//! Kleiner, namespace-aufgelöster Elementbaum für SL-Antworten (quick-xml `NsReader`).
//!
//! Sicherheitsregeln: keine DTD (`<!DOCTYPE …>` → Fehler), nur die fünf vordefinierten Entities und
//! Zeichenreferenzen, Schachtelungstiefe begrenzt.

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

use crate::{Result, SlError};

/// Höchste Schachtelungstiefe (SL-Antworten sind flach).
const MAX_DEPTH: usize = 64;

/// Element mit aufgelöstem Namespace.
#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub namespace: Option<String>,
    pub local: String,
    /// Attribute ohne Namespace (lokaler Name → Wert); `xmlns`-Deklarationen fehlen.
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// Verketteter Textinhalt (Text, CDATA, Referenzen) direkt in diesem Element.
    pub text: String,
}

impl Element {
    pub fn child<'a>(&'a self, namespace: &'a str, local: &'a str) -> Option<&'a Element> {
        self.children_named(namespace, local).next()
    }

    pub fn children_named<'a>(&'a self, namespace: &'a str, local: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.namespace.as_deref() == Some(namespace) && c.local == local)
    }

    pub fn attribute(&self, local: &str) -> Option<&str> {
        self.attributes.iter().find(|(k, _)| k == local).map(|(_, v)| v.as_str())
    }
}

fn xml_err(e: impl std::fmt::Display) -> SlError {
    SlError::Xml(e.to_string())
}

fn input_str(xml: &[u8]) -> Result<&str> {
    let xml = xml.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(xml);
    std::str::from_utf8(xml).map_err(|_| SlError::Xml("kein gültiges UTF-8".into()))
}

fn resolve_ns(result: ResolveResult<'_>) -> Result<Option<String>> {
    match result {
        ResolveResult::Bound(ns) => Ok(Some(ns.into_inner().to_owned())),
        ResolveResult::Unbound => Ok(None),
        ResolveResult::Unknown(prefix) => Err(SlError::Xml(format!("unbekanntes Präfix {prefix}"))),
    }
}

fn start_element(reader: &NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Element> {
    let (ns, local) = reader.resolver().resolve_element(start.name());
    let mut element =
        Element { namespace: resolve_ns(ns)?, local: local.into_inner().to_owned(), ..Element::default() };
    for attr in start.attributes() {
        let attr = attr.map_err(xml_err)?;
        let key = attr.key.into_inner();
        if key == "xmlns" || key.starts_with("xmlns:") {
            continue;
        }
        let (attr_ns, attr_local) = reader.resolver().resolve_attribute(attr.key);
        if !matches!(attr_ns, ResolveResult::Unbound) {
            continue; // nur unqualifizierte Attribute (z. B. `Key`) sind relevant
        }
        let value = attr.normalized_value(quick_xml::XmlVersion::Implicit1_0).map_err(xml_err)?;
        element.attributes.push((attr_local.into_inner().to_owned(), value.into_owned()));
    }
    Ok(element)
}

fn resolve_reference(name: &str) -> Result<char> {
    Ok(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let code = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok()
            } else if let Some(dec) = name.strip_prefix('#') {
                dec.parse::<u32>().ok()
            } else {
                return Err(SlError::Xml(format!("unbekannte Entity &{name};")));
            };
            code.and_then(char::from_u32).ok_or_else(|| SlError::Xml(format!("ungültige Zeichenreferenz &{name};")))?
        }
    })
}

/// Parst ein vollständiges Dokument und liefert das Wurzelelement.
pub(crate) fn parse(xml: &[u8]) -> Result<Element> {
    let mut reader = NsReader::from_str(input_str(xml)?);
    reader.config_mut().check_end_names = true;
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    loop {
        let event = reader.read_event().map_err(xml_err)?;
        match event {
            Event::Start(start) => {
                if root.is_some() {
                    return Err(SlError::Xml("Inhalt nach dem Wurzelelement".into()));
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(SlError::Xml("zu tief geschachtelt".into()));
                }
                let element = start_element(&reader, &start)?;
                stack.push(element);
            }
            Event::Empty(start) => {
                if root.is_some() {
                    return Err(SlError::Xml("Inhalt nach dem Wurzelelement".into()));
                }
                let element = start_element(&reader, &start)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => root = Some(element),
                }
            }
            Event::End(_) => {
                let element = stack.pop().ok_or_else(|| SlError::Xml("unerwartetes Endtag".into()))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => root = Some(element),
                }
            }
            Event::Text(text) => {
                let content = text.xml10_content();
                match stack.last_mut() {
                    Some(current) => current.text.push_str(&content),
                    None if content.trim().is_empty() => {}
                    None => return Err(SlError::Xml("Text außerhalb des Wurzelelements".into())),
                }
            }
            Event::CData(cdata) => {
                let content = cdata.xml10_content();
                match stack.last_mut() {
                    Some(current) => current.text.push_str(&content),
                    None => return Err(SlError::Xml("CDATA außerhalb des Wurzelelements".into())),
                }
            }
            Event::GeneralRef(reference) => {
                let c = resolve_reference(&reference.xml10_content())?;
                match stack.last_mut() {
                    Some(current) => current.text.push(c),
                    None => return Err(SlError::Xml("Referenz außerhalb des Wurzelelements".into())),
                }
            }
            Event::DocType(_) => return Err(SlError::Xml("DTD nicht erlaubt".into())),
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return Err(SlError::Xml("Dokument endet vor dem Endtag".into()));
    }
    root.ok_or_else(|| SlError::Xml("kein Wurzelelement".into()))
}

/// Namespace und lokaler Name des Wurzelelements, ohne das ganze Dokument zu prüfen.
pub(crate) fn root_name(xml: &[u8]) -> Option<(Option<String>, String)> {
    let mut reader = NsReader::from_str(input_str(xml).ok()?);
    loop {
        match reader.read_event().ok()? {
            Event::Start(start) | Event::Empty(start) => {
                let element = start_element(&reader, &start).ok()?;
                return Some((element.namespace, element.local));
            }
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) => {}
            Event::Text(t) if t.xml10_content().trim().is_empty() => {}
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_and_text() {
        let xml = br#"<?xml version="1.0"?><!-- k --><a:R xmlns:a="urn:a" xmlns="urn:d"><Child K="v&amp;w" a:Q="x">t&lt;1&#x41;<![CDATA[<c>]]></Child><a:E/></a:R>"#;
        let root = parse(xml).unwrap();
        assert_eq!((root.namespace.as_deref(), root.local.as_str()), (Some("urn:a"), "R"));
        let child = root.child("urn:d", "Child").unwrap();
        assert_eq!(child.attribute("K"), Some("v&w"));
        assert_eq!(child.attribute("Q"), None, "qualifizierte Attribute werden ignoriert");
        assert_eq!(child.text, "t<1A<c>");
        assert!(root.child("urn:a", "E").is_some());
        assert_eq!(root_name(xml), Some((Some("urn:a".into()), "R".into())));
    }

    #[test]
    fn rejects_dtd_entities_and_garbage() {
        assert!(parse(b"<!DOCTYPE r [<!ENTITY x \"y\">]><r>&x;</r>").is_err());
        assert!(parse(b"<r>&x;</r>").is_err());
        assert!(parse(b"<r><a></r>").is_err());
        assert!(parse(b"<r/><s/>").is_err());
        assert!(parse(b"text").is_err());
        assert!(parse(b"").is_err());
        assert!(parse(b"<p:r/>").is_err(), "unbekanntes Präfix");
        assert!(parse(b"\xff\xfe<r/>").is_err());
        let deep = format!("{}{}", "<a>".repeat(MAX_DEPTH + 1), "</a>".repeat(MAX_DEPTH + 1));
        assert!(parse(deep.as_bytes()).is_err());
        assert!(root_name(b"<!DOCTYPE r><r/>").is_none());
        assert!(parse(b"\xEF\xBB\xBF<r/>").is_ok(), "BOM wird übersprungen");
    }
}
