//! Abläufe über mehrere Schritte und Modell B:
//! - Mehrfachsignatur (3× nacheinander, neue Felder, Rasterscan unter den vorigen Block),
//! - QR-Platzhalter (3× nacheinander, `/Location`, benutzte Platzhalter werden übersprungen),
//! - vorhandene leere Signaturfelder (sichtbar, unsichtbar, mehrfach),
//! - Modell B mit Fake-BKU (Blackout-PDF + ExcludedByteRange → `openssl cms -sign -cades`) über
//!   Tabelle, xref-Stream, Hybrid, linearisiert, AES-256, vorsigniert, PDF/A,
//! - Fehlerfälle: CMS zu groß (Neuvorbereitung mit größerer Reserve), falscher Signer,
//!   falscher Digest, persistierter Zustand (Mobile), atomares Schreiben, Selbstprüfung,
//!   neue Seite auf signiertem Dokument, PDF/UA-Struktur.

mod common;

use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use common::*;
use pida_backend_api::testing::ScriptedBroker;
use pida_backend_api::{
    BackendSession, CancelToken, DocumentJob, InteractionBroker, SessionContext, SignJob, SignResult, SignatureBackend,
};
use pida_cades::{CmsError, CmsProfile, DigestAlg, Expect, ExpectedSigner};
use pida_pades::{
    CheckContext, FieldTarget, FinishError, Invariant, OutputTarget, PadesError, Prepared, Reserve, Visibility,
    analyze, empty_fields, find_placeholder, prepare,
};
use pida_pdf_cos::{Limits, Object, SourcePdf};
use pida_placement::PlaceholderMode;

fn ctx() -> SessionContext {
    SessionContext { language: Default::default(), batch_size: 1, cancel: CancelToken::new(), headless: false }
}

fn noop(_: &mut pida_pades::PlanOptions, _: &pida_pades::DocReport, _: &SourcePdf) {}

/// Signaturdictionary-Eintrag eines Felds in der Ausgabe.
fn sig_value(src: &SourcePdf, field_name: &str, key: &[u8]) -> Option<Object> {
    let f = src.signature_fields().ok()?.into_iter().find(|f| f.qualified_name == field_name)?;
    let d = src.get_object(f.value_id?).ok()?.as_dict().ok()?.clone();
    d.get(key).ok().map(|o| src.resolve(o).unwrap().clone())
}

#[test]
fn flows() {
    let or = Oracles::detect();
    let work = tempfile::tempdir().unwrap();
    let trust: Vec<PathBuf> = KEYS.iter().map(|(_, s)| key_cert_pem(s)).collect();
    let mut sections: Vec<(String, Vec<Outcome>)> = Vec::new();
    let mut signers: Vec<KeystoreSigner> = KEYS.iter().map(|(_, s)| KeystoreSigner::open(s)).collect();

    // --- Mehrfachsignatur: 3 neue Felder nacheinander -------------------------------------------
    {
        let path = testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf");
        let mut bytes = std::fs::read(&path).unwrap();
        let mut outcomes = Vec::new();
        for step in 0..3 {
            let profile = ["SIGNATURBLOCK_SMALL_DE", "SIGNATURBLOCK_SMALL_EN_NOTE", "SIGNATURBLOCK_SMALL_DE"][step];
            let cms = if step == 1 { CmsProfile::PadesBB } else { CmsProfile::PdfasCompat };
            let label = format!("`simple.pdf` Schritt {} ({}, {profile})", step + 1, KEYS[step].0);
            let o = run_case(
                &or,
                &mut signers[step],
                &path,
                &bytes,
                profile,
                cms,
                &noop,
                work.path(),
                &format!("multi-{step}"),
                &label,
            );
            assert_eq!(o.field, format!("PDF-AS Signatur{}", step + 1), "Feldnummerierung");
            // mit PDFium legt der Rasterscan den Block unter den vorigen (kein Hinweis); ohne
            // Renderer gilt die Seite wie bei PDF-AS als leer (Hinweis „Scan fehlgeschlagen“)
            if or.pdfium.is_some() {
                assert_eq!(o.notices, "–", "Schritt {step}: {}", o.notices);
            } else {
                assert!(o.notices.contains("placement.notice.scan_failed"), "Schritt {step}: {}", o.notices);
            }
            if let Some(s) = &o.signed {
                bytes = s.bytes();
            }
            outcomes.push(o);
        }
        // am Ende: alle drei Signaturen gültig, die ersten beiden höchstens FORM_FILLING
        if let (Some(py), Some(last)) = (&or.python, outcomes.last().and_then(|o| o.out.clone())) {
            let r = pyhanko_batch(py, std::slice::from_ref(&last), &trust).unwrap();
            let sigs = &r[&last.to_string_lossy().to_string()];
            assert_eq!(sigs.len(), 3);
            for s in sigs {
                assert!(s.intact && s.valid && s.trusted, "{s:?}");
                assert!(modification_rank(s.modification_level.as_deref()) <= 2, "{s:?}");
            }
        }
        sections.push(("Mehrfachsignatur (3 neue Felder nacheinander, Modell A)".into(), outcomes));
    }

    // --- QR-Platzhalter: 3× nacheinander ----------------------------------------------------------
    {
        let path = testdata().join("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf");
        let mut bytes = std::fs::read(&path).unwrap();
        let mut outcomes = Vec::new();
        let mut used = Vec::new();
        for step in 0..3 {
            let chosen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let c2 = Arc::clone(&chosen);
            let adjust = move |opts: &mut pida_pades::PlanOptions, report: &pida_pades::DocReport, src: &SourcePdf| {
                let renderer = open_renderer(src).expect("PDFium für die Platzhaltersuche");
                let p = find_placeholder(src, report, &*renderer, None, PlaceholderMode::Lenient).unwrap();
                *c2.lock().unwrap() = p.as_ref().map(|p| p.resource_name.clone());
                opts.placeholder = p;
            };
            if or.pdfium.is_none() {
                break;
            }
            let label = format!("`new_qr_2-2.pdf` Platzhalter Schritt {} ({})", step + 1, KEYS[step].0);
            let o = run_case(
                &or,
                &mut signers[step],
                &path,
                &bytes,
                "SIGNATURBLOCK_SMALL_DE",
                CmsProfile::PdfasCompat,
                &adjust,
                work.path(),
                &format!("qr-{step}"),
                &label,
            );
            let name = chosen.lock().unwrap().clone();
            if let Some(s) = &o.signed {
                bytes = s.bytes();
                let out = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
                let loc = sig_value(&out, &o.field, b"Location").and_then(|l| pida_pdf_cos::decode_text_string(&l));
                assert_eq!(loc, name, "Schritt {step}: /Location = Platzhaltername");
                if let Some(n) = &name {
                    assert!(!used.contains(n), "Platzhalter {n} doppelt benutzt");
                    used.push(n.clone());
                }
            }
            outcomes.push(o);
        }
        assert!(or.pdfium.is_none() || used.len() >= 2, "mindestens zwei Platzhalter erwartet: {used:?}");
        sections.push((format!("QR-Platzhalter nacheinander (benutzt: {})", used.join(", ")), outcomes));
    }

    // --- Vorhandene leere Felder ------------------------------------------------------------------
    {
        let path = testdata().join("pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf");
        let mut bytes = std::fs::read(&path).unwrap();
        let mut outcomes = Vec::new();
        for (step, profile) in
            ["SIGNATURBLOCK_SMALL_DE", "INVISIBLE", "AMTSSIGNATURBLOCK_DE_RECOMMENDED"].iter().enumerate()
        {
            let src = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
            let report = analyze(&src).unwrap();
            let cand = empty_fields(&report).into_iter().next().expect("leeres Feld");
            let field_before = report.fields.iter().find(|f| f.field_id == cand.field_id).unwrap().clone();
            let target = FieldTarget::Existing {
                field_id: cand.field_id,
                widget_id: cand.widget_id,
                qualified_name: cand.qualified_name.clone(),
            };
            let t2 = target.clone();
            let adjust = move |opts: &mut pida_pades::PlanOptions, _: &pida_pades::DocReport, _: &SourcePdf| {
                opts.field = t2.clone();
            };
            let label = format!("`manySignFields.pdf` Feld {} ({profile}, {})", step + 1, KEYS[step].0);
            let o = run_case(
                &or,
                &mut signers[step],
                &path,
                &bytes,
                profile,
                CmsProfile::PadesBB,
                &adjust,
                work.path(),
                &format!("field-{step}"),
                &label,
            );
            assert_eq!(o.field, cand.qualified_name);
            if let Some(s) = &o.signed {
                bytes = s.bytes();
                let out = SourcePdf::open(bytes.clone(), &Limits::default()).unwrap();
                let f = out.signature_fields().unwrap().into_iter().find(|f| f.field_id == cand.field_id).unwrap();
                assert!(f.signed);
                assert_eq!(f.widgets[0].rect, field_before.widgets[0].rect, "/Rect bleibt");
                assert_eq!(f.widgets[0].annot_flags & 4, 4, "Print-Flag");
                assert_eq!(
                    f.widgets[0].annot_flags & 128,
                    field_before.widgets[0].annot_flags & 128,
                    "Locked nicht gesetzt"
                );
                let wd = out.get_object(f.widgets[0].id).unwrap().as_dict().unwrap().clone();
                assert!(!wd.has(b"TU") || field_before.widgets.is_empty(), "kein /TU im Feldfall");
            }
            outcomes.push(o);
        }
        sections.push(("Vorhandene leere Signaturfelder (nacheinander)".into(), outcomes));
    }

    // --- Modell B mit Fake-BKU --------------------------------------------------------------------
    let probe_dir = work.path().join("bku-probe");
    std::fs::create_dir_all(&probe_dir).unwrap();
    if FakeBku::from_p12(KEYS[0].1, &probe_dir).is_some() {
        let cases: [(&str, &str, usize); 7] = [
            ("pdf/pdfas/pdf-as-legacy/simple.pdf", "SIGNATURBLOCK_SMALL_DE", 0),
            ("pdf/generated/simple-aes256-owner.pdf", "SIGNATURBLOCK_SMALL_DE", 1),
            ("pdf/generated/simple-xref-stream.pdf", "INVISIBLE", 2),
            ("pdf/generated/simple-xref-hybrid.pdf", "AMTSSIGNATURBLOCK_DE_RECOMMENDED", 0),
            ("pdf/generated/simple-linearized.pdf", "SIGNATURBLOCK_SMALL_EN_NOTE", 1),
            ("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf", "SIGNATURBLOCK_SMALL_DE", 2),
            ("pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf", "SIGNATURBLOCK_SMALL_DE_PDFA", 0),
        ];
        let mut outcomes = Vec::new();
        for (i, (rel_path, profile, key)) in cases.iter().enumerate() {
            let path = testdata().join(rel_path);
            let dir = work.path().join(format!("bku-{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            let bku = FakeBku::from_p12(KEYS[*key].1, &dir).unwrap();
            let src = open(&path);
            let report = analyze(&src).unwrap();
            let mut o = Outcome::new(format!("`{rel_path}` Modell B ({profile}, {})", KEYS[*key].0), &path);
            o.prior = report.signatures.len();
            match sign_model_b(&bku, &src, &report, profile, Reserve::ModelB) {
                Ok(signed) => {
                    assert!(signed.byte_range().0[2] - signed.byte_range().0[1] >= 2 * 16 * 1024, "Reserve 16 KiB");
                    let ctx = CheckContext { source: &src, expected_signer: None, renderer: or.renderer() };
                    if let Err(v) = signed.self_check(&ctx) {
                        o.problems.push(format!("self_check: {v}"));
                    }
                    let out = dir.join("signed.pdf");
                    std::fs::write(&out, signed.bytes()).unwrap();
                    let (q, s, _) = check_output(&or, &path, src.bytes(), &signed, &out, None, &dir, &mut o.problems);
                    o.qpdf = q;
                    o.openssl = s;
                    o.pixel = "–".into();
                    let out_src = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
                    o.field = out_src
                        .signature_fields()
                        .unwrap()
                        .into_iter()
                        .find(|f| f.signed && !report.fields.iter().any(|g| g.signed && g.field_id == f.field_id))
                        .map(|f| f.qualified_name)
                        .unwrap_or_default();
                    o.out = Some(out);
                    o.signed = Some(signed);
                }
                Err(e) => o.problems.push(format!("Modell B: {e}")),
            }
            outcomes.push(o);
        }
        sections.push((
            "Modell B (Fake-BKU: Blackout-PDF + ExcludedByteRange, `openssl cms -sign -cades`)".into(),
            outcomes,
        ));
    } else {
        eprintln!("HINWEIS: openssl (≥ 3, CAdES) fehlt – Modell B übersprungen");
    }

    for (_, outcomes) in sections.iter_mut() {
        pyhanko_all(&or, outcomes, &trust);
    }
    let mut md = String::from("## Abläufe (Mehrfachsignatur, QR-Platzhalter, vorhandene Felder, Modell B)\n\n");
    md.push_str("Erzeugt von `cargo test -p pida-pades --test e2e_flows`. Jeder Schritt verwendet die Ausgabe des vorigen als Eingabe; *Vorsignaturen* zeigt die pyHanko-Änderungsstufe der älteren Signaturen vorher → nachher. Modell B: Die Fake-BKU erhält das Blackout-PDF und die ExcludedByteRange über `DocumentSource` (wie der Workflow-Adapter), prüft die genullte Lücke, signiert die Bytes außerhalb mit `openssl cms -sign -binary -nosmimecap -cades -md sha256` und liefert `SignResult::Cms`; `finish_cms` prüft I3 gegen das von `certificate()` gemeldete Zertifikat, die Reserve ist 16 KiB.\n\n");
    md.push_str(&format!("Werkzeuge: {}.\n\n", or.versions()));
    for (title, outcomes) in &sections {
        let ok = outcomes.iter().filter(|o| o.problems.is_empty()).count();
        md.push_str(&format!("### {title}\n\n**{ok} von {} bestanden.**\n\n", outcomes.len()));
        md.push_str(&table(outcomes));
        md.push('\n');
    }
    write_results_section("ablaeufe", &md);
    for (_, outcomes) in &sections {
        assert_all_ok(outcomes);
    }
}

/// Modell B: CMS größer als die Reserve → `cms-too-large` mit benötigter Größe → Neuvorbereitung
/// mit `Reserve::Bytes(suggested_reserve)` gelingt.
#[test]
fn cms_too_large_then_retry_with_suggested_reserve() {
    let work = tempfile::tempdir().unwrap();
    let Some(bku) = FakeBku::from_p12("test-rsa2048", work.path()) else { return };
    let path = testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf");
    let src = open(&path);
    let report = analyze(&src).unwrap();
    let rt = runtime();
    let broker: Arc<dyn InteractionBroker> = Arc::new(ScriptedBroker::new([]));
    let mut session = rt.block_on(bku.open(ctx(), broker)).unwrap();
    let chain = rt.block_on(session.certificate()).unwrap();
    let attempt =
        |reserve: Reserve, session: &mut Box<dyn BackendSession>| -> Result<pida_pades::SignedPdf, FinishError> {
            let mut opts = pida_pades::PlanOptions::new(pida_pades::local_signing_time());
            opts.reserve = reserve;
            let planned = plan_for(&src, &report, "INVISIBLE", &chain.signer, &opts, None).unwrap();
            let prepared = prepare(&src, &planned.plan, Some(&chain)).unwrap();
            let shared = Arc::new(Mutex::new(Some(prepared)));
            let job = SignJob::Document(DocumentJob {
                source: Arc::new(PreparedSource(Arc::clone(&shared))),
                expected_signer: None,
            });
            let SignResult::Cms(cms) = rt.block_on(session.sign(vec![job])).unwrap().remove(0) else { panic!() };
            let prepared = shared.lock().unwrap().take().unwrap();
            prepared.finish_cms(&cms, &Expect::default())
        };
    let err = attempt(Reserve::Bytes(600), &mut session).unwrap_err();
    assert_eq!(err.code(), "cms-too-large");
    let FinishError::CmsTooLarge { needed, reserve } = err else { panic!() };
    assert_eq!(reserve, 600);
    assert!(needed > 600, "{needed}");
    let suggested = FinishError::CmsTooLarge { needed, reserve }.suggested_reserve().unwrap();
    assert!(suggested >= needed + 1024 && suggested.is_multiple_of(1024));
    let signed = attempt(Reserve::Bytes(suggested), &mut session).expect("Neuvorbereitung mit größerer Reserve");
    let ctx = CheckContext { source: &src, expected_signer: Some(&chain.signer), renderer: None };
    signed.self_check(&ctx).unwrap();
}

/// Falscher Signer (BKU meldet ein anderes Zertifikat) und falscher Digest (CMS über andere Daten).
#[test]
fn finish_cms_rejects_wrong_signer_and_digest() {
    let work = tempfile::tempdir().unwrap();
    let Some(mut bku) = FakeBku::from_p12("test-rsa2048", work.path()) else { return };
    bku.reported_cert = key_cert_pem("test-ecp256");
    let src = open(&testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf"));
    let report = analyze(&src).unwrap();
    let err = sign_model_b(&bku, &src, &report, "INVISIBLE", Reserve::ModelB).unwrap_err();
    assert!(err.starts_with("cms-unexpected-signer"), "{err}");

    // CMS über andere Daten
    let bku = FakeBku::from_p12("test-ecp256", work.path()).unwrap();
    let cert = pida_cades::Certificate::from_pem(&std::fs::read_to_string(&bku.cert_pem).unwrap()).unwrap();
    let chain = pida_cades::CertChain::new(cert.clone());
    let opts = pida_pades::PlanOptions {
        reserve: Reserve::ModelB,
        ..pida_pades::PlanOptions::new(pida_pades::local_signing_time())
    };
    let planned = plan_for(&src, &report, "INVISIBLE", &cert, &opts, None).unwrap();
    let prepared = prepare(&src, &planned.plan, Some(&chain)).unwrap();
    let cms = bku.sign_data(b"andere Daten").unwrap();
    let err = prepared
        .finish_cms(&cms, &Expect { signer: ExpectedSigner::Certificate(cert), ..Expect::default() })
        .unwrap_err();
    assert!(matches!(err, FinishError::Cms(CmsError::DigestMismatch)), "{err:?}");
}

/// Blackout-PDF und Digest passen zu den ByteRange-Daten des Platzhalter-PDFs.
#[test]
fn blacked_out_and_digest_match_placeholder() {
    let src = open(&testdata().join("pdf/generated/simple-xref-stream.pdf"));
    let report = analyze(&src).unwrap();
    let signer = KeystoreSigner::open("test-ecp256");
    let (prepared, _) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_DE",
        &signer.chain.signer,
        &signer.chain,
        CmsProfile::PadesBB,
        FieldTarget::New { name: None },
    )
    .unwrap();
    let full = prepared.placeholder_pdf();
    assert_eq!(full.len() as u64, prepared.total_len());
    assert_eq!(&full[..src.bytes().len()], src.bytes());
    let mut blacked = Vec::new();
    prepared.blacked_out().read_to_end(&mut blacked).unwrap();
    let ex = prepared.excluded_range();
    assert_eq!(blacked.len(), full.len());
    assert_eq!(full[ex.from as usize], b'<');
    assert_eq!(full[ex.to as usize], b'>');
    assert!(blacked[ex.from as usize..=ex.to as usize].iter().all(|b| *b == 0));
    assert_eq!(&blacked[..ex.from as usize], &full[..ex.from as usize]);
    assert_eq!(&blacked[ex.to as usize + 1..], &full[ex.to as usize + 1..]);
    let br = prepared.byte_range().0;
    assert_eq!((br[1], br[2]), (ex.from, ex.to + 1));
    let mut data = full[..br[1] as usize].to_vec();
    data.extend_from_slice(&full[br[2] as usize..]);
    assert_eq!(prepared.digest(DigestAlg::Sha256), DigestAlg::Sha256.digest(&data));
    assert_eq!(prepared.reserve() as u64 * 2 + 2, br[2] - br[1]);
}

/// Persistenz (Mobile): persist → restore → signieren; Manipulationen werden erkannt.
#[test]
fn persist_and_restore() {
    let dir = tempfile::tempdir().unwrap();
    let src = open(&testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf"));
    let report = analyze(&src).unwrap();
    let mut signer = KeystoreSigner::open("test-rsa2048");
    let chain = signer.chain.clone();
    let (prepared, _) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_EN_NOTE",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    let digest = prepared.digest(DigestAlg::Sha256);
    let handle = prepared.persist(dir.path()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&handle.path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    drop(prepared);
    let restored = Prepared::restore(&handle, &src).unwrap();
    assert_eq!(restored.digest(DigestAlg::Sha256), digest);
    let signed = signer.sign(restored).unwrap();
    signed.self_check(&CheckContext { source: &src, expected_signer: Some(&chain.signer), renderer: None }).unwrap();

    // anderes Original
    let other = open(&testdata().join("pdf/generated/simple-xref-table.pdf"));
    assert!(matches!(Prepared::restore(&handle, &other), Err(PadesError::InvalidState(_))));
    // manipuliertes PDF
    let mut bytes = std::fs::read(&handle.path).unwrap();
    let last = bytes.len() - 10;
    bytes[last] ^= 0x01;
    std::fs::write(&handle.path, &bytes).unwrap();
    assert!(matches!(Prepared::restore(&handle, &src), Err(PadesError::InvalidState(_))));
    bytes[last] ^= 0x01;
    std::fs::write(&handle.path, &bytes).unwrap();
    // manipulierte Begleitdatei (Offsets)
    let json_path = handle.path.with_extension("json");
    let json = std::fs::read_to_string(&json_path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let c0 = v["contents_range"][0].as_u64().unwrap();
    v["contents_range"][0] = serde_json::json!(c0 + 1);
    std::fs::write(&json_path, v.to_string()).unwrap();
    assert!(matches!(Prepared::restore(&handle, &src), Err(PadesError::InvalidState(_))));
    std::fs::write(&json_path, json).unwrap();
    assert!(Prepared::restore(&handle, &src).is_ok());
    Prepared::discard_persisted(&handle).unwrap();
    assert!(!handle.path.exists() && !json_path.exists());
}

/// Atomares Schreiben: kein Überschreiben ohne Erlaubnis, keine Temp-Reste, Prüfsumme stimmt.
#[test]
fn write_atomically_respects_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let src = open(&testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf"));
    let mut signer = KeystoreSigner::open("test-ecp256");
    let chain = signer.chain.clone();
    let plan = pida_pades::SignPlan::invisible(pida_pades::local_signing_time());
    let signed = signer.sign(prepare(&src, &plan, Some(&chain)).unwrap()).unwrap();
    let target = dir.path().join("out.pdf");
    // ohne bzw. nach fehlgeschlagener Selbstprüfung wird nichts geschrieben
    let not_checked = signed.write_atomically(&OutputTarget { path: target.clone(), overwrite: true });
    assert!(matches!(not_checked, Err(PadesError::NotSelfChecked)), "{not_checked:?}");
    assert_eq!(not_checked.unwrap_err().code(), "pades-not-self-checked");
    assert_eq!(signed.write_to(&mut Vec::new()).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    let other = open(&testdata().join("pdf/generated/simple-xref-table.pdf"));
    assert!(signed.self_check(&CheckContext { source: &other, expected_signer: None, renderer: None }).is_err());
    assert!(matches!(
        signed.write_atomically(&OutputTarget { path: target.clone(), overwrite: true }),
        Err(PadesError::NotSelfChecked)
    ));
    assert!(!target.exists());
    signed.self_check(&CheckContext { source: &src, expected_signer: Some(&chain.signer), renderer: None }).unwrap();
    assert!(signed.is_checked());
    std::fs::write(&target, b"vorhanden").unwrap();
    let err = signed.write_atomically(&OutputTarget { path: target.clone(), overwrite: false }).unwrap_err();
    assert!(matches!(&err, PadesError::Io(e) if e.kind() == std::io::ErrorKind::AlreadyExists), "{err:?}");
    assert_eq!(std::fs::read(&target).unwrap(), b"vorhanden");
    let info = signed.write_atomically(&OutputTarget { path: target.clone(), overwrite: true }).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), signed.bytes());
    assert_eq!(info.len, signed.len());
    assert_eq!(info.sha256, DigestAlg::Sha256.digest(&signed.bytes()).to_hex());
    let fresh = dir.path().join("neu.pdf");
    signed.write_atomically(&OutputTarget { path: fresh.clone(), overwrite: false }).unwrap();
    let names: Vec<String> =
        std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(names.iter().all(|n| !n.contains("pida-tmp")), "{names:?}");
}

/// Selbstprüfung erkennt falsches Original (I1) und falschen erwarteten Signer (I3).
#[test]
fn self_check_detects_wrong_source_and_signer() {
    let src = open(&testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf"));
    let mut signer = KeystoreSigner::open("test-rsa2048");
    let chain = signer.chain.clone();
    let plan = pida_pades::SignPlan::invisible(pida_pades::local_signing_time());
    let signed = signer.sign(prepare(&src, &plan, Some(&chain)).unwrap()).unwrap();
    let other = open(&testdata().join("pdf/generated/simple-xref-table.pdf"));
    let v = signed.self_check(&CheckContext { source: &other, expected_signer: None, renderer: None }).unwrap_err();
    assert_eq!(v.invariant, Invariant::I1Prefix);
    let wrong = KeystoreSigner::open("test-ecp384-title").chain.signer;
    let v =
        signed.self_check(&CheckContext { source: &src, expected_signer: Some(&wrong), renderer: None }).unwrap_err();
    assert_eq!(v.invariant, Invariant::I3Cms);
    assert_eq!(v.invariant.code(), "pades-invariant-i3");
    let ok = signed.self_check(&CheckContext { source: &src, expected_signer: None, renderer: None }).unwrap();
    assert_eq!(ok.warnings, vec![pida_pades::WARNING_I5_WITHOUT_PDFIUM.to_string()]);
}

/// Neue Seite auf einem signierten Dokument und Zertifikat außerhalb der Gültigkeit.
#[test]
fn new_page_on_signed_document_and_expired_certificate_are_refused() {
    let src = open(&testdata().join("pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf"));
    let report = analyze(&src).unwrap();
    assert!(report.is_signed());
    let signer = KeystoreSigner::open("test-rsa2048");
    let mut opts = pida_pades::PlanOptions::new(pida_pades::local_signing_time());
    opts.position = pida_placement::TablePos::parse("p:new").unwrap();
    let planned = plan_for(&src, &report, "SIGNATURBLOCK_SMALL_DE", &signer.chain.signer, &opts, None).unwrap();
    assert!(planned.notices.contains(&pida_placement::PlacementNotice::NewPageNotAllowed));
    let mut plan = planned.plan.clone();
    let Visibility::Visible(bp) = &mut plan.visibility else { panic!() };
    bp.new_page = true;
    bp.page_index = src.page_count();
    assert!(matches!(prepare(&src, &plan, Some(&signer.chain)), Err(PadesError::NewPageNotAllowed)));
    // Zertifikat erst ab Oktober 2026 gültig
    let mut early = planned.plan.clone();
    early.signing_time = time::macros::datetime!(2020-01-01 12:00 +01:00);
    assert!(matches!(prepare(&src, &early, Some(&signer.chain)), Err(PadesError::CertificateNotValid)));
}

/// PDF/UA: StructElem `/Form` mit OBJR, ParentTree-Eintrag, `/ParentTreeNextKey`, `/Tabs /S`;
/// bei Vorsignaturen nicht (Hinweis).
#[test]
fn pdfua_structure_is_extended() {
    let src = open(&testdata().join("pdf/pdfas/pdf-as-cli/rotate_180.pdf"));
    let report = analyze(&src).unwrap();
    assert!(report.tagged);
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
    assert!(planned.plan.pdfua);
    assert!(prepared.notices().is_empty(), "{:?}", prepared.notices());
    let signed = signer.sign(prepared).unwrap();
    signed.self_check(&CheckContext { source: &src, expected_signer: None, renderer: None }).unwrap();
    let out = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
    let field = out.signature_fields().unwrap().into_iter().find(|f| f.qualified_name == "PDF-AS Signatur1").unwrap();
    let widget = out.get_object(field.field_id).unwrap().as_dict().unwrap().clone();
    let key = widget.get(b"StructParent").unwrap().as_i64().unwrap();
    assert!(widget.has(b"TU"));
    let catalog = out.catalog().unwrap();
    let root = out.resolve(catalog.get(b"StructTreeRoot").unwrap()).unwrap().as_dict().unwrap();
    assert_eq!(out.resolve(root.get(b"ParentTreeNextKey").unwrap()).unwrap().as_i64().unwrap(), key + 1);
    // ParentTree: Schlüssel → StructElem → OBJR → Widget
    fn find(src: &SourcePdf, node: &pida_pdf_cos::Dictionary, key: i64) -> Option<Object> {
        if let Ok(nums) = node.get(b"Nums") {
            let a = src.resolve(nums).ok()?.as_array().ok()?;
            for pair in a.chunks(2) {
                if src.resolve(&pair[0]).ok()?.as_i64().ok()? == key {
                    return Some(pair[1].clone());
                }
            }
        }
        if let Ok(kids) = node.get(b"Kids") {
            for k in src.resolve(kids).ok()?.as_array().ok()? {
                if let Some(o) = find(src, src.resolve(k).ok()?.as_dict().ok()?, key) {
                    return Some(o);
                }
            }
        }
        None
    }
    let pt = out.resolve(root.get(b"ParentTree").unwrap()).unwrap().as_dict().unwrap();
    let elem_ref = find(&out, pt, key).expect("ParentTree-Eintrag");
    let elem = out.resolve(&elem_ref).unwrap().as_dict().unwrap();
    assert_eq!(elem.get(b"S").unwrap().as_name().unwrap(), b"Form");
    let k = out.resolve(elem.get(b"K").unwrap()).unwrap().as_array().unwrap();
    let objr = out.resolve(&k[0]).unwrap().as_dict().unwrap();
    assert_eq!(objr.get(b"Obj").unwrap(), &Object::Reference(field.field_id));
    let page = out.get_object(out.pages()[field.widgets[0].page_index.unwrap()].object_id).unwrap().as_dict().unwrap();
    assert_eq!(page.get(b"Tabs").unwrap().as_name().unwrap(), b"S");

    // vorsigniertes, getaggtes Dokument: keine Strukturänderung
    let src = open(&testdata().join("pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf"));
    let report = analyze(&src).unwrap();
    let (prepared, _) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_DE",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    assert_eq!(prepared.notices(), &["pades-pdfua-skipped".to_string()]);
}

/// PDF/A (Plan §6.9): ohne OutputIntent wird ein sRGB-Intent ergänzt, vorhandene bleiben; bei
/// PDF/A-Eingaben kein PDFBox-Fixup (`/Helv`/`/ZaDb`), Bildmarke als ICCBased.
#[test]
fn pdfa_output_intent_strategy() {
    let mut signer = KeystoreSigner::open("test-rsa2048");
    let chain = signer.chain.clone();
    let intents = |s: &SourcePdf| -> Vec<Object> {
        s.catalog()
            .unwrap()
            .get(b"OutputIntents")
            .ok()
            .and_then(|o| s.resolve(o).ok())
            .and_then(|o| o.as_array().ok().cloned())
            .unwrap_or_default()
    };
    // kein PDF/A-Dokument, PDF/A-Profil: Intent wird ergänzt (PDF-AS: Profil ohne XMP ⇒ Teil 1)
    let src = open(&testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf"));
    assert!(intents(&src).is_empty());
    let report = analyze(&src).unwrap();
    let (prepared, planned) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_DE_PDFA",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    assert!(planned.plan.add_output_intent_if_missing);
    let signed = signer.sign(prepared).unwrap();
    let out = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
    let oi = intents(&out);
    assert_eq!(oi.len(), 1);
    let oi = out.resolve(&oi[0]).unwrap().as_dict().unwrap();
    assert_eq!(oi.get(b"S").unwrap().as_name().unwrap(), b"GTS_PDFA1");
    let icc_id = match oi.get(b"DestOutputProfile").unwrap() {
        Object::Reference(id) => *id,
        other => panic!("{other:?}"),
    };
    let icc = out.decoded_stream(icc_id).unwrap();
    assert_eq!(&icc[36..40], b"acsp");
    let acro = out.resolve(out.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().as_dict().unwrap();
    let dr = out.resolve(acro.get(b"DR").unwrap()).unwrap().as_dict().unwrap();
    assert!(dr.has(b"Font"), "Fixup bei Nicht-PDF/A");

    // PDF/A-1b ohne OutputIntent: wird ergänzt
    let src = open(&testdata().join("pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf"));
    assert!(intents(&src).is_empty());
    let report = analyze(&src).unwrap();
    assert_eq!(report.pdfa.as_ref().map(|p| p.part), Some(1));
    let (prepared, _) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_DE_PDFA",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    let signed = signer.sign(prepared).unwrap();
    let out = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
    assert_eq!(intents(&out).len(), 1);
    // … aber nicht bei Vorsignaturen (Änderungsanalyse), mit Hinweis
    let report2 = analyze(&out).unwrap();
    let (prepared, _) = prepare_with(
        &out,
        &report2,
        "SIGNATURBLOCK_SMALL_DE_PDFA",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    assert!(prepared.notices().is_empty(), "Intent schon vorhanden: {:?}", prepared.notices());

    // PDF/A-1b mit vorhandenem OutputIntent: bleibt unverändert, kein Fixup
    let src = open(&testdata().join("pdf/pdfas-suites/POS_LAST/example.pdf"));
    let before = intents(&src);
    assert!(!before.is_empty());
    let report = analyze(&src).unwrap();
    assert_eq!(report.pdfa.as_ref().map(|p| p.part), Some(3));
    let (prepared, _) = prepare_with(
        &src,
        &report,
        "SIGNATURBLOCK_SMALL_DE_PDFA",
        &chain.signer,
        &chain,
        CmsProfile::PdfasCompat,
        FieldTarget::New { name: None },
    )
    .unwrap();
    let signed = signer.sign(prepared).unwrap();
    signed.self_check(&CheckContext { source: &src, expected_signer: None, renderer: None }).unwrap();
    let out = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
    assert_eq!(intents(&out), before, "vorhandene OutputIntents bleiben");
    let acro = out.resolve(out.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().as_dict().unwrap();
    let dr = out.resolve(acro.get(b"DR").unwrap()).unwrap().as_dict().unwrap();
    assert!(!dr.has(b"Font") && !acro.has(b"DA"), "kein Fixup bei PDF/A");
    // Bildmarke ICCBased statt DeviceRGB
    let field = out.signature_fields().unwrap().into_iter().find(|f| f.signed).unwrap();
    let w = out.get_object(field.widgets[0].id).unwrap().as_dict().unwrap();
    let holder = out.resolve(w.get(b"AP").unwrap()).unwrap().as_dict().unwrap().get(b"N").unwrap().clone();
    let holder = out.resolve(&holder).unwrap().as_stream().unwrap();
    let inner_ref = holder
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"FRM1")
        .unwrap()
        .clone();
    let inner = out.resolve(&inner_ref).unwrap().as_stream().unwrap();
    let res = inner.dict.get(b"Resources").unwrap().as_dict().unwrap();
    let im =
        out.resolve(res.get(b"XObject").unwrap().as_dict().unwrap().get(b"Im1").unwrap()).unwrap().as_stream().unwrap();
    let cs = out.resolve(im.dict.get(b"ColorSpace").unwrap()).unwrap();
    assert!(matches!(cs, Object::Array(a) if a.first().and_then(|n| n.as_name().ok()) == Some(b"ICCBased")), "{cs:?}");
    assert!(!im.dict.has(b"SMask"), "PDF/A-1: kein Alpha");
}
