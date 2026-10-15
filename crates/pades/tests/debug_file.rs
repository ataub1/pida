//! Diagnose (nicht Teil der Prüfung): signiert `PIDA_DEBUG_FILE` sichtbar und zeigt die
//! Signaturfelder vor/nach sowie das Ergebnis der Selbstprüfung.
//! `PIDA_DEBUG_FILE=<pdf> cargo test -p pida-pades --test debug_file -- --ignored --nocapture`

mod common;

use common::*;
use pida_cades::CmsProfile;
use pida_pades::{CheckContext, FieldTarget, analyze};
use pida_pdf_cos::{Limits, SourcePdf};

#[test]
#[ignore]
fn debug_file() {
    let Some(path) = std::env::var_os("PIDA_DEBUG_FILE") else { return };
    let src = open(std::path::Path::new(&path));
    let report = analyze(&src).unwrap();
    eprintln!("Bericht: {:?} Felder {:?}", report.findings, report.fields);
    let mut signer = KeystoreSigner::open("test-ecp256");
    let chain = signer.chain.clone();
    let (prepared, planned) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_DE",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    eprintln!("Plan: {:?}", planned.plan);
    eprintln!("Prepared: {prepared:?}");
    let signed = signer.sign(prepared).unwrap();
    let out = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
    eprintln!("Felder nachher: {:?}", out.signature_fields());
    eprintln!("Quelle: health {:?} revisions {:?} prev {}", src.xref_health(), src.revisions(), src.prev_offset());
    eprintln!("Ausgabe: health {:?} revisions {:?}", out.xref_health(), out.revisions());
    if let Some(n) = std::env::var("PIDA_DEBUG_OBJ").ok().and_then(|v| v.parse::<u32>().ok()) {
        eprintln!("Objekt {n} vorher: {:?}", src.get_object((n, 0)));
        eprintln!("Objekt {n} nachher: {:?}", out.get_object((n, 0)));
        eprintln!("Lage nachher: {:?}", out.object_locations().into_iter().find(|(id, _)| id.0 == n));
        eprintln!("Lage vorher: {:?}", src.object_locations().into_iter().find(|(id, _)| id.0 == n));
    }
    let cat = out.catalog_id().unwrap();
    eprintln!("Katalog {cat:?} Lage {:?}", out.object_locations().into_iter().find(|(id, _)| *id == cat));
    eprintln!(
        "Katalog nachher: {:?}",
        out.catalog().map(|c| c.iter().map(|(k, _)| String::from_utf8_lossy(k).into_owned()).collect::<Vec<_>>())
    );
    eprintln!("Check: {:?}", signed.self_check(&CheckContext { source: &src, expected_signer: None, renderer: None }));
    if let Some(keep) = std::env::var_os("PIDA_KEEP") {
        std::fs::write(std::path::PathBuf::from(keep).join("debug-signed.pdf"), signed.bytes()).unwrap();
    }
}

/// Wie [`debug_file`], aber unsichtbar (kein Renderer, kein Signaturblock) – für Eingaben, deren
/// sichtbare Planung scheitert. Schreibt die Ausgabe mit `PIDA_KEEP=<dir>`.
#[test]
#[ignore]
fn debug_file_invisible() {
    let Some(path) = std::env::var_os("PIDA_DEBUG_FILE") else { return };
    let src = open(std::path::Path::new(&path));
    let mut signer = KeystoreSigner::open("test-ecp256");
    let chain = signer.chain.clone();
    let plan = pida_pades::SignPlan::invisible(pida_pades::local_signing_time());
    let prepared = pida_pades::prepare(&src, &plan, Some(&chain)).unwrap();
    let signed = signer.sign(prepared).unwrap();
    eprintln!("Quelle: health {:?} revisions {:?} prev {}", src.xref_health(), src.revisions(), src.prev_offset());
    match SourcePdf::open(signed.bytes(), &Limits::default()) {
        Ok(out) => {
            eprintln!("Ausgabe: health {:?} Felder {:?}", out.xref_health(), out.signature_fields());
            if let Some(n) = std::env::var("PIDA_DEBUG_OBJ").ok().and_then(|v| v.parse::<u32>().ok()) {
                eprintln!("Objekt {n} vorher: {:?}", src.get_object((n, 0)));
                eprintln!("Objekt {n} nachher: {:?}", out.get_object((n, 0)));
            }
        }
        Err(e) => eprintln!("Ausgabe nicht lesbar: {e}"),
    }
    eprintln!("Check: {:?}", signed.self_check(&CheckContext { source: &src, expected_signer: None, renderer: None }));
    if let Some(keep) = std::env::var_os("PIDA_KEEP") {
        std::fs::write(std::path::PathBuf::from(keep).join("debug-signed.pdf"), signed.bytes()).unwrap();
    }
}
