//! Lokale BKU gegen einen Mock auf 127.0.0.1 (zufälliger Port). Orakel: der Mock signiert das
//! empfangene Dokument (ohne ExcludedByteRange) mit `openssl cms` und dem Testschlüssel; der Test
//! prüft die zurückgegebene CMS mit `openssl cms -verify` gegen die erwarteten Bytes. Ohne
//! `openssl` werden diese Prüfungen übersprungen.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use base64::Engine as _;
use pdfover_backend_api::testing::{BrokerCall, MemoryDocument, ScriptedBroker, ScriptedReply};
use pdfover_backend_api::{
    BackendError, CancelToken, DocumentJob, Environment, ExcludedByteRange, NoticeAnswer, NoticeButtons, NoticeCode,
    SessionContext, SignJob, SignResult, SignatureBackend,
};
use pdfover_backend_localbku::{
    Availability, LocalBkuBackend, LocalBkuConfig, RequestMode, mode_from_server_header, null_operation,
};
use pdfover_net::testing::{MockResponse, MockServer, RecordedRequest};
use pdfover_net::{HttpClient, NetConfig, ProxyConfig, Purpose};

const SL_NS: &str = "http://www.buergerkarte.at/namespaces/securitylayer/1.2#";
const PATH: &str = "/http-security-layer-request";

fn testdata(rel: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../testdata/{rel}", env!("CARGO_MANIFEST_DIR")))
}

fn b64(d: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(d)
}

fn ctx() -> SessionContext {
    SessionContext { language: Default::default(), batch_size: 1, cancel: CancelToken::new(), headless: false }
}

fn config(server: &MockServer) -> LocalBkuConfig {
    let mut c = LocalBkuConfig::new("0.1.0-test");
    c.endpoint = server.url(PATH);
    c.request_timeout = Duration::from_secs(30);
    c
}

fn xml_request(req: &RecordedRequest) -> String {
    req.part("XMLRequest").map(|p| p.text()).or_else(|| req.form_field("XMLRequest")).unwrap_or_default()
}

fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let start = s.find(a)? + a.len();
    Some(&s[start..start + s[start..].find(b)?])
}

fn infobox_response() -> MockResponse {
    let cert = std::fs::read(testdata("keystores/test-rsa2048.crt.der")).unwrap();
    MockResponse::xml(format!(
        "<sl:InfoboxReadResponse xmlns:sl=\"{SL_NS}\"><sl:AssocArrayData><sl:Pair Key=\"SecureSignatureKeypair\"><sl:Base64Content>{}</sl:Base64Content></sl:Pair></sl:AssocArrayData></sl:InfoboxReadResponse>",
        b64(&cert)
    ))
}

fn cms_response(der: &[u8]) -> MockResponse {
    MockResponse::xml(format!(
        "<sl:CreateCMSSignatureResponse xmlns:sl=\"{SL_NS}\"><sl:CMSSignature>{}</sl:CMSSignature></sl:CreateCMSSignatureResponse>",
        b64(der)
    ))
}

fn error_response(code: u32) -> MockResponse {
    MockResponse::xml(format!(
        "<sl:ErrorResponse xmlns:sl=\"{SL_NS}\"><sl:ErrorCode>{code}</sl:ErrorCode><sl:Info>Info {code}</sl:Info></sl:ErrorResponse>"
    ))
}

/// Testschlüssel als PEM (aus dem PKCS#12 extrahiert); `None` ohne openssl.
fn openssl_material(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let key = dir.join("key.pem");
    let ok = Command::new("openssl")
        .args(["pkcs12", "-in"])
        .arg(testdata("keystores/test-rsa2048.p12"))
        .args(["-nocerts", "-nodes", "-passin", "pass:test", "-out"])
        .arg(&key)
        .output()
        .ok()?
        .status
        .success();
    ok.then(|| (key, testdata("keystores/test-rsa2048.crt.pem")))
}

/// Was eine BKU tut: Inhalt ohne Lücke detached signieren (hier per openssl).
fn openssl_sign(content: &[u8], key: &Path, cert: &Path, dir: &Path) -> Vec<u8> {
    let input = dir.join(format!("content-{}.bin", content.len()));
    let output = dir.join(format!("cms-{}.der", content.len()));
    std::fs::write(&input, content).unwrap();
    let out = Command::new("openssl")
        .args(["cms", "-sign", "-binary", "-md", "sha256", "-outform", "DER", "-signer"])
        .arg(cert)
        .arg("-inkey")
        .arg(key)
        .arg("-in")
        .arg(&input)
        .arg("-out")
        .arg(&output)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    std::fs::read(&output).unwrap()
}

fn openssl_verify(cms: &[u8], content: &[u8], dir: &Path) -> bool {
    let c = dir.join("verify-content.bin");
    let s = dir.join("verify-cms.der");
    std::fs::write(&c, content).unwrap();
    std::fs::write(&s, cms).unwrap();
    Command::new("openssl")
        .args(["cms", "-verify", "-binary", "-inform", "DER", "-noverify", "-in"])
        .arg(&s)
        .arg("-content")
        .arg(&c)
        .arg("-out")
        .arg(dir.join("verify-out.bin")) // kein /dev/null: Windows-CI
        .output()
        .unwrap()
        .status
        .success()
}

/// Mock-BKU: NullOperation mit `Server`-Header, InfoboxRead, CreateCMS (signiert per openssl, falls
/// vorhanden, sonst Dummy-CMS).
async fn bku(server_header: &'static str, material: Option<(PathBuf, PathBuf)>, dir: PathBuf) -> MockServer {
    MockServer::start(move |req| {
        assert_eq!(req.method, "POST");
        assert_eq!(req.path(), PATH);
        let xml = xml_request(req);
        if xml.contains("NullOperationRequest") {
            return MockResponse::xml(format!("<sl:NullOperationResponse xmlns:sl=\"{SL_NS}\"/>"))
                .with_header("Server", server_header);
        }
        if xml.contains("<InfoboxReadRequest") {
            return infobox_response();
        }
        assert!(xml.contains("<CreateCMSSignatureRequest Structure=\"detached\""));
        let data = match req.part("fileupload") {
            Some(p) => {
                assert!(xml.contains("Reference=\"formdata:fileupload\""));
                p.data
            }
            None => base64::engine::general_purpose::STANDARD
                .decode(between(&xml, "<Base64Content>", "</Base64Content>").unwrap())
                .unwrap(),
        };
        let from: usize = between(&xml, "<From>", "</From>").unwrap().parse().unwrap();
        let to: usize = between(&xml, "<To>", "</To>").unwrap().parse().unwrap();
        assert!(data[from..=to].iter().all(|b| *b == 0), "Lücke ist genullt");
        let mut content = data[..from].to_vec();
        content.extend_from_slice(&data[to + 1..]);
        match &material {
            Some((key, cert)) => cms_response(&openssl_sign(&content, key, cert, &dir)),
            None => cms_response(&[0x30, 0x03, 0x02, 0x01, 0x01]),
        }
    })
    .await
}

fn document() -> (Vec<u8>, Vec<u8>, SignJob) {
    let mut pdf = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<</Type/Sig/Filter/Adobe.PPKLite/Contents <".to_vec();
    let from = pdf.len() as u64 - 1;
    pdf.extend(std::iter::repeat_n(0u8, 64));
    pdf.extend_from_slice(b">/ByteRange [0 1 2 3]>>\nendobj\n%%EOF\n");
    let to = from + 65;
    // Blackout wie PDF-AS: die ganze Lücke inklusive `<` und `>` ist 0x00
    for b in &mut pdf[from as usize..=to as usize] {
        *b = 0;
    }
    let mut signed_content = pdf[..from as usize].to_vec();
    signed_content.extend_from_slice(&pdf[to as usize + 1..]);
    let doc =
        MemoryDocument { name: "t.pdf".into(), blacked_out: pdf.clone(), excluded: ExcludedByteRange { from, to } };
    (pdf, signed_content, SignJob::Document(DocumentJob { source: Arc::new(doc), expected_signer: None }))
}

#[tokio::test]
async fn null_operation_request_and_mode_detection() {
    for (header, mode) in [
        ("trustDeskbasic/2.7", RequestMode::Base64),
        ("asignSecurityLayer 1.2", RequestMode::Base64),
        ("citizen-card-environment/1.2 MOCCA/1.3", RequestMode::Upload),
    ] {
        let server = bku(header, None, std::env::temp_dir()).await;
        let cfg =
            NetConfig { proxy: ProxyConfig::None, user_agent: Some("PDF-Over NG v9".into()), ..NetConfig::default() };
        let client = HttpClient::new(&cfg, Purpose::LocalBku).unwrap();
        assert_eq!(null_operation(&client, &server.url(PATH)).await.unwrap(), mode, "{header}");
        let req = &server.requests()[0];
        assert_eq!(req.header("content-type"), Some("application/x-www-form-urlencoded; charset=UTF-8"));
        assert_eq!(req.form_field("XMLRequest").as_deref(), Some(pdfover_sl12::NULL_OPERATION_REQUEST));
        assert_eq!(req.header("user-agent"), Some("PDF-Over NG v9"));
    }
    assert_eq!(mode_from_server_header(None), RequestMode::Upload);
}

#[tokio::test]
async fn upload_mode_full_flow_with_openssl_oracle() {
    let dir = tempfile::tempdir().unwrap();
    let material = openssl_material(dir.path());
    let server = bku("MOCCA", material.clone(), dir.path().to_path_buf()).await;
    let backend = LocalBkuBackend::new(config(&server), None);
    let broker = Arc::new(ScriptedBroker::new([]));
    let mut session = backend.open(ctx(), broker.clone()).await.unwrap();
    let chain = session.certificate().await.unwrap();
    assert_eq!(chain.signer.as_der(), std::fs::read(testdata("keystores/test-rsa2048.crt.der")).unwrap().as_slice());
    let (pdf, content, job) = document();
    let results = session.sign(vec![job]).await.unwrap();

    let reqs = server.requests();
    assert_eq!(reqs.len(), 3);
    assert!(reqs[1].form_field("XMLRequest").unwrap().contains("<InfoboxReadRequest"));
    let parts = reqs[2].multipart();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].name, "XMLRequest", "lokale BKU: XMLRequest zuerst");
    assert_eq!(parts[0].content_type.as_deref(), Some("text/plain; charset=UTF-8"));
    assert_eq!((parts[1].name.as_str(), parts[1].filename.as_deref()), ("fileupload", Some("sign.pdf")));
    assert_eq!(parts[1].content_type.as_deref(), Some("application/octet-stream"));
    assert_eq!(parts[1].data, pdf);
    assert!(reqs.iter().all(|r| r.header("user-agent") == Some("PDF-Over NG v0.1.0-test")));
    assert!(broker.calls().contains(&BrokerCall::Progress(pdfover_backend_api::Progress::Done)));

    let [SignResult::Cms(cms)] = results.as_slice() else { panic!("{results:?}") };
    if material.is_some() {
        assert!(openssl_verify(cms, &content, dir.path()), "CMS deckt genau die Bytes außerhalb der Lücke ab");
        let mut tampered = content.clone();
        tampered[3] ^= 1;
        assert!(!openssl_verify(cms, &tampered, dir.path()));
    } else {
        eprintln!("openssl fehlt – Orakel übersprungen");
    }
}

#[tokio::test]
async fn base64_mode_full_flow_with_openssl_oracle() {
    let dir = tempfile::tempdir().unwrap();
    let material = openssl_material(dir.path());
    let server = bku("trustDeskbasic", material.clone(), dir.path().to_path_buf()).await;
    let backend = LocalBkuBackend::new(config(&server), None);
    let mut session = backend.open(ctx(), Arc::new(ScriptedBroker::new([]))).await.unwrap();
    let (pdf, content, job) = document();
    let results = session.sign(vec![job]).await.unwrap();
    let req = &server.requests()[1];
    assert!(req.multipart().is_empty(), "Base64-Modus ist urlencoded");
    let xml = req.form_field("XMLRequest").unwrap();
    assert_eq!(between(&xml, "<Base64Content>", "</Base64Content>").unwrap(), b64(&pdf));
    let [SignResult::Cms(cms)] = results.as_slice() else { panic!() };
    if material.is_some() {
        assert!(openssl_verify(cms, &content, dir.path()));
    }
}

#[tokio::test]
async fn unreachable_bku_asks_retry_then_succeeds() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let a = attempts.clone();
    let server = MockServer::start(move |req| {
        if xml_request(req).contains("NullOperationRequest") && a.fetch_add(1, Ordering::SeqCst) == 0 {
            return MockResponse::status(500);
        }
        MockResponse::xml(format!("<sl:NullOperationResponse xmlns:sl=\"{SL_NS}\"/>"))
    })
    .await;
    let backend = LocalBkuBackend::new(config(&server), None);
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Notice(NoticeAnswer::Retry)]));
    backend.open(ctx(), broker.clone()).await.unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    let notices: Vec<_> =
        broker.calls().into_iter().filter_map(|c| if let BrokerCall::Notice(n) = c { Some(n) } else { None }).collect();
    assert_eq!(notices.len(), 1);
    assert_eq!(
        (notices[0].code.clone(), notices[0].buttons),
        (NoticeCode::LocalBkuNotReachable, NoticeButtons::RetryCancel)
    );
}

#[tokio::test]
async fn unreachable_bku_cancel_headless_and_unsupported() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let mut cfg = LocalBkuConfig::new("0.1.0");
    cfg.endpoint = pdfover_net::Url::parse(&format!("http://{addr}{PATH}")).unwrap();
    let backend = LocalBkuBackend::new(cfg, None);

    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Notice(NoticeAnswer::Cancel)]));
    assert!(matches!(backend.open(ctx(), broker).await, Err(BackendError::Cancelled)));

    let mut headless = ctx();
    headless.headless = true;
    let broker = Arc::new(ScriptedBroker::new([]));
    assert!(matches!(backend.open(headless, broker.clone()).await, Err(BackendError::Unavailable(_))));
    assert!(broker.calls().is_empty(), "headless ohne Rückfrage");

    let broker = Arc::new(ScriptedBroker::new([])); // notice → Unsupported
    assert!(matches!(backend.open(ctx(), broker).await, Err(BackendError::Unavailable(_))));
}

#[tokio::test]
async fn sl_and_http_errors() {
    let mode = Arc::new(AtomicUsize::new(0));
    let m = mode.clone();
    let server = MockServer::start(move |req| {
        if xml_request(req).contains("NullOperationRequest") {
            return MockResponse::xml(format!("<sl:NullOperationResponse xmlns:sl=\"{SL_NS}\"/>"));
        }
        match m.load(Ordering::SeqCst) {
            0 => error_response(6001),
            1 => error_response(6000),
            2 => error_response(4002),
            3 => MockResponse::status(500).with_body("BKU kaputt"),
            _ => MockResponse::html("<html>keine SL-Antwort</html>"),
        }
    })
    .await;
    let backend = LocalBkuBackend::new(config(&server), None);
    let mut session = backend.open(ctx(), Arc::new(ScriptedBroker::new([]))).await.unwrap();
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    mode.store(1, Ordering::SeqCst);
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    mode.store(2, Ordering::SeqCst);
    assert_eq!(
        session.certificate().await.unwrap_err(),
        BackendError::SecurityLayer { code: 4002, info: "Info 4002".into() }
    );
    mode.store(3, Ordering::SeqCst);
    let (_, _, job) = document();
    assert_eq!(
        session.sign(vec![job]).await.unwrap_err(),
        BackendError::Http { status: 500, reason: "Internal Server Error".into() }
    );
    mode.store(4, Ordering::SeqCst);
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::Protocol(_)));
    session.cancel().await;
}

#[tokio::test]
async fn cancel_token_aborts_waiting_for_pin() {
    let server = MockServer::start(|req| {
        if xml_request(req).contains("NullOperationRequest") {
            return MockResponse::xml(format!("<sl:NullOperationResponse xmlns:sl=\"{SL_NS}\"/>"));
        }
        infobox_response().with_delay(Duration::from_secs(10))
    })
    .await;
    let backend = LocalBkuBackend::new(config(&server), None);
    let context = ctx();
    let cancel = context.cancel.clone();
    let mut session = backend.open(context, Arc::new(ScriptedBroker::new([]))).await.unwrap();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
    });
    let started = std::time::Instant::now();
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn availability_probe_follows_the_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = LocalBkuConfig::new("0.1.0");
    cfg.probe_addr = listener.local_addr().unwrap();
    cfg.probe_interval_available = Duration::from_millis(50);
    cfg.probe_interval_unavailable = Duration::from_millis(50);
    let availability = Availability::spawn(&cfg);
    let wait_for = |want: bool, a: Availability| async move {
        for _ in 0..100 {
            if a.is_available() == want {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    };
    assert!(wait_for(true, availability.clone()).await, "erreichbar");
    let backend = LocalBkuBackend::new(cfg.clone(), Some(availability.clone()));
    let mut env = Environment::current();
    env.platform = pdfover_backend_api::Platform::MacOs;
    assert!(backend.capabilities(&env).is_some(), "Probe macht das Backend verfügbar");
    drop(listener);
    assert!(wait_for(false, availability.clone()).await, "nicht mehr erreichbar");
    assert!(backend.capabilities(&env).is_none());
}

#[test]
fn availability_without_runtime_uses_a_thread() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut cfg = LocalBkuConfig::new("0.1.0");
    cfg.probe_addr = listener.local_addr().unwrap();
    cfg.probe_interval_available = Duration::from_millis(30);
    cfg.probe_interval_unavailable = Duration::from_millis(30);
    let availability = Availability::spawn(&cfg);
    let mut ok = false;
    for _ in 0..100 {
        if availability.is_available() {
            ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ok);
    drop(availability); // beendet den Thread
}
