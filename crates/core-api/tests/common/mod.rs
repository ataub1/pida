//! Test-Bausteine: Fake-PAdES-Engine (aus `pida_core_api::testing`), Fake-Backend im Stil von
//! Szenario A (Masken über den Broker des Kerns), Warte-Helfer.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use pida_backend_api::{
    BackendError, BackendEvent, BackendId, BackendSession, Capabilities, CertChain, CertNeed, Environment, FactorSet,
    FormAnswer, FormContext, Interaction, InteractionBroker, InteractionError, NativeForm, Qualification, ServerNeed,
    SessionContext, SignInput, SignJob, SignOutput, SignResult, SignatureBackend,
};
use pida_backend_keystore::LoadedKey;
use pida_cades::{CmsProfile, DigestAlg, RawSigner, SigningTime};
use pida_config::{Overlay, Paths};
#[allow(unused_imports)]
pub use pida_core_api::testing::{FakeEngine, verify_fake_output};
use pida_core_api::{Core, CoreConfig, CoreOptions, ViewState};
use time::OffsetDateTime;

pub const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata");

pub fn testdata(rel: &str) -> PathBuf {
    Path::new(TESTDATA).join(rel)
}

/// Kopiert ein Test-PDF in einen Ordner.
pub fn copy_pdf(rel: &str, dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let dst = dir.join(name);
    std::fs::copy(testdata(rel), &dst).unwrap();
    dst
}

pub fn test_key() -> Arc<LoadedKey> {
    let bytes = std::fs::read(testdata("keystores/test-rsa2048.p12")).unwrap();
    Arc::new(LoadedKey::open_pkcs12(&bytes, "test", None).unwrap())
}

// ------------------------------------------------------------------------------------------------
// Fake-Backend (Szenario-A-artig: Masken über den Broker, Modell B)
// ------------------------------------------------------------------------------------------------

pub fn atrust_caps() -> Capabilities {
    Capabilities {
        input: SignInput::Document,
        output: SignOutput::Cms,
        certificate: CertNeed::OnlyForVisibleBlock,
        interaction: Interaction::NativeForms,
        server: ServerNeed::None,
        max_batch: 1,
        login_once_per_batch: true,
        document_leaves_device: true,
        qualification: Qualification::Qes,
        factors: FactorSet { app: true, sms: true, qr: true, fido2: false, card: false },
        cms_reserve: Some(16 * 1024),
    }
}

#[derive(Debug)]
pub struct FakeAtrust {
    pub key: Arc<LoadedKey>,
    pub opened: AtomicUsize,
    pub cancelled: AtomicUsize,
    /// Aufrufe von `certificate()` (InfoboxRead mit eigener Anmeldung)
    pub certificates: AtomicUsize,
}

impl FakeAtrust {
    pub fn new() -> Arc<FakeAtrust> {
        Arc::new(FakeAtrust {
            key: test_key(),
            opened: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            certificates: AtomicUsize::new(0),
        })
    }

    pub fn backend(self: &Arc<Self>) -> Arc<dyn SignatureBackend> {
        Arc::new(FakeAtrustBackend(self.clone()))
    }
}

/// Backend-Hülle (Zähler bleiben im Test sichtbar).
#[derive(Debug)]
pub struct FakeAtrustBackend(pub Arc<FakeAtrust>);

#[async_trait]
impl SignatureBackend for FakeAtrustBackend {
    fn id(&self) -> BackendId {
        BackendId::AtrustNative
    }
    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        (!env.headless && env.allows(BackendId::AtrustNative)).then(atrust_caps)
    }
    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        self.0.opened.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeAtrustSession { backend: self.0.clone(), ui, _ctx: ctx }))
    }
}

struct FakeAtrustSession {
    backend: Arc<FakeAtrust>,
    ui: Arc<dyn InteractionBroker>,
    _ctx: SessionContext,
}

fn interaction(e: InteractionError) -> BackendError {
    match e {
        InteractionError::Cancelled => BackendError::Cancelled,
        InteractionError::Unsupported => BackendError::Unavailable("keine Oberfläche".into()),
    }
}

impl FakeAtrustSession {
    async fn login(&self) -> Result<(), BackendError> {
        let form = NativeForm::Login {
            prefill_username: None,
            remember: false,
            ctx: FormContext { signature_data_url: Some("https://service.a-trust.at/x".into()), ..Default::default() },
        };
        match self.ui.native_form(form).await.map_err(interaction)? {
            FormAnswer::Login { username, password, remember } if password.len() >= 5 => {
                self.ui.event(BackendEvent::LoginSucceeded { username, remember });
                Ok(())
            }
            _ => Err(BackendError::Protocol("unerwartete Antwort".into())),
        }
    }
}

#[async_trait]
impl BackendSession for FakeAtrustSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        self.backend.certificates.fetch_add(1, Ordering::SeqCst);
        self.login().await?;
        Ok(self.backend.key.chain().clone())
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
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
                SigningTime(OffsetDateTime::now_utc()),
            )
            .map_err(|e| BackendError::Internal(e.to_string()))?;
            out.push(SignResult::Cms(cms));
        }
        Ok(out)
    }

    async fn cancel(&mut self) {
        self.backend.cancelled.fetch_add(1, Ordering::SeqCst);
    }
}

/// Backend mit Programmfehler: `capabilities` bricht ab (Panik im Kern-Task, F-106).
#[derive(Debug)]
pub struct PanickingBackend;

#[async_trait]
impl SignatureBackend for PanickingBackend {
    fn id(&self) -> BackendId {
        BackendId::Keystore
    }
    fn capabilities(&self, _env: &Environment) -> Option<Capabilities> {
        panic!("Programmfehler im Backend (Test)")
    }
    async fn open(
        &self,
        _ctx: SessionContext,
        _ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        Err(BackendError::Internal("nie".into()))
    }
}

// ------------------------------------------------------------------------------------------------
// Kern starten, warten
// ------------------------------------------------------------------------------------------------

pub struct Fixture {
    pub tmp: tempfile::TempDir,
    pub paths: Paths,
}

impl Fixture {
    pub fn new() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::under(&tmp.path().join("app"));
        Fixture { tmp, paths }
    }

    pub fn dir(&self, name: &str) -> PathBuf {
        let d = self.tmp.path().join(name);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub fn write_settings(&self, toml: &str) {
        std::fs::create_dir_all(&self.paths.config_dir).unwrap();
        std::fs::write(self.paths.settings_file(), toml).unwrap();
    }

    pub fn config(&self, overlay: Overlay, headless: bool) -> CoreConfig {
        CoreConfig {
            paths: self.paths.clone(),
            overlay,
            platform: pida_backend_api::Platform::Linux,
            pdfium_dir: None,
            headless,
            fido2: None,
            app_version: "0.1.0-test".into(),
        }
    }

    pub fn start(&self, overlay: Overlay, headless: bool, opts: CoreOptions) -> Core {
        Core::new_with(self.config(overlay, headless), opts).expect("Kern startet")
    }
}

/// Wartet, bis der ViewState `pred` erfüllt (höchstens 20 s).
pub async fn wait_for(core: &Core, what: &str, pred: impl Fn(&ViewState) -> bool) -> ViewState {
    let mut rx = core.subscribe();
    let r = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            {
                let v = rx.borrow_and_update().clone();
                if pred(&v) {
                    return v;
                }
            }
            if rx.changed().await.is_err() {
                panic!("Kern beendet");
            }
        }
    })
    .await;
    match r {
        Ok(v) => v,
        Err(_) => panic!("Zeitüberschreitung beim Warten auf {what}: {:#?}", core.view()),
    }
}
