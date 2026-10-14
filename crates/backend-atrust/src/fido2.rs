//! FIDO2/WebAuthn für Szenario A (docs/analyse/02 §3.8; Plan §4.4, §4.7).
//!
//! Desktop: Windows `webauthn.dll` (Origin über `WEBAUTHN_CLIENT_DATA`, Faktencheck 9a; Feature
//! `fido2-windows`) bzw. CTAP2 über USB-HID unter macOS/Linux (Feature `fido2-hid`). Mobile: kein
//! natives FIDO2 in v1 (Associated Domains/Asset Links fehlen). Kein Code aus `webauthn-client-java`.
//!
//! Der Ablauf ist für alle Plattformen gleich: `#credentialOptions@value` → [`AssertionOptions`],
//! eigenes `clientDataJSON` mit Origin `https://service.a-trust.at` ([`client_data_json`]),
//! Assertion vom Authenticator, Ergebnis-JSON exakt wie PDF-Over ([`result_json`]).

use base64::Engine as _;
use pdfover_backend_api::WindowHandle;

pub mod ctap;
#[cfg(feature = "fido2-hid")]
pub mod hid;
#[cfg(feature = "fido2-windows")]
pub mod windows;

/// Anfrage an den Authenticator. `Debug` ohne Challenge/Credential-IDs (Regel 9).
#[derive(Clone, PartialEq, Eq)]
pub struct Fido2Request {
    /// `#credentialOptions@value` unverändert (PublicKeyCredentialRequestOptions als JSON)
    pub options_json: String,
    /// Origin, fest `https://service.a-trust.at`
    pub origin: String,
}

impl std::fmt::Debug for Fido2Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fido2Request")
            .field("options_len", &self.options_json.len())
            .field("origin", &self.origin)
            .finish()
    }
}

/// Assertion (Felder für das Ergebnis-JSON). `Debug` ohne Signatur- und Benutzerdaten.
#[derive(Clone, PartialEq, Eq)]
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

impl std::fmt::Debug for Fido2Assertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fido2Assertion")
            .field("kind", &self.kind)
            .field("raw_id_len", &self.raw_id.len())
            .field("authenticator_data_len", &self.authenticator_data.len())
            .field("signature_len", &self.signature.len())
            .field("user_handle", &self.user_handle.as_ref().map(|_| "***"))
            .finish()
    }
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

impl Fido2Error {
    /// Stabiler Code für i18n und Diagnose (`fido2-…`).
    pub fn code(&self) -> &'static str {
        match self {
            Fido2Error::Unavailable => "fido2-unavailable",
            Fido2Error::Cancelled => "fido2-cancelled",
            Fido2Error::InvalidOptions(_) => "fido2-invalid-options",
            Fido2Error::Device(_) => "fido2-device",
        }
    }
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

/// Plattform-Provider der eingeschalteten Features: Windows `webauthn.dll` (`fido2-windows`),
/// macOS/Linux CTAP2-HID (`fido2-hid`); sonst `None` (FIDO2 „nicht verfügbar“, z. B. Mobile).
pub fn platform_provider() -> Option<std::sync::Arc<dyn Fido2Provider>> {
    #[cfg(all(feature = "fido2-windows", target_os = "windows"))]
    {
        return Some(std::sync::Arc::new(windows::WebAuthnProvider::new()));
    }
    #[cfg(all(feature = "fido2-hid", any(target_os = "linux", target_os = "macos")))]
    {
        return Some(std::sync::Arc::new(hid::HidFido2Provider::new()));
    }
    #[allow(unreachable_code)]
    None
}

/// JSON-Zeichenkette (serde_json-Escaping; für Base64 identisch mit org.json).
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("String ist immer serialisierbar")
}

/// Ergebnis-JSON exakt nach PDF-Over (`MobileBKUConnector.java:426-444`): Schlüssel `clientDataJson`
/// (kleines „son“), **Standard-Base64 mit Padding**, `extensions` immer `{}`, `userHandle` = `null`
/// wenn keiner geliefert wurde. Schlüsselreihenfolge: id, rawId, type, extensions, response
/// (authenticatorData, clientDataJson, signature, userHandle) – org.json legt keine fest.
pub fn result_json(assertion: &Fido2Assertion) -> String {
    let b64 = |d: &[u8]| json_string(&base64::engine::general_purpose::STANDARD.encode(d));
    let user_handle = assertion.user_handle.as_deref().map_or_else(|| "null".to_owned(), b64);
    format!(
        "{{\"id\":{},\"rawId\":{},\"type\":{},\"extensions\":{{}},\"response\":{{\"authenticatorData\":{},\"clientDataJson\":{},\"signature\":{},\"userHandle\":{}}}}}",
        json_string(&assertion.id),
        b64(&assertion.raw_id),
        json_string(&assertion.kind),
        b64(&assertion.authenticator_data),
        b64(&assertion.client_data_json),
        b64(&assertion.signature),
        user_handle
    )
}

/// `userVerification` der Optionen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UserVerification {
    /// `required`
    Required,
    /// `preferred` (Default laut WebAuthn)
    #[default]
    Preferred,
    /// `discouraged`
    Discouraged,
}

/// Eintrag aus `allowCredentials`. `Debug` nur mit Länge der ID.
#[derive(Clone, PartialEq, Eq)]
pub struct CredentialDescriptor {
    /// Credential-ID
    pub id: Vec<u8>,
    /// `transports` (`usb`, `nfc`, `ble`, `internal`, `hybrid`)
    pub transports: Vec<String>,
}

/// Ausgewertete `PublicKeyCredentialRequestOptions` (Format von A-Trust nicht dokumentiert,
/// docs/analyse/02 §9 Frage 3 – deshalb tolerant: Wurzel oder `publicKey`, Binärwerte als
/// base64url, Base64 oder Zahlen-Array). `Debug` ohne Challenge.
#[derive(Clone, PartialEq, Eq)]
pub struct AssertionOptions {
    /// Challenge (roh)
    pub challenge: Vec<u8>,
    /// RP-ID (`rpId`, sonst Host der Origin)
    pub rp_id: String,
    /// erlaubte Credentials (leer = residente Schlüssel)
    pub allow_credentials: Vec<CredentialDescriptor>,
    /// Nutzerverifikation
    pub user_verification: UserVerification,
    /// Timeout in Millisekunden
    pub timeout_ms: Option<u32>,
}

fn decode_binary(value: &serde_json::Value, field: &str) -> Result<Vec<u8>, Fido2Error> {
    use base64::engine::general_purpose::{STANDARD_PAD_INDIFFERENT, URL_SAFE_NO_PAD_INDIFFERENT};
    match value {
        serde_json::Value::String(s) => {
            let s = s.trim();
            URL_SAFE_NO_PAD_INDIFFERENT
                .decode(s.trim_end_matches('='))
                .or_else(|_| STANDARD_PAD_INDIFFERENT.decode(s))
                .map_err(|_| Fido2Error::InvalidOptions(format!("{field} ist kein Base64")))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(|v| v.as_u64().filter(|n| *n <= 255).map(|n| n as u8))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| Fido2Error::InvalidOptions(format!("{field} ist kein Byte-Array"))),
        serde_json::Value::Object(o) if o.get("type").and_then(|t| t.as_str()) == Some("Buffer") => {
            decode_binary(o.get("data").unwrap_or(&serde_json::Value::Null), field)
        }
        _ => Err(Fido2Error::InvalidOptions(format!("{field} fehlt oder hat einen unbekannten Typ"))),
    }
}

impl std::fmt::Debug for CredentialDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialDescriptor")
            .field("id_len", &self.id.len())
            .field("transports", &self.transports)
            .finish()
    }
}

impl std::fmt::Debug for AssertionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssertionOptions")
            .field("challenge_len", &self.challenge.len())
            .field("rp_id", &self.rp_id)
            .field("allow_credentials", &self.allow_credentials)
            .field("user_verification", &self.user_verification)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

impl AssertionOptions {
    /// Wertet `#credentialOptions@value` aus.
    pub fn parse(json: &str, origin: &str) -> Result<AssertionOptions, Fido2Error> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| Fido2Error::InvalidOptions(format!("kein JSON: {e}")))?;
        let root = value.get("publicKey").filter(|v| v.is_object()).unwrap_or(&value);
        let object = root.as_object().ok_or_else(|| Fido2Error::InvalidOptions("kein JSON-Objekt".into()))?;
        let challenge = decode_binary(object.get("challenge").unwrap_or(&serde_json::Value::Null), "challenge")?;
        if challenge.is_empty() {
            return Err(Fido2Error::InvalidOptions("challenge ist leer".into()));
        }
        let rp_id = match object.get("rpId").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            Some(r) => r.to_owned(),
            None => pdfover_net::Url::parse(origin)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .ok_or_else(|| Fido2Error::InvalidOptions("rpId fehlt und Origin ist ungültig".into()))?,
        };
        let mut allow_credentials = Vec::new();
        if let Some(list) = object.get("allowCredentials").and_then(|v| v.as_array()) {
            for entry in list {
                if entry.get("type").and_then(|t| t.as_str()).is_some_and(|t| t != "public-key") {
                    continue;
                }
                let id = decode_binary(entry.get("id").unwrap_or(&serde_json::Value::Null), "allowCredentials.id")?;
                let transports = entry
                    .get("transports")
                    .and_then(|t| t.as_array())
                    .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
                allow_credentials.push(CredentialDescriptor { id, transports });
            }
        }
        let user_verification = match object.get("userVerification").and_then(|v| v.as_str()) {
            Some("required") => UserVerification::Required,
            Some("discouraged") => UserVerification::Discouraged,
            _ => UserVerification::Preferred,
        };
        let timeout_ms = object.get("timeout").and_then(|v| v.as_u64()).map(|t| t.min(u32::MAX as u64) as u32);
        Ok(AssertionOptions { challenge, rp_id, allow_credentials, user_verification, timeout_ms })
    }
}

/// `clientDataJSON` wie ein Browser (WebAuthn L2 §5.8.1, Reihenfolge type, challenge, origin,
/// crossOrigin; Challenge base64url ohne Padding).
pub fn client_data_json(challenge: &[u8], origin: &str) -> Vec<u8> {
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge);
    format!(
        "{{\"type\":\"webauthn.get\",\"challenge\":{},\"origin\":{},\"crossOrigin\":false}}",
        json_string(&challenge),
        json_string(origin)
    )
    .into_bytes()
}

/// `credential.id` = base64url(rawId) ohne Padding (WebAuthn `PublicKeyCredential.id`).
pub fn credential_id_string(raw_id: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assertion(user_handle: Option<Vec<u8>>) -> Fido2Assertion {
        Fido2Assertion {
            id: "AQID".into(),
            raw_id: vec![1, 2, 3],
            kind: "public-key".into(),
            authenticator_data: vec![0xff, 0xfe, 0xfd],
            client_data_json: b"{\"type\":\"webauthn.get\"}".to_vec(),
            signature: vec![0x30, 0x45],
            user_handle,
        }
    }

    #[test]
    fn result_json_exactly_like_pdf_over() {
        let json = result_json(&assertion(Some(vec![0xfb, 0xff])));
        assert_eq!(
            json,
            r#"{"id":"AQID","rawId":"AQID","type":"public-key","extensions":{},"response":{"authenticatorData":"//79","clientDataJson":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0In0=","signature":"MEU=","userHandle":"+/8="}}"#
        );
        // Standard-Base64 mit Padding und `+`/`/`, kein base64url
        assert!(json.contains("\"+/8=\""));
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["response"]["clientDataJson"].is_string());
        assert!(v["response"].get("clientDataJSON").is_none());
        assert_eq!(v["extensions"], serde_json::json!({}));
        let none = result_json(&assertion(None));
        let v: serde_json::Value = serde_json::from_str(&none).unwrap();
        assert!(v["response"]["userHandle"].is_null());
        assert!(!format!("{:?}", assertion(Some(vec![9]))).contains("MEU"));
        let req = Fido2Request { options_json: r#"{"challenge":"GEHEIM"}"#.into(), origin: "o".into() };
        assert!(!format!("{req:?}").contains("GEHEIM"));
        assert_eq!(Fido2Error::Cancelled.code(), "fido2-cancelled");
        assert_eq!(Fido2Error::Unavailable.code(), "fido2-unavailable");
        assert_eq!(Fido2Error::InvalidOptions(String::new()).code(), "fido2-invalid-options");
        assert_eq!(Fido2Error::Device(String::new()).code(), "fido2-device");
    }

    #[test]
    fn options_parsing_variants() {
        let origin = "https://service.a-trust.at";
        let o = AssertionOptions::parse(
            r#"{"challenge":"AAEC_-8","timeout":60000,"rpId":"a-trust.at","allowCredentials":[{"type":"public-key","id":"BAUG","transports":["usb","nfc"]}],"userVerification":"required"}"#,
            origin,
        )
        .unwrap();
        assert_eq!(o.challenge, vec![0, 1, 2, 0xff, 0xef]);
        assert_eq!(o.rp_id, "a-trust.at");
        assert_eq!(
            o.allow_credentials,
            [CredentialDescriptor { id: vec![4, 5, 6], transports: vec!["usb".into(), "nfc".into()] }]
        );
        assert_eq!(o.user_verification, UserVerification::Required);
        assert_eq!(o.timeout_ms, Some(60000));
        let dbg = format!("{o:?}");
        assert!(dbg.contains("challenge_len") && !dbg.contains("239"), "{dbg}");

        // in `publicKey`, Standard-Base64, ohne rpId → Host der Origin
        let o =
            AssertionOptions::parse(r#"{"publicKey":{"challenge":"AAEC/+8=","allowCredentials":[]}}"#, origin).unwrap();
        assert_eq!(o.challenge, vec![0, 1, 2, 0xff, 0xef]);
        assert_eq!(o.rp_id, "service.a-trust.at");
        assert_eq!(o.user_verification, UserVerification::Preferred);
        // Zahlen-Array
        let o = AssertionOptions::parse(r#"{"challenge":[1,2,255],"userVerification":"discouraged"}"#, origin).unwrap();
        assert_eq!(o.challenge, vec![1, 2, 255]);
        assert_eq!(o.user_verification, UserVerification::Discouraged);

        for bad in ["", "[]", r#"{"challenge":""}"#, r#"{"challenge":"***"}"#, r#"{"challenge":[256]}"#, "{}"] {
            assert!(matches!(AssertionOptions::parse(bad, origin), Err(Fido2Error::InvalidOptions(_))), "{bad}");
        }
    }

    #[test]
    fn client_data_like_a_browser() {
        let cd = client_data_json(&[0xff, 0xee, 0x01], "https://service.a-trust.at");
        assert_eq!(
            String::from_utf8(cd).unwrap(),
            r#"{"type":"webauthn.get","challenge":"_-4B","origin":"https://service.a-trust.at","crossOrigin":false}"#
        );
        assert_eq!(credential_id_string(&[0xfb, 0xff]), "-_8");
    }
}
