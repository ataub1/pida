//! X.509-Zertifikate (dünne Hülle um `x509-cert` 0.2, DER bleibt erhalten).

use der::{Decode, Encode, Tag, Tagged};
use time::OffsetDateTime;

use crate::{CmsError, Digest, DigestAlg, Result};

/// Ein Attribut eines Distinguished Names (eine AVA), in Reihenfolge des DER (links = zuerst
/// kodiert = Wurzel).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RdnAttr {
    /// OID in Punktnotation, z. B. `2.5.4.3`.
    pub oid: String,
    /// Gebräuchlicher Kurzname (`CN`, `O`, `C`, `title`, `SERIALNUMBER` …), falls bekannt.
    /// Die Alias-Regeln für Profile (`title` **und** `T`, linkester Eintrag gewinnt) wendet
    /// `pdfover-sigblock` an (Plan §6.5 Kompatibilitätsregister).
    pub short_name: Option<String>,
    /// Wert als Text (UTF8String, PrintableString, IA5String, TeletexString, BMPString).
    pub value: String,
}

/// Schlüsseltyp des öffentlichen Schlüssels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum KeyType {
    /// RSA mit Modullänge in Bit.
    Rsa {
        /// Modullänge
        bits: u32,
    },
    /// Elliptische Kurve.
    Ec {
        /// Kurve
        curve: EcCurve,
    },
    /// DSA (wird abgelehnt, Plan §4.9).
    Dsa,
    /// Ed25519 o. ä. (PDF-AS: `NoSuchAlgorithmException`).
    Other(String),
}

/// Elliptische Kurven.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EcCurve {
    /// secp256r1
    P256,
    /// secp384r1
    P384,
    /// secp521r1
    P521,
    /// brainpoolP256r1
    BrainpoolP256r1,
    /// brainpoolP384r1
    BrainpoolP384r1,
    /// brainpoolP512r1
    BrainpoolP512r1,
    /// andere Kurve (OID)
    Other(String),
}

impl EcCurve {
    /// Feldgröße in Bit (Grundlage der PDF-AS-Hashwahl, docs/analyse/03 §5.1).
    pub fn field_bits(&self) -> Option<u32> {
        Some(match self {
            EcCurve::P256 | EcCurve::BrainpoolP256r1 => 256,
            EcCurve::P384 | EcCurve::BrainpoolP384r1 => 384,
            EcCurve::P521 => 521,
            EcCurve::BrainpoolP512r1 => 512,
            EcCurve::Other(_) => return None,
        })
    }
}

/// X.509-Zertifikat mit unveränderten DER-Bytes.
#[derive(Clone)]
pub struct Certificate {
    der: Vec<u8>,
    // geboxt, damit `Certificate` klein bleibt (Enums mit Zertifikaten, clippy::large_enum_variant)
    parsed: Box<x509_cert::Certificate>,
}

impl std::fmt::Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificate")
            .field("subject", &self.subject_string())
            .field("serial", &self.serial_decimal())
            .finish()
    }
}

impl PartialEq for Certificate {
    fn eq(&self, other: &Self) -> bool {
        self.der == other.der
    }
}
impl Eq for Certificate {}

impl Certificate {
    /// Aus DER (z. B. `Base64Content` einer `InfoboxReadResponse`, docs/analyse/02 §2.4).
    pub fn from_der(der: &[u8]) -> Result<Self> {
        let parsed = x509_cert::Certificate::from_der(der).map_err(|e| CmsError::Certificate(e.to_string()))?;
        // Bytes so übernehmen, wie sie kodiert sind (Fingerprint über das Original).
        Ok(Self { der: der.to_vec(), parsed: Box::new(parsed) })
    }

    /// Aus PEM (`-----BEGIN CERTIFICATE-----`).
    pub fn from_pem(pem: &str) -> Result<Self> {
        todo!("Paket B")
    }

    /// DER-Bytes.
    pub fn as_der(&self) -> &[u8] {
        &self.der
    }

    /// Zugriff auf den geparsten `x509-cert`-Typ (gepinnte Generation 0.2).
    pub fn x509(&self) -> &x509_cert::Certificate {
        &self.parsed
    }

    /// Fingerprint über das DER.
    pub fn fingerprint(&self, alg: DigestAlg) -> Digest {
        alg.digest(&self.der)
    }

    /// Seriennummer als vorzeichenbehaftete Dezimalzahl (Java `BigInteger.toString()`, Profilvariable
    /// `sn`, docs/analyse/04 §2.5).
    pub fn serial_decimal(&self) -> String {
        twos_complement_to_decimal(self.parsed.tbs_certificate.serial_number.as_bytes())
    }

    /// Seriennummer als Bytes (Zweierkomplement, big-endian).
    pub fn serial_bytes(&self) -> &[u8] {
        self.parsed.tbs_certificate.serial_number.as_bytes()
    }

    /// Subject-AVAs in DER-Reihenfolge.
    pub fn subject_rdns(&self) -> Vec<RdnAttr> {
        name_to_attrs(&self.parsed.tbs_certificate.subject)
    }

    /// Issuer-AVAs in DER-Reihenfolge.
    pub fn issuer_rdns(&self) -> Vec<RdnAttr> {
        name_to_attrs(&self.parsed.tbs_certificate.issuer)
    }

    /// Subject als RFC-4514-Text.
    pub fn subject_string(&self) -> String {
        self.parsed.tbs_certificate.subject.to_string()
    }

    /// Issuer als RFC-4514-Text.
    pub fn issuer_string(&self) -> String {
        self.parsed.tbs_certificate.issuer.to_string()
    }

    /// Erster Common Name des Subjects (für `/Name`, wenn kein Profilwert vorhanden).
    pub fn subject_cn(&self) -> Option<String> {
        self.subject_rdns().into_iter().find(|a| a.oid == "2.5.4.3").map(|a| a.value)
    }

    /// Beginn der Gültigkeit.
    pub fn not_before(&self) -> OffsetDateTime {
        x509_time(&self.parsed.tbs_certificate.validity.not_before)
    }

    /// Ende der Gültigkeit.
    pub fn not_after(&self) -> OffsetDateTime {
        x509_time(&self.parsed.tbs_certificate.validity.not_after)
    }

    /// Gültig zum Zeitpunkt `at` (PDF-AS prüft das vor dem Signieren, Fehler 11021).
    pub fn is_valid_at(&self, at: OffsetDateTime) -> bool {
        self.not_before() <= at && at <= self.not_after()
    }

    /// Schlüsseltyp aus der SubjectPublicKeyInfo.
    pub fn key_type(&self) -> Result<KeyType> {
        todo!("Paket B: SPKI auswerten (RSA-Modullänge, EC-Kurven-OID, DSA)")
    }

    /// DER der `SubjectPublicKeyInfo`.
    pub fn spki_der(&self) -> Result<Vec<u8>> {
        self.parsed.tbs_certificate.subject_public_key_info.to_der().map_err(|e| CmsError::Der(e.to_string()))
    }
}

/// Signer-Zertifikat plus optionale Zwischenzertifikate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertChain {
    /// Signerzertifikat.
    pub signer: Certificate,
    /// Zwischenzertifikate (PDF-AS bettet nur das Signerzertifikat ein, Plan §6.10).
    pub intermediates: Vec<Certificate>,
}

impl CertChain {
    /// Kette nur aus dem Signerzertifikat.
    pub fn new(signer: Certificate) -> Self {
        Self { signer, intermediates: Vec::new() }
    }

    /// Alle Zertifikate, Signer zuerst.
    pub fn iter(&self) -> impl Iterator<Item = &Certificate> + '_ {
        std::iter::once(&self.signer).chain(self.intermediates.iter())
    }
}

fn name_to_attrs(name: &x509_cert::name::Name) -> Vec<RdnAttr> {
    let mut out = Vec::new();
    for rdn in name.0.iter() {
        for ava in rdn.0.iter() {
            let oid = ava.oid.to_string();
            out.push(RdnAttr {
                short_name: short_name(&oid).map(str::to_owned),
                value: any_to_string(&ava.value),
                oid,
            });
        }
    }
    out
}

fn short_name(oid: &str) -> Option<&'static str> {
    Some(match oid {
        "2.5.4.3" => "CN",
        "2.5.4.4" => "SURNAME",
        "2.5.4.5" => "SERIALNUMBER",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.9" => "STREET",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.12" => "title",
        "2.5.4.42" => "GIVENNAME",
        "2.5.4.43" => "INITIALS",
        "2.5.4.44" => "GENERATION",
        "2.5.4.46" => "DNQ",
        "2.5.4.65" => "PSEUDONYM",
        "2.5.4.97" => "ORGANIZATIONIDENTIFIER",
        "1.2.840.113549.1.9.1" => "EMAILADDRESS",
        "0.9.2342.19200300.100.1.1" => "UID",
        "0.9.2342.19200300.100.1.25" => "DC",
        _ => return None,
    })
}

fn any_to_string(any: &der::Any) -> String {
    let bytes = any.value();
    match any.tag() {
        Tag::BmpString => {
            let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
            String::from_utf16_lossy(&units)
        }
        Tag::Utf8String | Tag::PrintableString | Tag::Ia5String | Tag::VisibleString => {
            String::from_utf8_lossy(bytes).into_owned()
        }
        // TeletexString: in der Praxis Latin-1.
        Tag::TeletexString => bytes.iter().map(|&b| char::from(b)).collect(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn x509_time(t: &x509_cert::time::Time) -> OffsetDateTime {
    let secs = t.to_unix_duration().as_secs() as i64;
    OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

/// Zweierkomplement (big-endian) → Dezimaltext wie `BigInteger.toString()`.
fn twos_complement_to_decimal(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0".into();
    }
    let negative = bytes[0] & 0x80 != 0;
    let mut mag: Vec<u8> = if negative {
        // Betrag = ~x + 1
        let mut v: Vec<u8> = bytes.iter().map(|b| !b).collect();
        for b in v.iter_mut().rev() {
            let (r, carry) = b.overflowing_add(1);
            *b = r;
            if !carry {
                break;
            }
        }
        v
    } else {
        bytes.to_vec()
    };
    // Wiederholte Division durch 10.
    let mut digits = Vec::new();
    while mag.iter().any(|&b| b != 0) {
        let mut rem: u32 = 0;
        for b in mag.iter_mut() {
            let cur = (rem << 8) | u32::from(*b);
            *b = (cur / 10) as u8;
            rem = cur % 10;
        }
        digits.push(char::from(b'0' + rem as u8));
    }
    if digits.is_empty() {
        digits.push('0');
    }
    if negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::twos_complement_to_decimal as dec;

    #[test]
    fn serial_to_decimal() {
        assert_eq!(dec(&[0x00]), "0");
        assert_eq!(dec(&[0x01, 0x00]), "256");
        assert_eq!(dec(&[0x00, 0xFF]), "255");
        assert_eq!(dec(&[0xFF]), "-1");
        assert_eq!(dec(&[0x80]), "-128");
        assert_eq!(dec(&[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]), "1311768467463790320");
    }
}
