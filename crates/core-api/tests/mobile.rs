//! Mobile-Lebenszyklus und Mobile-Funktionen des Kerns (Plan §3.5, §5.2; docs/integration/mobile.md):
//! App-Wechsel ist kein Abbruch und setzt den Long-Poll fort, „Signatur fortsetzen“ nach einem App-Kill
//! mit `Prepared::persist/restore`, Signiermittel je Plattform (iOS/Android nur ID Austria, ADR 0004
//! Nachtrag 11: Einstellungen, Passwörter, „Fortsetzen“ und Übernahme eines früheren Keystores), Ausgabe in
//! die App-Dokumente, Teilen/Exportieren über die Shell; PKCS#12-Import nur noch am Desktop wirksam.

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::*;
use pida_backend_api::{
    BackendError, BackendId, BackendSession, Capabilities, CertChain, Environment, FormAnswer, FormContext,
    InteractionBroker, InteractionError, NativeForm, Platform, SessionContext, SignJob, SignResult, SignatureBackend,
};
use pida_cades::{CmsProfile, DigestAlg, RawSigner, SigningTime};
use pida_config::{Overlay, PasswordStorage, SecretId, SecretStore, SecureStorage, Settings, ShellSecretStore};
use pida_core_api::{
    Core, CoreConfig, CoreOptions, FormAnswerView, InteractionView, PlatformOs, ShellHooks, StepView, UiEvent,
    ViewState,
};
use time::OffsetDateTime;
use zeroize::Zeroizing;

const PDF: &str = "pdf/generated/simple-xref-table.pdf";
const P12: &str = "keystores/test-rsa2048.p12";

fn config(fx: &Fixture, platform: Platform) -> CoreConfig {
    CoreConfig { platform, ..fx.config(Overlay::default(), false) }
}

fn mobile_options(
    engine: &Arc<FakeEngine>,
    docs: &Path,
    backends: Option<Vec<Arc<dyn SignatureBackend>>>,
) -> CoreOptions {
    CoreOptions {
        engine: Some(engine.clone()),
        backends,
        default_output_dir: Some(docs.to_path_buf()),
        ..CoreOptions::isolated()
    }
}

fn start(fx: &Fixture, platform: Platform, options: CoreOptions) -> Core {
    Core::new_with(config(fx, platform), options).expect("Kern startet")
}

fn form_request(v: &ViewState) -> Option<(u64, &NativeForm)> {
    match &v.step {
        StepView::Signing { interaction: InteractionView::Form { request, form }, .. } => Some((*request, form)),
        _ => None,
    }
}

async fn wait_form(core: &Core, what: &str, pred: fn(&NativeForm) -> bool) -> u64 {
    let v = wait_for(core, what, |v| form_request(v).is_some_and(|(_, f)| pred(f))).await;
    form_request(&v).expect("Maske").0
}

async fn login(core: &Core) {
    let request = wait_form(core, "Login", |f| matches!(f, NativeForm::Login { .. })).await;
    let answer = FormAnswerView::Login { username: "0664 1234567".into(), password: "geheim1".into(), remember: false };
    core.dispatch(UiEvent::SubmitForm { request, answer });
}

fn resume_dir(fx: &Fixture) -> PathBuf {
    fx.paths.temp_dir.join("resume")
}

// ------------------------------------------------------------------------------------------------
// Backend mit Bestätigung in der App „ID Austria“ (Long-Poll im Hintergrund)
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
struct AppConfirm {
    key: Arc<pida_backend_keystore::LoadedKey>,
    /// angezeigte Wartemasken
    shown: AtomicUsize,
    /// „weiter“ (Neuladen nach Rückkehr bzw. Poll fertig)
    continued: AtomicUsize,
    /// ab dieser Anzahl „weiter“ ist die Bestätigung bei A-Trust angekommen
    confirmed_after: usize,
}

#[derive(Debug)]
struct AppConfirmBackend(Arc<AppConfirm>);

#[async_trait]
impl SignatureBackend for AppConfirmBackend {
    fn id(&self) -> BackendId {
        BackendId::AtrustNative
    }
    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        let mut caps = atrust_caps();
        caps.document_leaves_device = false;
        (!env.headless).then_some(caps)
    }
    async fn open(
        &self,
        _ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        Ok(Box::new(AppConfirmSession { backend: self.0.clone(), ui }))
    }
}

struct AppConfirmSession {
    backend: Arc<AppConfirm>,
    ui: Arc<dyn InteractionBroker>,
}

#[async_trait]
impl BackendSession for AppConfirmSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        Ok(self.backend.key.chain().clone())
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        // wie `wait_with_poll`: Maske zeigen; „weiter“ = Seite neu laden (R7) – noch nicht bestätigt →
        // dieselbe Maske erneut
        loop {
            self.backend.shown.fetch_add(1, Ordering::SeqCst);
            let form = NativeForm::WaitForApp {
                reference_value: Some("AB12".into()),
                timeout_secs: Some(300),
                ctx: FormContext::default(),
            };
            match self.ui.native_form(form).await {
                Ok(FormAnswer::Continue) => {
                    let n = self.backend.continued.fetch_add(1, Ordering::SeqCst) + 1;
                    if n >= self.backend.confirmed_after {
                        break;
                    }
                }
                Ok(_) => return Err(BackendError::Protocol("unerwartet".into())),
                Err(InteractionError::Cancelled) => return Err(BackendError::Cancelled),
                Err(InteractionError::Unsupported) => return Err(BackendError::Unavailable("UI".into())),
            }
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
                SigningTime(OffsetDateTime::now_utc()),
            )
            .map_err(|e| BackendError::Internal(e.to_string()))?;
            out.push(SignResult::Cms(cms));
        }
        Ok(out)
    }

    async fn cancel(&mut self) {}
}

/// Rückkehr aus der App „ID Austria“: kein Abbruch; die wartende Maske wird sofort mit „weiter“
/// beantwortet (Long-Poll frisch), ohne vorherigen Hintergrund aber nicht.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn returning_from_the_id_austria_app_restarts_the_long_poll() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let docs = fx.dir("docs");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let engine = FakeEngine::new();
    let app = Arc::new(AppConfirm {
        key: test_key(),
        shown: AtomicUsize::new(0),
        continued: AtomicUsize::new(0),
        confirmed_after: 2,
    });
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![Arc::new(AppConfirmBackend(app.clone()))];
    let core = start(&fx, Platform::Ios, mobile_options(&engine, &docs, Some(backends)));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    wait_form(&core, "Warten auf die App", |f| matches!(f, NativeForm::WaitForApp { .. })).await;
    // Vordergrund ohne Hintergrund (z. B. erstes onResume): nichts
    core.dispatch(UiEvent::Foregrounded);
    // Wechsel in die App „ID Austria“: Maske bleibt, keine Abbruchwirkung
    core.dispatch(UiEvent::Backgrounded);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert!(form_request(&core.view()).is_some_and(|(_, f)| matches!(f, NativeForm::WaitForApp { .. })));
    assert_eq!(app.continued.load(Ordering::SeqCst), 0);
    assert_eq!(app.shown.load(Ordering::SeqCst), 1);
    // Rückkehr: Long-Poll neu (Seite neu geladen) – noch nicht bestätigt → erneut warten
    core.dispatch(UiEvent::Foregrounded);
    wait_for(&core, "zweite Wartemaske", |_| app.shown.load(Ordering::SeqCst) == 2).await;
    assert_eq!(app.continued.load(Ordering::SeqCst), 1);
    // erneut Hintergrund und Rückkehr: jetzt bestätigt → Signatur, Ausgabe in den App-Dokumenten
    core.dispatch(UiEvent::Backgrounded);
    core.dispatch(UiEvent::Foregrounded);
    let v =
        wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
            .await;
    let StepView::Output { result, .. } = v.step else { unreachable!() };
    let out = result.path.unwrap();
    assert_eq!(out, docs.join("a_signed.pdf"), "Mobile: App-Dokumente ohne Speichern-Dialog");
    verify_fake_output(&out, &std::fs::read(&a).unwrap());
    assert!(!resume_dir(&fx).exists(), "nach Erfolg kein persistierter Zustand");
}

// ------------------------------------------------------------------------------------------------
// „Signatur fortsetzen“
// ------------------------------------------------------------------------------------------------

/// App-Kill während der Bestätigung: Neustart bietet „Signatur fortsetzen“ an; das vorbereitete Dokument
/// wird wiederhergestellt (kein neues `prepare`), Anmeldung und Bestätigung laufen neu.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn app_kill_offers_resume_and_restores_the_prepared_document() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let docs = fx.dir("docs");
    let inbox = fx.dir("inbox");
    let a = copy_pdf(PDF, &inbox, "Vertrag.pdf");
    let b = copy_pdf(PDF, &inbox, "b.pdf");
    std::fs::write(&b, [std::fs::read(&b).unwrap().as_slice(), b"\n%b\n"].concat()).unwrap();
    let original = std::fs::read(&a).unwrap();

    // 1. Lauf: bis zur TAN (nach `prepare`), dann „vom System beendet“
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Android, mobile_options(&engine, &docs, Some(vec![fake.backend()])));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(core.view().platform.mobile);
    assert_eq!(core.view().platform.os, PlatformOs::Android);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b.clone()], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 1);
    assert!(resume_dir(&fx).join("resume.json").is_file(), "nach prepare gesichert");
    core.dispatch(UiEvent::Backgrounded);
    drop(core);
    assert!(resume_dir(&fx).join("resume.json").is_file(), "App-Kill löscht nichts");
    std::fs::remove_file(&a).unwrap();

    // 2. Lauf: Angebot im Auswahl-Screen
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Android, mobile_options(&engine, &docs, Some(vec![fake.backend()])));
    let v = wait_for(&core, "Angebot", |v| matches!(v.step, StepView::Open { .. }) && v.resume.is_some()).await;
    let offer = v.resume.unwrap();
    assert_eq!(offer.name, "Vertrag.pdf");
    assert_eq!(offer.backend, BackendId::AtrustNative);
    assert_eq!(offer.pending, 1);
    assert!(offer.age_minutes <= 1);
    // Kurzhinweis „wird an A-Trust übermittelt“ beim Angebot; erst „Fortsetzen“ öffnet die Sitzung (ADR 0003)
    assert!(offer.document_leaves_device);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "vor „Fortsetzen“ keine Sitzung");
    core.dispatch(UiEvent::ResumeSignature);
    // ohne Positionierung: Sitzung neu (Anmeldung, Passwort nie gespeichert), Zertifikat, dann wiederherstellen
    login(&core).await;
    let request = wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert_eq!(engine.restores.load(Ordering::SeqCst), 1, "Prepared::restore statt prepare");
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 0);
    assert!(core.view().resume.is_none());
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    // Ausgabe unter dem Originalnamen in den App-Dokumenten, danach das wartende Dokument
    let target = docs.join("Vertrag_signed.pdf");
    wait_for(&core, "erstes Dokument gespeichert", |v| v.done.iter().any(|d| d.name == "Vertrag.pdf")).await;
    verify_fake_output(&target, &original);
    // nächstes Dokument: wieder die Auswahl (je Dokument eine Nutzeraktion, ADR 0003)
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN für das wartende Dokument", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 1, "wartendes Dokument wird normal vorbereitet");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resume_can_be_discarded_and_expires() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let docs = fx.dir("docs");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let run_until_tan = |fx: &Fixture| {
        let engine = FakeEngine::new();
        let fake = FakeAtrust::new();
        start(fx, Platform::Ios, mobile_options(&engine, &docs, Some(vec![fake.backend()])))
    };
    // gesichert, verworfen
    let core = run_until_tan(&fx);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    drop(core);
    let core = run_until_tan(&fx);
    wait_for(&core, "Angebot", |v| v.resume.is_some()).await;
    core.dispatch(UiEvent::DiscardResume);
    wait_for(&core, "Angebot weg", |v| v.resume.is_none()).await;
    assert!(!resume_dir(&fx).exists());
    drop(core);
    // gesichert, zu alt
    let core = run_until_tan(&fx);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    drop(core);
    let record = resume_dir(&fx).join("resume.json");
    let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    json["created_unix"] = serde_json::json!(1_000_000_000u64);
    std::fs::write(&record, serde_json::to_vec(&json).unwrap()).unwrap();
    let core = run_until_tan(&fx);
    let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-resume-expired")).await;
    assert!(v.resume.is_none());
    assert!(!resume_dir(&fx).exists());
}

/// Abbruch im fortgesetzten Vorgang löscht den Zustand; ein nicht angenommenes Angebot übersteht den
/// Abbruch eines anderen Vorgangs vor `prepare`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_deletes_only_the_state_of_the_running_signature() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let docs = fx.dir("docs");
    let inbox = fx.dir("inbox");
    let a = copy_pdf(PDF, &inbox, "a.pdf");
    let b = copy_pdf(PDF, &inbox, "b.pdf");
    let new_core = || {
        start(&fx, Platform::Ios, mobile_options(&FakeEngine::new(), &docs, Some(vec![FakeAtrust::new().backend()])))
    };
    let core = new_core();
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    drop(core);
    // anderes Dokument, Abbruch in der Anmeldung (vor prepare): Angebot bleibt
    let core = new_core();
    wait_for(&core, "Angebot", |v| v.resume.is_some()).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![b.clone()], mode: None });
    choose_id_austria(&core).await;
    wait_form(&core, "Login", |f| matches!(f, NativeForm::Login { .. })).await;
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "Auswahl", |v| matches!(v.step, StepView::Backend { .. })).await;
    assert!(resume_dir(&fx).join("resume.json").is_file(), "fremdes Angebot unangetastet");
    core.dispatch(UiEvent::Navigate { step: pida_workflow::Step::Open });
    wait_for(&core, "Angebot wieder", |v| v.resume.is_some()).await;
    // fortsetzen und abbrechen → gelöscht
    core.dispatch(UiEvent::ResumeSignature);
    wait_form(&core, "Login", |f| matches!(f, NativeForm::Login { .. })).await;
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "Auswahl", |v| matches!(v.step, StepView::Backend { .. })).await;
    assert!(!resume_dir(&fx).exists());
}

/// Desktop: keine Persistenz, kein Angebot (Speichern-Dialog wie bisher).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desktop_does_not_persist_the_prepared_state() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let options = CoreOptions {
        engine: Some(FakeEngine::new()),
        backends: Some(vec![FakeAtrust::new().backend()]),
        ..CoreOptions::isolated()
    };
    let core = start(&fx, Platform::Linux, options);
    assert!(!core.view().platform.mobile);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert!(!resume_dir(&fx).exists());
}

// ------------------------------------------------------------------------------------------------
// Signiermittel je Plattform, Ausgabe, Teilen/Exportieren
// ------------------------------------------------------------------------------------------------

/// iOS/Android: nur ID Austria (ADR 0004 Nachtrag 11) – keine lokale BKU, kein Keystore (auch nicht mit
/// eingerichteter Datei), ID Austria mit Hinweis „FIDO2 nicht verfügbar“. Desktop unverändert: Chipkarte und
/// Keystore.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mobile_backend_list_has_only_id_austria_and_marks_fido2() {
    for (platform, mobile) in [(Platform::Ios, true), (Platform::Android, true), (Platform::MacOs, false)] {
        let fx = Fixture::new();
        let ks = fx.dir("ks").join("test.p12");
        std::fs::copy(testdata(P12), &ks).unwrap();
        fx.write_settings(&format!("[signature]\nauto_position = true\n[keystore]\nenabled = true\nfile = {ks:?}\n"));
        let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
        let options = CoreOptions { engine: Some(FakeEngine::new()), ..CoreOptions::isolated() };
        let core = start(&fx, platform, options);
        core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
        let v =
            wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options, .. } if !options.is_empty()))
                .await;
        let StepView::Backend { options } = v.step else { unreachable!() };
        let ids: Vec<BackendId> = options.iter().map(|o| o.id).collect();
        if mobile {
            assert_eq!(ids, vec![BackendId::AtrustNative], "{platform:?}");
            assert_eq!(options[0].notes, vec!["ng-fido2-unavailable-mobile".to_owned()]);
            // nur ID Austria, aber ohne „Signieren“ (automatische Position) kein Autostart (F-54, ADR 0003):
            // die Auswahl mit Kurzhinweis bleibt stehen – gleiche Oberfläche wie auf dem Desktop
            assert!(options[0].capabilities.as_ref().is_some_and(|c| c.document_leaves_device));
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            assert!(matches!(core.view().step, StepView::Backend { .. }), "{platform:?}: {:?}", core.view().step);
        } else {
            assert_eq!(ids, vec![BackendId::AtrustNative, BackendId::LocalBku, BackendId::Keystore], "{platform:?}");
            assert!(options.iter().all(|o| o.notes.is_empty()));
            assert!(options[2].available, "Desktop: Keystore wie bisher");
        }
    }
}

#[derive(Default)]
struct RecordingHooks {
    shared: Mutex<Vec<PathBuf>>,
    exported: Mutex<Vec<(PathBuf, String)>>,
}

impl ShellHooks for RecordingHooks {
    fn open_path(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
    fn open_url(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn share_path(&self, path: &Path) -> Result<(), String> {
        // das Share-Sheet braucht hier absichtlich länger als der Export: beide laufen je im Hintergrund
        // (`spawn_blocking`), ihre Reihenfolge ist nicht festgelegt – der Test darf sich nicht darauf verlassen
        std::thread::sleep(std::time::Duration::from_millis(150));
        self.shared.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
    fn export_path(&self, path: &Path, file_name: &str) -> Result<Option<String>, String> {
        self.exported.lock().unwrap().push((path.to_path_buf(), file_name.to_owned()));
        // wie die Dateiauswahl, wenn es den Namen am Ziel schon gibt
        Ok(Some(file_name.replace(".pdf", " (3).pdf")))
    }
}

/// Wartet, bis die Shell mindestens `n` Aufrufe aufgezeichnet hat (Teilen und Exportieren laufen je im Hintergrund;
/// der Hinweis „exportiert“ sagt nichts darüber, ob das Teilen schon bei der Shell angekommen ist).
async fn recorded<T: Clone>(what: &str, calls: &Mutex<Vec<T>>, n: usize) -> Vec<T> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let seen = calls.lock().unwrap().clone();
        if seen.len() >= n {
            return seen;
        }
        assert!(tokio::time::Instant::now() < deadline, "Zeitüberschreitung beim Warten auf {what}");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Ausgabe in die App-Dokumente; „Teilen“ und „Exportieren“ gehen an die Shell (Effekte des Kerns).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn output_goes_to_app_documents_and_can_be_shared_and_exported() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\nprofile = \"INVISIBLE\"\n");
    let docs = fx.tmp.path().join("Documents");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let hooks = Arc::new(RecordingHooks::default());
    let fake = FakeAtrust::new();
    let options = CoreOptions {
        shell: Some(hooks.clone()),
        ..mobile_options(&FakeEngine::new(), &docs, Some(vec![fake.backend()]))
    };
    let core = start(&fx, Platform::Ios, options);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    choose_id_austria(&core).await;
    // unsichtbar: ohne Zertifikat für den Block, also ohne eigene Anmeldung – gleich die TAN
    let request = wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    let v =
        wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
            .await;
    let StepView::Output { result, .. } = v.step else { unreachable!() };
    let out = result.path.unwrap();
    assert_eq!(out, docs.join("a_signed.pdf"), "Ordner wird angelegt, kein Speichern-Dialog");
    core.dispatch(UiEvent::ShareResult);
    core.dispatch(UiEvent::ExportResult);
    let v = wait_for(&core, "exportiert", |v| v.notices.iter().any(|n| n.code == "ng-export-done")).await;
    let done = v.notices.iter().find(|n| n.code == "ng-export-done").unwrap();
    assert_eq!(done.params.get("name").map(String::as_str), Some("a_signed (3).pdf"), "tatsächlicher Name");
    // Teilen und Exportieren laufen unabhängig im Hintergrund: auf beide warten, nicht auf eine Reihenfolge
    assert_eq!(recorded("Teilen", &hooks.shared, 1).await, vec![out.clone()]);
    assert_eq!(recorded("Export", &hooks.exported, 1).await, vec![(out, "a_signed.pdf".to_owned())]);
    let v = core.view();
    assert!(!v.notices.iter().any(|n| n.code == "ng-share-failed" || n.code == "ng-export-failed"), "{:?}", v.notices);
}

/// Nachtrag 12 mit Nachtrag 17 zusammengeführt (17.10.2026): Auf Mobilgeräten legt jedes „Teilen“ eine neue Kopie an.
/// Solange das Dokument offen ist, ist eine Kopie mit gleichem Namen und Inhalt ein Duplikat (Hinweis
/// `ng-open-duplicates`, kein zweites Dokument); signiert und gespeichert („Fertig“) öffnet sie wie vor Nachtrag 12 als
/// nächstes Dokument (F-89 „sofort weiter“) statt „ist bereits geöffnet“.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shared_copy_opens_again_after_signing_on_mobile() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\nprofile = \"INVISIBLE\"\n");
    let docs = fx.tmp.path().join("Documents");
    let a = copy_pdf(PDF, &fx.dir("inbox/1"), "a.pdf");
    let second = copy_pdf(PDF, &fx.dir("inbox/2"), "a.pdf");
    let third = copy_pdf(PDF, &fx.dir("inbox/3"), "a.pdf");
    let fake = FakeAtrust::new();
    let options = CoreOptions {
        workspace_ui: Some(true),
        ..mobile_options(&FakeEngine::new(), &docs, Some(vec![fake.backend()]))
    };
    let core = start(&fx, Platform::Ios, options);
    let editing = |v: &ViewState| v.document.as_ref().is_some_and(|d| d.phase == pida_core_api::DocumentPhase::Editing);
    let duplicates = |v: &ViewState| v.notices.iter().any(|n| n.code == "ng-open-duplicates");
    core.dispatch(UiEvent::FilesReceived { paths: vec![a] });
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let first = v.document.as_ref().map(|d| d.doc.id);
    // offen: die Kopie kommt nicht dazu
    core.dispatch(UiEvent::FilesReceived { paths: vec![second] });
    let v = wait_for(&core, "Duplikat", duplicates).await;
    assert!(editing(&v) && v.batch.is_none() && !v.actions.show_batch, "kein zweites Dokument");
    let dup = v.notices.iter().find(|n| n.code == "ng-open-duplicates").unwrap();
    assert_eq!(dup.params.get("file").map(String::as_str), Some("a.pdf"));
    for n in v.notices.iter().filter(|n| n.code == "ng-open-duplicates") {
        core.dispatch(UiEvent::DismissNotice { id: n.id });
    }
    // signiert und gespeichert: die nächste Kopie öffnet als nächstes Dokument
    core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
    let request = wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    core.dispatch(UiEvent::FilesReceived { paths: vec![third] });
    let v =
        wait_for(&core, "nächstes Dokument", |v| editing(v) && v.document.as_ref().map(|d| d.doc.id) != first).await;
    assert!(!duplicates(&v), "nicht „ist bereits geöffnet“");
}

// ------------------------------------------------------------------------------------------------
// Mobile nur mit ID Austria (Entscheidung des Auftraggebers 16.10.2026, ADR 0004 Nachtrag 11)
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct FakeSecureStorage(Mutex<std::collections::HashMap<String, String>>);

impl SecureStorage for FakeSecureStorage {
    fn get(&self, key: &str) -> Result<Option<Zeroizing<String>>, String> {
        Ok(self.0.lock().unwrap().get(key).cloned().map(Zeroizing::new))
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
    fn is_available(&self) -> bool {
        true
    }
}

/// Sicherer Speicher wie in der Mobile-Shell (`ShellSecretStore` auf einem Schlüssel-Wert-Speicher).
fn shell_secrets() -> (Arc<FakeSecureStorage>, Arc<dyn SecretStore>) {
    let storage = Arc::new(FakeSecureStorage::default());
    let secrets: Arc<dyn SecretStore> = Arc::new(ShellSecretStore::new(storage.clone()));
    (storage, secrets)
}

/// Einstellungen eines früheren Mobile-Builds: importierter Keystore (Datei in `data_dir/keystores`), als
/// Standard-Signiermittel, Passwörter im sicheren Speicher; daneben ein Proxy-Passwort und ein fremder Eintrag.
fn earlier_mobile_build(fx: &Fixture, storage: &FakeSecureStorage, secrets: &dyn SecretStore) -> PathBuf {
    let keystores = fx.paths.data_dir.join("keystores");
    std::fs::create_dir_all(&keystores).unwrap();
    let ks = keystores.join("test.p12");
    std::fs::copy(testdata(P12), &ks).unwrap();
    fx.write_settings(&format!(
        "[backend]\ndefault = \"keystore\"\n[keystore]\nenabled = true\nfile = {ks:?}\nalias = \"max\"\n\
         declared_type = \"PKCS12\"\npassword_storage = \"keychain\"\n"
    ));
    secrets.set(&SecretId::keystore_store(&ks), "test").unwrap();
    secrets.set(&SecretId::keystore_key(&ks, "max"), "test").unwrap();
    secrets.set(&SecretId::keystore_key(&ks, ""), "test").unwrap();
    // Schlüsselpasswort zu einem früher gewählten Schlüssel (die Einstellungen nennen inzwischen „max“)
    secrets.set(&SecretId::keystore_key(&ks, &file_alias(&ks)), "test").unwrap();
    secrets.set(&SecretId::proxy("proxy.example.test", "u"), "proxy-geheim").unwrap();
    storage.set("ANDERE-APP/keystore-store:/x.p12", "fremd").unwrap();
    ks
}

/// Alias des Schlüssels in der Test-Datei (weder „max“ noch leer).
fn file_alias(ks: &Path) -> String {
    let entries = pida_backend_keystore::list_aliases(&std::fs::read(ks).unwrap(), "test").unwrap();
    let alias = entries.into_iter().find(|e| e.is_key_entry).expect("Schlüssel").alias;
    assert!(!alias.is_empty() && alias != "max", "{alias}");
    alias
}

fn keystore_entries(storage: &FakeSecureStorage) -> Vec<String> {
    let mut keys: Vec<String> =
        storage.0.lock().unwrap().keys().filter(|k| k.starts_with("PIDA/keystore-")).cloned().collect();
    keys.sort();
    keys
}

fn workspace_options(secrets: &Arc<dyn SecretStore>, docs: &Path) -> CoreOptions {
    CoreOptions {
        secret_store: Some(secrets.clone()),
        workspace_ui: Some(true),
        batch_overview: Some(true),
        engine: Some(FakeEngine::new()),
        default_output_dir: Some(docs.to_path_buf()),
        ..CoreOptions::isolated()
    }
}

fn file_settings(fx: &Fixture) -> Settings {
    Settings::from_toml(&std::fs::read_to_string(fx.paths.settings_file()).unwrap()).unwrap()
}

/// Einstellungen eines früheren Mobile-Builds mit Keystore als Standard (ADR 0004 Nachtrag 11): der Kern
/// registriert keinen Keystore, bietet nur ID Austria an (Konto-Chip ohne „Keystore einrichten“, Standard ID
/// Austria), die Einstellungsansicht ohne Keystore-Angaben; die Werte unter `[keystore]` bleiben in der Datei.
/// Die Keystore-Passwörter im sicheren Speicher entfernt er beim ersten Start **einmalig** (nur Einträge von
/// PIDA). Import, Prüfung und jede Wahl des Keystores bleiben ohne Wirkung – ohne Sitzung.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keystore_of_an_earlier_mobile_build_is_ignored_and_its_passwords_removed_once() {
    let fx = Fixture::new();
    let (storage, secrets) = shell_secrets();
    let ks = earlier_mobile_build(&fx, &storage, secrets.as_ref());
    let keystore_before = file_settings(&fx).keystore;
    assert_eq!(keystore_entries(&storage).len(), 4);
    let docs = fx.dir("docs");
    let core = start(&fx, Platform::Ios, workspace_options(&secrets, &docs));
    let v = wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;

    // nur ID Austria, als Standard gewählt
    let ids: Vec<BackendId> = v.signers.options.iter().map(|o| o.backend).collect();
    assert_eq!(ids, vec![BackendId::AtrustNative]);
    assert_eq!(v.signers.default, Some(BackendId::AtrustNative));
    assert_eq!(v.signers.selected, Some(BackendId::AtrustNative));
    assert!(v.keystore_import.is_none());
    // Keystore-Passwörter entfernt, Proxy-Passwort und fremde Einträge bleiben; Merker gesetzt
    assert!(keystore_entries(&storage).is_empty(), "{:?}", keystore_entries(&storage));
    assert!(storage.0.lock().unwrap().contains_key("ANDERE-APP/keystore-store:/x.p12"));
    let proxy = secrets.get(&SecretId::proxy("proxy.example.test", "u")).unwrap();
    assert_eq!(proxy.as_deref().map(String::as_str), Some("proxy-geheim"));
    assert!(fx.paths.data_dir.join("keystore-secrets-removed").is_file());
    assert!(ks.is_file(), "die Datei selbst bleibt");

    // Einstellungen: Standard ID Austria, `[keystore]` unverändert, keine Keystore-Angaben
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_for(&core, "Einstellungen", |v| v.settings.is_some()).await;
    let s = v.settings.unwrap();
    assert_eq!(s.settings.backend.default, Some(pida_config::BackendChoice::IdAustria));
    assert_eq!(s.settings.keystore, keystore_before, "Keystore-Werte nicht löschen, nur ignorieren");
    assert!(!s.keystore_ready);
    assert_eq!((s.keystore_format, s.keystore_entries, s.keystore_inspect_problem), (None, None, None));
    assert!(s.problems.keys().all(|k| !k.starts_with("keystore.")), "{:?}", s.problems);
    // „Schlüssel anzeigen“ (mit bekanntem Passwort) und Import: ohne Wirkung
    core.dispatch(UiEvent::InspectKeystore { password: Some("test".into()), password_storage: None });
    let p12 = fx.dir("import").join("neu.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    core.dispatch(UiEvent::ImportKeystore { path: p12.clone(), password: Some("test".into()), remember: true });
    // sofort wirksame Einstellungen: Keystore und Keystore als Standard abgelehnt, anderes übernommen
    let changes = [
        ("keystore.enabled", serde_json::json!(false)),
        ("keystore.file", serde_json::json!("/x.p12")),
        ("backend.default", serde_json::json!("keystore")),
        ("signature.profile", serde_json::json!("AMTSSIGNATURBLOCK")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    core.dispatch(UiEvent::ApplySettings { changes, revision: 1 });
    let v = wait_for(&core, "übernommen", |v| v.settings.as_ref().is_some_and(|s| s.applied_revision == 1)).await;
    let s = v.settings.unwrap();
    for field in ["keystore.enabled", "keystore.file", "backend.default"] {
        assert_eq!(s.problems.get(field).map(String::as_str), Some("ng-settings-unavailable-mobile"), "{field}");
    }
    assert_eq!(s.settings.signature.profile, pida_config::SignatureProfile::Amtssignaturblock);
    assert!(s.keystore_entries.is_none() && s.keystore_inspect_problem.is_none());
    assert!(v.keystore_import.is_none(), "kein Import");
    assert!(!fx.paths.data_dir.join("keystores/neu.p12").exists());
    let saved = file_settings(&fx);
    assert_eq!(saved.backend.default, Some(pida_config::BackendChoice::IdAustria), "beim Lesen abgebildet");
    assert_eq!(saved.keystore, keystore_before, "Abschnitt `[keystore]` unverändert gespeichert");
    assert!(keystore_entries(&storage).is_empty(), "ein richtiges Passwort wird nicht gemerkt");
    core.dispatch(UiEvent::CloseSettings);

    // Arbeitsbereich und Stapel: jede Wahl des Keystores ohne Wirkung, keine Sitzung
    let inbox = fx.dir("inbox");
    let a = copy_pdf(PDF, &inbox, "a.pdf");
    let b = copy_pdf(PDF, &inbox, "b.pdf");
    std::fs::write(&b, [std::fs::read(&b).unwrap().as_slice(), b"\n%b\n"].concat()).unwrap();
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    wait_for(&core, "Arbeitsbereich", |v| v.document.is_some() && v.actions.sign).await;
    core.dispatch(UiEvent::SelectSigner { backend: Some(BackendId::Keystore), make_default: true });
    // die Chipkarte gibt es auf dem Gerät ebenso wenig: weder gewählt noch als Standard gespeichert
    core.dispatch(UiEvent::SelectSigner { backend: Some(BackendId::LocalBku), make_default: true });
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    core.dispatch(UiEvent::SubmitSecret {
        request: 1,
        secret: "test".into(),
        password_storage: Some(PasswordStorage::Keychain),
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let v = core.view();
    assert!(v.document.is_some() && v.actions.sign, "kein Signieren: {:?}", v.step);
    assert_eq!(v.signers.selected, Some(BackendId::AtrustNative));
    assert_eq!(v.signers.default, Some(BackendId::AtrustNative));
    assert_eq!(file_settings(&fx).backend.default, Some(pida_config::BackendChoice::IdAustria));
    core.dispatch(UiEvent::OpenFiles { paths: vec![b], mode: Some(pida_core_api::OpenFilesMode::Add) });
    let v = wait_for(&core, "Stapel", |v| batch_ready(v, 2)).await;
    let docs_in_batch = v.batch.as_ref().unwrap().items.iter().map(|i| i.doc).collect();
    core.dispatch(UiEvent::StartBatch {
        backend: BackendId::Keystore,
        docs: docs_in_batch,
        output_dir: None,
        on_existing: pida_workflow::ExistingPolicy::KeepBoth,
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(matches!(core.view().step, StepView::Batch), "kein Stapellauf mit Keystore: {:?}", core.view().step);
    let estimate = core.view().batch.and_then(|b| b.estimate).expect("Schätzung");
    assert_eq!(estimate.backend, BackendId::AtrustNative, "Stapel signiert mit ID Austria");
    drop(core);

    // zweiter Start: einmalig – ein (z. B. von einer anderen Quelle) neu abgelegtes Passwort bleibt
    secrets.set(&SecretId::keystore_store(&ks), "test").unwrap();
    let core = start(&fx, Platform::Ios, workspace_options(&secrets, &docs));
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    assert_eq!(keystore_entries(&storage).len(), 1, "nur beim ersten Start entfernt");
}

/// Desktop-Gegenprobe: dieselben Einstellungen und Passwörter – Keystore wie bisher (registriert, Standard,
/// Passwort aus dem Schlüsselbund), nichts entfernt, kein Merker.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desktop_keeps_the_keystore_of_the_same_settings() {
    let fx = Fixture::new();
    let (storage, secrets) = shell_secrets();
    let ks = earlier_mobile_build(&fx, &storage, secrets.as_ref());
    let core = start(&fx, Platform::Linux, workspace_options(&secrets, &fx.dir("docs")));
    let v =
        wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. }) && !v.signers.options.is_empty()).await;
    let keystore = v.signers.options.iter().find(|o| o.backend == BackendId::Keystore).expect("Keystore");
    assert!(keystore.available);
    assert_eq!(keystore.account.as_deref(), Some("test.p12 · max"));
    assert_eq!(v.signers.default, Some(BackendId::Keystore));
    assert_eq!(keystore_entries(&storage).len(), 4, "nichts entfernt");
    assert!(!fx.paths.data_dir.join("keystore-secrets-removed").exists());
    core.dispatch(UiEvent::OpenSettings);
    let v =
        wait_for(&core, "Einstellungen", |v| v.settings.as_ref().is_some_and(|s| s.keystore_format.is_some())).await;
    let s = v.settings.unwrap();
    assert_eq!(s.settings.backend.default, Some(pida_config::BackendChoice::Keystore));
    assert!(s.keystore_ready);
    assert_eq!(s.settings.keystore.file.as_deref(), Some(ks.as_path()));
}

/// Keine Signiermittel-Auswahl auf iOS/Android (ADR 0004 Nachtrag 20, Meldung des Auftraggebers „es gibt nur ID
/// Austria“): `choice_offered = false`, `selected` = ID Austria – auch mit „Jedes Mal fragen“ in der Datei, nach
/// `select_signer` ohne Signiermittel bzw. mit dem Keystore und in der Stapelübersicht; „Signieren mit ID Austria“
/// öffnet die Sitzung erst auf die ausdrückliche Aktion (A1). Desktop-Gegenprobe mit denselben Einstellungen:
/// ID Austria, Chipkarte und (nicht eingerichtet) Keystore, Auswahl angeboten, ohne Vorauswahl.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_id_austria_on_mobile_offers_no_signer_choice() {
    for platform in [Platform::Ios, Platform::Android, Platform::Linux] {
        let mobile = platform != Platform::Linux;
        let fx = Fixture::new();
        fx.write_settings("[general]\n"); // „Jedes Mal fragen“ (kein `backend.default`), ohne Ersteinrichtung
        let fake = FakeAtrust::new();
        let backends: Option<Vec<Arc<dyn SignatureBackend>>> = mobile.then(|| vec![fake.backend()]);
        let options = CoreOptions {
            workspace_ui: Some(true),
            batch_overview: Some(true),
            ..mobile_options(&FakeEngine::new(), &fx.dir("docs"), backends)
        };
        let core = start(&fx, platform, options);
        let inbox = fx.dir("inbox");
        let a = copy_pdf(PDF, &inbox, "a.pdf");
        core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
        let v = wait_for(&core, "Arbeitsbereich", |v| v.document.is_some() && v.actions.sign).await;
        if !mobile {
            let ids: Vec<BackendId> = v.signers.options.iter().map(|o| o.backend).collect();
            assert_eq!(ids, vec![BackendId::AtrustNative, BackendId::LocalBku, BackendId::Keystore], "Desktop");
            assert!(v.signers.choice_offered, "Desktop: Auswahl wie bisher");
            assert_eq!(v.signers.selected, None, "Desktop: „Jedes Mal fragen“ ohne Vorauswahl");
            continue;
        }
        assert!(!v.signers.choice_offered, "{platform:?}");
        assert_eq!(v.signers.selected, Some(BackendId::AtrustNative), "{platform:?}");
        for signer in [None, Some(BackendId::Keystore)] {
            core.dispatch(UiEvent::SelectSigner { backend: signer, make_default: false });
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let v = core.view();
            assert!(!v.signers.choice_offered);
            assert_eq!(v.signers.selected, Some(BackendId::AtrustNative), "{platform:?}: {signer:?}");
        }
        // Stapelübersicht: ebenso keine Auswahl
        let b = copy_pdf(PDF, &inbox, "b.pdf");
        std::fs::write(&b, [std::fs::read(&b).unwrap().as_slice(), b"\n%b\n"].concat()).unwrap();
        core.dispatch(UiEvent::OpenFiles { paths: vec![b], mode: Some(pida_core_api::OpenFilesMode::Add) });
        let v = wait_for(&core, "Stapel", |v| batch_ready(v, 2)).await;
        assert!(!v.signers.choice_offered && v.signers.selected == Some(BackendId::AtrustNative), "{platform:?}");
        assert!(v.batch.as_ref().unwrap().estimate.is_some(), "{platform:?}: Schätzung mit ID Austria");
        assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "{platform:?}: keine Sitzung ohne „Signieren“ (A1)");
        let doc = v.batch.as_ref().unwrap().items[0].doc;
        core.dispatch(UiEvent::OpenBatchItem { doc });
        wait_for(&core, "Eintrag", |v| v.document.as_ref().is_some_and(|d| d.doc.id == doc) && v.actions.sign).await;
        core.dispatch(UiEvent::SignWith { backend: BackendId::AtrustNative });
        wait_form(&core, "Anmeldung", |f| matches!(f, NativeForm::Login { .. })).await;
        assert_eq!(fake.opened.load(Ordering::SeqCst), 1, "{platform:?}");
    }
}

/// Eingeschalteter Keystore eines früheren Builds, Datei fehlt: auf Mobile nur ID Austria, aber **keine**
/// automatische ID-Austria-Sitzung (F-54, ADR 0003 – ohne „Signieren“ bleibt die Auswahl), und kein
/// Keystore-Problem in den Einstellungen. Desktop: Problem „Datei nicht gefunden“ wie bisher.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keystore_settings_of_an_earlier_build_do_not_start_id_austria_automatically() {
    for platform in [Platform::Ios, Platform::Linux] {
        let fx = Fixture::new();
        let gone = fx.tmp.path().join("weg/keystores/test.p12");
        fx.write_settings(&format!("[signature]\nauto_position = true\n[keystore]\nenabled = true\nfile = {gone:?}\n"));
        let options = CoreOptions { engine: Some(FakeEngine::new()), ..CoreOptions::isolated() };
        let core = start(&fx, platform, options);
        let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
        core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
        let v =
            wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options, .. } if !options.is_empty()))
                .await;
        let StepView::Backend { options } = v.step else { unreachable!() };
        let ids: Vec<BackendId> = options.iter().map(|o| o.id).collect();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(matches!(core.view().step, StepView::Backend { .. }), "{platform:?}: kein Autostart");
        core.dispatch(UiEvent::OpenSettings);
        let v = wait_for(&core, "Einstellungen", |v| v.settings.is_some()).await;
        let problem = v.settings.unwrap().problems.get("keystore.file").cloned();
        if platform == Platform::Ios {
            assert_eq!(ids, vec![BackendId::AtrustNative]);
            assert_eq!(problem, None);
        } else {
            assert_eq!(ids, vec![BackendId::AtrustNative, BackendId::LocalBku, BackendId::Keystore]);
            assert!(!options[2].available);
            assert_eq!(problem.as_deref(), Some("ng-keystore-file-not-found"));
        }
    }
}

/// „Signatur fortsetzen“ eines Keystore-Vorgangs aus einem früheren Mobile-Build: gibt es nicht – der Kern
/// verwirft den gesicherten Zustand beim Start mit Hinweis, statt ihn anzubieten; nichts bricht, ID Austria
/// signiert danach wie gewohnt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keystore_resume_of_an_earlier_build_is_discarded() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let docs = fx.dir("docs");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let core = start(
        &fx,
        Platform::Android,
        mobile_options(&FakeEngine::new(), &docs, Some(vec![FakeAtrust::new().backend()])),
    );
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    drop(core);
    // wie von einem früheren Build mit Keystore gesichert
    let record = resume_dir(&fx).join("resume.json");
    let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(json["backend"], "atrust_native");
    json["backend"] = serde_json::json!("keystore");
    std::fs::write(&record, serde_json::to_vec(&json).unwrap()).unwrap();

    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Android, mobile_options(&FakeEngine::new(), &docs, Some(vec![fake.backend()])));
    let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-resume-signer-not-offered")).await;
    assert!(v.resume.is_none(), "kein Angebot");
    assert!(!resume_dir(&fx).exists(), "gesicherter Zustand verworfen");
    core.dispatch(UiEvent::ResumeSignature);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    choose_id_austria(&core).await;
    login(&core).await;
    wait_form(&core, "TAN mit ID Austria", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1);
}

/// Übernahme aus PDF-Over auf Mobile (ADR 0004 Nachtrag 11): Keystore als Standard wird zu ID Austria, Datei,
/// Alias und Keystore-Passwörter werden nicht übernommen, die Übernahmeliste erklärt es kurz
/// (`ng-migration-item-signer-mobile`), ohne Frage nach Klartext-Passwörtern; die Ersteinrichtung bietet nur ID
/// Austria. Desktop-Gegenprobe: Keystore samt Passwort wie bisher.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn migration_on_mobile_takes_over_no_keystore() {
    for platform in [Platform::Android, Platform::Linux] {
        let fx = Fixture::new();
        let legacy = fx.dir("home/.pdf-over");
        let ks = legacy.join("test.p12");
        std::fs::copy(testdata(P12), &ks).unwrap();
        std::fs::write(
            legacy.join("PDF-Over.config"),
            format!(
                "BKU=KS\nKEYSTORE_ENABLED=true\nKEYSTORE_FILE={}\nKEYSTORE_ALIAS=max\nKEYSTORE_TYPE=PKCS12\n\
                 KEYSTORE_PASS_STORE_TYPE=disk\nKEYSTORE_STOREPASS=test\nKEYSTORE_KEYPASS=test\n\
                 MOBILE_NUMBER=+436641234567\nSIGNATURE_PROFILE=AMTSSIGNATURBLOCK\n",
                java_prop(&ks)
            ),
        )
        .unwrap();
        let store = Arc::new(pida_config::MemorySecretStore::default());
        let options = CoreOptions {
            workspace_ui: Some(true),
            engine: Some(FakeEngine::new()),
            legacy_dir: Some(legacy.clone()),
            secret_store: Some(store.clone()),
            default_output_dir: Some(fx.dir("docs")),
            ..CoreOptions::isolated()
        };
        let core = start(&fx, platform, options);
        let v = wait_for(&core, "Angebot", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
        let offer = v.first_run.unwrap().migration.unwrap();
        let codes: Vec<&str> = offer.items.iter().map(|i| i.code.as_str()).collect();
        let mobile = platform == Platform::Android;
        if mobile {
            assert!(codes.contains(&"ng-migration-item-signer-mobile"), "{codes:?}");
            assert!(!codes.contains(&"ng-migration-item-keystore"), "{codes:?}");
            assert!(!offer.plaintext_secrets, "nur Keystore-Passwörter in der Altdatei – keine Frage");
            assert_eq!(v.signers.options.iter().map(|o| o.backend).collect::<Vec<_>>(), vec![BackendId::AtrustNative]);
        } else {
            assert!(
                codes.contains(&"ng-migration-item-keystore") && !codes.contains(&"ng-migration-item-signer-mobile")
            );
            assert!(offer.plaintext_secrets);
        }
        assert!(codes.contains(&"ng-migration-item-username"), "anderes wird übernommen: {codes:?}");
        core.dispatch(UiEvent::AcceptMigration { store_secrets: true });
        let v = wait_for(&core, "übernommen", |v| v.notices.iter().any(|n| n.code == "ng-migrate-done")).await;
        assert!(v.first_run.is_some(), "Ersteinrichtung bleibt offen");
        let saved = file_settings(&fx);
        let stored = |id: &SecretId| store.get(id).unwrap().is_some();
        if mobile {
            assert_eq!(saved.backend.default, Some(pida_config::BackendChoice::IdAustria));
            assert_eq!(saved.keystore, pida_config::KeystoreSettings::default(), "ohne Pfad und Alias");
            assert!(!stored(&SecretId::keystore_store(&ks)) && !stored(&SecretId::keystore_key(&ks, "max")));
            // Schritt 2 der Ersteinrichtung: Keystore als Standard abgelehnt, das Profil übernommen
            core.dispatch(UiEvent::UpdateFirstRun {
                default_backend: Some(pida_config::BackendChoice::Keystore),
                profile: pida_config::SignatureProfile::SignaturblockSmall,
                block_language: None,
            });
            let v = wait_for(&core, "Schritt 2", |v| {
                v.first_run.as_ref().is_some_and(|f| f.profile == pida_config::SignatureProfile::SignaturblockSmall)
            })
            .await;
            assert_eq!(v.first_run.unwrap().default_backend, Some(pida_config::BackendChoice::IdAustria));
            assert_eq!(v.signers.selected, Some(BackendId::AtrustNative));
        } else {
            assert_eq!(saved.backend.default, Some(pida_config::BackendChoice::Keystore));
            assert_eq!(saved.keystore.file.as_deref(), Some(ks.as_path()));
            assert!(stored(&SecretId::keystore_store(&ks)));
        }
        assert_eq!(saved.id_austria.username.as_deref(), Some("+436641234567"));
    }
}

// ------------------------------------------------------------------------------------------------
// PKCS#12-Import in die Sandbox (Kern; seit ADR 0004 Nachtrag 11 nur noch am Desktop wirksam – die
// Oberfläche bietet ihn nirgends mehr an, auf iOS/Android ist das Ereignis ohne Wirkung)
// ------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_copies_into_the_data_folder_and_configures_the_keystore() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let pick = fx.dir("import");
    let p12 = pick.join("Mein Zertifikat.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let not_p12 = pick.join("kein.p12");
    std::fs::write(&not_p12, b"%PDF-1.7\n").unwrap();
    let (storage, secrets) = shell_secrets();
    let options = CoreOptions {
        secret_store: Some(secrets.clone()),
        ..mobile_options(&FakeEngine::new(), &fx.dir("docs"), None)
    };
    let core = start(&fx, Platform::Linux, options);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;

    // falsches Format, falsches Passwort: nichts geändert
    core.dispatch(UiEvent::ImportKeystore { path: not_p12, password: Some("test".into()), remember: true });
    let v = wait_for(&core, "Formatfehler", |v| v.keystore_import.is_some()).await;
    let r = v.keystore_import.unwrap();
    assert!(!r.ok);
    assert_eq!(r.code, "ng-keystore-import-format");
    core.dispatch(UiEvent::DismissKeystoreImport);
    wait_for(&core, "geschlossen", |v| v.keystore_import.is_none()).await;
    core.dispatch(UiEvent::ImportKeystore { path: p12.clone(), password: Some("falsch".into()), remember: true });
    let v = wait_for(&core, "Passwortfehler", |v| v.keystore_import.is_some()).await;
    assert_eq!(v.keystore_import.unwrap().code, "keystore-wrong-password");
    core.dispatch(UiEvent::DismissKeystoreImport);
    let keystores = fx.paths.data_dir.join("keystores");
    assert!(!keystores.join("Mein Zertifikat.p12").exists());

    // richtig: Kopie (0600), Einstellungen, Passwort im sicheren Speicher
    core.dispatch(UiEvent::ImportKeystore { path: p12.clone(), password: Some("test".into()), remember: true });
    let v = wait_for(&core, "importiert", |v| v.keystore_import.as_ref().is_some_and(|r| r.ok)).await;
    let r = v.keystore_import.unwrap();
    assert_eq!(r.code, "ng-keystore-imported");
    assert_eq!(r.file_name, "Mein Zertifikat.p12");
    assert!(r.password_stored);
    let copy = keystores.join("Mein Zertifikat.p12");
    assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(&p12).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&copy).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let saved = file_settings(&fx);
    assert!(saved.keystore.enabled);
    assert_eq!(saved.keystore.file.as_deref(), Some(copy.as_path()));
    assert_eq!(saved.keystore.declared_type.as_deref(), Some("PKCS12"));
    assert_eq!(saved.keystore.password_storage, pida_config::PasswordStorage::Keychain);
    assert_eq!(secrets.get(&SecretId::keystore_store(&copy)).unwrap().as_deref().map(String::as_str), Some("test"));
    assert!(storage.0.lock().unwrap().keys().all(|k| k.starts_with("PIDA/")));
    // die Quelldatei bleibt (gehört der Shell); signieren ohne Passwortabfrage
    assert!(p12.is_file());
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    let keystore_ready = |v: &ViewState| matches!(&v.step, StepView::Backend { options, .. } if options.iter().any(|o| o.id == BackendId::Keystore && o.available));
    wait_for(&core, "Auswahl", keystore_ready).await;
    core.dispatch(UiEvent::ChooseBackend { backend: BackendId::Keystore });
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_is_refused_while_signing_and_redacts_the_password() {
    let event = UiEvent::ImportKeystore { path: "/x/k.p12".into(), password: Some("geheim".into()), remember: false };
    let debug = format!("{event:?}");
    assert!(!debug.contains("geheim") && !debug.contains("/x/k.p12"), "{debug}");
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let p12 = fx.dir("import").join("k.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let core = start(
        &fx,
        Platform::Linux,
        mobile_options(&FakeEngine::new(), &fx.dir("docs"), Some(vec![FakeAtrust::new().backend()])),
    );
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    choose_id_austria(&core).await;
    wait_form(&core, "Login", |f| matches!(f, NativeForm::Login { .. })).await;
    core.dispatch(UiEvent::ImportKeystore { path: p12, password: None, remember: false });
    let v = wait_for(&core, "abgelehnt", |v| v.keystore_import.is_some()).await;
    assert_eq!(v.keystore_import.unwrap().code, "ng-keystore-import-busy");
    assert!(form_request(&core.view()).is_some(), "Signiervorgang läuft weiter");
}

/// Import direkt aus der Signiermittel-Auswahl (Keystore noch nicht eingerichtet): danach ist der Keystore
/// dort sofort verfügbar; ohne „merken“ wird das Passwort nicht gespeichert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_from_the_backend_selection_offers_the_keystore() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let p12 = fx.dir("import").join("test.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let options = CoreOptions { engine: Some(FakeEngine::new()), ..CoreOptions::isolated() };
    let core = start(&fx, Platform::Linux, options);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    // ohne „Signieren“ kein Autostart: die Auswahl erscheint (kein Zustimmungsdialog)
    wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options } if !options.is_empty())).await;
    core.dispatch(UiEvent::ImportKeystore { path: p12, password: Some("test".into()), remember: false });
    let v = wait_for(&core, "Keystore verfügbar", |v| {
        matches!(&v.step, StepView::Backend { options, .. }
            if options.iter().any(|o| o.id == BackendId::Keystore && o.available))
    })
    .await;
    let r = v.keystore_import.expect("Ergebnis");
    assert!(r.ok);
    assert!(!r.password_stored, "ohne „merken“ nichts im sicheren Speicher");
}

/// Die Kopie der Dateiauswahl im Cache (`<cache>/import/<n>/`) verschwindet nach einem erfolgreichen Import –
/// der private Schlüssel liegt danach nur noch einmal im Datenordner. Nach einem falschen Passwort bleibt sie
/// (neuer Versuch mit derselben Datei).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_removes_the_picker_copy_from_the_cache() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let pick = fx.paths.cache_dir.join("import").join("1");
    std::fs::create_dir_all(&pick).unwrap();
    let p12 = pick.join("test.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let core = start(&fx, Platform::Linux, mobile_options(&FakeEngine::new(), &fx.dir("docs"), None));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::ImportKeystore { path: p12.clone(), password: Some("falsch".into()), remember: false });
    let v = wait_for(&core, "Passwortfehler", |v| v.keystore_import.is_some()).await;
    assert!(!v.keystore_import.unwrap().ok);
    assert!(p12.is_file(), "nach einem Fehler bleibt die Kopie für den nächsten Versuch");
    core.dispatch(UiEvent::DismissKeystoreImport);
    wait_for(&core, "geschlossen", |v| v.keystore_import.is_none()).await;
    core.dispatch(UiEvent::ImportKeystore { path: p12.clone(), password: Some("test".into()), remember: false });
    wait_for(&core, "importiert", |v| v.keystore_import.as_ref().is_some_and(|r| r.ok)).await;
    assert!(fx.paths.data_dir.join("keystores/test.p12").is_file());
    assert!(!p12.exists(), "Kopie im Cache entfernt");
    assert!(!pick.exists(), "leerer Unterordner entfernt");
    assert!(fx.paths.cache_dir.join("import").is_dir(), "der Import-Ordner selbst bleibt");
}

/// iOS/Android: der Import ist ohne Wirkung – keine Kopie, keine Einstellung, kein Ergebnis, kein Passwort im
/// sicheren Speicher; die Kopie der Dateiauswahl bleibt unberührt (sie gehört der Shell).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_is_without_effect_on_mobile() {
    for platform in [Platform::Ios, Platform::Android] {
        let fx = Fixture::new();
        fx.write_settings("[signature]\nauto_position = true\n");
        let pick = fx.paths.cache_dir.join("import").join("1");
        std::fs::create_dir_all(&pick).unwrap();
        let p12 = pick.join("test.p12");
        std::fs::copy(testdata(P12), &p12).unwrap();
        let (storage, secrets) = shell_secrets();
        let options = CoreOptions {
            secret_store: Some(secrets.clone()),
            ..mobile_options(&FakeEngine::new(), &fx.dir("docs"), None)
        };
        let core = start(&fx, platform, options);
        wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
        core.dispatch(UiEvent::ImportKeystore { path: p12.clone(), password: Some("test".into()), remember: true });
        core.dispatch(UiEvent::DismissKeystoreImport);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(core.view().keystore_import.is_none(), "{platform:?}");
        assert!(!fx.paths.data_dir.join("keystores").exists());
        assert!(p12.is_file());
        assert!(keystore_entries(&storage).is_empty());
        assert_eq!(file_settings(&fx).keystore, pida_config::KeystoreSettings::default());
    }
}

/// „Zuletzt signiert“ speichert absolute Pfade; nach einem Update (neuer iOS-Container) zeigen sie in den
/// alten Container. Der Kern stellt sie auf die App-Dokumente um (gleicher Dateiname, Ordner gleichen
/// Namens) – die Liste zeigt die Dateien weiter als vorhanden. Desktop: unverändert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recent_documents_follow_a_moved_sandbox() {
    for platform in [Platform::Ios, Platform::Linux] {
        let fx = Fixture::new();
        let docs = fx.tmp.path().join("NEUE-UUID/Documents");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("a_signed.pdf"), b"%PDF-1.7").unwrap();
        let old = fx.tmp.path().join("ALTE-UUID/Documents");
        let elsewhere = fx.tmp.path().join("ALTE-UUID/anderswo/b_signed.pdf");
        let entries = serde_json::json!([
            { "id": 2, "file_name": "a_signed.pdf", "path": old.join("a_signed.pdf"), "signed_at": "2026-10-16T13:57:35+02:00" },
            { "id": 1, "file_name": "b_signed.pdf", "path": elsewhere, "signed_at": "2026-10-16T13:49:07+02:00" },
        ]);
        let recent_file = fx.paths.data_dir.join("recent.json");
        std::fs::create_dir_all(&fx.paths.data_dir).unwrap();
        std::fs::write(&recent_file, serde_json::to_vec(&entries).unwrap()).unwrap();
        let core = start(&fx, platform, mobile_options(&FakeEngine::new(), &docs, None));
        let v = wait_for(&core, "Liste", |v| v.recent.as_ref().is_some_and(|r| r.len() == 2)).await;
        let recent = v.recent.unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&recent_file).unwrap()).unwrap();
        if platform == Platform::Linux {
            assert!(!recent[0].exists, "Desktop: kein Umzug");
            assert_eq!(saved, entries);
            continue;
        }
        assert!(recent[0].exists, "umgestellt auf die App-Dokumente");
        assert!(!recent[1].exists, "anderer Ordner bleibt unangetastet");
        assert_eq!(saved[0]["path"], serde_json::json!(docs.join("a_signed.pdf")));
        assert_eq!(saved[1]["path"], serde_json::json!(elsewhere));
    }
}

// ------------------------------------------------------------------------------------------------
// Stapel auf Mobile (ADR 0004 §3.8, Abnahme E2 Punkt 8)
// ------------------------------------------------------------------------------------------------

fn batch_ready(v: &ViewState, n: usize) -> bool {
    matches!(v.step, StepView::Batch)
        && v.batch.as_ref().is_some_and(|b| {
            b.items.len() == n
                && b.items.iter().all(|i| !matches!(i.status, pida_core_api::BatchItemStatusView::Analyzing))
        })
}

fn batch_tan(v: &ViewState, index: u32) -> bool {
    form_request(v).is_some_and(|(_, f)| matches!(f, NativeForm::SmsTan { .. }))
        && matches!(&v.step, StepView::Signing { progress, .. } if progress.batch && progress.index == index)
}

/// Stapel mit ID Austria auf dem Telefon: der Wechsel in die App „ID Austria“ je Bestätigung ist kein Abbruch;
/// nach einem App-Kill mitten im Lauf setzt „Signatur fortsetzen“ **nur** das unterbrochene Dokument fort
/// (Ausgabe wie beim Einzeldokument in die App-Dokumente), danach folgt die Stapelübersicht mit dem Rest als
/// „Bereit“ – der Lauf wird nicht automatisch fortgesetzt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn batch_on_mobile_resumes_only_the_interrupted_document() {
    let fx = Fixture::new();
    let docs = fx.dir("docs");
    let inbox = fx.dir("inbox");
    let files: Vec<PathBuf> = ["a.pdf", "b.pdf", "c.pdf"]
        .iter()
        .map(|n| {
            let p = copy_pdf(PDF, &inbox, n);
            std::fs::write(&p, [std::fs::read(&p).unwrap().as_slice(), format!("\n%{n}\n").as_bytes()].concat())
                .unwrap();
            p
        })
        .collect();
    let batch_options = |engine: &Arc<FakeEngine>, fake: &Arc<FakeAtrust>| CoreOptions {
        workspace_ui: Some(true),
        batch_overview: Some(true),
        ..mobile_options(engine, &docs, Some(vec![fake.backend()]))
    };

    // 1. Lauf: a signiert, bei b „vom System beendet“
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Ios, batch_options(&engine, &fake));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: files.clone(), mode: None });
    let v = wait_for(&core, "Übersicht", |v| batch_ready(v, 3)).await;
    let b = v.batch.as_ref().unwrap();
    assert_eq!(b.output.mode, pida_core_api::OutputModeView::AppDocuments, "Mobile: App-Dokumente");
    assert!(!b.output.needs_folder && b.output.folder_display.is_none());
    let ids = b.items.iter().map(|i| i.doc).collect();
    core.dispatch(UiEvent::StartBatch {
        backend: BackendId::AtrustNative,
        docs: ids,
        output_dir: None,
        on_existing: pida_workflow::ExistingPolicy::KeepBoth,
    });
    login(&core).await;
    let v = wait_for(&core, "Bestätigung 1", |v| batch_tan(v, 1)).await;
    // Wechsel in die App „ID Austria“ und zurück: kein Abbruch
    core.dispatch(UiEvent::Backgrounded);
    core.dispatch(UiEvent::Foregrounded);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(batch_tan(&core.view(), 1), "App-Wechsel ist kein Abbruch: {:?}", core.view().step);
    assert_eq!(fake.cancelled.load(Ordering::SeqCst), 0);
    let (request, _) = form_request(&v).unwrap();
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    wait_for(&core, "Bestätigung 2", |v| batch_tan(v, 2)).await;
    assert!(docs.join("a_signed.pdf").is_file());
    assert!(resume_dir(&fx).join("resume.json").is_file(), "nach prepare gesichert");
    core.dispatch(UiEvent::Backgrounded);
    drop(core);

    // 2. Lauf: „Signatur fortsetzen“ nur für b; danach die Übersicht mit c „Bereit“
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Ios, batch_options(&engine, &fake));
    let v = wait_for(&core, "Angebot", |v| matches!(v.step, StepView::Open { .. }) && v.resume.is_some()).await;
    let offer = v.resume.unwrap();
    assert_eq!((offer.name.as_str(), offer.pending), ("b.pdf", 1), "a ist signiert, c wartet");
    core.dispatch(UiEvent::ResumeSignature);
    login(&core).await;
    let request = wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    let v = core.view();
    assert!(matches!(&v.step, StepView::Signing { progress, .. } if !progress.batch), "nur das unterbrochene Dokument");
    assert_eq!(engine.restores.load(Ordering::SeqCst), 1, "Prepared::restore statt prepare");
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    let v = wait_for(&core, "Übersicht mit dem Rest", |v| batch_ready(v, 2)).await;
    let items = &v.batch.as_ref().unwrap().items;
    let status = |name: &str| items.iter().find(|i| i.name == name).map(|i| i.status.clone()).unwrap();
    assert!(matches!(status("b.pdf"), pida_core_api::BatchItemStatusView::Signed { .. }));
    assert_eq!(status("c.pdf"), pida_core_api::BatchItemStatusView::Ready, "neue Voranalyse");
    assert!(v.notices.iter().any(|n| n.code == "ng-batch-item-signed"));
    assert!(docs.join("b_signed.pdf").is_file());
    // die fortgesetzte Zeile bekommt ihre Voranalyse – Seitenzahl und Größe wie die übrigen, Status bleibt
    // „Signiert“ (Mobile-Check 16.10.2026: Zeile ohne Miniatur und Seiten/Größe)
    let v = wait_for(&core, "Zeile b vollständig", |v| {
        v.batch.as_ref().is_some_and(|b| b.items.iter().any(|i| i.name == "b.pdf" && i.page_count.is_some()))
    })
    .await;
    let b = v.batch.as_ref().unwrap().items.iter().find(|i| i.name == "b.pdf").unwrap();
    assert!(b.size_bytes.is_some() && matches!(b.status, pida_core_api::BatchItemStatusView::Signed { .. }));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(matches!(core.view().step, StepView::Batch), "der Lauf wird nicht automatisch fortgesetzt");
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1, "keine weitere Sitzung");
    assert!(!resume_dir(&fx).exists(), "nach Erfolg kein persistierter Zustand");
}

/// Fortsetzen im Stapel behält die beim Start gewählte Regel für vorhandene Zieldateien (Emulator-Abnahme
/// 16.10.2026): Mit „Beide behalten“ schreibt das fortgesetzte Dokument die erste freie Nummer – ohne die
/// Rückfrage „Datei überschreiben?“, die vorhandene Datei bleibt unverändert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn batch_resume_keeps_the_rule_for_existing_files() {
    let fx = Fixture::new();
    let docs = fx.dir("docs");
    let inbox = fx.dir("inbox");
    let files: Vec<PathBuf> = ["a.pdf", "b.pdf"]
        .iter()
        .map(|n| {
            let p = copy_pdf(PDF, &inbox, n);
            std::fs::write(&p, [std::fs::read(&p).unwrap().as_slice(), format!("\n%{n}\n").as_bytes()].concat())
                .unwrap();
            p
        })
        .collect();
    let original_b = std::fs::read(&files[1]).unwrap();
    std::fs::write(docs.join("b_signed.pdf"), b"vorher signiert").unwrap();
    let batch_options = |engine: &Arc<FakeEngine>, fake: &Arc<FakeAtrust>| CoreOptions {
        workspace_ui: Some(true),
        batch_overview: Some(true),
        ..mobile_options(engine, &docs, Some(vec![fake.backend()]))
    };

    // 1. Lauf: „Beide behalten“, a signiert, bei b „vom System beendet“
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Android, batch_options(&engine, &fake));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: files.clone(), mode: None });
    let v = wait_for(&core, "Übersicht", |v| batch_ready(v, 2)).await;
    let ids = v.batch.as_ref().unwrap().items.iter().map(|i| i.doc).collect();
    core.dispatch(UiEvent::StartBatch {
        backend: BackendId::AtrustNative,
        docs: ids,
        output_dir: None,
        on_existing: pida_workflow::ExistingPolicy::KeepBoth,
    });
    login(&core).await;
    let v = wait_for(&core, "Bestätigung 1", |v| batch_tan(v, 1)).await;
    let (request, _) = form_request(&v).unwrap();
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    wait_for(&core, "Bestätigung 2", |v| batch_tan(v, 2)).await;
    assert!(resume_dir(&fx).join("resume.json").is_file(), "nach prepare gesichert");
    let record = std::fs::read_to_string(resume_dir(&fx).join("resume.json")).unwrap();
    assert!(record.contains("\"on_existing\":\"keep_both\""), "Regel des Laufs gesichert");
    core.dispatch(UiEvent::Backgrounded);
    drop(core);

    // 2. Lauf: „Signatur fortsetzen“ für b – Ausgabe „b_signed (2).pdf“, keine Rückfrage
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let core = start(&fx, Platform::Android, batch_options(&engine, &fake));
    wait_for(&core, "Angebot", |v| matches!(v.step, StepView::Open { .. }) && v.resume.is_some()).await;
    core.dispatch(UiEvent::ResumeSignature);
    login(&core).await;
    let request = wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    // kein wartendes Dokument mehr: „Fertig“ mit dem gespeicherten Ergebnis (ohne `ask_overwrite`)
    let saved = |v: &ViewState| match &v.step {
        StepView::Output { result, .. } => result.ask_overwrite.is_none() && result.path.is_some(),
        _ => false,
    };
    let v = wait_for(&core, "gespeichert ohne Rückfrage", saved).await;
    let StepView::Output { result, .. } = &v.step else { unreachable!() };
    assert_eq!(result.path.as_deref(), Some(docs.join("b_signed (2).pdf").as_path()), "erste freie Nummer");
    verify_fake_output(&docs.join("b_signed (2).pdf"), &original_b);
    assert_eq!(std::fs::read(docs.join("b_signed.pdf")).unwrap(), b"vorher signiert", "vorhandene Datei unverändert");
    assert!(!resume_dir(&fx).exists(), "nach Erfolg kein persistierter Zustand");
}

/// iOS: Teilt man mehrere PDFs aus der Dateien-App, kommt nur das erste an (keine Share-Extension; Entscheidung des
/// Auftraggebers „Hinweis reicht“, ADR 0004 Nachtrag 12). Nach dem Empfang über „Teilen“ bzw. „Öffnen in“
/// (`FilesReceived` der Shell) zeigt der Arbeitsbereich `share_hint` für das empfangene Dokument – nicht nach der
/// Dateiauswahl (`OpenFiles`), nicht auf Android und am Desktop, nicht mehr, sobald weitere PDFs dazukommen
/// („Weitere PDFs hinzufügen …“), und nach „Hinweis ausblenden“ bis zum Beenden nicht mehr. Startet nichts (A1).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ios_shows_the_share_hint_only_after_receiving_a_document() {
    let fx = Fixture::new();
    let docs = fx.tmp.path().join("Documents");
    let inbox = fx.dir("inbox");
    let a = copy_pdf(PDF, &inbox, "a.pdf");
    let b = copy_pdf("pdf/generated/simple-xref-stream.pdf", &inbox, "b.pdf");
    let fake = FakeAtrust::new();
    let core_on = |platform| {
        let options = CoreOptions {
            workspace_ui: Some(true),
            batch_overview: Some(true),
            ..mobile_options(&FakeEngine::new(), &docs, Some(vec![fake.backend()]))
        };
        start(&fx, platform, options)
    };
    let editing = |name: &'static str| {
        move |v: &ViewState| {
            v.document.as_ref().is_some_and(|d| d.doc.name == name && d.phase == pida_core_api::DocumentPhase::Editing)
        }
    };
    let opened = |v: &ViewState| matches!(v.step, StepView::Open { .. });

    // iOS, empfangen: Hinweis für dieses Dokument
    let core = core_on(Platform::Ios);
    wait_for(&core, "Start", opened).await;
    core.dispatch(UiEvent::FilesReceived { paths: vec![a.clone()] });
    let v = wait_for(&core, "a empfangen", editing("a.pdf")).await;
    let id = v.document.as_ref().unwrap().doc.id;
    assert_eq!(v.share_hint, Some(pida_core_api::ShareHintView { doc: id }));
    // weitere PDFs dazu („Weitere PDFs hinzufügen …“): erledigt, kein Hinweis mehr
    core.dispatch(UiEvent::OpenFiles { paths: vec![b.clone()], mode: Some(pida_core_api::OpenFilesMode::Add) });
    let v = wait_for(&core, "Stapel", |v| v.batch.is_some()).await;
    assert_eq!(v.share_hint, None);
    drop(core);

    // iOS, Dateiauswahl: kein Hinweis; danach empfangen: Hinweis, „Ausblenden“ gilt bis zum Beenden
    let core = core_on(Platform::Ios);
    wait_for(&core, "Start", opened).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    let v = wait_for(&core, "a gewählt", editing("a.pdf")).await;
    assert_eq!(v.share_hint, None, "nicht nach der Dateiauswahl");
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "zurück am Start", |v| opened(v) && v.document.is_none()).await;
    core.dispatch(UiEvent::FilesReceived { paths: vec![b.clone()] });
    let v = wait_for(&core, "b empfangen", |v| editing("b.pdf")(v) && v.share_hint.is_some()).await;
    assert_eq!(v.share_hint.map(|h| h.doc), Some(v.document.as_ref().unwrap().doc.id));
    core.dispatch(UiEvent::DismissShareHint);
    wait_for(&core, "ausgeblendet", |v| editing("b.pdf")(v) && v.share_hint.is_none()).await;
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "wieder am Start", |v| opened(v) && v.document.is_none()).await;
    core.dispatch(UiEvent::FilesReceived { paths: vec![a.clone()] });
    let v = wait_for(&core, "a erneut empfangen", editing("a.pdf")).await;
    assert_eq!(v.share_hint, None, "ausgeblendet bis zum Beenden");
    drop(core);

    // Android (mehrere über `SEND_MULTIPLE`) und Desktop („Öffnen mit“): kein Hinweis
    for platform in [Platform::Android, Platform::MacOs] {
        let core = core_on(platform);
        wait_for(&core, "Start", opened).await;
        core.dispatch(UiEvent::FilesReceived { paths: vec![a.clone()] });
        let v = wait_for(&core, "empfangen", editing("a.pdf")).await;
        assert_eq!(v.share_hint, None, "{platform:?}");
    }
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung (A1)");
}
