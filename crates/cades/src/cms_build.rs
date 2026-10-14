//! Aufbau von SignedAttributes und SignedData (Modell A, Plan §3.7, §6.10).
//!
//! Kodierung wie PDF-AS/IAIK-CMS (`PAdESSignerKeystore`, docs/analyse/03 §5.1), nachgemessen an
//! den PDF-AS-Proben P1 (RSA) und P2/P3 (ECDSA) – die Golden-Tests reproduzieren deren
//! SignedAttributes **und** das komplette CMS byte-genau:
//!
//! | Element | [`CmsProfile::PdfasCompat`] | [`CmsProfile::PadesBB`] |
//! |---|---|---|
//! | signierte Attribute | content-type, ETSI mime-type `application/pdf`, signing-time, message-digest, signing-certificate-v2 (v1 bei SHA-1) | content-type, message-digest, signing-certificate-v2 (v1 bei SHA-1) |
//! | signing-time | UTCTime 1950–2049, sonst GeneralizedTime (IAIK `ChoiceOfTime`, RFC 5652 §11.3), UTC, ganze Sekunden | – |
//! | ESSCertIDv2 | `hashAlgorithm` fehlt bei SHA-256 (DEFAULT), sonst mit NULL-Parametern; `issuerSerial` immer | wie links, `hashAlgorithm` ohne Parameter (RFC 5754) |
//! | digestAlgorithm(s) | mit NULL-Parametern (IAIK, gemessen) | ohne Parameter (RFC 5754 §2) |
//! | signatureAlgorithm RSA | `sha256WithRSAEncryption` usw. **mit NULL** – IAIK schreibt den kombinierten OID (gemessen an P1), nicht `rsaEncryption` wie OpenSSL | ebenso (RFC 4055 verlangt NULL) |
//! | signatureAlgorithm ECDSA | `ecdsa-with-SHA256` usw. **mit NULL** (gemessen an P2/P3) | ohne Parameter (RFC 5758 §3.2) |
//!
//! SignedData: Version 1, detached (`eContent` fehlt), nur das Signerzertifikat (optional
//! Zwischenzertifikate, `SET OF` DER-sortiert), keine CRLs, genau ein SignerInfo v1 mit
//! IssuerAndSerialNumber (Bytes unverändert aus dem Zertifikat), keine unsignierten Attribute.

use time::{OffsetDateTime, UtcOffset};

use crate::der_util::{self as d, tag};
use crate::{CertChain, Certificate, CmsError, Digest, DigestAlg, EcCurve, KeyType, Result, SignatureAlg, oids};

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

impl CmsProfile {
    /// NULL-Parameter in Hash- und ECDSA-AlgorithmIdentifiern (IAIK-Verhalten).
    fn null_params(self) -> bool {
        matches!(self, CmsProfile::PdfasCompat)
    }
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
    ///
    /// Das Hashverfahren von `message_digest` bestimmt auch das ESS-Attribut: SHA-1 →
    /// signing-certificate (v1, ESSCertID), sonst signing-certificate-v2 (ESSCertIDv2) – wie
    /// `PKS:314-335`.
    pub fn build(&self) -> Result<SignedAttrs> {
        let alg = self.message_digest.alg;
        if self.message_digest.value.len() != alg.output_len() {
            return Err(CmsError::Der(format!(
                "message-digest hat {} statt {} Bytes ({})",
                self.message_digest.value.len(),
                alg.output_len(),
                alg.name()
            )));
        }
        let mut attrs = vec![attribute(oids::CONTENT_TYPE, &d::oid(oids::DATA))];
        if self.profile == CmsProfile::PdfasCompat {
            attrs.push(attribute(oids::ETSI_MIME_TYPE, &d::tlv(tag::UTF8_STRING, b"application/pdf")));
            attrs.push(attribute(oids::SIGNING_TIME, &encode_signing_time(self.signing_time.0)?));
        }
        attrs.push(attribute(oids::MESSAGE_DIGEST, &d::octet_string(&self.message_digest.value)));
        attrs.push(signing_certificate_attribute(self.profile, alg, &self.signer)?);
        Ok(SignedAttrs { der: d::set_of(attrs), digest_alg: alg, profile: self.profile })
    }
}

/// `Attribute ::= SEQUENCE { attrType OID, attrValues SET OF AttributeValue }` mit einem Wert.
fn attribute(oid: &str, value: &[u8]) -> Vec<u8> {
    d::seq(&[&d::oid(oid), &d::constructed(tag::SET, &[value])])
}

/// ESS signing-certificate(-v2) wie IAIK `ESSCertID(cert, true)` bzw. `ESSCertIDv2(alg, cert, true)`.
fn signing_certificate_attribute(profile: CmsProfile, alg: DigestAlg, cert: &Certificate) -> Result<Vec<u8>> {
    let (issuer, serial) = cert.issuer_and_serial_raw()?;
    // IssuerSerial ::= SEQUENCE { issuer GeneralNames, serialNumber CertificateSerialNumber }
    // GeneralNames ::= SEQUENCE OF GeneralName; directoryName [4] Name (EXPLICIT, Name ist CHOICE)
    let issuer_serial = d::seq(&[&d::seq(&[&d::context(4, &[issuer])]), serial]);
    let cert_hash = d::octet_string(&cert.fingerprint(alg).value);
    if alg == DigestAlg::Sha1 {
        // SigningCertificate ::= SEQUENCE { certs SEQUENCE OF ESSCertID }
        let ess_cert_id = d::seq(&[&cert_hash, &issuer_serial]);
        Ok(attribute(oids::SIGNING_CERTIFICATE, &d::seq(&[&d::seq(&[&ess_cert_id])])))
    } else {
        // ESSCertIDv2 ::= SEQUENCE { hashAlgorithm DEFAULT sha256, certHash, issuerSerial }
        let ess_cert_id = if alg == DigestAlg::Sha256 {
            d::seq(&[&cert_hash, &issuer_serial])
        } else {
            d::seq(&[&d::alg_id(alg.oid(), profile.null_params()), &cert_hash, &issuer_serial])
        };
        Ok(attribute(oids::SIGNING_CERTIFICATE_V2, &d::seq(&[&d::seq(&[&ess_cert_id])])))
    }
}

/// signing-time: UTCTime für 1950–2049, sonst GeneralizedTime (RFC 5652 §11.3, IAIK `ChoiceOfTime`).
fn encode_signing_time(t: OffsetDateTime) -> Result<Vec<u8>> {
    let u = t.to_offset(UtcOffset::UTC);
    let year = u.year();
    let (m, dd, h, mi, s) = (u8::from(u.month()), u.day(), u.hour(), u.minute(), u.second());
    if (1950..2050).contains(&year) {
        let txt = format!("{:02}{m:02}{dd:02}{h:02}{mi:02}{s:02}Z", year % 100);
        Ok(d::tlv(tag::UTC_TIME, txt.as_bytes()))
    } else if (0..=9999).contains(&year) {
        let txt = format!("{year:04}{m:02}{dd:02}{h:02}{mi:02}{s:02}Z");
        Ok(d::tlv(tag::GENERALIZED_TIME, txt.as_bytes()))
    } else {
        Err(CmsError::Der(format!("signing-time außerhalb des darstellbaren Bereichs (Jahr {year})")))
    }
}

/// Fertige SignedAttributes.
#[derive(Clone, PartialEq, Eq)]
pub struct SignedAttrs {
    /// DER als `SET OF Attribute` (Tag `0x31`) – genau diese Bytes werden signiert.
    pub der: Vec<u8>,
    /// Hashverfahren von message-digest.
    pub digest_alg: DigestAlg,
    /// Profil.
    pub profile: CmsProfile,
}

impl std::fmt::Debug for SignedAttrs {
    /// Ohne DER: Es enthält Aussteller und Seriennummer des Signerzertifikats (Regel 9).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fp = DigestAlg::Sha256.digest(&self.der).to_hex();
        f.debug_struct("SignedAttrs")
            .field("der_len", &self.der.len())
            .field("sha256", &&fp[..16])
            .field("digest_alg", &self.digest_alg)
            .field("profile", &self.profile)
            .finish()
    }
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
        let mut out = self.der.clone();
        if let Some(first) = out.first_mut() {
            *first = 0xA0;
        }
        out
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
    ///
    /// `fixed` muss gerade Länge haben (je Hälfte `r` bzw. `s`, big-endian); `r` und `s` dürfen
    /// nicht 0 sein.
    pub fn ecdsa_from_fixed(alg: SignatureAlg, fixed: &[u8]) -> Result<Self> {
        if !matches!(alg, SignatureAlg::Ecdsa(_)) {
            return Err(CmsError::UnsupportedAlgorithm(format!("{alg:?} ist kein ECDSA-Verfahren")));
        }
        if fixed.is_empty() || !fixed.len().is_multiple_of(2) || fixed.len() > 2 * 66 {
            return Err(CmsError::InvalidSignature);
        }
        let (r, s) = fixed.split_at(fixed.len() / 2);
        if r.iter().all(|&b| b == 0) || s.iter().all(|&b| b == 0) {
            return Err(CmsError::InvalidSignature);
        }
        Ok(Self { alg, value: d::seq(&[&d::integer_unsigned(r), &d::integer_unsigned(s)]) })
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
///
/// Berechnet wird durch Montage mit Platzhaltern maximaler Länge (signing-time als
/// GeneralizedTime, Signaturwert mit längster Kodierung) – das Ergebnis ist damit exakt die
/// größtmögliche Länge eines [`assemble`]-Ergebnisses für diese Eingaben.
pub fn estimate_cms_len(
    chain: &CertChain,
    profile: CmsProfile,
    alg: SignatureAlg,
    include_intermediates: bool,
) -> Result<usize> {
    let key = chain.signer.key_type()?;
    let value = match (alg, &key) {
        (SignatureAlg::RsaPkcs1v15(_), KeyType::Rsa { bits }) => vec![0xFF; bits.div_ceil(8) as usize],
        (SignatureAlg::Ecdsa(_), KeyType::Ec { curve }) => {
            let bits = ec_order_bits(curve).ok_or_else(|| {
                CmsError::UnsupportedAlgorithm(format!("Kurve unbekannt: {}", crate::signer::describe_key(&key)))
            })?;
            let int_len = bits.div_ceil(8) as usize + usize::from(bits % 8 == 0);
            let mut int = vec![0xFF; int_len];
            int[0] = if bits % 8 == 0 { 0x00 } else { 0x01 };
            let int = d::tlv(tag::INTEGER, &int);
            d::seq(&[&int, &int])
        }
        _ => {
            return Err(CmsError::UnsupportedAlgorithm(format!(
                "{alg:?} passt nicht zum Schlüssel {}",
                crate::signer::describe_key(&key)
            )));
        }
    };
    let digest = Digest { alg: alg.digest(), value: vec![0xFF; alg.digest().output_len()] };
    // 2050 → GeneralizedTime (2 Bytes länger als UTCTime)
    let time = OffsetDateTime::from_unix_timestamp(2_556_143_999).map_err(|e| CmsError::Der(e.to_string()))?;
    let attrs = SignedAttrsBuilder::new(profile, &digest, &chain.signer, SigningTime(time)).build()?;
    let cms =
        assemble(&attrs, &RawSignature { alg, value }, chain, &AssembleOptions { profile, include_intermediates })?;
    Ok(cms.len())
}

/// Bitlänge der Gruppenordnung (bestimmt die maximale Länge von `r`, `s`).
fn ec_order_bits(curve: &EcCurve) -> Option<u32> {
    match curve {
        EcCurve::P256 | EcCurve::BrainpoolP256r1 => Some(256),
        EcCurve::P384 | EcCurve::BrainpoolP384r1 => Some(384),
        EcCurve::P521 => Some(521),
        EcCurve::BrainpoolP512r1 => Some(512),
        // Näherung über die Feldgröße (Hasse: Ordnung ≤ Feldgröße + 1 Bit)
        EcCurve::Other(oid) => oids::other_curve_field_bits(oid).map(|b| b + 1),
    }
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
///
/// Prüft die Konsistenz der Eingaben: Hashverfahren von Attributen und Signatur gleich, Profil von
/// Attributen und Optionen gleich, Signaturverfahren passend zum Signerschlüssel (RSA PKCS#1 v1.5
/// bzw. ECDSA; PSS nur zur Prüfung von Vorsignaturen), ECDSA-Wert als DER `SEQUENCE { r, s }`.
/// Ein RSA-Wert mit fehlenden führenden Nullen wird auf die Modullänge aufgefüllt (I2OSP; OpenSSL
/// lehnt kürzere Werte ab).
pub fn assemble(attrs: &SignedAttrs, sig: &RawSignature, chain: &CertChain, opts: &AssembleOptions) -> Result<Vec<u8>> {
    if attrs.profile != opts.profile {
        return Err(CmsError::UnsupportedAlgorithm(format!(
            "Profil der Attribute ({:?}) ≠ Montageprofil ({:?})",
            attrs.profile, opts.profile
        )));
    }
    if sig.alg.digest() != attrs.digest_alg {
        return Err(CmsError::UnsupportedAlgorithm(format!(
            "Signatur mit {} über Attribute mit {}",
            sig.alg.digest().name(),
            attrs.digest_alg.name()
        )));
    }
    let key = chain.signer.key_type()?;
    let null = opts.profile.null_params();
    let (sig_alg_id, value) = match (sig.alg, &key) {
        (SignatureAlg::RsaPkcs1v15(dg), KeyType::Rsa { bits }) => {
            let k = bits.div_ceil(8) as usize;
            if sig.value.len() > k || sig.value.is_empty() {
                return Err(CmsError::InvalidSignature);
            }
            let mut v = vec![0u8; k - sig.value.len()];
            v.extend_from_slice(&sig.value);
            (d::alg_id(SignatureAlg::RsaPkcs1v15(dg).oid(), true), v)
        }
        (SignatureAlg::Ecdsa(dg), KeyType::Ec { .. }) => {
            check_ecdsa_der(&sig.value)?;
            (d::alg_id(SignatureAlg::Ecdsa(dg).oid(), null), sig.value.clone())
        }
        (SignatureAlg::RsaPss { .. }, _) => {
            return Err(CmsError::UnsupportedAlgorithm("RSASSA-PSS nur zur Prüfung von Vorsignaturen".into()));
        }
        (alg, key) => {
            return Err(CmsError::UnsupportedAlgorithm(format!(
                "{alg:?} passt nicht zum Schlüssel {}",
                crate::signer::describe_key(key)
            )));
        }
    };

    let digest_alg_id = d::alg_id(attrs.digest_alg.oid(), null);
    let (issuer, serial) = chain.signer.issuer_and_serial_raw()?;
    let sid = d::seq(&[issuer, serial]);
    let signer_info = d::seq(&[
        &d::small_int(1),
        &sid,
        &digest_alg_id,
        &attrs.implicit_tagged_der(),
        &sig_alg_id,
        &d::octet_string(&value),
    ]);

    let mut certs: Vec<Vec<u8>> = vec![chain.signer.as_der().to_vec()];
    if opts.include_intermediates {
        for c in &chain.intermediates {
            if !certs.iter().any(|x| x.as_slice() == c.as_der()) {
                certs.push(c.as_der().to_vec());
            }
        }
    }
    d::der_sort(&mut certs);
    let cert_refs: Vec<&[u8]> = certs.iter().map(Vec::as_slice).collect();
    // certificates [0] IMPLICIT CertificateSet (SET OF, DER-sortiert)
    let certificates = d::context(0, &cert_refs);

    let signed_data = d::seq(&[
        &d::small_int(1),
        &d::constructed(tag::SET, &[&digest_alg_id]),
        &d::seq(&[&d::oid(oids::DATA)]),
        &certificates,
        &d::constructed(tag::SET, &[&signer_info]),
    ]);
    Ok(d::seq(&[&d::oid(oids::SIGNED_DATA), &d::context(0, &[&signed_data])]))
}

/// ECDSA-Signaturwert muss DER `SEQUENCE { INTEGER, INTEGER }` ohne Restdaten sein.
fn check_ecdsa_der(v: &[u8]) -> Result<()> {
    let seq = d::read_tlv(v).map_err(|_| CmsError::InvalidSignature)?;
    if seq.tag != tag::SEQUENCE || seq.raw.len() != v.len() || seq.indefinite {
        return Err(CmsError::InvalidSignature);
    }
    let mut r = seq.reader();
    for _ in 0..2 {
        let i = r.read().map_err(|_| CmsError::InvalidSignature)?;
        if i.tag != tag::INTEGER || i.content.is_empty() {
            return Err(CmsError::InvalidSignature);
        }
    }
    if !r.is_empty() {
        return Err(CmsError::InvalidSignature);
    }
    Ok(())
}

/// Algorithmuswahl wie PDF-AS (docs/analyse/03 §5.1) mit bewussten Abweichungen (Plan §4.9):
/// RSA → sha256WithRSA; EC ≥ 512 → SHA-512; EC ≥ 256 (auch P-384) → SHA-256;
/// EC < 256 und DSA → [`crate::CmsError::UnsupportedAlgorithm`].
///
/// Feldgröße wie Java `ECField.getFieldSize()`: P-521 = 521, brainpoolP512r1 = 512; für Kurven, die
/// [`EcCurve`] nur als OID führt, aus einer Tabelle (secp256k1 = 256, secp224r1 = 224 …).
/// Die exakte PDF-AS-Abbildung ohne die Abweichungen liefert [`pdfas_algorithm`].
pub fn select_algorithm(key: &KeyType) -> Result<SignatureAlg> {
    let alg = pdfas_algorithm(key)?;
    if alg.digest() == DigestAlg::Sha1 {
        return Err(CmsError::UnsupportedAlgorithm(format!(
            "{} (Feldgröße < 256 Bit): PDF-AS würde ecdsa-with-SHA1 verwenden – für neue Signaturen abgelehnt (Plan §4.9)",
            crate::signer::describe_key(key)
        )));
    }
    Ok(alg)
}

/// Exakte PDF-AS-Regel `CertificateUtils.getAlgorithmIDs` (`CU:34-75`) – ohne die Abweichungen von
/// [`select_algorithm`]: EC mit Feldgröße < 256 → `ecdsa-with-SHA1`. Für Diagnose und das
/// Kompatibilitätsregister; DSA (`dsaWithSHA256` bei PDF-AS) ist hier nicht darstellbar und ergibt
/// wie unbekannte Schlüssel [`CmsError::UnsupportedAlgorithm`].
pub fn pdfas_algorithm(key: &KeyType) -> Result<SignatureAlg> {
    match key {
        KeyType::Rsa { .. } => Ok(SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256)),
        KeyType::Ec { curve } => {
            let bits = curve
                .field_bits()
                .or_else(|| match curve {
                    EcCurve::Other(oid) => oids::other_curve_field_bits(oid),
                    _ => None,
                })
                .ok_or_else(|| {
                    CmsError::UnsupportedAlgorithm(format!(
                        "Feldgröße der Kurve unbekannt: {}",
                        crate::signer::describe_key(key)
                    ))
                })?;
            Ok(SignatureAlg::Ecdsa(if bits >= 512 {
                DigestAlg::Sha512
            } else if bits >= 256 {
                DigestAlg::Sha256
            } else {
                DigestAlg::Sha1
            }))
        }
        KeyType::Dsa => {
            Err(CmsError::UnsupportedAlgorithm("DSA (PDF-AS: dsaWithSHA256) wird nicht unterstützt (Plan §4.9)".into()))
        }
        KeyType::Other(name) => {
            Err(CmsError::UnsupportedAlgorithm(format!("Schlüsselalgorithmus {name} nicht unterstützt")))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_time_encoding_switches_at_2050() {
        let t = OffsetDateTime::from_unix_timestamp(1_710_527_613).unwrap(); // 2024-03-15T18:33:33Z
        assert_eq!(encode_signing_time(t).unwrap(), d::tlv(tag::UTC_TIME, b"240315183333Z"));
        // Ortszeit mit Offset → UTC
        let local = t.to_offset(UtcOffset::from_hms(1, 0, 0).unwrap());
        assert_eq!(encode_signing_time(local).unwrap(), encode_signing_time(t).unwrap());
        let t2049 = OffsetDateTime::from_unix_timestamp(2_524_607_999).unwrap(); // 2049-12-31T23:59:59Z
        assert_eq!(encode_signing_time(t2049).unwrap(), d::tlv(tag::UTC_TIME, b"491231235959Z"));
        let t2050 = OffsetDateTime::from_unix_timestamp(2_524_608_000).unwrap();
        assert_eq!(encode_signing_time(t2050).unwrap(), d::tlv(tag::GENERALIZED_TIME, b"20500101000000Z"));
        let t1949 = OffsetDateTime::from_unix_timestamp(-631_152_001).unwrap(); // 1949-12-31T23:59:59Z
        assert_eq!(encode_signing_time(t1949).unwrap(), d::tlv(tag::GENERALIZED_TIME, b"19491231235959Z"));
        // Sekundenbruchteile entfallen
        let frac = t + time::Duration::milliseconds(999);
        assert_eq!(encode_signing_time(frac).unwrap(), encode_signing_time(t).unwrap());
    }

    #[test]
    fn select_algorithm_follows_pdfas_with_documented_deviations() {
        use EcCurve::*;
        let ec = |c: EcCurve| KeyType::Ec { curve: c };
        assert_eq!(
            select_algorithm(&KeyType::Rsa { bits: 1024 }).unwrap(),
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256)
        );
        assert_eq!(
            select_algorithm(&KeyType::Rsa { bits: 4096 }).unwrap(),
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256)
        );
        assert_eq!(select_algorithm(&ec(P256)).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha256));
        // P-384 → SHA-256 (PDF-AS-Verhalten, bewusst übernommen)
        assert_eq!(select_algorithm(&ec(P384)).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha256));
        assert_eq!(select_algorithm(&ec(P521)).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha512));
        assert_eq!(select_algorithm(&ec(BrainpoolP256r1)).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha256));
        assert_eq!(select_algorithm(&ec(BrainpoolP384r1)).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha256));
        assert_eq!(select_algorithm(&ec(BrainpoolP512r1)).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha512));
        assert_eq!(
            select_algorithm(&ec(Other("1.3.132.0.10".into()))).unwrap(),
            SignatureAlg::Ecdsa(DigestAlg::Sha256)
        );
        assert_eq!(
            select_algorithm(&ec(Other("1.3.132.0.38".into()))).unwrap(),
            SignatureAlg::Ecdsa(DigestAlg::Sha512)
        );
        // Abweichungen: EC < 256 und DSA abgelehnt
        let p224 = ec(Other("1.3.132.0.33".into()));
        assert!(matches!(select_algorithm(&p224), Err(CmsError::UnsupportedAlgorithm(_))));
        assert_eq!(pdfas_algorithm(&p224).unwrap(), SignatureAlg::Ecdsa(DigestAlg::Sha1));
        assert!(matches!(select_algorithm(&KeyType::Dsa), Err(CmsError::UnsupportedAlgorithm(_))));
        assert!(select_algorithm(&KeyType::Other("Ed25519".into())).is_err());
        assert!(select_algorithm(&ec(Other("1.2.3.4".into()))).is_err());
        assert!(select_algorithm(&ec(Other("explicit".into()))).is_err());
    }

    #[test]
    fn ecdsa_from_fixed() {
        let mut fixed = vec![0u8; 64];
        fixed[31] = 1;
        fixed[32] = 0x80;
        let s = RawSignature::ecdsa_from_fixed(SignatureAlg::Ecdsa(DigestAlg::Sha256), &fixed).unwrap();
        let mut expected_s = vec![0x00, 0x80];
        expected_s.extend_from_slice(&[0u8; 31]);
        let expected = d::seq(&[&d::tlv(tag::INTEGER, &[1]), &d::tlv(tag::INTEGER, &expected_s)]);
        assert_eq!(s.value, expected);
        assert!(RawSignature::ecdsa_from_fixed(SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256), &fixed).is_err());
        assert!(RawSignature::ecdsa_from_fixed(SignatureAlg::Ecdsa(DigestAlg::Sha256), &fixed[..63]).is_err());
        assert!(RawSignature::ecdsa_from_fixed(SignatureAlg::Ecdsa(DigestAlg::Sha256), &[0u8; 64]).is_err());
        assert!(RawSignature::ecdsa_from_fixed(SignatureAlg::Ecdsa(DigestAlg::Sha256), &[]).is_err());
    }
}
