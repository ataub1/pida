//! Gemeinsame Test-Hilfen: Fixtures, SL-Antworten, Backend-Aufbau, gemockter FIDO2-Provider.
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use pida_backend_api::testing::MemoryDocument;
use pida_backend_api::{CancelToken, DocumentJob, ExcludedByteRange, SessionContext, SignJob, WindowHandle};
use pida_backend_atrust::poll::PollConfig;
use pida_backend_atrust::{AtrustConfig, AtrustNativeBackend, Fido2Assertion, Fido2Error, Fido2Provider, Fido2Request};
use pida_net::testing::{MockResponse, MockServer};
use pida_net::{NetConfig, ProxyConfig};

/// Pfadpräfix des SL-Endpunkts.
pub const BASE: &str = "/mobile/https-security-layer-request/";
pub const SL_NS: &str = "http://www.buergerkarte.at/namespaces/securitylayer/1.2#";

pub fn testdata(rel: &str) -> Vec<u8> {
    let path = format!("{}/../../testdata/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Fixture mit Platzhaltern `{{KEY}}`.
pub fn fixture(name: &str, subs: &[(&str, &str)]) -> String {
    let mut text = String::from_utf8(testdata(&format!("backend-atrust/{name}"))).unwrap();
    for (k, v) in subs {
        text = text.replace(&format!("{{{{{k}}}}}"), v);
    }
    text
}

pub fn page(name: &str, subs: &[(&str, &str)]) -> MockResponse {
    MockResponse::html(fixture(name, subs))
}

pub fn cert_der() -> Vec<u8> {
    testdata("keystores/test-rsa2048.crt.der")
}

pub fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub fn infobox_xml() -> MockResponse {
    MockResponse::xml(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><sl:InfoboxReadResponse xmlns:sl=\"{SL_NS}\"><sl:AssocArrayData><sl:Pair Key=\"SecureSignatureKeypair\"><sl:Base64Content>{}</sl:Base64Content></sl:Pair></sl:AssocArrayData></sl:InfoboxReadResponse>",
        b64(&cert_der())
    ))
    .with_header("Date", "Wed, 14 Oct 2026 10:00:00 GMT")
}

pub const FAKE_CMS: &[u8] = &[0x30, 0x80, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02];

pub fn cms_xml() -> MockResponse {
    cms_xml_with(FAKE_CMS)
}

pub fn cms_xml_with(cms: &[u8]) -> MockResponse {
    MockResponse::body(
        "application/xml; charset=UTF-8",
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><CreateCMSSignatureResponse xmlns=\"{SL_NS}\"><CMSSignature>{}</CMSSignature></CreateCMSSignatureResponse>",
            b64(cms)
        ),
    )
}

pub fn sl_error_xml(code: u32) -> MockResponse {
    MockResponse::xml(format!(
        "<sl:ErrorResponse xmlns:sl=\"{SL_NS}\"><sl:ErrorCode>{code}</sl:ErrorCode><sl:Info>Fehler {code}</sl:Info></sl:ErrorResponse>"
    ))
}

pub fn error_buttons(back: bool, cancel: bool) -> String {
    let mut s = String::new();
    if back {
        s.push_str("<input type=\"submit\" name=\"ctl00$Main$Button_Back\" id=\"Button_Back\" value=\"Zurück\" />");
    }
    if cancel {
        s.push_str(
            "<input type=\"submit\" name=\"ctl00$Main$Button_Cancel\" id=\"Button_Cancel\" value=\"Abbrechen\" />",
        );
    }
    s
}

/// Schnelle Poll-Zeiten für Tests.
pub fn fast_poll() -> PollConfig {
    PollConfig {
        total_timeout: Duration::from_secs(5),
        error_backoff: Duration::from_millis(50),
        after_finish: Duration::from_millis(10),
    }
}

pub fn config(server: &MockServer) -> AtrustConfig {
    let mut cfg = AtrustConfig::new(server.url(&format!("{BASE}default.aspx")));
    cfg.net = NetConfig { proxy: ProxyConfig::None, ..NetConfig::default() };
    cfg.poll = fast_poll();
    cfg
}

pub fn backend(server: &MockServer, tweak: impl FnOnce(&mut AtrustConfig)) -> AtrustNativeBackend {
    let mut cfg = config(server);
    tweak(&mut cfg);
    AtrustNativeBackend::new(cfg)
}

pub fn ctx() -> SessionContext {
    SessionContext { language: Default::default(), batch_size: 1, cancel: CancelToken::new(), headless: false }
}

/// Vorbereitetes Dokument (Blackout bereits angewendet).
pub fn document() -> (Vec<u8>, SignJob) {
    let mut pdf = b"%PDF-1.7\n1 0 obj<</Type/Sig/Contents <".to_vec();
    let from = pdf.len() as u64 - 1;
    pdf.extend(std::iter::repeat_n(0u8, 32));
    pdf.extend_from_slice(b">>>endobj\n%%EOF\n");
    let to = from + 33;
    // Blackout wie PDF-AS: die ganze Lücke inklusive `<` und `>` ist 0x00
    for b in &mut pdf[from as usize..=to as usize] {
        *b = 0;
    }
    let doc =
        MemoryDocument { name: "test.pdf".into(), blacked_out: pdf.clone(), excluded: ExcludedByteRange { from, to } };
    (pdf, SignJob::Document(DocumentJob { source: Arc::new(doc), expected_signer: None }))
}

/// Broker, der jede Maske erst nach `delay` beantwortet (Nebenläufigkeit mit dem Long-Poll).
pub struct SlowBroker {
    pub inner: pida_backend_api::testing::ScriptedBroker,
    pub delay: Duration,
}

#[async_trait::async_trait]
impl pida_backend_api::InteractionBroker for SlowBroker {
    async fn native_form(
        &self,
        form: pida_backend_api::NativeForm,
    ) -> Result<pida_backend_api::FormAnswer, pida_backend_api::InteractionError> {
        tokio::time::sleep(self.delay).await;
        self.inner.native_form(form).await
    }
    async fn secret(
        &self,
        prompt: pida_backend_api::SecretPrompt,
    ) -> Result<zeroize::Zeroizing<String>, pida_backend_api::InteractionError> {
        self.inner.secret(prompt).await
    }
    async fn notice(
        &self,
        notice: pida_backend_api::Notice,
    ) -> Result<pida_backend_api::NoticeAnswer, pida_backend_api::InteractionError> {
        self.inner.notice(notice).await
    }
    fn progress(&self, progress: pida_backend_api::Progress) {
        self.inner.progress(progress)
    }
    fn event(&self, event: pida_backend_api::BackendEvent) {
        self.inner.event(event)
    }
    fn open_external(&self, url: &str) -> Result<(), pida_backend_api::InteractionError> {
        self.inner.open_external(url)
    }
}

/// Zählt Aufrufe je Schlüssel (für Zustandsautomaten in Mock-Handlern).
#[derive(Debug, Default)]
pub struct Counter(AtomicUsize);

impl Counter {
    pub fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
    pub fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

/// Gemockter FIDO2-Provider mit Aufrufprotokoll.
#[derive(Debug)]
pub struct FakeFido2 {
    pub available: bool,
    pub results: Mutex<Vec<Result<Fido2Assertion, Fido2Error>>>,
    pub requests: Mutex<Vec<Fido2Request>>,
    pub cancels: AtomicUsize,
    pub delay: Duration,
}

impl FakeFido2 {
    pub fn new(results: Vec<Result<Fido2Assertion, Fido2Error>>) -> Arc<Self> {
        Arc::new(Self {
            available: true,
            results: Mutex::new(results),
            requests: Mutex::new(Vec::new()),
            cancels: AtomicUsize::new(0),
            delay: Duration::from_millis(20),
        })
    }

    pub fn assertion() -> Fido2Assertion {
        Fido2Assertion {
            id: "AQID".into(),
            raw_id: vec![1, 2, 3],
            kind: "public-key".into(),
            authenticator_data: vec![0xaa; 37],
            client_data_json: br#"{"type":"webauthn.get","challenge":"AAEC","origin":"https://service.a-trust.at","crossOrigin":false}"#.to_vec(),
            signature: vec![0x30, 0x44, 0xff],
            user_handle: None,
        }
    }
}

impl Fido2Provider for FakeFido2 {
    fn is_available(&self) -> bool {
        self.available
    }

    fn get_assertion(
        &self,
        request: &Fido2Request,
        _parent: Option<WindowHandle>,
    ) -> Result<Fido2Assertion, Fido2Error> {
        self.requests.lock().unwrap().push(request.clone());
        std::thread::sleep(self.delay);
        let mut results = self.results.lock().unwrap();
        if results.is_empty() { Err(Fido2Error::Device("keine Antwort vorgesehen".into())) } else { results.remove(0) }
    }

    fn cancel(&self) {
        self.cancels.fetch_add(1, Ordering::SeqCst);
    }
}

/// Eingespeiste Zeit (ADR 0004 Nachtrag 19): Startzeitpunkt plus verstellbarer Versatz.
#[derive(Debug)]
pub struct FakeClock {
    start: std::time::Instant,
    offset_ms: std::sync::atomic::AtomicU64,
}

impl FakeClock {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { start: std::time::Instant::now(), offset_ms: std::sync::atomic::AtomicU64::new(0) })
    }

    /// Zeit vorstellen (z. B. „die Nutzerin lässt die Anmeldemaske fünf Minuten offen“).
    pub fn advance(&self, by: Duration) {
        self.offset_ms.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }
}

impl pida_backend_atrust::Clock for FakeClock {
    fn now(&self) -> std::time::Instant {
        self.start + Duration::from_millis(self.offset_ms.load(Ordering::SeqCst))
    }
}

/// Broker, der vor jeder Antwort auf die **Anmeldemaske** die eingespeiste Zeit um `login_delay` vorstellt.
pub struct ClockBroker {
    pub inner: pida_backend_api::testing::ScriptedBroker,
    pub clock: Arc<FakeClock>,
    pub login_delay: Duration,
}

#[async_trait::async_trait]
impl pida_backend_api::InteractionBroker for ClockBroker {
    async fn native_form(
        &self,
        form: pida_backend_api::NativeForm,
    ) -> Result<pida_backend_api::FormAnswer, pida_backend_api::InteractionError> {
        if matches!(form, pida_backend_api::NativeForm::Login { .. }) {
            self.clock.advance(self.login_delay);
        }
        self.inner.native_form(form).await
    }
    async fn secret(
        &self,
        prompt: pida_backend_api::SecretPrompt,
    ) -> Result<zeroize::Zeroizing<String>, pida_backend_api::InteractionError> {
        self.inner.secret(prompt).await
    }
    async fn notice(
        &self,
        notice: pida_backend_api::Notice,
    ) -> Result<pida_backend_api::NoticeAnswer, pida_backend_api::InteractionError> {
        self.inner.notice(notice).await
    }
    fn progress(&self, progress: pida_backend_api::Progress) {
        self.inner.progress(progress)
    }
    fn event(&self, event: pida_backend_api::BackendEvent) {
        self.inner.event(event)
    }
    fn open_external(&self, url: &str) -> Result<(), pida_backend_api::InteractionError> {
        self.inner.open_external(url)
    }
}

/// Fängt alle Log-Ereignisse und Span-Felder (alle Stufen) als Text, um zu prüfen, dass Zugangsdaten nie im Log
/// stehen (ADR 0001 §4 Regel 9). Nur mit `tracing` selbst, ohne weitere Abhängigkeit.
#[derive(Debug, Clone, Default)]
pub struct LogCapture(pub Arc<Mutex<Vec<String>>>);

impl LogCapture {
    /// Für den aktuellen Thread einschalten (der Test läuft auf der Laufzeit des aktuellen Threads).
    pub fn install() -> (Self, tracing::subscriber::DefaultGuard) {
        let capture = LogCapture::default();
        let guard = tracing::subscriber::set_default(capture.clone());
        (capture, guard)
    }

    pub fn text(&self) -> String {
        self.0.lock().unwrap().join("\n")
    }

    fn push(&self, line: String) {
        self.0.lock().unwrap().push(line);
    }
}

struct FieldText<'a>(&'a mut String);

impl tracing::field::Visit for FieldText<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!(" {}={value:?}", field.name()));
    }
}

impl tracing::Subscriber for LogCapture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut line = format!("span {}", span.metadata().name());
        span.record(&mut FieldText(&mut line));
        self.push(line);
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        let mut line = String::from("record");
        values.record(&mut FieldText(&mut line));
        self.push(line);
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut line = format!("{} {}", event.metadata().level(), event.metadata().target());
        event.record(&mut FieldText(&mut line));
        self.push(line);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
