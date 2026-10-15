//! `pdfover-backend-api` – gemeinsame Schnittstelle aller Signatur-Backends (Plan §3.3, §3.4, §4).
//!
//! **Szenario A ist Standard für ID Austria** (Entscheidung des Auftraggebers, ADR 0001 §2):
//! `atrust-native` bedient den A-Trust-SL-1.2-HTTPS-Endpunkt nativ und zeigt alle Masken über
//! [`InteractionBroker::native_form`]. Die Varianten für Szenario B (Browser-Control/Relay), C (CSC)
//! und D (EUDI-Wallet) bleiben im Capability-Modell **reserviert**, werden in v1 aber nicht gebaut.
//!
//! Abbildung der Dialoge aus docs/analyse/01 §4.6/§4.7/§4.10 und docs/analyse/02 §3.5:
//!
//! | Dialog (PDF-Over) | Abbildung |
//! |---|---|
//! | Anmeldung (`MobileBKUEnterNumberComposite`) | [`NativeForm::Login`] → [`FormAnswer::Login`] |
//! | SMS-TAN (`MobileBKUEnterTANComposite`) | [`NativeForm::SmsTan`] → [`FormAnswer::Tan`] |
//! | QR-Code (`MobileBKUQRComposite`) | [`NativeForm::QrCode`] (Bild + Vergleichswert; Long-Poll schließt den Dialog per Drop) |
//! | App-Bestätigung (`MobileBKUNeedsAppComposite`) | [`NativeForm::WaitForApp`] (Long-Poll, Drop) |
//! | FIDO2 (`MobileBKUFido2Composite`) | [`NativeForm::Fido2`] + `Fido2Provider` in `pdfover-backend-atrust` |
//! | Interstitial (`ExpiresInfo.aspx`) | [`InteractionBroker::notice`] mit [`NoticeCode::AtrustInterstitial`] |
//! | A-Trust-Fehler (`error*.aspx`) | [`InteractionBroker::notice`] mit [`NoticeCode::AtrustError`], Retry/Cancel bzw. OK |
//! | HTTP 407/413/sonstige, Verbindungsfehler | [`InteractionBroker::notice`] (OK → Abbruch) |
//! | „Signaturdaten anzeigen“ (F-68) | [`FormContext::signature_data_url`] + [`InteractionBroker::open_external`] |
//! | Keystore-Passwörter (`KSState`) | [`InteractionBroker::secret`] mit [`SecretKind`] |
//! | FIDO2-PIN (CTAP2 `clientPIN`, macOS/Linux) | [`InteractionBroker::secret`] mit [`SecretKind::Fido2Pin`] |
//! | FIDO2-Fehler mit Code | [`FormProblem::Fido2Error`] (`code` = `fido2-…`) |
//! | Keystore-Fehler mit Untercode | [`BackendError::KeystoreDetail`] / [`BackendError::detail_code`] |
//! | Warten („Signature creation in progress…“) | [`InteractionBroker::progress`] |
//!
//! Abbruch: Jede Methode liefert [`InteractionError::Cancelled`], wenn die Nutzerin abbricht oder
//! das Fenster schließt (F-73). Ein **App-Wechsel auf Mobile ist kein Abbruch** (Plan §3.5).
//! Futures des Brokers dürfen vom Backend gedroppt werden (z. B. Long-Poll meldet „fertig“); der
//! Broker muss den zugehörigen Dialog dann schließen.
//!
//! Ownership: **Integration** (Schnittstelle ist für die Pakete B und D eingefroren).

mod cancel;
mod capabilities;
mod error;
mod interaction;
mod job;
#[cfg(feature = "ts")]
pub mod ts_mirror;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use cancel::CancelToken;
pub use capabilities::{
    BackendId, Capabilities, Capture, CertNeed, Environment, FactorSet, Interaction, Platform, Qualification,
    ServerNeed, SignInput, SignOutput,
};
pub use error::BackendError;
pub use interaction::{
    BackendEvent, BrowserRequest, CallbackData, FormAnswer, FormContext, FormProblem, InteractionBroker,
    InteractionError, NativeForm, Notice, NoticeAnswer, NoticeButtons, NoticeCode, NoticeKind, Progress, QrImage,
    SecretKind, SecretPrompt, WebOutcome, WebRequest, WindowHandle,
};
pub use job::{DigestJob, DocumentJob, DocumentSource, ExcludedByteRange, SignJob, SignResult};
pub use pdfover_cades::{CertChain, Certificate, DigestAlg, RawSignature, SignatureAlg};

use std::sync::Arc;

use async_trait::async_trait;

/// Kontext einer Backend-Sitzung (ein Signaturvorgang bzw. ein Batch).
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// UI-Sprache (Texte der eigenen Masken; A-Trust-Texte kommen roh).
    pub language: pdfover_i18n::Lang,
    /// Anzahl der Dokumente im Batch (Szenario A: eine Anmeldung je Batch, Plan §4.4).
    pub batch_size: usize,
    /// Abbruch von außen (Nutzerin, Fenster geschlossen, App beendet) – der einzige Weg, ein
    /// laufendes `certificate`/`sign` abzubrechen (siehe [`BackendSession`]).
    pub cancel: CancelToken,
    /// Ohne UI (CLI `--headless`): Backends, die Interaktion brauchen, liefern
    /// [`BackendError::Unavailable`] statt zu fragen.
    pub headless: bool,
}

/// Ein Signatur-Backend.
#[async_trait]
pub trait SignatureBackend: Send + Sync + std::fmt::Debug {
    /// Kennung.
    fn id(&self) -> BackendId;

    /// `None` = auf dieser Plattform, in dieser Konfiguration oder per Richtlinie nicht verfügbar.
    fn capabilities(&self, env: &Environment) -> Option<Capabilities>;

    /// Öffnet eine Sitzung. Für Szenario A beginnt hier noch kein Netzverkehr.
    async fn open(
        &self,
        ctx: SessionContext,
        ui: Arc<dyn InteractionBroker>,
    ) -> Result<Box<dyn BackendSession>, BackendError>;
}

/// Eine offene Sitzung.
///
/// Reihenfolge (Plan §3.6/§3.8): optional [`BackendSession::certificate`] (InfoboxRead bzw.
/// Keystore), dann `prepare()` im Kern, dann [`BackendSession::sign`]. Zwischen den beiden
/// SL-Requests ändert sich das PDF nicht mehr.
///
/// **Abbruch** (ADR 0001 §8): Solange `certificate`/`sign` laufen, ist die Sitzung `&mut` geliehen –
/// der Kern bricht dann über [`SessionContext::cancel`] ab (Backends `select!`en darauf und liefern
/// [`BackendError::Cancelled`]) oder droppt das Future. Beide Futures müssen deshalb **drop-sicher**
/// sein (laufende Polls/FIDO2-Anfragen beenden, keine halben Zustände). [`BackendSession::cancel`]
/// räumt danach auf.
#[async_trait]
pub trait BackendSession: Send {
    /// Signaturzertifikat (für den sichtbaren Block und die Prüfung I3).
    async fn certificate(&mut self) -> Result<CertChain, BackendError>;

    /// Signiert die Jobs. Modell B (`SignInput::Document`) liefert [`SignResult::Cms`],
    /// Modell A (`SignInput::Digest`) [`SignResult::Raw`]. Ergebnisreihenfolge = Jobreihenfolge.
    async fn sign(&mut self, jobs: Vec<SignJob>) -> Result<Vec<SignResult>, BackendError>;

    /// Räumt nach einem Abbruch auf (idempotent, auch ohne laufenden Vorgang): gemerkte Passwörter
    /// werden gelöscht (F-72: Benutzername bleibt), offene Verbindungen geschlossen. Laufende
    /// Vorgänge bricht der Kern vorher über [`SessionContext::cancel`] ab (siehe oben).
    async fn cancel(&mut self);
}
