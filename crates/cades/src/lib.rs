//! `pdfover-cades` – CMS/CAdES für PAdES (Plan §2.1, §6.10; docs/analyse/03 §5).
//!
//! - [`SignedAttrsBuilder`]: SignedAttributes **DER-genau selbst gebaut** (SET OF sortiert) für die
//!   Profile [`CmsProfile::PdfasCompat`] und [`CmsProfile::PadesBB`].
//! - [`assemble`]: SignedData v1, detached, SignerInfo v1 mit IssuerAndSerialNumber (Modell A:
//!   Keystore, später CSC).
//! - [`verify_detached`]: Prüfung fremder CMS wie PDF-AS (`SignatureUtils.verifySignature`,
//!   docs/analyse/03 §5.2): genau ein SignerInfo, message-digest, Signaturwert,
//!   signing-certificate(-v2), Fingerprint gleich dem erwarteten Zertifikat (Invariante I3/I4).
//! - [`Certificate`], [`CertChain`], [`DigestAlg`], [`SignatureAlg`], [`RawSigner`]: gemeinsame
//!   Krypto-Typen für `pdfover-backend-api` und alle Backends.
//!
//! RustCrypto-Generation: `cms` 0.2 / `der` 0.7 / `x509-cert` 0.2 / `spki` 0.7 (gepinnt, Plan §2.5).
//! Diese Typen erscheinen nur hinter [`Certificate::x509`] in der öffentlichen API.
//!
//! Bereits im Fundament implementiert (ADR 0001 §5.1, damit Paket D ohne Paket B testen kann):
//! [`DigestAlg`]-Hashing, [`Certificate::from_der`] und die einfachen Zertifikats-Getter.
//!
//! Ownership: Paket **B** (zusammen mit `pdfover-backend-keystore`).

mod cert;
mod cms_build;
mod digest;
mod verify;

pub use cert::{CertChain, Certificate, EcCurve, KeyType, RdnAttr};
pub use cms_build::{
    AssembleOptions, CmsProfile, RawSignature, RawSigner, SignedAttrs, SignedAttrsBuilder, SigningTime, assemble,
    select_algorithm, sign_detached,
};
pub use digest::{Digest, DigestAlg, Hasher, SignatureAlg};
pub use verify::{
    AlgorithmPolicy, CmsInfo, CmsReport, Expect, ExpectedSigner, parse_cms, trim_contents_padding, verify_detached,
};

/// Fehler in CMS/X.509.
#[derive(Debug, thiserror::Error)]
pub enum CmsError {
    /// DER/ASN.1 nicht dekodierbar.
    #[error("DER-Fehler: {0}")]
    Der(String),
    /// Zertifikat nicht lesbar.
    #[error("Zertifikat nicht lesbar: {0}")]
    Certificate(String),
    /// Algorithmus nicht unterstützt bzw. per Politik abgelehnt (z. B. EC < 256 Bit, DSA; Plan §4.9).
    #[error("Algorithmus nicht unterstützt: {0}")]
    UnsupportedAlgorithm(String),
    /// Kein `SignedData`.
    #[error("kein CMS SignedData")]
    NotSignedData,
    /// Anzahl SignerInfos ≠ 1 (PDF-AS 11008).
    #[error("{0} SignerInfos statt genau einem")]
    SignerCount(usize),
    /// Pflichtattribut fehlt.
    #[error("signiertes Attribut fehlt: {0}")]
    MissingAttribute(&'static str),
    /// message-digest passt nicht zum Hash der ByteRange-Daten.
    #[error("message-digest stimmt nicht")]
    DigestMismatch,
    /// Signaturwert ungültig.
    #[error("Signaturwert ungültig")]
    InvalidSignature,
    /// Signerzertifikat fehlt in `certificates`.
    #[error("Signerzertifikat nicht im CMS")]
    SignerCertificateMissing,
    /// signing-certificate(-v2) referenziert ein anderes Zertifikat.
    #[error("signing-certificate passt nicht zum Signer")]
    SigningCertificateMismatch,
    /// Signer ist nicht das erwartete Zertifikat (PDF-AS „Certificates missmatch!“).
    #[error("Zertifikat des Signers weicht vom erwarteten ab")]
    UnexpectedSigner,
    /// Schlüssel nicht verwendbar.
    #[error("Schlüsselfehler: {0}")]
    Key(String),
    /// CMS länger als erlaubt.
    #[error("CMS zu groß: {len} > {max}")]
    TooLarge {
        /// Länge
        len: usize,
        /// Grenze
        max: usize,
    },
    /// E/A beim Lesen des Inhalts.
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
}

/// Ergebnis-Alias.
pub type Result<T, E = CmsError> = std::result::Result<T, E>;
