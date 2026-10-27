//! Lokale Signatur- und Prüfprimitive: RSA PKCS#1 v1.5 (Signieren und Prüfen), RSASSA-PSS (nur
//! Prüfen) und ECDSA P-256/P-384/P-521 (Signieren und Prüfen, DER-Signaturwert).
//!
//! ECDSA über einen fertigen Hash („prehash“): Ist der Hash kürzer als die Feldgröße, wird er links
//! mit Nullen aufgefüllt. Das entspricht `bits2int` aus FIPS 186-5/RFC 6979 und erlaubt jede
//! Kombination aus Kurve und SHA-2 (z. B. P-521 mit SHA-256; `ecdsa` 0.16 lehnt Hashes unter der
//! halben Feldgröße sonst ab). Längere Hashes kürzt `ecdsa` selbst auf die Feldgröße.

use p256::pkcs8::DecodePrivateKey as _; // pkcs8 0.11 (EC, Folgegeneration)
use rsa::pkcs8::DecodePrivateKey as _; // pkcs8 0.10 (RSA)
use rsa::traits::PublicKeyParts as _;
use rsa::{Pkcs1v15Sign, Pss, RsaPrivateKey, RsaPublicKey};
use signature::hazmat::{PrehashSigner as _, PrehashVerifier as _};

use crate::der_util::{integer_magnitude, read_tlv, tag};
use crate::{
    CertChain, Certificate, CmsError, Digest, DigestAlg, EcCurve, KeyType, RawSignature, RawSigner, Result,
    SignatureAlg, select_algorithm,
};

/// Schlüsselmaterial (wird beim Drop von `rsa`/`ecdsa` selbst genullt).
enum KeyMaterial {
    Rsa(Box<RsaPrivateKey>),
    P256(p256::ecdsa::SigningKey),
    P384(p384::ecdsa::SigningKey),
    P521(p521::ecdsa::SigningKey),
}

/// Lokaler Signierer aus einem PKCS#8-Schlüssel und der zugehörigen Kette (Keystore, Tests).
///
/// Prüft beim Erzeugen, dass der private Schlüssel zum Signerzertifikat passt, und wählt das
/// Verfahren mit [`select_algorithm`] (PDF-AS-Regeln). `Debug` zeigt nur Schlüsseltyp, Verfahren
/// und Zertifikats-Fingerprint (Regel 9).
pub struct LocalSigner {
    chain: CertChain,
    algorithm: SignatureAlg,
    key: KeyMaterial,
}

impl std::fmt::Debug for LocalSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = match &self.key {
            KeyMaterial::Rsa(k) => format!("RSA-{}", k.n().bits()),
            KeyMaterial::P256(_) => "P-256".into(),
            KeyMaterial::P384(_) => "P-384".into(),
            KeyMaterial::P521(_) => "P-521".into(),
        };
        f.debug_struct("LocalSigner")
            .field("key", &key)
            .field("algorithm", &self.algorithm)
            .field("signer", &self.chain.signer)
            .finish_non_exhaustive()
    }
}

impl LocalSigner {
    /// Aus einem unverschlüsselten PKCS#8-`PrivateKeyInfo` (DER) und der Kette.
    ///
    /// Fehler: Schlüsseltyp nicht unterstützt (DSA, Brainpool, Ed25519, EC < 256 Bit …) →
    /// [`CmsError::UnsupportedAlgorithm`]; Schlüssel unlesbar → [`CmsError::Key`]; passt nicht zum
    /// Zertifikat → [`CmsError::KeyMismatch`].
    pub fn from_pkcs8_der(pkcs8: &[u8], chain: CertChain) -> Result<Self> {
        let key_type = chain.signer.key_type()?;
        check_pkcs8_matches_certificate(pkcs8, &key_type)?;
        let algorithm = select_algorithm(&key_type)?;
        let key = match &key_type {
            KeyType::Rsa { .. } => {
                let key = RsaPrivateKey::from_pkcs8_der(pkcs8).map_err(|e| key_err(format!("RSA-Schlüssel: {e}")))?;
                key.validate().map_err(|e| key_err(format!("RSA-Schlüssel ungültig: {e}")))?;
                let cert_pub = rsa_public_key(&chain.signer)?;
                if RsaPublicKey::from(&key) != cert_pub {
                    return Err(key_mismatch());
                }
                KeyMaterial::Rsa(Box::new(key))
            }
            KeyType::Ec { curve: EcCurve::P256 } => {
                let sk =
                    p256::SecretKey::from_pkcs8_der(pkcs8).map_err(|e| key_err(format!("P-256-Schlüssel: {e}")))?;
                let key = p256::ecdsa::SigningKey::from(sk);
                let cert_vk = p256::ecdsa::VerifyingKey::from_sec1_bytes(chain.signer.public_key_bits()?)
                    .map_err(|_| CmsError::Certificate("öffentlicher P-256-Schlüssel unlesbar".into()))?;
                if *key.verifying_key() != cert_vk {
                    return Err(key_mismatch());
                }
                KeyMaterial::P256(key)
            }
            KeyType::Ec { curve: EcCurve::P384 } => {
                let sk =
                    p384::SecretKey::from_pkcs8_der(pkcs8).map_err(|e| key_err(format!("P-384-Schlüssel: {e}")))?;
                let key = p384::ecdsa::SigningKey::from(sk);
                let cert_vk = p384::ecdsa::VerifyingKey::from_sec1_bytes(chain.signer.public_key_bits()?)
                    .map_err(|_| CmsError::Certificate("öffentlicher P-384-Schlüssel unlesbar".into()))?;
                if *key.verifying_key() != cert_vk {
                    return Err(key_mismatch());
                }
                KeyMaterial::P384(key)
            }
            KeyType::Ec { curve: EcCurve::P521 } => {
                let sk =
                    p521::SecretKey::from_pkcs8_der(pkcs8).map_err(|e| key_err(format!("P-521-Schlüssel: {e}")))?;
                let key = p521::ecdsa::SigningKey::from(sk);
                let cert_vk = p521::ecdsa::VerifyingKey::from_sec1_bytes(chain.signer.public_key_bits()?)
                    .map_err(|_| CmsError::Certificate("öffentlicher P-521-Schlüssel unlesbar".into()))?;
                if *key.verifying_key() != cert_vk {
                    return Err(key_mismatch());
                }
                KeyMaterial::P521(key)
            }
            other => {
                return Err(CmsError::UnsupportedAlgorithm(format!(
                    "Signieren mit {} nicht unterstützt (nur RSA, P-256, P-384, P-521)",
                    describe_key(other)
                )));
            }
        };
        Ok(Self { chain, algorithm, key })
    }

    /// Schlüsseltyp.
    pub fn key_type(&self) -> KeyType {
        match &self.key {
            KeyMaterial::Rsa(k) => KeyType::Rsa { bits: k.n().bits() as u32 },
            KeyMaterial::P256(_) => KeyType::Ec { curve: EcCurve::P256 },
            KeyMaterial::P384(_) => KeyType::Ec { curve: EcCurve::P384 },
            KeyMaterial::P521(_) => KeyType::Ec { curve: EcCurve::P521 },
        }
    }
}

impl RawSigner for LocalSigner {
    fn chain(&self) -> &CertChain {
        &self.chain
    }

    fn algorithm(&self) -> SignatureAlg {
        self.algorithm
    }

    /// Signiert mit genau `digest.alg` (RSA: DigestInfo dieses Verfahrens, PKCS#1 v1.5 mit Blinding;
    /// ECDSA: RFC 6979, DER `SEQUENCE { r, s }`). SHA-1 wird für neue Signaturen abgelehnt.
    fn sign_digest(&self, digest: &Digest) -> Result<RawSignature> {
        check_digest_len(digest)?;
        if digest.alg == DigestAlg::Sha1 {
            return Err(CmsError::UnsupportedAlgorithm("SHA-1 für neue Signaturen abgelehnt".into()));
        }
        match &self.key {
            KeyMaterial::Rsa(key) => {
                let value = key
                    .sign_with_rng(&mut rand_core::OsRng, pkcs1v15_padding(digest.alg), &digest.value)
                    .map_err(|e| key_err(format!("RSA-Signatur fehlgeschlagen: {e}")))?;
                Ok(RawSignature { alg: SignatureAlg::RsaPkcs1v15(digest.alg), value })
            }
            KeyMaterial::P256(key) => {
                let sig: p256::ecdsa::Signature = key
                    .sign_prehash(&pad_prehash(&digest.value, 32))
                    .map_err(|_| key_err("ECDSA-Signatur fehlgeschlagen"))?;
                Ok(RawSignature { alg: SignatureAlg::Ecdsa(digest.alg), value: sig.to_der().as_bytes().to_vec() })
            }
            KeyMaterial::P384(key) => {
                let sig: p384::ecdsa::Signature = key
                    .sign_prehash(&pad_prehash(&digest.value, 48))
                    .map_err(|_| key_err("ECDSA-Signatur fehlgeschlagen"))?;
                Ok(RawSignature { alg: SignatureAlg::Ecdsa(digest.alg), value: sig.to_der().as_bytes().to_vec() })
            }
            KeyMaterial::P521(key) => {
                let sig: p521::ecdsa::Signature = key
                    .sign_prehash(&pad_prehash(&digest.value, 66))
                    .map_err(|_| key_err("ECDSA-Signatur fehlgeschlagen"))?;
                Ok(RawSignature { alg: SignatureAlg::Ecdsa(digest.alg), value: sig.to_der().as_bytes().to_vec() })
            }
        }
    }
}

/// Prüft eine Rohsignatur über einen Hash mit dem öffentlichen Schlüssel des Zertifikats
/// (Selbstprüfung nach dem Signieren wie PDF-AS `PKS:192-193`, Prüfung von CSC-Ergebnissen).
pub fn verify_raw(cert: &Certificate, digest: &Digest, signature: &RawSignature) -> Result<()> {
    if signature.alg.digest() != digest.alg {
        return Err(CmsError::UnsupportedAlgorithm(format!(
            "Signaturverfahren {:?} passt nicht zum Hash {}",
            signature.alg,
            digest.alg.name()
        )));
    }
    check_digest_len(digest)?;
    verify_with_cert(cert, signature.alg, &digest.value, &signature.value)
}

/// Kern der Signaturprüfung (auch für [`crate::verify_detached`]).
pub(crate) fn verify_with_cert(cert: &Certificate, alg: SignatureAlg, digest: &[u8], sig: &[u8]) -> Result<()> {
    match alg {
        SignatureAlg::RsaPkcs1v15(d) => {
            let key = rsa_public_key(cert)?;
            let sig = pad_rsa_signature(&key, sig)?;
            key.verify(pkcs1v15_padding(d), digest, &sig).map_err(|_| CmsError::InvalidSignature)
        }
        SignatureAlg::RsaPss { digest: d, salt_len } => {
            let key = rsa_public_key(cert)?;
            let sig = pad_rsa_signature(&key, sig)?;
            let salt = salt_len as usize;
            let padding = match d {
                DigestAlg::Sha1 => Pss::new_with_salt::<sha1::Sha1>(salt),
                DigestAlg::Sha256 => Pss::new_with_salt::<sha2::Sha256>(salt),
                DigestAlg::Sha384 => Pss::new_with_salt::<sha2::Sha384>(salt),
                DigestAlg::Sha512 => Pss::new_with_salt::<sha2::Sha512>(salt),
            };
            key.verify(padding, digest, &sig).map_err(|_| CmsError::InvalidSignature)
        }
        SignatureAlg::Ecdsa(_) => {
            let curve = match cert.key_type()? {
                KeyType::Ec { curve } => curve,
                other => {
                    return Err(CmsError::UnsupportedAlgorithm(format!(
                        "ECDSA-Signatur, aber Zertifikat mit {}",
                        describe_key(&other)
                    )));
                }
            };
            let point = cert.public_key_bits()?;
            match curve {
                EcCurve::P256 => {
                    let vk = p256::ecdsa::VerifyingKey::from_sec1_bytes(point)
                        .map_err(|_| CmsError::Certificate("öffentlicher P-256-Schlüssel unlesbar".into()))?;
                    let rs = ecdsa_der_to_fixed(sig, 32)?;
                    let s = p256::ecdsa::Signature::from_slice(&rs).map_err(|_| CmsError::InvalidSignature)?;
                    vk.verify_prehash(&pad_prehash(digest, 32), &s).map_err(|_| CmsError::InvalidSignature)
                }
                EcCurve::P384 => {
                    let vk = p384::ecdsa::VerifyingKey::from_sec1_bytes(point)
                        .map_err(|_| CmsError::Certificate("öffentlicher P-384-Schlüssel unlesbar".into()))?;
                    let rs = ecdsa_der_to_fixed(sig, 48)?;
                    let s = p384::ecdsa::Signature::from_slice(&rs).map_err(|_| CmsError::InvalidSignature)?;
                    vk.verify_prehash(&pad_prehash(digest, 48), &s).map_err(|_| CmsError::InvalidSignature)
                }
                EcCurve::P521 => {
                    let vk = p521::ecdsa::VerifyingKey::from_sec1_bytes(point)
                        .map_err(|_| CmsError::Certificate("öffentlicher P-521-Schlüssel unlesbar".into()))?;
                    let rs = ecdsa_der_to_fixed(sig, 66)?;
                    let s = p521::ecdsa::Signature::from_slice(&rs).map_err(|_| CmsError::InvalidSignature)?;
                    vk.verify_prehash(&pad_prehash(digest, 66), &s).map_err(|_| CmsError::InvalidSignature)
                }
                other => Err(CmsError::UnsupportedAlgorithm(format!(
                    "ECDSA auf {} nicht unterstützt",
                    describe_key(&KeyType::Ec { curve: other })
                ))),
            }
        }
    }
}

/// Größte unterstützte RSA-Modullänge. `rsa` begrenzt öffentliche Schlüssel aus DER auf 4096 Bit
/// (`RsaPublicKey::MAX_SIZE`); PDF-AS/Java kennt keine solche Grenze (RSA-8192-Keystores und
/// Vorsignaturen kommen vor). 16384 Bit hält den Prüfaufwand bösartiger CMS in Grenzen (der
/// Exponent ist von `rsa` ohnehin auf ≤ 2³³ begrenzt).
const MAX_RSA_BITS: usize = 16384;

/// Öffentlicher RSA-Schlüssel (SPKI `rsaEncryption` oder `RSASSA-PSS`).
fn rsa_public_key(cert: &Certificate) -> Result<RsaPublicKey> {
    use der::Decode as _;
    match cert.key_type()? {
        KeyType::Rsa { .. } => {}
        KeyType::Other(s) if s == "RSASSA-PSS" => {}
        other => {
            return Err(CmsError::UnsupportedAlgorithm(format!(
                "RSA-Signatur, aber Zertifikat mit {}",
                describe_key(&other)
            )));
        }
    }
    let unreadable =
        |e: &dyn std::fmt::Display| CmsError::Certificate(format!("öffentlicher RSA-Schlüssel unlesbar: {e}"));
    let pk = rsa::pkcs1::RsaPublicKey::from_der(cert.public_key_bits()?).map_err(|e| unreadable(&e))?;
    let n = rsa::BigUint::from_bytes_be(pk.modulus.as_bytes());
    let e = rsa::BigUint::from_bytes_be(pk.public_exponent.as_bytes());
    RsaPublicKey::new_with_max_size(n, e, MAX_RSA_BITS).map_err(|e| unreadable(&e))
}

/// Algorithmus des PKCS#8-`PrivateKeyInfo` muss zur SPKI des Zertifikats passen (RSA ↔ RSA, EC mit
/// derselben Kurve). Sonst [`CmsError::KeyMismatch`] statt eines irreführenden Lesefehlers.
fn check_pkcs8_matches_certificate(pkcs8: &[u8], key_type: &KeyType) -> Result<()> {
    use crate::der_util::Reader;
    use crate::oids;
    // PrivateKeyInfo ::= SEQUENCE { version, privateKeyAlgorithm AlgorithmIdentifier, privateKey, … }
    let info = (|| {
        let seq = read_tlv(pkcs8)?.expect(tag::SEQUENCE, "PrivateKeyInfo")?;
        let mut r: Reader<'_> = seq.reader();
        r.read_expect(tag::INTEGER, "version")?;
        let alg = r.read_expect(tag::SEQUENCE, "privateKeyAlgorithm")?;
        let mut ar = alg.reader();
        let oid = ar.read()?.oid_string()?;
        let param = match ar.read_optional(tag::OID)? {
            Some(p) => Some(p.oid_string()?),
            None => None,
        };
        Ok::<_, CmsError>((oid, param))
    })();
    let Ok((oid, curve)) = info else {
        return Err(key_err("PKCS#8-Struktur unlesbar"));
    };
    let ok = match key_type {
        KeyType::Rsa { .. } => oid == oids::RSA_ENCRYPTION,
        KeyType::Ec { curve: c } => {
            let want = match c {
                EcCurve::P256 => Some(oids::P256),
                EcCurve::P384 => Some(oids::P384),
                EcCurve::P521 => Some(oids::P521),
                _ => None,
            };
            // Kurve fehlt im AlgorithmIdentifier (steht dann in ECPrivateKey) → beim Parsen geprüft
            oid == oids::EC_PUBLIC_KEY && (curve.is_none() || want.is_none() || curve.as_deref() == want)
        }
        _ => true,
    };
    if ok { Ok(()) } else { Err(key_mismatch()) }
}

/// RSA-Signaturwerte mit fehlenden führenden Nullen auf die Modullänge auffüllen (I2OSP).
fn pad_rsa_signature(key: &RsaPublicKey, sig: &[u8]) -> Result<Vec<u8>> {
    let k = key.size();
    if sig.len() > k {
        return Err(CmsError::InvalidSignature);
    }
    let mut out = vec![0u8; k - sig.len()];
    out.extend_from_slice(sig);
    Ok(out)
}

fn pkcs1v15_padding(d: DigestAlg) -> Pkcs1v15Sign {
    match d {
        DigestAlg::Sha1 => Pkcs1v15Sign::new::<sha1::Sha1>(),
        DigestAlg::Sha256 => Pkcs1v15Sign::new::<sha2::Sha256>(),
        DigestAlg::Sha384 => Pkcs1v15Sign::new::<sha2::Sha384>(),
        DigestAlg::Sha512 => Pkcs1v15Sign::new::<sha2::Sha512>(),
    }
}

/// Hash links mit Nullen auf `field_len` auffüllen (bits2int für kurze Hashes).
fn pad_prehash(digest: &[u8], field_len: usize) -> Vec<u8> {
    if digest.len() >= field_len {
        return digest.to_vec();
    }
    let mut out = vec![0u8; field_len - digest.len()];
    out.extend_from_slice(digest);
    out
}

/// DER `SEQUENCE { r INTEGER, s INTEGER }` → `r || s` mit fester Länge (tolerant gegenüber
/// fehlenden bzw. überzähligen führenden Nullen; Restdaten werden abgelehnt).
pub(crate) fn ecdsa_der_to_fixed(sig: &[u8], field_len: usize) -> Result<Vec<u8>> {
    let seq = read_tlv(sig).map_err(|_| CmsError::InvalidSignature)?;
    if seq.tag != tag::SEQUENCE || seq.raw.len() != sig.len() {
        return Err(CmsError::InvalidSignature);
    }
    let mut r = seq.reader();
    let mut out = Vec::with_capacity(2 * field_len);
    for _ in 0..2 {
        let int = r.read().map_err(|_| CmsError::InvalidSignature)?;
        if int.tag != tag::INTEGER || int.content.is_empty() || int.content[0] & 0x80 != 0 {
            return Err(CmsError::InvalidSignature);
        }
        let m = integer_magnitude(int.content);
        if m.len() > field_len {
            return Err(CmsError::InvalidSignature);
        }
        out.extend(std::iter::repeat_n(0u8, field_len - m.len()));
        out.extend_from_slice(m);
    }
    if !r.is_empty() {
        return Err(CmsError::InvalidSignature);
    }
    Ok(out)
}

fn check_digest_len(digest: &Digest) -> Result<()> {
    if digest.value.len() != digest.alg.output_len() {
        return Err(CmsError::Key(format!(
            "Hashwert hat {} statt {} Bytes ({})",
            digest.value.len(),
            digest.alg.output_len(),
            digest.alg.name()
        )));
    }
    Ok(())
}

fn key_err(msg: impl Into<String>) -> CmsError {
    CmsError::Key(msg.into())
}

fn key_mismatch() -> CmsError {
    CmsError::KeyMismatch
}

/// Lesbare Kurzbeschreibung eines Schlüsseltyps (ohne Personendaten).
pub(crate) fn describe_key(k: &KeyType) -> String {
    match k {
        KeyType::Rsa { bits } => format!("RSA-{bits}"),
        KeyType::Ec { curve } => match curve {
            EcCurve::P256 => "EC P-256".into(),
            EcCurve::P384 => "EC P-384".into(),
            EcCurve::P521 => "EC P-521".into(),
            EcCurve::BrainpoolP256r1 => "EC brainpoolP256r1".into(),
            EcCurve::BrainpoolP384r1 => "EC brainpoolP384r1".into(),
            EcCurve::BrainpoolP512r1 => "EC brainpoolP512r1".into(),
            EcCurve::Other(oid) => format!("EC {}", crate::oids::curve_name(oid)),
        },
        KeyType::Dsa => "DSA".into(),
        KeyType::Other(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecdsa_der_parsing_is_tolerant_but_strict_on_garbage() {
        // r = 0x01, s = 0x80 (mit führender Null)
        let der = [0x30, 0x07, 0x02, 0x01, 0x01, 0x02, 0x02, 0x00, 0x80];
        let fixed = ecdsa_der_to_fixed(&der, 4).unwrap();
        assert_eq!(fixed, [0, 0, 0, 1, 0, 0, 0, 0x80]);
        // überzählige führende Null (nicht minimal) wird toleriert
        let der = [0x30, 0x08, 0x02, 0x02, 0x00, 0x01, 0x02, 0x02, 0x00, 0x80];
        assert_eq!(ecdsa_der_to_fixed(&der, 4).unwrap(), fixed);
        // negative Zahl, Restdaten, zu lang
        assert!(ecdsa_der_to_fixed(&[0x30, 0x06, 0x02, 0x01, 0x81, 0x02, 0x01, 0x01], 4).is_err());
        assert!(ecdsa_der_to_fixed(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x00], 4).is_err());
        assert!(ecdsa_der_to_fixed(&[0x30, 0x07, 0x02, 0x01, 0x01, 0x02, 0x02, 0x01, 0x01], 1).is_err());
        assert!(ecdsa_der_to_fixed(&[0x30, 0x03, 0x02, 0x01, 0x01], 4).is_err());
    }

    #[test]
    fn prehash_padding() {
        assert_eq!(pad_prehash(&[1, 2], 4), [0, 0, 1, 2]);
        assert_eq!(pad_prehash(&[1, 2, 3, 4, 5], 4), [1, 2, 3, 4, 5]);
    }
}
