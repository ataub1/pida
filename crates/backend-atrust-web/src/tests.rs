use std::collections::VecDeque;
use std::sync::Mutex;

use base64::Engine;
use pida_backend_api::DocumentJob;
use pida_backend_api::testing::{MemoryDocument, ScriptedBroker};

use super::*;

/// Rückkanal aus dem Drehbuch: merkt sich die Befehle, liefert die Antworten der Reihe nach.
#[derive(Debug, Default)]
struct ScriptedRelay {
    answers: Mutex<VecDeque<Result<Vec<u8>, RelayError>>>,
    requests: Mutex<Vec<String>>,
    closed: Mutex<Vec<CloseOutcome>>,
}

impl ScriptedRelay {
    fn new(answers: Vec<Result<Vec<u8>, RelayError>>) -> Arc<Self> {
        Arc::new(Self { answers: Mutex::new(answers.into()), ..Self::default() })
    }
}

#[async_trait]
impl SlRelay for ScriptedRelay {
    async fn exchange(&self, request: String) -> Result<Vec<u8>, RelayError> {
        self.requests.lock().unwrap().push(request);
        self.answers.lock().unwrap().pop_front().unwrap_or(Err(RelayError::Failed("Drehbuch zu Ende".into())))
    }

    fn close(&self, outcome: CloseOutcome) {
        self.closed.lock().unwrap().push(outcome);
    }
}

const SL: &str = "http://www.buergerkarte.at/namespaces/securitylayer/1.2#";

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn infobox(cert: &[u8]) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><sl:InfoboxReadResponse xmlns:sl="{SL}"><sl:AssocArrayData><sl:Pair Key="SecureSignatureKeypair"><sl:Base64Content>{}</sl:Base64Content></sl:Pair></sl:AssocArrayData></sl:InfoboxReadResponse>"#,
        b64(cert)
    )
    .into_bytes()
}

fn cms_response(cms: &[u8]) -> Vec<u8> {
    format!(r#"<sl:CreateCMSSignatureResponse xmlns:sl="{SL}"><sl:CMSSignature>{}</sl:CMSSignature></sl:CreateCMSSignatureResponse>"#, b64(cms))
        .into_bytes()
}

fn error(code: u32) -> Vec<u8> {
    format!(r#"<sl:ErrorResponse xmlns:sl="{SL}"><sl:ErrorCode>{code}</sl:ErrorCode><sl:Info>x</sl:Info></sl:ErrorResponse>"#).into_bytes()
}

fn ctx() -> SessionContext {
    SessionContext { language: Default::default(), batch_size: 1, cancel: CancelToken::new(), headless: false }
}

fn job(data: &[u8]) -> SignJob {
    SignJob::Document(DocumentJob {
        source: Arc::new(MemoryDocument {
            name: "a.pdf".into(),
            blacked_out: data.to_vec(),
            excluded: pida_backend_api::ExcludedByteRange { from: 2, to: 5 },
        }),
        expected_signer: None,
    })
}

fn cert() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.crt.der")).unwrap()
}

async fn open(relay: Arc<ScriptedRelay>) -> Box<dyn BackendSession> {
    let backend = AtrustWebBackend::new(relay);
    backend.open(ctx(), Arc::new(ScriptedBroker::new([]))).await.unwrap()
}

#[tokio::test]
async fn chain_certificate_then_documents_then_done() {
    let relay = ScriptedRelay::new(vec![Ok(infobox(&cert())), Ok(cms_response(b"CMS_A")), Ok(cms_response(b"CMS_B"))]);
    let mut s = open(Arc::clone(&relay)).await;
    let chain = s.certificate().await.unwrap();
    assert_eq!(chain.signer.as_der(), cert().as_slice());
    assert_eq!(s.sign_one(job(b"%PDF-1"), CancelToken::new()).await.unwrap(), SignResult::Cms(b"CMS_A".to_vec()));
    assert_eq!(s.sign_one(job(b"%PDF-2"), CancelToken::new()).await.unwrap(), SignResult::Cms(b"CMS_B".to_vec()));
    assert!(relay.closed.lock().unwrap().is_empty(), "Kette bleibt offen, solange die Sitzung lebt");
    drop(s);
    assert_eq!(*relay.closed.lock().unwrap(), vec![CloseOutcome::Done]);
    let requests = relay.requests.lock().unwrap();
    assert!(requests[0].contains("InfoboxReadRequest"), "{}", requests[0]);
    // Dokument als Base64 im Befehl (die Antwort an A-Trust ist der Befehl selbst, kein Upload möglich)
    assert!(
        requests[1].contains("CreateCMSSignatureRequest") && requests[1].contains(&b64(b"%PDF-1")),
        "{}",
        requests[1]
    );
    assert!(requests[1].contains("<From>2</From>") && requests[1].contains("<To>5</To>"), "{}", requests[1]);
}

#[tokio::test]
async fn user_cancel_and_closed_overlay_are_cancelled() {
    for answer in [Ok(error(6001)), Err(RelayError::Cancelled)] {
        let relay = ScriptedRelay::new(vec![answer]);
        let mut s = open(Arc::clone(&relay)).await;
        assert_eq!(s.certificate().await.unwrap_err(), BackendError::Cancelled);
        assert_eq!(relay.closed.lock().unwrap().first(), Some(&CloseOutcome::Aborted));
    }
}

#[tokio::test]
async fn other_errors_map_like_the_native_backend() {
    let relay = ScriptedRelay::new(vec![Ok(error(4000))]);
    let mut s = open(relay).await;
    assert!(matches!(
        s.sign_one(job(b"x"), CancelToken::new()).await,
        Err(BackendError::SecurityLayer { code: 4000, .. })
    ));
    let relay = ScriptedRelay::new(vec![Err(RelayError::Timeout)]);
    let mut s = open(relay).await;
    assert_eq!(s.certificate().await.unwrap_err(), BackendError::Timeout);
    let relay = ScriptedRelay::new(vec![Ok(b"<html/>".to_vec())]);
    let mut s = open(relay).await;
    assert!(matches!(s.certificate().await, Err(BackendError::Protocol(_))));
}

#[tokio::test]
async fn unused_session_closes_nothing() {
    let relay = ScriptedRelay::new(vec![]);
    let s = open(Arc::clone(&relay)).await;
    drop(s);
    assert!(relay.closed.lock().unwrap().is_empty());
}

#[test]
fn capabilities_embedded_web_with_relay() {
    let backend = AtrustWebBackend::new(ScriptedRelay::new(vec![]));
    let env = Environment::current();
    let caps = backend.capabilities(&env).unwrap();
    assert_eq!(caps.interaction, Interaction::EmbeddedWeb);
    assert_eq!(caps.server, ServerNeed::DataUrlRelay);
    assert!(caps.login_once_per_batch && caps.factors.fido2);
    assert!(backend.capabilities(&Environment { headless: true, ..env.clone() }).is_none());
    assert!(backend.capabilities(&Environment { allowed_by_policy: Some(vec![BackendId::Keystore]), ..env }).is_none());
}
