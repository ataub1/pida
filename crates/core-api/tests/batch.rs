//! Stapel gegen den echten Kern (ADR 0004 §3.8, Paket K2; Abnahme E2, Kriterien [K] 1–7 und 9 – Punkt 8 in
//! `tests/mobile.rs`): Übersicht mit Voranalyse (Miniatur, Seiten, Größe, Status inkl. abgelehnt und
//! QR-Platzhalter), Schätzung, „Alle signieren“ mit **einer** Sitzung (eine Anmeldung, höchstens ein
//! Zertifikat, je Dokument eine Bestätigung mit eigenem Vergleichswert), Überspringen ≠ Abbrechen, Pausen je
//! Fehlerart, Zusammenfassung, Einzeldokument öffnen/zurück, vorhandene Zieldateien, D-06 ohne Übersicht.
//!
//! Das A-Trust-Verhalten bildet ein Fake-Backend nach (eine Anmeldung je Sitzung, Zugangsdaten nur in der
//! Sitzung, Abbruch der Sitzung löscht sie, je Dokument eine Bestätigung mit Vergleichswert, Fehler je Auftrag
//! nach Skript). Mit dem Keystore läuft die echte Engine; pyHanko prüft die Ergebnisse. PDFium-Fälle
//! (Miniatur, QR-Platzhalter) laufen nur mit `PIDA_PDFIUM_DIR`.

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use common::*;
use pida_backend_api::{
    BackendError, BackendEvent, BackendId, BackendSession, Capabilities, CertChain, Environment, FormAnswer,
    FormContext, InteractionBroker, InteractionError, NativeForm, SessionContext, SignJob, SignResult,
    SignatureBackend,
};
use pida_backend_keystore::LoadedKey;
use pida_cades::{CmsProfile, DigestAlg, RawSigner, SigningTime};
use pida_config::Overlay;
use pida_core_api::{
    BatchItemStatusView, BatchView, CertificateSource, Core, CoreOptions, FormAnswerView, InteractionView,
    OutputModeView, PauseScope, PositionSummaryView, SigningProgressView, SigningStage, StepView, UiEvent, ViewState,
};
use pida_workflow::{DocId, ExistingPolicy, ExitCode};
use zeroize::Zeroizing;

const PDF: &str = "pdf/generated/simple-xref-table.pdf";
const TEXT_PDF: &str = "pdf/pdfas/pdf-as-legacy/simple.pdf";
const PASSWORD_PDF: &str = "pdf/generated/simple-aes256-userpw.pdf";
const QR_PDF: &str = "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf";

fn pdfium_dir() -> Option<PathBuf> {
    std::env::var_os("PIDA_PDFIUM_DIR").filter(|d| !d.is_empty()).map(Into::into)
}

fn tool(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok_and(|o| o.status.success())
}

// ------------------------------------------------------------------------------------------------
// ID-Austria-artiges Fake-Backend mit Skript
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
struct Atrust {
    key: Arc<LoadedKey>,
    opened: AtomicUsize,
    /// `cancel()` der Sitzung (Abbruch, F-72: Zugangsdaten gelöscht)
    cancelled: AtomicUsize,
    /// `certificate()` (InfoboxRead)
    certificates: AtomicUsize,
    /// gezeigte Login-Masken
    logins: AtomicUsize,
    /// Aufträge (`sign`, über die Standard-Implementierung von `sign_one`)
    signs: AtomicUsize,
    /// Fehler des n-ten Auftrags (1-basiert)
    failures: Mutex<HashMap<usize, BackendError>>,
    /// je Auftrag: Dokumentname und Vergleichswert der Bestätigung
    confirmations: Mutex<Vec<(String, String)>>,
}

impl Atrust {
    fn new(failures: Vec<(usize, BackendError)>) -> Arc<Atrust> {
        Arc::new(Atrust {
            key: test_key(),
            opened: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            certificates: AtomicUsize::new(0),
            logins: AtomicUsize::new(0),
            signs: AtomicUsize::new(0),
            failures: Mutex::new(failures.into_iter().collect()),
            confirmations: Mutex::new(Vec::new()),
        })
    }

    fn backends(self: &Arc<Self>) -> Option<Vec<Arc<dyn SignatureBackend>>> {
        Some(vec![Arc::new(AtrustBackend(self.clone()))])
    }

    fn n(&self, c: &AtomicUsize) -> usize {
        c.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
struct AtrustBackend(Arc<Atrust>);

#[async_trait]
impl SignatureBackend for AtrustBackend {
    fn id(&self) -> BackendId {
        BackendId::AtrustNative
    }
    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        (!env.headless).then(atrust_caps)
    }
    async fn open(
        &self,
        _ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        self.0.opened.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(AtrustFakeSession { backend: self.0.clone(), ui, logged_in: false }))
    }
}

struct AtrustFakeSession {
    backend: Arc<Atrust>,
    ui: Arc<dyn InteractionBroker>,
    /// Zugangsdaten der Sitzung (wie `AtrustSession::credentials`)
    logged_in: bool,
}

fn interaction(e: InteractionError) -> BackendError {
    match e {
        InteractionError::Cancelled => BackendError::Cancelled,
        InteractionError::Unsupported => BackendError::Unavailable("keine Oberfläche".into()),
    }
}

impl AtrustFakeSession {
    /// eine Anmeldung je Sitzung; jede weitere Anfrage meldet automatisch an
    async fn ensure_login(&mut self) -> Result<(), BackendError> {
        if self.logged_in {
            return Ok(());
        }
        self.backend.logins.fetch_add(1, Ordering::SeqCst);
        let form = NativeForm::Login { prefill_username: None, remember: false, ctx: FormContext::default() };
        match self.ui.native_form(form).await.map_err(interaction)? {
            FormAnswer::Login { username, remember, .. } => {
                self.ui.event(BackendEvent::LoginSucceeded { username, remember });
                self.logged_in = true;
                Ok(())
            }
            _ => Err(BackendError::Protocol("unerwartet".into())),
        }
    }
}

#[async_trait]
impl BackendSession for AtrustFakeSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        self.backend.certificates.fetch_add(1, Ordering::SeqCst);
        self.ensure_login().await?;
        Ok(self.backend.key.chain().clone())
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        let n = self.backend.signs.fetch_add(1, Ordering::SeqCst) + 1;
        self.ensure_login().await?;
        let failure = self.backend.failures.lock().unwrap().remove(&n);
        if let Some(e) = failure {
            return Err(e);
        }
        let mut out = Vec::new();
        for job in jobs {
            let SignJob::Document(d) = job else { return Err(BackendError::Internal("nur Modell B".into())) };
            // je Dokument eine eigene Bestätigung mit eigenem Vergleichswert
            let reference = format!("V{n}");
            self.backend.confirmations.lock().unwrap().push((d.source.display_name().to_owned(), reference.clone()));
            let form = NativeForm::SmsTan { reference_value: reference, ctx: FormContext::default() };
            match self.ui.native_form(form).await.map_err(interaction)? {
                FormAnswer::Tan(t) if t.as_str() == "123456" => {}
                _ => return Err(BackendError::Protocol("TAN falsch".into())),
            }
            let bytes = d.source.blacked_out_bytes().map_err(|e| BackendError::Internal(e.to_string()))?;
            let r = d.source.excluded_range();
            let mut content = bytes[..r.from as usize].to_vec();
            content.extend_from_slice(&bytes[r.to as usize + 1..]);
            let digest = DigestAlg::Sha256.digest(&content);
            let cms = pida_cades::sign_detached(
                self.backend.key.as_ref() as &dyn RawSigner,
                &digest,
                CmsProfile::PdfasCompat,
                SigningTime(time::OffsetDateTime::now_utc()),
            )
            .map_err(|e| BackendError::Internal(e.to_string()))?;
            out.push(SignResult::Cms(cms));
        }
        Ok(out)
    }

    async fn cancel(&mut self) {
        // F-72: Passwort gelöscht – die nächste Sitzung fragt erneut
        self.logged_in = false;
        self.backend.cancelled.fetch_add(1, Ordering::SeqCst);
    }
}

// ------------------------------------------------------------------------------------------------
// Kern, Warten, Bedienen
// ------------------------------------------------------------------------------------------------

struct Opts {
    backends: Option<Vec<Arc<dyn SignatureBackend>>>,
    engine: Option<Arc<FakeEngine>>,
    pdfium: bool,
}

fn start(fx: &Fixture, overlay: Overlay, o: Opts) -> Option<Core> {
    let mut cfg = fx.config(overlay, false);
    if o.pdfium {
        cfg.pdfium_dir = Some(pdfium_dir()?);
    }
    let opts = CoreOptions {
        workspace_ui: Some(true),
        batch_overview: Some(true),
        backends: o.backends,
        engine: o.engine.map(|e| e as Arc<dyn pida_core_api::engine::PadesEngine>),
        use_pdfium: o.pdfium,
        ..CoreOptions::isolated()
    };
    Some(Core::new_with(cfg, opts).expect("Kern startet"))
}

fn fake(backends: Option<Vec<Arc<dyn SignatureBackend>>>) -> Opts {
    Opts { backends, engine: Some(FakeEngine::new()), pdfium: false }
}

fn overlay(inputs: Vec<PathBuf>, out: &Path) -> Overlay {
    Overlay { input_files: inputs, output: Some(out.to_path_buf()), ..Overlay::default() }
}

fn keystore_overlay(inputs: Vec<PathBuf>, out: &Path) -> Overlay {
    Overlay {
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        ..overlay(inputs, out)
    }
}

/// Dateien mit unterschiedlichem Inhalt (sonst gleiche Signatur-Eingabe) im Ordner `in`.
fn inputs(fx: &Fixture, names: &[&str]) -> Vec<PathBuf> {
    let dir = fx.dir("in");
    names
        .iter()
        .map(|n| {
            let p = copy_pdf(PDF, &dir, n);
            std::fs::write(&p, [std::fs::read(&p).unwrap().as_slice(), format!("\n%{n}\n").as_bytes()].concat())
                .unwrap();
            p
        })
        .collect()
}

fn batch(v: &ViewState) -> &BatchView {
    v.batch.as_ref().expect("Stapel")
}

fn item_status(v: &ViewState, name: &str) -> BatchItemStatusView {
    batch(v).items.iter().find(|i| i.name == name).map(|i| i.status.clone()).expect(name)
}

fn doc_of(v: &ViewState, name: &str) -> DocId {
    batch(v).items.iter().find(|i| i.name == name).map(|i| i.doc).expect(name)
}

/// Übersicht mit abgeschlossener Voranalyse.
async fn overview_ready(core: &Core, n: usize) -> ViewState {
    wait_for(core, "Übersicht voranalysiert", |v| {
        matches!(v.step, StepView::Batch)
            && v.batch.as_ref().is_some_and(|b| {
                b.items.len() == n && b.items.iter().all(|i| !matches!(i.status, BatchItemStatusView::Analyzing))
            })
    })
    .await
}

fn start_batch(
    core: &Core,
    v: &ViewState,
    backend: BackendId,
    on_existing: ExistingPolicy,
    output_dir: Option<PathBuf>,
) {
    let docs = batch(v).items.iter().map(|i| i.doc).collect();
    core.dispatch(UiEvent::StartBatch { backend, docs, output_dir, on_existing });
}

fn progress(v: &ViewState) -> Option<&SigningProgressView> {
    match &v.step {
        StepView::Signing { progress, .. } if progress.batch => Some(progress),
        _ => None,
    }
}

fn form(v: &ViewState) -> Option<(u64, &NativeForm)> {
    match &v.step {
        StepView::Signing { interaction: InteractionView::Form { request, form }, .. } => Some((*request, form)),
        _ => None,
    }
}

async fn login(core: &Core) {
    let v = wait_for(core, "Anmeldung", |v| form(v).is_some_and(|(_, f)| matches!(f, NativeForm::Login { .. }))).await;
    let (request, _) = form(&v).unwrap();
    let answer =
        FormAnswerView::Login { username: "+436641234567".into(), password: "geheim1".into(), remember: false };
    core.dispatch(UiEvent::SubmitForm { request, answer });
}

/// Wartet auf die Bestätigung (SMS-TAN) des Dokuments `index` und liefert Zustand und Vergleichswert.
async fn confirmation(core: &Core, index: u32) -> (ViewState, String) {
    let v = wait_for(core, &format!("Bestätigung {index}"), |v| {
        form(v).is_some_and(|(_, f)| matches!(f, NativeForm::SmsTan { .. }))
            && progress(v).is_some_and(|p| p.index == index)
    })
    .await;
    let Some((_, NativeForm::SmsTan { reference_value, .. })) = form(&v) else { unreachable!() };
    let r = reference_value.clone();
    (v, r)
}

fn tan(core: &Core, v: &ViewState) {
    let (request, _) = form(v).unwrap();
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
}

/// Übersicht nach dem Lauf (mit Zusammenfassung).
async fn summary(core: &Core) -> ViewState {
    wait_for(core, "Zusammenfassung", |v| {
        matches!(v.step, StepView::Batch) && v.batch.as_ref().is_some_and(|b| b.summary.is_some() && !b.running)
    })
    .await
}

async fn paused(core: &Core, scope: PauseScope) -> ViewState {
    wait_for(core, &format!("Pause {scope:?}"), |v| {
        progress(v).and_then(|p| p.pause.as_ref()).is_some_and(|p| p.scope == scope)
    })
    .await
}

fn select(core: &Core, backend: BackendId) {
    core.dispatch(UiEvent::SelectSigner { backend: Some(backend), make_default: false });
}

fn png_width(png: &[u8]) -> u32 {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    u32::from_be_bytes(png[16..20].try_into().unwrap())
}

// ------------------------------------------------------------------------------------------------
// Punkt 1: Übersicht und Schätzung
// ------------------------------------------------------------------------------------------------

/// Vier Dateien (eine kennwortgeschützt, eine mit QR-Platzhalter, Platzhaltersuche an): Miniatur, Seiten,
/// Größe, Status; Schätzung „3 Dokumente · 1 Anmeldung · 3 Bestätigungen (ggf. +1 für das Zertifikat)“.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overview_with_preanalysis_thumbnails_and_estimate() {
    let fx = Fixture::new();
    fx.write_settings("[placeholder]\nsearch = true\n");
    let input = fx.dir("in");
    let a = copy_pdf(TEXT_PDF, &input, "Vertrag.pdf");
    let locked = copy_pdf(PASSWORD_PDF, &input, "Geheim.pdf");
    let qr = copy_pdf(QR_PDF, &input, "Antrag.pdf");
    let d = copy_pdf(PDF, &input, "Beilage.pdf");
    let pdfium = pdfium_dir().is_some();
    let atrust = Atrust::new(vec![]);
    let o = Opts { backends: atrust.backends(), engine: None, pdfium };
    let core = start(&fx, overlay(vec![a, locked, qr, d], &fx.dir("out")), o).expect("Kern");
    let v = overview_ready(&core, 4).await;
    assert!(v.document.is_none(), "Übersicht ohne Einzeldokument");
    assert_eq!(item_status(&v, "Geheim.pdf"), BatchItemStatusView::Rejected { code: "error-PDFPwdProtected".into() });
    for name in ["Vertrag.pdf", "Beilage.pdf", "Antrag.pdf"] {
        let i = batch(&v).items.iter().find(|i| i.name == name).unwrap();
        assert!(i.page_count.is_some_and(|n| n > 0) && i.size_bytes.is_some_and(|n| n > 0), "{name}");
        assert_eq!(i.target_name.as_deref(), Some(format!("{}_signed.pdf", name.trim_end_matches(".pdf")).as_str()));
        assert!(!i.target_exists);
        assert_eq!(i.thumbnail, pdfium, "{name}: Miniatur nur mit PDFium");
        if pdfium {
            let png = core.thumbnail(i.doc).expect("Miniatur");
            assert_eq!(png_width(&png), 96, "Breite 96 px");
        }
    }
    if pdfium {
        assert_eq!(item_status(&v, "Antrag.pdf"), BatchItemStatusView::Positioned, "QR-Platzhalter");
        let i = batch(&v).items.iter().find(|i| i.name == "Antrag.pdf").unwrap();
        assert_eq!(i.position, PositionSummaryView::Qr);
    } else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – QR-Platzhalter und Miniaturen übersprungen");
    }
    assert_eq!(item_status(&v, "Vertrag.pdf"), BatchItemStatusView::Ready);
    assert!(core.thumbnail(doc_of(&v, "Geheim.pdf")).is_err());
    assert_eq!(batch(&v).output.mode, OutputModeView::Folder);
    assert!(!batch(&v).output.needs_folder);
    assert!(v.actions.start_batch && v.actions.open_files && !v.actions.sign);
    // ohne gewähltes Signiermittel keine Schätzung
    assert!(batch(&v).estimate.is_none());
    select(&core, BackendId::AtrustNative);
    let v = wait_for(&core, "Schätzung", |v| v.batch.as_ref().is_some_and(|b| b.estimate.is_some())).await;
    let e = batch(&v).estimate.clone().unwrap();
    assert_eq!((e.documents, e.logins, e.confirmations), (3, 1, 3), "{e:?}");
    assert!(e.certificate_confirmation_possible && !e.large);
    assert_eq!(atrust.n(&atrust.opened), 0, "die Übersicht öffnet keine Sitzung (A1)");
    drop(core);

    // gültiger Eintrag im Zertifikats-Cache: ohne „+1“
    let fx = Fixture::new();
    fx.write_settings(
        "[certificate_cache]\nenabled = true\n[id_austria]\nusername = \"+436641234567\"\nremember = true\n",
    );
    let settings = pida_config::CertificateCacheSettings { enabled: true };
    let cache = pida_config::CertificateCache::new(&fx.paths, &settings);
    let key = pida_config::CertCacheKey::new(BackendId::AtrustNative.as_str(), "+436641234567");
    cache.put(&key, test_key().chain().signer.as_der()).unwrap();
    let files = inputs(&fx, &["a.pdf", "b.pdf"]);
    let core = start(&fx, overlay(files, &fx.dir("out")), fake(Atrust::new(vec![]).backends())).unwrap();
    overview_ready(&core, 2).await;
    select(&core, BackendId::AtrustNative);
    let v = wait_for(&core, "Schätzung", |v| v.batch.as_ref().is_some_and(|b| b.estimate.is_some())).await;
    let e = batch(&v).estimate.clone().unwrap();
    if test_key().chain().signer.is_valid_at(time::OffsetDateTime::now_utc()) {
        assert!(!e.certificate_confirmation_possible, "gültiger Cache-Eintrag: keine Bestätigung für das Zertifikat");
    }
    assert_eq!(e.documents, 2);
    drop(core);

    // Passwort im „Merken“-Speicher (echtes ID-Austria-Backend, ohne Sitzung): „0 Anmeldungen“
    let fx = Fixture::new();
    let files = inputs(&fx, &["a.pdf", "b.pdf"]);
    let mut o = overlay(files, &fx.dir("out"));
    o.username = Some("+436641234567".into());
    o.password = Some(Zeroizing::new("geheim1".into()));
    let core = start(&fx, o, Opts { backends: None, engine: Some(FakeEngine::new()), pdfium: false }).unwrap();
    overview_ready(&core, 2).await;
    select(&core, BackendId::AtrustNative);
    let v = wait_for(&core, "Schätzung", |v| v.batch.as_ref().is_some_and(|b| b.estimate.is_some())).await;
    assert_eq!(batch(&v).estimate.as_ref().unwrap().logins, 0, "gemerktes Passwort");
    // Keystore mit Passwort aus dem Overlay: ebenfalls ohne Anmeldung, keine Bestätigung je Dokument
    drop(core);
    let fx = Fixture::new();
    let files = inputs(&fx, &["a.pdf", "b.pdf"]);
    let core = start(&fx, keystore_overlay(files, &fx.dir("out")), fake(None)).unwrap();
    overview_ready(&core, 2).await;
    select(&core, BackendId::Keystore);
    let v = wait_for(&core, "Schätzung", |v| v.batch.as_ref().is_some_and(|b| b.estimate.is_some())).await;
    let e = batch(&v).estimate.clone().unwrap();
    assert_eq!((e.logins, e.confirmations, e.certificate_confirmation_possible), (0, 0, false), "{e:?}");
}

// ------------------------------------------------------------------------------------------------
// Punkt 2: eine Anmeldung, höchstens ein Zertifikat, je Dokument eine Bestätigung
// ------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_with_one_login_and_one_confirmation_per_document() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let names = ["Vertrag.pdf", "Antrag.pdf", "Beilage.pdf"];
    let atrust = Atrust::new(vec![]);
    let core = start(&fx, overlay(inputs(&fx, &names), &out), fake(atrust.backends())).unwrap();
    let v = overview_ready(&core, 3).await;
    select(&core, BackendId::AtrustNative);
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let mut references = Vec::new();
    for (k, name) in names.iter().enumerate() {
        let index = u32::try_from(k + 1).unwrap();
        let (v, reference) = confirmation(&core, index).await;
        let p = progress(&v).unwrap();
        assert_eq!((p.index, p.total, p.batch), (index, 3, true));
        assert_eq!(p.doc.as_ref().map(|d| d.name.as_str()), Some(*name), "Karte nennt das Dokument");
        assert_eq!(p.stage, SigningStage::Confirming);
        assert_eq!(p.confirmations_done, index - 1);
        assert_eq!(p.confirmations_total, 3);
        assert!(p.login_done);
        assert_eq!(p.certificate, if k == 0 { CertificateSource::Read } else { CertificateSource::Session });
        assert!(batch(&v).running && v.document.is_none(), "Karte über der Übersicht");
        assert!(matches!(item_status(&v, name), BatchItemStatusView::Active { stage: SigningStage::Confirming }));
        for later in &names[k + 1..] {
            assert_eq!(item_status(&v, later), BatchItemStatusView::Queued);
        }
        assert!(matches!(&v.step, StepView::Signing { document_leaves_device: true, .. }));
        references.push(reference);
        tan(&core, &v);
    }
    let v = summary(&core).await;
    let s = batch(&v).summary.clone().unwrap();
    assert_eq!((s.total, s.signed, s.skipped, s.rejected, s.failed), (3, 3, 0, 0, 0));
    assert_eq!(s.folder_display.as_deref(), Some(out.display().to_string().as_str()));
    assert!(
        v.notices.iter().any(|n| n.code == "ng-batch-done" && n.params["signed"] == "3" && n.params["total"] == "3")
    );
    assert_eq!(atrust.n(&atrust.opened), 1, "eine Sitzung");
    assert_eq!(atrust.n(&atrust.logins), 1, "eine Login-Maske");
    assert!(atrust.n(&atrust.certificates) <= 1, "höchstens ein InfoboxRead");
    assert_eq!(atrust.n(&atrust.signs), 3, "je Dokument ein Auftrag");
    let confirmations = atrust.confirmations.lock().unwrap().clone();
    assert_eq!(
        confirmations.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        names,
        "eigene Bestätigung je Dokument"
    );
    references.dedup();
    assert_eq!(references.len(), 3, "eigener Vergleichswert je Dokument");
    for name in names {
        let signed = out.join(name.replace(".pdf", "_signed.pdf"));
        assert!(signed.is_file(), "{name}");
        assert!(matches!(item_status(&v, name), BatchItemStatusView::Signed { path } if path == signed));
    }
    assert_eq!(v.done.len(), 3, "Ergebnisliste");
    assert_eq!(v.signers.selected, None, "„Jedes Mal fragen“: Auswahl nur für den Lauf (§8 F12)");
}

// ------------------------------------------------------------------------------------------------
// Punkt 3 und 5: Überspringen ≠ Abbrechen, Zusammenfassung
// ------------------------------------------------------------------------------------------------

/// Überspringen beim 2. Dokument: weiter mit dem 3. ohne neue Login-Maske; danach „2 von 4 signiert,
/// 1 übersprungen, 1 abgelehnt“ mit Speicherort.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn skip_keeps_the_login_and_the_summary_counts_all_entries() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let mut files = inputs(&fx, &["a.pdf", "b.pdf", "c.pdf"]);
    files.insert(1, copy_pdf(PASSWORD_PDF, &fx.dir("in"), "geheim.pdf"));
    let atrust = Atrust::new(vec![]);
    let core = start(&fx, overlay(files, &out), fake(atrust.backends())).unwrap();
    let v = overview_ready(&core, 4).await;
    assert!(matches!(item_status(&v, "geheim.pdf"), BatchItemStatusView::Rejected { .. }));
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let (v, _) = confirmation(&core, 1).await;
    tan(&core, &v);
    let (v, _) = confirmation(&core, 2).await;
    assert_eq!(progress(&v).unwrap().total, 3, "abgelehnte Dokumente gehören nicht zum Lauf");
    core.dispatch(UiEvent::SkipBatchDocument);
    let (v, _) = confirmation(&core, 3).await;
    assert_eq!(item_status(&v, "b.pdf"), BatchItemStatusView::Skipped { existing: false });
    assert!(v.notices.iter().any(|n| n.code == "ng-batch-pending-app-request"), "Hinweis auf die offene App-Anfrage");
    assert_eq!(atrust.n(&atrust.logins), 1, "keine neue Login-Maske");
    assert_eq!(atrust.n(&atrust.cancelled), 0, "Überspringen bricht die Sitzung nicht ab");
    tan(&core, &v);
    let v = summary(&core).await;
    let s = batch(&v).summary.clone().unwrap();
    assert_eq!((s.total, s.signed, s.skipped, s.rejected, s.failed, s.cancelled, s.not_started), (4, 2, 1, 1, 0, 0, 0));
    assert_eq!(s.folder_display.as_deref(), Some(out.display().to_string().as_str()));
    assert!(!out.join("b_signed.pdf").exists());
    assert_eq!(atrust.n(&atrust.opened), 1);
    // „Im Ordner zeigen“ darf genau die geschriebenen Dateien zeigen (Status `Signed`)
    assert!(
        batch(&v)
            .items
            .iter()
            .any(|i| matches!(&i.status, BatchItemStatusView::Signed { path } if path == &out.join("a_signed.pdf")))
    );
}

/// „Stapel abbrechen“: Rest bleibt „Bereit“, Gespeichertes bleibt, Passwort gelöscht (F-72).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn abort_keeps_saved_documents_and_clears_the_password() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let atrust = Atrust::new(vec![]);
    let core = start(&fx, overlay(inputs(&fx, &["a.pdf", "b.pdf", "c.pdf"]), &out), fake(atrust.backends())).unwrap();
    let v = overview_ready(&core, 3).await;
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let (v, _) = confirmation(&core, 1).await;
    tan(&core, &v);
    confirmation(&core, 2).await;
    core.dispatch(UiEvent::AbortBatch);
    let v = summary(&core).await;
    assert!(matches!(item_status(&v, "a.pdf"), BatchItemStatusView::Signed { .. }));
    assert_eq!(item_status(&v, "b.pdf"), BatchItemStatusView::Cancelled);
    assert_eq!(item_status(&v, "c.pdf"), BatchItemStatusView::Ready);
    assert!(out.join("a_signed.pdf").is_file(), "Gespeichertes bleibt");
    assert!(v.notices.iter().any(|n| n.code == "ng-batch-aborted" && n.params["signed"] == "1"));
    for _ in 0..50 {
        if atrust.n(&atrust.cancelled) >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(atrust.n(&atrust.cancelled), 1, "Sitzung abgebrochen: Passwort gelöscht (F-72)");
    assert!(v.actions.start_batch, "der Rest kann erneut signiert werden");
    // neuer Lauf mit dem Rest: neue Sitzung, neue Anmeldung
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let (v, _) = confirmation(&core, 1).await;
    assert_eq!(progress(&v).unwrap().total, 1, "nur „Bereit“");
    assert_eq!(atrust.n(&atrust.logins), 2);
}

// ------------------------------------------------------------------------------------------------
// Punkt 4: Pausen
// ------------------------------------------------------------------------------------------------

/// Netzfehler nach der Anmeldung → Pause (Verbindung) → „Erneut versuchen“ in derselben Sitzung ohne neue
/// Login-Maske.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connection_pause_resumes_in_the_same_session() {
    let fx = Fixture::new();
    let atrust = Atrust::new(vec![(2, BackendError::Network("Verbindung getrennt".into()))]);
    let core = start(&fx, overlay(inputs(&fx, &["a.pdf", "b.pdf"]), &fx.dir("out")), fake(atrust.backends())).unwrap();
    let v = overview_ready(&core, 2).await;
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let (v, _) = confirmation(&core, 1).await;
    tan(&core, &v);
    let v = paused(&core, PauseScope::Session).await;
    let p = progress(&v).unwrap();
    let pause = p.pause.clone().unwrap();
    assert_eq!((pause.code.as_str(), pause.new_session, pause.can_retry), ("backend-network", false, true));
    assert_eq!(pause.doc.as_ref().map(|d| d.name.as_str()), Some("b.pdf"));
    assert_eq!(p.stage, SigningStage::Paused);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(progress(&core.view()).and_then(|p| p.pause.as_ref()).is_some(), "nie automatisch weiter");
    core.dispatch(UiEvent::Retry);
    let (v, _) = confirmation(&core, 2).await;
    assert!(progress(&v).unwrap().pause.is_none());
    tan(&core, &v);
    summary(&core).await;
    assert_eq!(atrust.n(&atrust.opened), 1, "dieselbe Sitzung");
    assert_eq!(atrust.n(&atrust.logins), 1, "ohne neue Login-Maske");
}

/// Sitzungsfehler (`UnknownPage`) → Pause mit „neue Anmeldung möglich“; „Erneut versuchen“ öffnet eine neue
/// Sitzung.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn session_pause_opens_a_new_session_on_retry() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let atrust = Atrust::new(vec![(2, BackendError::UnknownPage { diagnostic: "x".into() })]);
    let core = start(&fx, overlay(inputs(&fx, &["a.pdf", "b.pdf"]), &out), fake(atrust.backends())).unwrap();
    let v = overview_ready(&core, 2).await;
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let (v, _) = confirmation(&core, 1).await;
    tan(&core, &v);
    let v = paused(&core, PauseScope::Session).await;
    let pause = progress(&v).unwrap().pause.clone().unwrap();
    assert!(pause.new_session && pause.code == "backend-unknown-page");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(atrust.n(&atrust.opened), 1, "keine neue Sitzung ohne „Erneut versuchen“ (A1)");
    core.dispatch(UiEvent::Retry);
    login(&core).await;
    let (v, _) = confirmation(&core, 2).await;
    assert_eq!(progress(&v).unwrap().doc.as_ref().unwrap().name, "b.pdf", "weiter beim aktuellen Dokument");
    tan(&core, &v);
    let v = summary(&core).await;
    assert_eq!(batch(&v).summary.as_ref().unwrap().signed, 2);
    assert_eq!(atrust.n(&atrust.opened), 2);
    assert_eq!(atrust.n(&atrust.logins), 2, "neue Sitzung, neue Anmeldung");
}

/// Ablehnung in der App → Pause (Dokument), nie automatisch weiter; „Erneut anfordern“ ohne neue Anmeldung.
/// Nicht wiederholbarer Dokumentfehler (`DocumentTooLarge`) → Zeile „Fehler“, der Lauf geht weiter.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn declined_pauses_the_document_and_too_large_fails_it() {
    let fx = Fixture::new();
    let atrust = Atrust::new(vec![(1, BackendError::Cancelled), (3, BackendError::DocumentTooLarge)]);
    let core = start(&fx, overlay(inputs(&fx, &["a.pdf", "b.pdf", "c.pdf"]), &fx.dir("out")), fake(atrust.backends()))
        .unwrap();
    let v = overview_ready(&core, 3).await;
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    login(&core).await;
    let v = paused(&core, PauseScope::Document).await;
    let pause = progress(&v).unwrap().pause.clone().unwrap();
    assert_eq!(pause.code, "ng-signing-declined");
    assert_eq!(pause.doc.as_ref().map(|d| d.name.as_str()), Some("a.pdf"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(progress(&core.view()).and_then(|p| p.pause.as_ref()).is_some(), "nie automatisch weiter");
    assert_eq!(atrust.n(&atrust.cancelled), 0, "Zugangsdaten bleiben bis zur Entscheidung (§8 F10)");
    core.dispatch(UiEvent::Retry);
    let (v, _) = confirmation(&core, 1).await;
    tan(&core, &v);
    // Auftrag 3 (Dokument b): zu groß → Fehler, weiter mit c
    let (v, _) = confirmation(&core, 3).await;
    assert!(
        matches!(item_status(&v, "b.pdf"), BatchItemStatusView::Failed { code, .. } if code == "backend-document-too-large")
    );
    tan(&core, &v);
    let v = summary(&core).await;
    let s = batch(&v).summary.clone().unwrap();
    assert_eq!((s.signed, s.failed), (2, 1));
    assert_eq!(atrust.n(&atrust.logins), 1);
    assert_eq!(atrust.n(&atrust.opened), 1);
}

// ------------------------------------------------------------------------------------------------
// Punkt 6: Einzeldokument aus der Übersicht
// ------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_document_from_the_overview_keeps_its_plan_and_counts_as_signed() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let engine = FakeEngine::new();
    let pdfium = pdfium_dir().is_some();
    let o = Opts { backends: None, engine: Some(engine.clone()), pdfium };
    let core = start(&fx, keystore_overlay(inputs(&fx, &["a.pdf", "b.pdf", "c.pdf"]), &out), o).unwrap();
    let v = overview_ready(&core, 3).await;
    // b öffnen, verschieben bzw. unsichtbar, „Zurück zum Stapel“ → „Positioniert“ mit diesem Plan
    let b = doc_of(&v, "b.pdf");
    core.dispatch(UiEvent::OpenBatchItem { doc: b });
    let v = wait_for(&core, "Arbeitsbereich", |v| v.document.as_ref().is_some_and(|d| d.doc.id == b)).await;
    assert_eq!(batch(&v).open_item, Some(b), "Übersicht bleibt erhalten");
    assert!(v.actions.show_batch);
    let expected = if pdfium {
        let v = wait_for(&core, "Sticker", |v| v.document.as_ref().is_some_and(|d| d.sticker.is_some())).await;
        let d = v.document.unwrap();
        core.dispatch(UiEvent::MoveBlock { page: 0, x: 60.0, y: 400.0 });
        wait_for(&core, "verschoben", |v| {
            v.document.as_ref().is_some_and(|x| x.revision > d.revision && x.placement.x == 60.0)
        })
        .await;
        PositionSummaryView::Manual { page: 0 }
    } else {
        core.dispatch(UiEvent::SetInvisible { invisible: true });
        wait_for(&core, "unsichtbar", |v| v.document.as_ref().is_some_and(|d| d.invisible)).await;
        PositionSummaryView::Invisible
    };
    core.dispatch(UiEvent::BackToBatch);
    let v = wait_for(&core, "zurück", |v| matches!(v.step, StepView::Batch)).await;
    let item = batch(&v).items.iter().find(|i| i.doc == b).unwrap();
    assert_eq!((item.status.clone(), item.position.clone()), (BatchItemStatusView::Positioned, expected));
    assert!(v.document.is_none() && batch(&v).open_item.is_none());
    // c einzeln signieren → Hinweis, zurück zur Übersicht
    let c = doc_of(&v, "c.pdf");
    core.dispatch(UiEvent::OpenBatchItem { doc: c });
    wait_for(&core, "Arbeitsbereich c", |v| {
        v.document.as_ref().is_some_and(|d| d.doc.id == c && d.phase == pida_core_api::DocumentPhase::Editing)
    })
    .await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    let v = wait_for(&core, "zurück nach dem Signieren", |v| {
        matches!(v.step, StepView::Batch)
            && v.batch.as_ref().is_some_and(|b| {
                b.items.iter().any(|i| i.doc == c && matches!(i.status, BatchItemStatusView::Signed { .. }))
            })
    })
    .await;
    assert!(v.notices.iter().any(|n| n.code == "ng-batch-item-signed" && n.params["file"] == "c.pdf"));
    assert!(out.join("c_signed.pdf").is_file());
    // Lauf für den Rest: zählt c als signiert
    start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::KeepBoth, None);
    let v = summary(&core).await;
    let s = batch(&v).summary.clone().unwrap();
    assert_eq!((s.total, s.signed), (3, 3));
    let plans = engine.plans.lock().unwrap().clone();
    assert_eq!(plans.len(), 3);
    if !pdfium {
        assert_eq!(
            plans.iter().filter(|p| matches!(p.visibility, pida_pades::Visibility::Invisible)).count(),
            1,
            "Plan von b"
        );
    }
}

// ------------------------------------------------------------------------------------------------
// Punkt 7: vorhandene Zieldateien, Ordnerwahl
// ------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn existing_targets_keep_both_skip_and_folder_choice() {
    // „Beide behalten“: (2), beim nächsten Mal (3)
    let fx = Fixture::new();
    let out = fx.dir("out");
    std::fs::write(out.join("a_signed.pdf"), b"alt").unwrap();
    for (round, expected) in [(1, "a_signed (2).pdf"), (2, "a_signed (3).pdf")] {
        let engine = FakeEngine::new();
        let names = ["a.pdf".to_owned(), format!("x{round}.pdf")];
        let files = inputs(&fx, &[names[0].as_str(), names[1].as_str()]);
        let core = start(&fx, keystore_overlay(files, &out), fake(None).with_engine(engine)).unwrap();
        let v = overview_ready(&core, 2).await;
        let a = batch(&v).items.iter().find(|i| i.name == "a.pdf").unwrap();
        assert!(a.target_exists, "Vorabprüfung");
        start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::KeepBoth, None);
        summary(&core).await;
        assert!(out.join(expected).is_file(), "{expected}");
        assert_eq!(std::fs::read(out.join("a_signed.pdf")).unwrap(), b"alt", "nie ersetzt");
    }
    // „Überspringen“: keine Bestätigung (kein `prepare`) für dieses Dokument
    let engine = FakeEngine::new();
    let core =
        start(&fx, keystore_overlay(inputs(&fx, &["a.pdf", "y.pdf"]), &out), fake(None).with_engine(engine.clone()))
            .unwrap();
    let v = overview_ready(&core, 2).await;
    start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::Skip, None);
    let v = summary(&core).await;
    assert_eq!(item_status(&v, "a.pdf"), BatchItemStatusView::Skipped { existing: true });
    assert!(matches!(item_status(&v, "y.pdf"), BatchItemStatusView::Signed { .. }));
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 1, "nur y vorbereitet");
    drop(core);
    // „Überschreiben“
    let core = start(&fx, keystore_overlay(inputs(&fx, &["a.pdf", "z.pdf"]), &out), fake(None)).unwrap();
    let v = overview_ready(&core, 2).await;
    start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::Overwrite, None);
    summary(&core).await;
    assert_ne!(std::fs::read(out.join("a_signed.pdf")).unwrap(), b"alt", "ersetzt");
    drop(core);

    // „jedes Mal fragen“ (kein Ausgabeordner): Ordnerwahl vor dem Start
    let fx = Fixture::new();
    let mut o = keystore_overlay(inputs(&fx, &["a.pdf", "b.pdf"]), &fx.dir("unused"));
    o.output = None;
    let core = start(&fx, o, fake(None)).unwrap();
    let v = overview_ready(&core, 2).await;
    assert_eq!((batch(&v).output.mode, batch(&v).output.needs_folder), (OutputModeView::Ask, true));
    start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::KeepBoth, None);
    wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-batch-output-folder-required")).await;
    assert!(matches!(core.view().step, StepView::Batch));
    let chosen = fx.dir("gewählt");
    start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::KeepBoth, Some(chosen.clone()));
    let v = summary(&core).await;
    assert!(chosen.join("a_signed.pdf").is_file() && chosen.join("b_signed.pdf").is_file());
    assert_eq!(
        batch(&v).summary.as_ref().unwrap().folder_display.as_deref(),
        Some(chosen.display().to_string().as_str())
    );
    drop(core);
    // Ausgabeordner ist ein Dateipfad (F-82): sonst landeten alle Dokumente in derselben Datei
    let fx = Fixture::new();
    let file_target = fx.dir("x").join("ergebnis.pdf");
    let core = start(&fx, keystore_overlay(inputs(&fx, &["a.pdf", "b.pdf"]), &file_target), fake(None)).unwrap();
    let v = overview_ready(&core, 2).await;
    assert!(batch(&v).output.needs_folder);
    // neben dem Original („.“)
    let fx = Fixture::new();
    fx.write_settings("[output]\nfolder = \".\"\n");
    let mut o = keystore_overlay(inputs(&fx, &["a.pdf", "b.pdf"]), &fx.dir("unused"));
    o.output = None;
    let core = start(&fx, o, fake(None)).unwrap();
    let v = overview_ready(&core, 2).await;
    assert_eq!((batch(&v).output.mode, batch(&v).output.needs_folder), (OutputModeView::NextToSource, false));
    assert_eq!(batch(&v).output.folder_display.as_deref(), Some(fx.dir("in").display().to_string().as_str()));
}

impl Opts {
    fn with_engine(mut self, engine: Arc<FakeEngine>) -> Opts {
        self.engine = Some(engine);
        self
    }
}

// ------------------------------------------------------------------------------------------------
// Punkt 9: D-06 ohne Übersicht; echte Engine mit pyHanko
// ------------------------------------------------------------------------------------------------

/// `-b KS -ks … -a -s a.pdf b.pdf` signiert mit eingeschalteter Stapelübersicht unverändert ohne Übersicht.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_arguments_with_keystore_stay_without_the_overview() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let mut o = keystore_overlay(inputs(&fx, &["a.pdf", "b.pdf"]), &out);
    o.backend = Some(pida_config::BackendChoice::Keystore);
    o.auto_position = true;
    o.skip_finish = true;
    let core = start(&fx, o, fake(None)).unwrap();
    let mut rx = core.subscribe();
    let watcher = tokio::spawn(async move {
        let mut overview = false;
        loop {
            overview |= matches!(rx.borrow_and_update().step, StepView::Batch) || rx.borrow().batch.is_some();
            if rx.changed().await.is_err() {
                return overview;
            }
        }
    });
    let code = tokio::time::timeout(Duration::from_secs(30), core.wait_finished()).await.unwrap();
    assert_eq!(code, ExitCode::Success, "{:#?}", core.view());
    assert!(out.join("a_signed.pdf").is_file() && out.join("b_signed.pdf").is_file());
    drop(core);
    assert!(!watcher.await.unwrap(), "nie eine Stapelübersicht (D-06)");
}

/// Stapel mit dem Keystore und der echten Engine: beide Ergebnisse prüft pyHanko (und qpdf).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keystore_batch_with_the_real_engine_validates_with_pyhanko() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let input = fx.dir("in");
    let files = vec![copy_pdf(TEXT_PDF, &input, "Vertrag.pdf"), copy_pdf(PDF, &input, "Beilage.pdf")];
    let pdfium = pdfium_dir().is_some();
    let o = Opts { backends: None, engine: None, pdfium };
    let core = start(&fx, keystore_overlay(files, &out), o).unwrap();
    let v = overview_ready(&core, 2).await;
    start_batch(&core, &v, BackendId::Keystore, ExistingPolicy::KeepBoth, None);
    let v = summary(&core).await;
    assert_eq!(batch(&v).summary.as_ref().unwrap().signed, 2);
    for name in ["Vertrag_signed.pdf", "Beilage_signed.pdf"] {
        let signed = out.join(name);
        assert!(signed.is_file(), "{name}");
        if tool("pyhanko") {
            let o =
                Command::new("pyhanko").args(["sign", "validate", "--no-strict-syntax"]).arg(&signed).output().unwrap();
            let text = String::from_utf8_lossy(&o.stdout);
            assert!(text.contains("INTACT") || text.contains("intact"), "pyHanko {name}: {text}");
        } else {
            eprintln!("pyHanko fehlt – Orakel übersprungen");
        }
        if tool("qpdf") {
            let o = Command::new("qpdf").arg("--check").arg(&signed).output().unwrap();
            assert!(o.status.success(), "qpdf {name}: {}", String::from_utf8_lossy(&o.stdout));
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Zustände des echten Kerns für die Stapel-Oberfläche (A2)
// ------------------------------------------------------------------------------------------------

/// Echte `ViewState`s des Stapels für die Oberfläche (A2, ADR 0004 §6.2 Punkt 4; eigene Datei neben
/// `real-core-views.json`, damit deren UI-Probe unverändert bleibt): Übersicht, Anmeldung, Bestätigung k/n,
/// Pause (Verbindung), Zusammenfassung. Neu schreiben:
/// `PIDA_BLESS=1 cargo test -p pida-core-api --test batch real_core_batch_views`.
const UI_FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/app/src/core/fixtures/real-core-batch-views.json");

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
                    if matches!(key.as_str(), "signing_time" | "occurred_at") && item.is_string() {
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
async fn real_core_batch_views_for_the_ui() {
    let fx = Fixture::new();
    let tmp = fx.tmp.path().to_path_buf();
    let roots: Vec<(String, &str)> = [tmp.clone(), tmp.canonicalize().unwrap_or(tmp)]
        .into_iter()
        .map(|p| (p.display().to_string(), "/Users/test"))
        .collect();
    let out = fx.dir("Dokumente");
    let mut files = inputs(&fx, &["Vertrag.pdf", "Antrag.pdf", "Beilage.pdf"]);
    files.insert(1, copy_pdf(PASSWORD_PDF, &fx.dir("in"), "Geheim.pdf"));
    let atrust = Atrust::new(vec![(2, BackendError::Network("Verbindung getrennt".into()))]);
    let core = start(&fx, overlay(files, &out), fake(atrust.backends())).unwrap();
    let mut states: Vec<(&str, ViewState)> = Vec::new();
    let v = overview_ready(&core, 4).await;
    states.push(("batch_overview", v.clone()));
    select(&core, BackendId::AtrustNative);
    let v = wait_for(&core, "Schätzung", |v| v.batch.as_ref().is_some_and(|b| b.estimate.is_some())).await;
    states.push(("batch_overview_estimate", v.clone()));
    start_batch(&core, &v, BackendId::AtrustNative, ExistingPolicy::KeepBoth, None);
    let v = wait_for(&core, "Anmeldung", |v| form(v).is_some_and(|(_, f)| matches!(f, NativeForm::Login { .. }))).await;
    states.push(("batch_login", v));
    login(&core).await;
    let (v, _) = confirmation(&core, 1).await;
    states.push(("batch_confirm_1", v.clone()));
    tan(&core, &v);
    let v = paused(&core, PauseScope::Session).await;
    states.push(("batch_paused_connection", v));
    core.dispatch(UiEvent::Retry);
    let (v, _) = confirmation(&core, 2).await;
    states.push(("batch_confirm_2", v.clone()));
    core.dispatch(UiEvent::SkipBatchDocument);
    let (v, _) = confirmation(&core, 3).await;
    tan(&core, &v);
    let v = summary(&core).await;
    states.push(("batch_summary", v));
    drop(core);

    let fixture: serde_json::Map<String, serde_json::Value> =
        states.iter().map(|(name, v)| ((*name).to_owned(), fixture_state(v, &roots))).collect();
    if std::env::var_os("PIDA_BLESS").is_some() {
        let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
        std::fs::write(UI_FIXTURE, text).expect("Fixture schreiben");
        return;
    }
    let text = std::fs::read_to_string(UI_FIXTURE).expect("Fixture lesen (PIDA_BLESS=1 erzeugt sie)");
    let stored: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    for name in fixture.keys() {
        assert!(stored.contains_key(name), "real-core-batch-views.json ohne {name}: PIDA_BLESS=1 neu erzeugen");
    }
    for (name, value) in stored {
        let _: ViewState = serde_json::from_value(value)
            .unwrap_or_else(|e| panic!("{name}: real-core-batch-views.json passt nicht mehr zum Vertrag ({e})"));
    }
}
