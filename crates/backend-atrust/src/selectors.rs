//! Versionierte Selektor-Datei (Plan §4.4): Element-IDs und URL-Muster aus docs/analyse/02 §8, damit
//! Seitenänderungen ohne Code-Release behoben werden können (signiert, mit dem Update-Manifest).

/// Selektoren und URL-Muster.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectorSet {
    /// Version der Datei
    pub version: u32,
    /// URL-Teile für AutoSkip (`/tanAppInfo.aspx`, `/UsernameInfo.aspx`)
    pub autoskip_urls: Vec<String>,
    /// URL-Teile für Interstitial (`/ExpiresInfo.aspx`)
    pub interstitial_urls: Vec<String>,
    /// Fehlerseiten: Präfix des letzten Pfadsegments (`/error`) und exakte Seiten (`/SessionClosed.aspx`)
    pub error_prefix: String,
    /// exakte Fehlerseiten
    pub error_pages: Vec<String>,
    /// Element-IDs (`handynummer`, `signaturpasswort`, `Button_Identification`, …)
    pub ids: ElementIds,
}

/// Element-IDs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)]
pub struct ElementIds {
    pub username: String,
    pub password: String,
    pub button_identification: String,
    pub button_cancel: String,
    pub button_back: String,
    pub button_next: String,
    pub next_btn: String,
    pub input_tan: String,
    pub sign_button: String,
    pub reference_value: String,
    pub qr_image: String,
    pub long_poll: String,
    pub smartphone_animation: String,
    pub biometric_image: String,
    pub sign_type: String,
    pub fido_block: String,
    pub credential_options: String,
    pub credential_result: String,
    pub fido_continue: String,
    pub sms_button: String,
    pub fido_button: String,
    pub link_list: String,
    pub label1: String,
    pub label_detail: String,
    pub label2: String,
    pub signature_data_href: String,
}

impl SelectorSet {
    /// Eingebettete Standarddatei (Stand PDF-Over 4.4.x).
    pub fn builtin() -> SelectorSet {
        todo!("Paket D: include_str!(\"../selectors/atrust-v1.json\")")
    }

    /// Aus JSON (signiert geladene Aktualisierung).
    pub fn from_json(json: &str) -> Result<SelectorSet, serde_json::Error> {
        serde_json::from_str(json)
    }
}
