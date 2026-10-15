//! `analyze` und die Annahmepolitik an synthetischen Dokumenten (Plan §6.1): DocMDP P=1/2/3,
//! `/Lock`, Seed Values, XFA (hybrid/dynamisch), Usage Rights, Fremdinhalt nach `%%EOF`.
//! Die Dokumente entstehen als Increment über `simple.pdf` (`pdf-incremental`).

mod common;

use common::*;
use pdfover_pades::{AcceptanceClass, FieldTarget, PadesError, SignPlan, analyze, codes, empty_fields, prepare};
use pdfover_pdf_cos::{Dictionary, Limits, Object, SourcePdf, StringFormat};
use pdfover_pdf_incremental::{IncrementBuilder, IncrementOptions};

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn text(s: &str) -> Object {
    Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
}

fn base() -> SourcePdf {
    open(&testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf"))
}

/// Increment über `src`, das der Katalog-Callback und weitere Objekte bestimmen.
fn derive(src: &SourcePdf, f: impl FnOnce(&mut IncrementBuilder<'_>, &mut Dictionary)) -> SourcePdf {
    let mut b = IncrementBuilder::new(src, IncrementOptions::default());
    let mut catalog = src.catalog().unwrap().clone();
    f(&mut b, &mut catalog);
    b.update_object(src.catalog_id().unwrap(), Object::Dictionary(catalog)).unwrap();
    let inc = b.build().unwrap();
    SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap()
}

/// Signaturdictionary (Inhalt beliebig; nicht prüfbar/ungültig) mit optionaler DocMDP-Referenz.
fn sig_dict(docmdp: Option<i64>) -> Object {
    let mut d = Dictionary::new();
    d.set("Type", name("Sig"));
    d.set("Filter", name("Adobe.PPKLite"));
    d.set("SubFilter", name("ETSI.CAdES.detached"));
    d.set("ByteRange", Object::Array(vec![0.into(), 10.into(), 20.into(), 10.into()]));
    d.set("Contents", Object::String(vec![0x30, 0x03, 0x02, 0x01, 0x01], StringFormat::Hexadecimal));
    if let Some(p) = docmdp {
        let mut tp = Dictionary::new();
        tp.set("Type", name("TransformParams"));
        tp.set("P", Object::Integer(p));
        tp.set("V", name("1.2"));
        let mut r = Dictionary::new();
        r.set("Type", name("SigRef"));
        r.set("TransformMethod", name("DocMDP"));
        r.set("TransformParams", Object::Dictionary(tp));
        d.set("Reference", Object::Array(vec![Object::Dictionary(r)]));
    }
    Object::Dictionary(d)
}

fn sig_field(t: &str, value: Option<Object>, extra: &[(&str, Object)]) -> Dictionary {
    let mut w = Dictionary::new();
    w.set("FT", name("Sig"));
    w.set("Type", name("Annot"));
    w.set("Subtype", name("Widget"));
    w.set("F", Object::Integer(4));
    w.set("T", text(t));
    w.set("Rect", Object::Array(vec![100.into(), 100.into(), 300.into(), 150.into()]));
    if let Some(v) = value {
        w.set("V", v);
    }
    for (k, v) in extra {
        w.set(*k, v.clone());
    }
    w
}

fn with_fields(src: &SourcePdf, fields: Vec<Dictionary>, extra_acro: &[(&str, Object)]) -> SourcePdf {
    derive(src, |b, catalog| {
        let ids: Vec<Object> =
            fields.into_iter().map(|f| Object::Reference(b.add_object(Object::Dictionary(f)))).collect();
        let mut acro = Dictionary::new();
        acro.set("Fields", Object::Array(ids));
        acro.set("SigFlags", Object::Integer(3));
        for (k, v) in extra_acro {
            acro.set(*k, v.clone());
        }
        catalog.set("AcroForm", Object::Dictionary(acro));
    })
}

fn codes_of(r: &pdfover_pades::DocReport) -> Vec<String> {
    r.findings.iter().map(|f| f.code.clone()).collect()
}

#[test]
fn plain_document_is_k1_without_findings() {
    let r = analyze(&base()).unwrap();
    assert_eq!(r.class, AcceptanceClass::K1Regular);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    assert_eq!(r.next_field_number, 1);
    assert!(!r.is_signed());
}

#[test]
fn docmdp_levels() {
    let src = base();
    for (p, class, code) in [
        (1, AcceptanceClass::K5Rejected, codes::DOCMDP_P1),
        (2, AcceptanceClass::K4Restricted, codes::DOCMDP_P2),
        (3, AcceptanceClass::K4Restricted, codes::DOCMDP_P3),
    ] {
        let doc = derive(&src, |b, catalog| {
            let sig = b.add_object(sig_dict(Some(p)));
            let mut perms = Dictionary::new();
            perms.set("DocMDP", Object::Reference(sig));
            catalog.set("Perms", Object::Dictionary(perms));
        });
        let r = analyze(&doc).unwrap();
        assert_eq!(r.doc_mdp.map(|d| d.permission), Some(p as u8));
        assert!(codes_of(&r).contains(&code.to_string()), "{:?}", r.findings);
        assert_eq!(r.class, class);
        let plan = SignPlan::invisible(pdfover_pades::local_signing_time());
        match (p, prepare(&doc, &plan, None)) {
            (1, Err(PadesError::Rejected(c))) => assert_eq!(c, codes::DOCMDP_P1),
            (1, other) => panic!("P=1 nicht abgelehnt: {other:?}"),
            (_, r) => assert!(r.is_ok(), "P={p}: {r:?}"),
        }
    }
}

#[test]
fn lock_marks_fields_read_only_and_prepare_refuses_them() {
    let src = base();
    let mut lock = Dictionary::new();
    lock.set("Type", name("SigFieldLock"));
    lock.set("Action", name("Include"));
    lock.set("Fields", Object::Array(vec![text("Gesperrt")]));
    let doc = derive(&src, |b, catalog| {
        let sig = b.add_object(sig_dict(None));
        let signed = sig_field("Erste", Some(Object::Reference(sig)), &[("Lock", Object::Dictionary(lock))]);
        let locked = sig_field("Gesperrt", None, &[]);
        let free = sig_field("Frei", None, &[]);
        let ids: Vec<Object> = [signed, locked, free]
            .into_iter()
            .map(|f| Object::Reference(b.add_object(Object::Dictionary(f))))
            .collect();
        let mut acro = Dictionary::new();
        acro.set("Fields", Object::Array(ids));
        acro.set("SigFlags", Object::Integer(3));
        catalog.set("AcroForm", Object::Dictionary(acro));
    });
    let r = analyze(&doc).unwrap();
    assert!(codes_of(&r).contains(&codes::FIELD_LOCKED.to_string()), "{:?}", r.findings);
    assert!(codes_of(&r).contains(&codes::PRIOR_SIGNATURE_INVALID.to_string()), "{:?}", r.findings);
    assert_eq!(r.class, AcceptanceClass::K4Restricted);
    let empty: Vec<String> = empty_fields(&r).into_iter().map(|c| c.qualified_name).collect();
    assert_eq!(empty, vec!["Frei".to_string()]);
    let locked = r.fields.iter().find(|f| f.qualified_name == "Gesperrt").unwrap();
    let plan = SignPlan {
        field: FieldTarget::Existing {
            field_id: locked.field_id,
            widget_id: locked.widgets[0].id,
            qualified_name: locked.qualified_name.clone(),
        },
        ..SignPlan::invisible(pdfover_pades::local_signing_time())
    };
    assert!(matches!(prepare(&doc, &plan, None), Err(PadesError::FieldLocked(_))));
    // das signierte Feld selbst ist kein Ziel
    let first = r.fields.iter().find(|f| f.qualified_name == "Erste").unwrap();
    let plan = SignPlan {
        field: FieldTarget::Existing {
            field_id: first.field_id,
            widget_id: first.widgets[0].id,
            qualified_name: first.qualified_name.clone(),
        },
        ..plan
    };
    assert!(matches!(prepare(&doc, &plan, None), Err(PadesError::FieldAlreadySigned(_))));
}

#[test]
fn seed_values() {
    let src = base();
    let mut ok_sv = Dictionary::new();
    ok_sv.set("SubFilter", Object::Array(vec![name("ETSI.CAdES.detached")]));
    ok_sv.set("Ff", Object::Integer(2));
    let r =
        analyze(&with_fields(&src, vec![sig_field("SV", None, &[("SV", Object::Dictionary(ok_sv))])], &[])).unwrap();
    assert_eq!(codes_of(&r), vec![codes::SEED_VALUE.to_string()]);
    assert!(!r.findings[0].warning);

    let mut bad_sv = Dictionary::new();
    bad_sv.set("SubFilter", Object::Array(vec![name("adbe.pkcs7.detached")]));
    bad_sv.set("Ff", Object::Integer(2));
    let r =
        analyze(&with_fields(&src, vec![sig_field("SV", None, &[("SV", Object::Dictionary(bad_sv))])], &[])).unwrap();
    assert_eq!(codes_of(&r), vec![codes::SEED_VALUE_INCOMPATIBLE.to_string()]);
    assert!(r.findings[0].warning);
    assert_eq!(r.class, AcceptanceClass::K4Restricted);
}

#[test]
fn xfa_usage_rights_and_need_appearances() {
    let src = base();
    let xfa = with_fields(&src, vec![], &[("XFA", Object::Array(vec![])), ("NeedAppearances", Object::Boolean(true))]);
    let r = analyze(&xfa).unwrap();
    assert_eq!(r.xfa, pdfover_pades::XfaKind::Hybrid);
    assert!(r.need_appearances);
    assert!(codes_of(&r).contains(&codes::XFA_HYBRID.to_string()));
    assert!(r.warnings().any(|f| f.code == codes::XFA_HYBRID));

    let dynamic = derive(&src, |_, c| c.set("NeedsRendering", Object::Boolean(true)));
    let r = analyze(&dynamic).unwrap();
    assert_eq!((r.xfa, r.class), (pdfover_pades::XfaKind::Dynamic, AcceptanceClass::K5Rejected));
    let plan = SignPlan::invisible(pdfover_pades::local_signing_time());
    assert!(matches!(prepare(&dynamic, &plan, None), Err(PadesError::Rejected(c)) if c == codes::XFA_DYNAMIC));

    let ur = derive(&src, |_, c| {
        let mut perms = Dictionary::new();
        perms.set("UR3", Object::Dictionary(Dictionary::new()));
        c.set("Perms", Object::Dictionary(perms));
    });
    let r = analyze(&ur).unwrap();
    assert!(r.usage_rights && codes_of(&r).contains(&codes::USAGE_RIGHTS.to_string()));
    assert_eq!(r.class, AcceptanceClass::K1Regular);
}

#[test]
fn garbage_after_eof_is_k2() {
    let mut bytes = std::fs::read(testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf")).unwrap();
    bytes.extend_from_slice(b"\nDas ist kein PDF-Inhalt, sondern Fremdinhalt.\n");
    let src = SourcePdf::open(bytes, &Limits::default()).unwrap();
    let r = analyze(&src).unwrap();
    assert_eq!(r.tail, pdfover_pdf_cos::TailKind::Garbage);
    assert!(codes_of(&r).contains(&codes::TAIL_GARBAGE.to_string()));
    assert_eq!(r.class, AcceptanceClass::K2Repairable);
    // K2 braucht eine Zustimmung (Plan §6.1)
    assert!(r.warnings().any(|f| f.code == codes::TAIL_GARBAGE));
}

#[test]
fn field_numbering_continues_after_existing_names() {
    let src = base();
    let doc =
        with_fields(&src, vec![sig_field("PDF-AS Signatur7", None, &[]), sig_field("Signature1", None, &[])], &[]);
    let r = analyze(&doc).unwrap();
    assert_eq!(r.next_field_number, 8);
    let p = prepare(&doc, &SignPlan::invisible(pdfover_pades::local_signing_time()), None).unwrap();
    assert_eq!(p.field_name(), "PDF-AS Signatur8");
    // expliziter, bereits vergebener Name
    let plan = SignPlan {
        field: FieldTarget::New { name: Some("Signature1".into()) },
        ..SignPlan::invisible(pdfover_pades::local_signing_time())
    };
    assert!(matches!(prepare(&doc, &plan, None), Err(PadesError::InvalidPlan(_))));
}
