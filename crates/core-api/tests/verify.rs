//! Prüfmodus gegen den echten Kern (ADR 0004 §3.9, Etappe E4, Paket K4): `VerifyView` je Datei aus
//! `testdata/verify/*` nach der Tabelle in §3.17 (intakt/verändert/nicht prüfbar, SHA-1-Hinweis,
//! Unterzeichner, Zeit als Angabe des Signierenden, Zertifikatsdetails, QES „laut Zertifikat“, spätere
//! Revisionen), `AppearanceView` in `PAGE_ABS` auch auf einer gedrehten Seite, eigenes Prüfdokument (nicht in
//! der Warteschlange, beim Schließen freigegeben), Zustand bleibt, während des Signierens `ng-verify-busy`,
//! „Dieses Dokument signieren“ wie `open_files` (Warteschlange + Hinweis bei offenem Dokument).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::*;
use pida_backend_api::{BackendId, SignatureBackend};
use pida_config::Overlay;
use pida_core_api::{
    CoreOptions, DocumentPhase, QualificationClaim, SignatureCoverage, SignatureFormat, SignatureIntegrity, StepView,
    TrustCheck, UiEvent, VerifiedSignatureView, VerifyStatus, VerifySummaryView, VerifyView, ViewState,
};
use zeroize::Zeroizing;

const TEXT_PDF: &str = "pdf/pdfas/pdf-as-legacy/simple.pdf";
const CN_RSA: &str = "PDF-Over NG Test RSA (NUR TEST)";
const CN_P256: &str = "PDF-Over NG Test ECDSA P-256 (NUR TEST)";
const ORG: &str = "PDF-Over NG Test (NUR TEST)";

fn pdfium_dir() -> Option<PathBuf> {
    std::env::var_os("PIDA_PDFIUM_DIR").filter(|d| !d.is_empty()).map(Into::into)
}

/// Kern mit Oberfläche im Arbeitsbereich-Modus (Bestandsnutzer: ohne Ersteinrichtung).
fn start(fx: &Fixture, overlay: Overlay, adjust: impl FnOnce(&mut CoreOptions)) -> pida_core_api::Core {
    fx.write_settings("[general]\n");
    let mut opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        ..CoreOptions::isolated()
    };
    adjust(&mut opts);
    pida_core_api::Core::new_with(fx.config(overlay, false), opts).expect("Kern startet")
}

fn verify_file(name: &str) -> PathBuf {
    testdata(&format!("verify/{name}"))
}

fn ready(v: &ViewState) -> bool {
    v.verify.as_ref().is_some_and(|f| !matches!(f.status, VerifyStatus::Loading))
}

async fn open(core: &pida_core_api::Core, path: &Path) -> VerifyView {
    core.dispatch(UiEvent::OpenVerify { path: path.to_path_buf() });
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let v =
        wait_for(core, "Prüfergebnis", |v| ready(v) && v.verify.as_ref().is_some_and(|f| f.file_name == name)).await;
    v.verify.unwrap()
}

fn one(view: &VerifyView) -> &VerifiedSignatureView {
    let [s] = view.signatures.as_slice() else { panic!("genau eine Signatur: {view:?}") };
    s
}

fn rfc3339(s: &Option<String>) -> time::OffsetDateTime {
    time::OffsetDateTime::parse(s.as_deref().expect("Zeitangabe"), &time::format_description::well_known::Rfc3339)
        .expect("RFC 3339")
}

fn editing(v: &ViewState) -> bool {
    v.document.as_ref().is_some_and(|d| d.phase == DocumentPhase::Editing)
}

/// Abnahme E4 Punkt 1/2 [K]: Ergebnis je Testdatei nach der Tabelle (ADR 0004 §3.17, testdata/verify/README.md).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_view_follows_the_table() {
    let fx = Fixture::new();
    let core = start(&fx, Overlay::default(), |_| {});
    let v = wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(v.actions.verify, "Prüfmodus aus der Startansicht");

    // ohne Signatur
    let f = open(&core, &verify_file("unsigned.pdf")).await;
    assert_eq!((f.status.clone(), f.summary, f.revisions), (VerifyStatus::Ready, VerifySummaryView::NoSignatures, 1));
    assert!(f.signatures.is_empty());
    let doc = f.doc.as_ref().expect("Kacheln");
    assert!(doc.page_count == 1 && !doc.signed && doc.page_sizes == vec![(612.0, 792.0)], "{doc:?}");

    // PDF-AS-kompatibel signiert: intakt, alle Angaben
    let f = open(&core, &verify_file("pdfas-signed.pdf")).await;
    assert_eq!((f.summary, f.revisions), (VerifySummaryView::AllIntact, 2));
    assert!(f.doc.as_ref().unwrap().signed);
    let s = one(&f);
    let cert =
        pida_cades::Certificate::from_der(&std::fs::read(testdata("keystores/test-rsa2048.crt.der")).unwrap()).unwrap();
    assert_eq!(s.index, 1);
    assert_eq!(s.signer.as_deref(), Some(CN_RSA));
    assert_eq!(s.organization.as_deref(), Some(ORG));
    assert_eq!(s.issuer.as_deref(), Some(CN_RSA), "selbstsigniert");
    assert_eq!(s.serial.as_deref(), Some(cert.serial_decimal().as_str()));
    assert_eq!(rfc3339(&s.cert_valid_from).unix_timestamp(), cert.not_before().unix_timestamp());
    assert_eq!(rfc3339(&s.cert_valid_to).unix_timestamp(), cert.not_after().unix_timestamp());
    let signed_at = rfc3339(&s.signing_time);
    assert!(cert.is_valid_at(signed_at) && s.cert_valid_at_signing == Some(true));
    assert_eq!(s.integrity, SignatureIntegrity::Intact);
    assert_eq!(s.coverage, SignatureCoverage::WholeDocument);
    assert_eq!(s.qualification, QualificationClaim::NotQualified);
    assert_eq!(s.format, SignatureFormat::Pades);
    assert_eq!((s.digest.as_deref(), s.algorithm.as_deref()), (Some("SHA-256"), Some("RSA 2048")));
    assert!(s.warnings.is_empty());
    assert_eq!(s.trust, TrustCheck::NotChecked, "Vertrauenskette und Sperrstatus prüft PIDA nicht");
    let a = s.appearance.expect("sichtbar");
    assert_eq!(a.page, 0);
    assert!(a.rect.right - a.rect.left > 100.0 && a.rect.top - a.rect.bottom > 20.0, "{a:?}");
    assert!(a.rect.left >= 0.0 && a.rect.right <= 612.0 && a.rect.bottom >= 0.0 && a.rect.top <= 792.0, "{a:?}");
    // Personendaten nie im `Debug` (Regel 9): Unterzeichner, Organisation, Seriennummer, Feld- und Dateiname
    let debug = format!("{:?}", core.view());
    for secret in [CN_RSA, ORG, s.serial.as_deref().unwrap(), s.field_name.as_str(), "pdfas-signed"] {
        assert!(!debug.contains(secret), "„{secret}“ im Debug des ViewState");
    }

    // manipuliert: verändert, Unterzeichner trotzdem genannt
    let f = open(&core, &verify_file("tampered.pdf")).await;
    assert_eq!(f.summary, VerifySummaryView::SomeBroken);
    let s = one(&f);
    assert_eq!(s.integrity, SignatureIntegrity::Broken { code: "cms-digest-mismatch".into() });
    assert_eq!(s.signer.as_deref(), Some(CN_RSA));
    assert!(s.warnings.is_empty(), "Hinweise nur zu intakten Signaturen");

    // adbe.pkcs7.sha1: nicht prüfbar
    let f = open(&core, &verify_file("pkcs7-sha1.pdf")).await;
    assert_eq!(f.summary, VerifySummaryView::SomeNotVerifiable);
    let s = one(&f);
    assert_eq!(s.integrity, SignatureIntegrity::NotVerifiable { code: "pdf-signature-subfilter".into() });
    assert_eq!(s.format, SignatureFormat::Pkcs7Sha1);
    assert_eq!(s.signer.as_deref(), Some(CN_RSA));

    // CAdES mit SHA-1: intakt mit Hinweis
    let f = open(&core, &verify_file("cades-sha1.pdf")).await;
    assert_eq!(f.summary, VerifySummaryView::AllIntact);
    let s = one(&f);
    assert_eq!(s.integrity, SignatureIntegrity::Intact);
    assert_eq!(s.digest.as_deref(), Some("SHA-1"));
    assert_eq!(s.warnings, vec!["ng-verify-warning-sha1", "ng-verify-warning-signing-certificate-v1"]);

    // zwei Signaturen, die erste von einer späteren Revision überdeckt
    let f = open(&core, &verify_file("two-signatures.pdf")).await;
    assert_eq!((f.summary, f.revisions), (VerifySummaryView::AllIntact, 3));
    let [first, second] = f.signatures.as_slice() else { panic!("{f:?}") };
    assert_eq!((first.index, second.index), (1, 2));
    assert_eq!((first.signer.as_deref(), second.signer.as_deref()), (Some(CN_RSA), Some(CN_P256)));
    assert_eq!(first.coverage, SignatureCoverage::Revision { later_revisions: 1, later_signatures: 1 });
    assert_eq!(second.coverage, SignatureCoverage::WholeDocument);
    assert_eq!(second.algorithm.as_deref(), Some("ECDSA P-256"));
    assert!(first.appearance.is_some() && second.appearance.is_some());

    // nach der letzten Signatur ergänzt
    let f = open(&core, &verify_file("modified-after-signing.pdf")).await;
    assert_eq!((f.summary, f.revisions), (VerifySummaryView::ModifiedAfterLastSignature, 3));
    let s = one(&f);
    assert_eq!(s.integrity, SignatureIntegrity::Intact);
    assert_eq!(s.coverage, SignatureCoverage::Revision { later_revisions: 1, later_signatures: 0 });

    // unsichtbar: ohne Lage
    let f = open(&core, &verify_file("invisible.pdf")).await;
    let s = one(&f);
    assert_eq!((s.integrity.clone(), s.appearance), (SignatureIntegrity::Intact, None));

    // QES-Testzertifikat: qualifiziert laut Zertifikat (QcCompliance + QcSSCD)
    let f = open(&core, &verify_file("qes.pdf")).await;
    let s = one(&f);
    assert_eq!(s.qualification, QualificationClaim::Qualified { qscd: true });
    assert_eq!(s.signer.as_deref(), Some("PIDA Test QES P-256 (NUR TEST)"));
    assert_eq!(s.organization.as_deref(), Some("PIDA Test (NUR TEST)"));
    assert_eq!(s.algorithm.as_deref(), Some("ECDSA P-256"));

    // gedrehte Seite (`/Rotate 90`): Rechteck im sichtbaren Raum (PAGE_ABS) an der beim Signieren gewählten
    // Lage `p:1;x:100;y:500` (crates/pades/tests/verify_testdata.rs)
    let f = open(&core, &verify_file("rotated-90.pdf")).await;
    assert_eq!(f.doc.as_ref().unwrap().page_sizes, vec![(792.0, 612.0)], "sichtbar quer");
    let a = one(&f).appearance.expect("sichtbar");
    assert_eq!(a.page, 0);
    assert!((a.rect.left - 100.0).abs() < 0.5 && (a.rect.top - 500.0).abs() < 0.5, "{a:?}");
    assert!(a.rect.right - a.rect.left > 100.0 && a.rect.top - a.rect.bottom > 20.0, "{a:?}");
    assert!(a.rect.right <= 792.0 && a.rect.bottom >= 0.0, "{a:?}");

    // nicht lesbar
    let junk = fx.dir("x").join("kein.pdf");
    std::fs::write(&junk, b"kein PDF").unwrap();
    let f = open(&core, &junk).await;
    assert_eq!(f.status, VerifyStatus::Failed { code: "ng-verify-open-failed".into() });
    assert!(f.doc.is_none() && f.signatures.is_empty());
    core.dispatch(UiEvent::CloseVerify);
    wait_for(&core, "geschlossen", |v| v.verify.is_none()).await;
    assert_eq!(core.open_document_count(), 0, "Prüfdokumente freigegeben");
}

/// Prüfdokument mit eigener `DocId` (nicht in der Warteschlange): Kacheln über `page_tile`; der Zustand des
/// Arbeitsbereichs bleibt; Schließen gibt es frei; eine neue Prüfung ersetzt die vorige.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_mode_keeps_the_workspace_and_frees_its_document() {
    let fx = Fixture::new();
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "Vertrag.pdf");
    let pdfium = pdfium_dir();
    let core = start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }, |o| {
        if pdfium.is_some() {
            o.use_pdfium = true;
        }
    });
    let before = wait_for(&core, "Arbeitsbereich", editing).await;
    let doc_before = before.document.clone().unwrap();
    assert!(before.actions.verify);
    let open_docs = core.open_document_count();
    let f = open(&core, &verify_file("two-signatures.pdf")).await;
    let v = core.view();
    assert!(editing(&v) && matches!(v.step, StepView::Position { .. }), "Zustand bleibt: {:?}", v.step);
    assert_eq!(v.document.as_ref().unwrap().doc.id, doc_before.doc.id);
    let verify_doc = f.doc.as_ref().unwrap().id;
    assert_ne!(verify_doc, doc_before.doc.id, "eigene DocId");
    assert!(v.batch.is_none() && matches!(&v.step, StepView::Position { .. }));
    assert_eq!(core.open_document_count(), open_docs + 1);
    if pdfium.is_some() && core.has_renderer() {
        let png = core.page_tile(verify_doc, 0, 0.5).expect("Kachel des Prüfdokuments");
        assert!(png.starts_with(b"\x89PNG"));
    }
    // eine neue Prüfung ersetzt die vorige (deren Dokument wird freigegeben)
    let g = open(&core, &verify_file("pdfas-signed.pdf")).await;
    assert_ne!(g.doc.as_ref().unwrap().id, verify_doc);
    assert_eq!(core.open_document_count(), open_docs + 1);
    assert!(core.page_tile(verify_doc, 0, 0.5).is_err(), "altes Prüfdokument freigegeben");
    core.dispatch(UiEvent::CloseVerify);
    let v = wait_for(&core, "geschlossen", |v| v.verify.is_none()).await;
    assert_eq!(core.open_document_count(), open_docs);
    assert_eq!(v.document.unwrap().doc.id, doc_before.doc.id);
}

/// Während des Signierens kein Prüfmodus (Hinweis `ng-verify-busy`), keine Sitzung durch Prüfereignisse.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_is_busy_while_signing() {
    let fx = Fixture::new();
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "Antrag.pdf");
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let legacy = fx.dir("home/.pdf-over");
    std::fs::write(legacy.join("PDF-Over.config"), "BKU=KS\nSIGNATURE_PROFILE=AMTSSIGNATURBLOCK\n").unwrap();
    let core = start(&fx, Overlay { input_files: vec![a], output: Some(fx.dir("out")), ..Overlay::default() }, |o| {
        o.backends = Some(backends);
        o.legacy_dir = Some(legacy);
    });
    wait_for(&core, "Arbeitsbereich", editing).await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
    let v = wait_for(&core, "Anmeldung", |v| matches!(v.step, StepView::Signing { .. })).await;
    assert!(!v.actions.verify);
    core.dispatch(UiEvent::OpenVerify { path: verify_file("pdfas-signed.pdf") });
    let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-verify-busy")).await;
    assert!(v.verify.is_none() && matches!(v.step, StepView::Signing { .. }));
    core.dispatch(UiEvent::SignVerified);
    core.dispatch(UiEvent::CloseVerify);
    // auch kein erneutes Übernahmeangebot mitten im Signieren (es tauschte Einstellungen und Signiermittel)
    core.dispatch(UiEvent::OfferMigration);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let v = core.view();
    assert!(v.migration.is_none() && matches!(v.step, StepView::Signing { .. }), "{:?}", v.step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1, "nur die ausdrücklich gestartete Sitzung");
    core.dispatch(UiEvent::Cancel);
}

/// Abnahme E4 Punkt 3 [K]: „Dieses Dokument signieren“ öffnet aus der Startansicht den Arbeitsbereich mit der
/// Datei; bei offenem Dokument kommt sie in die Warteschlange (Hinweis `ng-batch-added`) – mit und ohne
/// Stapelübersicht.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sign_verified_opens_the_file_like_open_files() {
    for batch in [false, true] {
        let fx = Fixture::new();
        let file = copy_pdf("verify/pdfas-signed.pdf", &fx.dir("in"), "Kaufvertrag_signed.pdf");
        let core = start(&fx, Overlay::default(), |o| o.batch_overview = Some(batch));
        wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
        open(&core, &file).await;
        core.dispatch(UiEvent::SignVerified);
        let v = wait_for(&core, "Arbeitsbereich mit der Datei", |v| {
            editing(v) && v.document.as_ref().is_some_and(|d| d.doc.name == "Kaufvertrag_signed.pdf")
        })
        .await;
        assert!(v.verify.is_none(), "Prüfmodus geschlossen");
        assert_eq!(core.open_document_count(), 1, "Prüfdokument freigegeben, nur das Arbeitsdokument");
        assert!(!v.notices.iter().any(|n| n.code == "ng-batch-added"));

        // offenes Dokument: Warteschlange + Hinweis
        let other = copy_pdf(TEXT_PDF, &fx.dir("in"), "Gegenzeichnung.pdf");
        open(&core, &other).await;
        core.dispatch(UiEvent::SignVerified);
        let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-batch-added")).await;
        assert!(v.verify.is_none());
        assert_eq!(v.document.as_ref().unwrap().doc.name, "Kaufvertrag_signed.pdf", "Arbeitsdokument bleibt ({batch})");
        let notice = v.notices.iter().find(|n| n.code == "ng-batch-added").unwrap();
        assert_eq!(notice.params.get("count").map(String::as_str), Some("1"));
        assert!(v.actions.show_batch || !batch, "mit Übersicht: „Zur Stapelübersicht“");
    }
}

/// `actions.open_files` gilt: ohne offene Prüfung bzw. ohne Freigabe bewirkt `SignVerified` nichts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sign_verified_without_verify_view_does_nothing() {
    let fx = Fixture::new();
    let core = start(&fx, Overlay::default(), |_| {});
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::SignVerified);
    core.dispatch(UiEvent::CloseVerify);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let v = core.view();
    assert!(matches!(v.step, StepView::Open { .. }) && v.verify.is_none() && v.document.is_none());
    // Datei inzwischen gelöscht: Hinweis statt Arbeitsbereich
    let file = copy_pdf("verify/pdfas-signed.pdf", &fx.dir("in"), "weg.pdf");
    open(&core, &file).await;
    std::fs::remove_file(&file).unwrap();
    core.dispatch(UiEvent::SignVerified);
    let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-verify-open-failed")).await;
    assert!(v.verify.is_none() && v.document.is_none());
}

/// Prüfmodus auch nach dem Speichern („Fertig“) und aus der Stapelübersicht; Keystore-Ergebnis der echten
/// Engine wird als intakt erkannt (Ende-zu-Ende).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_after_signing_with_the_real_engine() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "Vertrag.pdf");
    let overlay = Overlay {
        input_files: vec![a],
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        output: Some(out.clone()),
        ..Overlay::default()
    };
    let core = start(&fx, overlay, |o| o.engine = None);
    wait_for(&core, "Arbeitsbereich", editing).await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    let v =
        wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
            .await;
    assert!(v.actions.verify, "Prüfmodus aus „Fertig“");
    let summary = match &v.step {
        StepView::Output { result, .. } => result.summary.clone().expect("Zusammenfassung"),
        _ => unreachable!(),
    };
    assert_eq!(summary.qualification, pida_backend_api::Qualification::AdES, "Testzertifikat ohne qcStatements");
    let f = open(&core, &out.join("Vertrag_signed.pdf")).await;
    assert_eq!(f.summary, VerifySummaryView::AllIntact);
    assert_eq!(one(&f).signer.as_deref(), Some(CN_RSA));
    assert!(matches!(core.view().step, StepView::Output { .. }), "Zustand „Fertig“ bleibt");
}

/// „Fertig“ nennt die Qualifikation laut Zertifikat (qcStatements des Signerzertifikats, ADR 0004 §3.7): das
/// QES-Testzertifikat (QcCompliance + QcSSCD) ergibt QES – auch mit dem Keystore, dessen Fähigkeiten AdES
/// melden; der Prüfmodus zeigt dieselbe Signatur als „qualifiziert laut Zertifikat“.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn done_summary_and_verify_name_the_qualification_from_the_certificate() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "Bescheid.pdf");
    let overlay = Overlay {
        input_files: vec![a],
        keystore_file: Some(testdata("verify/keys/test-qes-ecp256.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        output: Some(out.clone()),
        ..Overlay::default()
    };
    let core = start(&fx, overlay, |o| o.engine = None);
    wait_for(&core, "Arbeitsbereich", editing).await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    let v = wait_for(
        &core,
        "gespeichert",
        |v| matches!(&v.step, StepView::Output { result, .. } if result.summary.is_some()),
    )
    .await;
    let StepView::Output { result, .. } = &v.step else { unreachable!() };
    let summary = result.summary.clone().unwrap();
    assert_eq!(summary.qualification, pida_backend_api::Qualification::Qes);
    assert_eq!(summary.signer.as_deref(), Some("PIDA Test QES P-256 (NUR TEST)"));
    let f = open(&core, &out.join("Bescheid_signed.pdf")).await;
    assert_eq!(one(&f).qualification, QualificationClaim::Qualified { qscd: true });
    assert_eq!(f.summary, VerifySummaryView::AllIntact);
}

// ------------------------------------------------------------------------------------------------
// Zustände des echten Kerns für Prüfmodus und Ersteinrichtung (B4)
// ------------------------------------------------------------------------------------------------

/// Echte `ViewState`s für die Oberfläche (B4, ADR 0004 §6.2 Punkt 4; eigene Datei neben
/// `real-core-views.json` und `real-core-batch-views.json`): Prüfmodus je Datei aus `testdata/verify`
/// (geladen, nicht lesbar), Ersteinrichtung ohne und mit Übernahmeangebot (Liste der Übernahmen) und nach dem
/// Übernehmen. Pfade und Zeitpunkte ersetzt. Neu schreiben:
/// `PIDA_BLESS=1 cargo test -p pida-core-api --test verify real_core_tools_views`.
const UI_FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/app/src/core/fixtures/real-core-tools-views.json");

fn fixture_state(v: &ViewState, roots: &[(String, &str)]) -> serde_json::Value {
    fn walk(value: &mut serde_json::Value, roots: &[(String, &str)]) {
        match value {
            serde_json::Value::String(s) => {
                for (from, to) in roots {
                    if s.contains(from.as_str()) {
                        *s = s.replace(from.as_str(), to);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|i| walk(i, roots)),
            serde_json::Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    if matches!(key.as_str(), "signing_time" | "occurred_at" | "signed_at") && item.is_string() {
                        *item = serde_json::Value::String("2026-10-15T10:42:17+02:00".into());
                    } else {
                        walk(item, roots);
                    }
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(v).expect("ViewState serialisierbar");
    walk(&mut value, roots);
    value["language"] = serde_json::Value::String("de".into());
    value
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_core_tools_views_for_the_ui() {
    let mut states: Vec<(String, ViewState)> = Vec::new();
    let mut roots: Vec<(String, &str)> = Vec::new();
    let mut add_roots = |fx: &Fixture| {
        let tmp = fx.tmp.path().to_path_buf();
        for p in [tmp.clone(), tmp.canonicalize().unwrap_or(tmp)] {
            roots.push((p.display().to_string(), "/Users/test"));
        }
    };

    // Prüfmodus aus der Startansicht
    let fx = Fixture::new();
    add_roots(&fx);
    let core = start(&fx, Overlay::default(), |_| {});
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    let docs = fx.dir("Dokumente");
    for (state, file) in [
        ("verify_no_signatures", "unsigned.pdf"),
        ("verify_all_intact", "pdfas-signed.pdf"),
        ("verify_some_broken", "tampered.pdf"),
        ("verify_some_not_verifiable", "pkcs7-sha1.pdf"),
        ("verify_sha1_warning", "cades-sha1.pdf"),
        ("verify_two_signatures", "two-signatures.pdf"),
        ("verify_modified_after_last_signature", "modified-after-signing.pdf"),
        ("verify_invisible", "invisible.pdf"),
        ("verify_qualified", "qes.pdf"),
        ("verify_rotated", "rotated-90.pdf"),
    ] {
        let path = docs.join(file);
        std::fs::copy(verify_file(file), &path).unwrap();
        open(&core, &path).await;
        states.push((state.into(), core.view()));
    }
    let junk = docs.join("Rechnung.pdf");
    std::fs::write(&junk, b"kein PDF").unwrap();
    open(&core, &junk).await;
    states.push(("verify_failed".into(), core.view()));
    drop(core);

    // Ersteinrichtung ohne Übernahmeangebot (Neuinstallation)
    let fx = Fixture::new();
    add_roots(&fx);
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(fx.config(Overlay::default(), false), opts).unwrap();
    let v = wait_for(&core, "Ersteinrichtung", |v| v.first_run.as_ref().is_some_and(|f| f.preview_revision > 0)).await;
    states.push(("first_run_fresh".into(), v));
    drop(core);

    // Ersteinrichtung mit Übernahmeangebot, danach übernommen
    let fx = Fixture::new();
    add_roots(&fx);
    let legacy = fx.dir("home/.pdf-over");
    let out = fx.dir("home/Dokumente/Signiert");
    let emblem = fx.dir("home/Bilder").join("firmenlogo.png");
    std::fs::copy(testdata("config/legacy-home/.pdf-over/.emblem.png"), &emblem).unwrap();
    std::fs::write(
        legacy.join("PDF-Over.config"),
        format!(
            "BKU=MOBILE\nMOBILE_NUMBER=+436641234567\nOUTPUT_FOLDER={}\nSIGNATURE_PROFILE=SIGNATURBLOCK_SMALL\n\
             SIGNLOCALE=de\nEMBLEM={}\nKEYSTORE_ENABLED=true\nKEYSTORE_FILE={}\nPROXY_HOST=proxy.example.test\n\
             PROXY_PORT=3128\nUSE_PLACEHOLDER_SEARCH=true\nUSE_MARKER=true\nLOCALE=de\n",
            out.display(),
            emblem.display(),
            fx.dir("home/Dokumente").join("test-rsa2048.p12").display()
        ),
    )
    .unwrap();
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        legacy_dir: Some(legacy),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(fx.config(Overlay::default(), false), opts).unwrap();
    let v = wait_for(&core, "Schritt 1", |v| {
        v.first_run
            .as_ref()
            .is_some_and(|f| f.preview_revision > 0 && f.migration.as_ref().is_some_and(|m| m.items.len() == 8))
    })
    .await;
    let preview = v.first_run.as_ref().unwrap().preview_revision;
    states.push(("first_run_migration".into(), v));
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| {
        v.first_run.as_ref().is_some_and(|f| f.migration.is_none() && f.preview_revision > preview)
    })
    .await;
    states.push(("first_run_imported".into(), v));
    core.dispatch(UiEvent::FinishFirstRun);
    let v = wait_for(&core, "fertig", |v| v.first_run.is_none()).await;
    states.push(("first_run_done".into(), v));
    drop(core);

    let fixture: serde_json::Map<String, serde_json::Value> =
        states.iter().map(|(name, v)| (name.clone(), fixture_state(v, &roots))).collect();
    if std::env::var_os("PIDA_BLESS").is_some() {
        let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
        std::fs::write(UI_FIXTURE, text).expect("Fixture schreiben");
        return;
    }
    let text = std::fs::read_to_string(UI_FIXTURE).expect("Fixture lesen (PIDA_BLESS=1 erzeugt sie)");
    let stored: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    for name in fixture.keys() {
        assert!(stored.contains_key(name), "real-core-tools-views.json ohne {name}: PIDA_BLESS=1 neu erzeugen");
    }
    for (name, value) in stored {
        let _: ViewState = serde_json::from_value(value)
            .unwrap_or_else(|e| panic!("{name}: real-core-tools-views.json passt nicht mehr zum Vertrag ({e})"));
    }
}

/// F-54/A1: Weder Prüfmodus noch „Dieses Dokument signieren“ noch die Ersteinrichtung (Übernahme mit
/// `BKU=MOBILE`, Standard-Signiermittel ID Austria, automatische Position) öffnen eine Sitzung – nur
/// „Signieren“ tut das.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_and_first_run_events_never_open_a_session() {
    // Bestandsnutzer mit ID Austria als Standard und automatischer Position
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n[backend]\ndefault = \"id_austria\"\n");
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        backends: Some(backends),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(fx.config(Overlay::default(), false), opts).expect("Kern");
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    let file = copy_pdf("verify/pdfas-signed.pdf", &fx.dir("in"), "Vertrag_signed.pdf");
    open(&core, &file).await;
    core.dispatch(UiEvent::SignVerified);
    wait_for(&core, "Arbeitsbereich", editing).await;
    open(&core, &verify_file("two-signatures.pdf")).await;
    core.dispatch(UiEvent::SignVerified);
    core.dispatch(UiEvent::CloseVerify);
    core.dispatch(UiEvent::OfferMigration);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(editing(&core.view()), "{:?}", core.view().step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung ohne „Signieren“");
    drop(core);

    // Ersteinrichtung: Übernahme (ID Austria als Standard), Schritte 2/3, „Fertig“ – mit offenem Dokument
    let fx = Fixture::new();
    let legacy = fx.dir("home/.pdf-over");
    std::fs::write(
        legacy.join("PDF-Over.config"),
        "BKU=MOBILE\nMOBILE_NUMBER=+436641234567\nSIGNATURE_POSITION=auto\n",
    )
    .unwrap();
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "Antrag.pdf");
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        backends: Some(backends),
        legacy_dir: Some(legacy),
        ..CoreOptions::isolated()
    };
    let core =
        pida_core_api::Core::new_with(fx.config(Overlay { input_files: vec![a], ..Overlay::default() }, false), opts)
            .expect("Kern");
    wait_for(&core, "Schritt 1", |v| editing(v) && v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    assert_eq!(v.first_run.unwrap().default_backend, Some(pida_config::BackendChoice::IdAustria));
    core.dispatch(UiEvent::UpdateFirstRun {
        default_backend: Some(pida_config::BackendChoice::IdAustria),
        profile: pida_config::SignatureProfile::SignaturblockSmall,
        block_language: None,
    });
    core.dispatch(UiEvent::FinishFirstRun);
    wait_for(&core, "fertig", |v| v.first_run.is_none()).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(editing(&core.view()), "{:?}", core.view().step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "Ersteinrichtung startet nie eine Sitzung");
}

/// Merge E4: Liegt der Prüfmodus über dem Arbeitsbereich bzw. der Stapelübersicht, ist das Dokument darunter
/// verdeckt – „Signieren“/„Alle signieren“ sind gesperrt (`actions`) und wirken auch als Ereignis nicht (F-54:
/// keine Sitzung für ein Dokument, das die Nutzerin gerade nicht sieht). Nach dem Verlassen gilt wieder alles.
/// „Öffnen“ (Dialog, Ablegen) zeigt die Datei im Arbeitsbereich und schließt dafür den Prüfmodus.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_mode_over_the_workspace_blocks_signing_the_hidden_document() {
    let fx = Fixture::new();
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "Antrag.pdf");
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let core = start(&fx, Overlay { input_files: vec![a], output: Some(fx.dir("out")), ..Overlay::default() }, |o| {
        o.backends = Some(backends);
    });
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    assert!(v.actions.sign && v.actions.edit_placement && v.actions.verify);
    open(&core, &verify_file("pdfas-signed.pdf")).await;
    let v = core.view();
    assert!(editing(&v), "Zustand bleibt");
    assert!(!v.actions.sign && !v.actions.edit_placement && !v.actions.start_batch && !v.actions.show_batch);
    assert!(v.actions.open_files && v.actions.verify, "„Dieses Dokument signieren“, „Anderes Dokument prüfen“");
    for e in [
        UiEvent::SignWith { backend: BackendId::AtrustNative },
        UiEvent::ConfirmPosition,
        UiEvent::ChooseInvisible,
        UiEvent::StartBatch {
            backend: BackendId::AtrustNative,
            docs: Vec::new(),
            output_dir: None,
            on_existing: pida_workflow::ExistingPolicy::KeepBoth,
        },
    ] {
        core.dispatch(e);
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let v = core.view();
    assert!(editing(&v) && v.verify.is_some(), "{:?}", v.step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung unter dem Prüfmodus");
    core.dispatch(UiEvent::CloseVerify);
    let v = wait_for(&core, "geschlossen", |v| v.verify.is_none()).await;
    assert!(v.actions.sign && v.actions.edit_placement);

    // „Öffnen“ bei offenem Prüfmodus: die Datei erscheint (Warteschlange), der Prüfmodus schließt
    open(&core, &verify_file("two-signatures.pdf")).await;
    let docs = core.open_document_count();
    let b = copy_pdf(TEXT_PDF, &fx.dir("in"), "Beilage.pdf");
    core.dispatch(UiEvent::OpenFiles { paths: vec![b], mode: None });
    let v = wait_for(&core, "Prüfmodus geschlossen", |v| v.verify.is_none()).await;
    assert!(editing(&v));
    assert!(core.open_document_count() < docs + 1, "Prüfdokument freigegeben");
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0);
}
