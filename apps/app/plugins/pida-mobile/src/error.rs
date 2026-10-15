//! Fehler des Mobile-Plugins – nur Codes, nie Pfade, Schlüssel oder Werte (ADR 0001 Regel 9).

/// Fehler einer nativen Methode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Plattform ohne native Seite (Desktop).
    #[error("auf dieser Plattform nicht unterstützt")]
    Unsupported,
    /// Die native Seite ist noch eine leere Hülle (Code `not-implemented`; M-iOS/M-Android).
    #[error("native Seite noch nicht umgesetzt")]
    NotImplemented,
    /// Die Nutzerin hat abgebrochen (Code `cancelled`).
    #[error("abgebrochen")]
    Cancelled,
    /// Keine Antwort binnen der Frist (Shell).
    #[error("Zeitüberschreitung")]
    Timeout,
    /// Sonstiger Fehler der nativen Seite (kurzer Code).
    #[error("Fehler der nativen Seite: {0}")]
    Native(String),
}

impl Error {
    /// Stabile Meldungs-ID (i18n, Logs).
    pub fn code(&self) -> &'static str {
        match self {
            Error::Unsupported => "ng-mobile-unsupported",
            Error::NotImplemented => "ng-mobile-not-implemented",
            Error::Cancelled => "ng-mobile-cancelled",
            Error::Timeout => "ng-mobile-timeout",
            Error::Native(_) => "ng-mobile-native",
        }
    }

    /// Aus dem Code einer Ablehnung (`invoke.reject(message, code)`); die Nachricht wird verworfen, weil
    /// sie Pfade enthalten kann.
    pub fn from_native_code(code: Option<&str>) -> Error {
        match code {
            Some("not-implemented") => Error::NotImplemented,
            Some("cancelled") => Error::Cancelled,
            Some(c)
                if c.len() <= 64 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')) =>
            {
                Error::Native(c.to_owned())
            }
            _ => Error::Native("native".into()),
        }
    }
}

/// Ergebnis-Alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(mobile)]
impl From<tauri::plugin::mobile::PluginInvokeError> for Error {
    fn from(e: tauri::plugin::mobile::PluginInvokeError) -> Self {
        use tauri::plugin::mobile::PluginInvokeError;
        match e {
            PluginInvokeError::InvokeRejected(r) => Error::from_native_code(r.code.as_deref()),
            PluginInvokeError::UnreachableWebview => Error::Native("webview-unreachable".into()),
            PluginInvokeError::CannotDeserializeResponse(_) => Error::Native("bad-response".into()),
            PluginInvokeError::CannotSerializePayload(_) => Error::Native("bad-payload".into()),
            #[allow(unreachable_patterns)]
            _ => Error::Native("native".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_codes() {
        assert_eq!(Error::from_native_code(Some("not-implemented")), Error::NotImplemented);
        assert_eq!(Error::from_native_code(Some("cancelled")), Error::Cancelled);
        assert_eq!(Error::from_native_code(Some("keychain-locked")), Error::Native("keychain-locked".into()));
        // Pfade bzw. Freitext werden nicht übernommen
        assert_eq!(Error::from_native_code(Some("/var/mobile/x.p12 fehlt")), Error::Native("native".into()));
        assert_eq!(Error::from_native_code(None), Error::Native("native".into()));
    }
}
