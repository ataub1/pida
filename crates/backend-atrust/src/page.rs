//! Klassifikation der A-Trust-HTML-Seiten (docs/analyse/02 §3.5; PDF-Over `ATrustParser`).

use pdfover_net::Url;

use crate::poll::PollInfo;
use crate::selectors::SelectorSet;

/// Ergebnis der Seitenauswertung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageBlock {
    /// `/tanAppInfo.aspx`, `/UsernameInfo.aspx` → automatisch mit `#NextBtn` absenden.
    AutoSkip {
        /// Name des Submit-Buttons
        submit: Option<SubmitButton>,
    },
    /// `/ExpiresInfo.aspx` → Info-Dialog (`#Label2`), dann `#Button_Next`.
    Interstitial {
        /// Text
        message: String,
        /// Weiter-Button
        next: Option<SubmitButton>,
    },
    /// `/error*.aspx`, `/SessionClosed.aspx`.
    Error {
        /// `#Label1` + optional `\n#LabelDetail`
        message: String,
        /// `#Button_Back` vorhanden (Wiederholen möglich)
        back: Option<SubmitButton>,
        /// `#Button_Cancel` vorhanden (Abbruch an A-Trust melden)
        cancel: Option<SubmitButton>,
    },
    /// `#handynummer` + `#signaturpasswort` (Feldnamen aus `@name`).
    UsernamePassword {
        /// Feldname Benutzer
        username_field: String,
        /// Feldname Passwort
        password_field: String,
        /// `#Button_Identification`
        submit: Option<SubmitButton>,
        /// `#Button_Cancel`
        cancel: Option<SubmitButton>,
        /// Fehlermeldung auf der Login-Seite (PDF-Over wertet sie nicht aus; der Klon schon)
        error_message: Option<String>,
    },
    /// `#input_tan` + `#vergleichswert`.
    SmsTan {
        /// Feldname TAN
        tan_field: String,
        /// Vergleichswert
        reference_value: String,
        /// `#SignButton`
        submit: Option<SubmitButton>,
    },
    /// `#qrimage` (+ `#jsLongPoll script`).
    QrCode {
        /// Vergleichswert
        reference_value: String,
        /// QR-Bild-URL (absolut)
        image_url: Url,
        /// Poll-Info
        poll: Option<PollInfo>,
    },
    /// `#smartphoneAnimation` | `#biometricimage` | `.ida-signtype #SignType`.
    WaitingForApp2Fa {
        /// Vergleichswert
        reference_value: Option<String>,
        /// Poll-Info
        poll: Option<PollInfo>,
    },
    /// `#fidoBlock`.
    Fido2 {
        /// `#credentialOptions@value` (JSON, unverändert)
        options_json: String,
        /// `#credentialResult@name`
        result_field: String,
        /// `#FidoContinue`
        submit: Option<SubmitButton>,
    },
}

/// „Geklickter“ Submit-Button (`<input type=submit>` mit nichtleerem `name`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitButton {
    /// `name`
    pub name: String,
    /// `value`
    pub value: String,
}

/// Ausgewertete Seite. `Debug` zeigt von `form_fields` nur die Namen (Werte können Benutzername,
/// `__VIEWSTATE` u. Ä. enthalten; ADR 0001 §4 Regel 9).
#[derive(Clone, PartialEq, Eq)]
pub struct ParsedPage {
    /// Basis-URL (Request-URI)
    pub base_url: Url,
    /// `form@action` (absolut)
    pub form_action: Url,
    /// alle `input` mit nichtleerem `name`, `type` ≠ submit (inkl. Hidden-Felder wie `__VIEWSTATE`;
    /// Checkboxen unabhängig von `checked`; `select`/`textarea` nicht)
    pub form_fields: Vec<(String, String)>,
    /// `#LinkList a[href*="ShowSigobj.aspx"]`
    pub signature_data_link: Option<Url>,
    /// `#SmsButton@href`
    pub sms_link: Option<Url>,
    /// `#FidoButton@href`
    pub fido2_link: Option<Url>,
    /// Block
    pub block: PageBlock,
}

impl std::fmt::Debug for ParsedPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedPage")
            .field("base_url", &self.base_url.path())
            .field("form_action", &self.form_action.path())
            .field("form_fields", &self.form_fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>())
            .field("signature_data_link", &self.signature_data_link.is_some())
            .field("sms_link", &self.sms_link.is_some())
            .field("fido2_link", &self.fido2_link.is_some())
            .field("block", &self.block)
            .finish()
    }
}

/// Fehler der Seitenauswertung (→ `BackendError::UnknownPage` mit Diagnose).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PageError {
    /// ≠ 1 Formular.
    #[error("{0} Formulare statt genau einem")]
    FormCount(usize),
    /// 0 oder > 1 Blöcke erkannt.
    #[error("{0} Blöcke erkannt")]
    BlockCount(usize),
    /// Pflichtattribut fehlt (z. B. `#handynummer@name`).
    #[error("Attribut fehlt: {0}")]
    MissingAttribute(String),
    /// URL ungültig.
    #[error("URL ungültig: {0}")]
    InvalidUrl(String),
}

/// Klassifiziert eine `text/html`-Seite.
pub fn classify(base_url: &Url, html: &str, selectors: &SelectorSet) -> Result<ParsedPage, PageError> {
    todo!("Paket D: scraper")
}

/// Diagnosecode ohne Personendaten (Pfad, gefundene Element-IDs).
pub fn diagnostic(base_url: &Url, html: &str) -> String {
    todo!("Paket D")
}
