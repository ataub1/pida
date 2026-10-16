//! `SignatureBackend`/`BackendSession` des Keystores mit skriptgesteuertem Broker: Passwortquellen
//! (Overlay → Ablage → Abfrage), Wiederholung nach falschem Passwort, Speicherarten, Dialoge für
//! fehlende Datei/Alias, headless, Abbruch, Signieraufträge (Modell A) und Capabilities.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use pida_backend_api::testing::{BrokerCall, MemoryDocument, ScriptedBroker, ScriptedReply};
use pida_backend_api::{
    BackendError, BackendEvent, BackendId, BackendSession, CancelToken, CertNeed, DigestJob, DocumentJob, Environment,
    ExcludedByteRange, FormAnswer, Interaction, InteractionBroker, InteractionError, NativeForm, Notice, NoticeAnswer,
    NoticeButtons, NoticeCode, Progress, SecretKind, SecretPrompt, SessionContext, SignInput, SignJob, SignOutput,
    SignResult, SignatureBackend,
};
use pida_backend_keystore::{
    KeystoreBackend, KeystoreConfig, MemoryPasswordCache, NoPasswordCache, PasswordCache, PasswordKind, PasswordStorage,
};
use pida_cades::{
    AssembleOptions, CmsProfile, DigestAlg, Expect, ExpectedSigner, SignatureAlg, SignedAttrsBuilder, SigningTime,
    assemble, verify_detached, verify_raw,
};
use zeroize::Zeroizing;

fn td(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel)
}

fn ctx(headless: bool) -> SessionContext {
    SessionContext { language: Default::default(), batch_size: 1, cancel: CancelToken::new(), headless }
}

fn config(path: PathBuf) -> KeystoreConfig {
    KeystoreConfig::new(path)
}

fn secret_prompts(calls: &[BrokerCall]) -> Vec<SecretPrompt> {
    calls
        .iter()
        .filter_map(|c| match c {
            BrokerCall::Secret(p) => Some(p.clone()),
            _ => None,
        })
        .collect()
}

fn env_configured() -> Environment {
    Environment { keystore_configured: true, ..Environment::current() }
}

#[test]
fn capabilities_model_a() {
    let b = KeystoreBackend::new(config(td("keystores/test-rsa2048.p12")));
    assert_eq!(b.id(), BackendId::Keystore);
    assert!(b.capabilities(&Environment::current()).is_none(), "nicht konfiguriert → nicht verfügbar");
    let forbidden = Environment { allowed_by_policy: Some(vec![BackendId::AtrustNative]), ..env_configured() };
    assert!(b.capabilities(&forbidden).is_none(), "Richtlinie");
    let c = b.capabilities(&env_configured()).unwrap();
    assert_eq!(c.input, SignInput::Digest { algs: vec![DigestAlg::Sha256, DigestAlg::Sha512] });
    assert_eq!(c.output, SignOutput::RawSignature);
    assert_eq!(c.certificate, CertNeed::Required);
    assert_eq!(c.interaction, Interaction::Secret);
    assert_eq!(c.max_batch, u16::MAX);
    assert!(c.login_once_per_batch);
    assert!(!c.document_leaves_device);
    assert_eq!(c.cms_reserve, None);
    // auch headless/mobil verfügbar (Passwort über Overlay)
    for platform in [pida_backend_api::Platform::Ios, pida_backend_api::Platform::Android] {
        assert!(b.capabilities(&Environment { platform, headless: true, ..env_configured() }).is_some());
    }
}

#[tokio::test]
async fn overlay_password_needs_no_prompt_and_is_not_cached() {
    let mut cfg = config(td("keystores/test-ecp256.p12"));
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let cache = cfg.cache.clone();
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([]));
    let mut s = b.open(ctx(false), broker.clone()).await.unwrap();
    let chain = s.certificate().await.unwrap();
    assert_eq!(chain.signer.as_der(), std::fs::read(td("keystores/test-ecp256.crt.der")).unwrap().as_slice());
    assert!(secret_prompts(&broker.calls()).is_empty());
    // Overlay-Passwörter werden nicht abgelegt (PDF-Over: kein Speichern bei CLI-Override)
    assert!(cache.get(&td("keystores/test-ecp256.p12"), PasswordKind::Store).is_none());
}

#[tokio::test]
async fn prompt_then_memory_cache_on_second_open() {
    let cfg = config(td("keystores/test-rsa2048.p12"));
    let path = cfg.path.clone();
    let cache = cfg.cache.clone();
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]));
    b.open(ctx(false), broker.clone()).await.unwrap();
    let prompts = secret_prompts(&broker.calls());
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].kind, SecretKind::KeystorePassword);
    assert_eq!(prompts[0].subject, "test-rsa2048.p12", "nur Dateiname, kein Pfad");
    assert!(!prompts[0].retry);
    assert_eq!(cache.get(&path, PasswordKind::Store).unwrap().as_str(), "test");
    // zweites Öffnen: aus der Ablage, ohne Abfrage
    let broker2 = Arc::new(ScriptedBroker::new([]));
    b.open(ctx(false), broker2.clone()).await.unwrap();
    assert!(secret_prompts(&broker2.calls()).is_empty());
}

#[tokio::test]
async fn wrong_password_is_asked_again_with_retry_flag() {
    let b = KeystoreBackend::new(config(td("keystores/test-rsa2048-legacy-3des.p12")));
    let broker = Arc::new(ScriptedBroker::new([
        ScriptedReply::Secret("falsch".into()),
        ScriptedReply::Secret("auch falsch".into()),
        ScriptedReply::Secret("test".into()),
    ]));
    b.open(ctx(false), broker.clone()).await.unwrap();
    let retries: Vec<bool> = secret_prompts(&broker.calls()).iter().map(|p| p.retry).collect();
    assert_eq!(retries, [false, true, true]);
}

#[tokio::test]
async fn wrong_cached_password_is_forgotten() {
    let cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.cache.put(&cfg.path, PasswordKind::Store, "veraltet");
    let cache = cfg.cache.clone();
    let path = cfg.path.clone();
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]));
    b.open(ctx(false), broker.clone()).await.unwrap();
    let prompts = secret_prompts(&broker.calls());
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].retry, "Abfrage nach falschem gespeichertem Passwort");
    assert_eq!(cache.get(&path, PasswordKind::Store).unwrap().as_str(), "test");
}

#[tokio::test]
async fn storage_none_ignores_cache() {
    let mut cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.storage = PasswordStorage::None;
    let cache = Arc::new(MemoryPasswordCache::default());
    cache.put(&cfg.path, PasswordKind::Store, "test");
    cfg.cache = cache.clone();
    let path = cfg.path.clone();
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]));
    b.open(ctx(false), broker.clone()).await.unwrap();
    assert_eq!(secret_prompts(&broker.calls()).len(), 1, "Speicherart none fragt jedes Mal");
    cache.forget(&path, PasswordKind::Store);
    b.open(ctx(false), Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]))).await.unwrap();
    assert!(cache.get(&path, PasswordKind::Store).is_none(), "none legt nichts ab");
}

#[tokio::test]
async fn keychain_storage_uses_given_cache_and_key_password_from_cache() {
    // Speicherart „disk“ = Schlüsselbund über den Adapter der Integration (hier ein RAM-Cache)
    let mut cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.storage = PasswordStorage::Keychain;
    cfg.cache.put(&cfg.path, PasswordKind::Store, "falsch");
    cfg.cache.put(&cfg.path, PasswordKind::Key, "test"); // alte Konfiguration: KEYPASS = richtiges Passwort
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([]));
    // Store-Passwort falsch, Schlüsselpasswort öffnet (Ein-Passwort-Abbildung) – keine Abfrage
    b.open(ctx(false), broker.clone()).await.unwrap();
    assert!(secret_prompts(&broker.calls()).is_empty());
}

#[tokio::test]
async fn cancel_at_prompt_and_unsupported_broker() {
    let b = KeystoreBackend::new(config(td("keystores/test-rsa2048.p12")));
    let r = b.open(ctx(false), Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]))).await;
    assert_eq!(r.err(), Some(BackendError::Cancelled));
    // Broker ohne Antwort (z. B. Oberfläche ohne Passwortmaske)
    let r = b.open(ctx(false), Arc::new(ScriptedBroker::new([]))).await;
    assert!(matches!(r.err(), Some(BackendError::Unavailable(_))));
}

#[tokio::test]
async fn headless_rules() {
    // ohne Passwort → nicht verfügbar, ohne Abfrage
    let b = KeystoreBackend::new(config(td("keystores/test-rsa2048.p12")));
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]));
    assert!(matches!(b.open(ctx(true), broker.clone()).await.err(), Some(BackendError::Unavailable(_))));
    assert!(broker.calls().is_empty());
    // falsches Overlay-Passwort → WrongPassword statt Endlosschleife
    let mut cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.store_password = Some(Zeroizing::new("falsch".into()));
    let b = KeystoreBackend::new(cfg);
    assert_eq!(b.open(ctx(true), Arc::new(ScriptedBroker::new([]))).await.err(), Some(BackendError::WrongPassword));
    // fehlende Datei → Fehler ohne Dialog
    let mut cfg = config(PathBuf::from("/nicht/vorhanden/k.p12"));
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([]));
    let e = b.open(ctx(true), broker.clone()).await.err().unwrap();
    assert!(
        matches!(&e, BackendError::KeystoreDetail { message: m, .. } if m.contains("k.p12") && !m.contains("/nicht/")),
        "{e:?}"
    );
    assert!(broker.calls().is_empty());
}

#[tokio::test]
async fn missing_file_dialog_retry_and_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spaeter.p12");
    let mut cfg = config(path.clone());
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let b = KeystoreBackend::new(cfg);
    // Abbrechen
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Notice(NoticeAnswer::Cancel)]));
    assert_eq!(b.open(ctx(false), broker.clone()).await.err(), Some(BackendError::Cancelled));
    let calls = broker.calls();
    assert!(matches!(
        &calls[0],
        BrokerCall::Notice(Notice { code: NoticeCode::KeystoreFileMissing, buttons: NoticeButtons::RetryCancel, text: Some(t), .. })
            if t == "spaeter.p12"
    ));
    // Wiederholen, nachdem die Datei (z. B. USB-Stick) da ist
    let broker = Arc::new(FileOnNotice {
        inner: ScriptedBroker::new([ScriptedReply::Notice(NoticeAnswer::Retry)]),
        src: td("keystores/test-rsa2048.p12"),
        dst: path.clone(),
    });
    b.open(ctx(false), broker.clone()).await.unwrap();
    assert_eq!(broker.inner.calls().len(), 1);
}

/// Broker, der beim Anzeigen der Meldung die Datei anlegt (Nutzerin steckt den Datenträger an).
struct FileOnNotice {
    inner: ScriptedBroker,
    src: PathBuf,
    dst: PathBuf,
}

#[async_trait]
impl InteractionBroker for FileOnNotice {
    async fn native_form(&self, form: NativeForm) -> Result<FormAnswer, InteractionError> {
        self.inner.native_form(form).await
    }
    async fn secret(&self, prompt: SecretPrompt) -> Result<Zeroizing<String>, InteractionError> {
        self.inner.secret(prompt).await
    }
    async fn notice(&self, notice: Notice) -> Result<NoticeAnswer, InteractionError> {
        std::fs::copy(&self.src, &self.dst).unwrap();
        self.inner.notice(notice).await
    }
    fn progress(&self, progress: Progress) {
        self.inner.progress(progress)
    }
    fn event(&self, event: BackendEvent) {
        self.inner.event(event)
    }
    fn open_external(&self, url: &str) -> Result<(), InteractionError> {
        self.inner.open_external(url)
    }
}

#[tokio::test]
async fn missing_alias_dialog() {
    let mut cfg = config(td("backend-keystore/java-two-keys.p12"));
    cfg.alias = Some("dritter".into());
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let b = KeystoreBackend::new(cfg);
    let broker = Arc::new(ScriptedBroker::new([
        ScriptedReply::Notice(NoticeAnswer::Retry),
        ScriptedReply::Notice(NoticeAnswer::Cancel),
    ]));
    assert_eq!(b.open(ctx(false), broker.clone()).await.err(), Some(BackendError::Cancelled));
    let notices: Vec<_> = broker
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            BrokerCall::Notice(n) => Some(n),
            _ => None,
        })
        .collect();
    assert_eq!(notices.len(), 2, "Wiederholen versucht denselben Alias erneut");
    assert!(notices.iter().all(|n| n.code == NoticeCode::KeystoreAliasMissing && n.text.as_deref() == Some("dritter")));
    assert!(secret_prompts(&broker.calls()).is_empty(), "Passwort wird bei Wiederholen behalten");
    // headless → Fehler
    let e = b.open(ctx(true), Arc::new(ScriptedBroker::new([]))).await.err().unwrap();
    assert!(matches!(&e, BackendError::KeystoreDetail { message: m, .. } if m.contains("dritter")));
    // gültiger Alias ohne Groß-/Kleinschreibung
    let mut cfg = config(td("backend-keystore/java-two-keys.p12"));
    cfg.alias = Some("Zweiter".into());
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let mut s = KeystoreBackend::new(cfg).open(ctx(true), Arc::new(ScriptedBroker::new([]))).await.unwrap();
    assert!(s.certificate().await.unwrap().signer.subject_string().contains("P-256"));
}

#[tokio::test]
async fn terminal_errors_without_prompt() {
    // JKS/JCEKS → Konvertierung, vor jeder Passwortabfrage
    for (rel, fmt) in [("backend-keystore/java-rsa2048.jks", "JKS"), ("backend-keystore/java-rsa2048.jceks", "JCEKS")] {
        let b = KeystoreBackend::new(config(td(rel)));
        let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]));
        let e = b.open(ctx(false), broker.clone()).await.err().unwrap();
        assert_eq!(e, BackendError::UnsupportedKeystoreFormat { format: fmt.into() });
        assert_eq!(e.code(), "backend-keystore-format");
        assert!(broker.calls().is_empty());
    }
    // MD5-PBE → Neu-Export, ohne Passwortabfrage
    let b = KeystoreBackend::new(config(td("backend-keystore/openssl-md5-des.p12")));
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Secret("test".into())]));
    let e = b.open(ctx(false), broker.clone()).await.err().unwrap();
    assert!(
        matches!(&e, BackendError::KeystoreDetail { message: m, .. } if m.contains("pbeWithMD5AndDES-CBC")),
        "{e:?}"
    );
    assert_eq!((e.code(), e.detail_code()), ("backend-keystore", Some("keystore-unsupported-pbe")));
    assert!(broker.calls().is_empty());
    // getrennte Passwörter: klare Meldung statt Passwortschleife
    let mut cfg = config(td("backend-keystore/java-separate-keypass.p12"));
    cfg.store_password = Some(Zeroizing::new("test".into()));
    cfg.key_password = Some(Zeroizing::new("schluessel".into()));
    let e = KeystoreBackend::new(cfg).open(ctx(false), Arc::new(ScriptedBroker::new([]))).await.err().unwrap();
    assert!(
        matches!(&e, BackendError::KeystoreDetail { message: m, .. } if m.contains("einheitlichem Passwort")),
        "{e:?}"
    );
    assert_eq!(e.detail_code(), Some("keystore-different-key-password"));
    // nicht unterstützter Schlüssel
    let mut cfg = config(td("backend-keystore/openssl-dsa.p12"));
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let e = KeystoreBackend::new(cfg).open(ctx(false), Arc::new(ScriptedBroker::new([]))).await.err().unwrap();
    assert!(matches!(&e, BackendError::KeystoreDetail { message: m, .. } if m.contains("DSA")), "{e:?}");
    assert_eq!(e.detail_code(), Some("keystore-unsupported-key"));
}

async fn open_rsa() -> (Box<dyn BackendSession>, Arc<ScriptedBroker>) {
    let mut cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let broker = Arc::new(ScriptedBroker::new([]));
    let s = KeystoreBackend::new(cfg).open(ctx(false), broker.clone()).await.unwrap();
    (s, broker)
}

fn digest_job(alg: DigestAlg, data: &[u8]) -> SignJob {
    SignJob::Digest(DigestJob { display_name: "a.pdf".into(), alg, digest: alg.digest(data).value })
}

#[tokio::test]
async fn sign_batch_of_digest_jobs() {
    let (mut s, broker) = open_rsa().await;
    let chain = s.certificate().await.unwrap();
    let jobs = vec![
        digest_job(DigestAlg::Sha256, b"eins"),
        digest_job(DigestAlg::Sha384, b"zwei"),
        digest_job(DigestAlg::Sha512, b"drei"),
    ];
    let results = s.sign(jobs).await.unwrap();
    assert_eq!(results.len(), 3);
    for ((res, alg), data) in results.iter().zip([DigestAlg::Sha256, DigestAlg::Sha384, DigestAlg::Sha512]).zip([
        &b"eins"[..],
        b"zwei",
        b"drei",
    ]) {
        let SignResult::Raw { signature, chain: c } = res else { panic!("Rohsignatur erwartet") };
        assert_eq!(signature.alg, SignatureAlg::RsaPkcs1v15(alg));
        assert_eq!(c, &chain);
        verify_raw(&chain.signer, &alg.digest(data), signature).unwrap();
    }
    let progress: Vec<_> = broker
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            BrokerCall::Progress(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(progress[0], Progress::Signing);
    assert!(progress.contains(&Progress::Document { index: 3, total: 3 }));
}

#[tokio::test]
async fn sign_rejects_document_sha1_and_bad_length() {
    let (mut s, _) = open_rsa().await;
    let doc = MemoryDocument {
        name: "a.pdf".into(),
        blacked_out: b"%PDF".to_vec(),
        excluded: ExcludedByteRange { from: 1, to: 2 },
    };
    let job = SignJob::Document(DocumentJob { source: Arc::new(doc), expected_signer: None });
    assert!(matches!(s.sign(vec![job]).await, Err(BackendError::Internal(_))));
    assert!(matches!(s.sign(vec![digest_job(DigestAlg::Sha1, b"x")]).await, Err(BackendError::Internal(_))));
    let bad = SignJob::Digest(DigestJob { display_name: "a.pdf".into(), alg: DigestAlg::Sha256, digest: vec![0; 20] });
    assert!(matches!(s.sign(vec![bad]).await, Err(BackendError::Internal(_))));
    assert_eq!(s.sign(vec![]).await.unwrap(), vec![]);
}

#[tokio::test]
async fn cancel_token_and_session_cancel() {
    let mut cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let b = KeystoreBackend::new(cfg);
    let c = ctx(false);
    let token = c.cancel.clone();
    let mut s = b.open(c, Arc::new(ScriptedBroker::new([]))).await.unwrap();
    token.cancel();
    assert_eq!(s.sign(vec![digest_job(DigestAlg::Sha256, b"x")]).await.err(), Some(BackendError::Cancelled));
    // bereits abgebrochen → open bricht ab
    let c = ctx(false);
    c.cancel.cancel();
    assert_eq!(b.open(c, Arc::new(ScriptedBroker::new([]))).await.err(), Some(BackendError::Cancelled));
    // cancel() gibt den Schlüssel frei (idempotent)
    let (mut s, _) = open_rsa().await;
    s.cancel().await;
    s.cancel().await;
    assert_eq!(s.certificate().await.err(), Some(BackendError::Cancelled));
    assert_eq!(s.sign(vec![digest_job(DigestAlg::Sha256, b"x")]).await.err(), Some(BackendError::Cancelled));
}

/// Broker, dessen Passwortmaske nie antwortet (Nutzerin tippt noch) – Abbruch über das Token.
#[derive(Debug, Default)]
struct HangingBroker;

#[async_trait]
impl InteractionBroker for HangingBroker {
    async fn native_form(&self, _: NativeForm) -> Result<FormAnswer, InteractionError> {
        Err(InteractionError::Unsupported)
    }
    async fn secret(&self, _: SecretPrompt) -> Result<Zeroizing<String>, InteractionError> {
        std::future::pending().await
    }
    async fn notice(&self, _: Notice) -> Result<NoticeAnswer, InteractionError> {
        Err(InteractionError::Unsupported)
    }
    fn progress(&self, _: Progress) {}
    fn event(&self, _: BackendEvent) {}
    fn open_external(&self, _: &str) -> Result<(), InteractionError> {
        Ok(())
    }
}

#[tokio::test]
async fn cancel_while_prompt_is_open() {
    let b = KeystoreBackend::new(config(td("keystores/test-rsa2048.p12")));
    let c = ctx(false);
    let token = c.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        token.cancel();
    });
    let r = tokio::time::timeout(std::time::Duration::from_secs(5), b.open(c, Arc::new(HangingBroker))).await;
    assert_eq!(r.expect("Abbruch muss die Maske beenden").err(), Some(BackendError::Cancelled));
}

#[tokio::test]
async fn model_a_flow_through_backend_session() {
    // Ablauf ADR 0001 §8.3: certificate → select_algorithm → Digest der ByteRange-Daten →
    // SignedAttrs → attrs.digest(alg) → sign → assemble → verify_detached (I3)
    let mut cfg = config(td("backend-keystore/openssl-p521-chain.p12"));
    cfg.alias = Some("p521".into());
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let mut s = KeystoreBackend::new(cfg).open(ctx(false), Arc::new(ScriptedBroker::new([]))).await.unwrap();
    let chain = s.certificate().await.unwrap();
    let alg = pida_cades::select_algorithm(&chain.signer.key_type().unwrap()).unwrap().digest();
    assert_eq!(alg, DigestAlg::Sha512);
    let content = b"ByteRange-Daten (NUR TEST)".repeat(50);
    for profile in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
        let md = alg.digest(&content);
        let attrs = SignedAttrsBuilder::new(profile, &md, &chain.signer, SigningTime(time::OffsetDateTime::now_utc()))
            .build()
            .unwrap();
        let job = SignJob::Digest(DigestJob { display_name: "a.pdf".into(), alg, digest: attrs.digest(alg).value });
        let res = s.sign(vec![job]).await.unwrap();
        let SignResult::Raw { signature, chain: c } = &res[0] else { panic!() };
        assert_eq!(signature.alg.digest(), alg);
        let cms = assemble(&attrs, signature, c, &AssembleOptions { profile, include_intermediates: true }).unwrap();
        let expect = Expect { signer: ExpectedSigner::Certificate(chain.signer.clone()), ..Expect::default() };
        let report = verify_detached(&cms, &mut content.as_slice(), &expect).unwrap();
        assert_eq!(report.certificates.len(), 2, "Endzertifikat + Zwischen-CA");
    }
}

#[test]
fn debug_has_no_password() {
    let mut cfg = config(td("keystores/test-rsa2048.p12"));
    cfg.store_password = Some(Zeroizing::new("supergeheim".into()));
    cfg.key_password = Some(Zeroizing::new("auchgeheim".into()));
    cfg.cache = Arc::new(NoPasswordCache);
    let dbg = format!("{:?}", KeystoreBackend::new(cfg));
    assert!(!dbg.contains("supergeheim") && !dbg.contains("auchgeheim"), "{dbg}");
}
