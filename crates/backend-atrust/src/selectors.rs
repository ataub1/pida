//! Versionierte Selektor-Datei (Plan §4.4): Element-Selektoren und URL-Muster aus docs/analyse/02 §8,
//! damit Seitenänderungen ohne Code-Release behoben werden können (signiert, mit dem Update-Manifest;
//! die Signaturprüfung liegt beim Update-Mechanismus der Integration).
//!
//! Die Standarddatei liegt unter `crates/backend-atrust/selectors/atrust-v1.json`.

/// Eingebettete Standarddatei.
pub const BUILTIN_JSON: &str = include_str!("../selectors/atrust-v1.json");

fn default_poll_function() -> String {
    "qrpoll".into()
}

fn default_trusted_hosts() -> Vec<String> {
    vec!["a-trust.at".into(), "handy-signatur.at".into()]
}

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
    /// Element-Selektoren (CSS, in der Regel `#id`: `#handynummer`, `#signaturpasswort`, …)
    pub ids: ElementIds,
    /// Name der Long-Poll-Funktion im Skript (`qrpoll("URL", "TOKEN");`).
    #[serde(default = "default_poll_function")]
    pub poll_function: String,
    /// Selektor einer Fehlermeldung **auf der Login-Seite** (leer = nicht auswerten wie PDF-Over,
    /// `ATrustParser.java:152`). Ist er gesetzt und liefert Text, wird nie automatisch angemeldet.
    #[serde(default)]
    pub login_error: String,
    /// Domains (mit Subdomains, nur `https://`), an die Zugangsdaten, TAN und FIDO2-Assertion
    /// gesendet werden dürfen – zusätzlich zur Origin des konfigurierten Endpunkts. Ältere Dateien
    /// ohne das Feld: `a-trust.at`, `handy-signatur.at`.
    #[serde(default = "default_trusted_hosts")]
    pub trusted_hosts: Vec<String>,
}

/// Element-Selektoren. Alle Werte sind CSS-Selektoren außer `signature_data_href` (Teil des
/// `href`, kombiniert zu `{link_list} a[href*="{signature_data_href}"]`).
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

/// Fehler einer Selektor-Datei.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectorError {
    /// JSON nicht lesbar.
    #[error("Selektor-Datei nicht lesbar: {0}")]
    Json(String),
    /// Ein Selektor ist leer oder kein gültiger CSS-Selektor.
    #[error("Selektor {field} ungültig: {reason}")]
    Invalid {
        /// Feldname
        field: &'static str,
        /// Grund
        reason: String,
    },
}

impl ElementIds {
    /// Alle CSS-Selektoren mit Feldnamen (ohne `signature_data_href`).
    pub fn css_fields(&self) -> [(&'static str, &str); 25] {
        [
            ("username", &self.username),
            ("password", &self.password),
            ("button_identification", &self.button_identification),
            ("button_cancel", &self.button_cancel),
            ("button_back", &self.button_back),
            ("button_next", &self.button_next),
            ("next_btn", &self.next_btn),
            ("input_tan", &self.input_tan),
            ("sign_button", &self.sign_button),
            ("reference_value", &self.reference_value),
            ("qr_image", &self.qr_image),
            ("long_poll", &self.long_poll),
            ("smartphone_animation", &self.smartphone_animation),
            ("biometric_image", &self.biometric_image),
            ("sign_type", &self.sign_type),
            ("fido_block", &self.fido_block),
            ("credential_options", &self.credential_options),
            ("credential_result", &self.credential_result),
            ("fido_continue", &self.fido_continue),
            ("sms_button", &self.sms_button),
            ("fido_button", &self.fido_button),
            ("link_list", &self.link_list),
            ("label1", &self.label1),
            ("label_detail", &self.label_detail),
            ("label2", &self.label2),
        ]
    }
}

impl SelectorSet {
    /// Eingebettete Standarddatei (Stand PDF-Over 4.4.x).
    pub fn builtin() -> SelectorSet {
        let set = SelectorSet::from_json(BUILTIN_JSON).expect("eingebettete Selektor-Datei ist gültig");
        debug_assert!(set.validate().is_ok());
        set
    }

    /// Aus JSON (signiert geladene Aktualisierung).
    pub fn from_json(json: &str) -> Result<SelectorSet, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Aus JSON mit Prüfung aller Selektoren ([`SelectorSet::validate`]).
    pub fn from_json_validated(json: &str) -> Result<SelectorSet, SelectorError> {
        let set = Self::from_json(json).map_err(|e| SelectorError::Json(e.to_string()))?;
        set.validate()?;
        Ok(set)
    }

    /// Prüft, dass alle Selektoren gültiges CSS sind und die Pflichtwerte nicht leer.
    pub fn validate(&self) -> Result<(), SelectorError> {
        for (field, css) in self.ids.css_fields() {
            check_css(field, css)?;
        }
        check_css("signature_data_link", &self.signature_data_link_css())?;
        if !self.login_error.trim().is_empty() {
            check_css("login_error", &self.login_error)?;
        }
        if self.trusted_hosts.is_empty() {
            return Err(SelectorError::Invalid { field: "trusted_hosts", reason: "leer".into() });
        }
        for host in &self.trusted_hosts {
            let valid_label = |l: &str| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            if host.split('.').count() < 2 || !host.split('.').all(valid_label) {
                return Err(SelectorError::Invalid {
                    field: "trusted_hosts",
                    reason: format!("kein Domainname: {host}"),
                });
            }
        }
        let non_empty = [
            ("error_prefix", self.error_prefix.as_str()),
            ("poll_function", self.poll_function.as_str()),
            ("signature_data_href", self.ids.signature_data_href.as_str()),
        ];
        for (field, value) in non_empty {
            if value.trim().is_empty() {
                return Err(SelectorError::Invalid { field, reason: "leer".into() });
            }
        }
        Ok(())
    }

    /// `true`, wenn `host` eine der [`SelectorSet::trusted_hosts`] oder eine Subdomain davon ist.
    pub fn is_trusted_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        self.trusted_hosts.iter().any(|t| {
            let t = t.to_ascii_lowercase();
            host == t || host.strip_suffix(t.as_str()).is_some_and(|prefix| prefix.ends_with('.'))
        })
    }

    /// `#LinkList a[href*="ShowSigobj.aspx"]` (`ATrustParser.java:326`).
    pub fn signature_data_link_css(&self) -> String {
        format!("{} a[href*=\"{}\"]", self.ids.link_list, self.ids.signature_data_href.replace('"', "\\\""))
    }
}

fn check_css(field: &'static str, css: &str) -> Result<(), SelectorError> {
    if css.trim().is_empty() {
        return Err(SelectorError::Invalid { field, reason: "leer".into() });
    }
    scraper::Selector::parse(css).map(|_| ()).map_err(|e| SelectorError::Invalid { field, reason: e.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_matches_the_analysis() {
        let s = SelectorSet::builtin();
        assert_eq!(s.version, 1);
        assert!(s.validate().is_ok());
        assert_eq!(s.autoskip_urls, ["/tanAppInfo.aspx", "/UsernameInfo.aspx"]);
        assert_eq!(s.interstitial_urls, ["/ExpiresInfo.aspx"]);
        assert_eq!(s.error_prefix, "/error");
        assert_eq!(s.error_pages, ["/SessionClosed.aspx"]);
        assert_eq!(s.poll_function, "qrpoll");
        assert_eq!(s.login_error, "");
        assert_eq!(s.trusted_hosts, ["a-trust.at", "handy-signatur.at"]);
        assert!(s.is_trusted_host("service.a-trust.at") && s.is_trusted_host("A-TRUST.at"));
        assert!(s.is_trusted_host("www.handy-signatur.at"));
        assert!(!s.is_trusted_host("evil-a-trust.at") && !s.is_trusted_host("a-trust.at.evil.example"));
        assert_eq!(s.ids.username, "#handynummer");
        assert_eq!(s.ids.sign_type, ".ida-signtype #SignType");
        assert_eq!(s.signature_data_link_css(), "#LinkList a[href*=\"ShowSigobj.aspx\"]");
        // alle 25 IDs aus docs/analyse/02 §8 kommen vor
        let all: Vec<&str> = s.ids.css_fields().iter().map(|(_, v)| *v).collect();
        for id in [
            "handynummer",
            "signaturpasswort",
            "Button_Identification",
            "Button_Cancel",
            "Button_Back",
            "Button_Next",
            "NextBtn",
            "input_tan",
            "SignButton",
            "vergleichswert",
            "qrimage",
            "jsLongPoll",
            "smartphoneAnimation",
            "biometricimage",
            "SignType",
            "fidoBlock",
            "credentialOptions",
            "credentialResult",
            "FidoContinue",
            "SmsButton",
            "FidoButton",
            "LinkList",
            "Label1",
            "LabelDetail",
            "Label2",
        ] {
            assert!(all.iter().any(|v| v.contains(id)), "{id}");
        }
    }

    #[test]
    fn json_roundtrip_and_defaults() {
        let s = SelectorSet::builtin();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(SelectorSet::from_json(&json).unwrap(), s);
        // ältere Dateien ohne die optionalen Felder
        let mut v: serde_json::Value = serde_json::from_str(BUILTIN_JSON).unwrap();
        v.as_object_mut().unwrap().remove("poll_function");
        v.as_object_mut().unwrap().remove("login_error");
        v.as_object_mut().unwrap().remove("trusted_hosts");
        let old = SelectorSet::from_json(&v.to_string()).unwrap();
        assert_eq!(old.poll_function, "qrpoll");
        assert_eq!(old.login_error, "");
        assert_eq!(old.trusted_hosts, ["a-trust.at", "handy-signatur.at"]);
    }

    #[test]
    fn invalid_selectors_are_rejected() {
        let mut s = SelectorSet::builtin();
        s.ids.qr_image = "#[".into();
        assert!(matches!(s.validate(), Err(SelectorError::Invalid { field: "qr_image", .. })));
        let mut s = SelectorSet::builtin();
        s.ids.label1 = " ".into();
        assert!(s.validate().is_err());
        let mut s = SelectorSet::builtin();
        s.login_error = ".err[".into();
        assert!(s.validate().is_err());
        for bad in [vec![], vec!["at".to_owned()], vec!["https://a-trust.at".to_owned()], vec!["a..at".to_owned()]] {
            let mut s = SelectorSet::builtin();
            s.trusted_hosts = bad;
            assert!(matches!(s.validate(), Err(SelectorError::Invalid { field: "trusted_hosts", .. })));
        }
        assert!(matches!(SelectorSet::from_json_validated("{"), Err(SelectorError::Json(_))));
        assert!(SelectorSet::from_json_validated(BUILTIN_JSON).is_ok());
    }
}
