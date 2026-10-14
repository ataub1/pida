//! Prüfung fremder CMS (Modell B, Invarianten I3/I4, Plan §6.10 „Verifier“).

use std::io::Read;

use time::OffsetDateTime;

use crate::{Certificate, DigestAlg, Result, SignatureAlg};

/// Welche Algorithmen beim Prüfen akzeptiert werden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlgorithmPolicy {
    /// Neue Signatur (I3): SHA-256/384/512, RSA PKCS#1 v1.5, ECDSA P-256/384/521.
    #[default]
    NewSignature,
    /// Vorsignaturen (I4): zusätzlich SHA-1 und RSASSA-PSS.
    PriorSignature,
}

/// Erwarteter Signer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ExpectedSigner {
    /// Keine Erwartung (Vorsignaturen).
    #[default]
    Any,
    /// Genau dieses Zertifikat (InfoboxRead, Cache, Keystore, CSC `credentials/info`) –
    /// PDF-AS vergleicht den Fingerprint („Certificates missmatch!“).
    Certificate(Certificate),
}

/// Erwartungen an ein CMS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expect {
    /// Erwarteter Signer.
    pub signer: ExpectedSigner,
    /// signing-certificate(-v2) muss vorhanden sein und passen (PDF-AS: ja).
    pub require_signing_certificate: bool,
    /// Höchstlänge des CMS (Reserve).
    pub max_len: Option<usize>,
    /// Algorithmuspolitik.
    pub policy: AlgorithmPolicy,
}

impl Default for Expect {
    fn default() -> Self {
        Self {
            signer: ExpectedSigner::Any,
            require_signing_certificate: true,
            max_len: None,
            policy: AlgorithmPolicy::NewSignature,
        }
    }
}

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsReport {
    /// Signerzertifikat.
    pub signer: Certificate,
    /// Alle Zertifikate im CMS.
    pub certificates: Vec<Certificate>,
    /// Hashverfahren.
    pub digest_alg: DigestAlg,
    /// Signaturalgorithmus.
    pub signature_alg: SignatureAlg,
    /// signing-time, falls vorhanden.
    pub signing_time: Option<OffsetDateTime>,
    /// OIDs der signierten Attribute in DER-Reihenfolge.
    pub signed_attr_oids: Vec<String>,
    /// signing-certificate (v1) vorhanden.
    pub has_signing_certificate_v1: bool,
    /// signing-certificate-v2 vorhanden.
    pub has_signing_certificate_v2: bool,
    /// Länge des CMS (ohne `0`-Padding).
    pub der_len: usize,
}

/// Struktur eines CMS ohne kryptografische Prüfung (für `pades::analyze`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsInfo {
    /// Anzahl SignerInfos.
    pub signer_count: usize,
    /// Zertifikate im CMS.
    pub certificates: Vec<Certificate>,
    /// Hashverfahren des ersten SignerInfo.
    pub digest_alg: Option<DigestAlg>,
    /// Signaturalgorithmus des ersten SignerInfo.
    pub signature_alg: Option<SignatureAlg>,
    /// Länge des DER.
    pub der_len: usize,
}

/// Prüft ein detached CMS gegen `content` (ByteRange-Konkatenation, gestreamt).
///
/// Prüfschritte wie PDF-AS (docs/analyse/03 §5.2 Punkte 1–5): DER parsebar, genau ein SignerInfo,
/// message-digest = Hash(content), Signaturwert gültig, signing-certificate(-v2) passt,
/// Signer = erwartetes Zertifikat. Differentiell gegen `openssl cms -verify` zu testen (Plan §8.4).
pub fn verify_detached(cms: &[u8], content: &mut dyn Read, expect: &Expect) -> Result<CmsReport> {
    todo!("Paket B")
}

/// Liest die Struktur eines CMS ohne Prüfung.
pub fn parse_cms(cms: &[u8]) -> Result<CmsInfo> {
    todo!("Paket B")
}

/// Schneidet das `0`-Padding hinter dem DER aus einem `/Contents`-Wert ab (über die DER-Länge).
pub fn trim_contents_padding(contents: &[u8]) -> Result<&[u8]> {
    todo!("Paket B")
}
