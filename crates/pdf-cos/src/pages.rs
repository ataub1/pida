//! Seitenbaum mit geerbten Attributen (ISO 32000-1 §7.7.3.4; docs/analyse/04 §4.6).
//!
//! Wie PDFBox: fehlende `/MediaBox` → US-Letter, `/CropBox` wird auf die MediaBox beschnitten,
//! `/Rotate` außerhalb von Vielfachen von 90 gilt als 0.

use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::{CosError, Limits, PageInfo, Rect, Result, Rotation};

/// Default-MediaBox (PDFBox `PDRectangle.LETTER`).
pub(crate) const LETTER: Rect = Rect::new(0.0, 0.0, 612.0, 792.0);

pub(crate) fn resolve<'a>(doc: &'a Document, obj: &'a Object, max_depth: u32) -> Option<&'a Object> {
    let mut o = obj;
    for _ in 0..=max_depth {
        match o {
            Object::Reference(id) => o = doc.get_object(*id).ok()?,
            _ => return Some(o),
        }
    }
    None
}

pub(crate) fn number(doc: &Document, obj: &Object) -> Option<f32> {
    match resolve(doc, obj, 8)? {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) if r.is_finite() => Some(*r),
        _ => None,
    }
}

/// Rechteck aus einem Array mit vier Zahlen (normalisiert).
pub(crate) fn rect_from(doc: &Document, obj: &Object) -> Option<Rect> {
    let arr = resolve(doc, obj, 8)?.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let v: Vec<f32> = arr.iter().take(4).map(|o| number(doc, o)).collect::<Option<_>>()?;
    Some(Rect::new(v[0], v[1], v[2], v[3]))
}

#[derive(Debug, Clone, Default)]
struct Inherited {
    media_box: Option<Rect>,
    crop_box: Option<Rect>,
    rotate: Option<i64>,
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let a = a.normalized();
    let b = b.normalized();
    Rect::new(a.llx.max(b.llx), a.lly.max(b.lly), a.urx.min(b.urx), a.ury.min(b.ury))
}

/// Durchläuft den Seitenbaum ab `/Root /Pages`.
pub(crate) fn collect_pages(doc: &Document, catalog: &Dictionary, limits: &Limits) -> Result<Vec<PageInfo>> {
    let root_ref = catalog.get(b"Pages").map_err(|_| CosError::InvalidStructure("Katalog ohne /Pages".into()))?;
    let root_id = match root_ref {
        Object::Reference(id) => *id,
        _ => return Err(CosError::InvalidStructure("/Pages ist keine Referenz".into())),
    };
    let mut pages = Vec::new();
    let mut visited = HashSet::new();
    walk(doc, root_id, Inherited::default(), 0, limits, &mut visited, &mut pages)?;
    Ok(pages)
}

fn walk(
    doc: &Document,
    id: ObjectId,
    inherited: Inherited,
    depth: u32,
    limits: &Limits,
    visited: &mut HashSet<ObjectId>,
    out: &mut Vec<PageInfo>,
) -> Result<()> {
    if depth > limits.max_depth {
        return Err(CosError::LimitExceeded { what: "Tiefe des Seitenbaums", limit: u64::from(limits.max_depth) });
    }
    if !visited.insert(id) {
        // Zyklus oder mehrfach referenzierte Seite: ignorieren (PDFBox ebenso).
        return Ok(());
    }
    let Ok(Object::Dictionary(node)) = doc.get_object(id) else {
        return Ok(()); // fehlende/defekte Knoten überspringen (lenient)
    };
    let mut inh = inherited;
    if let Some(r) = node.get(b"MediaBox").ok().and_then(|o| rect_from(doc, o)) {
        inh.media_box = Some(r.normalized());
    }
    if let Some(r) = node.get(b"CropBox").ok().and_then(|o| rect_from(doc, o)) {
        inh.crop_box = Some(r.normalized());
    }
    if let Some(r) = node.get(b"Rotate").ok().and_then(|o| number(doc, o)) {
        inh.rotate = Some(r as i64);
    }

    let kids = node.get(b"Kids").ok().and_then(|o| resolve(doc, o, limits.max_depth)).and_then(|o| o.as_array().ok());
    // Blatt ohne /Type gilt nachsichtig als Seite (PDFBox ebenso), eines mit fremdem /Type (z. B.
    // der Katalog selbst nach `/Pages 1 0 R`, Mutationstest) nicht: sonst schriebe ein Increment
    // `/Annots` in den Katalog.
    let foreign_type =
        node.get(b"Type").ok().and_then(|t| t.as_name().ok()).is_some_and(|t| t != b"Page" && t != b"Pages");
    let is_page = node.has_type(b"Page") || (kids.is_none() && !node.has_type(b"Pages") && !foreign_type);
    if is_page {
        let media_box = inh.media_box.unwrap_or(LETTER);
        let crop_box = inh.crop_box.map(|c| intersect(c, media_box)).unwrap_or(media_box);
        let rotation = inh.rotate.and_then(Rotation::from_degrees).unwrap_or_default();
        let user_unit = node.get(b"UserUnit").ok().and_then(|o| number(doc, o)).filter(|u| *u > 0.0).unwrap_or(1.0);
        let annots_indirect = match node.get(b"Annots") {
            Ok(Object::Reference(aid)) if matches!(doc.get_object(*aid), Ok(Object::Array(_))) => Some(*aid),
            _ => None,
        };
        out.push(PageInfo {
            index: out.len(),
            object_id: id,
            media_box,
            crop_box,
            rotation,
            user_unit,
            annots_indirect,
        });
        return Ok(());
    }
    if let Some(kids) = kids {
        for kid in kids {
            if let Object::Reference(kid_id) = kid {
                walk(doc, *kid_id, inh.clone(), depth + 1, limits, visited, out)?;
            }
        }
    }
    Ok(())
}

/// Geerbtes Attribut einer Seite (`/Resources` usw.) über die `/Parent`-Kette.
pub(crate) fn inherited_attr<'a>(
    doc: &'a Document,
    page_id: ObjectId,
    key: &[u8],
    limits: &Limits,
) -> Option<&'a Object> {
    let mut id = page_id;
    let mut seen = HashSet::new();
    for _ in 0..=limits.max_depth {
        if !seen.insert(id) {
            return None;
        }
        let dict = doc.get_object(id).ok()?.as_dict().ok()?;
        if let Ok(v) = dict.get(key) {
            return resolve(doc, v, limits.max_depth);
        }
        match dict.get(b"Parent") {
            Ok(Object::Reference(p)) => id = *p,
            _ => return None,
        }
    }
    None
}
