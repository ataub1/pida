//! Additive API aus Welle 2 (für `pdfover-pades`): `SourcePdf::signature_locations` (benutzte
//! QR-Platzhalter) und `SourcePdf::object_locations` (Objekte eines Increments, Invariante I6).

use std::path::PathBuf;

use pdfover_pdf_cos::{Limits, ObjectLocation, SourcePdf};

fn open(rel: &str) -> SourcePdf {
    let p = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel);
    SourcePdf::open_file(&p, &Limits::default()).unwrap()
}

#[test]
fn signature_locations_of_pdfas_placeholder_signatures() {
    // PDF-AS hat zweimal auf QR-Platzhalter signiert: /Location = Ressourcenname
    let src = open("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf");
    let locs = src.signature_locations().unwrap();
    assert_eq!(locs.len(), 2, "{locs:?}");
    assert!(locs.iter().all(|l| l.starts_with("Image")), "{locs:?}");
    assert_ne!(locs[0], locs[1]);
    // ohne Signaturen
    assert!(open("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf").signature_locations().unwrap().is_empty());
    // Signatur ohne /Location
    assert!(open("pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf").signature_locations().unwrap().is_empty());
}

#[test]
fn object_locations_cover_offsets_and_object_streams() {
    let src = open("pdf/generated/simple-xref-stream.pdf");
    let locs = src.object_locations();
    assert!(!locs.is_empty());
    let len = src.len();
    let mut offsets = 0;
    let mut in_stream = 0;
    for (id, loc) in &locs {
        match loc {
            ObjectLocation::Offset(o) => {
                offsets += 1;
                assert!(*o < len, "{id:?} {o}");
                let at = &src.bytes()[*o as usize..];
                assert!(at.starts_with(format!("{} {} obj", id.0, id.1).as_bytes()), "{id:?}");
            }
            ObjectLocation::InStream { stream, .. } => {
                in_stream += 1;
                assert_eq!(id.1, 0);
                assert!(locs.iter().any(|(c, l)| c.0 == *stream && matches!(l, ObjectLocation::Offset(_))));
            }
        }
    }
    assert!(offsets > 0 && in_stream > 0, "{offsets} {in_stream}");
    // sortiert nach Objektnummer
    assert!(locs.windows(2).all(|w| w[0].0.0 < w[1].0.0));
}
