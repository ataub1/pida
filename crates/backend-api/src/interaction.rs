//! `InteractionBroker` und Masken (Szenario A; docs/analyse/01 §4.6, docs/analyse/02 §3.5–§3.11).

use async_trait::async_trait;
use zeroize::Zeroizing;

/// Fehler einer Interaktion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum InteractionError {
    /// Nutzerin hat abgebrochen bzw. Fenster geschlossen.
    #[error("abgebrochen")]
    Cancelled,
    /// Diese Interaktion ist in dieser Shell nicht verfügbar (z. B. headless, Szenario B in v1).
    #[error("nicht unterstützt")]
    Unsupported,
}

/// Plattform-Fensterhandle (HWND, NSWindow*, …) für modale System-Dialoge (Windows Hello/webauthn.dll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub u64);

/// Validierungs- bzw. Fehlerhinweis in einer Maske (i18n-Keys aus PDF-Over).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FormProblem {
    /// `exception.PasswordTooShort` (< 5)
    PasswordTooShort,
    /// `exception.PasswordTooLong` (> 200)
    PasswordTooLong,
    /// `error.NoTan`
    NoTan,
    /// `error.TanTooLong` (> 6)
    TanTooLong,
    /// `error.EnteredReferenceValue` (TAN ist Präfix des Vergleichswerts)
    EnteredReferenceValue,
    /// `error.FailedToLoadQRCode`
    QrImageFailed,
    /// FIDO2 fehlgeschlagen (Button wieder aktiv)
    Fido2Failed(String),
    /// Erneute Login-Seite: Passwort wurde gelöscht und muss neu eingegeben werden (Plan §4.4, keine
    /// automatische Wiederholung – Schutz vor Kontosperre)
    LoginRejected,
    /// Text von A-Trust
    Atrust(String),
}

/// Gemeinsamer Kontext der A-Trust-Masken.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct FormContext {
    /// Link „Signaturdaten anzeigen“ (`ShowSigobj.aspx`), nur wenn A-Trust ihn liefert (F-68).
    pub signature_data_url: Option<String>,
    /// „SMS anfordern“ möglich (`#SmsButton`).
    pub can_switch_to_sms: bool,
    /// „FIDO2“ angeboten (`#FidoButton`).
    pub can_switch_to_fido2: bool,
    /// FIDO2 auf dieser Plattform verfügbar (Button sichtbar).
    pub fido2_visible: bool,
    /// Meldungszeile (Disclaimer `mobileBKU.aTrustDisclaimer` bzw. Fehler).
    pub problem: Option<FormProblem>,
}

/// QR-Bild (per HTTP von A-Trust geladen, R8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QrImage {
    /// Bildbytes (typisch PNG)
    pub bytes: Vec<u8>,
    /// MIME-Typ, falls bekannt
    pub mime: Option<String>,
}

/// Native Maske (Szenario A).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NativeForm {
    /// Anmeldung: Benutzername/Telefonnummer + Passwort (5–200), „Merken“.
    Login {
        /// Vorbelegung (gemerkt bzw. `-n`; in der UI nicht normalisiert)
        prefill_username: Option<String>,
        /// Zustand „Merken“ (`MOBILE_REMEMBER_PASSWORD`)
        remember: bool,
        /// Kontext
        ctx: FormContext,
    },
    /// SMS-TAN mit Vergleichswert.
    SmsTan {
        /// `#vergleichswert`
        reference_value: String,
        /// Kontext
        ctx: FormContext,
    },
    /// QR-Code mit Vergleichswert (Long-Poll im Hintergrund).
    QrCode {
        /// Vergleichswert
        reference_value: String,
        /// Bild (fehlt, wenn das Laden scheiterte)
        image: Option<QrImage>,
        /// Restzeit des Long-Polls in Sekunden beim Anzeigen (Countdown, Plan §4.4; Default 300 s
        /// aus `PollConfig`); `None` = unbekannt. Die UI zählt selbst herunter.
        timeout_secs: Option<u32>,
        /// Kontext
        ctx: FormContext,
    },
    /// Bestätigung in der App „ID Austria“ (Push/Biometrie, Long-Poll).
    WaitForApp {
        /// Vergleichswert
        reference_value: Option<String>,
        /// Restzeit des Long-Polls in Sekunden (Countdown, Plan §4.4); `None` = unbekannt.
        timeout_secs: Option<u32>,
        /// Kontext
        ctx: FormContext,
    },
    /// FIDO2/WebAuthn (startet automatisch beim Anzeigen).
    Fido2 {
        /// Vorgang läuft (`common.working`, Button deaktiviert)
        in_progress: bool,
        /// Kontext
        ctx: FormContext,
    },
}

/// Antwort auf eine Maske. Abbruch kommt als [`InteractionError::Cancelled`].
#[derive(Clone, PartialEq, Eq)]
pub enum FormAnswer {
    /// Anmeldedaten (Passwort nur im RAM).
    Login {
        /// Benutzername/Telefonnummer, unverändert
        username: String,
        /// Passwort
        password: Zeroizing<String>,
        /// „Merken“
        remember: bool,
    },
    /// TAN.
    Tan(Zeroizing<String>),
    /// Wechsel auf SMS-TAN.
    SwitchToSms,
    /// Wechsel auf FIDO2.
    SwitchToFido2,
    /// FIDO2 starten bzw. erneut versuchen.
    StartFido2,
    /// QR/App: Nutzerin meldet „fertig“ → Seite neu laden (R7).
    Continue,
}

impl std::fmt::Debug for FormAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormAnswer::Login { username, remember, .. } => f
                .debug_struct("Login")
                .field("username", username)
                .field("password", &"***")
                .field("remember", remember)
                .finish(),
            FormAnswer::Tan(_) => f.write_str("Tan(***)"),
            FormAnswer::SwitchToSms => f.write_str("SwitchToSms"),
            FormAnswer::SwitchToFido2 => f.write_str("SwitchToFido2"),
            FormAnswer::StartFido2 => f.write_str("StartFido2"),
            FormAnswer::Continue => f.write_str("Continue"),
        }
    }
}

/// Art der Passwortabfrage (Keystore, Proxy).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SecretKind {
    /// `keystore.KeystoreStorePassEntry`
    KeystorePassword,
    /// `keystore.KeystoreKeyPassEntry`
    KeyPassword,
    /// Proxy-Passwort
    ProxyPassword,
}

/// Passwortabfrage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecretPrompt {
    /// Art
    pub kind: SecretKind,
    /// Bezug (Keystore-Dateiname, Alias, Proxy-Host) – ohne Pfad zu Nutzerdaten in Logs
    pub subject: String,
    /// Vorheriger Versuch war falsch (`error.KeyStoreStorePass` …)
    pub retry: bool,
}

/// Art einer Meldung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NoticeKind {
    /// Info
    Info,
    /// Warnung
    Warning,
    /// Fehler
    Error,
}

/// Knöpfe einer Meldung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NoticeButtons {
    /// OK
    Ok,
    /// OK/Abbrechen
    OkCancel,
    /// Wiederholen/Abbrechen
    RetryCancel,
}

/// Inhalt einer Meldung (i18n über den Code; Texte von A-Trust roh in [`Notice::text`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NoticeCode {
    /// `ExpiresInfo.aspx` (`#Label2`) – Info, danach weiter
    AtrustInterstitial,
    /// `error*.aspx`/`SessionClosed.aspx` („a-trust.at meldet: …“)
    AtrustError,
    /// HTTP 407
    ProxyAuthRequired,
    /// HTTP 413
    DocumentTooLarge,
    /// sonstiger HTTP-Status
    Http {
        /// Status
        status: u16,
    },
    /// Verbindungsfehler
    ConnectionFailed,
    /// Lokale BKU nicht erreichbar (`error.LocalBKU`)
    LocalBkuNotReachable,
    /// Keystore-Datei fehlt (`error.KeyStoreFileNotExist`)
    KeystoreFileMissing,
    /// Alias fehlt (`error.KeyStoreAliasExist`)
    KeystoreAliasMissing,
    /// sonstiger Keystore-Fehler (`error.KeyStore`)
    KeystoreError,
    /// JKS/JCEKS: Konvertierungsassistent
    KeystoreConversionNeeded,
}

/// Meldung.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Notice {
    /// Art
    pub kind: NoticeKind,
    /// Code
    pub code: NoticeCode,
    /// Freitext (A-Trust-Meldung, HTTP-Reason)
    pub text: Option<String>,
    /// Knöpfe
    pub buttons: NoticeButtons,
}

/// Antwort auf eine Meldung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NoticeAnswer {
    /// OK
    Ok,
    /// Wiederholen
    Retry,
    /// Abbrechen
    Cancel,
}

/// Fortschritt (Warte-Screen, Live-Region).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Progress {
    /// Verbindung wird aufgebaut
    Connecting,
    /// Zertifikat wird gelesen
    ReadingCertificate,
    /// Warten auf den Signaturdienst
    WaitingForServer,
    /// Signatur wird erstellt
    Signing,
    /// Dokument `index` von `total` (Batch)
    Document {
        /// 1-basiert
        index: usize,
        /// gesamt
        total: usize,
    },
    /// fertig
    Done,
}

/// Ereignisse an die Anwendung (Persistenz liegt beim Aufrufer, nicht im Backend).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BackendEvent {
    /// Anmeldung erfolgreich (erste Seite ohne Error-/Login-Block) – bei `remember` speichert die
    /// Anwendung den Benutzernamen; das Passwort **nie**.
    LoginSucceeded {
        /// Benutzername
        username: String,
        /// „Merken“
        remember: bool,
    },
    /// „Merken“ abgewählt: Benutzername und Passwort vergessen.
    ForgetCredentials,
    /// HTTP-`Date` der Gegenstelle (Plausibilisierung von `/M`, Warnung ab 5 min).
    ServerTime(#[serde(with = "time::serde::rfc3339")] time::OffsetDateTime),
}

/// Szenario B (reserviert, nicht in v1): Start-Request für das Browser-Control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebRequest {
    /// SL-Endpunkt
    pub endpoint: String,
    /// Formularfelder (`XMLRequest`, `telephonenumber`, `lang`, …)
    pub form_fields: Vec<(String, String)>,
    /// Navigations-Allowlist
    pub allow_hosts: Vec<String>,
}

/// Szenario B (reserviert): abgegriffene SL-Antwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebOutcome {
    /// Inhalt
    pub body: Vec<u8>,
    /// MIME-Typ
    pub content_type: String,
}

/// Szenario B-Relay/C (reserviert): Systembrowser-Start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRequest {
    /// Start-URL
    pub start_url: String,
    /// Präfix der Rückruf-URL (Loopback/App Link)
    pub callback_prefix: String,
}

/// Szenario B-Relay/C (reserviert): Rückruf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackData {
    /// vollständige Rückruf-URL
    pub url: String,
}

/// Brücke vom Backend zur Oberfläche (Tauri-Shell, CLI, Tests).
///
/// Implementiert von `pdfover-core-api` (projiziert in `ViewState`) bzw. der CLI.
#[async_trait]
pub trait InteractionBroker: Send + Sync {
    /// Zeigt eine native Maske und wartet auf die Antwort.
    async fn native_form(&self, form: NativeForm) -> Result<FormAnswer, InteractionError>;

    /// Fragt ein Passwort ab (Keystore, Proxy).
    async fn secret(&self, prompt: SecretPrompt) -> Result<Zeroizing<String>, InteractionError>;

    /// Zeigt eine Meldung.
    async fn notice(&self, notice: Notice) -> Result<NoticeAnswer, InteractionError>;

    /// Meldet Fortschritt (nicht blockierend).
    fn progress(&self, progress: Progress);

    /// Meldet ein Ereignis (nicht blockierend).
    fn event(&self, event: BackendEvent);

    /// Öffnet eine URL im Systembrowser (Signaturdaten, Hilfe-Links). Nur `https://`.
    fn open_external(&self, url: &str) -> Result<(), InteractionError>;

    /// Elternfenster für modale Systemdialoge (FIDO2 unter Windows).
    fn parent_window(&self) -> Option<WindowHandle> {
        None
    }

    /// Szenario B (reserviert, nicht in v1).
    async fn embedded_web(&self, request: WebRequest) -> Result<WebOutcome, InteractionError> {
        let _ = request;
        Err(InteractionError::Unsupported)
    }

    /// Szenario B-Relay/C (reserviert, nicht in v1).
    async fn system_browser(&self, request: BrowserRequest) -> Result<CallbackData, InteractionError> {
        let _ = request;
        Err(InteractionError::Unsupported)
    }
}
