//! Klassifikation der A-Trust-HTML-Seiten (docs/analyse/02 §3.5; PDF-Over `ATrustParser`).
//!
//! Regeln (in dieser Reihenfolge):
//! 1. genau **ein** `<form>` ([`PageError::FormCount`]),
//! 2. `form@action` absolut aufgelöst (fehlt `action`, gilt die Dokument-URL),
//! 3. Formularfelder: alle `input` im Formular mit nichtleerem `name` und `type` ≠ `submit`
//!    (Groß-/Kleinschreibung egal), `value` oder leer; gleiche Namen überschreiben den Wert an der
//!    ersten Position (Java-`HashMap.put`); `select`/`textarea` werden nicht übernommen,
//!    Checkboxen unabhängig von `checked`,
//! 4. optionale Links (`absUrl("href")`): Signaturdaten, SMS, FIDO2,
//! 5. alle acht Block-Parser; **genau einer** muss greifen ([`PageError::BlockCount`]).
//!
//! **Sitzung beendet** (ADR 0004 Nachtrag 19, Abweichung von PDF-Over): Eine Seite unter
//! [`SelectorSet::session_closed_pages`] (`/SessionClosed.aspx`) ist **vor** Regel 1 eine
//! [`PageBlock::SessionClosed`] – mit oder ohne Formular und `#Label1`. Ebenso eine Fehlerseite
//! (`/error*.aspx`), die keinen Fehlerblock ergibt (≠ 1 Formular bzw. ohne `#Label1`) und auf der auch sonst
//! kein Block greift. PDF-Over (`ATrustParser.ErrorBlock`) verlangt genau ein Formular und `#Label1` und bricht
//! sonst mit „Unknown A-Trust page reached?“ ab; mit `#Label1` zeigt es den Text und bietet über
//! `#Button_Back` „Wiederholen“ – in einer geschlossenen Sitzung führt das nicht weiter, nur eine neue.
//!
//! Die Dokument-Basis ist die Request-URL bzw. – wie bei jsoup – das erste `<base href>`.
//! Element-Texte folgen jsoup `ownText()` (direkte Textknoten, Leerraum zusammengefasst, `<br>` als
//! Leerzeichen, getrimmt).

use pida_net::Url;
use scraper::{ElementRef, Html, Node, Selector};

use crate::poll::{PollInfo, parse_poll_script_named};
use crate::selectors::SelectorSet;

/// Ergebnis der Seitenauswertung. `Debug` ohne Texte von A-Trust und ohne FIDO2-Optionen (Regel 9).
#[derive(Clone, PartialEq, Eq)]
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
    /// `/error*.aspx` mit genau einem Formular und `#Label1` (`/SessionClosed.aspx` ist seit Nachtrag 19 immer
    /// [`PageBlock::SessionClosed`]).
    Error {
        /// `#Label1` + optional `\n#LabelDetail`
        message: String,
        /// `#Button_Back` als gültiger Submit-Button
        back: Option<SubmitButton>,
        /// `#Button_Cancel` als gültiger Submit-Button
        cancel: Option<SubmitButton>,
        /// `#Button_Back` existiert (Wiederholen möglich, `isRecoverable`)
        recoverable: bool,
        /// `#Button_Cancel` existiert (Abbruch an A-Trust melden, `requiresResponse`)
        requires_response: bool,
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
        /// Fehlermeldung auf der Login-Seite (PDF-Over wertet sie nicht aus; der Klon über
        /// [`SelectorSet::login_error`])
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
    /// `#qrimage` + `#vergleichswert` + `#jsLongPoll script` mit `qrpoll("URL", "TOKEN");` (alle
    /// Pflicht wie `ATrustParser.QRCodeBlock`).
    QrCode {
        /// Vergleichswert
        reference_value: String,
        /// QR-Bild-URL (absolut)
        image_url: Url,
        /// Poll-Info; [`classify`] erkennt den Block nur mit gültigem Poll-Skript, der Wert ist
        /// daher immer `Some` (Option nur aus Gründen der API-Stabilität).
        poll: Option<PollInfo>,
    },
    /// `#smartphoneAnimation` | `#biometricimage` | `.ida-signtype #SignType`, dazu Pflicht
    /// `#vergleichswert` und Poll-Skript (wie `ATrustParser.WaitingForApp2FABlock`).
    WaitingForApp2Fa {
        /// Vergleichswert ([`classify`]: immer `Some`)
        reference_value: Option<String>,
        /// Poll-Info ([`classify`]: immer `Some`)
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
    /// Sitzung beendet bzw. abgelaufen (Nachtrag 19): [`SelectorSet::session_closed_pages`] (`/SessionClosed.aspx`,
    /// mit oder ohne Formular und `#Label1`) oder eine Fehlerseite (`/error*.aspx`) ohne Fehlerblock. Weiter geht
    /// es nur mit einer neuen Sitzung.
    SessionClosed {
        /// `true` für [`SelectorSet::session_closed_pages`] (die Sitzung ist sicher beendet – nach dem Absenden
        /// der Anmeldedaten darf der Dialog sie einmal automatisch erneuern), `false` für eine Fehlerseite ohne
        /// Meldung
        closed_page: bool,
        /// `#Label1` (+ `\n#LabelDetail`), falls vorhanden – Text von A-Trust, nie im Log
        message: Option<String>,
        /// Diagnose ohne Personendaten ([`diagnostic`]: Pfad, Formularzahl, Element-IDs)
        diagnostic: String,
    },
}

impl std::fmt::Debug for PageBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct(self.kind());
        match self {
            PageBlock::AutoSkip { submit } => d.field("submit", submit),
            PageBlock::Interstitial { message, next } => d.field("message_len", &message.len()).field("next", next),
            PageBlock::Error { message, back, cancel, recoverable, requires_response } => d
                .field("message_len", &message.len())
                .field("back", back)
                .field("cancel", cancel)
                .field("recoverable", recoverable)
                .field("requires_response", requires_response),
            PageBlock::UsernamePassword { username_field, password_field, submit, cancel, error_message } => d
                .field("username_field", username_field)
                .field("password_field", password_field)
                .field("submit", submit)
                .field("cancel", cancel)
                .field("error_message_len", &error_message.as_ref().map(String::len)),
            PageBlock::SmsTan { tan_field, reference_value, submit } => {
                d.field("tan_field", tan_field).field("reference_value", reference_value).field("submit", submit)
            }
            PageBlock::QrCode { reference_value, image_url, poll } => d
                .field("reference_value", reference_value)
                .field("image_url", &pida_net::redact_url(image_url))
                .field("poll", poll),
            PageBlock::WaitingForApp2Fa { reference_value, poll } => {
                d.field("reference_value", reference_value).field("poll", poll)
            }
            PageBlock::Fido2 { options_json, result_field, submit } => {
                d.field("options_len", &options_json.len()).field("result_field", result_field).field("submit", submit)
            }
            PageBlock::SessionClosed { closed_page, message, diagnostic } => d
                .field("closed_page", closed_page)
                .field("message_len", &message.as_ref().map(String::len))
                .field("diagnostic", diagnostic),
        };
        d.finish()
    }
}

impl PageBlock {
    /// Kurzname für Diagnose und Logs.
    pub fn kind(&self) -> &'static str {
        match self {
            PageBlock::AutoSkip { .. } => "AutoSkip",
            PageBlock::Interstitial { .. } => "Interstitial",
            PageBlock::Error { .. } => "Error",
            PageBlock::UsernamePassword { .. } => "UsernamePassword",
            PageBlock::SmsTan { .. } => "SmsTan",
            PageBlock::QrCode { .. } => "QrCode",
            PageBlock::WaitingForApp2Fa { .. } => "WaitingForApp2Fa",
            PageBlock::Fido2 { .. } => "Fido2",
            PageBlock::SessionClosed { .. } => "SessionClosed",
        }
    }
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
    /// Basis-URL (Request-URI bzw. `<base href>`)
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
            .field("block", &self.block.kind())
            .finish()
    }
}

impl ParsedPage {
    /// Setzt ein Formularfeld (Java `formOptions.put`: vorhandenen Wert ersetzen, sonst anhängen).
    pub fn set_field(&mut self, name: &str, value: impl Into<String>) {
        set_field(&mut self.form_fields, name, value.into());
    }
}

fn set_field(fields: &mut Vec<(String, String)>, name: &str, value: String) {
    match fields.iter_mut().find(|(k, _)| k == name) {
        Some((_, v)) => *v = value,
        None => fields.push((name.to_owned(), value)),
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
    /// Selektor der Selektor-Datei ist kein gültiges CSS.
    #[error("Selektor ungültig: {0}")]
    InvalidSelector(String),
}

/// Vorkompilierte Selektoren eines [`SelectorSet`].
struct Compiled<'s> {
    set: &'s SelectorSet,
}

impl Compiled<'_> {
    fn sel(&self, css: &str) -> Result<Selector, PageError> {
        Selector::parse(css).map_err(|e| PageError::InvalidSelector(format!("{css}: {e}")))
    }
}

/// jsoup `ownText()`: direkte Textknoten mit zusammengefasstem Leerraum, `<br>` als Leerzeichen,
/// anschließend Java-`trim()`.
/// Vergleichswert ohne Leerzeichen (Entscheidung des Auftraggebers: A-Trust gliedert ihn mit Leerzeichen,
/// PIDA zeigt ihn kompakt; der TAN-Abgleich `EnteredReferenceValue` vergleicht ebenfalls ohne Leerzeichen).
pub(crate) fn reference_text(element: ElementRef<'_>) -> String {
    own_text(element).chars().filter(|c| !c.is_whitespace()).collect()
}

pub(crate) fn own_text(element: ElementRef<'_>) -> String {
    let preserve = preserves_whitespace(element);
    let mut acc = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                if preserve {
                    acc.push_str(text);
                } else {
                    append_normalised(&mut acc, text);
                }
            }
            Node::Element(e) if e.name() == "br" && !acc.ends_with(' ') => acc.push(' '),
            _ => {}
        }
    }
    java_trim(&acc).to_owned()
}

/// Java `String.trim()`: entfernt Zeichen ≤ U+0020 an beiden Enden.
pub(crate) fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= '\u{20}')
}

fn preserves_whitespace(element: ElementRef<'_>) -> bool {
    let mut node = Some(*element);
    for _ in 0..6 {
        let Some(n) = node else { break };
        if let Node::Element(e) = n.value()
            && matches!(e.name(), "pre" | "plaintext" | "title" | "textarea")
        {
            return true;
        }
        node = n.parent();
    }
    false
}

fn append_normalised(acc: &mut String, text: &str) {
    let strip_leading = acc.ends_with(' ');
    let mut last_was_white = false;
    let mut reached_non_white = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\u{0C}' | '\r' | '\u{A0}') {
            if (strip_leading && !reached_non_white) || last_was_white {
                continue;
            }
            acc.push(' ');
            last_was_white = true;
        } else if !matches!(c, '\u{200B}' | '\u{AD}') {
            acc.push(c);
            last_was_white = false;
            reached_non_white = true;
        }
    }
}

/// Textinhalt eines Skripts (jsoup `data()`).
fn script_data(element: ElementRef<'_>) -> String {
    element.children().filter_map(|c| if let Node::Text(t) = c.value() { Some(&**t) } else { None }).collect()
}

/// Dokument-Basis: erstes `<base href>` (absolut aufgelöst), sonst die Request-URL.
fn effective_base(doc: &Html, request_url: &Url) -> Url {
    let Ok(base_sel) = Selector::parse("base[href]") else { return request_url.clone() };
    doc.select(&base_sel)
        .next()
        .and_then(|b| b.attr("href"))
        .filter(|h| !h.trim().is_empty())
        .and_then(|h| pida_net::resolve(request_url, h).ok())
        .unwrap_or_else(|| request_url.clone())
}

fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

struct Ctx<'a> {
    doc: &'a Html,
    base: Url,
    c: Compiled<'a>,
}

impl<'a> Ctx<'a> {
    fn first(&self, css: &str) -> Result<Option<ElementRef<'a>>, PageError> {
        let sel = self.c.sel(css)?;
        Ok(self.doc.select(&sel).next())
    }

    fn exists(&self, css: &str) -> Result<bool, PageError> {
        Ok(self.first(css)?.is_some())
    }

    /// `absUrl("href")`; nur `http(s)` – `javascript:`/`data:` u. Ä. gelten als fehlender Link (PDF-Over
    /// hätte daraus einen nicht ausführbaren Request gebaut).
    fn href(&self, css: &str) -> Result<Option<Url>, PageError> {
        Ok(self
            .first(css)?
            .and_then(|e| e.attr("href"))
            .and_then(|h| pida_net::resolve(&self.base, h).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https")))
    }

    /// `buildFormSubmit`: nur `<input type=submit>` mit nichtleerem `name`, sonst ohne Button.
    fn submit_button(&self, css: &str) -> Result<Option<SubmitButton>, PageError> {
        let Some(el) = self.first(css)? else {
            tracing::warn!(selector = css, "Submit-Button nicht gefunden – Formular wird ohne Button gesendet");
            return Ok(None);
        };
        if el.value().name() != "input" {
            tracing::warn!(selector = css, tag = el.value().name(), "Submit-Button ist kein <input>");
            return Ok(None);
        }
        if !el.attr("type").is_some_and(|t| t.eq_ignore_ascii_case("submit")) {
            tracing::warn!(selector = css, "Submit-Button hat nicht type=submit");
            return Ok(None);
        }
        let name = el.attr("name").unwrap_or_default();
        if name.is_empty() {
            return Ok(None);
        }
        Ok(Some(SubmitButton { name: name.to_owned(), value: el.attr("value").unwrap_or_default().to_owned() }))
    }

    fn poll_info(&self) -> Result<Option<PollInfo>, PageError> {
        let set = self.c.set;
        Ok(self
            .first(&set.ids.long_poll)?
            .and_then(|script| parse_poll_script_named(&set.poll_function, &script_data(script), &self.base)))
    }

    fn base_contains_any(&self, patterns: &[String]) -> bool {
        patterns.iter().any(|p| !p.is_empty() && self.base.as_str().contains(p.as_str()))
    }

    fn auto_skip(&self) -> Result<Option<PageBlock>, PageError> {
        if !self.base_contains_any(&self.c.set.autoskip_urls) {
            return Ok(None);
        }
        Ok(Some(PageBlock::AutoSkip { submit: self.submit_button(&self.c.set.ids.next_btn)? }))
    }

    fn interstitial(&self) -> Result<Option<PageBlock>, PageError> {
        let ids = &self.c.set.ids;
        if !self.base_contains_any(&self.c.set.interstitial_urls) {
            return Ok(None);
        }
        let Some(label) = self.first(&ids.label2)? else { return Ok(None) };
        Ok(Some(PageBlock::Interstitial { message: own_text(label), next: self.submit_button(&ids.button_next)? }))
    }

    /// Letztes Pfadsegment der Dokument-Basis (mit `/`, Prozentkodierung aufgelöst).
    fn last_segment(&self) -> String {
        let path = self.base.path();
        percent_decode(&path[path.rfind('/').unwrap_or(0)..])
    }

    /// `/error*.aspx` bzw. exakte Fehlerseite (`ATrustParser.ErrorBlock`).
    fn is_error_page(&self) -> bool {
        let set = self.c.set;
        let last = self.last_segment();
        (last.starts_with(set.error_prefix.as_str()) && last.ends_with(".aspx")) || set.error_pages.contains(&last)
    }

    /// Seite einer beendeten Sitzung ([`SelectorSet::session_closed_pages`], exakt wie `error_pages`).
    fn is_session_closed_page(&self) -> bool {
        let last = self.last_segment();
        self.c.set.session_closed_pages.contains(&last)
    }

    /// `#Label1` + optional `\n#LabelDetail` (jsoup `ownText().trim()`).
    fn error_text(&self) -> Result<Option<String>, PageError> {
        let set = self.c.set;
        let Some(label1) = self.first(&set.ids.label1)? else { return Ok(None) };
        let mut message = java_trim(&own_text(label1)).to_owned();
        if let Some(detail) = self.first(&set.ids.label_detail)? {
            message.push('\n');
            message.push_str(java_trim(&own_text(detail)));
        }
        Ok(Some(message))
    }

    fn session_closed(&self, closed_page: bool) -> Result<PageBlock, PageError> {
        Ok(PageBlock::SessionClosed {
            closed_page,
            message: self.error_text()?.filter(|m| !m.trim().is_empty()),
            diagnostic: diagnostic_of(self.doc, &self.base),
        })
    }

    fn error(&self) -> Result<Option<PageBlock>, PageError> {
        let set = self.c.set;
        if !self.is_error_page() {
            return Ok(None);
        }
        let Some(message) = self.error_text()? else { return Ok(None) };
        Ok(Some(PageBlock::Error {
            message,
            back: self.submit_button(&set.ids.button_back)?,
            cancel: self.submit_button(&set.ids.button_cancel)?,
            recoverable: self.exists(&set.ids.button_back)?,
            requires_response: self.exists(&set.ids.button_cancel)?,
        }))
    }

    fn username_password(&self) -> Result<Option<PageBlock>, PageError> {
        let set = self.c.set;
        let Some(user) = self.first(&set.ids.username)? else { return Ok(None) };
        let Some(username_field) = user.attr("name") else { return Ok(None) };
        let Some(password_field) = self.first(&set.ids.password)?.and_then(|p| p.attr("name")) else {
            return Ok(None);
        };
        let error_message = if set.login_error.trim().is_empty() {
            None
        } else {
            self.first(&set.login_error)?.map(own_text).filter(|t| !t.is_empty())
        };
        Ok(Some(PageBlock::UsernamePassword {
            username_field: username_field.to_owned(),
            password_field: password_field.to_owned(),
            submit: self.submit_button(&set.ids.button_identification)?,
            cancel: self.submit_button(&set.ids.button_cancel)?,
            error_message,
        }))
    }

    fn sms_tan(&self) -> Result<Option<PageBlock>, PageError> {
        let ids = &self.c.set.ids;
        let Some(tan) = self.first(&ids.input_tan)? else { return Ok(None) };
        let Some(tan_field) = tan.attr("name") else { return Ok(None) };
        let Some(reference) = self.first(&ids.reference_value)? else { return Ok(None) };
        Ok(Some(PageBlock::SmsTan {
            tan_field: tan_field.to_owned(),
            reference_value: reference_text(reference),
            submit: self.submit_button(&ids.sign_button)?,
        }))
    }

    fn qr_code(&self) -> Result<Option<PageBlock>, PageError> {
        let ids = &self.c.set.ids;
        let Some(image) = self.first(&ids.qr_image)? else { return Ok(None) };
        let Some(reference) = self.first(&ids.reference_value)? else { return Ok(None) };
        let Some(image_url) =
            image.attr("src").filter(|s| !s.trim().is_empty()).and_then(|src| pida_net::resolve(&self.base, src).ok())
        else {
            return Ok(None);
        };
        // Ohne gültiges Poll-Skript kein QR-Block (PDF-Over `getLongPollInfo` → ComponentParseFailed):
        // eine tolerantere Erkennung könnte auf echten Seiten zusätzliche Blöcke finden und so aus
        // einer eindeutigen Seite eine „unbekannte“ machen.
        let Some(poll) = self.poll_info()? else { return Ok(None) };
        Ok(Some(PageBlock::QrCode { reference_value: reference_text(reference), image_url, poll: Some(poll) }))
    }

    fn waiting_for_app(&self) -> Result<Option<PageBlock>, PageError> {
        let ids = &self.c.set.ids;
        let any = format!("{}, {}, {}", ids.smartphone_animation, ids.biometric_image, ids.sign_type);
        if !self.exists(&any)? {
            return Ok(None);
        }
        // Vergleichswert und Poll-Skript sind Pflicht wie in PDF-Over.
        let Some(reference) = self.first(&ids.reference_value)? else { return Ok(None) };
        let Some(poll) = self.poll_info()? else { return Ok(None) };
        Ok(Some(PageBlock::WaitingForApp2Fa { reference_value: Some(reference_text(reference)), poll: Some(poll) }))
    }

    fn fido2(&self) -> Result<Option<PageBlock>, PageError> {
        let ids = &self.c.set.ids;
        if !self.exists(&ids.fido_block)? {
            return Ok(None);
        }
        let Some(options_json) = self.first(&ids.credential_options)?.and_then(|e| e.attr("value")) else {
            return Ok(None);
        };
        let Some(result_field) = self.first(&ids.credential_result)?.and_then(|e| e.attr("name")) else {
            return Ok(None);
        };
        Ok(Some(PageBlock::Fido2 {
            options_json: options_json.to_owned(),
            result_field: result_field.to_owned(),
            submit: self.submit_button(&ids.fido_continue)?,
        }))
    }
}

/// Klassifiziert eine `text/html`-Seite.
pub fn classify(base_url: &Url, html: &str, selectors: &SelectorSet) -> Result<ParsedPage, PageError> {
    let doc = Html::parse_document(html);
    let base = effective_base(&doc, base_url);
    let ctx = Ctx { doc: &doc, base: base.clone(), c: Compiled { set: selectors } };

    let form_sel = ctx.c.sel("form")?;
    let forms: Vec<ElementRef<'_>> = doc.select(&form_sel).collect();
    // Nachtrag 19: beendete Sitzung vor der Formularprüfung (`SessionClosed.aspx` ohne Formular)
    let ended = if ctx.is_session_closed_page() {
        Some(ctx.session_closed(true)?)
    } else if forms.len() != 1 && ctx.is_error_page() {
        Some(ctx.session_closed(false)?)
    } else {
        None
    };
    if let Some(block) = ended {
        return Ok(ended_page(base, block));
    }
    if forms.len() != 1 {
        return Err(PageError::FormCount(forms.len()));
    }
    let form = forms[0];
    let form_action = match form.attr("action") {
        Some(action) => pida_net::resolve(&base, action).map_err(|e| PageError::InvalidUrl(e.to_string()))?,
        None => base.clone(),
    };

    let input_sel = ctx.c.sel("input")?;
    let mut form_fields: Vec<(String, String)> = Vec::new();
    for input in form.select(&input_sel) {
        let name = input.attr("name").unwrap_or_default();
        if name.is_empty() || input.attr("type").is_some_and(|t| t.eq_ignore_ascii_case("submit")) {
            continue;
        }
        set_field(&mut form_fields, name, input.attr("value").unwrap_or_default().to_owned());
    }

    let signature_data_link = ctx.href(&selectors.signature_data_link_css())?;
    let sms_link = ctx.href(&selectors.ids.sms_button)?;
    let fido2_link = ctx.href(&selectors.ids.fido_button)?;

    let candidates = [
        ctx.auto_skip()?,
        ctx.interstitial()?,
        ctx.error()?,
        ctx.username_password()?,
        ctx.sms_tan()?,
        ctx.qr_code()?,
        ctx.waiting_for_app()?,
        ctx.fido2()?,
    ];
    let mut found: Vec<PageBlock> = candidates.into_iter().flatten().collect();
    if found.is_empty() && ctx.is_error_page() {
        // Fehlerseite ohne `#Label1` (und ohne anderen Block): Sitzung beendet (Nachtrag 19)
        found.push(ctx.session_closed(false)?);
    }
    if found.len() != 1 {
        tracing::error!(
            blocks = ?found.iter().map(PageBlock::kind).collect::<Vec<_>>(),
            path = base.path(),
            "unbekannte A-Trust-Seite"
        );
        return Err(PageError::BlockCount(found.len()));
    }
    let block = found.remove(0);
    Ok(ParsedPage { base_url: base, form_action, form_fields, signature_data_link, sms_link, fido2_link, block })
}

/// Seite einer beendeten Sitzung: ohne Formularfelder und Links (nichts davon führt weiter).
fn ended_page(base: Url, block: PageBlock) -> ParsedPage {
    ParsedPage {
        form_action: base.clone(),
        base_url: base,
        form_fields: Vec::new(),
        signature_data_link: None,
        sms_link: None,
        fido2_link: None,
        block,
    }
}

/// Diagnosecode ohne Personendaten (Pfad ohne Abfrage, Formularzahl, gefundene Element-IDs).
pub fn diagnostic(base_url: &Url, html: &str) -> String {
    diagnostic_of(&Html::parse_document(html), base_url)
}

fn diagnostic_of(doc: &Html, base_url: &Url) -> String {
    let forms = Selector::parse("form").map(|s| doc.select(&s).count()).unwrap_or(0);
    let mut ids: Vec<String> = Vec::new();
    if let Ok(all) = Selector::parse("[id]") {
        for element in doc.select(&all) {
            let Some(id) = element.value().id() else { continue };
            let safe = !id.is_empty()
                && id.len() <= 64
                && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.' | '$'))
                && !id.chars().all(|c| c.is_ascii_digit());
            if safe && !ids.iter().any(|i| i == id) {
                ids.push(id.to_owned());
            }
            if ids.len() >= 40 {
                break;
            }
        }
    }
    format!("path={} forms={} ids=[{}]", base_url.path(), forms, ids.join(","))
}

/// Höchstzahl der Einträge von [`link_targets`].
const MAX_LINK_TARGETS: usize = 40;

/// Diagnose für einen möglichen App-Link (ADR 0004 Nachtrag 19 Punkt 4; nur fürs Debug-Log der
/// App-Warteseite, kein Verhalten): die **verschiedenen** Link-Ziele einer Seite als `quelle ziel`, das Ziel nur
/// als Schema, Host (mit Port) und Pfad – **ohne** Abfrage und Fragment. Quellen: `<a href>`, `<area href>`,
/// Formular-`action`, `<iframe src>`, `<script src>`, `data-*`-Attribute mit URL-Wert (`://`, `/…`, `./…`,
/// `intent:`) und Zeichenketten im Inline-Skript (absolute URLs und `intent:` bzw. Ziele von `location = …`,
/// `location.href = …`, `location.replace(…)`, `location.assign(…)`, `window.open(…)`). Ohne `mailto:`, `tel:`,
/// `sms:` (Personendaten) und ohne `javascript:`, `data:`, `blob:`, `about:`; Ziele ohne Host nur mit dem Schema
/// (`idaustria:`). Host und Pfadsegmente, die Personendaten oder Kennungen sein könnten (`@`, sechs oder mehr
/// Ziffern in Folge, Segment länger als 32 Zeichen), erscheinen als `…`. Höchstens 40 Einträge, Pfade auf 120
/// Zeichen gekürzt.
pub fn link_targets(base_url: &Url, html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let base = effective_base(&doc, base_url);
    let mut out: Vec<String> = Vec::new();
    let mut add = |source: &str, raw: &str| {
        if out.len() >= MAX_LINK_TARGETS {
            return;
        }
        if let Some(target) = link_target(&base, raw) {
            let entry = format!("{source} {target}");
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    };
    for (css, attr, source) in [
        ("a[href]", "href", "a"),
        ("area[href]", "href", "area"),
        ("form[action]", "action", "form"),
        ("iframe[src]", "src", "iframe"),
        ("script[src]", "src", "script-src"),
    ] {
        let Ok(sel) = Selector::parse(css) else { continue };
        for el in doc.select(&sel) {
            if let Some(value) = el.attr(attr) {
                add(source, value);
            }
        }
    }
    if let Ok(all) = Selector::parse("*") {
        for el in doc.select(&all) {
            for (name, value) in el.value().attrs() {
                let safe_name = name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
                if name.starts_with("data-") && safe_name && looks_like_url(value) {
                    add(name, value);
                }
            }
        }
    }
    if let Ok(sel) = Selector::parse("script") {
        for el in doc.select(&sel) {
            for literal in script_urls(&script_data(el)) {
                add("script", &literal);
            }
        }
    }
    out
}

/// Ziel ohne Abfrage und Fragment (`scheme://host[:port]/pfad`); `None` für Personendaten und Nicht-Ziele.
fn link_target(base: &Url, raw: &str) -> Option<String> {
    let raw = raw.trim();
    let lower = raw.to_ascii_lowercase();
    const SKIP: &[&str] = &["javascript:", "data:", "blob:", "about:", "mailto:", "tel:", "sms:", "#"];
    if raw.is_empty() || SKIP.iter().any(|p| lower.starts_with(p)) {
        return None;
    }
    let url = pida_net::resolve(base, raw).ok()?;
    if matches!(url.scheme(), "mailto" | "tel" | "sms" | "javascript" | "data" | "blob" | "about") {
        return None;
    }
    let Some(host) = url.host_str() else { return Some(format!("{}:", url.scheme())) };
    let host = if looks_personal(host) { "…" } else { host };
    let mut target = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        target.push_str(&format!(":{port}"));
    }
    let path = url
        .path()
        .split('/')
        .map(|segment| if segment.chars().count() > 32 || looks_personal(segment) { "…" } else { segment })
        .collect::<Vec<_>>()
        .join("/");
    target.extend(path.chars().take(120));
    Some(target)
}

/// Kann Personendaten oder eine Kennung tragen (E-Mail-Adresse, Telefonnummer, Konto- bzw. Vorgangsnummer):
/// enthält `@` (auch `%40`) oder sechs bzw. mehr Ziffern in Folge – dann nur `…` in [`link_targets`].
fn looks_personal(text: &str) -> bool {
    let mut digits = 0;
    for c in text.chars() {
        digits = if c.is_ascii_digit() { digits + 1 } else { 0 };
        if digits >= 6 {
            return true;
        }
    }
    text.contains('@') || text.to_ascii_lowercase().contains("%40")
}

/// URL-artiger Attributwert (absolut, Wurzel- bzw. relativer Pfad, `intent:`).
fn looks_like_url(value: &str) -> bool {
    let v = value.trim();
    v.contains("://")
        || v.starts_with('/')
        || v.starts_with("./")
        || v.starts_with("../")
        || v.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("intent:"))
}

/// Zeichenketten eines Inline-Skripts, die ein Ziel sein können: URL-artig (`://`, `intent:`) oder Ziel einer
/// Navigation (`location = "…"`, `location.href = "…"`, `location.replace("…")`, `location.assign("…")`,
/// `window.open("…")`). Einfache Lexik ohne Kommentare – reicht für die Diagnose.
fn script_urls(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() && out.len() < MAX_LINK_TARGETS {
        let quote = bytes[i];
        if !matches!(quote, b'"' | b'\'' | b'`') {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != quote && bytes[j] != b'\n' {
            if bytes[j] == b'\\' {
                j += 1;
            }
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        if bytes[j] == quote {
            // `start` folgt einem ASCII-Anführungszeichen, `j` steht auf einem: beides Zeichengrenzen
            let literal = &text[start..j];
            let lower = literal.trim().to_ascii_lowercase();
            if lower.contains("://") || lower.starts_with("intent:") || navigates(&text[..i]) {
                out.push(literal.to_owned());
            }
        }
        i = j + 1;
    }
    out
}

/// Steht vor der Zeichenkette eine Navigation (`location =`, `.href =`, `location.replace(`, `location.assign(`,
/// `window.open(`)?
fn navigates(before: &str) -> bool {
    let b = before.trim_end();
    if let Some(lhs) = b.strip_suffix('=') {
        let lhs = lhs.trim_end();
        return !lhs.ends_with(['=', '!', '<', '>']) && (lhs.ends_with("location") || lhs.ends_with(".href"));
    }
    if let Some(callee) = b.strip_suffix('(') {
        let c = callee.trim_end();
        return c.ends_with("location.replace") || c.ends_with("location.assign") || c.ends_with("window.open");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_literals_for_link_diagnostics() {
        let js = r#"var a = "x"; if (y == "/no") {} window.location.href = "/mobile/next.aspx?sid=1";
            location.replace('idaustria://sign?t=GEHEIM'); var u = `intent://open#Intent;end`; var s = 'unterminated
            window.open("https://www.a-trust.at/app"); var esc = "a\"b://c";"#;
        let found = script_urls(js);
        assert_eq!(
            found,
            [
                "/mobile/next.aspx?sid=1",
                "idaustria://sign?t=GEHEIM",
                "intent://open#Intent;end",
                "https://www.a-trust.at/app",
                "a\\\"b://c"
            ]
        );
        assert!(navigates("x.location ="));
        assert!(!navigates("if (x.location =="));
        assert!(!navigates("var a ="));
        // Mehrbyte-Zeichen und Escapes vor dem Ende
        assert_eq!(script_urls("\"ä\\€://x\""), ["ä\\€://x"]);
        assert!(script_urls("\"offen").is_empty());
    }

    fn el_text(html: &str) -> String {
        let doc = Html::parse_document(html);
        let sel = Selector::parse("#x").unwrap();
        own_text(doc.select(&sel).next().unwrap())
    }

    #[test]
    fn reference_value_without_spaces() {
        let html = "<span id=x> Ab 7x\u{a0}3k </span>";
        let doc = Html::parse_fragment(html);
        let el = doc.select(&Selector::parse("#x").unwrap()).next().unwrap();
        assert_eq!(reference_text(el), "Ab7x3k");
    }

    #[test]
    fn own_text_like_jsoup() {
        assert_eq!(el_text("<p id=x>  Hallo   <b>fett</b>  Welt\n\t! </p>"), "Hallo Welt !");
        assert_eq!(el_text("<p id=x>a<br>b<br/> c</p>"), "a b c");
        assert_eq!(el_text("<p id=x>a&nbsp;&nbsp;b&#8203;c&shy;d</p>"), "a bcd");
        assert_eq!(el_text("<span id=x></span>"), "");
        assert_eq!(el_text("<pre id=x>  a  b </pre>"), "a  b");
        assert_eq!(el_text("<span id=x><!-- k -->X<i>y</i>Z</span>"), "XZ");
    }

    #[test]
    fn page_block_debug_is_redacted() {
        let b = PageBlock::Error {
            message: "Benutzer max.mustermann gesperrt".into(),
            back: None,
            cancel: None,
            recoverable: true,
            requires_response: false,
        };
        assert!(!format!("{b:?}").contains("mustermann"));
        let f = PageBlock::Fido2 {
            options_json: r#"{"challenge":"GEHEIM"}"#.into(),
            result_field: "r".into(),
            submit: None,
        };
        assert!(!format!("{f:?}").contains("GEHEIM"));
        assert!(format!("{f:?}").starts_with("Fido2"));
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("/error%31.aspx"), "/error1.aspx");
        assert_eq!(percent_decode("/a%"), "/a%");
        assert_eq!(percent_decode("/a%2"), "/a%2");
        assert_eq!(percent_decode("/a%zz"), "/a%zz");
    }

    #[test]
    fn set_field_semantics() {
        let mut f = vec![("a".to_owned(), "1".to_owned()), ("b".to_owned(), "2".to_owned())];
        set_field(&mut f, "a", "neu".into());
        set_field(&mut f, "c", "3".into());
        assert_eq!(f, [("a".into(), "neu".into()), ("b".into(), "2".into()), ("c".into(), "3".into())]);
    }
}
