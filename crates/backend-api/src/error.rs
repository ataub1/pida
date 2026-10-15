//! Fehlertaxonomie der Backends (docs/analyse/02 §2.6, §3.11; Plan §7.4 Exit-Codes).

/// Fehler eines Backends.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackendError {
    /// Abbruch durch die Nutzerin (auch SL 6000/6001) → stiller Rücksprung zur Backend-Auswahl,
    /// CLI-Exit 4.
    #[error("Vorgang abgebrochen")]
    Cancelled,
    /// Backend nicht verfügbar (lokale BKU nicht erreichbar, headless ohne Passwort …).
    #[error("Signiermittel nicht verfügbar: {0}")]
    Unavailable(String),
    /// Verbindung fehlgeschlagen (`error.FailedToConnect`).
    #[error("Verbindung fehlgeschlagen: {0}")]
    Network(String),
    /// HTTP 407 (`error.ProxyAuthRequired`).
    #[error("Proxy verlangt Anmeldung")]
    ProxyAuthRequired,
    /// HTTP 413 (`atrusterror.http_413`).
    #[error("Dokument überschreitet die Größengrenze des Signaturdienstes")]
    DocumentTooLarge,
    /// sonstiger HTTP-Status (`atrusterror.http_generic`; auch 303/307/308).
    #[error("HTTP {status} {reason}")]
    Http {
        /// Status
        status: u16,
        /// Reason-Phrase
        reason: String,
    },
    /// SL-`ErrorResponse` (außer 6000/6001), `error.SLError`.
    #[error("Vorgang vom Signaturserver abgebrochen ({code}): {info}")]
    SecurityLayer {
        /// ErrorCode
        code: u32,
        /// Info
        info: String,
    },
    /// Unbekannte A-Trust-Seite – Szenario A bricht mit Diagnosecode ab und rät nie (Plan §4.4).
    #[error("unbekannte Seite des Signaturdienstes ({diagnostic})")]
    UnknownPage {
        /// Diagnose ohne Personendaten (URL-Pfad, erkannte IDs)
        diagnostic: String,
    },
    /// Endlosschleifen-Schutz (50 Requests in 2 s).
    #[error("Endlosschleife erkannt")]
    LoopDetected,
    /// Protokollfehler (unerwartetes XML/HTML, fehlendes `Location` …).
    #[error("Protokollfehler: {0}")]
    Protocol(String),
    /// Falsches Passwort (Keystore); der Aufrufer fragt erneut.
    #[error("falsches Passwort")]
    WrongPassword,
    /// Keystore-Fehler (Datei fehlt, Alias fehlt, unlesbares PBE …).
    #[error("Keystore: {0}")]
    Keystore(String),
    /// Keystore-Fehler mit stabilem Untercode (additiv, Welle 2): `code` =
    /// `pida_backend_keystore::KeystoreError::code` (`keystore-different-key-password`,
    /// `keystore-unsupported-pbe`, …) für die Übersetzung, `message` nur für Diagnose (ohne
    /// Pfade, Regel 9). [`BackendError::code`] bleibt `backend-keystore`, den Untercode liefert
    /// [`BackendError::detail_code`].
    #[error("Keystore: {message}")]
    KeystoreDetail {
        /// stabiler Untercode (`keystore-…`)
        code: String,
        /// Diagnosetext
        message: String,
    },
    /// JKS/JCEKS erkannt: Konvertierungshinweis statt Reader (Plan §4.9).
    #[error("Keystore-Format {format} wird nicht unterstützt – bitte nach PKCS#12 konvertieren")]
    UnsupportedKeystoreFormat {
        /// erkanntes Format (`JKS`, `JCEKS`)
        format: String,
    },
    /// Signer-Zertifikat weicht vom erwarteten ab („Certificates missmatch!“).
    #[error("Zertifikat weicht vom erwarteten ab")]
    CertificateMismatch,
    /// FIDO2-Fehler (nicht verfügbar, Authenticator-Fehler).
    #[error("FIDO2: {0}")]
    Fido2(String),
    /// Zeitüberschreitung (Long-Poll 300 s, Netz).
    #[error("Zeitüberschreitung")]
    Timeout,
    /// Interner Fehler.
    #[error("interner Fehler: {0}")]
    Internal(String),
}

/// Wirkungsbereich eines Backend-Fehlers für Stapel und „Erneut versuchen“ (ADR 0004 §3.6, §3.8):
/// entscheidet, ob die Sitzung (samt Anmeldung) offen bleiben darf.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorScope {
    /// betrifft nur dieses Dokument: [`BackendError::DocumentTooLarge`] (nicht wiederholbar),
    /// [`BackendError::SecurityLayer`] (wiederholbar)
    Document,
    /// Verbindung gestört, Sitzung und Zugangsdaten intakt: [`BackendError::Network`],
    /// [`BackendError::Timeout`], [`BackendError::Http`]
    Transient,
    /// Sitzung unbrauchbar, neu öffnen: alle übrigen Fehler außer [`BackendError::Cancelled`]
    Session,
    /// [`BackendError::Cancelled`] – ob Abbruch in PIDA oder Ablehnung in der App, entscheidet der Kern
    Cancelled,
}

impl BackendError {
    /// Wirkungsbereich (ADR 0004 §3.8); vollständig über alle Varianten (kein Platzhalter-Arm, damit eine
    /// neue Variante bewusst zugeordnet wird).
    pub fn scope(&self) -> ErrorScope {
        match self {
            BackendError::Cancelled => ErrorScope::Cancelled,
            BackendError::DocumentTooLarge | BackendError::SecurityLayer { .. } => ErrorScope::Document,
            BackendError::Network(_) | BackendError::Timeout | BackendError::Http { .. } => ErrorScope::Transient,
            BackendError::Unavailable(_)
            | BackendError::ProxyAuthRequired
            | BackendError::UnknownPage { .. }
            | BackendError::LoopDetected
            | BackendError::Protocol(_)
            | BackendError::WrongPassword
            | BackendError::Keystore(_)
            | BackendError::KeystoreDetail { .. }
            | BackendError::UnsupportedKeystoreFormat { .. }
            | BackendError::CertificateMismatch
            | BackendError::Fido2(_)
            | BackendError::Internal(_) => ErrorScope::Session,
        }
    }

    /// `true` für Nutzerabbruch (inkl. SL 6000/6001, die Backends bereits auf `Cancelled` abbilden).
    pub fn is_user_cancel(&self) -> bool {
        matches!(self, BackendError::Cancelled)
    }

    /// Lohnt ein „Wiederholen“ (neue Vorbereitung, F-71)?
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            BackendError::Network(_)
                | BackendError::Timeout
                | BackendError::SecurityLayer { .. }
                | BackendError::Http { .. }
                | BackendError::WrongPassword
        )
    }

    /// Stabiler Fehlercode für i18n (`backend-…`) und Diagnose.
    pub fn code(&self) -> &'static str {
        match self {
            BackendError::Cancelled => "backend-cancelled",
            BackendError::Unavailable(_) => "backend-unavailable",
            BackendError::Network(_) => "backend-network",
            BackendError::ProxyAuthRequired => "backend-proxy-auth",
            BackendError::DocumentTooLarge => "backend-document-too-large",
            BackendError::Http { .. } => "backend-http",
            BackendError::SecurityLayer { .. } => "backend-security-layer",
            BackendError::UnknownPage { .. } => "backend-unknown-page",
            BackendError::LoopDetected => "backend-loop",
            BackendError::Protocol(_) => "backend-protocol",
            BackendError::WrongPassword => "backend-wrong-password",
            BackendError::Keystore(_) | BackendError::KeystoreDetail { .. } => "backend-keystore",
            BackendError::UnsupportedKeystoreFormat { .. } => "backend-keystore-format",
            BackendError::CertificateMismatch => "backend-certificate-mismatch",
            BackendError::Fido2(_) => "backend-fido2",
            BackendError::Timeout => "backend-timeout",
            BackendError::Internal(_) => "backend-internal",
        }
    }

    /// Feinerer, stabiler Untercode, falls vorhanden (additiv, Welle 2) – z. B.
    /// `keystore-different-key-password` bei [`BackendError::KeystoreDetail`]. Die UI übersetzt
    /// bevorzugt diesen Code und fällt sonst auf [`BackendError::code`] zurück.
    pub fn detail_code(&self) -> Option<&str> {
        match self {
            BackendError::KeystoreDetail { code, .. } => Some(code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keystore_detail_keeps_family_code() {
        let e = BackendError::KeystoreDetail {
            code: "keystore-different-key-password".into(),
            message: "abweichendes Schlüsselpasswort".into(),
        };
        assert_eq!(e.code(), "backend-keystore");
        assert_eq!(e.detail_code(), Some("keystore-different-key-password"));
        assert_eq!(e.to_string(), "Keystore: abweichendes Schlüsselpasswort");
        assert!(!e.is_retryable() && !e.is_user_cancel());
        assert_eq!(BackendError::Keystore("x".into()).detail_code(), None);
    }

    /// ADR 0004 §3.8: Tabelle Fehler → Wirkungsbereich, je Variante.
    #[test]
    fn scope_of_every_variant() {
        use ErrorScope::*;
        let s = |x: &str| x.to_owned();
        let table: Vec<(BackendError, ErrorScope)> = vec![
            (BackendError::Cancelled, Cancelled),
            (BackendError::Unavailable(s("x")), Session),
            (BackendError::Network(s("x")), Transient),
            (BackendError::ProxyAuthRequired, Session),
            (BackendError::DocumentTooLarge, Document),
            (BackendError::Http { status: 503, reason: s("x") }, Transient),
            (BackendError::SecurityLayer { code: 6002, info: s("x") }, Document),
            (BackendError::UnknownPage { diagnostic: s("x") }, Session),
            (BackendError::LoopDetected, Session),
            (BackendError::Protocol(s("x")), Session),
            (BackendError::WrongPassword, Session),
            (BackendError::Keystore(s("x")), Session),
            (BackendError::KeystoreDetail { code: s("keystore-x"), message: s("x") }, Session),
            (BackendError::UnsupportedKeystoreFormat { format: s("JKS") }, Session),
            (BackendError::CertificateMismatch, Session),
            (BackendError::Fido2(s("x")), Session),
            (BackendError::Timeout, Transient),
            (BackendError::Internal(s("x")), Session),
        ];
        // jede Variante genau einmal (über den stabilen Code; `Keystore`/`KeystoreDetail` teilen ihn)
        let codes: std::collections::BTreeSet<&str> = table.iter().map(|(e, _)| e.code()).collect();
        assert_eq!(codes.len(), table.len() - 1);
        for (e, scope) in &table {
            assert_eq!(e.scope(), *scope, "{e:?}");
        }
    }
}
