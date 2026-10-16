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
    /// `/error*.aspx`, `/SessionClosed.aspx`.
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

    fn error(&self) -> Result<Option<PageBlock>, PageError> {
        let set = self.c.set;
        let path = self.base.path();
        let last = percent_decode(&path[path.rfind('/').unwrap_or(0)..]);
        let is_error =
            (last.starts_with(set.error_prefix.as_str()) && last.ends_with(".aspx")) || set.error_pages.contains(&last);
        if !is_error {
            return Ok(None);
        }
        let Some(label1) = self.first(&set.ids.label1)? else { return Ok(None) };
        let mut message = java_trim(&own_text(label1)).to_owned();
        if let Some(detail) = self.first(&set.ids.label_detail)? {
            message.push('\n');
            message.push_str(java_trim(&own_text(detail)));
        }
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

/// Diagnosecode ohne Personendaten (Pfad ohne Abfrage, Formularzahl, gefundene Element-IDs).
pub fn diagnostic(base_url: &Url, html: &str) -> String {
    let doc = Html::parse_document(html);
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

#[cfg(test)]
mod tests {
    use super::*;

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
