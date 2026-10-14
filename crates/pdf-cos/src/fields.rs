//! Signaturfelder im AcroForm-Baum (docs/analyse/03 §4.4/§4.5, Plan §6.4).
//!
//! Abweichend von PDF-AS (nur oberste Ebene von `/Fields`) wird der ganze Feldbaum durchlaufen;
//! Namen werden voll qualifiziert (`a.b.c`). Widgets sind entweder mit dem Feld verschmolzen oder
//! eigene `/Kids` ohne `/T`.

use std::collections::{HashMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::pages::{rect_from, resolve};
use crate::text::decode_text;
use crate::{CosError, Limits, PageInfo, Result, SigField, SigWidget};

struct Ctx<'a> {
    doc: &'a Document,
    limits: &'a Limits,
    page_index: HashMap<ObjectId, usize>,
    /// Widget-ID → Seite (aus `/Annots`), lazy
    annot_pages: Option<HashMap<ObjectId, usize>>,
    pages: &'a [PageInfo],
    visited: HashSet<ObjectId>,
    out: Vec<SigField>,
}

#[derive(Clone, Default)]
struct Inherit {
    ft: Option<Vec<u8>>,
    ff: Option<u32>,
    names: Vec<String>,
}

/// Widget-Annotation (verschmolzen oder eigenes Kind).
fn is_widget(d: &Dictionary) -> bool {
    d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Widget") || d.has(b"Rect")
}

fn int(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<i64> {
    d.get(key).ok().and_then(|o| resolve(doc, o, 8)).and_then(|o| o.as_i64().ok())
}

impl Ctx<'_> {
    fn annot_page(&mut self, widget: ObjectId) -> Option<usize> {
        if self.annot_pages.is_none() {
            let mut map = HashMap::new();
            for p in self.pages {
                let Ok(Object::Dictionary(page)) = self.doc.get_object(p.object_id) else { continue };
                let Some(Object::Array(annots)) = page.get(b"Annots").ok().and_then(|o| resolve(self.doc, o, 8)) else {
                    continue;
                };
                for a in annots {
                    if let Object::Reference(aid) = a {
                        map.entry(*aid).or_insert(p.index);
                    }
                }
            }
            self.annot_pages = Some(map);
        }
        self.annot_pages.as_ref().and_then(|m| m.get(&widget).copied())
    }

    fn widget(&mut self, id: ObjectId, dict: &Dictionary) -> SigWidget {
        let page_index = match dict.get(b"P") {
            Ok(Object::Reference(p)) => self.page_index.get(p).copied(),
            _ => None,
        }
        .or_else(|| self.annot_page(id));
        let rect = dict.get(b"Rect").ok().and_then(|o| rect_from(self.doc, o)).unwrap_or_default();
        let annot_flags = int(self.doc, dict, b"F").map(|f| (f & 0xFFFF_FFFF) as u32).unwrap_or(0);
        SigWidget { id, page_index, rect, annot_flags }
    }

    fn visit(&mut self, id: ObjectId, inherited: &Inherit, depth: usize) -> Result<()> {
        if depth as u64 > u64::from(self.limits.max_depth) {
            return Err(CosError::LimitExceeded {
                what: "Tiefe des Feldbaums",
                limit: u64::from(self.limits.max_depth),
            });
        }
        if !self.visited.insert(id) {
            return Ok(());
        }
        let doc = self.doc;
        let Ok(Object::Dictionary(dict)) = doc.get_object(id) else { return Ok(()) };

        let mut inh = inherited.clone();
        if let Ok(Object::Name(ft)) = dict.get(b"FT").map(|o| resolve(doc, o, 8).unwrap_or(&Object::Null)) {
            inh.ft = Some(ft.clone());
        }
        if let Some(ff) = int(doc, dict, b"Ff") {
            inh.ff = Some((ff & 0xFFFF_FFFF) as u32);
        }
        let partial = dict.get(b"T").ok().and_then(|o| resolve(doc, o, 8)).and_then(decode_text);
        if let Some(t) = &partial {
            inh.names.push(t.clone());
        }

        // Kinder: mit /T → Unterfelder, ohne /T (Widget-Annotation) → Widgets dieses Felds.
        let mut child_fields = Vec::new();
        let mut widgets = Vec::new();
        if let Some(Object::Array(kids)) = dict.get(b"Kids").ok().and_then(|o| resolve(doc, o, 8)) {
            for k in kids {
                let Object::Reference(kid) = k else { continue };
                let Ok(Object::Dictionary(kd)) = doc.get_object(*kid) else { continue };
                if !kd.has(b"T") && is_widget(kd) {
                    widgets.push((*kid, kd));
                } else {
                    child_fields.push(*kid);
                }
            }
        }
        let is_terminal = child_fields.is_empty();
        for kid in child_fields {
            self.visit(kid, &inh, depth + 1)?;
        }

        if is_terminal && inh.ft.as_deref() == Some(b"Sig") {
            let mut sig_widgets = Vec::new();
            if is_widget(dict) {
                sig_widgets.push(self.widget(id, dict));
            }
            for (wid, wd) in widgets {
                sig_widgets.push(self.widget(wid, wd));
            }
            let v = dict.get(b"V").ok();
            let signed = matches!(v, Some(o) if !matches!(resolve(doc, o, 8), None | Some(Object::Null)));
            let value_id = match v {
                Some(Object::Reference(r)) => Some(*r),
                _ => None,
            };
            self.out.push(SigField {
                qualified_name: inh.names.join("."),
                partial_name: partial,
                field_id: id,
                depth,
                widgets: sig_widgets,
                signed,
                value_id,
                has_lock: dict.has(b"Lock"),
                has_seed_value: dict.has(b"SV"),
                field_flags: inh.ff.unwrap_or(0),
            });
        }
        Ok(())
    }
}

/// Alle Signaturfelder in Baumreihenfolge.
pub(crate) fn signature_fields(
    doc: &Document,
    catalog: &Dictionary,
    pages: &[PageInfo],
    limits: &Limits,
) -> Result<Vec<SigField>> {
    let Some(Object::Dictionary(acro)) = catalog.get(b"AcroForm").ok().and_then(|o| resolve(doc, o, limits.max_depth))
    else {
        return Ok(Vec::new());
    };
    let Some(Object::Array(fields)) = acro.get(b"Fields").ok().and_then(|o| resolve(doc, o, limits.max_depth)) else {
        return Ok(Vec::new());
    };
    let mut ctx = Ctx {
        doc,
        limits,
        page_index: pages.iter().map(|p| (p.object_id, p.index)).collect(),
        annot_pages: None,
        pages,
        visited: HashSet::new(),
        out: Vec::new(),
    };
    for f in fields {
        if let Object::Reference(id) = f {
            ctx.visit(*id, &Inherit::default(), 0)?;
        }
    }
    Ok(ctx.out)
}
