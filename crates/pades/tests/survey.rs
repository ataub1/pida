//! Diagnose (nicht Teil der Prüfung): `analyze` über den ganzen Korpus mit Klasse, Befunden,
//! xref-Art, Signaturen und Feldern je Datei (nur Kennzahlen, keine Inhalte).
//! `cargo test -p pida-pades --test survey -- --ignored --nocapture`

mod common;

use common::*;
use pida_pades::analyze;
use pida_pdf_cos::{Limits, SourcePdf};

#[test]
#[ignore]
fn survey() {
    for p in corpus() {
        let name = rel(&p);
        match SourcePdf::open_file(&p, &Limits::default()) {
            Err(e) => eprintln!("{name}: OPEN {e}"),
            Ok(src) => {
                let r = analyze(&src).unwrap();
                let empty = pida_pades::empty_fields(&r).len();
                eprintln!(
                    "{name}: {:?} {:?} pages={} xref={:?} enc={} sigs={} empty={} tagged={} pdfa={:?} next={} statuses={:?}",
                    r.class,
                    r.findings.iter().map(|f| f.code.clone()).collect::<Vec<_>>(),
                    r.page_count,
                    r.xref_kind,
                    r.encrypted,
                    r.signatures.len(),
                    empty,
                    r.tagged,
                    r.pdfa.as_ref().map(|p| p.part),
                    r.next_field_number,
                    r.signatures
                        .iter()
                        .map(|s| (s.status.clone(), s.covers_revision, s.covers_whole_file))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
