//! Aufbau von SignedAttributes und SignedData (Modell A, Plan §3.7, §6.10).

use time::OffsetDateTime;

use crate::{CertChain, Certificate, Digest, DigestAlg, KeyType, Result, SignatureAlg};

/// CMS-Profil der signierten Attribute (Plan §6.10, Tabelle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum CmsProfile {
    /// Verhaltensgleich zu PDF-AS: content-type, ETSI mime-type `application/pdf`,
    /// signing-time (UTCTime), message-digest, signing-certificate-v2 (issuerSerial, ohne
    /// hashAlgorithm bei SHA-256); ECDSA-AlgorithmIdentifier mit NULL-Parametern.
    #[default]
    PdfasCompat,
    /// PAdES-BASELINE-B nach ETSI EN 319 142-1: content-type, message-digest,
    /// signing-certificate-v2; ohne signing-time und mime-type; ECDSA ohne Parameter.
    PadesBB,
}

/// Signierzeitpunkt (wird bei `PdfasCompat` als `signing-time` kodiert und ist gleich `/M`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SigningTime(pub OffsetDateTime);

/// Builder für die SignedAttributes.
#[derive(Debug, Clone)]
pub struct SignedAttrsBuilder {
    profile: CmsProfile,
    message_digest: Digest,
    signer: Certificate,
    signing_time: SigningTime,
}

impl SignedAttrsBuilder {
    /// Neuer Builder. `message_digest` ist der Hash über die ByteRange-Daten.
    pub fn new(profile: CmsProfile, message_digest: &Digest, signer: &Certificate, now: SigningTime) -> Self {
        Self { profile, message_digest: message_digest.clone(), signer: signer.clone(), signing_time: now }
    }

    /// Baut die Attribute DER-genau (SET OF sortiert, docs/analyse/03 §5.1).
    pub fn build(&self) -> Result<SignedAttrs> {
        todo!(
            "Paket B: content-type, [mime-type], [signing-time], message-digest, signing-certificate-v2 (bzw. v1 bei SHA-1)"
        )
    }
}

/// Fertige SignedAttributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAttrs {
    /// DER als `SET OF Attribute` (Tag `0x31`) – genau diese Bytes werden signiert.
    pub der: Vec<u8>,
    /// Hashverfahren von message-digest.
    pub digest_alg: DigestAlg,
    /// Profil.
    pub profile: CmsProfile,
}

impl SignedAttrs {
    /// DER (`SET OF`, Tag 0x31).
    pub fn to_der(&self) -> &[u8] {
        &self.der
    }

    /// Hash über das DER – Eingabe für [`RawSigner::sign_digest`] bzw. CSC `signHash`.
    pub fn digest(&self, alg: DigestAlg) -> Digest {
        alg.digest(&self.der)
    }

    /// DER mit implizitem Tag `[0]` (`0xA0`) für den Einbau in SignerInfo.
    pub fn implicit_tagged_der(&self) -> Vec<u8> {
        todo!("Paket B")
    }
}

/// Rohsignatur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSignature {
    /// Algorithmus.
    pub alg: SignatureAlg,
    /// Wert: RSA = Modullänge in Bytes; ECDSA = DER `SEQUENCE { r, s }`.
    pub value: Vec<u8>,
}

impl RawSignature {
    /// Wandelt ECDSA `r||s` (fest, z. B. WebCrypto/CSC) nach DER (docs/analyse/02 §5.4).
    pub fn ecdsa_from_fixed(alg: SignatureAlg, fixed: &[u8]) -> Result<Self> {
        todo!("Paket B")
    }
}

/// Lokaler Signierer über einen Hash (Keystore; später CSC über einen Adapter).
///
/// Vertrag (ADR 0001 §8, gilt auch für `pdfover_backend_api::DigestJob`): Der Kern wählt das
/// Hashverfahren mit [`select_algorithm`] aus dem Zertifikat, hasht damit die ByteRange-Daten
/// (message-digest) **und** die SignedAttributes. [`RawSigner::sign_digest`] signiert mit genau dem
/// Verfahren von `digest.alg` (RSA: DigestInfo dieses Verfahrens; ECDSA: Hash direkt) und liefert
/// `RawSignature::alg` mit `alg.digest() == digest.alg`.
pub trait RawSigner: Send + Sync {
    /// Zertifikatskette des Schlüssels.
    fn chain(&self) -> &CertChain;
    /// Algorithmus nach PDF-AS-Regeln ([`select_algorithm`]) – Vorgabe für den Kern.
    fn algorithm(&self) -> SignatureAlg;
    /// Signiert den Hash der SignedAttributes (Verfahren = `digest.alg`, siehe Vertrag oben).
    fn sign_digest(&self, digest: &Digest) -> Result<RawSignature>;
}

/// Obere Schranke der DER-Länge eines CMS, wie es [`assemble`] bzw. ein Modell-B-Dienst für diese
/// Kette liefert – Grundlage der `/Contents`-Reserve (Plan §6.2): Modell A „berechnet + 1 KiB,
/// mindestens 4 KiB“; Modell B Plausibilitätsprüfung vor dem Upload (Reserve vergrößern, wenn die
/// Schätzung 50 % übersteigt). Berücksichtigt Zertifikate (`include_intermediates`), die signierten
/// Attribute des Profils und die Höchstlänge des Signaturwerts (RSA = Modullänge, ECDSA =
/// DER `SEQUENCE { r, s }` mit maximaler Integer-Länge).
pub fn estimate_cms_len(
    chain: &CertChain,
    profile: CmsProfile,
    alg: SignatureAlg,
    include_intermediates: bool,
) -> Result<usize> {
    todo!("Paket B")
}

/// Optionen für [`assemble`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssembleOptions {
    /// Profil (bestimmt u. a. ECDSA-Parameterkodierung NULL vs. fehlend).
    pub profile: CmsProfile,
    /// Zwischenzertifikate mit einbetten (PDF-AS: nein; Entscheidung S3, Plan §6.10).
    pub include_intermediates: bool,
}

/// Montiert `ContentInfo(SignedData)` v1, detached, ein SignerInfo v1 (IssuerAndSerialNumber),
/// ohne unsignierte Attribute und ohne CMSAlgorithmProtection.
pub fn assemble(attrs: &SignedAttrs, sig: &RawSignature, chain: &CertChain, opts: &AssembleOptions) -> Result<Vec<u8>> {
    todo!("Paket B: SignedData über cms::builder bzw. eigenes DER (Faktencheck 4a)")
}

/// Algorithmuswahl wie PDF-AS (docs/analyse/03 §5.1) mit bewussten Abweichungen (Plan §4.9):
/// RSA → sha256WithRSA; EC ≥ 512 → SHA-512; EC ≥ 256 (auch P-384) → SHA-256;
/// EC < 256 und DSA → [`crate::CmsError::UnsupportedAlgorithm`].
pub fn select_algorithm(key: &KeyType) -> Result<SignatureAlg> {
    todo!("Paket B")
}

/// Bequemlichkeitsfunktion: Attribute bauen, signieren, montieren.
pub fn sign_detached(
    signer: &dyn RawSigner,
    message_digest: &Digest,
    profile: CmsProfile,
    now: SigningTime,
) -> Result<Vec<u8>> {
    let attrs = SignedAttrsBuilder::new(profile, message_digest, &signer.chain().signer, now).build()?;
    let sig = signer.sign_digest(&attrs.digest(signer.algorithm().digest()))?;
    assemble(&attrs, &sig, signer.chain(), &AssembleOptions { profile, include_intermediates: false })
}
