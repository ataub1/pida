//! Invariante I6: Das Increment enthält nur erlaubte Änderungen (Plan §3.6).
//!
//! Die Objekte des Increments bestimmt die Prüfung **unabhängig vom Writer** aus dem Querverweis
//! der Ausgabe (`SourcePdf::object_locations`: Offset ≥ Länge des Originals bzw. Object Stream
//! im Increment). Jedes fortgeschriebene Objekt wird mit seiner Originalfassung verglichen und nur
//! zugelassen, wenn sich ausschließlich Schlüssel seiner Rolle ändern:
//!
//! | Rolle (aus dem Original bestimmt) | erlaubte Änderung |
//! |---|---|
//! | Katalog | `/AcroForm`, `/OutputIntents` (nur wenn vorher keine), `/StructTreeRoot` (direkt) |
//! | AcroForm | `/Fields`, `/SigFlags`, `/DR`, `/DA` |
//! | Seite | `/Annots`, `/Tabs` |
//! | Seitenbaum-Wurzel (nur bei neuer Seite) | `/Kids`, `/Count` |
//! | befülltes Feld/Widget | `/V` (vorher leer, jetzt das neue Signaturdictionary), `/AP`, `/F`, `/StructParent` |
//! | `/DR` und dessen `/Font`/`/XObject` | nur hinzugefügte Schlüssel (`/FRM1` darf ersetzt werden) |
//! | StructTreeRoot | `/ParentTreeNextKey`, `/ParentTree` |
//! | Strukturelement | `/K` |
//! | Number-Tree-Knoten (ParentTree) | `/Nums`, `/Kids` |
//! | Arrays (`/Fields`, `/Annots`, `/Kids`, `/Nums`, `/K`) | nur angehängt |
//!
//! Neue Objekte: genau ein Signaturdictionary (das neue), keine neuen Kataloge, Seitenbäume oder
//! Object Streams, eine neue Seite nur, wenn sie geplant war. Der Trailer behält `/Root`, `/Info`
//! und `/Encrypt`.

use std::collections::{BTreeSet, HashSet};

use pdfover_pdf_cos::{Dictionary, Object, ObjectId, ObjectLocation, SourcePdf};

use crate::signed::{Invariant, InvariantViolation, SignedMeta};
use crate::util::{append_only, as_ref_id, changed_keys, key_text, same};

fn violation(detail: impl Into<String>) -> InvariantViolation {
    InvariantViolation::new(Invariant::I6AllowedChanges, detail)
}

/// Steht bei `off` der Objektkopf `n g obj` (führender Leerraum erlaubt)?
fn header_at(data: &[u8], off: usize, id: ObjectId) -> bool {
    let Some(rest) = data.get(off..) else { return false };
    let start = rest.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(rest.len());
    let head = format!("{} {} obj", id.0, id.1);
    rest[start..].starts_with(head.as_bytes())
}

/// Rollen der Objekte, die ein Signier-Increment fortschreiben darf.
#[derive(Default)]
struct Roles {
    catalog: Option<ObjectId>,
    acroform: Option<ObjectId>,
    pages_root: Option<ObjectId>,
    pages: HashSet<ObjectId>,
    dr: HashSet<ObjectId>,
    struct_root: Option<ObjectId>,
    arrays: HashSet<ObjectId>,
}

fn collect_roles(src: &SourcePdf) -> Roles {
    let mut r = Roles { catalog: src.catalog_id().ok(), ..Roles::default() };
    r.pages = src.pages().iter().map(|p| p.object_id).collect();
    for p in src.pages() {
        if let Some(a) = p.annots_indirect {
            r.arrays.insert(a);
        }
    }
    let Ok(catalog) = src.catalog() else { return r };
    r.pages_root = catalog.get(b"Pages").ok().and_then(as_ref_id);
    if let Some(root) = r.pages_root
        && let Ok(Object::Dictionary(d)) = src.get_object(root)
        && let Some(k) = d.get(b"Kids").ok().and_then(as_ref_id)
    {
        r.arrays.insert(k);
    }
    if let Ok(acro_obj) = catalog.get(b"AcroForm") {
        r.acroform = as_ref_id(acro_obj);
        if let Ok(Object::Dictionary(acro)) = src.resolve(acro_obj) {
            if let Some(f) = acro.get(b"Fields").ok().and_then(as_ref_id) {
                r.arrays.insert(f);
            }
            if let Ok(dr_obj) = acro.get(b"DR") {
                if let Some(id) = as_ref_id(dr_obj) {
                    r.dr.insert(id);
                }
                if let Ok(Object::Dictionary(dr)) = src.resolve(dr_obj) {
                    for k in [&b"Font"[..], b"XObject"] {
                        if let Some(id) = dr.get(k).ok().and_then(as_ref_id) {
                            r.dr.insert(id);
                        }
                    }
                }
            }
        }
    }
    r.struct_root = catalog.get(b"StructTreeRoot").ok().and_then(as_ref_id);
    r
}

/// Prüft ein fortgeschriebenes Dictionary: nur `allowed` ändern sich.
fn only_keys(id: ObjectId, old: &Dictionary, new: &Dictionary, allowed: &[&[u8]]) -> Result<(), InvariantViolation> {
    for k in changed_keys(old, new) {
        if !allowed.contains(&k.as_slice()) {
            return Err(violation(format!("Objekt {} {}: Schlüssel {} geändert", id.0, id.1, key_text(&k))));
        }
    }
    Ok(())
}

/// Nur hinzugefügte Schlüssel (bzw. ersetzbare `replaceable`).
fn only_added(
    id: ObjectId,
    old: &Dictionary,
    new: &Dictionary,
    replaceable: &[&[u8]],
) -> Result<(), InvariantViolation> {
    for (k, v) in old.iter() {
        match new.get(k) {
            Ok(w) if same(v, w) => {}
            Ok(_) if replaceable.contains(&k.as_slice()) => {}
            _ => return Err(violation(format!("Objekt {} {}: Schlüssel {} geändert", id.0, id.1, key_text(k)))),
        }
    }
    Ok(())
}

/// Arrays in einem Dictionary nur verlängert (direkt eingebettet).
fn arrays_appended(old: &Dictionary, new: &Dictionary, keys: &[&[u8]]) -> bool {
    keys.iter().all(|k| match (old.get(k), new.get(k)) {
        (Ok(Object::Array(a)), Ok(Object::Array(b))) => append_only(a, b),
        (Err(_), _) => true,
        (Ok(a), Ok(b)) => same(a, b),
        (Ok(_), Err(_)) => false,
    })
}

pub(crate) fn allowed_changes(src: &SourcePdf, new: &SourcePdf, meta: &SignedMeta) -> Result<(), InvariantViolation> {
    let base = src.len();
    let build = &meta.prepared.build;
    let roles = collect_roles(src);

    // Trailer
    for key in [&b"Root"[..], b"Info", b"Encrypt"] {
        let a = src.trailer().get(key).ok();
        let b = new.trailer().get(key).ok();
        let equal = match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => same(x, y),
            _ => false,
        };
        if !equal {
            return Err(violation(format!("Trailer-Schlüssel {} geändert", key_text(key))));
        }
    }

    // Objekte des Increments laut Querverweis der Ausgabe
    let locations = new.object_locations();
    let total = new.len();
    let mut in_increment: BTreeSet<ObjectId> = BTreeSet::new();
    let mut new_streams: HashSet<u32> = HashSet::new();
    for (id, loc) in &locations {
        // nur Offsets innerhalb des Increments, an denen auch der Objektkopf steht (defekte
        // Originale können Offsets hinter dem Dateiende oder mitten ins Increment eintragen –
        // solche Einträge gehören nicht zum Increment)
        if let ObjectLocation::Offset(off) = loc
            && (base..total).contains(off)
            && header_at(new.bytes(), *off as usize, *id)
        {
            in_increment.insert(*id);
            new_streams.insert(id.0);
        }
    }
    for (id, loc) in &locations {
        if let ObjectLocation::InStream { stream, .. } = loc
            && new_streams.contains(stream)
        {
            return Err(violation(format!("Objekt {} {} liegt in einem neuen Object Stream", id.0, id.1)));
        }
    }
    let old_ids: HashSet<ObjectId> = src.document().objects.keys().copied().collect();

    let mut sig_dicts = 0usize;
    let mut new_pages = 0usize;
    for id in &in_increment {
        let Ok(obj) = new.get_object(*id) else {
            // xref-Stream des Increments (lopdf hält ihn nicht im Objektmodell) bzw. Encrypt
            continue;
        };
        let dict = match obj {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        };
        if dict.is_some_and(|d| d.has_type(b"Sig") || d.has_type(b"DocTimeStamp")) {
            if *id != build.sig_dict_id {
                return Err(violation(format!("weiteres Signaturdictionary {} {}", id.0, id.1)));
            }
            sig_dicts += 1;
            continue;
        }
        if !old_ids.contains(id) {
            // neues Objekt
            if let Some(d) = dict {
                if d.has_type(b"Catalog") || d.has_type(b"Pages") || d.has_type(b"ObjStm") {
                    return Err(violation(format!("unerlaubtes neues Objekt {} {}", id.0, id.1)));
                }
                if d.has_type(b"XRef") {
                    continue;
                }
                if d.has_type(b"Page") {
                    if build.new_page_id != Some(*id) {
                        return Err(violation(format!("ungeplante neue Seite {} {}", id.0, id.1)));
                    }
                    new_pages += 1;
                }
            }
            continue;
        }
        // fortgeschriebenes Objekt
        let old = src.get_object(*id).map_err(|_| violation("Original-Objekt nicht lesbar"))?;
        check_changed(src, *id, old, obj, &roles, meta)?;
    }
    if sig_dicts != 1 {
        return Err(violation(format!("{sig_dicts} Signaturdictionaries im Increment statt einem")));
    }
    if build.new_page_id.is_some() && new_pages != 1 {
        return Err(violation("geplante neue Seite fehlt im Increment"));
    }
    Ok(())
}

fn check_changed(
    src: &SourcePdf,
    id: ObjectId,
    old: &Object,
    new: &Object,
    roles: &Roles,
    meta: &SignedMeta,
) -> Result<(), InvariantViolation> {
    let build = &meta.prepared.build;
    match (old, new) {
        (Object::Array(a), Object::Array(b)) => {
            if !append_only(a, b) {
                return Err(violation(format!("Array {} {} nicht nur verlängert", id.0, id.1)));
            }
            Ok(())
        }
        (Object::Dictionary(o), Object::Dictionary(n)) => {
            if Some(id) == roles.catalog {
                let had_intents = matches!(o.get(b"OutputIntents").ok().and_then(|x| src.resolve(x).ok()), Some(Object::Array(a)) if !a.is_empty());
                let mut allowed: Vec<&[u8]> = vec![b"AcroForm", b"StructTreeRoot"];
                if !had_intents {
                    allowed.push(b"OutputIntents");
                }
                if o.get(b"StructTreeRoot").ok().and_then(as_ref_id).is_some() {
                    allowed.retain(|k| *k != b"StructTreeRoot");
                }
                return only_keys(id, o, n, &allowed);
            }
            if Some(id) == roles.acroform {
                only_keys(id, o, n, &[b"Fields", b"SigFlags", b"DR", b"DA"])?;
                if !arrays_appended(o, n, &[b"Fields"]) {
                    return Err(violation("/Fields nicht nur verlängert"));
                }
                return Ok(());
            }
            if roles.pages.contains(&id) {
                only_keys(id, o, n, &[b"Annots", b"Tabs"])?;
                if !arrays_appended(o, n, &[b"Annots"]) {
                    return Err(violation(format!("Seite {} {}: /Annots nicht nur verlängert", id.0, id.1)));
                }
                return Ok(());
            }
            if Some(id) == roles.pages_root && build.new_page_id.is_some() {
                only_keys(id, o, n, &[b"Kids", b"Count"])?;
                if !arrays_appended(o, n, &[b"Kids"]) {
                    return Err(violation("/Kids der Seitenbaum-Wurzel nicht nur verlängert"));
                }
                return Ok(());
            }
            if !build.new_field && (id == build.field_id || id == build.widget_id) {
                only_keys(id, o, n, &[b"V", b"AP", b"F", b"StructParent"])?;
                if id == build.field_id {
                    let had_value = o.get(b"V").is_ok_and(|v| !matches!(v, Object::Null));
                    if had_value || n.get(b"V").ok().and_then(as_ref_id) != Some(build.sig_dict_id) {
                        return Err(violation("/V des Zielfelds unerwartet"));
                    }
                }
                return Ok(());
            }
            if roles.dr.contains(&id) {
                return only_added(id, o, n, &[b"FRM1"]);
            }
            if Some(id) == roles.struct_root {
                return only_keys(id, o, n, &[b"ParentTreeNextKey", b"ParentTree"]);
            }
            if o.has_type(b"StructElem") || (o.has(b"S") && o.has(b"P")) {
                only_keys(id, o, n, &[b"K"])?;
                if !arrays_appended(o, n, &[b"K"]) && o.get(b"K").ok().is_some_and(|k| matches!(k, Object::Array(_))) {
                    return Err(violation("/K eines Strukturelements nicht nur verlängert"));
                }
                return Ok(());
            }
            if o.has(b"Nums") || o.has(b"Kids") && !o.has_type(b"Pages") {
                only_keys(id, o, n, &[b"Nums", b"Kids"])?;
                if !arrays_appended(o, n, &[b"Nums", b"Kids"]) {
                    return Err(violation("Number Tree nicht nur verlängert"));
                }
                return Ok(());
            }
            let ty = o.get(b"Type").ok().and_then(|t| t.as_name().ok()).map(key_text).unwrap_or_else(|| "-".into());
            let keys: Vec<String> = changed_keys(o, n).iter().map(|k| key_text(k)).collect();
            Err(violation(format!(
                "Objekt {} {} (Typ {ty}) unerwartet fortgeschrieben, Schlüssel {}",
                id.0,
                id.1,
                keys.join(" ")
            )))
        }
        _ => {
            if roles.arrays.contains(&id) || same(old, new) {
                return Ok(());
            }
            Err(violation(format!("Objekt {} {} unerwartet fortgeschrieben", id.0, id.1)))
        }
    }
}
