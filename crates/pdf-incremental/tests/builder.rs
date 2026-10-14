//! Builder- und Increment-API von `pdfover-pdf-incremental` (Fehlerfälle, Platzhalter-Layout,
//! Längenprüfung, Verschlüsselung neuer Objekte, Optionen).

mod common;

use pdfover_pdf_cos::{CryptTarget, Dictionary, Limits, Object, SourcePdf, Stream, StringFormat, XrefHealth, XrefKind};
use pdfover_pdf_incremental::{
    BYTE_RANGE_PLACEHOLDER, ByteRange, IncError, Increment, IncrementBuilder, IncrementOptions, RESERVE_MIN_MODEL_A,
    SignatureDictSpec, WrittenKind, XrefFormat,
};

fn open(rel: &str) -> SourcePdf {
    SourcePdf::open_file(&common::testdata().join(rel), &Limits::default()).unwrap()
}

const TABLE: &str = "pdf/generated/simple-xref-table.pdf";

#[test]
fn builder_error_cases() {
    let src = open(TABLE);
    let catalog = src.catalog_id().unwrap();
    let mut b = IncrementBuilder::new(&src, IncrementOptions::default());
    assert_eq!(b.source().len(), src.len());
    // unbekanntes Objekt
    assert!(matches!(b.update_object((9999, 0), Object::Null), Err(IncError::UnknownObject((9999, 0)))));
    // falsche Generation
    assert!(matches!(b.update_object((catalog.0, 7), Object::Null), Err(IncError::UnknownObject(_))));
    // doppelt
    b.update_object(catalog, src.get_object(catalog).unwrap().clone()).unwrap();
    assert!(matches!(b.update_object(catalog, Object::Null), Err(IncError::DuplicateObject(_))));
    // insert_new ohne Reservierung
    assert!(matches!(b.insert_new((5000, 0), Object::Null), Err(IncError::UnknownObject(_))));
    let id = b.allocate_id();
    assert_eq!(id, (src.next_object_number(), 0));
    b.insert_new(id, Object::Integer(1)).unwrap();
    assert!(matches!(b.insert_new(id, Object::Integer(2)), Err(IncError::DuplicateObject(_))));
    // nur ein Signaturdictionary
    let sig = b.add_signature_dictionary(SignatureDictSpec::default()).unwrap();
    assert!(matches!(
        b.add_signature_dictionary(SignatureDictSpec::default()),
        Err(IncError::MultipleSignatureDictionaries)
    ));
    let added = b.add_object(Object::Boolean(true));
    assert_eq!(
        b.pending(),
        vec![
            (catalog, WrittenKind::Changed),
            (id, WrittenKind::New),
            (sig, WrittenKind::SignatureDictionary),
            (added, WrittenKind::New)
        ]
    );
    let dbg = format!("{b:?}");
    assert!(dbg.contains("objects: 4") && !dbg.contains("Catalog"), "{dbg}");
    // reservierte, aber nicht gesetzte ID → Fehler beim Bauen
    let mut b = IncrementBuilder::new(&src, IncrementOptions::default());
    let _ = b.allocate_id();
    assert!(matches!(b.build(), Err(IncError::Serialize(_))));
}

#[test]
fn increment_without_signature_dictionary() {
    let src = open(TABLE);
    let mut b = IncrementBuilder::new(&src, IncrementOptions::default());
    let info = src.info_id().unwrap();
    let mut d = src.get_object(info).unwrap().as_dict().unwrap().clone();
    d.set("Producer", Object::String(b"PDF-Over NG (NUR TEST)".to_vec(), StringFormat::Literal));
    b.update_object(info, Object::Dictionary(d)).unwrap();
    let mut inc = b.build().unwrap();
    assert!(inc.signature_dict_id.is_none() && inc.contents_range.is_empty());
    assert!(matches!(inc.patch_byte_range(), Err(IncError::NoSignatureDictionary)));
    assert!(matches!(inc.embed_contents(&[1, 2, 3]), Err(IncError::NoSignatureDictionary)));
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    let p = re.get_object(info).unwrap().as_dict().unwrap().get(b"Producer").unwrap();
    assert_eq!(p.as_str().unwrap(), b"PDF-Over NG (NUR TEST)");
    assert_eq!(inc.byte_range_parts(src.bytes())[1].len(), inc.bytes.len());
}

#[test]
fn signature_dictionary_layout_like_pdfas() {
    let src = open(TABLE);
    let mut b =
        IncrementBuilder::new(&src, IncrementOptions { new_id: Some(vec![0xAB; 32]), ..IncrementOptions::default() });
    let spec = SignatureDictSpec {
        signing_time: Some("D:20240315193333+01'00'".into()),
        location: Some("Image5".into()),
        name: Some("Dipl.-Ing. Max Müller".into()),
        reason: Some("Signaturpruefung unter https://www.signaturpruefung.gv.at".into()),
        contents_reserve: 16,
        ..SignatureDictSpec::default()
    };
    let id = b.add_signature_dictionary(spec).unwrap();
    let inc = b.build().unwrap();
    let text = String::from_utf8_lossy(&inc.bytes).into_owned();
    let start = text.find(&format!("{} 0 obj\n", id.0)).unwrap();
    let end = start + text[start..].find("endobj\n").unwrap() + 7;
    let br = inc.byte_range.to_pdf_string();
    let expected = format!(
        "{} 0 obj\n<<\n/Type /Sig\n/Filter /Adobe.PPKLite\n/SubFilter /ETSI.CAdES.detached\n/M (D:20240315193333+01'00')\n/Location (Image5)\n/Name <4469706C2E2D496E672E204D6178204DFC6C6C6572>\n/Reason (Signaturpruefung unter https://www.signaturpruefung.gv.at)\n/Contents <{}>\n/ByteRange {}{}\n>>\nendobj\n",
        id.0,
        "0".repeat(32),
        br,
        " ".repeat(36 - br.len())
    );
    assert_eq!(&text[start..end], expected);
    assert_eq!(inc.reserve(), 16);
    assert_eq!(inc.byte_range_field.end - inc.byte_range_field.start, BYTE_RANGE_PLACEHOLDER.len() as u64);
    let ex = inc.excluded_range();
    assert_eq!(ex.from, inc.contents_range.start);
    assert_eq!(ex.to, inc.contents_range.end - 1);
    let out = inc.to_vec(src.bytes());
    assert_eq!(out[ex.from as usize], b'<');
    assert_eq!(out[ex.to as usize], b'>');
    // /ID[1] wie angegeben, /ID[0] unverändert
    let re = SourcePdf::open(out, &Limits::default()).unwrap();
    let [id0, id1] = re.trailer_id().unwrap();
    assert_eq!(id0, src.trailer_id().unwrap()[0]);
    assert_eq!(id1, vec![0xAB; 32]);
    // Default-Reserve wie PDF-AS (4096)
    assert_eq!(SignatureDictSpec::default().contents_reserve, RESERVE_MIN_MODEL_A);
}

#[test]
fn contents_length_check_and_hex() {
    let src = open(TABLE);
    let mut inc = common::add_dummy_signature(&src, IncrementOptions::default(), 64).unwrap();
    assert_eq!(inc.reserve(), 64);
    let r = inc.local(inc.contents_range.clone());
    // genau N passt
    inc.embed_contents(&[0xAB; 64]).unwrap();
    assert_eq!(&inc.bytes[r.start + 1..r.end - 1], "AB".repeat(64).as_bytes());
    // N+1 und 2N werden abgewiesen, ohne zu schreiben
    for n in [65, 128] {
        match inc.embed_contents(&vec![0x11; n]) {
            Err(IncError::ContentsTooLarge { needed, available }) => assert_eq!((needed, available), (n, 64)),
            other => panic!("{other:?}"),
        }
        assert_eq!(&inc.bytes[r.start + 1..r.end - 1], "AB".repeat(64).as_bytes(), "nichts geschrieben");
    }
    // kürzeres CMS: Rest wieder '0', Großbuchstaben
    inc.embed_contents(&[0x30, 0x0a, 0xff]).unwrap();
    let hex = &inc.bytes[r.start + 1..r.end - 1];
    assert!(hex.starts_with(b"300AFF"));
    assert!(hex[6..].iter().all(|&b| b == b'0'));
    // Die ByteRange-Daten bleiben unverändert (Lücke ausgeschnitten).
    let before = inc.byte_range_data(src.bytes());
    inc.embed_contents(&[0x55; 10]).unwrap();
    assert_eq!(inc.byte_range_data(src.bytes()), before);
    assert_eq!(inc.byte_range.covered_len(), before.len() as u64);
}

#[test]
fn byte_range_overflow_is_reported() {
    let mut inc = Increment {
        base_len: 10,
        bytes: b"\r\n<0000>\n[0 1]\n".to_vec(),
        contents_range: 12..18,
        byte_range_field: 19..24,
        byte_range: ByteRange([0, 0, 0, 0]),
        signature_dict_id: Some((1, 0)),
        written: Vec::new(),
    };
    assert!(matches!(inc.patch_byte_range(), Err(IncError::ByteRangeOverflow)));
    let dbg = format!("{inc:?}");
    assert!(dbg.contains("len: 15") && !dbg.contains("0000"), "{dbg}");
}

#[test]
fn types_are_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<SourcePdf>();
    check::<Increment>();
    check::<IncrementBuilder<'static>>();
    check::<pdfover_pdf_cos::Encryptor>();
}

/// Falscher `startxref` (K2 „Repaired“): `/Prev` zeigt auf den tatsächlichen Abschnitt, die
/// neue Kette ist danach wieder einwandfrei lesbar.
#[test]
fn repaired_startxref_gets_correct_prev() {
    let good = std::fs::read(common::testdata().join(TABLE)).unwrap();
    let pos = good.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10;
    let end = pos + good[pos..].iter().position(|&b| b == b'\n').unwrap();
    let sx: u64 = std::str::from_utf8(&good[pos..end]).unwrap().parse().unwrap();
    let mut bad = good[..pos].to_vec();
    bad.extend_from_slice((sx + 2).to_string().as_bytes());
    bad.extend_from_slice(&good[end..]);
    let src = SourcePdf::open(bad.clone(), &Limits::default()).unwrap();
    assert_eq!(src.xref_health(), XrefHealth::Repaired);
    assert_eq!(src.prev_offset(), sx);
    let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 1024).unwrap();
    let re = SourcePdf::open(inc.to_vec(&bad), &Limits::default()).unwrap();
    assert_eq!(re.last_revision().prev, Some(sx));
    assert_eq!(re.xref_health(), XrefHealth::Intact, "Kette über das neue Increment ohne Korrektur lesbar");
    assert_eq!(re.signature_fields().unwrap().len(), 1);
}

#[test]
fn reconstructed_source_cannot_get_prev_increment() {
    let mut bytes = std::fs::read(common::testdata().join(TABLE)).unwrap();
    let cut = bytes.windows(9).rposition(|w| w == b"startxref").unwrap();
    bytes.truncate(cut);
    let src = SourcePdf::open(bytes, &Limits::default()).unwrap();
    assert_eq!(src.xref_health(), XrefHealth::Reconstructed);
    let err = common::add_dummy_signature(&src, IncrementOptions::default(), 4096).unwrap_err();
    assert!(matches!(err, IncError::Cos(_)), "{err:?}");
}

#[test]
fn options_crlf_compression_and_forced_formats() {
    let src = open("pdf/generated/simple-xref-stream.pdf");
    // ohne CRLF: Increment beginnt direkt (Quelle endet mit EOL)
    let mut b = IncrementBuilder::new(
        &src,
        IncrementOptions { leading_crlf: false, compress_new_streams: true, ..IncrementOptions::default() },
    );
    let content = b"q 1 0 0 1 0 0 cm Q ".repeat(50);
    let id = b.add_object(Object::Stream(Stream::new(Dictionary::new(), content.clone())));
    let inc = b.build().unwrap();
    assert!(inc.bytes.starts_with(format!("{} 0 obj", id.0).as_bytes()));
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    let s = re.get_object(id).unwrap().as_stream().unwrap();
    assert_eq!(s.dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
    assert!(s.content.len() < content.len());
    assert_eq!(re.decoded_stream(id).unwrap(), content);
    assert_eq!(re.xref_kind(), XrefKind::Stream);
    assert!(inc.written.iter().any(|(_, k)| *k == WrittenKind::XrefStream));

    // erzwungene Tabelle auf xref-Stream-Quelle
    let inc = common::add_dummy_signature(
        &src,
        IncrementOptions { xref: XrefFormat::Table, ..IncrementOptions::default() },
        1024,
    )
    .unwrap();
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    assert_eq!(re.xref_kind(), XrefKind::Table);
    assert_eq!(re.revisions()[0].xref_kind, XrefKind::Stream);
    assert!(re.get_object(re.catalog_id().unwrap()).unwrap().as_dict().unwrap().has(b"AcroForm"));
    assert_eq!(re.xref_health(), XrefHealth::Intact);
    // erzwungener Hybrid
    let inc = common::add_dummy_signature(
        &src,
        IncrementOptions { xref: XrefFormat::Hybrid, ..IncrementOptions::default() },
        1024,
    )
    .unwrap();
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    assert_eq!(re.xref_kind(), XrefKind::Hybrid);

    // Quelle ohne abschließendes EOL und ohne CRLF-Option: Zeilenumbruch wird eingefügt
    let mut raw = std::fs::read(common::testdata().join(TABLE)).unwrap();
    while raw.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        raw.pop();
    }
    let src = SourcePdf::open(raw.clone(), &Limits::default()).unwrap();
    let inc = IncrementBuilder::new(&src, IncrementOptions { leading_crlf: false, ..IncrementOptions::default() })
        .build()
        .unwrap();
    assert_eq!(inc.bytes[0], b'\n');
    let re = SourcePdf::open(inc.to_vec(&raw), &Limits::default()).unwrap();
    assert_eq!(re.revisions().len(), 2);
}

#[test]
fn objects_from_object_streams_are_rewritten_as_regular_objects() {
    let src = open("pdf/generated/simple-xref-stream.pdf");
    let catalog = src.catalog_id().unwrap();
    let mut d = src.catalog().unwrap().clone();
    d.set("PageMode", Object::Name(b"UseNone".to_vec()));
    let mut b = IncrementBuilder::new(&src, IncrementOptions::default());
    b.update_object(catalog, Object::Dictionary(d)).unwrap();
    let inc = b.build().unwrap();
    let text = String::from_utf8_lossy(&inc.bytes);
    assert!(text.contains(&format!("{} {} obj\n<<\n", catalog.0, catalog.1)));
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    assert_eq!(re.catalog().unwrap().get(b"PageMode").unwrap().as_name().unwrap(), b"UseNone");
    assert_eq!(re.page_count(), 1);
}

#[test]
fn unchanged_reals_keep_original_text() {
    let src = open("pdf/pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf");
    let page = src.pages()[0].object_id;
    let raw_page = String::from_utf8_lossy(src.bytes()).into_owned();
    assert!(raw_page.contains("595.303937007874"), "Testannahme: Originaltext im Seitenobjekt");
    let mut d = src.get_object(page).unwrap().as_dict().unwrap().clone();
    d.set("UserUnit", Object::Real(1.5));
    let mut b = IncrementBuilder::new(&src, IncrementOptions::default());
    b.update_object(page, Object::Dictionary(d)).unwrap();
    let inc = b.build().unwrap();
    let text = String::from_utf8_lossy(&inc.bytes);
    assert!(text.contains("595.303937007874"), "Originalschreibweise beibehalten");
    assert!(!text.contains("595.30395"), "keine f32-Rundung");
    assert!(text.contains("/UserUnit 1.5"));
}

#[test]
fn encryption_of_new_objects_and_exceptions() {
    for rel in [
        "pdf/generated/simple-aes128-owner.pdf",
        "pdf/generated/simple-aes256-owner.pdf",
        "pdf/generated/simple-rc4-128-owner.pdf",
        "pdf/generated/simple-aes256-owner-xref-stream.pdf",
    ] {
        let src = open(rel);
        let enc = src.encryptor().unwrap();
        let mut b = IncrementBuilder::new(&src, IncrementOptions::default());
        let secret = b"Klartext NUR TEST 0123456789";
        let mut d = Dictionary::new();
        d.set("S", Object::String(secret.to_vec(), StringFormat::Literal));
        d.set("A", Object::Array(vec![Object::String(b"tief".to_vec(), StringFormat::Literal)]));
        let sid = b.add_object(Object::Dictionary(d));
        let stream_id = b.add_object(Object::Stream(Stream::new(Dictionary::new(), secret.to_vec())));
        let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 512).unwrap();
        let built = b.build().unwrap();
        for (label, bytes) in [("Objekte", &built.bytes), ("Signatur", &inc.bytes)] {
            assert!(!bytes.windows(secret.len()).any(|w| w == secret), "{rel}/{label}: Klartext im Increment");
            assert!(
                !bytes.windows(common::TEST_NAME.len()).any(|w| w == common::TEST_NAME.as_bytes()),
                "{rel}/{label}: /Name im Klartext"
            );
        }
        let re = SourcePdf::open(built.to_vec(src.bytes()), &Limits::default()).unwrap();
        let rd = re.get_object(sid).unwrap().as_dict().unwrap();
        assert_eq!(rd.get(b"S").unwrap().as_str().unwrap(), secret, "{rel}");
        assert_eq!(rd.get(b"A").unwrap().as_array().unwrap()[0].as_str().unwrap(), b"tief");
        assert_eq!(re.get_object(stream_id).unwrap().as_stream().unwrap().content, secret, "{rel}");
        // Rohbytes der Signatur: /Contents bleibt Klartext-Hex aus '0'
        let out = inc.to_vec(src.bytes());
        let c = inc.contents_range.clone();
        assert!(out[c.start as usize + 1..c.end as usize - 1].iter().all(|&b| b == b'0'));
        // Encryptor ist mit dem Lesepfad verträglich
        let ct = enc.encrypt((1, 0), b"x", CryptTarget::String).unwrap();
        assert_eq!(enc.decrypt((1, 0), &ct, CryptTarget::String).unwrap(), b"x");
    }
}

#[test]
fn cleartext_metadata_stays_cleartext() {
    let src = open("pdf-incremental/pdfa1b-aes128-cleartext-metadata.pdf");
    let e = src.encryption().unwrap();
    assert!(!e.encrypt_metadata);
    let meta_id = src.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
    let xmp = src.decoded_stream(meta_id).unwrap();
    assert!(xmp.windows(9).any(|w| w == b"x:xmpmeta"));
    let mut s = src.get_object(meta_id).unwrap().as_stream().unwrap().clone();
    let mut new_xmp = xmp.clone();
    new_xmp.extend_from_slice(b"<!-- NUR TEST -->");
    s.set_plain_content(new_xmp.clone());
    // auch mit Kompressionsoption bleibt XMP ungefiltert (PDF/A-1 §6.7.2)
    let mut b =
        IncrementBuilder::new(&src, IncrementOptions { compress_new_streams: true, ..IncrementOptions::default() });
    b.update_object(meta_id, Object::Stream(s)).unwrap();
    let inc = b.build().unwrap();
    assert!(inc.bytes.windows(17).any(|w| w == b"<!-- NUR TEST -->"), "Metadata unverschlüsselt und unkomprimiert");
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    assert_eq!(re.decoded_stream(meta_id).unwrap(), new_xmp);
    assert_eq!(re.pdfa_identification().unwrap().unwrap().part, 1);
}

#[test]
fn leading_junk_offsets_follow_source_convention() {
    let raw = std::fs::read(common::testdata().join(TABLE)).unwrap();
    let mut bytes = b"Vorspann vor dem Header\n".to_vec();
    bytes.extend_from_slice(&raw);
    let src = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
    assert_eq!(src.xref_offset_base(), 24);
    let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 1024).unwrap();
    let out = inc.to_vec(&bytes);
    let re = SourcePdf::open(out.clone(), &Limits::default()).unwrap();
    assert_eq!(re.xref_health(), XrefHealth::Intact);
    assert_eq!(re.revisions().len(), 2);
    let sig = re.get_object(inc.signature_dict_id.unwrap()).unwrap().as_dict().unwrap();
    assert!(sig.has_type(b"Sig"));
    // ByteRange bleibt absolut (Dateioffsets, nicht xref-Basis)
    assert!(inc.byte_range.is_consistent_with(out.len() as u64));
    assert_eq!(out[inc.byte_range.0[1] as usize], b'<');
}

#[test]
fn source_without_id_gets_two_equal_new_ids() {
    let raw = std::fs::read(common::testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf")).unwrap();
    let src = SourcePdf::open(raw, &Limits::default()).unwrap();
    let inc = IncrementBuilder::new(&src, IncrementOptions::default()).build().unwrap();
    let re = SourcePdf::open(inc.to_vec(src.bytes()), &Limits::default()).unwrap();
    let [a, b] = re.trailer_id().unwrap();
    if let Some([oa, _]) = src.trailer_id() {
        assert_eq!(a, oa);
    } else {
        assert_eq!(a, b);
    }
    assert_eq!(b.len(), 32, "SHA-256 wie PDFBox 2.0.34");
}
