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

impl BackendError {
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
            BackendError::Keystore(_) => "backend-keystore",
            BackendError::UnsupportedKeystoreFormat { .. } => "backend-keystore-format",
            BackendError::CertificateMismatch => "backend-certificate-mismatch",
            BackendError::Fido2(_) => "backend-fido2",
            BackendError::Timeout => "backend-timeout",
            BackendError::Internal(_) => "backend-internal",
        }
    }
}
