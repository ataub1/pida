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

/// Mutationstest (Welle 3): der Kopf des `/XRefStm`-Streams eines Hybrid-Abschnitts trägt die
/// Kennung des Katalogs (`7 0 obj` → `1 0 obj`). Der Katalog bleibt der aus der Tabelle (der
/// xref-Stream überschreibt ihn im Objektmodell nicht).
#[test]
fn xref_stream_header_with_foreign_id_does_not_replace_the_catalog() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/pdf/generated/simple-xref-hybrid.pdf");
    let mut data = std::fs::read(path).unwrap();
    let pat = b"7 0 obj\n<< /Type /XRef";
    let pos = data.windows(pat.len()).position(|w| w == pat).expect("xref-Stream-Kopf");
    data[pos] = b'1';
    let src = pdfover_pdf_cos::SourcePdf::open(data, &pdfover_pdf_cos::Limits::default()).unwrap();
    let cat = src.catalog().unwrap();
    assert_eq!(cat.get(b"Type").unwrap().as_name().unwrap(), b"Catalog");
    assert_eq!(src.page_count(), 1);
}

/// Wie oben für einen reinen xref-Stream: Kopf `13 0 obj` → `8 0 obj` (Kennung des Katalogs, gleich lang).
#[test]
fn xref_stream_only_header_with_foreign_id_does_not_replace_the_catalog() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/pdf/generated/simple-xref-stream.pdf");
    let mut data = std::fs::read(path).unwrap();
    let pat = b"13 0 obj\n<< /Type /XRef";
    let pos = data.windows(pat.len()).position(|w| w == pat).expect("xref-Stream-Kopf");
    data[pos..pos + 2].copy_from_slice(b" 8");
    let src = pdfover_pdf_cos::SourcePdf::open(data, &pdfover_pdf_cos::Limits::default()).unwrap();
    let cat = src.catalog().unwrap();
    assert_eq!(cat.get(b"Type").unwrap().as_name().unwrap(), b"Catalog");
    assert_eq!(src.page_count(), 1);
}

/// Mutationstest (Welle 3): ein Byte in den komprimierten Daten des `/XRefStm`-Streams macht aus
/// Objekt 5 einen Eintrag „komprimiert in Objekt 3“ (die Seite, kein Object Stream). Die Datei
/// öffnet weiter, gilt aber als repariert (K2) – vorher „intakt“, und die Selbstprüfung meldete
/// beim Fortschreiben der Seite I6 („Objekt liegt in einem neuen Object Stream“).
#[test]
fn compressed_entry_in_a_non_object_stream_is_repaired() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/pdf/generated/simple-xref-hybrid.pdf");
    let mut data = std::fs::read(path).unwrap();
    assert_eq!(data[628], 0x63, "Testdatei unverändert");
    data[628] = 0x96;
    let src = pdfover_pdf_cos::SourcePdf::open(data, &pdfover_pdf_cos::Limits::default()).unwrap();
    assert_eq!(src.xref_health(), pdfover_pdf_cos::XrefHealth::Repaired);
    assert_eq!(src.page_count(), 1);
}

/// Mutationstest (Welle 3): fehlendes `/StrF` bzw. `/StmF` bedeutet nach ISO 32000-1 Tabelle 20
/// `/Identity` (lopdf 0.45 nahm RC4). Strings bzw. Streams werden dann weder gelesen noch beim
/// Fortschreiben verschlüsselt; der jeweils andere Filter bleibt AESV2. Ein unbekannter Name ist
/// ein Fehler (K5), kein stilles RC4.
#[test]
fn missing_or_unknown_crypt_filter_names_follow_the_spec() {
    use pdfover_pdf_cos::{CosError, CryptTarget, Limits, SourcePdf};
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/pdf/generated/simple-aes128-owner.pdf");
    let original = std::fs::read(path).unwrap();
    let patched = |from: &[u8], to: &[u8]| {
        let mut d = original.clone();
        let pos = d.windows(from.len()).position(|w| w == from).expect("Muster im Encrypt-Dictionary");
        d[pos..pos + to.len()].copy_from_slice(to);
        d
    };
    let plain = b"NUR TEST";
    // Referenz: beide Filter AESV2 (IV + Chiffrat, länger als der Klartext)
    let src = SourcePdf::open(original.clone(), &Limits::default()).unwrap();
    let enc = src.encryptor().unwrap();
    assert!(enc.encrypt((1, 0), plain, CryptTarget::String).unwrap().len() > plain.len());
    for (from, to, string_identity, stream_identity) in [
        (&b"/StrF /StdCF"[..], &b"/XXXX /StdCF"[..], true, false),
        (b"/StmF /StdCF", b"/XXXX /StdCF", false, true),
        (b"/StmF /StdCF /StrF /StdCF", b"/XXXX /StdCF /YYYY /StdCF", true, true),
    ] {
        let label = String::from_utf8_lossy(to).into_owned();
        let src = SourcePdf::open(patched(from, to), &Limits::default()).unwrap_or_else(|e| panic!("{label}: {e}"));
        let enc = src.encryptor().expect("verschlüsselt");
        for (target, identity) in [(CryptTarget::String, string_identity), (CryptTarget::Stream, stream_identity)] {
            let c = enc.encrypt((1, 0), plain, target).unwrap();
            assert_eq!(c == plain, identity, "{label} {target:?}");
            assert_eq!(enc.decrypt((1, 0), &c, target).unwrap(), plain, "{label} {target:?}");
        }
    }
    // unbekannter Filtername: abgelehnt
    match SourcePdf::open(patched(b"/StrF /StdCF", b"/StrF /StdCX"), &Limits::default()) {
        Err(CosError::UnsupportedSecurityHandler(m)) => assert!(m.contains("StdCX"), "{m}"),
        Err(e) => panic!("unerwarteter Fehler {e}"),
        Ok(_) => panic!("unbekannter Crypt-Filter darf nicht still als RC4 gelten"),
    }
}
