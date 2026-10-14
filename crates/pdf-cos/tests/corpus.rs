//! Korpus-Tests für `pdfover-pdf-cos` (Paket A) gegen `testdata/pdf/`.
//!
//! Orakel: bekannte Struktur der erzeugten Dateien (qpdf/tools/make-testdata.sh), Klartext-Original
//! der verschlüsselten Varianten, QR-Erkennung mit rxing (Platzhalterbilder) und – falls
//! `PDFOVER_PDFIUM_DIR` gesetzt ist – die Bildobjekt-Matrizen von PDFium (ADR 0001 §8.2).

mod common;

use pdfover_pdf_cos::{
    CosError, CryptAlgorithm, CryptTarget, Limits, Object, PixelFormat, Rotation, SecurityHandler, SourcePdf, TailKind,
    XrefHealth, XrefKind,
};

fn open(rel: &str) -> SourcePdf {
    SourcePdf::open_file(&common::pdf(rel), &Limits::default()).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn all_corpus_files_open_with_consistent_structure() {
    let files = common::corpus();
    assert!(files.len() >= 50, "Korpus: {}", files.len());
    let mut opened = 0;
    for p in &files {
        let name = common::rel(p);
        let src = match SourcePdf::open_file(p, &Limits::default()) {
            Ok(s) => s,
            Err(CosError::PasswordRequired) if name.ends_with("simple-aes256-userpw.pdf") => continue,
            Err(e) => panic!("{name}: {e}"),
        };
        opened += 1;
        let revs = src.revisions();
        assert!(!revs.is_empty(), "{name}");
        for (i, r) in revs.iter().enumerate() {
            assert_eq!(r.index, i, "{name}");
            assert!(r.startxref > 0 && r.end_offset > r.startxref, "{name}: {r:?}");
            if i > 0 {
                assert!(r.end_offset > revs[i - 1].end_offset, "{name}: Revisionen sortiert");
            }
        }
        let tail = src.tail();
        assert_eq!(tail.kind, TailKind::Clean, "{name}");
        assert_eq!(tail.eof_offset, src.last_revision().end_offset, "{name}");
        assert_eq!(tail.eof_offset + tail.len, src.len(), "{name}");
        assert_eq!(src.xref_health(), XrefHealth::Intact, "{name}");
        assert_eq!(src.xref_offset_base(), 0, "{name}");
        assert!(src.page_count() >= 1);
        for (i, p) in src.pages().iter().enumerate() {
            assert_eq!(p.index, i);
            assert!(src.get_object(p.object_id).unwrap().as_dict().unwrap().has_type(b"Page"), "{name}: Seite {i}");
            assert!(p.media_box.width() > 0.0 && p.media_box.height() > 0.0, "{name}");
            assert!(p.crop_box.width() > 0.0 && p.crop_box.height() > 0.0, "{name}");
        }
        let max_obj = src.document().objects.keys().map(|id| id.0).max().unwrap();
        assert!(src.next_object_number() > max_obj, "{name}: nächste Nummer");
        assert!(src.next_object_number() >= src.last_revision().size, "{name}: /Size");
        assert!(src.catalog().is_ok(), "{name}: Katalog");
        assert!(src.pdf_version().starts_with("1."), "{name}: {}", src.pdf_version());
        src.signature_fields().unwrap_or_else(|e| panic!("{name}: Felder: {e}"));
        for i in 0..src.page_count() {
            src.image_invocations(i, false).unwrap_or_else(|e| panic!("{name}: Bilder: {e}"));
            src.image_invocations(i, true).unwrap_or_else(|e| panic!("{name}: Bilder (Forms): {e}"));
        }
        // Debug ohne Dokumentinhalt (Regel 9)
        let dbg = format!("{src:?}");
        assert!(!dbg.contains("obj") && !dbg.contains("stream") && dbg.len() < 600, "{name}: {dbg}");
    }
    assert!(opened >= 59);
}

#[test]
fn xref_kinds_and_revisions_of_generated_files() {
    let cases = [
        ("generated/simple-xref-table.pdf", XrefKind::Table, 1),
        ("generated/simple-xref-stream.pdf", XrefKind::Stream, 1),
        ("generated/simple-xref-hybrid.pdf", XrefKind::Hybrid, 1),
        ("generated/simple-linearized.pdf", XrefKind::Table, 1),
        ("generated/simple-linearized-xref-stream.pdf", XrefKind::Stream, 1),
        ("generated/manySignFields-xref-stream.pdf", XrefKind::Stream, 1),
        ("generated/simple-aes256-owner-xref-stream.pdf", XrefKind::Stream, 1),
        ("pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf", XrefKind::Hybrid, 2),
        ("pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf", XrefKind::Table, 5),
        ("pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf", XrefKind::Stream, 2),
        ("pdfas/pdf-as-web/data/dummy-pdf-signed.pdf", XrefKind::Table, 3),
    ];
    for (rel, kind, revs) in cases {
        let s = open(rel);
        assert_eq!(s.xref_kind(), kind, "{rel}");
        assert_eq!(s.revisions().len(), revs, "{rel}");
    }
    // Linearisiert: Dummy-Marke `startxref 0` zählt nicht; startxref zeigt auf die Erstseiten-xref,
    // /Prev auf die Haupt-xref am Dateiende.
    let lin = open("generated/simple-linearized.pdf");
    assert_eq!(lin.last_revision().startxref, 216);
    assert_eq!(lin.last_revision().prev, Some(6513));
    assert_eq!(lin.last_revision().end_offset, lin.len());
    // Hybrid: Objekt 5 steht nur im xref-Stream (Object Stream 6).
    let hy = open("generated/simple-xref-hybrid.pdf");
    assert!(hy.get_object((5, 0)).unwrap().as_dict().unwrap().has_type(b"Font"));
    assert!(hy.trailer().has(b"XRefStm"));
    // Trailer-Angaben
    let t = open("generated/simple-xref-table.pdf");
    assert_eq!(t.trailer_id().map(|[a, b]| (a.len(), b.len())), Some((16, 16)));
    assert!(t.info_id().is_some());
    assert_eq!(t.catalog_id().unwrap(), t.trailer().get(b"Root").unwrap().as_reference().unwrap());
    // xref-Stream-Trailer ohne Strukturschlüssel
    let st = open("generated/simple-xref-stream.pdf");
    for k in [&b"W"[..], b"Index", b"Filter", b"Length", b"Type"] {
        assert!(!st.trailer().has(k), "{}", String::from_utf8_lossy(k));
    }
    assert!(st.trailer().has(b"Root"));
}

#[test]
fn encryption_detection_and_rights() {
    let cases: [(&str, CryptAlgorithm, u8, u8, bool); 6] = [
        ("generated/simple-aes128-owner.pdf", CryptAlgorithm::Aes128, 4, 4, true),
        ("generated/simple-aes256-owner.pdf", CryptAlgorithm::Aes256, 5, 6, true),
        ("generated/simple-aes256-owner-xref-stream.pdf", CryptAlgorithm::Aes256, 5, 6, true),
        ("generated/simple-rc4-128-owner.pdf", CryptAlgorithm::Rc4_128, 2, 3, true),
        ("generated/simple-aes256-nosign.pdf", CryptAlgorithm::Aes256, 5, 6, false),
        ("pdfas/pdf-as-web/data/enc_own.pdf", CryptAlgorithm::Rc4_128, 2, 3, true),
    ];
    for (rel, alg, v, r, may_sign) in cases {
        let s = open(rel);
        let e = s.encryption().unwrap_or_else(|| panic!("{rel}: nicht verschlüsselt?"));
        assert_eq!(e.handler, SecurityHandler::Standard, "{rel}");
        assert_eq!((e.algorithm.clone(), e.v, e.r), (alg, v, r), "{rel}");
        assert!(e.encrypt_dict_id.is_some(), "{rel}");
        assert!(s.rights().encrypted);
        assert_eq!(s.rights().may_sign(), may_sign, "{rel}");
        assert!(s.encryptor().is_some());
        assert!(s.trailer().has(b"Encrypt"), "{rel}: /Encrypt im Trailer");
        // Encrypt-Dictionary ist nicht Teil des (entschlüsselten) Objektmodells.
        assert!(s.document().get_object(e.encrypt_dict_id.unwrap()).is_err(), "{rel}");
        let dbg = format!("{:?}", s.encryptor().unwrap());
        assert!(dbg.contains("algorithm") && !dbg.contains("key"), "{dbg}");
        // Der Dateischlüssel ist über das Objektmodell nicht erreichbar (Regel 9).
        assert!(s.document().encryption_state.is_none(), "{rel}");
    }
    // enc_own: nur „Formulare ausfüllen“ → PDF-AS erlaubt das Signieren über canFillInForm.
    let p = open("pdfas/pdf-as-web/data/enc_own.pdf").rights().permissions;
    assert!(p.can_fill_in_form() && !p.can_modify() && !p.can_modify_annotations());
    let p = open("generated/simple-aes256-nosign.pdf").rights().permissions;
    assert!(!p.can_fill_in_form() && !p.can_modify() && !p.can_modify_annotations());
    // Unverschlüsselt: alle Rechte
    let plain = open("generated/simple-xref-table.pdf");
    assert!(plain.encryption().is_none() && plain.encryptor().is_none());
    assert_eq!(plain.rights().permissions, pdfover_pdf_cos::Permissions::ALL);
    // Benutzerpasswort → K5
    let err = SourcePdf::open_file(&common::pdf("generated/simple-aes256-userpw.pdf"), &Limits::default()).unwrap_err();
    assert!(matches!(err, CosError::PasswordRequired), "{err:?}");
}

/// Leeres Owner-Passwort: PDFBox prüft zuerst das Owner-Passwort und gewährt dann Owner-Rechte
/// unabhängig von `/P`, PDF-AS lässt das Signieren zu (gegengeprüft mit PDFBox 2.0.34:
/// `isOwnerPermission() == true`, `checkPDFPermissions` ohne Fehler). `/P` verbietet in diesen
/// Dateien Ändern, Annotationen und Formulare (`qpdf --show-encryption`: P = -1340).
#[test]
fn empty_owner_password_grants_owner_rights_like_pdfbox() {
    let dir = common::testdata().join("pdf-cos");
    for (file, alg) in [
        ("empty-owner-rc4.pdf", CryptAlgorithm::Rc4_128),
        ("empty-owner-aes128.pdf", CryptAlgorithm::Aes128),
        ("empty-owner-aes256.pdf", CryptAlgorithm::Aes256),
    ] {
        let s = SourcePdf::open_file(&dir.join(file), &Limits::default()).unwrap_or_else(|e| panic!("{file}: {e}"));
        let e = s.encryption().unwrap();
        assert_eq!(e.algorithm, alg, "{file}");
        // Rohwert /P: keine Änderungs-, Annotations- oder Formularrechte
        assert!(!e.permissions.can_modify() && !e.permissions.can_modify_annotations(), "{file}");
        assert!(!e.permissions.can_fill_in_form(), "{file}");
        // wirksam: Owner-Rechte
        assert!(s.rights().encrypted, "{file}");
        assert_eq!(s.rights().permissions, pdfover_pdf_cos::Permissions::ALL, "{file}");
        assert!(s.rights().may_sign(), "{file}");
    }
    // R2 (RC4-40, V1): Erkennung, Entschlüsselung und Encryptor-Roundtrip
    let s = SourcePdf::open_file(&dir.join("simple-rc4-40-owner.pdf"), &Limits::default()).unwrap();
    let e = s.encryption().unwrap();
    assert_eq!((e.algorithm.clone(), e.v, e.r, e.length_bits), (CryptAlgorithm::Rc4_40, 1, 2, 40));
    let plain = open("generated/simple-xref-table.pdf");
    assert_eq!(
        s.page_content(s.pages()[0].object_id).unwrap(),
        plain.page_content(plain.pages()[0].object_id).unwrap()
    );
    let enc = s.encryptor().unwrap();
    let c = enc.encrypt((7, 0), b"NUR TEST", CryptTarget::String).unwrap();
    assert_ne!(c, b"NUR TEST");
    assert_eq!(enc.decrypt((7, 0), &c, CryptTarget::String).unwrap(), b"NUR TEST");
    // Owner-Passwort gesetzt (nicht leer): /P gilt weiter.
    for rel in ["generated/simple-aes256-nosign.pdf", "pdfas/pdf-as-web/data/enc_own.pdf"] {
        let s = open(rel);
        assert_eq!(s.rights().permissions, s.encryption().unwrap().permissions, "{rel}");
    }
}

#[test]
fn decryption_matches_plaintext_source() {
    let plain = open("pdfas/pdf-as-legacy/simple.pdf");
    let plain_content = plain.page_content(plain.pages()[0].object_id).unwrap();
    assert!(plain_content.len() > 50 && plain_content.windows(2).any(|w| w == b"BT"));
    let info = |s: &SourcePdf| -> Vec<(Vec<u8>, String)> {
        let d = s.get_object(s.info_id().unwrap()).unwrap().as_dict().unwrap();
        d.iter().filter_map(|(k, v)| pdfover_pdf_cos::decode_text_string(v).map(|t| (k.clone(), t))).collect()
    };
    let plain_info = info(&plain);
    assert!(!plain_info.is_empty());
    for rel in [
        "generated/simple-aes128-owner.pdf",
        "generated/simple-aes256-owner.pdf",
        "generated/simple-aes256-owner-xref-stream.pdf",
        "generated/simple-rc4-128-owner.pdf",
        "generated/simple-aes256-nosign.pdf",
        "generated/simple-xref-stream.pdf",
        "generated/simple-linearized.pdf",
    ] {
        let s = open(rel);
        let c = s.page_content(s.pages()[0].object_id).unwrap();
        assert_eq!(c, plain_content, "{rel}: entschlüsselter Seiteninhalt");
        let i = info(&s);
        for (k, v) in &plain_info {
            assert!(i.contains(&(k.clone(), v.clone())), "{rel}: /Info {}", String::from_utf8_lossy(k));
        }
    }
}

/// Rekonstruierter Querverweis (K2) bei verschlüsselten Dateien: Befund und Entschlüsselung bleiben.
#[test]
fn reconstructed_encrypted_files_keep_encryption_info() {
    let plain = open("pdfas/pdf-as-legacy/simple.pdf");
    let plain_content = plain.page_content(plain.pages()[0].object_id).unwrap();
    for (rel, alg) in [
        ("generated/simple-aes256-owner.pdf", CryptAlgorithm::Aes256),
        ("generated/simple-rc4-128-owner.pdf", CryptAlgorithm::Rc4_128),
        ("generated/simple-aes128-owner.pdf", CryptAlgorithm::Aes128),
    ] {
        let mut b = std::fs::read(common::pdf(rel)).unwrap();
        let cut = b.windows(9).rposition(|w| w == b"startxref").unwrap();
        b.truncate(cut);
        let s = SourcePdf::open(b, &Limits::default()).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(s.xref_health(), XrefHealth::Reconstructed, "{rel}");
        let e = s.encryption().unwrap_or_else(|| panic!("{rel}: Verschlüsselung erkannt"));
        assert_eq!(e.algorithm, alg, "{rel}");
        assert!(s.rights().encrypted && s.rights().may_sign());
        assert_eq!(s.page_content(s.pages()[0].object_id).unwrap(), plain_content, "{rel}");
    }
    let mut b = std::fs::read(common::pdf("generated/simple-aes256-userpw.pdf")).unwrap();
    let cut = b.windows(9).rposition(|w| w == b"startxref").unwrap();
    b.truncate(cut);
    assert!(matches!(SourcePdf::open(b, &Limits::default()), Err(CosError::PasswordRequired)));
}

#[test]
fn encryptor_roundtrip_per_algorithm() {
    for rel in
        ["generated/simple-aes128-owner.pdf", "generated/simple-aes256-owner.pdf", "generated/simple-rc4-128-owner.pdf"]
    {
        let s = open(rel);
        let enc = s.encryptor().unwrap();
        let id = (77, 0);
        for target in [CryptTarget::String, CryptTarget::Stream] {
            for msg in [&b""[..], b"x", b"exactly 16 bytes", b"PDF-Over NG Testdaten (NUR TEST) mit etwas mehr Inhalt"]
            {
                let c = enc.encrypt(id, msg, target).unwrap();
                match enc.algorithm() {
                    CryptAlgorithm::Rc4_128 | CryptAlgorithm::Rc4_40 => assert_eq!(c.len(), msg.len()),
                    _ => assert_eq!(c.len(), 16 + (msg.len() / 16 + 1) * 16, "{rel}: IV + PKCS#5"),
                }
                assert_eq!(enc.decrypt(id, &c, target).unwrap(), msg, "{rel}");
                if !msg.is_empty() {
                    // objektspezifischer Schlüssel (außer AES-256)
                    let other = enc.decrypt((78, 0), &c, target);
                    if !matches!(enc.algorithm(), CryptAlgorithm::Aes256) {
                        assert!(other.map(|o| o != msg).unwrap_or(true), "{rel}: Schlüssel je Objekt");
                    }
                }
            }
        }
    }
}

#[test]
fn signed_documents_signatures_end_at_revision_boundaries() {
    for (rel, n) in [
        ("pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf", 1),
        ("pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf", 2),
        ("pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf", 3),
        ("pdfas/pdf-as-web/data/dummy-pdf-signed.pdf", 1),
        ("pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf", 1),
        ("pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf", 1),
    ] {
        let s = open(rel);
        let signed: Vec<_> = s.signature_fields().unwrap().into_iter().filter(|f| f.signed).collect();
        assert_eq!(signed.len(), n, "{rel}");
        let ends: Vec<u64> = s.revisions().iter().map(|r| r.end_offset).collect();
        for f in signed {
            let sd = s.get_object(f.value_id.unwrap()).unwrap().as_dict().unwrap();
            assert!(sd.has_type(b"Sig"), "{rel}");
            let br: Vec<u64> =
                sd.get(b"ByteRange").unwrap().as_array().unwrap().iter().map(|o| o.as_i64().unwrap() as u64).collect();
            assert_eq!(br[0], 0);
            assert_eq!(s.bytes()[br[1] as usize], b'<', "{rel}");
            assert_eq!(s.bytes()[br[2] as usize - 1], b'>', "{rel}");
            assert!(
                ends.contains(&(br[2] + br[3])),
                "{rel}: {} endet nicht an einer Revision {ends:?}",
                f.qualified_name
            );
            let contents = sd.get(b"Contents").unwrap().as_str().unwrap();
            assert_eq!(contents[0], 0x30, "{rel}: DER-SEQUENCE");
            assert_eq!(f.widgets.len(), 1);
            assert_eq!(f.widgets[0].annot_flags & 4, 4, "{rel}: Print-Flag");
        }
    }
    let s = open("pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf");
    let names: Vec<String> = s.signature_fields().unwrap().into_iter().map(|f| f.qualified_name).collect();
    assert_eq!(names, ["PDF-AS Signatur1", "PDF-AS Signatur2", "PDF-AS Signatur3"]);
    // /Location der QR-Signaturen (belegte Platzhalter)
    let locs: Vec<_> = s
        .signature_fields()
        .unwrap()
        .iter()
        .filter_map(|f| {
            s.get_object(f.value_id?)
                .ok()?
                .as_dict()
                .ok()?
                .get(b"Location")
                .ok()
                .and_then(pdfover_pdf_cos::decode_text_string)
        })
        .collect();
    assert!(locs.iter().any(|l| l.starts_with("Image")), "{locs:?}");
}

#[test]
fn empty_signature_fields_like_pdfas_samples() {
    let s = open("pdfas/pdf-as-pdfbox-2/manySignFields.pdf");
    let f = s.signature_fields().unwrap();
    assert_eq!(f.len(), 8);
    for (i, field) in f.iter().enumerate() {
        assert_eq!(field.qualified_name, format!("Signature_{i}"));
        assert_eq!(field.partial_name.as_deref(), Some(field.qualified_name.as_str()));
        assert_eq!(field.depth, 0);
        assert!(!field.signed && field.value_id.is_none());
        assert_eq!(field.widgets.len(), 1);
        assert_eq!(field.widgets[0].id, field.field_id, "verschmolzenes Widget");
        assert_eq!(field.widgets[0].page_index, Some(if i == 7 { 1 } else { 0 }));
    }
    // P2-Sonderfall aus analyse/03 §4.5: Rect ≈ 3 × 224 pt
    let r = f[4].widgets[0].rect;
    assert!((r.width() - 3.159).abs() < 0.01 && (r.height() - 223.963).abs() < 0.01, "{r:?}");
    // Dasselbe Dokument mit Object Streams liefert dieselben Felder.
    let os = open("generated/manySignFields-xref-stream.pdf").signature_fields().unwrap();
    assert_eq!(
        os.iter().map(|x| &x.qualified_name).collect::<Vec<_>>(),
        f.iter().map(|x| &x.qualified_name).collect::<Vec<_>>()
    );
    let acro = s.acroform_info().unwrap().unwrap();
    assert_eq!((acro.id, acro.field_count, acro.sig_flags), (Some((62, 0)), 8, 1));
    assert!(acro.has_dr && acro.has_da && !acro.need_appearances && !acro.has_xfa);
}

fn qr_text(s: &SourcePdf, id: pdfover_pdf_cos::ObjectId) -> Option<String> {
    let img = s.decode_image(id).unwrap();
    let luma = img.to_luma();
    rxing::helpers::detect_in_luma(luma, img.width, img.height, Some(rxing::BarcodeFormat::QR_CODE))
        .ok()
        .map(|r| r.getText().to_string())
}

#[test]
fn qr_placeholder_images_decode_and_are_readable() {
    let cases: [(&str, &[&str]); 4] = [
        ("pdfas/pdf-as-pdfbox-2/1Sign_manyQR.pdf", &["FXX1", "FXX3", "FXX4", "FXX5", "FXX6"]),
        ("pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf", &["Image5", "Image8"]),
        ("pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf", &["Im48", "Im50", "Im52"]),
        ("pdfas/pdf-as-web/data/placeholder_sigfield_and_qr.pdf", &["FXX3"]),
    ];
    for (rel, names) in cases {
        let s = open(rel);
        let inv = s.image_invocations(0, false).unwrap();
        for name in names {
            let i = inv.iter().find(|i| i.resource_name == *name).unwrap_or_else(|| panic!("{rel}: {name} fehlt"));
            let text =
                qr_text(&s, i.xobject_id.unwrap()).unwrap_or_else(|| panic!("{rel}/{name}: kein QR-Code erkannt"));
            assert!(text.starts_with("PDF-AS-POS"), "{rel}/{name}: {text}");
        }
    }
    // Format: Flate-RGB mit SMask → RGBA, DCT-Grau → Luma, DCT-RGB → RGBA
    let s = open("pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf");
    let id = s.image_invocations(0, false).unwrap()[0].xobject_id.unwrap();
    let img = s.decode_image(id).unwrap();
    assert_eq!((img.width, img.height, img.format), (300, 60, PixelFormat::Luma8));
    let s = open("pdfas/pdf-as-pdfbox-2/1Sign_manyQR.pdf");
    let inv = s.image_invocations(0, false).unwrap();
    let photo = inv.iter().find(|i| i.resource_name == "Image14").unwrap();
    let img = s.decode_image(photo.xobject_id.unwrap()).unwrap();
    assert_eq!((img.width, img.height, img.format), (879, 1318, PixelFormat::Rgba8));
    assert_eq!(img.data.len(), 879 * 1318 * 4);
    assert!(qr_text(&s, photo.xobject_id.unwrap()).is_none());
}

#[test]
fn image_invocation_ctm_and_order() {
    let s = open("pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf");
    let inv = s.image_invocations(0, false).unwrap();
    assert_eq!(inv.len(), 2);
    assert_eq!(inv[0].resource_name, "Image5");
    assert_eq!(inv[0].ctm, [225.0, 0.0, 0.0, 45.0, 72.0, 675.0]);
    assert_eq!((inv[0].pixel_width, inv[0].pixel_height), (300, 60));
    assert_eq!(inv[1].order, 1);
    assert!(!inv[0].inside_form_xobject);
    // Mehrfach aufgerufene XObjects zählen je Aufruf (dummy-pdf-signed: Image16 dreimal).
    let s = open("pdfas/pdf-as-web/data/dummy-pdf-signed.pdf");
    let inv = s.image_invocations(0, false).unwrap();
    assert_eq!(inv.iter().filter(|i| i.resource_name == "Image16").count(), 3);
    assert_eq!(inv.iter().map(|i| i.order).collect::<Vec<_>>(), (0..inv.len()).collect::<Vec<_>>());
    // Rotierte Seite: Bildmatrix mit Rotation
    let s = open("pdfas/pdf-as-cli/rotate_090.pdf");
    let inv = s.image_invocations(0, false).unwrap();
    assert_eq!(inv.len(), 1);
    assert!(inv[0].ctm[0].abs() < 1e-3 && inv[0].ctm[3].abs() < 1e-3 && inv[0].ctm[1] > 0.0 && inv[0].ctm[2] < 0.0);
    // Ungültiger Seitenindex
    assert!(s.image_invocations(5, false).is_err());
}

#[test]
fn image_ctm_matches_pdfium() {
    let Some(dir) = std::env::var_os("PDFOVER_PDFIUM_DIR") else {
        eprintln!("HINWEIS: PDFOVER_PDFIUM_DIR nicht gesetzt – PDFium-Orakel übersprungen");
        return;
    };
    use pdfium_render::prelude::{PdfPageObjectsCommon, Pdfium};
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir)).expect("PDFium laden");
    let pdfium = Pdfium::new(bindings);
    let mut compared = 0;
    for p in common::corpus() {
        let name = common::rel(&p);
        let Ok(src) = SourcePdf::open_file(&p, &Limits::default()) else { continue };
        if src.encryption().is_some() {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let doc = pdfium.load_pdf_from_byte_slice(&bytes, None).expect("PDFium öffnet");
        for (i, page) in doc.pages().iter().enumerate() {
            let ours = src.image_invocations(i, false).unwrap();
            let theirs: Vec<[f32; 6]> = page
                .objects()
                .iter()
                .filter_map(|o| o.as_image_object().map(|img| img.matrix().unwrap()))
                .map(|m| [m.a(), m.b(), m.c(), m.d(), m.e(), m.f()])
                .collect();
            // PDFium zählt Inline-Bilder mit; jede pdf-cos-Matrix muss bei PDFium vorkommen (≤ 0,01).
            for inv in &ours {
                let hit = theirs.iter().any(|m| m.iter().zip(inv.ctm.iter()).all(|(a, b)| (a - b).abs() <= 0.01));
                assert!(hit, "{name} S.{i} {}: {:?} nicht unter {theirs:?}", inv.resource_name, inv.ctm);
                compared += 1;
            }
            assert!(theirs.len() >= ours.len(), "{name} S.{i}");
        }
    }
    assert!(compared >= 20, "nur {compared} Bilder verglichen");
}

#[test]
fn page_geometry_rotation_and_boxes() {
    for (rel, rot) in [
        ("pdfas/pdf-as-cli/rotate_090.pdf", Rotation::R90),
        ("pdfas/pdf-as-cli/rotate_180.pdf", Rotation::R180),
        ("pdfas/pdf-as-cli/rotate_270.pdf", Rotation::R270),
        ("pdfas/pdf-as-cli/simple_rotated_0.pdf", Rotation::R0),
        ("pdfas/pdf-as-cli/simple_rotated_90.pdf", Rotation::R90),
        ("pdfas/pdf-as-cli/simple_rotated_180.pdf", Rotation::R180),
        ("pdfas/pdf-as-cli/simple_rotated_270.pdf", Rotation::R270),
    ] {
        let s = open(rel);
        let p = &s.pages()[0];
        assert_eq!(p.rotation, rot, "{rel}");
        let (w, h) = p.visible_size();
        if rot.swaps_axes() {
            assert_eq!((w, h), (p.crop_box.height(), p.crop_box.width()), "{rel}");
            assert!(w > h, "{rel}: Querformat sichtbar");
        } else {
            assert!(h > w, "{rel}");
        }
    }
    let s = open("pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf");
    let p = &s.pages()[0];
    assert!((p.media_box.urx - 595.304).abs() < 0.01 && (p.media_box.ury - 841.89).abs() < 0.01);
    assert_eq!(p.crop_box, p.media_box);
    assert_eq!(p.user_unit, 1.0);
}

#[test]
fn pdfa_identification_and_catalog_features() {
    let s = open("pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf");
    let id = s.pdfa_identification().unwrap().unwrap();
    assert_eq!((id.part, id.conformance.as_deref()), (1, Some("B")));
    assert!(s.xmp_metadata().unwrap().unwrap().windows(9).any(|w| w == b"x:xmpmeta"));
    let s = open("pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf");
    let id = s.pdfa_identification().unwrap().unwrap();
    assert_eq!((id.part, id.conformance.as_deref()), (3, Some("B")));
    assert!(s.is_tagged());
    assert_eq!(open("generated/simple-xref-table.pdf").pdfa_identification().unwrap(), None);
    for rel in ["pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf", "generated/simple-xref-table.pdf"] {
        let s = open(rel);
        assert_eq!(s.doc_mdp().unwrap(), None, "{rel}");
        assert!(!s.has_usage_rights() && !s.needs_rendering(), "{rel}");
    }
    // AcroForm direkt im Katalog (PDFBox-Neuanlage) bei den PDF-AS-Proben
    let a = open("pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf").acroform_info().unwrap().unwrap();
    assert_eq!((a.id, a.field_count, a.sig_flags), (None, 3, 3));
    assert_eq!(open("generated/simple-xref-table.pdf").acroform_info().unwrap(), None);
}

#[test]
fn limits_are_enforced() {
    let path = common::pdf("pdfas/pdf-as-pdfbox-2/manySignFields.pdf");
    let l = Limits { max_file_size: 1000, ..Limits::default() };
    assert!(matches!(SourcePdf::open_file(&path, &l), Err(CosError::LimitExceeded { what: "Dateigröße", .. })));
    let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(SourcePdf::open(bytes.clone(), &l), Err(CosError::LimitExceeded { .. })));
    let l = Limits { max_objects: 10, ..Limits::default() };
    assert!(matches!(SourcePdf::open(bytes.clone(), &l), Err(CosError::LimitExceeded { what: "Objektanzahl", .. })));
    // Kleine Stream-Grenze: Öffnen gelingt (Object Streams klein), Seiteninhalt nicht.
    let l = Limits { max_decoded_stream: 50, ..Limits::default() };
    let s = SourcePdf::open_file(&common::pdf("pdfas/pdf-as-legacy/simple.pdf"), &l).unwrap();
    assert!(matches!(s.page_content(s.pages()[0].object_id), Err(CosError::LimitExceeded { .. })));
    assert!(matches!(s.image_invocations(0, false), Err(CosError::LimitExceeded { .. })));
    // Nicht-PDF
    assert!(matches!(SourcePdf::open(b"Hallo Welt".to_vec(), &Limits::default()), Err(CosError::NotAPdf)));
    assert!(matches!(SourcePdf::open(Vec::new(), &Limits::default()), Err(CosError::NotAPdf)));
}

#[test]
fn decoded_stream_and_resolve() {
    let s = open("generated/simple-xref-stream.pdf");
    let page = s.get_object(s.pages()[0].object_id).unwrap().as_dict().unwrap();
    let contents = page.get(b"Contents").unwrap();
    let id = contents.as_reference().unwrap();
    let data = s.decoded_stream(id).unwrap();
    assert!(data.windows(2).any(|w| w == b"BT"));
    assert!(matches!(s.resolve(contents).unwrap(), Object::Stream(_)));
    assert!(matches!(s.decoded_stream(s.catalog_id().unwrap()), Err(CosError::InvalidStructure(_))));
    assert!(matches!(s.get_object((9999, 0)), Err(CosError::MissingObject((9999, 0)))));
    assert!(matches!(s.resolve(&Object::Reference((9999, 0))), Err(CosError::MissingObject(_))));
}
