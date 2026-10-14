//! FIDO2/WebAuthn für Szenario A (docs/analyse/02 §3.8; Plan §4.4, §4.7).
//!
//! Desktop: Windows `webauthn.dll` (Origin über `WEBAUTHN_CLIENT_DATA`, Faktencheck 9a) bzw.
//! libfido2/CTAP-HID unter macOS/Linux (udev/uaccess, Flatpak `--device=all`). Mobile: kein
//! natives FIDO2 in v1 (Associated Domains/Asset Links fehlen). Kein Code aus `webauthn-client-java`.

use pdfover_backend_api::WindowHandle;

/// Anfrage an den Authenticator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fido2Request {
    /// `#credentialOptions@value` unverändert (PublicKeyCredentialRequestOptions als JSON)
    pub options_json: String,
    /// Origin, fest `https://service.a-trust.at`
    pub origin: String,
}

/// Assertion (Felder für das Ergebnis-JSON).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fido2Assertion {
    /// `credential.id` (String wie geliefert)
    pub id: String,
    /// rawId
    pub raw_id: Vec<u8>,
    /// `public-key`
    pub kind: String,
    /// authenticatorData
    pub authenticator_data: Vec<u8>,
    /// clientDataJSON (vom Client selbst gebaut: type, challenge, origin, crossOrigin)
    pub client_data_json: Vec<u8>,
    /// signature
    pub signature: Vec<u8>,
    /// userHandle
    pub user_handle: Option<Vec<u8>>,
}

/// FIDO2-Fehler.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Fido2Error {
    /// Nicht verfügbar (kein Authenticator, keine Bibliothek).
    #[error("FIDO2 nicht verfügbar")]
    Unavailable,
    /// Nutzerin hat am Token abgebrochen (keine Fehlermeldung, Button wieder aktiv).
    #[error("am Sicherheitsschlüssel abgebrochen")]
    Cancelled,
    /// Optionen nicht lesbar.
    #[error("credentialOptions ungültig: {0}")]
    InvalidOptions(String),
    /// Fehler des Authenticators.
    #[error("Authenticator-Fehler: {0}")]
    Device(String),
}

/// Plattform-Authenticator.
pub trait Fido2Provider: Send + Sync + std::fmt::Debug {
    /// Verfügbar (steuert Sichtbarkeit des FIDO2-Buttons und `FIDO2_DEFAULT`).
    fn is_available(&self) -> bool;

    /// Holt eine Assertion (blockierend; der Aufrufer nutzt `spawn_blocking`).
    fn get_assertion(&self, request: &Fido2Request, parent: Option<WindowHandle>)
    -> Result<Fido2Assertion, Fido2Error>;

    /// Bricht eine laufende Anfrage ab.
    fn cancel(&self);
}

/// Ergebnis-JSON exakt nach PDF-Over (`MobileBKUConnector.java:426-444`): Schlüssel `clientDataJson`
/// (kleines „son“), **Standard-Base64 mit Padding**, `extensions` immer `{}`, `userHandle` = `null`
/// wenn keiner geliefert wurde.
pub fn result_json(assertion: &Fido2Assertion) -> String {
    todo!("Paket D")
}
