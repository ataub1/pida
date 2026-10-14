//! `pdfover-sl12` – Security Layer 1.2 (gemeinsam für A-Trust Szenario A und lokale BKU).
//!
//! Spezifikation: docs/analyse/02 §2 (Strukturen, Fehler, Transportvarianten), §8 (Konstanten),
//! docs/analyse/03 §6.2. Reiner XML/Binding-Code ohne Netz – [`HttpBody`] beschreibt den
//! Request-Körper, `pdfover-net` überträgt ihn.
//!
//! Ownership: Paket **D**.

/// SL-1.2-Namespace.
pub const SL_NAMESPACE: &str = "http://www.buergerkarte.at/namespaces/securitylayer/1.2#";
/// Infobox für das Zertifikat.
pub const INFOBOX_CERTIFICATES: &str = "Certificates";
/// Schlüssel/Keybox.
pub const SECURE_SIGNATURE_KEYPAIR: &str = "SecureSignatureKeypair";
/// MIME-Typ des Dokuments.
pub const MIME_PDF: &str = "application/pdf";
/// Upload-Referenz.
pub const UPLOAD_REFERENCE: &str = "formdata:fileupload";
/// Formularfeld für das XML.
pub const FIELD_XML_REQUEST: &str = "XMLRequest";
/// Formularfeld für die Datei.
pub const FIELD_FILE_UPLOAD: &str = "fileupload";
/// Dateiname des Uploads.
pub const UPLOAD_FILENAME: &str = "sign.pdf";
/// SL-Fehlercodes, die als Nutzerabbruch gelten (6000 Abbruch, 6001 Timeout; docs/analyse/02 §2.6).
pub const USER_CANCEL_CODES: [u32; 2] = [6000, 6001];

/// Fehler beim Bauen/Parsen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SlError {
    /// XML nicht wohlgeformt.
    #[error("XML-Fehler: {0}")]
    Xml(String),
    /// Unerwartete Wurzel (keine `sl:*Response`).
    #[error("unerwartete Wurzel: {0}")]
    UnexpectedRoot(String),
    /// Pflichtelement fehlt.
    #[error("Element fehlt: {0}")]
    MissingElement(&'static str),
    /// Base64 ungültig.
    #[error("Base64 ungültig: {0}")]
    Base64(String),
}

/// Ergebnis-Alias.
pub type Result<T, E = SlError> = std::result::Result<T, E>;

/// `ExcludedByteRange` (From/To nullbasiert, **beide inklusive**; docs/analyse/02 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExcludedByteRange {
    /// Offset von `<`
    pub from: u64,
    /// Offset von `>`
    pub to: u64,
}

/// Wie das Dokument im `CreateCMSSignatureRequest` steht (docs/analyse/02 §2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentContent {
    /// `<sl:Content Reference="formdata:fileupload"/>` – Datei als Multipart-Teil (A-Trust immer).
    Upload,
    /// `<sl:Content><sl:Base64Content>…</sl:Base64Content></sl:Content>` (lokale BKU mit
    /// `Server: trustDeskbasic|asignSecurityLayer`).
    Base64(Vec<u8>),
}

/// Parameter des `CreateCMSSignatureRequest` (`Structure="detached"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCmsRequest {
    /// `KeyboxIdentifier`, Default [`SECURE_SIGNATURE_KEYPAIR`].
    pub keybox: String,
    /// `MetaInfo/MimeType`, Default [`MIME_PDF`].
    pub mime_type: String,
    /// Dokument (Upload oder Base64; Blackout-PDF).
    pub content: DocumentContent,
    /// Ausgeschlossener Bereich.
    pub excluded: ExcludedByteRange,
    /// Attribut `PAdESCompatibility="true"` (Test in S3, Plan §6.10).
    pub pades_compatibility: bool,
}

impl CreateCmsRequest {
    /// Standardwerte wie PDF-AS (`BaseSLConnector`).
    pub fn new(content: DocumentContent, excluded: ExcludedByteRange) -> Self {
        Self {
            keybox: SECURE_SIGNATURE_KEYPAIR.into(),
            mime_type: MIME_PDF.into(),
            content,
            excluded,
            pades_compatibility: false,
        }
    }
}

/// SL-Requests, die der Klon sendet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlRequest {
    /// `InfoboxReadRequest` (Certificates / SecureSignatureKeypair).
    InfoboxReadCertificates,
    /// `CreateCMSSignatureRequest`.
    CreateCmsSignature(CreateCmsRequest),
    /// `NullOperationRequest` (Probe der lokalen BKU).
    NullOperation,
}

impl SlRequest {
    /// Serialisiert als UTF-8-XML mit Präfix `sl:` (Elementreihenfolge laut `propOrder`).
    pub fn to_xml(&self) -> String {
        todo!("Paket D: quick-xml Writer; NullOperation exakt wie PDF-Over (docs/analyse/02 §4.2)")
    }
}

/// Teil eines Multipart-Bodys (übernimmt `pdfover-net::FormPart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// Feldname
    pub name: String,
    /// Dateiname
    pub filename: Option<String>,
    /// Content-Type
    pub content_type: Option<String>,
    /// Inhalt
    pub data: Vec<u8>,
}

/// HTTP-Körper eines SL-Requests (SL-HTTPS-Binding, docs/analyse/02 §2.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpBody {
    /// urlencoded, ein Feld `XMLRequest`.
    UrlEncoded(Vec<(String, String)>),
    /// multipart in genau dieser Reihenfolge.
    Multipart(Vec<Part>),
}

/// Reihenfolge der Multipart-Teile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartOrder {
    /// A-Trust: `fileupload` (`application/pdf`) zuerst, dann `XMLRequest` (`text/plain; charset=UTF-8`).
    FileFirst,
    /// Lokale BKU: `XMLRequest` zuerst, dann `fileupload` (`application/octet-stream`).
    XmlFirst,
}

/// Baut den HTTP-Körper: ohne Datei urlencoded, mit Datei (Upload-Modus) multipart.
pub fn http_body(request: &SlRequest, upload: Option<&[u8]>, order: PartOrder) -> HttpBody {
    todo!("Paket D")
}

/// `InfoboxReadResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoboxReadResponse {
    /// Paare `Key` → dekodierter `Base64Content`.
    pub pairs: Vec<(String, Vec<u8>)>,
}

impl InfoboxReadResponse {
    /// DER des Signaturzertifikats (erstes Paar mit Key `SecureSignatureKeypair`).
    pub fn signature_certificate(&self) -> Option<&[u8]> {
        self.pairs.iter().find(|(k, _)| k == SECURE_SIGNATURE_KEYPAIR).map(|(_, v)| v.as_slice())
    }
}

/// `CreateCMSSignatureResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCmsSignatureResponse {
    /// DER von `CMSSignature` (ContentInfo/SignedData).
    pub cms: Vec<u8>,
}

/// `ErrorResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorResponse {
    /// `ErrorCode`
    pub code: u32,
    /// `Info`
    pub info: String,
}

impl ErrorResponse {
    /// 6000/6001 = Nutzerabbruch (stiller Rücksprung zur Backend-Auswahl, docs/analyse/02 §2.6).
    pub fn is_user_cancel(&self) -> bool {
        USER_CANCEL_CODES.contains(&self.code)
    }
}

/// Geparste SL-Antwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlResponse {
    /// `InfoboxReadResponse`
    InfoboxRead(InfoboxReadResponse),
    /// `CreateCMSSignatureResponse`
    CreateCmsSignature(CreateCmsSignatureResponse),
    /// `NullOperationResponse`
    NullOperation,
    /// `ErrorResponse`
    Error(ErrorResponse),
}

/// Parst eine SL-Antwort (namespace-korrekt, beliebiges Präfix).
pub fn parse_response(xml: &[u8]) -> Result<SlResponse> {
    todo!("Paket D")
}

/// `true`, wenn `body` ein XML-Dokument mit Wurzel `*Response` im SL-Namespace ist
/// (Ende-Erkennung der A-Trust-Schleife zusammen mit MIME `…/xml`).
pub fn is_sl_response(body: &[u8]) -> bool {
    todo!("Paket D")
}
