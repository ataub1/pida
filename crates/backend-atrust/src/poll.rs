//! Long-Poll für QR- und App-Bestätigung (docs/analyse/02 §3.6).

use std::time::Duration;

use pdfover_backend_api::CancelToken;
use pdfover_net::{HttpClient, Url};

/// Poll-Info aus `qrpoll("URL", "TOKEN");` im ersten `<script>` in `#jsLongPoll`.
/// `Debug` schwärzt das Token (ADR 0001 §4 Regel 9).
#[derive(Clone, PartialEq, Eq)]
pub struct PollInfo {
    /// Poll-URL (relativ zur Dokument-Basis aufgelöst)
    pub url: Url,
    /// Bearer-Token
    pub token: String,
}

impl std::fmt::Debug for PollInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PollInfo").field("url", &self.url.as_str()).field("token", &"***").finish()
    }
}

/// Extrahiert die Poll-Info aus dem Skripttext.
pub fn parse_poll_script(script: &str, base: &Url) -> Option<PollInfo> {
    todo!("Paket D")
}

/// Antwort eines Polls (`getBoolean` in der Reihenfolge `Fin`, `Wait`, `Error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PollStatus {
    /// `Fin == true` → Seite neu laden (R7), dann 500 ms Pause
    Finished,
    /// `Wait == true` → sofort weiter
    Wait,
    /// `Error == true` → ebenfalls neu laden (A-Trust zeigt den Fehler)
    Error,
    /// weder noch → Schleife beenden, UI wartet weiter auf die Nutzerin
    Unknown,
    /// JSON-Fehler/fehlender Schlüssel → 10 s warten, dann neu laden
    Malformed,
}

/// Wertet die JSON-Antwort aus.
pub fn parse_poll_response(body: &[u8]) -> PollStatus {
    todo!("Paket D")
}

/// Zeitgrenzen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollConfig {
    /// Gesamtzeit („a-trust timeout is 5 minutes“)
    pub total_timeout: Duration,
    /// Pause nach Fehler/Exception
    pub error_backoff: Duration,
    /// Pause nach „fertig“
    pub after_finish: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            total_timeout: Duration::from_secs(300),
            error_backoff: Duration::from_secs(10),
            after_finish: Duration::from_millis(500),
        }
    }
}

/// Ergebnis der Poll-Schleife.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PollOutcome {
    /// Seite neu laden (fertig, Fehler, Timeout)
    Reload,
    /// Poll beendet ohne Signal – weiter auf die Nutzerin warten
    Stopped,
    /// abgebrochen
    Cancelled,
}

/// Führt die Poll-Schleife aus (eigener Client **ohne** Cookies, `Authorization: Bearer`).
pub async fn run(client: &HttpClient, info: &PollInfo, config: PollConfig, cancel: &CancelToken) -> PollOutcome {
    todo!("Paket D")
}
