//! Prüfung fremder CMS (Modell B, Invarianten I3/I4, Plan §6.10 „Verifier“).
//!
//! Prüfschritte wie PDF-AS `SignatureUtils.verifySignature` (`SU:26-137`) und
//! `ISignatureConnectorSLWrapper` (`SCW:92-125`), docs/analyse/03 §5.2:
//!
//! 1. DER/BER-parsebares `ContentInfo(SignedData)`, detached (kein `eContent`).
//! 2. **Genau ein** SignerInfo (sonst [`CmsError::SignerCount`], PDF-AS 11008).
//! 3. Signerzertifikat in `certificates` (IssuerAndSerialNumber oder SubjectKeyIdentifier),
//!    message-digest = Hash der ByteRange-Daten, content-type = `eContentType`, Signaturwert gültig
//!    (RSA PKCS#1 v1.5, RSASSA-PSS nur mit [`AlgorithmPolicy::PriorSignature`], ECDSA
//!    P-256/384/521).
//! 4. ESS signing-certificate **oder** signing-certificate-v2 vorhanden und auf das Signerzertifikat
//!    verweisend (certHash; issuerSerial, falls vorhanden). Abweichung von PDF-AS: Sind beide
//!    vorhanden, werden **beide** geprüft (PDF-AS prüft nur v1).
//! 5. Signerzertifikat = erwartetes Zertifikat („Certificates missmatch!“, `SCW:119-123`).
//!
//! Algorithmuspolitik: [`AlgorithmPolicy::NewSignature`] (I3) erlaubt SHA-256/384/512 mit RSA
//! PKCS#1 v1.5 bzw. ECDSA; [`AlgorithmPolicy::PriorSignature`] (I4) zusätzlich SHA-1 (nur
//! Warnung [`CmsWarning::Sha1Digest`]) und RSASSA-PSS. Ketten, Sperrstatus und QES-Status prüft
//! der Klon wie PDF-AS nicht.

use std::io::Read;

use time::OffsetDateTime;

use crate::der_util::{Reader, Tlv, integer_magnitude, read_tlv, small_uint, tag};
use crate::{Certificate, CmsError, DigestAlg, KeyType, Result, SignatureAlg, oids};

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

/// Hinweis zu einem gültigen CMS (kein Fehler).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CmsWarning {
    /// SHA-1 als Hashverfahren (nur bei [`AlgorithmPolicy::PriorSignature`] akzeptiert).
    Sha1Digest,
    /// ESS signing-certificate v1 (ESSCertID mit SHA-1-Zertifikatshash).
    SigningCertificateV1,
    /// RSA-Schlüssel kürzer als 2048 Bit.
    WeakRsaKey {
        /// Modullänge
        bits: u32,
    },
    /// Signierte Attribute nicht DER-sortiert (gültig signiert, aber nicht kanonisch).
    NonDerSignedAttributes,
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
    /// Hinweise (z. B. SHA-1 bei Vorsignaturen) – leer bei einwandfreien neuen Signaturen.
    pub warnings: Vec<CmsWarning>,
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
///
/// `cms` darf mit `0x00` aufgefüllt sein (`/Contents`); das Padding wird über die DER-Länge
/// abgeschnitten, andere Daten hinter dem CMS sind ein Fehler.
pub fn verify_detached(cms: &[u8], content: &mut dyn Read, expect: &Expect) -> Result<CmsReport> {
    let cms = trim_contents_padding(cms)?;
    if let Some(max) = expect.max_len
        && cms.len() > max
    {
        return Err(CmsError::TooLarge { len: cms.len(), max });
    }
    let parsed = parse_signed_data(cms)?;
    if parsed.econtent_present {
        return Err(CmsError::Der("CMS mit eingebettetem Inhalt – detached erwartet".into()));
    }
    if parsed.signer_infos.len() != 1 {
        return Err(CmsError::SignerCount(parsed.signer_infos.len()));
    }
    let si = &parsed.signer_infos[0];
    let digest_alg = map_digest(&si.digest_alg.oid)
        .ok_or_else(|| CmsError::UnsupportedAlgorithm(format!("Hashverfahren {}", si.digest_alg.oid)))?;
    let signature_alg = resolve_signature_alg(si)?;

    let mut warnings = Vec::new();
    check_policy(expect.policy, digest_alg, signature_alg, &mut warnings)?;

    let signer = find_signer(&parsed.certificates, &si.sid).ok_or(CmsError::SignerCertificateMissing)?.clone();
    if let KeyType::Rsa { bits } = signer.key_type()?
        && bits < 2048
    {
        warnings.push(CmsWarning::WeakRsaKey { bits });
    }

    // Hash der ByteRange-Daten (gestreamt)
    let mut hasher = digest_alg.hasher();
    std::io::copy(content, &mut hasher)?;
    let content_digest = hasher.finalize();

    let mut signing_time = None;
    let mut signed_attr_oids = Vec::new();
    let mut ess_v1 = None;
    let mut ess_v2 = None;
    let signed_digest = match &si.signed_attrs {
        Some(attrs_tlv) => {
            let attrs = parse_attributes(attrs_tlv)?;
            let mut message_digest = None;
            let mut content_type = None;
            for a in &attrs {
                signed_attr_oids.push(a.oid.clone());
                let single = || -> Result<&Tlv<'_>> {
                    if a.values.len() == 1 {
                        Ok(&a.values[0])
                    } else {
                        Err(CmsError::Der(format!("Attribut {} mit {} Werten", a.oid, a.values.len())))
                    }
                };
                let slot = match a.oid.as_str() {
                    oids::MESSAGE_DIGEST => &mut message_digest,
                    oids::CONTENT_TYPE => &mut content_type,
                    oids::SIGNING_TIME => {
                        if signing_time.is_some() {
                            return Err(CmsError::Der("signing-time mehrfach".into()));
                        }
                        signing_time = Some(parse_time(single()?)?);
                        continue;
                    }
                    oids::SIGNING_CERTIFICATE => &mut ess_v1,
                    oids::SIGNING_CERTIFICATE_V2 => &mut ess_v2,
                    _ => continue,
                };
                if slot.is_some() {
                    return Err(CmsError::Der(format!("Attribut {} mehrfach", a.oid)));
                }
                *slot = Some(*single()?);
            }
            let md = message_digest.ok_or(CmsError::MissingAttribute("message-digest"))?;
            if md.tag != tag::OCTET_STRING {
                return Err(CmsError::Der("message-digest ist kein OCTET STRING".into()));
            }
            if md.content != content_digest.value.as_slice() {
                return Err(CmsError::DigestMismatch);
            }
            let ct = content_type.ok_or(CmsError::MissingAttribute("content-type"))?;
            if ct.oid_string()? != parsed.econtent_type {
                return Err(CmsError::Der("content-type passt nicht zu eContentType".into()));
            }
            if !is_der_sorted(attrs_tlv)? {
                warnings.push(CmsWarning::NonDerSignedAttributes);
            }
            // Signiert sind die Attribute mit Tag SET (0x31) statt [0] IMPLICIT (RFC 5652 §5.4).
            let mut to_hash = attrs_tlv.raw.to_vec();
            to_hash[0] = tag::SET;
            digest_alg.digest(&to_hash).value
        }
        None => content_digest.value.clone(),
    };

    crate::signer::verify_with_cert(&signer, signature_alg, &signed_digest, si.signature)?;

    if ess_v1.is_none() && ess_v2.is_none() && expect.require_signing_certificate {
        return Err(CmsError::MissingAttribute("signing-certificate"));
    }
    if let Some(v) = &ess_v1 {
        check_signing_certificate(v, &signer, false)?;
        warnings.push(CmsWarning::SigningCertificateV1);
    }
    if let Some(v) = &ess_v2 {
        check_signing_certificate(v, &signer, true)?;
    }

    if let ExpectedSigner::Certificate(expected) = &expect.signer
        && expected.fingerprint(DigestAlg::Sha256) != signer.fingerprint(DigestAlg::Sha256)
    {
        return Err(CmsError::UnexpectedSigner);
    }

    Ok(CmsReport {
        signer,
        certificates: parsed.certificates,
        digest_alg,
        signature_alg,
        signing_time,
        signed_attr_oids,
        has_signing_certificate_v1: ess_v1.is_some(),
        has_signing_certificate_v2: ess_v2.is_some(),
        der_len: cms.len(),
        warnings,
    })
}

/// Liest die Struktur eines CMS ohne Prüfung (Padding wie bei [`verify_detached`] erlaubt).
pub fn parse_cms(cms: &[u8]) -> Result<CmsInfo> {
    let cms = trim_contents_padding(cms)?;
    let parsed = parse_signed_data(cms)?;
    let first = parsed.signer_infos.first();
    Ok(CmsInfo {
        signer_count: parsed.signer_infos.len(),
        digest_alg: first.and_then(|si| map_digest(&si.digest_alg.oid)),
        signature_alg: first.and_then(|si| resolve_signature_alg(si).ok()),
        certificates: parsed.certificates,
        der_len: cms.len(),
    })
}

/// Schneidet das `0`-Padding hinter dem DER aus einem `/Contents`-Wert ab (über die DER-Länge).
///
/// Fehler: leer, kein `SEQUENCE`, Länge über das Ende hinaus oder andere Bytes als `0x00` hinter
/// dem CMS. BER mit unbestimmter Länge wird über das End-of-Contents bestimmt.
pub fn trim_contents_padding(contents: &[u8]) -> Result<&[u8]> {
    if contents.is_empty() {
        return Err(CmsError::Der("leerer /Contents-Wert".into()));
    }
    let t = read_tlv(contents)?;
    if t.tag != tag::SEQUENCE {
        return Err(CmsError::Der(format!("CMS beginnt mit Tag {:#04x} statt SEQUENCE", t.tag)));
    }
    if contents[t.raw.len()..].iter().any(|&b| b != 0) {
        return Err(CmsError::Der("Daten hinter dem CMS (kein 0-Padding)".into()));
    }
    Ok(t.raw)
}

// ---------------------------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------------------------

struct AlgId<'a> {
    oid: String,
    params: Option<Tlv<'a>>,
}

enum Sid<'a> {
    IssuerSerial { issuer: &'a [u8], serial: &'a [u8] },
    Ski(&'a [u8]),
}

struct SignerInfoRaw<'a> {
    sid: Sid<'a>,
    digest_alg: AlgId<'a>,
    signed_attrs: Option<Tlv<'a>>,
    sig_alg: AlgId<'a>,
    signature: &'a [u8],
}

struct ParsedSignedData<'a> {
    econtent_type: String,
    econtent_present: bool,
    certificates: Vec<Certificate>,
    signer_infos: Vec<SignerInfoRaw<'a>>,
}

struct Attr<'a> {
    oid: String,
    values: Vec<Tlv<'a>>,
}

fn parse_signed_data(cms: &[u8]) -> Result<ParsedSignedData<'_>> {
    let ci = read_tlv(cms)?.expect(tag::SEQUENCE, "ContentInfo")?;
    let mut r = ci.reader();
    let ct = r.read().and_then(|t| t.oid_string()).map_err(|_| CmsError::NotSignedData)?;
    if ct != oids::SIGNED_DATA {
        return Err(CmsError::NotSignedData);
    }
    let explicit = r.read_expect(0xA0, "ContentInfo.content")?;
    if !r.is_empty() {
        return Err(CmsError::Der("Restdaten in ContentInfo".into()));
    }
    let mut er0 = explicit.reader();
    let sd = er0.read_expect(tag::SEQUENCE, "SignedData")?;
    if !er0.is_empty() {
        return Err(CmsError::Der("Restdaten hinter SignedData".into()));
    }
    let mut s = sd.reader();
    let version = small_uint(&s.read_expect(tag::INTEGER, "SignedData.version")?)?;
    if version > 5 {
        return Err(CmsError::Der(format!("SignedData-Version {version}")));
    }
    s.read_expect(tag::SET, "digestAlgorithms")?;
    let eci = s.read_expect(tag::SEQUENCE, "encapContentInfo")?;
    let mut er = eci.reader();
    let econtent_type = er.read()?.oid_string()?;
    let econtent_present = er.read_optional(0xA0)?.is_some();

    let mut certificates = Vec::new();
    if let Some(certs) = s.read_optional(0xA0)? {
        for c in certs.reader().read_all()? {
            // CertificateChoices: nur X.509 (SEQUENCE); Attributzertifikate [1]/[2], other [3] überspringen.
            // Nicht lesbare Zusatzzertifikate (fremde CMS, Vorsignaturen) werden übersprungen – fehlt
            // dadurch das Signerzertifikat, meldet die Prüfung SignerCertificateMissing.
            if c.tag == tag::SEQUENCE
                && let Ok(cert) = Certificate::from_der(c.raw)
            {
                certificates.push(cert);
            }
        }
    }
    s.read_optional(0xA1)?; // crls
    let sis = s.read_expect(tag::SET, "signerInfos")?;
    if !s.is_empty() {
        return Err(CmsError::Der("Restdaten in SignedData".into()));
    }
    let mut signer_infos = Vec::new();
    for si in sis.reader().read_all()? {
        signer_infos.push(parse_signer_info(&si)?);
    }
    Ok(ParsedSignedData { econtent_type, econtent_present, certificates, signer_infos })
}

fn parse_alg_id<'a>(t: &Tlv<'a>) -> Result<AlgId<'a>> {
    let t = t.expect(tag::SEQUENCE, "AlgorithmIdentifier")?;
    let mut r = t.reader();
    let oid = r.read()?.oid_string()?;
    let params = if r.is_empty() { None } else { Some(r.read()?) };
    Ok(AlgId { oid, params })
}

fn parse_signer_info<'a>(si: &Tlv<'a>) -> Result<SignerInfoRaw<'a>> {
    let si = si.expect(tag::SEQUENCE, "SignerInfo")?;
    let mut r: Reader<'a> = si.reader();
    small_uint(&r.read_expect(tag::INTEGER, "SignerInfo.version")?)?;
    let sid_tlv = r.read()?;
    let sid = match sid_tlv.tag {
        tag::SEQUENCE => {
            let mut ir = sid_tlv.reader();
            let issuer = ir.read_expect(tag::SEQUENCE, "issuer")?;
            let serial = ir.read_expect(tag::INTEGER, "serialNumber")?;
            Sid::IssuerSerial { issuer: issuer.raw, serial: serial.content }
        }
        0x80 => Sid::Ski(sid_tlv.content),
        other => return Err(CmsError::Der(format!("SignerIdentifier mit Tag {other:#04x}"))),
    };
    let digest_alg = parse_alg_id(&r.read()?)?;
    let signed_attrs = r.read_optional(0xA0)?;
    let sig_alg = parse_alg_id(&r.read()?)?;
    let signature = r.read_expect(tag::OCTET_STRING, "signature")?.content;
    r.read_optional(0xA1)?; // unsignedAttrs
    if !r.is_empty() {
        return Err(CmsError::Der("Restdaten in SignerInfo".into()));
    }
    Ok(SignerInfoRaw { sid, digest_alg, signed_attrs, sig_alg, signature })
}

fn parse_attributes<'a>(attrs: &Tlv<'a>) -> Result<Vec<Attr<'a>>> {
    let mut out = Vec::new();
    for a in attrs.reader().read_all()? {
        let a = a.expect(tag::SEQUENCE, "Attribute")?;
        let mut r = a.reader();
        let oid = r.read()?.oid_string()?;
        let values = r.read_expect(tag::SET, "attrValues")?.reader().read_all()?;
        out.push(Attr { oid, values });
    }
    Ok(out)
}

fn is_der_sorted(attrs: &Tlv<'_>) -> Result<bool> {
    if attrs.indefinite {
        return Ok(false);
    }
    let items: Vec<Vec<u8>> = attrs.reader().read_all()?.iter().map(|t| t.raw.to_vec()).collect();
    let mut sorted = items.clone();
    crate::der_util::der_sort(&mut sorted);
    Ok(sorted == items)
}

/// Hash-OID → [`DigestAlg`].
fn map_digest(oid: &str) -> Option<DigestAlg> {
    Some(match oid {
        "1.3.14.3.2.26" => DigestAlg::Sha1,
        "2.16.840.1.101.3.4.2.1" => DigestAlg::Sha256,
        "2.16.840.1.101.3.4.2.2" => DigestAlg::Sha384,
        "2.16.840.1.101.3.4.2.3" => DigestAlg::Sha512,
        _ => return None,
    })
}

/// signatureAlgorithm + digestAlgorithm → [`SignatureAlg`]. Kombinierte OIDs müssen zum
/// digestAlgorithm passen; `rsaEncryption` (OpenSSL) bzw. `id-ecPublicKey` übernehmen ihn.
fn resolve_signature_alg(si: &SignerInfoRaw<'_>) -> Result<SignatureAlg> {
    let digest = map_digest(&si.digest_alg.oid)
        .ok_or_else(|| CmsError::UnsupportedAlgorithm(format!("Hashverfahren {}", si.digest_alg.oid)))?;
    let (alg, implied) = match si.sig_alg.oid.as_str() {
        oids::RSA_ENCRYPTION => (SignatureAlg::RsaPkcs1v15(digest), None),
        "1.2.840.113549.1.1.5" => (SignatureAlg::RsaPkcs1v15(DigestAlg::Sha1), Some(DigestAlg::Sha1)),
        "1.2.840.113549.1.1.11" => (SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256), Some(DigestAlg::Sha256)),
        "1.2.840.113549.1.1.12" => (SignatureAlg::RsaPkcs1v15(DigestAlg::Sha384), Some(DigestAlg::Sha384)),
        "1.2.840.113549.1.1.13" => (SignatureAlg::RsaPkcs1v15(DigestAlg::Sha512), Some(DigestAlg::Sha512)),
        oids::RSASSA_PSS => {
            let (hash, salt_len) = parse_pss_params(si.sig_alg.params.as_ref())?;
            (SignatureAlg::RsaPss { digest: hash, salt_len }, Some(hash))
        }
        oids::EC_PUBLIC_KEY => (SignatureAlg::Ecdsa(digest), None),
        "1.2.840.10045.4.1" => (SignatureAlg::Ecdsa(DigestAlg::Sha1), Some(DigestAlg::Sha1)),
        "1.2.840.10045.4.3.2" => (SignatureAlg::Ecdsa(DigestAlg::Sha256), Some(DigestAlg::Sha256)),
        "1.2.840.10045.4.3.3" => (SignatureAlg::Ecdsa(DigestAlg::Sha384), Some(DigestAlg::Sha384)),
        "1.2.840.10045.4.3.4" => (SignatureAlg::Ecdsa(DigestAlg::Sha512), Some(DigestAlg::Sha512)),
        other => return Err(CmsError::UnsupportedAlgorithm(format!("Signaturverfahren {other}"))),
    };
    if let Some(implied) = implied
        && implied != digest
    {
        return Err(CmsError::UnsupportedAlgorithm(format!(
            "Signaturverfahren mit {} bei digestAlgorithm {}",
            implied.name(),
            digest.name()
        )));
    }
    Ok(alg)
}

/// `RSASSA-PSS-params` (RFC 4055): hashAlgorithm [0] (Default SHA-1), maskGenAlgorithm [1]
/// (Default MGF1-SHA-1, muss denselben Hash nutzen), saltLength [2] (Default 20), trailerField [3]
/// (muss 1 sein).
fn parse_pss_params(params: Option<&Tlv<'_>>) -> Result<(DigestAlg, u16)> {
    let unsupported = |m: &str| CmsError::UnsupportedAlgorithm(format!("RSASSA-PSS: {m}"));
    let mut hash = DigestAlg::Sha1;
    let mut mgf_hash = DigestAlg::Sha1;
    let mut salt: u32 = 20;
    if let Some(p) = params
        && p.tag != tag::NULL
    {
        let p = p.expect(tag::SEQUENCE, "RSASSA-PSS-params")?;
        for field in p.reader().read_all()? {
            let inner = field.reader().read()?;
            match field.tag {
                0xA0 => {
                    let a = parse_alg_id(&inner)?;
                    hash = map_digest(&a.oid).ok_or_else(|| unsupported(&a.oid))?;
                }
                0xA1 => {
                    let a = parse_alg_id(&inner)?;
                    if a.oid != oids::MGF1 {
                        return Err(unsupported(&format!("Maskengenerator {}", a.oid)));
                    }
                    let h = a.params.as_ref().ok_or_else(|| unsupported("MGF1 ohne Hash"))?;
                    let h = parse_alg_id(h)?;
                    mgf_hash = map_digest(&h.oid).ok_or_else(|| unsupported(&h.oid))?;
                }
                0xA2 => salt = small_uint(&inner)?,
                0xA3 => {
                    if small_uint(&inner)? != 1 {
                        return Err(unsupported("trailerField ≠ 1"));
                    }
                }
                other => return Err(CmsError::Der(format!("RSASSA-PSS-params: Tag {other:#04x}"))),
            }
        }
    }
    if mgf_hash != hash {
        return Err(unsupported("MGF1-Hash ≠ Hash"));
    }
    let salt = u16::try_from(salt).map_err(|_| unsupported("saltLength zu groß"))?;
    Ok((hash, salt))
}

fn check_policy(
    policy: AlgorithmPolicy,
    digest: DigestAlg,
    alg: SignatureAlg,
    warnings: &mut Vec<CmsWarning>,
) -> Result<()> {
    let prior = policy == AlgorithmPolicy::PriorSignature;
    if digest == DigestAlg::Sha1 {
        if !prior {
            return Err(CmsError::UnsupportedAlgorithm("SHA-1 ist für neue Signaturen nicht zulässig".into()));
        }
        warnings.push(CmsWarning::Sha1Digest);
    }
    if matches!(alg, SignatureAlg::RsaPss { .. }) && !prior {
        return Err(CmsError::UnsupportedAlgorithm("RSASSA-PSS ist für neue Signaturen nicht zulässig".into()));
    }
    Ok(())
}

fn find_signer<'c>(certs: &'c [Certificate], sid: &Sid<'_>) -> Option<&'c Certificate> {
    certs.iter().find(|c| match sid {
        Sid::IssuerSerial { issuer, serial } => {
            integer_magnitude(c.serial_bytes()) == integer_magnitude(serial)
                && c.issuer_and_serial_raw().is_ok_and(|(ci, _)| names_equal(ci, issuer))
        }
        Sid::Ski(ski) => c.subject_key_identifier().is_some_and(|k| k.as_slice() == *ski),
    })
}

/// Namensvergleich: DER-gleich oder textuell gleich (RFC 4514, Groß-/Kleinschreibung und
/// Leerraum normalisiert) – fremde CMS kodieren den Aussteller mitunter mit anderen Stringtypen.
fn names_equal(a: &[u8], b: &[u8]) -> bool {
    if a == b {
        return true;
    }
    use der::Decode as _;
    let norm = |raw: &[u8]| {
        x509_cert::name::Name::from_der(raw)
            .ok()
            .map(|n| n.to_string().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" "))
    };
    matches!((norm(a), norm(b)), (Some(x), Some(y)) if x == y)
}

/// Prüft `SigningCertificate`/`SigningCertificateV2` (erstes ESSCertID[v2] = Signer, RFC 5035 §3).
fn check_signing_certificate(value: &Tlv<'_>, signer: &Certificate, v2: bool) -> Result<()> {
    let seq = value.expect(tag::SEQUENCE, "SigningCertificate")?;
    let certs = seq.reader().read_expect(tag::SEQUENCE, "SigningCertificate.certs")?;
    let first = certs.reader().read_expect(tag::SEQUENCE, "ESSCertID")?;
    let mut r = first.reader();
    let alg = if v2 && r.peek_tag() == Some(tag::SEQUENCE) {
        let a = parse_alg_id(&r.read()?)?;
        map_digest(&a.oid).ok_or_else(|| CmsError::UnsupportedAlgorithm(format!("ESSCertIDv2-Hash {}", a.oid)))?
    } else if v2 {
        DigestAlg::Sha256
    } else {
        DigestAlg::Sha1
    };
    let hash = r.read_expect(tag::OCTET_STRING, "certHash")?;
    if hash.content != signer.fingerprint(alg).value.as_slice() {
        return Err(CmsError::SigningCertificateMismatch);
    }
    if let Some(is) = r.read_optional(tag::SEQUENCE)? {
        let mut ir = is.reader();
        let names = ir.read_expect(tag::SEQUENCE, "GeneralNames")?;
        let serial = ir.read_expect(tag::INTEGER, "serialNumber")?;
        let (issuer, _) = signer.issuer_and_serial_raw()?;
        let issuer_ok = names
            .reader()
            .read_all()?
            .iter()
            .any(|gn| gn.tag == 0xA4 && gn.reader().read().is_ok_and(|name| names_equal(name.raw, issuer)));
        if !issuer_ok || integer_magnitude(serial.content) != integer_magnitude(signer.serial_bytes()) {
            return Err(CmsError::SigningCertificateMismatch);
        }
    }
    Ok(())
}

/// UTCTime/GeneralizedTime (DER und gängige BER-Formen: ohne Sekunden, mit Offset, Bruchteile).
///
/// Arbeitet auf Bytes und verlangt ASCII-Ziffern an allen Zahlenpositionen: Eingaben aus fremden
/// CMS sind nicht vertrauenswürdig (kein Slicing von `str` an Nicht-Zeichengrenzen, kein `+5` als
/// Zahl).
fn parse_time(t: &Tlv<'_>) -> Result<OffsetDateTime> {
    let b = t.content;
    let bad = || CmsError::Der(format!("Zeitangabe nicht lesbar: \"{}\"", b.escape_ascii()));
    // `n` ASCII-Ziffern ab `pos` als Zahl
    let num = |pos: usize, n: usize| -> Option<u16> {
        let d = b.get(pos..pos.checked_add(n)?)?;
        d.iter().all(u8::is_ascii_digit).then(|| d.iter().fold(0u16, |a, &c| a * 10 + u16::from(c - b'0')))
    };
    let (year, mut pos) = match t.tag {
        tag::UTC_TIME => {
            let yy = i32::from(num(0, 2).ok_or_else(bad)?);
            (if yy >= 50 { 1900 + yy } else { 2000 + yy }, 2)
        }
        tag::GENERALIZED_TIME => (i32::from(num(0, 4).ok_or_else(bad)?), 4),
        _ => return Err(CmsError::Der("signing-time ist keine Zeitangabe".into())),
    };
    let field = |p: usize| -> Result<u8> { num(p, 2).and_then(|v| u8::try_from(v).ok()).ok_or_else(bad) };
    let (month, day, hour, minute) = (field(pos)?, field(pos + 2)?, field(pos + 4)?, field(pos + 6)?);
    pos += 8;
    let second = if b.get(pos).is_some_and(u8::is_ascii_digit) {
        let s = field(pos)?;
        pos += 2;
        s
    } else {
        0
    };
    // Sekundenbruchteile (GeneralizedTime) ignorieren
    if matches!(b.get(pos), Some(b'.' | b',')) {
        pos += 1;
        let start = pos;
        while b.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        if pos == start {
            return Err(bad());
        }
    }
    let offset = match b.get(pos..).ok_or_else(bad)? {
        b"Z" => time::UtcOffset::UTC,
        [sign @ (b'+' | b'-'), rest @ ..] if rest.len() == 4 => {
            let (h, m) = (field(pos + 1)?, field(pos + 3)?);
            let (h, m) = (i8::try_from(h).map_err(|_| bad())?, i8::try_from(m).map_err(|_| bad())?);
            let sign = if *sign == b'-' { -1 } else { 1 };
            time::UtcOffset::from_hms(sign * h, sign * m, 0).map_err(|_| bad())?
        }
        _ => return Err(bad()),
    };
    let month = time::Month::try_from(month).map_err(|_| bad())?;
    let date = time::Date::from_calendar_date(year, month, day).map_err(|_| bad())?;
    let tod = time::Time::from_hms(hour, minute, second).map_err(|_| bad())?;
    Ok(time::PrimitiveDateTime::new(date, tod).assume_offset(offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tlv(tag: u8, s: &str) -> Vec<u8> {
        crate::der_util::tlv(tag, s.as_bytes())
    }

    #[test]
    fn time_parsing() {
        let utc = |s: &str| parse_time(&read_tlv(&tlv(tag::UTC_TIME, s)).unwrap());
        let gtime = |s: &str| parse_time(&read_tlv(&tlv(tag::GENERALIZED_TIME, s)).unwrap());
        assert_eq!(utc("240315183333Z").unwrap().unix_timestamp(), 1_710_527_613);
        assert_eq!(utc("2403151833Z").unwrap().unix_timestamp(), 1_710_527_580);
        assert_eq!(utc("240315193333+0100").unwrap().unix_timestamp(), 1_710_527_613);
        assert_eq!(utc("500101000000Z").unwrap().year(), 1950);
        assert_eq!(utc("491231235959Z").unwrap().year(), 2049);
        assert_eq!(gtime("20240315183333Z").unwrap().unix_timestamp(), 1_710_527_613);
        assert_eq!(gtime("20240315183333.123Z").unwrap().unix_timestamp(), 1_710_527_613);
        assert!(utc("241315183333Z").is_err());
        assert!(utc("2403151833").is_err());
        assert!(gtime("2024031518333Z").is_err());
        // negativer Offset
        assert_eq!(utc("240315173333-0100").unwrap().unix_timestamp(), 1_710_527_613);
    }

    #[test]
    fn time_parsing_rejects_hostile_input_without_panic() {
        // Review: Nicht-ASCII im Offset führte zu einem Panic beim Slicing (`o[1..3]`).
        let utc = |s: &[u8]| parse_time(&read_tlv(&crate::der_util::tlv(tag::UTC_TIME, s)).unwrap());
        let gtime = |s: &[u8]| parse_time(&read_tlv(&crate::der_util::tlv(tag::GENERALIZED_TIME, s)).unwrap());
        for s in [
            "2610141200+1\u{e9}1",
            "261014120000+0\u{20ac}",
            "2610141200\u{e9}",
            "\u{e9}10141200Z",
            "2610141200+1",
            "2610141200+01000",
            "2610141200+-500",
            "+61014120000Z",
            "26+1141200Z",
            "26101412+0Z",
            "261014120000",
            "",
            "2",
        ] {
            assert!(utc(s.as_bytes()).is_err(), "{s:?}");
        }
        assert!(utc(&[0xFF; 13]).is_err());
        assert!(gtime(b"20261014120000.Z").is_err());
        assert!(gtime("20261014120000.1\u{e9}".as_bytes()).is_err());
        assert!(gtime(b"20261014120000+2600").is_err());
        assert!(gtime(b"20261014120000+0160").is_err());
        // jedes Präfix und jede Ein-Byte-Ersetzung einer gültigen Zeit: Fehler oder Wert, nie Panic
        let good = b"20261014120000.123+0130";
        for i in 0..=good.len() {
            let _ = gtime(&good[..i]);
            for v in [0x00u8, b'+', b'-', b'.', b'Z', 0xC3, 0xE2, 0x80] {
                let mut m = good.to_vec();
                if let Some(c) = m.get_mut(i) {
                    *c = v;
                }
                let _ = gtime(&m);
                let _ = utc(&m[2..]);
            }
        }
    }

    #[test]
    fn trim_padding() {
        let cms = [0x30, 0x03, 0x02, 0x01, 0x01];
        let mut padded = cms.to_vec();
        padded.extend_from_slice(&[0u8; 10]);
        assert_eq!(trim_contents_padding(&padded).unwrap(), &cms);
        assert_eq!(trim_contents_padding(&cms).unwrap(), &cms);
        padded.push(1);
        assert!(trim_contents_padding(&padded).is_err());
        assert!(trim_contents_padding(&[]).is_err());
        assert!(trim_contents_padding(&[0u8; 8]).is_err());
        assert!(trim_contents_padding(&[0x30, 0x10, 0x00]).is_err());
        // BER, unbestimmte Länge
        let ber = [0x30, 0x80, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(trim_contents_padding(&ber).unwrap(), &ber[..7]);
    }

    #[test]
    fn pss_params_defaults_and_errors() {
        assert_eq!(parse_pss_params(None).unwrap(), (DigestAlg::Sha1, 20));
        let null = crate::der_util::null();
        assert_eq!(parse_pss_params(Some(&read_tlv(&null).unwrap())).unwrap(), (DigestAlg::Sha1, 20));
        use crate::der_util as d;
        let sha256 = d::alg_id("2.16.840.1.101.3.4.2.1", true);
        let mgf = d::seq(&[&d::oid(oids::MGF1), &sha256]);
        let params =
            d::seq(&[&d::context(0, &[&sha256]), &d::context(1, &[&mgf]), &d::context(2, &[&d::small_int(32)])]);
        assert_eq!(parse_pss_params(Some(&read_tlv(&params).unwrap())).unwrap(), (DigestAlg::Sha256, 32));
        // MGF1 mit anderem Hash wird abgelehnt
        let params = d::seq(&[&d::context(0, &[&sha256])]);
        assert!(parse_pss_params(Some(&read_tlv(&params).unwrap())).is_err());
        let params =
            d::seq(&[&d::context(0, &[&sha256]), &d::context(1, &[&mgf]), &d::context(3, &[&d::small_int(2)])]);
        assert!(parse_pss_params(Some(&read_tlv(&params).unwrap())).is_err());
    }
}
