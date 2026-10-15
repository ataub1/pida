//! Korpus-Test (Paket A): jedes PDF unter `testdata/` bekommt ein Increment mit
//! Dummy-Signaturdictionary und leerem Widget. Geprüft werden Präfix-Invariante (I1), ByteRange
//! (I2), Re-Parse mit pdf-cos **und** lopdf (I5), Format des Querverweises und – falls installiert –
//! `qpdf --check` ohne neue Befunde.

mod common;

use pida_pdf_cos::{CosError, Limits, Object, SourcePdf, XrefKind};
use pida_pdf_incremental::{IncrementOptions, RESERVE_MODEL_B, WrittenKind};

/// Prüft ein fertiges Increment strukturell; liefert die Ausgabe.
fn check_structure(name: &str, src: &SourcePdf, inc: &pida_pdf_incremental::Increment) -> Vec<u8> {
    let original = src.bytes();
    let out = inc.to_vec(original);

    // I1: Präfix, CRLF am Anfang
    assert_eq!(&out[..original.len()], original, "{name}: Präfix verändert");
    assert!(inc.bytes.starts_with(b"\r\n"), "{name}: Increment beginnt nicht mit CRLF");
    assert_eq!(inc.base_len, src.len());
    assert_eq!(inc.total_len(), out.len() as u64);

    // I2: ByteRange
    let br = inc.byte_range.0;
    assert!(inc.byte_range.is_consistent_with(out.len() as u64), "{name}: ByteRange {br:?} inkonsistent");
    assert_eq!(out[br[1] as usize], b'<', "{name}: br[1] zeigt nicht auf '<'");
    assert_eq!(out[br[2] as usize - 1], b'>', "{name}: br[2]-1 zeigt nicht auf '>'");
    assert_eq!(br[1], inc.contents_range.start);
    assert_eq!(br[2], inc.contents_range.end);
    let hex = &out[br[1] as usize + 1..br[2] as usize - 1];
    assert_eq!(hex.len(), 2 * inc.reserve());
    assert!(hex.iter().all(|&b| b == b'0'), "{name}: Platzhalter nicht '0'-gefüllt");
    assert_eq!(inc.byte_range.covered_len(), out.len() as u64 - (br[2] - br[1]));
    let field = common::byte_range_text(&out, inc);
    assert_eq!(field.len(), 36);
    assert!(field.starts_with(&inc.byte_range.to_pdf_string()), "{name}: ByteRange-Text {field:?}");
    assert!(field[inc.byte_range.to_pdf_string().len()..].bytes().all(|b| b == b' '), "{name}: Padding");
    assert_eq!(out[inc.byte_range_field.end as usize], b'\n');
    let ex = inc.excluded_range();
    assert_eq!((ex.from, ex.to), (br[1], br[2] - 1));
    // Die ByteRange-Daten sind genau alles außer der Lücke.
    let data = inc.byte_range_data(original);
    assert_eq!(data.len() as u64, br[1] + br[3]);
    assert_eq!(&data[..br[1] as usize], &out[..br[1] as usize]);
    assert_eq!(&data[br[1] as usize..], &out[br[2] as usize..]);

    // Rollen
    assert_eq!(inc.written.iter().filter(|(_, k)| *k == WrittenKind::SignatureDictionary).count(), 1);
    out
}

/// Re-Parse mit pdf-cos und lopdf.
fn check_reparse(name: &str, src: &SourcePdf, inc: &pida_pdf_incremental::Increment, out: &[u8]) {
    let re =
        SourcePdf::open(out.to_vec(), &Limits::default()).unwrap_or_else(|e| panic!("{name}: Re-Parse pdf-cos: {e}"));
    assert_eq!(re.page_count(), src.page_count(), "{name}: Seitenzahl");
    assert_eq!(re.revisions().len(), src.revisions().len() + 1, "{name}: Revisionen");
    let last = re.last_revision();
    assert_eq!(last.prev, Some(src.last_revision().startxref), "{name}: /Prev");
    assert_eq!(last.xref_kind, src.xref_kind(), "{name}: xref-Format wie Vorrevision");
    assert_eq!(re.tail().kind, pida_pdf_cos::TailKind::Clean, "{name}: Dateiende");
    assert_eq!(re.xref_health(), src.xref_health(), "{name}: Querverweis-Zustand");
    if let Some([id0, _]) = src.trailer_id() {
        assert_eq!(re.trailer_id().unwrap()[0], id0, "{name}: /ID[0] beibehalten");
    }
    assert_ne!(re.trailer_id().unwrap()[1], src.trailer_id().map(|[_, b]| b).unwrap_or_default(), "{name}: /ID[1] neu");
    assert_eq!(re.encryption().is_some(), src.encryption().is_some(), "{name}: /Encrypt beibehalten");
    assert!(last.size >= src.next_object_number(), "{name}: /Size");

    let sig_id = inc.signature_dict_id.unwrap();
    let fields = re.signature_fields().unwrap();
    let f = fields.iter().find(|f| f.value_id == Some(sig_id)).unwrap_or_else(|| panic!("{name}: neues Feld fehlt"));
    assert!(f.signed);
    assert_eq!(f.widgets.len(), 1);
    assert_eq!(f.widgets[0].page_index, Some(0), "{name}: Widget-Seite");
    assert_eq!(f.widgets[0].annot_flags, 132);
    assert_eq!(fields.len(), src.signature_fields().unwrap().len() + 1);

    // Signaturdictionary im Objektmodell (bei Verschlüsselung entschlüsselt, /Contents roh)
    let sd = re.get_object(sig_id).unwrap().as_dict().unwrap();
    assert_eq!(sd.get(b"Type").unwrap().as_name().unwrap(), b"Sig");
    assert_eq!(sd.get(b"SubFilter").unwrap().as_name().unwrap(), b"ETSI.CAdES.detached");
    let contents = sd.get(b"Contents").unwrap().as_str().unwrap();
    assert_eq!(contents.len(), inc.reserve(), "{name}: /Contents-Länge");
    assert!(contents.iter().all(|&b| b == 0), "{name}: /Contents nicht unverschlüsselt");
    let br: Vec<i64> = sd.get(b"ByteRange").unwrap().as_array().unwrap().iter().map(|o| o.as_i64().unwrap()).collect();
    assert_eq!(br, inc.byte_range.0.iter().map(|&v| v as i64).collect::<Vec<_>>());
    let nm = pida_pdf_cos::decode_text_string(sd.get(b"Name").unwrap()).unwrap();
    assert_eq!(nm, common::TEST_NAME, "{name}: /Name (Verschlüsselung)");
    assert_eq!(sd.get(b"M").unwrap().as_str().unwrap(), common::signing_time().as_bytes());
    // Schlüsselreihenfolge wie PDF-AS
    let keys: Vec<&[u8]> = sd.iter().map(|(k, _)| k.as_slice()).collect();
    assert_eq!(keys, vec![&b"Type"[..], b"Filter", b"SubFilter", b"M", b"Name", b"Reason", b"Contents", b"ByteRange"]);

    // lopdf direkt (zweiter Leser)
    let doc = pida_pdf_cos::Document::load_mem(out).unwrap_or_else(|e| panic!("{name}: lopdf: {e}"));
    assert_eq!(doc.get_pages().len(), src.page_count(), "{name}: lopdf Seitenzahl");
    let lsd = doc.get_object(sig_id).unwrap_or_else(|e| panic!("{name}: lopdf Signaturdictionary: {e}"));
    assert!(matches!(lsd, Object::Dictionary(d) if d.has(b"ByteRange")));
}

fn qpdf_ok(name: &str, original: &std::path::Path, out_path: &std::path::Path) -> Option<String> {
    let qpdf = common::which("qpdf")?;
    let before = common::qpdf_check(&qpdf, original);
    let after = common::qpdf_check(&qpdf, out_path);
    assert!(
        after.code == 0 || after.code == 3,
        "{name}: qpdf --check Fehler (Code {}): {:?}",
        after.code,
        after.messages
    );
    let new: Vec<&String> = after.messages.iter().filter(|m| !before.messages.contains(m)).collect();
    assert!(new.is_empty(), "{name}: neue qpdf-Befunde: {new:?}");
    Some(format!("qpdf {}", after.code))
}

#[test]
fn every_corpus_pdf_gets_a_valid_increment() {
    let tmp = tempfile::tempdir().unwrap();
    let files = common::corpus();
    assert!(files.len() >= 50, "Korpus unvollständig: {}", files.len());
    let mut done = 0;
    let mut qpdf_runs = 0;
    for p in &files {
        let name = common::rel(p);
        let src = match SourcePdf::open_file(p, &Limits::default()) {
            Ok(s) => s,
            Err(CosError::PasswordRequired) if common::expected_open_error(&name) => continue,
            Err(e) => panic!("{name}: öffnen: {e}"),
        };
        let inc = common::add_dummy_signature(&src, IncrementOptions::default(), RESERVE_MODEL_B)
            .unwrap_or_else(|e| panic!("{name}: Increment: {e}"));
        let out = check_structure(&name, &src, &inc);
        check_reparse(&name, &src, &inc, &out);
        let out_path = tmp.path().join(format!("{done}.pdf"));
        std::fs::write(&out_path, &out).unwrap();
        if qpdf_ok(&name, p, &out_path).is_some() {
            qpdf_runs += 1;
        }
        done += 1;
    }
    assert!(done >= 50);
    if qpdf_runs == 0 {
        eprintln!("HINWEIS: qpdf nicht gefunden – Prüfung übersprungen");
    }
}

/// Mehrfachsignatur: drei aufeinanderfolgende Increments je Querverweis-Art.
#[test]
fn multiple_signatures_chain() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "pdf/generated/simple-xref-table.pdf",
        "pdf/generated/simple-xref-stream.pdf",
        "pdf/generated/simple-xref-hybrid.pdf",
        "pdf/generated/simple-linearized.pdf",
        "pdf/generated/simple-aes256-owner.pdf",
        "pdf/generated/simple-rc4-128-owner.pdf",
        "pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf",
    ] {
        let path = common::testdata().join(rel);
        let mut bytes = std::fs::read(&path).unwrap();
        let first = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
        let kind = first.xref_kind();
        let base_fields = first.signature_fields().unwrap().len();
        let mut boundaries = Vec::new();
        for round in 1..=3 {
            let src = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
            let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 4096).unwrap();
            let out = check_structure(rel, &src, &inc);
            check_reparse(rel, &src, &inc, &out);
            boundaries.push(out.len());
            bytes = out;
            let re = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
            assert_eq!(re.xref_kind(), kind, "{rel}: Format bleibt über Runden gleich");
            let fields = re.signature_fields().unwrap();
            assert_eq!(fields.len(), base_fields + round);
            let names: Vec<_> = fields.iter().filter_map(|f| f.partial_name.clone()).collect();
            assert!(names.iter().any(|n| n.starts_with("PDF-AS Signatur")), "{rel}: {names:?}");
        }
        // Jede Runde endet an einer Revisionsgrenze.
        let fin = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
        let ends: Vec<u64> = fin.revisions().iter().map(|r| r.end_offset).collect();
        for b in boundaries {
            assert!(ends.contains(&(b as u64)), "{rel}: Grenze {b} keine Revision ({ends:?})");
        }
        let out_path = tmp.path().join("multi.pdf");
        std::fs::write(&out_path, &bytes).unwrap();
        qpdf_ok(rel, &path, &out_path);
    }
}

/// Hybrid-Quelle → Tabelle + xref-Stream mit `/XRefStm`; der Stream enthält sich selbst.
#[test]
fn hybrid_increment_has_xrefstm() {
    let path = common::testdata().join("pdf/generated/simple-xref-hybrid.pdf");
    let src = SourcePdf::open_file(&path, &Limits::default()).unwrap();
    assert_eq!(src.xref_kind(), XrefKind::Hybrid);
    let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 4096).unwrap();
    let out = inc.to_vec(src.bytes());
    let re = SourcePdf::open(out.clone(), &Limits::default()).unwrap();
    assert_eq!(re.xref_kind(), XrefKind::Hybrid);
    let t = re.trailer();
    let stm = t.get(b"XRefStm").unwrap().as_i64().unwrap() as usize;
    let text = String::from_utf8_lossy(&out[stm..stm + 200]);
    assert!(text.contains("/Type /XRef"), "{text}");
    assert!(!text.split("stream").next().unwrap().contains("/Prev"), "Hybrid-Stream ohne /Prev");
    let xs = inc.written.iter().find(|(_, k)| *k == WrittenKind::XrefStream).unwrap().0;
    assert!(text.starts_with(&format!("{} 0 obj", xs.0)));
    // Font-Objekt 5 aus dem Object Stream der Originalrevision ist weiter erreichbar.
    assert!(re.get_object((5, 0)).unwrap().as_dict().unwrap().has_type(b"Font"));
}

/// xref-Stream-Quelle → xref-Stream mit minimalem `/W` und Eigeneintrag.
#[test]
fn stream_increment_layout() {
    let path = common::testdata().join("pdf/generated/simple-xref-stream.pdf");
    let src = SourcePdf::open_file(&path, &Limits::default()).unwrap();
    let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 4096).unwrap();
    let out = inc.to_vec(src.bytes());
    let tail = String::from_utf8_lossy(&inc.bytes);
    let xs = inc.written.iter().find(|(_, k)| *k == WrittenKind::XrefStream).unwrap().0;
    let at = tail.find(&format!("{} 0 obj", xs.0)).unwrap();
    let dict = &tail[at..at + 300];
    assert!(dict.contains("/W [1 2 0]") || dict.contains("/W [1 3 0]"), "{dict}");
    assert!(dict.contains("/Filter /FlateDecode"));
    assert!(dict.contains(&format!("/Prev {}", src.last_revision().startxref)));
    assert!(!tail.contains("\nxref\n"), "keine Tabelle bei xref-Stream-Quelle");
    let re = SourcePdf::open(out, &Limits::default()).unwrap();
    assert_eq!(re.last_revision().xref_kind, XrefKind::Stream);
    assert_eq!(re.xref_health(), pida_pdf_cos::XrefHealth::Intact);
}

/// Klassische Tabelle wie PDFBox: `0 n` mit Freieintrag, 20-Byte-Einträge, Trailer mit /Prev.
#[test]
fn table_increment_layout() {
    let path = common::testdata().join("pdf/generated/simple-xref-table.pdf");
    let src = SourcePdf::open_file(&path, &Limits::default()).unwrap();
    let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 4096).unwrap();
    let tail = String::from_utf8_lossy(&inc.bytes).into_owned();
    let x = tail.rfind("\nxref\n").unwrap();
    let section = &tail[x + 1..];
    assert!(
        section.starts_with("xref\n0 1\n0000000000 65535 f\r\n")
            || section.starts_with("xref\n0 2\n0000000000 65535 f\r\n"),
        "{section}"
    );
    assert!(section.contains("trailer\n<<\n/Size "));
    assert!(section.contains(&format!("/Prev {}\n", src.last_revision().startxref)));
    assert!(section.ends_with("%%EOF\n"));
    let startxref: u64 = section.lines().rev().nth(1).unwrap().parse().unwrap();
    assert_eq!(startxref, inc.base_len + x as u64 + 1);
}
