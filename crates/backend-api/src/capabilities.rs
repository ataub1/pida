//! Capability-Modell (Plan §3.3).

use pida_cades::DigestAlg;

/// Kennung eines Backends.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendId {
    /// ID Austria über A-Trust, **Szenario A** (nativer Ablauf, Standard in v1).
    AtrustNative,
    /// ID Austria über Browser-Control/Relay, Szenario B – reserviert, nicht in v1.
    AtrustBrowser,
    /// CSC-Hash-Signing, Szenario C – reserviert, nicht in v1.
    Csc,
    /// Lokale BKU (127.0.0.1:3495), nur Desktop.
    LocalBku,
    /// PKCS#12-Keystore.
    Keystore,
    /// EUDI-Wallet, Szenario D – reserviert, nicht in v1.
    Wallet,
}

impl BackendId {
    /// Stabiler Name (Konfiguration, CLI, Telemetriefreie Diagnose).
    pub fn as_str(self) -> &'static str {
        match self {
            BackendId::AtrustNative => "atrust_native",
            BackendId::AtrustBrowser => "atrust_browser",
            BackendId::Csc => "csc",
            BackendId::LocalBku => "local_bku",
            BackendId::Keystore => "keystore",
            BackendId::Wallet => "wallet",
        }
    }

    /// Aus PDF-Over `BKUs` (`MOBILE`, `LOCAL`, `KS`; `NONE`/ungültig → `None`, case-sensitive).
    pub fn from_legacy_bku(value: &str) -> Option<BackendId> {
        match value {
            "MOBILE" => Some(BackendId::AtrustNative),
            "LOCAL" => Some(BackendId::LocalBku),
            "KS" => Some(BackendId::Keystore),
            _ => None,
        }
    }
}

/// Was das Backend signiert.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SignInput {
    /// Modell B: ganzes vorbereitetes PDF mit geschwärztem `/Contents` + ExcludedByteRange.
    Document,
    /// Modell A: Hash über die SignedAttributes.
    Digest {
        /// unterstützte Hashverfahren
        #[cfg_attr(feature = "ts", ts(as = "Vec<crate::ts_mirror::DigestAlg>"))]
        algs: Vec<DigestAlg>,
    },
}

/// Was das Backend liefert.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SignOutput {
    /// fertiges CMS (SL `CreateCMSSignatureResponse`)
    Cms,
    /// Rohsignatur (Keystore, CSC)
    RawSignature,
}

/// Wann ein Zertifikat vorab benötigt wird.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CertNeed {
    /// immer (Modell A, sichtbarer Block)
    Required,
    /// nur für den sichtbaren Block (unsichtbar: ein einziger SL-Durchlauf)
    OnlyForVisibleBlock,
    /// nie
    No,
}

/// Art der Interaktion.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Interaction {
    /// keine
    None,
    /// Passwortabfrage (Keystore)
    Secret,
    /// Native Masken (Szenario A)
    NativeForms,
    /// Browser-Control (Szenario B, reserviert)
    EmbeddedWeb,
    /// Systembrowser (B-Relay/CSC, reserviert)
    SystemBrowser,
    /// Eigene Oberfläche der BKU
    External,
    /// Fremde App (Wallet, reserviert)
    ExternalApp,
}

/// Benötigter Server.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ServerNeed {
    /// keiner
    None,
    /// `sl-relay` (reserviert)
    DataUrlRelay,
    /// `csc-bff` (reserviert)
    TokenProxy,
}

/// Abgriff der SL-Antwort in Szenario B (reserviert, nicht in v1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Capture {
    /// im WebView
    InWebView,
    /// über DataURL-Relay
    DataUrlRelay,
}

/// Qualifikation der Signatur (Anzeige).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Qualification {
    /// qualifiziert
    Qes,
    /// fortgeschritten
    AdES,
    /// unbekannt
    Unknown,
}

/// Faktoren (nur Anzeige).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct FactorSet {
    /// ID-Austria-App
    pub app: bool,
    /// SMS-TAN
    pub sms: bool,
    /// QR-Code
    pub qr: bool,
    /// FIDO2
    pub fido2: bool,
    /// Signaturkarte
    pub card: bool,
}

/// Fähigkeiten eines Backends in einer Umgebung.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Capabilities {
    /// Eingabe
    pub input: SignInput,
    /// Ausgabe
    pub output: SignOutput,
    /// Zertifikat vorab
    pub certificate: CertNeed,
    /// Interaktion
    pub interaction: Interaction,
    /// Server
    pub server: ServerNeed,
    /// Höchstzahl Dokumente je Freigabe (1 = je Dokument eine Freigabe)
    pub max_batch: u16,
    /// Eine Anmeldung für den ganzen Batch (Szenario A wie PDF-Over)
    pub login_once_per_batch: bool,
    /// Dokument verlässt das Gerät → Pflicht-Datenschutzhinweis vor dem ersten Upload
    pub document_leaves_device: bool,
    /// Qualifikation
    pub qualification: Qualification,
    /// Faktoren
    pub factors: FactorSet,
    /// Empfohlene CMS-Reserve in Bytes (Modell B: 16 KiB, Plan §6.2)
    pub cms_reserve: Option<usize>,
}

/// Plattform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Platform {
    /// Windows
    Windows,
    /// macOS
    MacOs,
    /// Linux
    Linux,
    /// iOS
    Ios,
    /// Android
    Android,
}

impl Platform {
    /// Plattform des laufenden Prozesses.
    pub fn current() -> Platform {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "ios") {
            Platform::Ios
        } else if cfg!(target_os = "android") {
            Platform::Android
        } else {
            Platform::Linux
        }
    }

    /// Desktop?
    pub fn is_desktop(self) -> bool {
        matches!(self, Platform::Windows | Platform::MacOs | Platform::Linux)
    }
}

/// Umgebung, in der Capabilities ermittelt werden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// Plattform
    pub platform: Platform,
    /// ohne UI (CLI `--headless`)
    pub headless: bool,
    /// FIDO2-Authenticator nutzbar (`Fido2Provider::is_available`)
    pub fido2_available: bool,
    /// lokale BKU per TCP-Probe erreichbar (F-51)
    pub localbku_reachable: bool,
    /// Keystore aktiviert und Datei vorhanden (F-52)
    pub keystore_configured: bool,
    /// Richtlinie: erlaubte Backends (`None` = alle)
    pub allowed_by_policy: Option<Vec<BackendId>>,
}

impl Environment {
    /// Umgebung des laufenden Prozesses mit konservativen Defaults.
    pub fn current() -> Environment {
        Environment {
            platform: Platform::current(),
            headless: false,
            fido2_available: false,
            localbku_reachable: false,
            keystore_configured: false,
            allowed_by_policy: None,
        }
    }

    /// Richtlinie erlaubt das Backend.
    pub fn allows(&self, id: BackendId) -> bool {
        self.allowed_by_policy.as_ref().is_none_or(|ids| ids.contains(&id))
    }
}
