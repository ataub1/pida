//! `pida-sl12` – Security Layer 1.2 (gemeinsam für A-Trust Szenario A und lokale BKU).
//!
//! Spezifikation: docs/analyse/02 §2 (Strukturen, Fehler, Transportvarianten), §8 (Konstanten),
//! docs/analyse/03 §6.2. Reiner XML/Binding-Code ohne Netz – [`HttpBody`] beschreibt den
//! Request-Körper, `pida-net` überträgt ihn.
//!
//! **Serialisierung:** [`SlRequest::to_xml`] erzeugt Byte für Byte die Ausgabe des JAXB-Marshallers
//! von PDF-AS 4.4.2 (`SLMarschaller`: `JAXB_FORMATTED_OUTPUT`, UTF-8, jaxb-runtime 2.3.3): SL-Namespace
//! als **Default-Namespace**, zusätzlich `xmlns:ns2` für XMLDSig (der JAXB-Kontext kennt beide),
//! `standalone="yes"`, vier Leerzeichen Einrückung, abschließendes `\n`. Die Golden-Dateien unter
//! `testdata/sl12/jaxb-golden/` wurden mit den Original-Schemaklassen erzeugt. Ausnahme:
//! `NullOperationRequest` ist wie in PDF-Over handgeschrieben (`LocalBKUState.java:68-69`).
//!
//! **Parsen:** namespace-korrekt mit beliebigem Präfix; DTDs und unbekannte Entities werden
//! abgelehnt (wie `SLMarschaller` mit `SUPPORT_DTD=false`).
//!
//! Ownership: Paket **D**.

mod dom;

use base64::Engine as _;

/// SL-1.2-Namespace.
pub const SL_NAMESPACE: &str = "http://www.buergerkarte.at/namespaces/securitylayer/1.2#";
/// XMLDSig-Namespace (von JAXB am Wurzelelement als `ns2` deklariert).
pub const XMLDSIG_NAMESPACE: &str = "http://www.w3.org/2000/09/xmldsig#";
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
/// `Structure`-Attribut des `CreateCMSSignatureRequest`.
pub const STRUCTURE_DETACHED: &str = "detached";
/// Content-Type der Textteile (`XMLRequest`) im Multipart-Körper.
pub const TEXT_PLAIN_UTF8: &str = "text/plain; charset=UTF-8";
/// Content-Type des Dateiteils bei der lokalen BKU (commons-httpclient-3-`FilePart`-Default).
pub const OCTET_STREAM: &str = "application/octet-stream";

/// `NullOperationRequest` exakt wie PDF-Over (`LocalBKUState.java:68-69`, ohne Zeilenumbruch).
pub const NULL_OPERATION_REQUEST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><sl:NullOperationRequest xmlns:sl=\"http://www.buergerkarte.at/namespaces/securitylayer/1.2#\"/>";

/// XML-Deklaration der JAXB-Ausgabe.
const JAXB_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

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

impl ExcludedByteRange {
    /// Aus der PDF-`/ByteRange` `[0, a, b, c]` wie PDF-AS `PDFUtils.buildExcludeRange`:
    /// `From = br[0] + br[1]`, `To = br[2] - 1`. `None`, wenn `br[2] == 0` oder die Lücke leer ist.
    pub fn from_byte_range(byte_range: [u64; 4]) -> Option<Self> {
        let from = byte_range[0].checked_add(byte_range[1])?;
        let to = byte_range[2].checked_sub(1)?;
        (from <= to).then_some(Self { from, to })
    }

    /// Länge der Lücke in Bytes.
    pub fn len(&self) -> u64 {
        self.to.saturating_sub(self.from) + 1
    }

    /// Immer `false` (die Lücke umfasst mindestens `<` und `>`).
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// Wie das Dokument im `CreateCMSSignatureRequest` steht (docs/analyse/02 §2.5). `Debug` ohne
/// Dokumentinhalt (ADR 0001 §4 Regel 9).
#[derive(Clone, PartialEq, Eq)]
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

/// Text-Escaping wie der JAXB-RI-`MinimumEscapeHandler` (`&`, `<`, `>`; in Attributen zusätzlich `"`).
fn escape(value: &str, attribute: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

fn root_namespaces() -> String {
    format!("xmlns=\"{SL_NAMESPACE}\" xmlns:ns2=\"{XMLDSIG_NAMESPACE}\"")
}

impl SlRequest {
    /// Serialisiert als UTF-8-XML exakt wie der JAXB-Marshaller von PDF-AS (siehe Crate-Doku;
    /// Elementreihenfolge laut `propOrder`). `NullOperation` liefert [`NULL_OPERATION_REQUEST`].
    pub fn to_xml(&self) -> String {
        match self {
            SlRequest::NullOperation => NULL_OPERATION_REQUEST.to_owned(),
            SlRequest::InfoboxReadCertificates => format!(
                "{JAXB_DECLARATION}<InfoboxReadRequest {ns}>\n    <InfoboxIdentifier>{INFOBOX_CERTIFICATES}</InfoboxIdentifier>\n    <AssocArrayParameters>\n        <ReadValue Key=\"{SECURE_SIGNATURE_KEYPAIR}\"/>\n    </AssocArrayParameters>\n</InfoboxReadRequest>\n",
                ns = root_namespaces()
            ),
            SlRequest::CreateCmsSignature(req) => {
                let mut xml = String::with_capacity(
                    720 + match &req.content {
                        DocumentContent::Base64(d) => d.len() * 4 / 3 + 4,
                        DocumentContent::Upload => 0,
                    },
                );
                xml.push_str(JAXB_DECLARATION);
                xml.push_str("<CreateCMSSignatureRequest Structure=\"");
                xml.push_str(STRUCTURE_DETACHED);
                xml.push('"');
                if req.pades_compatibility {
                    xml.push_str(" PAdESCompatibility=\"true\"");
                }
                xml.push(' ');
                xml.push_str(&root_namespaces());
                xml.push_str(">\n    <KeyboxIdentifier>");
                xml.push_str(&escape(&req.keybox, false));
                xml.push_str("</KeyboxIdentifier>\n    <DataObject>\n        <MetaInfo>\n            <MimeType>");
                xml.push_str(&escape(&req.mime_type, false));
                xml.push_str("</MimeType>\n        </MetaInfo>\n");
                match &req.content {
                    DocumentContent::Upload => {
                        xml.push_str("        <Content Reference=\"");
                        xml.push_str(&escape(UPLOAD_REFERENCE, true));
                        xml.push_str("\"/>\n");
                    }
                    DocumentContent::Base64(data) => {
                        xml.push_str("        <Content>\n            <Base64Content>");
                        xml.push_str(&base64::engine::general_purpose::STANDARD.encode(data));
                        xml.push_str("</Base64Content>\n        </Content>\n");
                    }
                }
                xml.push_str(&format!(
                    "        <ExcludedByteRange>\n            <From>{}</From>\n            <To>{}</To>\n        </ExcludedByteRange>\n    </DataObject>\n</CreateCMSSignatureRequest>\n",
                    req.excluded.from, req.excluded.to
                ));
                xml
            }
        }
    }
}

/// Teil eines Multipart-Bodys (übernimmt `pida-net::FormPart`). `Debug` nur mit Länge.
#[derive(Clone, PartialEq, Eq)]
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

/// HTTP-Körper eines SL-Requests (SL-HTTPS-Binding, docs/analyse/02 §2.7). `Debug` ohne Werte
/// (`XMLRequest` kann `Base64Content` mit dem Dokument enthalten).
#[derive(Clone, PartialEq, Eq)]
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
///
/// - A-Trust (`MobileBKUConnector.java:200-213`): `fileupload` (`sign.pdf`, `application/pdf`) +
///   `XMLRequest` (`text/plain; charset=UTF-8`).
/// - lokale BKU (`LocalBKUConnector.java:86-99`): `XMLRequest` (`text/plain; charset=UTF-8`) +
///   `fileupload` (`sign.pdf`, `application/octet-stream`).
pub fn http_body(request: &SlRequest, upload: Option<&[u8]>, order: PartOrder) -> HttpBody {
    let xml = request.to_xml();
    let Some(file) = upload else {
        return HttpBody::UrlEncoded(vec![(FIELD_XML_REQUEST.to_owned(), xml)]);
    };
    let xml_part = Part {
        name: FIELD_XML_REQUEST.into(),
        filename: None,
        content_type: Some(TEXT_PLAIN_UTF8.into()),
        data: xml.into_bytes(),
    };
    let file_part = |content_type: &str| Part {
        name: FIELD_FILE_UPLOAD.into(),
        filename: Some(UPLOAD_FILENAME.into()),
        content_type: Some(content_type.into()),
        data: file.to_vec(),
    };
    HttpBody::Multipart(match order {
        PartOrder::FileFirst => vec![file_part(MIME_PDF), xml_part],
        PartOrder::XmlFirst => vec![xml_part, file_part(OCTET_STREAM)],
    })
}

/// `InfoboxReadResponse`. `Debug` ohne Zertifikatsinhalt (Personendaten).
#[derive(Clone, PartialEq, Eq)]
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

/// `CreateCMSSignatureResponse`. `Debug` nur mit Länge.
#[derive(Clone, PartialEq, Eq)]
pub struct CreateCmsSignatureResponse {
    /// DER von `CMSSignature` (ContentInfo/SignedData).
    pub cms: Vec<u8>,
}

impl std::fmt::Debug for DocumentContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentContent::Upload => f.write_str("Upload"),
            DocumentContent::Base64(doc) => f.debug_struct("Base64").field("len", &doc.len()).finish(),
        }
    }
}

impl std::fmt::Debug for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Part")
            .field("name", &self.name)
            .field("filename", &self.filename)
            .field("content_type", &self.content_type)
            .field("len", &self.data.len())
            .finish()
    }
}

impl std::fmt::Debug for HttpBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpBody::UrlEncoded(fields) => {
                let shown: Vec<(&str, usize)> = fields.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
                f.debug_tuple("UrlEncoded").field(&shown).finish()
            }
            HttpBody::Multipart(parts) => f.debug_tuple("Multipart").field(parts).finish(),
        }
    }
}

impl std::fmt::Debug for InfoboxReadResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shown: Vec<(&str, usize)> = self.pairs.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
        f.debug_struct("InfoboxReadResponse").field("pairs", &shown).finish()
    }
}

impl std::fmt::Debug for CreateCmsSignatureResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateCmsSignatureResponse").field("cms_len", &self.cms.len()).finish()
    }
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

/// Base64 wie `DatatypeConverter.parseBase64Binary` (Leerraum wird ignoriert, Padding optional).
fn decode_base64(text: &str) -> Result<Vec<u8>> {
    let compact: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    base64::engine::general_purpose::STANDARD_PAD_INDIFFERENT
        .decode(compact.as_bytes())
        .map_err(|e| SlError::Base64(e.to_string()))
}

/// Parst eine SL-Antwort (namespace-korrekt, beliebiges Präfix).
pub fn parse_response(xml: &[u8]) -> Result<SlResponse> {
    let root = dom::parse(xml)?;
    if root.namespace.as_deref() != Some(SL_NAMESPACE) {
        return Err(SlError::UnexpectedRoot(match &root.namespace {
            Some(ns) => format!("{{{ns}}}{}", root.local),
            None => root.local.clone(),
        }));
    }
    match root.local.as_str() {
        "InfoboxReadResponse" => {
            let mut pairs = Vec::new();
            if let Some(data) = root.child(SL_NAMESPACE, "AssocArrayData") {
                for pair in data.children_named(SL_NAMESPACE, "Pair") {
                    let Some(key) = pair.attribute("Key") else { continue };
                    let Some(content) = pair.child(SL_NAMESPACE, "Base64Content") else { continue };
                    match decode_base64(&content.text) {
                        Ok(value) => pairs.push((key.to_owned(), value)),
                        // Nur das Signaturzertifikat ist Pflicht (PDF-AS liest nur dieses Paar);
                        // ein unlesbares anderes Paar macht die Antwort nicht unbrauchbar.
                        Err(e) if key == SECURE_SIGNATURE_KEYPAIR => return Err(e),
                        Err(_) => continue,
                    }
                }
            }
            Ok(SlResponse::InfoboxRead(InfoboxReadResponse { pairs }))
        }
        "CreateCMSSignatureResponse" => {
            let cms = root.child(SL_NAMESPACE, "CMSSignature").ok_or(SlError::MissingElement("CMSSignature"))?;
            Ok(SlResponse::CreateCmsSignature(CreateCmsSignatureResponse { cms: decode_base64(&cms.text)? }))
        }
        "NullOperationResponse" => Ok(SlResponse::NullOperation),
        "ErrorResponse" => {
            let code = root.child(SL_NAMESPACE, "ErrorCode").ok_or(SlError::MissingElement("ErrorCode"))?;
            let code = code.text.trim().parse::<u32>().map_err(|_| SlError::Xml("ErrorCode ist keine Zahl".into()))?;
            let info = root.child(SL_NAMESPACE, "Info").map(|e| e.text.clone()).unwrap_or_default();
            Ok(SlResponse::Error(ErrorResponse { code, info }))
        }
        other => Err(SlError::UnexpectedRoot(other.to_owned())),
    }
}

/// `true`, wenn `body` ein XML-Dokument mit Wurzel `*Response` im SL-Namespace ist
/// (Ende-Erkennung der A-Trust-Schleife zusammen mit MIME `…/xml`).
pub fn is_sl_response(body: &[u8]) -> bool {
    dom::root_name(body).is_some_and(|(ns, local)| ns.as_deref() == Some(SL_NAMESPACE) && local.ends_with("Response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_range_from_byte_range() {
        // br = [0, a, b, len-b], From = a, To = b-1
        assert_eq!(
            ExcludedByteRange::from_byte_range([0, 100, 4198, 50]),
            Some(ExcludedByteRange { from: 100, to: 4197 })
        );
        assert_eq!(ExcludedByteRange { from: 100, to: 4197 }.len(), 4098);
        assert_eq!(ExcludedByteRange::from_byte_range([0, 100, 0, 0]), None);
        assert_eq!(ExcludedByteRange::from_byte_range([0, 100, 100, 0]), None);
    }

    #[test]
    fn escaping() {
        assert_eq!(escape("a&b<c>\"d", false), "a&amp;b&lt;c&gt;\"d");
        assert_eq!(escape("a\"b", true), "a&quot;b");
        let mut r = CreateCmsRequest::new(DocumentContent::Upload, ExcludedByteRange { from: 1, to: 2 });
        r.keybox = "K<&>".into();
        assert!(
            SlRequest::CreateCmsSignature(r).to_xml().contains("<KeyboxIdentifier>K&lt;&amp;&gt;</KeyboxIdentifier>")
        );
    }

    #[test]
    fn debug_is_redacted() {
        let body = http_body(
            &SlRequest::CreateCmsSignature(CreateCmsRequest::new(
                DocumentContent::Base64(b"GEHEIMES-DOKUMENT".to_vec()),
                ExcludedByteRange { from: 1, to: 2 },
            )),
            None,
            PartOrder::FileFirst,
        );
        let dbg = format!("{body:?}");
        assert!(!dbg.contains("R0VIRUlNRVMtRE9LVU1FTlQ"), "{dbg}");
        assert!(dbg.contains("XMLRequest"));
        let r = InfoboxReadResponse { pairs: vec![("SecureSignatureKeypair".into(), b"CERTBYTES".to_vec())] };
        assert!(!format!("{r:?}").contains("CERTBYTES"));
        let c = CreateCmsSignatureResponse { cms: b"CMSBYTES".to_vec() };
        assert!(!format!("{c:?}").contains("CMSBYTES"));
        assert!(!format!("{:?}", DocumentContent::Base64(b"X1".to_vec())).contains("X1"));
    }
}
