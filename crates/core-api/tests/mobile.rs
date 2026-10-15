//! Mobile-Lebenszyklus und Mobile-Funktionen des Kerns (Plan §3.5, §5.2; docs/integration/mobile.md):
//! App-Wechsel ist kein Abbruch und setzt den Long-Poll fort, „Signatur fortsetzen“ nach einem App-Kill
//! mit `Prepared::persist/restore`, Signiermittel je Plattform, Ausgabe in die App-Dokumente,
//! Teilen/Exportieren über die Shell und PKCS#12-Import in die Sandbox.

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::*;
use pdfover_backend_api::{
    BackendError, BackendId, BackendSession, Capabilities, CertChain, Environment, FormAnswer, FormContext,
    InteractionBroker, InteractionError, NativeForm, Platform, SessionContext, SignJob, SignResult, SignatureBackend,
};
use pdfover_cades::{CmsProfile, DigestAlg, RawSigner, SigningTime};
use pdfover_config::{Overlay, SecretId, SecretStore, SecureStorage, Settings, ShellSecretStore};
use pdfover_core_api::{
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
    key: Arc<pdfover_backend_keystore::LoadedKey>,
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
            let cms = pdfover_cades::sign_detached(
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
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
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
    fx.write_settings("[signature]\nauto_position = true\n[privacy.acknowledged]\natrust_native = 1\n");
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
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b.clone()] });
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
    login(&core).await;
    wait_form(&core, "TAN für das wartende Dokument", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 1, "wartendes Dokument wird normal vorbereitet");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resume_can_be_discarded_and_expires() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n[privacy.acknowledged]\natrust_native = 1\n");
    let docs = fx.dir("docs");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let run_until_tan = |fx: &Fixture| {
        let engine = FakeEngine::new();
        let fake = FakeAtrust::new();
        start(fx, Platform::Ios, mobile_options(&engine, &docs, Some(vec![fake.backend()])))
    };
    // gesichert, verworfen
    let core = run_until_tan(&fx);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
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
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
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
    fx.write_settings("[signature]\nauto_position = true\n[privacy.acknowledged]\natrust_native = 1\n");
    let docs = fx.dir("docs");
    let inbox = fx.dir("inbox");
    let a = copy_pdf(PDF, &inbox, "a.pdf");
    let b = copy_pdf(PDF, &inbox, "b.pdf");
    let new_core = || {
        start(&fx, Platform::Ios, mobile_options(&FakeEngine::new(), &docs, Some(vec![FakeAtrust::new().backend()])))
    };
    let core = new_core();
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    drop(core);
    // anderes Dokument, Abbruch in der Anmeldung (vor prepare): Angebot bleibt
    let core = new_core();
    wait_for(&core, "Angebot", |v| v.resume.is_some()).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![b.clone()] });
    wait_form(&core, "Login", |f| matches!(f, NativeForm::Login { .. })).await;
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "Auswahl", |v| matches!(v.step, StepView::Backend { .. })).await;
    assert!(resume_dir(&fx).join("resume.json").is_file(), "fremdes Angebot unangetastet");
    core.dispatch(UiEvent::Navigate { step: pdfover_workflow::Step::Open });
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
    fx.write_settings("[signature]\nauto_position = true\n[privacy.acknowledged]\natrust_native = 1\n");
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let options = CoreOptions {
        engine: Some(FakeEngine::new()),
        backends: Some(vec![FakeAtrust::new().backend()]),
        ..CoreOptions::isolated()
    };
    let core = start(&fx, Platform::Linux, options);
    assert!(!core.view().platform.mobile);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a] });
    login(&core).await;
    wait_form(&core, "TAN", |f| matches!(f, NativeForm::SmsTan { .. })).await;
    assert!(!resume_dir(&fx).exists());
}

// ------------------------------------------------------------------------------------------------
// Signiermittel je Plattform, Ausgabe, Teilen/Exportieren
// ------------------------------------------------------------------------------------------------

/// iOS/Android: keine lokale BKU, ID Austria mit Hinweis „FIDO2 nicht verfügbar“, Keystore nur mit
/// eingerichteter Datei.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mobile_backend_list_has_no_local_bku_and_marks_fido2() {
    for (platform, mobile) in [(Platform::Ios, true), (Platform::Android, true), (Platform::MacOs, false)] {
        let fx = Fixture::new();
        fx.write_settings("[signature]\nauto_position = true\n");
        let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
        let options = CoreOptions { engine: Some(FakeEngine::new()), ..CoreOptions::isolated() };
        let core = start(&fx, platform, options);
        core.dispatch(UiEvent::OpenFiles { paths: vec![a] });
        let v =
            wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options, .. } if !options.is_empty()))
                .await;
        let StepView::Backend { options, privacy_notice } = v.step else { unreachable!() };
        let ids: Vec<BackendId> = options.iter().map(|o| o.id).collect();
        if mobile {
            assert_eq!(ids, vec![BackendId::AtrustNative], "{platform:?}");
            assert_eq!(options[0].notes, vec!["ng-fido2-unavailable-mobile".to_owned()]);
            // nur ID Austria → automatisch gewählt (F-54) → Datenschutzhinweis
            assert!(privacy_notice.is_some());
        } else {
            assert_eq!(ids, vec![BackendId::AtrustNative, BackendId::LocalBku], "{platform:?}");
            assert!(options.iter().all(|o| o.notes.is_empty()));
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
        self.shared.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
    fn export_path(&self, path: &Path, file_name: &str) -> Result<bool, String> {
        self.exported.lock().unwrap().push((path.to_path_buf(), file_name.to_owned()));
        Ok(true)
    }
}

fn keystore_settings(file: &Path) -> String {
    format!(
        "[signature]\nauto_position = true\nprofile = \"INVISIBLE\"\n[backend]\ndefault = \"keystore\"\n[keystore]\nenabled = true\nfile = {file:?}\n"
    )
}

/// Ausgabe in die App-Dokumente; „Teilen“ und „Exportieren“ gehen an die Shell (Effekte des Kerns).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn output_goes_to_app_documents_and_can_be_shared_and_exported() {
    let fx = Fixture::new();
    let ks = fx.dir("ks").join("test.p12");
    std::fs::copy(testdata(P12), &ks).unwrap();
    fx.write_settings(&keystore_settings(&ks));
    let docs = fx.tmp.path().join("Documents");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let hooks = Arc::new(RecordingHooks::default());
    let options = CoreOptions { shell: Some(hooks.clone()), ..mobile_options(&FakeEngine::new(), &docs, None) };
    let core = start(&fx, Platform::Ios, options);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a] });
    let v = wait_for(&core, "Passwort", |v| {
        matches!(&v.step, StepView::Signing { interaction: InteractionView::Secret { .. }, .. })
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Secret { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitSecret { request, secret: "test".into() });
    let v =
        wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
            .await;
    let StepView::Output { result, .. } = v.step else { unreachable!() };
    let out = result.path.unwrap();
    assert_eq!(out, docs.join("a_signed.pdf"), "Ordner wird angelegt, kein Speichern-Dialog");
    core.dispatch(UiEvent::ShareResult);
    core.dispatch(UiEvent::ExportResult);
    wait_for(&core, "exportiert", |v| v.notices.iter().any(|n| n.code == "ng-export-done")).await;
    assert_eq!(*hooks.shared.lock().unwrap(), vec![out.clone()]);
    assert_eq!(*hooks.exported.lock().unwrap(), vec![(out, "a_signed.pdf".to_owned())]);
}

// ------------------------------------------------------------------------------------------------
// PKCS#12-Import in die Sandbox
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_copies_into_the_sandbox_and_configures_the_keystore() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let pick = fx.dir("import");
    let p12 = pick.join("Mein Zertifikat.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let not_p12 = pick.join("kein.p12");
    std::fs::write(&not_p12, b"%PDF-1.7\n").unwrap();
    let storage = Arc::new(FakeSecureStorage::default());
    let secrets: Arc<dyn SecretStore> = Arc::new(ShellSecretStore::new(storage.clone()));
    let options = CoreOptions {
        secret_store: Some(secrets.clone()),
        ..mobile_options(&FakeEngine::new(), &fx.dir("docs"), None)
    };
    let core = start(&fx, Platform::Ios, options);
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
    let saved = Settings::from_toml(&std::fs::read_to_string(fx.paths.settings_file()).unwrap()).unwrap();
    assert!(saved.keystore.enabled);
    assert_eq!(saved.keystore.file.as_deref(), Some(copy.as_path()));
    assert_eq!(saved.keystore.declared_type.as_deref(), Some("PKCS12"));
    assert_eq!(saved.keystore.password_storage, pdfover_config::PasswordStorage::Keychain);
    assert_eq!(secrets.get(&SecretId::keystore_store(&copy)).unwrap().as_deref().map(String::as_str), Some("test"));
    assert!(storage.0.lock().unwrap().keys().all(|k| k.starts_with("PDF-Over NG/")));
    // die Quelldatei bleibt (gehört der Shell); signieren ohne Passwortabfrage
    assert!(p12.is_file());
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    core.dispatch(UiEvent::OpenFiles { paths: vec![a] });
    let v = wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options, .. } if options.len() == 2))
        .await;
    let StepView::Backend { options, .. } = v.step else { unreachable!() };
    assert!(options.iter().any(|o| o.id == BackendId::Keystore && o.available));
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
    fx.write_settings("[signature]\nauto_position = true\n[privacy.acknowledged]\natrust_native = 1\n");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let p12 = fx.dir("import").join("k.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let core = start(
        &fx,
        Platform::Ios,
        mobile_options(&FakeEngine::new(), &fx.dir("docs"), Some(vec![FakeAtrust::new().backend()])),
    );
    core.dispatch(UiEvent::OpenFiles { paths: vec![a] });
    wait_form(&core, "Login", |f| matches!(f, NativeForm::Login { .. })).await;
    core.dispatch(UiEvent::ImportKeystore { path: p12, password: None, remember: false });
    let v = wait_for(&core, "abgelehnt", |v| v.keystore_import.is_some()).await;
    assert_eq!(v.keystore_import.unwrap().code, "ng-keystore-import-busy");
    assert!(form_request(&core.view()).is_some(), "Signiervorgang läuft weiter");
}

/// Import direkt aus der Signiermittel-Auswahl (Mobile, Keystore noch nicht eingerichtet): danach ist der
/// Keystore dort sofort verfügbar; ohne „merken“ wird das Passwort nicht gespeichert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pkcs12_import_from_the_backend_selection_offers_the_keystore() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let a = copy_pdf(PDF, &fx.dir("inbox"), "a.pdf");
    let p12 = fx.dir("import").join("test.p12");
    std::fs::copy(testdata(P12), &p12).unwrap();
    let options = CoreOptions { engine: Some(FakeEngine::new()), ..CoreOptions::isolated() };
    let core = start(&fx, Platform::Android, options);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a] });
    // nur ID Austria → automatisch gewählt → Datenschutzhinweis; Ablehnen führt zur Auswahl
    wait_for(&core, "Hinweis", |v| matches!(&v.step, StepView::Backend { privacy_notice: Some(_), .. })).await;
    core.dispatch(UiEvent::AcknowledgePrivacy { accepted: false });
    wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { privacy_notice: None, .. })).await;
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
