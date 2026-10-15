//! `pida-backend-atrust` – ID Austria über A-Trust, **Szenario A** (Standard in v1).
//!
//! Portiert die Spezifikation aus docs/analyse/02 §3 (PDF-Over `MobileBKUConnector`/`ATrustParser`):
//! - je SL-Request ein neuer [`pida_net::HttpClient`] mit leerem Cookie-Jar,
//! - nur 301/302 werden verfolgt, immer als GET; `Refresh`-Header und `<meta http-equiv=refresh>`,
//! - genau ein Formular, genau ein Block ([`page::PageBlock`]) in der Dispatcher-Reihenfolge
//!   FIDO2-Default → AutoSkip → Interstitial → Error → Login → SMS-TAN → QR → App-2FA → FIDO2,
//! - Formular-Submit immer `multipart/form-data`: alle Felder plus genau ein „geklickter“ Button,
//! - Long-Poll `qrpoll` mit Bearer-Token, 300 s, Backoff 10 s ([`poll`]); Poll und QR-Bild ohne
//!   Cookies,
//! - Loop-Guard 50 Requests / 2 s ([`LoopGuard`]); zusätzlich (auch bei langsamer Gegenstelle)
//!   höchstens 100 Weiterleitungen je Seite, 20 AutoSkip-Seiten und 30 Poll-Neuladungen je SL-Request,
//! - Zugangsdaten, TAN und FIDO2-Assertion nur über `https://` an die vertrauten Domains der
//!   Selektor-Datei (`trusted_hosts`: `a-trust.at`, `handy-signatur.at`) bzw. an die Origin des
//!   konfigurierten Endpunkts (Seite und Formularziel); die FIDO2-RP-ID muss zur Origin
//!   `https://service.a-trust.at` passen,
//! - Selektoren/URL-Muster als **versionierte Datendatei** ([`selectors::SelectorSet`]),
//! - unbekannte Seite → Abbruch mit Diagnosecode, nie raten,
//! - **kein automatisches Neusenden von Zugangsdaten** bei erneuter Login-Seite bzw. Fehlermeldung
//!   auf der Login-Seite (Plan §4.4, Schutz vor Kontosperre),
//! - Batch mit **einer** Anmeldung (Credentials nur als `Zeroizing` im RAM der Sitzung; das
//!   Passwort wird nie persistiert); im zweiten SL-Request wird automatisch angemeldet,
//! - FIDO2 nativ nur auf dem Desktop über [`fido2::Fido2Provider`] (Origin
//!   `https://service.a-trust.at`, Ergebnis-JSON exakt nach docs/analyse/02 §3.8).
//!
//! **Fehler und Meldungen:** HTTP 407/413/sonstige zeigt das Backend selbst als
//! [`pida_backend_api::Notice`] (OK) und liefert danach [`BackendError::Cancelled`] – wie
//! PDF-Over, das nach dem Dialog kommentarlos zur Auswahl zurückkehrt. Kann der Broker keine
//! Meldung zeigen (`InteractionError::Unsupported`), kommt der spezifische Fehler
//! ([`BackendError::ProxyAuthRequired`], [`BackendError::DocumentTooLarge`],
//! [`BackendError::Http`]). Verbindungsfehler ergeben [`BackendError::Network`] bzw.
//! [`BackendError::Timeout`] ohne eigene Meldung (der Workflow bietet „Wiederholen“ an wie
//! PDF-Over `SigningState`). SL-`ErrorResponse` 6000/6001 → `Cancelled`, sonst
//! [`BackendError::SecurityLayer`].
//!
//! **Ein Dokument je Aufruf** (`BackendSession::sign_one`, ADR 0004 §3.8; Arbeitsbereich und Stapel): HTTP
//! 407/413/sonstige ohne eigene Meldung als spezifischer Fehler; weder der Job-Abbruch („Dokument
//! überspringen“) noch SL 6000/6001 löschen die Zugangsdaten der Sitzung (§8 F10) – gelöscht werden sie
//! bei `cancel`, nach einer A-Trust-Fehlerseite, nach abgelehnter Anmeldung und beim Abbruch in der
//! QR-/App-Maske.
//!
//! Ownership: Paket **D**.

mod dialog;
pub mod fido2;
pub mod page;
pub mod poll;
pub mod selectors;
pub mod validate;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use pida_backend_api::{
    BackendError, BackendId, BackendSession, CancelToken, Capabilities, CertChain, CertNeed, Environment, FactorSet,
    Interaction, InteractionBroker, Qualification, ServerNeed, SessionContext, SignInput, SignJob, SignOutput,
    SignResult, SignatureBackend,
};
use pida_net::{NetConfig, Url};
use zeroize::Zeroizing;

pub use fido2::{Fido2Assertion, Fido2Error, Fido2Provider, Fido2Request};

/// Produktiv-Endpunkt.
pub const PROD_ENDPOINT: &str = "https://service.a-trust.at/mobile/https-security-layer-request/default.aspx";
/// Test-Endpunkt (`hs-abnahme`; nur in Abstimmung mit A-Trust).
pub const TEST_ENDPOINT: &str = "https://hs-abnahme.a-trust.at/mobile/https-security-layer-request/default.aspx";
/// WebAuthn-Origin.
pub const WEBAUTHN_ORIGIN: &str = "https://service.a-trust.at";
/// Empfohlene `/Contents`-Reserve für Modell B (Plan §6.2, 16 KiB).
pub const CMS_RESERVE: usize = 16 * 1024;

/// „Signaturdaten anzeigen“ (F-68, R10): öffnet `url` aus [`pida_backend_api::FormContext::signature_data_url`]
/// im Systembrowser über [`InteractionBroker::open_external`] – nur `https://` zu A-Trust bzw. zum
/// konfigurierten Endpunkt (ADR 0001 §8.6). Der Browser hat keine Sitzungs-Cookies (wie PDF-Over).
pub fn open_signature_data(ui: &dyn InteractionBroker, url: &str, endpoint: &Url) -> Result<(), BackendError> {
    let parsed = Url::parse(url).map_err(|_| BackendError::Protocol("Link „Signaturdaten“ ist keine URL".into()))?;
    if !dialog::allowed_external(&parsed, endpoint) {
        return Err(BackendError::Protocol("Link „Signaturdaten“ führt nicht zu A-Trust".into()));
    }
    ui.open_external(parsed.as_str()).map_err(|e| match e {
        pida_backend_api::InteractionError::Cancelled => BackendError::Cancelled,
        pida_backend_api::InteractionError::Unsupported => {
            BackendError::Unavailable("Systembrowser nicht verfügbar".into())
        }
    })
}

/// Endlosschleifen-Schutz (docs/analyse/02 §3.4 Schritt 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopGuardConfig {
    /// Fenster
    pub window: Duration,
    /// Höchstzahl Requests mit Abstand < `window`
    pub max_requests: u32,
}

impl Default for LoopGuardConfig {
    fn default() -> Self {
        Self { window: Duration::from_secs(2), max_requests: 50 }
    }
}

/// Zustand des Loop-Guards (gilt über beide SL-Requests einer Sitzung).
///
/// Semantik wie `MobileBKUConnector.sendHTTPRequest`: Ein Request innerhalb von `window` nach dem
/// Fensterbeginn erhöht den Zähler, sonst beginnt ein neues Fenster (Zähler 0). Ist der Zähler bei
/// `max_requests` angekommen, wird kein weiterer Request mehr gesendet.
#[derive(Debug, Clone)]
pub struct LoopGuard {
    config: LoopGuardConfig,
    window_start: Option<std::time::Instant>,
    count: u32,
}

impl LoopGuard {
    /// Neuer Guard.
    pub fn new(config: LoopGuardConfig) -> Self {
        Self { config, window_start: None, count: 0 }
    }

    /// Vor jedem Request aufrufen; `Err(LoopDetected)` ab der Grenze.
    pub fn check(&mut self, now: std::time::Instant) -> Result<(), BackendError> {
        if self.count >= self.config.max_requests {
            return Err(BackendError::LoopDetected);
        }
        match self.window_start {
            Some(start) if now.saturating_duration_since(start) < self.config.window => self.count += 1,
            _ => {
                self.count = 0;
                self.window_start = Some(now);
            }
        }
        Ok(())
    }
}

/// Konfiguration des Backends.
#[derive(Clone)]
pub struct AtrustConfig {
    /// SL-Endpunkt ([`PROD_ENDPOINT`]).
    pub endpoint: Url,
    /// Netz (Proxy, Timeouts, User-Agent – Zusatz nur nach Abstimmung mit A-Trust, L10).
    pub net: NetConfig,
    /// Vorbelegter Benutzername (`MOBILE_NUMBER`/`-n`). Startwert des „Merken“-Speichers des
    /// Backends ([`AtrustNativeBackend::new`]); [`AtrustNativeBackend::forget_credentials`] bzw. das
    /// Abwählen von „Merken“ löschen ihn wie in PDF-Over.
    pub prefill_username: Option<String>,
    /// Passwort aus `-p` (nur RAM). Startwert wie `prefill_username`; nach einer Fehlerseite, einer
    /// abgelehnten Anmeldung oder einem Nutzerabbruch ist es gelöscht und wird **nicht** in der
    /// nächsten Sitzung erneut gesendet (PDF-Over: Overlay `clearRememberedPassword`).
    pub prefill_password: Option<Zeroizing<String>>,
    /// „Merken“-Zustand (`MOBILE_REMEMBER_PASSWORD`).
    pub remember: bool,
    /// FIDO2 standardmäßig (`FIDO2_DEFAULT`) – wirkt nur mit verfügbarem Provider, einmal je Sitzung.
    pub fido2_default: bool,
    /// Plattform-Authenticator (nur Desktop).
    pub fido2: Option<Arc<dyn Fido2Provider>>,
    /// Selektoren/URL-Muster (Default: eingebettete Datei).
    pub selectors: selectors::SelectorSet,
    /// Loop-Guard.
    pub loop_guard: LoopGuardConfig,
    /// Long-Poll.
    pub poll: poll::PollConfig,
}

impl AtrustConfig {
    /// Standardkonfiguration für einen Endpunkt (eingebettete Selektoren, ohne FIDO2). Das
    /// Request-Timeout ist mit 300 s großzügig, weil der CMS-Request das ganze PDF hochlädt.
    pub fn new(endpoint: Url) -> Self {
        Self {
            endpoint,
            net: NetConfig { request_timeout: Some(Duration::from_secs(300)), ..NetConfig::default() },
            prefill_username: None,
            prefill_password: None,
            remember: false,
            fido2_default: false,
            fido2: None,
            selectors: selectors::SelectorSet::builtin(),
            loop_guard: LoopGuardConfig::default(),
            poll: poll::PollConfig::default(),
        }
    }

    /// Produktiv-Endpunkt [`PROD_ENDPOINT`].
    pub fn production() -> Self {
        Self::new(Url::parse(PROD_ENDPOINT).expect("konstante URL"))
    }
}

impl std::fmt::Debug for AtrustConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtrustConfig")
            .field("endpoint", &self.endpoint.as_str())
            .field("net", &self.net)
            .field("prefill_username", &self.prefill_username.as_deref().map(redacted))
            .field("prefill_password", &self.prefill_password.as_ref().map(|_| "***"))
            .field("remember", &self.remember)
            .field("fido2_default", &self.fido2_default)
            .field("fido2", &self.fido2)
            .field("selectors_version", &self.selectors.version)
            .field("loop_guard", &self.loop_guard)
            .field("poll", &self.poll)
            .finish()
    }
}

/// Benutzername/Telefonnummer in `Debug` nur als Länge (Personendaten, ADR 0001 §4 Regel 9).
fn redacted(value: &str) -> String {
    format!("<{} Zeichen>", value.chars().count())
}

/// Credentials einer Sitzung (nur RAM).
#[derive(Clone, Default)]
pub struct Credentials {
    /// Benutzername/Telefonnummer (unverändert gesendet)
    pub username: Option<String>,
    /// Passwort
    pub password: Option<Zeroizing<String>>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username.as_deref().map(redacted))
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Backend.
#[derive(Debug)]
pub struct AtrustNativeBackend {
    config: AtrustConfig,
    /// Bei „Merken“: Passwort bis Programmende im RAM (wie PDF-Over-Overlay), nie auf Platte.
    remembered: Arc<Mutex<Credentials>>,
}

impl AtrustNativeBackend {
    /// Neues Backend. Der „Merken“-Speicher startet mit der Vorbelegung aus `-n`/`-p` bzw.
    /// `MOBILE_NUMBER` (wie die PDF-Over-Overlays), damit ein abgelehntes Passwort gelöscht bleibt.
    pub fn new(config: AtrustConfig) -> Self {
        let initial =
            Credentials { username: config.prefill_username.clone(), password: config.prefill_password.clone() };
        Self { config, remembered: Arc::new(Mutex::new(initial)) }
    }

    /// Vergisst gemerkte Credentials (Abwählen von „Merken“, Fehlerseite, Nutzerabbruch).
    pub fn forget_credentials(&self) {
        *self.remembered.lock().expect("poisoned") = Credentials::default();
    }

    /// Konfiguration.
    pub fn config(&self) -> &AtrustConfig {
        &self.config
    }

    fn fido2_available(&self) -> bool {
        self.config.fido2.as_ref().is_some_and(|p| p.is_available())
    }
}

#[async_trait]
impl SignatureBackend for AtrustNativeBackend {
    fn id(&self) -> BackendId {
        BackendId::AtrustNative
    }

    fn capabilities(&self, env: &Environment) -> Option<Capabilities> {
        if env.headless || !env.allows(BackendId::AtrustNative) {
            return None;
        }
        let fido2 = env.platform.is_desktop() && self.fido2_available();
        Some(Capabilities {
            input: SignInput::Document,
            output: SignOutput::Cms,
            certificate: CertNeed::OnlyForVisibleBlock,
            interaction: Interaction::NativeForms,
            server: ServerNeed::None,
            max_batch: 1,
            login_once_per_batch: true,
            document_leaves_device: true,
            qualification: Qualification::Qes,
            factors: FactorSet { app: true, sms: true, qr: true, fido2, card: false },
            cms_reserve: Some(CMS_RESERVE),
        })
    }

    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError> {
        if ctx.headless {
            return Err(BackendError::Unavailable("ID Austria braucht Eingaben der Nutzerin (nicht headless)".into()));
        }
        // nur der „Merken“-Speicher (mit der Vorbelegung initialisiert): was dort gelöscht wurde,
        // kommt nicht aus der Konfiguration zurück
        let credentials = self.remembered.lock().expect("poisoned").clone();
        let fido2_available = self.fido2_available();
        Ok(Box::new(AtrustSession {
            credentials,
            loop_guard: LoopGuard::new(self.config.loop_guard),
            inner: dialog::Dialog::new(self.config.clone(), Arc::clone(&self.remembered), ui, ctx, fido2_available),
        }))
    }
}

/// Offene Sitzung (InfoboxRead → CreateCMS, je Request neuer Client).
pub struct AtrustSession {
    credentials: Credentials,
    loop_guard: LoopGuard,
    inner: dialog::Dialog,
}

impl std::fmt::Debug for AtrustSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtrustSession")
            .field("credentials", &self.credentials)
            .field("loop_guard", &self.loop_guard)
            .field("dialog", &self.inner)
            .finish()
    }
}

impl AtrustSession {
    /// Ein Nutzerabbruch löscht das Passwort (Sitzung und „Merken“-Speicher; F-72: Benutzername
    /// bleibt), wie PDF-Over `SigningState`.
    fn after<T>(&mut self, result: Result<T, BackendError>) -> Result<T, BackendError> {
        if matches!(result, Err(BackendError::Cancelled)) {
            self.inner.clear_password(&mut self.credentials);
        }
        result
    }
}

#[async_trait]
impl BackendSession for AtrustSession {
    async fn certificate(&mut self) -> Result<CertChain, BackendError> {
        let result = self.inner.certificate(&mut self.credentials, &mut self.loop_guard).await;
        self.after(result)
    }

    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError> {
        let result = self.inner.sign(&mut self.credentials, &mut self.loop_guard, jobs).await;
        self.after(result)
    }

    /// Ein Dokument (Arbeitsbereich und Stapel, ADR 0004 §3.8). Anders als [`BackendSession::sign`]:
    ///
    /// - weder ein Job-Abbruch (`job_cancel`, „Dokument überspringen“) noch `Cancelled` aus SL 6000/6001
    ///   (Ablehnung bzw. Zeitablauf in der App) löschen die Zugangsdaten der Sitzung – die nächste
    ///   Bestätigung braucht keine neue Anmeldung (§8 F10). Gelöscht werden sie wie bisher bei
    ///   [`BackendSession::cancel`] (Sitzungsabbruch, F-72), nach einer A-Trust-Fehlerseite, nach einer
    ///   abgelehnten Anmeldung und beim Abbruch in der QR-/App-Maske (`dialog.rs`);
    /// - HTTP 407/413/sonstige erscheinen **ohne** eigene Meldung als [`BackendError::ProxyAuthRequired`],
    ///   [`BackendError::DocumentTooLarge`] bzw. [`BackendError::Http`];
    /// - der laufende SL-Request wird beim Job-Abbruch gedroppt (A-Trust erfährt davon nichts, §1.2);
    ///   Loop-Guard und FIDO2-Vorgabe gelten für die ganze Sitzung weiter.
    async fn sign_one(&mut self, job: SignJob, job_cancel: CancelToken) -> Result<SignResult, BackendError> {
        if job_cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }
        self.inner.set_quiet_fatal(true);
        let result = tokio::select! {
            r = self.inner.sign(&mut self.credentials, &mut self.loop_guard, vec![job]) => r,
            () = job_cancel.cancelled() => Err(BackendError::Cancelled),
        };
        self.inner.set_quiet_fatal(false);
        result?.into_iter().next().ok_or_else(|| BackendError::Internal("sign lieferte kein Ergebnis".into()))
    }

    async fn cancel(&mut self) {
        self.inner.clear_password(&mut self.credentials);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn loop_guard_like_pdf_over() {
        let mut g = LoopGuard::new(LoopGuardConfig::default());
        let t0 = Instant::now();
        // 1 Request eröffnet das Fenster, 50 weitere zählen, der 52. wird verweigert
        for i in 0..51 {
            assert!(g.check(t0 + Duration::from_millis(i)).is_ok(), "{i}");
        }
        assert_eq!(g.check(t0 + Duration::from_millis(60)), Err(BackendError::LoopDetected));
        // einmal ausgelöst bleibt er ausgelöst (Java prüft vor dem Zurücksetzen)
        assert_eq!(g.check(t0 + Duration::from_secs(10)), Err(BackendError::LoopDetected));

        let mut g = LoopGuard::new(LoopGuardConfig::default());
        for i in 0..200u64 {
            // alle 50 ms ein Request: nach jeweils 2 s beginnt ein neues Fenster
            assert!(g.check(t0 + Duration::from_millis(i * 50)).is_ok(), "{i}");
        }
    }

    #[test]
    fn config_debug_hides_password() {
        let mut c = AtrustConfig::production();
        c.prefill_password = Some(Zeroizing::new("geheim".into()));
        c.prefill_username = Some("+436641234567".into());
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("geheim"));
        assert!(!dbg.contains("6641234567"), "{dbg}");
        assert_eq!(c.endpoint.as_str(), PROD_ENDPOINT);
        let creds =
            Credentials { username: Some("max.mustermann".into()), password: Some(Zeroizing::new("geheim".into())) };
        let dbg = format!("{creds:?}");
        assert!(!dbg.contains("geheim") && !dbg.contains("mustermann"), "{dbg}");
        assert!(dbg.contains("<14 Zeichen>"), "{dbg}");
    }

    #[test]
    fn capabilities() {
        let backend = AtrustNativeBackend::new(AtrustConfig::production());
        let env = Environment::current();
        let caps = backend.capabilities(&env).unwrap();
        assert_eq!(caps.input, SignInput::Document);
        assert_eq!(caps.output, SignOutput::Cms);
        assert_eq!(caps.certificate, CertNeed::OnlyForVisibleBlock);
        assert_eq!(caps.interaction, Interaction::NativeForms);
        assert!(caps.document_leaves_device && caps.login_once_per_batch);
        assert_eq!(caps.max_batch, 1);
        assert_eq!(caps.cms_reserve, Some(16 * 1024));
        assert!(!caps.factors.fido2, "ohne Provider kein FIDO2");
        assert!(backend.capabilities(&Environment { headless: true, ..env.clone() }).is_none());
        assert!(
            backend.capabilities(&Environment { allowed_by_policy: Some(vec![BackendId::Keystore]), ..env }).is_none()
        );
    }
}
