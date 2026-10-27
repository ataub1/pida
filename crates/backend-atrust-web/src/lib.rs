//! `pida-backend-atrust-web` – ID Austria im Browser (Konzept Webdienst, Variante E „A-Trust-Overlay“).
//!
//! Im Browser darf PIDA nicht mit `service.a-trust.at` sprechen (CORS) und soll die A-Trust-Seiten auch nicht
//! nachbilden. Stattdessen läuft der Dialog wie bei PDF-AS-Web über das Security-Layer-HTTPS-Binding mit
//! `DataURL` (SL-Bindings §3.2.2):
//!
//! 1. Die Seite schickt den ersten SL-Befehl per **Formular-POST** (`XMLRequest`, `DataURL`) in ein Overlay
//!    (iframe, sonst Popup) mit der echten A-Trust-Seite. Anmeldung, SMS-TAN, App-Bestätigung und FIDO2 laufen
//!    dort – PIDA sieht keine Zugangsdaten.
//! 2. A-Trust schickt die SL-Antwort (`XMLResponse`) an die `DataURL`, einen kleinen Rückkanal des PIDA-Servers.
//!    Der Rückkanal reicht sie an die Seite weiter und hält die Anfrage von A-Trust offen.
//! 3. Die Seite antwortet mit dem **nächsten Befehl** (`text/xml`), den der Rückkanal als Antwort an A-Trust
//!    zurückgibt: A-Trust führt ihn in derselben Sitzung aus – eine Anmeldung für Zertifikat und alle Dokumente.
//!    Zum Schluss beendet die Seite die Kette (Abschlussseite im Overlay).
//!
//! Dieses Crate ist der Teil im Kern: die Befehlsfolge `InfoboxRead` → `CreateCMSSignature` (Base64, ein Dokument
//! je Befehl) über einen [`SlRelay`]. Den Transport (Overlay, Rückkanal) stellt die Shell (`apps/web`).
//!
//! **Abweichung von der Konzept-Vorgabe (Entscheidung des Auftraggebers, 26.10.2026):** Weil die Antwort an A-Trust
//! der nächste Befehl ist, läuft das Dokument (Base64 im `CreateCMSSignatureRequest`) über den PIDA-Server – nur im
//! Speicher, sofort verworfen. Zugangsdaten erreichen den Server weiterhin nie.
//!
//! **Fehler:** Overlay geschlossen bzw. SL 6000/6001 → [`BackendError::Cancelled`]; andere SL-Fehler →
//! [`BackendError::SecurityLayer`]; Rückkanal nicht erreichbar → [`BackendError::Network`], Zeitablauf →
//! [`BackendError::Timeout`].

use std::sync::Arc;

use async_trait::async_trait;
use pida_backend_api::{
    BackendError, BackendId, BackendSession, CancelToken, Capabilities, CertChain, CertNeed, Certificate, Environment,
    FactorSet, Interaction, InteractionBroker, Progress, Qualification, ServerNeed, SessionContext, SignInput, SignJob,
    SignOutput, SignResult, SignatureBackend,
};
use pida_sl12::{CreateCmsRequest, DocumentContent, ExcludedByteRange, SlRequest, SlResponse};

/// Empfohlene `/Contents`-Reserve (wie das native A-Trust-Backend, Plan §6.2).
pub const CMS_RESERVE: usize = 16 * 1024;

/// Fehler des Transports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayError {
    /// Overlay geschlossen bzw. Abbruch in der Seite
    Cancelled,
    /// keine Antwort in der vorgesehenen Zeit
    Timeout,
    /// Rückkanal nicht erreichbar oder Kette abgerissen
    Failed(String),
}

/// Wie die Kette endet ([`SlRelay::close`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseOutcome {
    /// alle Befehle ausgeführt
    Done,
    /// abgebrochen bzw. fehlgeschlagen
    Aborted,
}

/// Transport der SL-Befehle (Shell): Der erste Aufruf von [`SlRelay::exchange`] öffnet das Overlay und schickt
/// den Befehl per Formular an A-Trust, jeder weitere antwortet A-Trust über den Rückkanal mit dem nächsten Befehl.
/// Ist die Kette abgerissen (Overlay geschlossen, Zeitablauf), beginnt der nächste Aufruf eine neue.
#[async_trait]
pub trait SlRelay: Send + Sync + std::fmt::Debug {
    /// Schickt `request` (SL-XML) und liefert die SL-Antwort (`XMLResponse`, Bytes).
    async fn exchange(&self, request: String) -> Result<Vec<u8>, RelayError>;

    /// Beendet die Kette (Abschlussseite im Overlay, Overlay schließen). Idempotent.
    fn close(&self, outcome: CloseOutcome);
}

/// Backend „ID Austria“ im Browser (Kennung [`BackendId::AtrustNative`]: für Oberfläche, Richtlinie und
/// Einstellungen ist es ID Austria).
#[derive(Debug, Clone)]
pub struct AtrustWebBackend {
    relay: Arc<dyn SlRelay>,
}

impl AtrustWebBackend {
    /// Neues Backend über `relay`.
    pub fn new(relay: Arc<dyn SlRelay>) -> Self {
        Self { relay }
    }
}

#[async_trait]
impl SignatureBackend for AtrustWebBackend {
    fn id(&self) -> BackendId {
        BackendId::AtrustNative
    }

    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        if env.headless || !env.allows(BackendId::AtrustNative) {
            return None;
        }
        Some(Capabilities {
            input: SignInput::Document,
            output: SignOutput::Cms,
            certificate: CertNeed::OnlyForVisibleBlock,
            interaction: Interaction::EmbeddedWeb,
            server: ServerNeed::DataUrlRelay,
            max_batch: 1,
            // eine Anmeldung bei A-Trust für die ganze Kette (Zertifikat und alle Dokumente)
            login_once_per_batch: true,
            document_leaves_device: true,
            qualification: Qualification::Qes,
            // alle Faktoren auf der A-Trust-Seite, FIDO2 im Browser (WebAuthn mit der echten Origin)
            factors: FactorSet { app: true, sms: true, qr: true, fido2: true, card: false },
            cms_reserve: Some(CMS_RESERVE),
        })
    }

    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        if ctx.headless {
            return Err(BackendError::Unavailable("ID Austria braucht die A-Trust-Seite (nicht headless)".into()));
        }
        Ok(Box::new(AtrustWebSession { relay: Arc::clone(&self.relay), ui, ctx, used: false }))
    }
}

/// Offene Sitzung: eine Befehlskette bei A-Trust.
pub struct AtrustWebSession {
    relay: Arc<dyn SlRelay>,
    ui: Arc<dyn InteractionBroker>,
    ctx: SessionContext,
    /// mindestens ein Befehl gesendet (sonst braucht `close` nichts zu tun)
    used: bool,
}

impl std::fmt::Debug for AtrustWebSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtrustWebSession").field("relay", &self.relay).field("used", &self.used).finish_non_exhaustive()
    }
}

fn relay_error(e: RelayError) -> BackendError {
    match e {
        RelayError::Cancelled => BackendError::Cancelled,
        RelayError::Timeout => BackendError::Timeout,
        RelayError::Failed(m) => BackendError::Network(m),
    }
}

impl AtrustWebSession {
    async fn send(
        &mut self,
        request: &SlRequest,
        job_cancel: Option<&CancelToken>,
    ) -> Result<SlResponse, BackendError> {
        self.used = true;
        let xml = request.to_xml();
        let never = CancelToken::new();
        let job_cancel = job_cancel.unwrap_or(&never);
        let answer = tokio::select! {
            r = self.relay.exchange(xml) => r.map_err(relay_error),
            () = self.ctx.cancel.cancelled() => Err(BackendError::Cancelled),
            () = job_cancel.cancelled() => Err(BackendError::Cancelled),
        };
        let bytes = match answer {
            Ok(b) => b,
            Err(e) => {
                // Kette abgerissen bzw. abgebrochen: A-Trust bekommt die Abschlussseite, das Overlay schließt
                self.relay.close(CloseOutcome::Aborted);
                return Err(e);
            }
        };
        match pida_sl12::parse_response(&bytes) {
            Ok(SlResponse::Error(e)) if e.is_user_cancel() => {
                self.relay.close(CloseOutcome::Aborted);
                Err(BackendError::Cancelled)
            }
            Ok(SlResponse::Error(e)) => {
                self.relay.close(CloseOutcome::Aborted);
                Err(BackendError::SecurityLayer { code: e.code, info: e.info })
            }
            Ok(other) => Ok(other),
            Err(e) => {
                self.relay.close(CloseOutcome::Aborted);
                Err(BackendError::Protocol(format!("SL-Antwort von A-Trust nicht lesbar: {e}")))
            }
        }
    }

    async fn sign_job(&mut self, job: SignJob, job_cancel: Option<&CancelToken>) -> Result<SignResult, BackendError> {
        let SignJob::Document(job) = job else {
            return Err(BackendError::Internal("ID Austria signiert nur Dokumente (Modell B)".into()));
        };
        let data = job
            .source
            .blacked_out_bytes()
            .map_err(|e| BackendError::Internal(format!("vorbereitetes Dokument nicht lesbar: {e}")))?;
        let range = job.source.excluded_range();
        drop(job);
        let excluded = ExcludedByteRange { from: range.from, to: range.to };
        self.ui.progress(Progress::Signing);
        let request = SlRequest::CreateCmsSignature(CreateCmsRequest::new(DocumentContent::Base64(data), excluded));
        match self.send(&request, job_cancel).await? {
            SlResponse::CreateCmsSignature(r) => Ok(SignResult::Cms(r.cms)),
            _ => {
                self.relay.close(CloseOutcome::Aborted);
                Err(BackendError::Protocol("unerwartete SL-Antwort auf CreateCMSSignature".into()))
            }
        }
    }
}

#[async_trait]
impl BackendSession for AtrustWebSession {
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
            _ => {
                self.relay.close(CloseOutcome::Aborted);
                Err(BackendError::Protocol("unerwartete SL-Antwort auf InfoboxRead".into()))
            }
        }
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        let total = jobs.len();
        let mut results = Vec::with_capacity(total);
        for (index, job) in jobs.into_iter().enumerate() {
            if total > 1 {
                self.ui.progress(Progress::Document { index: index + 1, total });
            }
            results.push(self.sign_job(job, None).await?);
        }
        self.ui.progress(Progress::Done);
        Ok(results)
    }

    async fn sign_one(&mut self, job: SignJob, job_cancel: CancelToken) -> Result<SignResult, BackendError> {
        if job_cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }
        self.sign_job(job, Some(&job_cancel)).await
    }

    async fn cancel(&mut self) {
        if self.used {
            self.relay.close(CloseOutcome::Aborted);
        }
    }
}

impl Drop for AtrustWebSession {
    /// Sitzung zu Ende (alle Dokumente signiert bzw. Kern fertig): Kette beenden. Nach einem Abbruch hat
    /// [`SlRelay::close`] schon gewirkt (idempotent).
    fn drop(&mut self) {
        if self.used {
            self.relay.close(CloseOutcome::Done);
        }
    }
}

#[cfg(test)]
mod tests;
