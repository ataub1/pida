//! Oberfläche „Arbeitsbereich“ gegen den echten Kern (ADR 0004 §3.2–§3.7, Paket K1): Kernpfad ohne
//! Bildschirmwechsel (Keystore, echte Engine, pyHanko), Invariante A1/F-54 (keine Sitzung ohne „Signieren
//! mit“), Startargument-Automatik (D-06), Abbruch zurück in den Arbeitsbereich, Wiederholen in derselben
//! Sitzung, Ablehnung in der App, ohne PDFium, Ziele (Felder, QR), Einschränkungen, Layout und Überdeckung
//! ohne Renderer im Ereignispfad. PDFium-Fälle laufen nur mit `PIDA_PDFIUM_DIR` (sonst übersprungen).

mod common;

use std::collections::VecDeque;
use std::path::PathBuf;
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
    CertificateSource, CoreOptions, DocumentPhase, ErrorOrigin, FormAnswerView, InteractionView, PlacementTarget,
    SigningStage, StepView, UiEvent, ViewState,
};
use pida_workflow::{ExitCode, PlacementOrigin};
use zeroize::Zeroizing;

const PDF: &str = "pdf/generated/simple-xref-table.pdf";
const TEXT_PDF: &str = "pdf/pdfas/pdf-as-legacy/simple.pdf";

fn pdfium_dir() -> Option<PathBuf> {
    std::env::var_os("PIDA_PDFIUM_DIR").filter(|d| !d.is_empty()).map(Into::into)
}

fn tool(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok_and(|o| o.status.success())
}

/// Kern im Arbeitsbereich-Modus; `pdfium` = mit PDFium (sonst `None`, wenn es fehlt).
fn start(
    fx: &Fixture,
    overlay: Overlay,
    backends: Option<Vec<Arc<dyn SignatureBackend>>>,
    engine: Option<Arc<FakeEngine>>,
    pdfium: bool,
) -> Option<pida_core_api::Core> {
    let mut cfg = fx.config(overlay, false);
    if pdfium {
        cfg.pdfium_dir = Some(pdfium_dir()?);
    }
    let opts = CoreOptions {
        workspace_ui: Some(true),
        backends,
        engine: engine.map(|e| e as Arc<dyn pida_core_api::engine::PadesEngine>),
        use_pdfium: pdfium,
        ..CoreOptions::isolated()
    };
    Some(pida_core_api::Core::new_with(cfg, opts).expect("Kern startet"))
}

fn keystore_overlay(inputs: Vec<PathBuf>, out: &std::path::Path) -> Overlay {
    Overlay {
        input_files: inputs,
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        output: Some(out.to_path_buf()),
        ..Overlay::default()
    }
}

fn editing(v: &ViewState) -> bool {
    matches!(v.step, StepView::Position { .. })
        && v.document.as_ref().is_some_and(|d| d.phase == DocumentPhase::Editing)
}

fn saved(v: &ViewState) -> bool {
    matches!(&v.step, StepView::Output { result, .. } if result.path.is_some())
}

/// Sammelt alle beobachteten Zustände, bis `pred` gilt (höchstens 30 s).
async fn collect_until(core: &pida_core_api::Core, what: &str, pred: impl Fn(&ViewState) -> bool) -> Vec<ViewState> {
    let mut rx = core.subscribe();
    let mut seen = Vec::new();
    let r = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let v = rx.borrow_and_update().clone();
            let done = pred(&v);
            seen.push(v);
            if done {
                return;
            }
            if rx.changed().await.is_err() {
                panic!("Kern beendet");
            }
        }
    })
    .await;
    if r.is_err() {
        panic!("Zeitüberschreitung beim Warten auf {what}: {:#?}", core.view());
    }
    seen
}

async fn form_request(core: &pida_core_api::Core, what: &str, pred: impl Fn(&NativeForm) -> bool) -> u64 {
    let v = wait_for(
        core,
        what,
        |v| matches!(&v.step, StepView::Signing { interaction: InteractionView::Form { form, .. }, .. } if pred(form)),
    )
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    request
}

async fn login(core: &pida_core_api::Core) {
    let request = form_request(core, "Anmeldung", |f| matches!(f, NativeForm::Login { .. })).await;
    let answer =
        FormAnswerView::Login { username: "+436641234567".into(), password: "geheim1".into(), remember: false };
    core.dispatch(UiEvent::SubmitForm { request, answer });
}

async fn tan(core: &pida_core_api::Core) {
    let request = form_request(core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
}

// ------------------------------------------------------------------------------------------------
// ID-Austria-artiges Fake-Backend mit Skript: Anmeldung einmal je Sitzung, Fehler je `sign`-Aufruf
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
struct Scripted {
    key: Arc<LoadedKey>,
    opened: AtomicUsize,
    cancelled: AtomicUsize,
    certificates: AtomicUsize,
    logins: AtomicUsize,
    signs: AtomicUsize,
    /// Fehler der nächsten `sign`-Aufrufe (danach Erfolg)
    failures: Mutex<VecDeque<BackendError>>,
}

impl Scripted {
    fn new(failures: Vec<BackendError>) -> Arc<Scripted> {
        Arc::new(Scripted {
            key: test_key(),
            opened: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            certificates: AtomicUsize::new(0),
            logins: AtomicUsize::new(0),
            signs: AtomicUsize::new(0),
            failures: Mutex::new(failures.into()),
        })
    }
}

#[derive(Debug)]
struct ScriptedBackend(Arc<Scripted>);

#[async_trait]
impl SignatureBackend for ScriptedBackend {
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
        Ok(Box::new(ScriptedSession { backend: self.0.clone(), ui, logged_in: false }))
    }
}

struct ScriptedSession {
    backend: Arc<Scripted>,
    ui: Arc<dyn InteractionBroker>,
    logged_in: bool,
}

fn interaction(e: InteractionError) -> BackendError {
    match e {
        InteractionError::Cancelled => BackendError::Cancelled,
        InteractionError::Unsupported => BackendError::Unavailable("keine Oberfläche".into()),
    }
}

impl ScriptedSession {
    /// wie A-Trust: eine Anmeldung je Sitzung (Zugangsdaten bleiben in der Sitzung)
    async fn ensure_login(&mut self) -> Result<(), BackendError> {
        if self.logged_in {
            return Ok(());
        }
        let form = NativeForm::Login { prefill_username: None, remember: false, ctx: FormContext::default() };
        match self.ui.native_form(form).await.map_err(interaction)? {
            FormAnswer::Login { username, remember, .. } => {
                self.backend.logins.fetch_add(1, Ordering::SeqCst);
                self.ui.event(BackendEvent::LoginSucceeded { username, remember });
                self.logged_in = true;
                Ok(())
            }
            _ => Err(BackendError::Protocol("unerwartet".into())),
        }
    }
}

#[async_trait]
impl BackendSession for ScriptedSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        self.backend.certificates.fetch_add(1, Ordering::SeqCst);
        self.ensure_login().await?;
        Ok(self.backend.key.chain().clone())
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        self.backend.signs.fetch_add(1, Ordering::SeqCst);
        self.ensure_login().await?;
        let failure = self.backend.failures.lock().unwrap().pop_front();
        if let Some(e) = failure {
            return Err(e);
        }
        let form = NativeForm::SmsTan { reference_value: "AB12".into(), ctx: FormContext::default() };
        match self.ui.native_form(form).await.map_err(interaction)? {
            FormAnswer::Tan(t) if t.as_str() == "123456" => {}
            _ => return Err(BackendError::Protocol("TAN falsch".into())),
        }
        let mut out = Vec::new();
        for job in jobs {
            let SignJob::Document(d) = job else { return Err(BackendError::Internal("nur Modell B".into())) };
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
        self.logged_in = false;
        self.backend.cancelled.fetch_add(1, Ordering::SeqCst);
    }
}

fn scripted(failures: Vec<BackendError>) -> (Arc<Scripted>, Option<Vec<Arc<dyn SignatureBackend>>>) {
    let s = Scripted::new(failures);
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![Arc::new(ScriptedBackend(s.clone()))];
    (s, Some(backends))
}

// ------------------------------------------------------------------------------------------------
// Kernpfad (Abnahme E1 Punkt 1, 7)
// ------------------------------------------------------------------------------------------------

/// Datei → Arbeitsbereich (Startlage = PDF-AS-Automatik wie `-a`) → „Signieren mit Keystore“ → Fertig, ohne
/// dass die Dokumentansicht wegfällt; echte Engine, pyHanko validiert, Zusammenfassung mit Unterzeichner.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keystore_core_path_without_screen_change() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(TEXT_PDF, &input, "doc.pdf");
    let pdfium = pdfium_dir().is_some();
    let core = start(&fx, keystore_overlay(vec![a.clone()], &out), None, None, pdfium).expect("Kern");
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.as_ref().unwrap();
    assert_eq!((d.origin, d.target, d.invisible), (PlacementOrigin::Auto, PlacementTarget::Free, false));
    assert!(d.size_bytes > 0 && d.doc.page_count > 0);
    assert!(v.signers.options.iter().any(|s| s.backend == BackendId::Keystore && s.available));
    assert!(v.actions.sign && v.actions.edit_placement);
    if pdfium {
        // Startlage = Ergebnis von `PositionChoice::Auto` (PDF-AS-Regel, wie `-a`)
        let expected = expected_auto_position(&a);
        let v = wait_for(&core, "automatische Startlage", |v| {
            v.document
                .as_ref()
                .is_some_and(|d| (d.placement.x - expected.x).abs() < 0.5 && (d.placement.y - expected.y).abs() < 0.5)
        })
        .await;
        let d = v.document.unwrap();
        let n = d.doc.page_count;
        assert_eq!(d.placement.page_index, if expected.new_page { n } else { expected.page_index });
        assert_eq!(d.origin, PlacementOrigin::Auto);
        assert!(d.sticker.is_some());
    } else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – Startlage ohne Vorschau geprüft");
        assert!(d.sticker.is_none());
    }
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    let seen = collect_until(&core, "gespeichert", saved).await;
    // die Seitenansicht bleibt eingehängt: in jedem beobachteten Zustand ein Dokument (Bearbeiten →
    // Signieren → Fertig), nie die Signiermittel-Auswahl
    for v in &seen {
        assert!(v.document.is_some(), "Dokumentansicht fehlt: {:?}", v.step);
        assert!(!matches!(v.step, StepView::Backend { .. }), "kein Auswahlschritt");
    }
    let phases: Vec<DocumentPhase> = seen.iter().filter_map(|v| v.document.as_ref().map(|d| d.phase)).collect();
    assert_eq!(phases.last(), Some(&DocumentPhase::Done));
    let v = seen.last().unwrap();
    let StepView::Output { result, .. } = &v.step else { unreachable!() };
    let summary = result.summary.as_ref().expect("Zusammenfassung");
    assert_eq!(summary.file_name, "doc_signed.pdf");
    assert_eq!(summary.folder_display.as_deref(), Some(out.display().to_string().as_str()));
    assert!(summary.signer.as_deref().is_some_and(|s| s.contains("NUR TEST")), "{:?}", summary.signer);
    assert!(summary.self_check);
    assert_eq!(summary.backend, BackendId::Keystore);
    assert!(summary.page.is_some(), "sichtbarer Block");
    assert!(time::OffsetDateTime::parse(&summary.signing_time, &time::format_description::well_known::Rfc3339).is_ok());
    let signed = out.join("doc_signed.pdf");
    assert!(signed.is_file());
    if tool("pyhanko") {
        let o = Command::new("pyhanko").args(["sign", "validate", "--no-strict-syntax"]).arg(&signed).output().unwrap();
        let text = String::from_utf8_lossy(&o.stdout);
        assert!(text.contains("INTACT") || text.contains("intact"), "pyHanko: {text}");
    } else {
        eprintln!("pyHanko fehlt – Orakel übersprungen");
    }
    if tool("qpdf") {
        let o = Command::new("qpdf").arg("--check").arg(&signed).output().unwrap();
        assert!(o.status.success(), "qpdf: {}", String::from_utf8_lossy(&o.stdout));
    }
    // „Nächstes Dokument“ bzw. Start: Dokumentansicht entfällt
    core.dispatch(UiEvent::Navigate { step: pida_workflow::Step::Open });
    let v = wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(v.document.is_none());
}

/// Ergebnis der PDF-AS-Automatik (`plan::build_plan` mit `PositionChoice::Auto`) für das Dokument.
fn expected_auto_position(path: &std::path::Path) -> pida_placement::ResolvedPosition {
    use pida_core_api::plan;
    let src = pida_pdf_cos::SourcePdf::open_file(path, &pida_pdf_cos::Limits::default()).unwrap();
    let report = pida_pades::analyze(&src).unwrap();
    let store = pida_sigblock::ProfileStore::builtin().unwrap();
    let fonts = pida_sigblock::FontProvider::builtin();
    let settings = pida_config::Settings::default();
    let overlay = Overlay::default();
    let spec = plan::block_spec(
        &store,
        plan::SpecInput {
            effective: pida_config::Effective::new(&settings, &overlay, None),
            system_language: pida_i18n::Lang::system(),
            pdfa: report.pdfa.as_ref(),
            cert: None,
            signing_time: plan::local_now(),
            emblem: None,
        },
    )
    .unwrap();
    let backend = pida_render::PdfiumBackend::bind(pdfium_dir().as_deref()).unwrap();
    let renderer = pida_render::RenderBackend::open(&backend, src.shared_bytes(), None).unwrap();
    let built = plan::build_plan(plan::PlanInput {
        report: &report,
        pages: src.pages(),
        position: &pida_workflow::PositionChoice::Auto,
        spec: &spec,
        chain: None,
        reserve: pida_pades::Reserve::ModelB,
        cms_profile: Default::default(),
        renderer: Some(renderer.as_ref()),
        store: &store,
        fonts: &fonts,
    })
    .unwrap();
    built.resolved.expect("sichtbare Position")
}

// ------------------------------------------------------------------------------------------------
// A1 / F-54 (Abnahme E1 Punkt 2, 5, 10)
// ------------------------------------------------------------------------------------------------

/// `-b MOBILE -a doc.pdf`: bleibt im Arbeitsbereich ohne Sitzung; erst „Signieren mit ID Austria“ öffnet sie.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn id_austria_start_arguments_stay_in_the_workspace() {
    let fx = Fixture::new();
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let fake = FakeAtrust::new();
    let overlay = Overlay {
        input_files: vec![a],
        backend: Some(pida_config::BackendChoice::IdAustria),
        auto_position: true,
        ..Overlay::default()
    };
    let core = start(&fx, overlay, Some(vec![fake.backend()]), Some(FakeEngine::new()), false).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung ohne „Signieren“ (A1)");
    assert!(editing(&core.view()), "{:?}", core.view().step);
    assert_eq!(v.signers.selected, Some(BackendId::AtrustNative), "Konto-Chip: Signiermittel aus -b");
    // ohne Vorschau (isolierter Kern ohne PDFium): Hinweis, kein Sticker – signiert wird automatisch platziert
    assert!(v.document.as_ref().unwrap().sticker.is_none());
    assert!(v.notices.iter().any(|n| n.code == "error-PositioningNotPossible"));
    // Altereignis ohne Auswahl bzw. mit Auswahl: `confirm_position` → „Signieren mit“ (Konto-Chip)
    core.dispatch(UiEvent::ConfirmPosition);
    let request = form_request(&core, "Anmeldung", |f| matches!(f, NativeForm::Login { .. })).await;
    assert!(request > 0);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1);
    let v = core.view();
    assert!(matches!(v.step, StepView::Signing { document_leaves_device: true, .. }));
    let StepView::Signing { progress, .. } = &v.step else { unreachable!() };
    assert_eq!(progress.stage, SigningStage::Login);
    assert_eq!(progress.backend, Some(BackendId::AtrustNative));
    assert_eq!((progress.index, progress.total, progress.batch), (1, 1, false));
    assert_eq!(v.document.as_ref().unwrap().phase, DocumentPhase::Signing);
}

/// Einstellungen mit automatischer Position und Standard-Signiermittel (ID Austria, Keystore, Bürgerkarte):
/// im Arbeitsbereich startet nie etwas ohne „Signieren“ (§8 F14) – auch nicht der Keystore (bisher F-53).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn default_signer_with_auto_position_never_starts_by_itself() {
    for default in ["id_austria", "keystore"] {
        let fx = Fixture::new();
        let input = fx.dir("in");
        let a = copy_pdf(PDF, &input, "a.pdf");
        fx.write_settings(&format!("[signature]\nauto_position = true\n[backend]\ndefault = \"{default}\"\n"));
        let fake = FakeAtrust::new();
        let engine = FakeEngine::new();
        let mut overlay = keystore_overlay(vec![a], &fx.dir("out"));
        overlay.keystore_store_password = Some(Zeroizing::new("test".into()));
        let backends: Option<Vec<Arc<dyn SignatureBackend>>> =
            if default == "id_austria" { Some(vec![fake.backend()]) } else { None };
        let core = start(&fx, overlay, backends, Some(engine.clone()), false).unwrap();
        wait_for(&core, "Arbeitsbereich", editing).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(editing(&core.view()), "{default}: {:?}", core.view().step);
        assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "{default}");
        assert_eq!(engine.prepares.load(Ordering::SeqCst), 0, "{default}: nichts vorbereitet");
        // Konto-Chip wählen startet nichts (A1)
        core.dispatch(UiEvent::SelectSigner { backend: Some(BackendId::Keystore), make_default: false });
        wait_for(&core, "Auswahl", |v| v.signers.selected == Some(BackendId::Keystore)).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(editing(&core.view()));
        assert_eq!(engine.prepares.load(Ordering::SeqCst), 0);
    }
}

/// D-06: `-b KS -ks … -a -s a.pdf b.pdf` signiert im Arbeitsbereich-Modus beide Dokumente automatisch und
/// endet (Signiermittel ohne `document_leaves_device`).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_arguments_with_keystore_sign_both_documents_and_finish() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let b = copy_pdf(PDF, &input, "b.pdf");
    let mut overlay = keystore_overlay(vec![a, b], &out);
    overlay.backend = Some(pida_config::BackendChoice::Keystore);
    overlay.auto_position = true;
    overlay.skip_finish = true;
    let core = start(&fx, overlay, None, Some(FakeEngine::new()), false).unwrap();
    let code = tokio::time::timeout(Duration::from_secs(30), core.wait_finished()).await.unwrap();
    assert_eq!(code, ExitCode::Success, "{:#?}", core.view());
    assert!(out.join("a_signed.pdf").is_file() && out.join("b_signed.pdf").is_file());
}

/// `SelectSigner { make_default }` speichert den Standard (auch „Jedes Mal fragen“) und öffnet nie eine
/// Sitzung; die Kontoanzeige ist maskiert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signer_chip_selection_and_default() {
    let fx = Fixture::new();
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    fx.write_settings("[id_austria]\nusername = \"+436641234567\"\nremember = true\n");
    let fake = FakeAtrust::new();
    let core = start(
        &fx,
        Overlay { input_files: vec![a], ..Overlay::default() },
        Some(vec![fake.backend()]),
        Some(FakeEngine::new()),
        false,
    )
    .unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    assert_eq!((v.signers.selected, v.signers.default), (None, None), "„Jedes Mal fragen“");
    let atrust = v.signers.options.iter().find(|s| s.backend == BackendId::AtrustNative).unwrap();
    assert_eq!(atrust.account.as_deref(), Some("+43 664 ···567"));
    assert!(atrust.capabilities.as_ref().is_some_and(|c| c.document_leaves_device));
    core.dispatch(UiEvent::SelectSigner { backend: Some(BackendId::AtrustNative), make_default: true });
    let v = wait_for(&core, "Standard", |v| v.signers.default == Some(BackendId::AtrustNative)).await;
    assert_eq!(v.signers.selected, Some(BackendId::AtrustNative));
    let saved = std::fs::read_to_string(fx.paths.settings_file()).unwrap();
    assert!(saved.contains("id_austria"), "{saved}");
    core.dispatch(UiEvent::SelectSigner { backend: None, make_default: true });
    wait_for(&core, "Jedes Mal fragen", |v| v.signers.default.is_none() && v.signers.selected.is_none()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "SelectSigner öffnet nie eine Sitzung");
}

/// Ohne PDFium (Abnahme E1 Punkt 10): Arbeitsbereich bedienbar, Hinweis, „Signieren“ signiert automatisch
/// platziert; ohne „Signieren mit“ keine Sitzung.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_pdfium_the_workspace_signs_automatically_placed() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let engine = FakeEngine::new();
    let core = start(&fx, keystore_overlay(vec![a], &out), None, Some(engine.clone()), false).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.as_ref().unwrap();
    assert!(d.sticker.is_none() && d.layout.is_empty());
    assert_eq!(d.check.overlaps_content, None, "ohne Renderer unbekannt");
    assert!(v.notices.iter().any(|n| n.code == "error-PositioningNotPossible"));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 0);
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    wait_for(&core, "gespeichert", saved).await;
    let plan = engine.last_plan();
    assert!(matches!(plan.visibility, pida_pades::Visibility::Visible(_)), "automatisch platziert, sichtbar");
}

// ------------------------------------------------------------------------------------------------
// Signierkarte: Abbruch, Wiederholen, Ablehnung (Abnahme E1 Punkt 6)
// ------------------------------------------------------------------------------------------------

/// Abbrechen in der Anmeldung → Arbeitsbereich mit unveränderter Platzierung und „Es wurde nichts signiert“.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_returns_to_the_workspace_unchanged() {
    let fx = Fixture::new();
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let (backend, backends) = scripted(vec![]);
    let core =
        start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }, backends, Some(FakeEngine::new()), false)
            .unwrap();
    let before = wait_for(&core, "Arbeitsbereich", editing).await.document.unwrap();
    core.dispatch(UiEvent::SetInvisible { invisible: true });
    let before_invisible = wait_for(&core, "unsichtbar", |v| v.document.as_ref().is_some_and(|d| d.invisible)).await;
    let before_invisible = before_invisible.document.unwrap();
    assert!(before_invisible.revision > before.revision, "Revision steigt mit jeder Änderung");
    core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
    // unsichtbar: kein Zertifikat nötig – Anmeldung im Signieren
    form_request(&core, "Anmeldung", |f| matches!(f, NativeForm::Login { .. })).await;
    let v = core.view();
    let StepView::Signing { progress, .. } = &v.step else { panic!() };
    assert_eq!(progress.certificate, CertificateSource::NotNeeded);
    core.dispatch(UiEvent::Cancel);
    let v = wait_for(&core, "zurück im Arbeitsbereich", |v| {
        editing(v) && v.notices.iter().any(|n| n.code == "ng-signing-cancelled")
    })
    .await;
    let after = v.document.unwrap();
    assert_eq!(
        (after.placement, after.target, after.origin, after.invisible),
        (before_invisible.placement, before_invisible.target, before_invisible.origin, true)
    );
    assert!(!matches!(v.step, StepView::Error { .. }), "eigener Abbruch ist kein Fehler");
    // `CancelSession` räumt die Sitzung asynchron auf (Passwort gelöscht, F-72)
    for _ in 0..50 {
        if backend.cancelled.load(Ordering::SeqCst) >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(backend.cancelled.load(Ordering::SeqCst) >= 1, "Sitzung abgebrochen (F-72)");
}

/// Netzfehler **nach** erfolgreicher Anmeldung: Fehlerschritt in der Karte (Herkunft, Zeitpunkt); „Erneut
/// versuchen“ in derselben Sitzung – ohne zweite Login-Maske und ohne zweites `certificate()`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn retry_after_network_error_reuses_the_session() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let (backend, backends) = scripted(vec![BackendError::Network("Verbindung getrennt".into())]);
    let overlay = Overlay { input_files: vec![a], output: Some(out.clone()), ..Overlay::default() };
    let core = start(&fx, overlay, backends, Some(FakeEngine::new()), false).unwrap();
    wait_for(&core, "Arbeitsbereich", editing).await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
    login(&core).await;
    let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
    let StepView::Error { code, retry, context, .. } = &v.step else { unreachable!() };
    assert_eq!((code.as_str(), *retry), ("backend-network", true));
    assert_eq!(context.origin, ErrorOrigin::Signing);
    assert_eq!(context.backend, Some(BackendId::AtrustNative));
    assert!(context.occurred_at.is_some() && context.doc.is_some());
    assert_eq!(v.document.as_ref().map(|d| d.phase), Some(DocumentPhase::Signing), "Karte über dem Dokument");
    core.dispatch(UiEvent::Retry);
    let v = wait_for(&core, "TAN", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::SmsTan { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { progress, .. } = &v.step else { unreachable!() };
    assert!(progress.login_done, "Anmeldung der Sitzung gilt weiter");
    assert_eq!(progress.certificate, CertificateSource::Session);
    assert_eq!(progress.stage, SigningStage::Confirming);
    tan(&core).await;
    wait_for(&core, "gespeichert", saved).await;
    assert_eq!(backend.opened.load(Ordering::SeqCst), 1, "dieselbe Sitzung");
    assert_eq!(backend.logins.load(Ordering::SeqCst), 1, "keine zweite Login-Maske");
    assert_eq!(backend.certificates.load(Ordering::SeqCst), 1, "kein zweites InfoboxRead");
    assert_eq!(backend.signs.load(Ordering::SeqCst), 2);
}

/// `Cancelled`, das PIDA nicht selbst ausgelöst hat (SL 6000/6001): Fehlerschritt `ng-signing-declined`;
/// „Erneut versuchen“ fordert in derselben Sitzung eine neue Bestätigung an; „Schließen“ verwirft sie.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn declined_in_the_app_offers_retry_in_the_same_session() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let (backend, backends) = scripted(vec![BackendError::Cancelled, BackendError::Cancelled]);
    let overlay = Overlay { input_files: vec![a], output: Some(out), ..Overlay::default() };
    let core = start(&fx, overlay, backends, Some(FakeEngine::new()), false).unwrap();
    wait_for(&core, "Arbeitsbereich", editing).await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
    login(&core).await;
    let v = wait_for(
        &core,
        "abgelehnt",
        |v| matches!(&v.step, StepView::Error { code, .. } if code == "ng-signing-declined"),
    )
    .await;
    let StepView::Error { retry, context, .. } = &v.step else { unreachable!() };
    assert!(*retry);
    assert_eq!(context.origin, ErrorOrigin::Signing);
    assert_eq!(backend.cancelled.load(Ordering::SeqCst), 0, "Sitzung (samt Zugangsdaten) bleibt bis zur Entscheidung");
    // erneut abgelehnt, dann „Schließen“ → Arbeitsbereich, Sitzung verworfen
    core.dispatch(UiEvent::Retry);
    wait_for(&core, "wieder abgelehnt", |v| {
        matches!(&v.step, StepView::Error { code, .. } if code == "ng-signing-declined")
            && backend.signs.load(Ordering::SeqCst) == 2
    })
    .await;
    assert_eq!(backend.logins.load(Ordering::SeqCst), 1, "keine neue Anmeldung");
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "Arbeitsbereich", editing).await;
    assert_eq!(backend.cancelled.load(Ordering::SeqCst), 1, "Schließen verwirft die Sitzung (F-72)");
    // neuer Versuch: neue Sitzung, neue Anmeldung, Erfolg
    core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
    login(&core).await;
    tan(&core).await;
    wait_for(&core, "gespeichert", saved).await;
    assert_eq!(backend.opened.load(Ordering::SeqCst), 2);
}

// ------------------------------------------------------------------------------------------------
// Ziele, Einschränkungen, vorhandene Signaturen (Abnahme E1 Punkt 4)
// ------------------------------------------------------------------------------------------------

/// Leere Signaturfelder werden ohne Platzhaltersuche angeboten (nie automatisch); „Ins Feld setzen“ setzt
/// den Sticker ins Feld, „Signieren“ signiert ins Feld; „Frei platzieren“ kehrt zurück.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn empty_fields_are_offered_and_used_only_on_request() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf("pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf", &fx.dir("in"), "fields.pdf");
    let engine = FakeEngine::new();
    let core = start(&fx, keystore_overlay(vec![a], &out), None, Some(engine.clone()), false).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    assert!(!d.fields.is_empty(), "Felder auch ohne placeholder.search");
    assert_eq!(d.target, PlacementTarget::Free, "nie automatisch ins Feld (§8 F2)");
    let f0 = d.fields[0].clone();
    assert!(f0.rect.is_some() && f0.selectable && f0.page.is_some());
    core.dispatch(UiEvent::UseTarget { target: PlacementTarget::Field { index: 0 } });
    let v = wait_for(&core, "im Feld", |v| {
        v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Field { index: 0 })
    })
    .await;
    let d = v.document.unwrap();
    assert_eq!(d.origin, PlacementOrigin::Field);
    assert_eq!(d.sticker, f0.rect, "Sticker = Feld-Rechteck");
    assert_eq!(d.check.overlaps_content, Some(false), "das Zielfeld ist kein Inhalt");
    assert_eq!(Some(d.placement.page_index), f0.page);
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    wait_for(&core, "gespeichert", saved).await;
    let plan = engine.last_plan();
    assert!(
        matches!(&plan.field, pida_pades::FieldTarget::Existing { qualified_name, .. } if *qualified_name == f0.name)
    );
}

/// QR-Platzhalter mit eingeschalteter Platzhaltersuche: automatisch Ziel QR (PDFium nötig).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn qr_placeholder_is_used_automatically_with_search() {
    let fx = Fixture::new();
    let a = copy_pdf("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf", &fx.dir("in"), "qr.pdf");
    fx.write_settings("[placeholder]\nsearch = true\n");
    let Some(core) = start(&fx, keystore_overlay(vec![a], &fx.dir("out")), None, Some(FakeEngine::new()), true) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    assert_eq!((d.target, d.origin), (PlacementTarget::Qr, PlacementOrigin::Qr), "{:?}", d.qr);
    let qr = d.qr.expect("QR-Platzhalter");
    assert_eq!(d.sticker, Some(qr.rect));
    assert!(!matches!(v.step, StepView::Placeholder { .. }), "keine Platzhalterfrage");
}

/// Bereits signiertes bzw. DocMDP-zertifiziertes Dokument (echte Analyse): Einschränkungen mit Begründung,
/// vorhandene Signaturen; „Signieren“ frei platziert ist auf dem zertifizierten Dokument nicht freigegeben.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn constraints_of_signed_and_certified_documents() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let signed = copy_pdf("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf", &input, "signed.pdf");
    let core = start(&fx, keystore_overlay(vec![signed], &fx.dir("out")), None, None, false).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", |v| editing(v) || matches!(v.step, StepView::Warning { .. })).await;
    let d = v.document.unwrap();
    assert!(d.doc.signed && !d.constraints.new_page_allowed);
    assert!(d.constraints.reasons.contains(&"ng-constraint-signed".to_owned()));
    assert!(!d.existing.is_empty());
    for e in &d.existing {
        assert!(!e.field_name.is_empty());
        if e.status == pida_core_api::PriorStatusView::Valid {
            assert!(e.signer.is_some() && e.signing_time.is_some(), "{e:?}");
        }
    }
    if matches!(v.step, StepView::Position { .. }) {
        core.dispatch(UiEvent::ToggleNewPage);
        wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "placement.notice.new_page_not_allowed"))
            .await;
    }

    let fx = Fixture::new();
    let cert = copy_pdf("pades/certified-docmdp-p2.pdf", &fx.dir("in"), "cert.pdf");
    let core = start(&fx, keystore_overlay(vec![cert], &fx.dir("out")), None, None, false).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", |v| {
        v.document.is_some() || matches!(v.step, StepView::Error { .. } | StepView::Warning { .. })
    })
    .await;
    if matches!(v.step, StepView::Warning { .. }) {
        core.dispatch(UiEvent::AnswerWarning { proceed: true });
    }
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    assert!(!d.constraints.new_visible_field_allowed);
    assert!(d.constraints.reasons.contains(&"ng-constraint-certified".to_owned()), "{:?}", d.constraints);
    assert!(!v.actions.sign, "frei platziert nicht zulässig");
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-constraint-certified")).await;
    assert!(editing(&core.view()));
    core.dispatch(UiEvent::SetInvisible { invisible: true });
    let v = wait_for(&core, "unsichtbar", |v| v.document.as_ref().is_some_and(|d| d.invisible)).await;
    assert!(v.actions.sign, "unsichtbar zulässig");
}

// ------------------------------------------------------------------------------------------------
// Layout, Einrastlinien, Überdeckung (Abnahme E1 Punkt 3) – Zähl-Renderer: nie PDFium im Ereignispfad
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Counts {
    forced: AtomicUsize,
    /// künstliche Dauer je Raster in ms (Prüfung „None, bis das Raster da ist“)
    delay_ms: AtomicUsize,
}

struct CountingBackend {
    inner: pida_render::PdfiumBackend,
    counts: Arc<Counts>,
}

impl pida_render::RenderBackend for CountingBackend {
    fn open(&self, data: Arc<[u8]>, password: Option<&str>) -> pida_render::Result<Box<dyn pida_render::Renderer>> {
        Ok(Box::new(CountingRenderer { inner: self.inner.open(data, password)?, counts: self.counts.clone() }))
    }
    fn describe(&self) -> String {
        self.inner.describe()
    }
}

struct CountingRenderer {
    inner: Box<dyn pida_render::Renderer>,
    counts: Arc<Counts>,
}

impl pida_render::Renderer for CountingRenderer {
    fn page_count(&self) -> usize {
        self.inner.page_count()
    }
    fn page_size(&self, page: usize) -> pida_render::Result<pida_render::PageSize> {
        self.inner.page_size(page)
    }
    fn render_page(&self, page: usize, scale: f32) -> pida_render::Result<pida_render::RgbaImage> {
        self.inner.render_page(page, scale)
    }
    fn render_forced_color(
        &self,
        page: usize,
        opts: &pida_render::ForcedColorOptions,
    ) -> pida_render::Result<pida_render::RgbaImage> {
        self.counts.forced.fetch_add(1, Ordering::SeqCst);
        let delay = self.counts.delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay as u64));
        }
        self.inner.render_forced_color(page, opts)
    }
    fn image_objects(&self, page: usize) -> pida_render::Result<Vec<pida_render::ImageObject>> {
        self.inner.image_objects(page)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn layout_and_overlap_check_read_only_the_grid() {
    let Some(dir) = pdfium_dir() else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let fx = Fixture::new();
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "doc.pdf");
    let counts = Arc::new(Counts::default());
    let backend =
        CountingBackend { inner: pida_render::PdfiumBackend::bind(Some(&dir)).unwrap(), counts: counts.clone() };
    let mut cfg = fx.config(keystore_overlay(vec![a], &fx.dir("out")), false);
    cfg.pdfium_dir = Some(dir);
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new()),
        render: Some(Arc::new(backend)),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(cfg, opts).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let page = v.document.as_ref().unwrap().placement.page_index;
    core.dispatch(UiEvent::InspectPages { pages: vec![page] });
    // Layout der Seite und Prüfung, sobald das Raster da ist
    let v = wait_for(&core, "Layout", |v| {
        v.document.as_ref().is_some_and(|d| {
            d.layout.iter().any(|l| l.page == page && l.content.is_some()) && d.check.overlaps_content.is_some()
        })
    })
    .await;
    let d = v.document.unwrap();
    let l = d.layout.iter().find(|l| l.page == page).unwrap();
    let content = l.content.unwrap();
    assert!(!l.guides.is_empty() && l.free.len() <= 16);
    assert!(l.guides.iter().any(|g| g.kind == pida_core_api::GuideKind::ContentBottom));
    assert!(d.announce.contains_key("bottom"), "Live-Region mit Unterkante");
    // warten, bis die automatische Platzierung (PDF-AS-Scan) durch ist, dann zählen
    tokio::time::sleep(Duration::from_millis(500)).await;
    let before = counts.forced.load(Ordering::SeqCst);
    // auf den Inhalt ziehen → Überdeckung; in eine freie Fläche → keine
    core.dispatch(UiEvent::MoveBlock { page, x: content.left, y: content.top });
    let v = wait_for(&core, "Überdeckung", |v| {
        v.document
            .as_ref()
            .is_some_and(|d| d.check.for_revision == d.revision && d.check.overlaps_content == Some(true))
    })
    .await;
    assert_eq!(v.document.as_ref().unwrap().origin, PlacementOrigin::User);
    let free = l.free.first().copied().expect("freie Fläche");
    core.dispatch(UiEvent::MoveBlock { page, x: free.left, y: free.top });
    wait_for(&core, "frei", |v| {
        v.document
            .as_ref()
            .is_some_and(|d| d.check.for_revision == d.revision && d.check.overlaps_content == Some(false))
    })
    .await;
    for i in 0..20 {
        core.dispatch(UiEvent::NudgeBlock { dx: if i % 2 == 0 { 1 } else { -1 }, dy: 0, fine: true, coarse: false });
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(counts.forced.load(Ordering::SeqCst), before, "kein PDFium-Aufruf im Ereignispfad");
    // „Freie Fläche suchen“: automatische Platzierung (PDF-AS-Scan), Herkunft wieder automatisch
    core.dispatch(UiEvent::AutoPlace);
    wait_for(&core, "automatisch", |v| v.document.as_ref().is_some_and(|d| d.origin == PlacementOrigin::Auto)).await;
}

/// Seite ohne fertiges Raster: `overlaps_content = None`, bis es da ist – dann gilt das Ergebnis für dieselbe
/// Revision (Raster künstlich verzögert).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overlap_is_unknown_until_the_grid_is_ready() {
    let Some(dir) = pdfium_dir() else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let fx = Fixture::new();
    let a = copy_pdf("pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf", &fx.dir("in"), "multi.pdf");
    let counts = Arc::new(Counts::default());
    counts.delay_ms.store(600, Ordering::SeqCst);
    let backend =
        CountingBackend { inner: pida_render::PdfiumBackend::bind(Some(&dir)).unwrap(), counts: counts.clone() };
    let mut cfg = fx.config(keystore_overlay(vec![a], &fx.dir("out")), false);
    cfg.pdfium_dir = Some(dir);
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new()),
        render: Some(Arc::new(backend)),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(cfg, opts).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    if d.doc.page_count < 2 {
        eprintln!("Dokument mit einer Seite – übersprungen");
        return;
    }
    let other = if d.placement.page_index == 0 { 1 } else { 0 };
    core.dispatch(UiEvent::MoveBlock { page: other, x: 40.0, y: 300.0 });
    let v =
        wait_for(&core, "verschoben", |v| v.document.as_ref().is_some_and(|d| d.placement.page_index == other)).await;
    let d = v.document.unwrap();
    assert_eq!(d.check.overlaps_content, None, "Raster der Seite noch nicht berechnet");
    let revision = d.revision;
    let v =
        wait_for(&core, "Raster", |v| v.document.as_ref().is_some_and(|d| d.check.overlaps_content.is_some())).await;
    assert_eq!(v.document.unwrap().check.for_revision, revision, "neue Projektion mit derselben Revision");
}
