//! `pdfover-backend-localbku` – lokale Bürgerkartenumgebung (docs/analyse/02 §4; Plan §4.9).
//!
//! - Endpunkt `http://127.0.0.1:3495/http-security-layer-request`, nur HTTP, nur IPv4-Loopback,
//!   **kein Proxy**; User-Agent `PDF-Over NG v<Version>`.
//! - Erkennung per TCP-Probe alle 5 s (nicht verfügbar) bzw. 30 s (verfügbar) ([`Availability`]).
//! - Vor dem Signieren `NullOperationRequest`; enthält der `Server`-Header `trustDeskbasic` oder
//!   `asignSecurityLayer`, wird Base64 gesendet, sonst Upload ([`RequestMode`]). Schlägt die Probe
//!   fehl, fragt das Backend wie PDF-Over mit `error.LocalBKU` (Wiederholen/Abbrechen).
//! - `InfoboxReadRequest` und Base64-`CreateCMSSignatureRequest` urlencoded (`XMLRequest`),
//!   Upload als multipart mit `XMLRequest` **zuerst**, dann `fileupload` (`sign.pdf`,
//!   `application/octet-stream`).
//! - PIN-Eingabe, Kartenwahl und Anzeige übernimmt die BKU selbst (`Interaction::External`).
//! - HTTP ≠ 200 → [`BackendError::Http`] (Wiederholen möglich, wie PDF-Over `SigningState`);
//!   SL-`ErrorResponse` 6000/6001 → [`BackendError::Cancelled`], sonst
//!   [`BackendError::SecurityLayer`].
//!
//! Ownership: Paket **D**.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use pdfover_backend_api::{
    BackendError, BackendId, BackendSession, Capabilities, CertChain, CertNeed, Certificate, Environment, FactorSet,
    Interaction, InteractionBroker, InteractionError, Notice, NoticeAnswer, NoticeButtons, NoticeCode, NoticeKind,
    Progress, Qualification, ServerNeed, SessionContext, SignInput, SignJob, SignOutput, SignResult, SignatureBackend,
};
use pdfover_net::{Body, FormPart, HttpClient, NetConfig, NetError, ProxyConfig, Purpose, Request, Url};
use pdfover_sl12::{CreateCmsRequest, DocumentContent, HttpBody, PartOrder, SlRequest, SlResponse};

/// Standard-Endpunkt.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:3495/http-security-layer-request";
/// Probe-Adresse.
pub const PROBE_ADDR: &str = "127.0.0.1:3495";
/// Empfohlene `/Contents`-Reserve (Modell B, Plan §6.2).
pub const CMS_RESERVE: usize = 16 * 1024;

/// Übertragungsart des Dokuments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestMode {
    /// multipart: `XMLRequest` zuerst, dann `fileupload` (`sign.pdf`)
    Upload,
    /// urlencoded `XMLRequest` mit eingebettetem `Base64Content`
    Base64,
}

/// Modus aus dem `Server`-Header der NullOperation-Antwort.
pub fn mode_from_server_header(server: Option<&str>) -> RequestMode {
    match server {
        Some(s) if s.contains("trustDeskbasic") || s.contains("asignSecurityLayer") => RequestMode::Base64,
        _ => RequestMode::Upload,
    }
}

/// Konfiguration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBkuConfig {
    /// Endpunkt
    pub endpoint: Url,
    /// Probe-Adresse
    pub probe_addr: SocketAddr,
    /// User-Agent
    pub user_agent: String,
    /// Probe-Intervall, solange nicht verfügbar (5 s)
    pub probe_interval_unavailable: Duration,
    /// Probe-Intervall, solange verfügbar (30 s)
    pub probe_interval_available: Duration,
    /// Timeout je Request (die BKU wartet auf die PIN – großzügig)
    pub request_timeout: Duration,
}

impl LocalBkuConfig {
    /// Standardwerte.
    pub fn new(app_version: &str) -> Self {
        Self {
            endpoint: Url::parse(DEFAULT_ENDPOINT).expect("konstante URL"),
            probe_addr: PROBE_ADDR.parse().expect("konstante Adresse"),
            user_agent: format!("PDF-Over NG v{app_version}"),
            probe_interval_unavailable: Duration::from_secs(5),
            probe_interval_available: Duration::from_secs(30),
            request_timeout: Duration::from_secs(600),
        }
    }

    /// Netz-Konfiguration: nie ein Proxy, eigener User-Agent.
    fn net(&self) -> NetConfig {
        NetConfig {
            proxy: ProxyConfig::None,
            user_agent: Some(self.user_agent.clone()),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Some(self.request_timeout),
            ..NetConfig::default()
        }
    }
}

/// Timeout der TCP-Probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// Hintergrund-Probe der Erreichbarkeit (F-51).
#[derive(Debug, Clone)]
pub struct Availability {
    state: Arc<std::sync::atomic::AtomicBool>,
    stop: Arc<StopOnDrop>,
}

/// Beendet die Probe, wenn die letzte [`Availability`]-Kopie verschwindet.
#[derive(Debug)]
struct StopOnDrop {
    stop: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Availability {
    /// Startet die Probe: als Tokio-Task, wenn eine Laufzeit läuft, sonst als Hintergrund-Thread.
    /// Die erste Probe erfolgt sofort.
    pub fn spawn(config: &LocalBkuConfig) -> Availability {
        let state = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let addr = config.probe_addr;
        let (unavailable, available) = (config.probe_interval_unavailable, config.probe_interval_available);
        let task = match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                let state = Arc::clone(&state);
                Some(handle.spawn(async move {
                    loop {
                        let up = pdfover_net::tcp_probe(addr, PROBE_TIMEOUT).await;
                        state.store(up, Ordering::SeqCst);
                        tokio::time::sleep(if up { available } else { unavailable }).await;
                    }
                }))
            }
            Err(_) => {
                let state = Arc::clone(&state);
                let stop = Arc::clone(&stop);
                std::thread::Builder::new()
                    .name("LocalBKUProbe".into())
                    .spawn(move || {
                        while !stop.load(Ordering::SeqCst) {
                            let up = std::net::TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok();
                            state.store(up, Ordering::SeqCst);
                            let wait = if up { available } else { unavailable };
                            let start = std::time::Instant::now();
                            while start.elapsed() < wait && !stop.load(Ordering::SeqCst) {
                                std::thread::sleep(Duration::from_millis(50).min(wait));
                            }
                        }
                    })
                    .expect("Probe-Thread");
                None
            }
        };
        Availability { state, stop: Arc::new(StopOnDrop { stop, task }) }
    }

    /// Letzter bekannter Zustand.
    pub fn is_available(&self) -> bool {
        self.state.load(std::sync::atomic::Ordering::SeqCst)
    }
}

fn net_error(e: NetError) -> BackendError {
    match e {
        NetError::Timeout => BackendError::Timeout,
        NetError::Config(m) => BackendError::Internal(m),
        other => BackendError::Network(other.to_string()),
    }
}

fn to_net_body(body: HttpBody) -> Body {
    match body {
        HttpBody::UrlEncoded(fields) => Body::Form(fields),
        HttpBody::Multipart(parts) => Body::Multipart(
            parts
                .into_iter()
                .map(|p| FormPart { name: p.name, filename: p.filename, content_type: p.content_type, data: p.data })
                .collect(),
        ),
    }
}

/// `NullOperationRequest` senden und Modus bestimmen (HTTP ≠ 200 → `error.LocalBKU`).
pub async fn null_operation(client: &pdfover_net::HttpClient, endpoint: &Url) -> Result<RequestMode, BackendError> {
    let body = to_net_body(pdfover_sl12::http_body(&SlRequest::NullOperation, None, PartOrder::XmlFirst));
    let response = client
        .execute(Request::post(endpoint.clone(), body))
        .await
        .map_err(|e| BackendError::Unavailable(format!("lokale BKU nicht erreichbar: {e}")))?;
    if response.status != 200 {
        return Err(BackendError::Unavailable(format!("lokale BKU antwortet mit HTTP {}", response.status)));
    }
    Ok(mode_from_server_header(response.header("server")))
}

/// Backend.
#[derive(Debug)]
pub struct LocalBkuBackend {
    config: LocalBkuConfig,
    availability: Option<Availability>,
}

impl LocalBkuBackend {
    /// Neues Backend (optional mit laufender Probe).
    pub fn new(config: LocalBkuConfig, availability: Option<Availability>) -> Self {
        Self { config, availability }
    }

    /// Konfiguration.
    pub fn config(&self) -> &LocalBkuConfig {
        &self.config
    }
}

async fn cancellable<F: std::future::Future>(ctx: &SessionContext, future: F) -> Result<F::Output, BackendError> {
    let cancel = ctx.cancel.clone();
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(BackendError::Cancelled),
        out = future => Ok(out),
    }
}

#[async_trait]
impl SignatureBackend for LocalBkuBackend {
    fn id(&self) -> BackendId {
        BackendId::LocalBku
    }

    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        let reachable = env.localbku_reachable || self.availability.as_ref().is_some_and(Availability::is_available);
        if !env.platform.is_desktop() || !reachable || !env.allows(BackendId::LocalBku) {
            return None;
        }
        Some(Capabilities {
            input: SignInput::Document,
            output: SignOutput::Cms,
            certificate: CertNeed::Required,
            interaction: Interaction::External,
            server: ServerNeed::None,
            max_batch: 1,
            login_once_per_batch: false,
            document_leaves_device: false,
            qualification: Qualification::Qes,
            factors: FactorSet { card: true, ..FactorSet::default() },
            cms_reserve: Some(CMS_RESERVE),
        })
    }

    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        let client = HttpClient::new(&self.config.net(), Purpose::LocalBku).map_err(net_error)?;
        loop {
            match cancellable(&ctx, null_operation(&client, &self.config.endpoint)).await? {
                Ok(mode) => {
                    tracing::debug!(?mode, "lokale BKU erreichbar");
                    return Ok(Box::new(LocalBkuSession { mode, client, config: self.config.clone(), ui, ctx }));
                }
                Err(error) => {
                    tracing::warn!(%error, "lokale BKU nicht erreichbar");
                    if ctx.headless {
                        return Err(error);
                    }
                    let notice = Notice {
                        kind: NoticeKind::Error,
                        code: NoticeCode::LocalBkuNotReachable,
                        text: None,
                        buttons: NoticeButtons::RetryCancel,
                    };
                    match cancellable(&ctx, ui.notice(notice)).await? {
                        Ok(NoticeAnswer::Retry | NoticeAnswer::Ok) => continue,
                        Ok(NoticeAnswer::Cancel) | Err(InteractionError::Cancelled) => {
                            return Err(BackendError::Cancelled);
                        }
                        Err(InteractionError::Unsupported) => return Err(error),
                    }
                }
            }
        }
    }
}

/// Offene Sitzung.
pub struct LocalBkuSession {
    mode: RequestMode,
    client: HttpClient,
    config: LocalBkuConfig,
    ui: Arc<dyn InteractionBroker>,
    ctx: SessionContext,
}

impl std::fmt::Debug for LocalBkuSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalBkuSession")
            .field("mode", &self.mode)
            .field("endpoint", &self.config.endpoint.as_str())
            .finish_non_exhaustive()
    }
}

impl LocalBkuSession {
    /// Übertragungsart aus der NullOperation-Probe.
    pub fn mode(&self) -> RequestMode {
        self.mode
    }

    async fn send(&self, request: &SlRequest, upload: Option<&[u8]>) -> Result<SlResponse, BackendError> {
        let body = to_net_body(pdfover_sl12::http_body(request, upload, PartOrder::XmlFirst));
        let response = cancellable(&self.ctx, self.client.execute(Request::post(self.config.endpoint.clone(), body)))
            .await?
            .map_err(net_error)?;
        if response.status != 200 {
            return Err(BackendError::Http { status: response.status, reason: response.reason.clone() });
        }
        match pdfover_sl12::parse_response(response.text().as_bytes()) {
            Ok(SlResponse::Error(e)) if e.is_user_cancel() => Err(BackendError::Cancelled),
            Ok(SlResponse::Error(e)) => Err(BackendError::SecurityLayer { code: e.code, info: e.info }),
            Ok(other) => Ok(other),
            Err(e) => Err(BackendError::Protocol(format!("SL-Antwort der lokalen BKU nicht lesbar: {e}"))),
        }
    }
}

#[async_trait]
impl BackendSession for LocalBkuSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        self.ui.progress(Progress::ReadingCertificate);
        match self.send(&SlRequest::InfoboxReadCertificates, None).await? {
            SlResponse::InfoboxRead(r) => {
                let der = r
                    .signature_certificate()
                    .ok_or_else(|| BackendError::Protocol("InfoboxReadResponse ohne SecureSignatureKeypair".into()))?;
                let cert = Certificate::from_der(der)
                    .map_err(|e| BackendError::Protocol(format!("Signaturzertifikat nicht lesbar: {e}")))?;
                Ok(CertChain::new(cert))
            }
            _ => Err(BackendError::Protocol("unerwartete SL-Antwort auf InfoboxRead".into())),
        }
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        let total = jobs.len();
        let mut results = Vec::with_capacity(total);
        for (index, job) in jobs.into_iter().enumerate() {
            if total > 1 {
                self.ui.progress(Progress::Document { index: index + 1, total });
            }
            let SignJob::Document(job) = job else {
                return Err(BackendError::Internal("die lokale BKU signiert nur Dokumente (Modell B)".into()));
            };
            let data = job
                .source
                .blacked_out_bytes()
                .map_err(|e| BackendError::Internal(format!("vorbereitetes Dokument nicht lesbar: {e}")))?;
            let range = job.source.excluded_range();
            drop(job);
            let excluded = pdfover_sl12::ExcludedByteRange { from: range.from, to: range.to };
            self.ui.progress(Progress::Signing);
            let response = match self.mode {
                RequestMode::Upload => {
                    let request =
                        SlRequest::CreateCmsSignature(CreateCmsRequest::new(DocumentContent::Upload, excluded));
                    self.send(&request, Some(&data)).await?
                }
                RequestMode::Base64 => {
                    let request =
                        SlRequest::CreateCmsSignature(CreateCmsRequest::new(DocumentContent::Base64(data), excluded));
                    self.send(&request, None).await?
                }
            };
            match response {
                SlResponse::CreateCmsSignature(r) => results.push(SignResult::Cms(r.cms)),
                _ => return Err(BackendError::Protocol("unerwartete SL-Antwort auf CreateCMSSignature".into())),
            }
        }
        self.ui.progress(Progress::Done);
        Ok(results)
    }

    async fn cancel(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_header_selects_mode() {
        assert_eq!(mode_from_server_header(Some("trustDeskbasic/1.0")), RequestMode::Base64);
        assert_eq!(mode_from_server_header(Some("a.sign asignSecurityLayer")), RequestMode::Base64);
        assert_eq!(mode_from_server_header(Some("MOCCA")), RequestMode::Upload);
        assert_eq!(mode_from_server_header(None), RequestMode::Upload);
        // Groß-/Kleinschreibung zählt (Java `contains`)
        assert_eq!(mode_from_server_header(Some("trustdeskbasic")), RequestMode::Upload);
    }

    #[test]
    fn defaults() {
        let c = LocalBkuConfig::new("0.1.0");
        assert_eq!(c.endpoint.as_str(), DEFAULT_ENDPOINT);
        assert_eq!(c.probe_addr.to_string(), PROBE_ADDR);
        assert_eq!(c.user_agent, "PDF-Over NG v0.1.0");
        assert_eq!(c.probe_interval_unavailable, Duration::from_secs(5));
        assert_eq!(c.probe_interval_available, Duration::from_secs(30));
        let net = c.net();
        assert_eq!(net.proxy, ProxyConfig::None);
        assert_eq!(net.user_agent.as_deref(), Some("PDF-Over NG v0.1.0"));
    }

    #[test]
    fn capabilities_only_on_reachable_desktop() {
        let backend = LocalBkuBackend::new(LocalBkuConfig::new("0.1.0"), None);
        let mut env = Environment::current();
        env.platform = pdfover_backend_api::Platform::Linux;
        assert!(backend.capabilities(&env).is_none(), "nicht erreichbar");
        env.localbku_reachable = true;
        let caps = backend.capabilities(&env).unwrap();
        assert_eq!(caps.interaction, Interaction::External);
        assert_eq!(caps.certificate, CertNeed::Required);
        assert!(!caps.document_leaves_device);
        assert!(caps.factors.card);
        // headless ist erlaubt (die BKU hat ihre eigene Oberfläche)
        env.headless = true;
        assert!(backend.capabilities(&env).is_some());
        env.platform = pdfover_backend_api::Platform::Android;
        assert!(backend.capabilities(&env).is_none());
        env.platform = pdfover_backend_api::Platform::Windows;
        env.allowed_by_policy = Some(vec![BackendId::Keystore]);
        assert!(backend.capabilities(&env).is_none());
    }
}
