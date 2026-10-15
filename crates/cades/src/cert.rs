//! X.509-Zertifikate (dünne Hülle um `x509-cert` 0.2, DER bleibt erhalten).

use der::{Decode, Encode, Tag, Tagged};
use time::OffsetDateTime;

use crate::{CmsError, Digest, DigestAlg, Result, oids};

/// Ein Attribut eines Distinguished Names (eine AVA), in Reihenfolge des DER (links = zuerst
/// kodiert = Wurzel). `Debug` zeigt OID, Kurzname und nur die Länge des Werts (Personendaten,
/// ADR 0001 Regel 9).
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RdnAttr {
    /// OID in Punktnotation, z. B. `2.5.4.3`.
    pub oid: String,
    /// Gebräuchlicher Kurzname (`CN`, `O`, `C`, `title`, `SERIALNUMBER` …), falls bekannt.
    /// Die Alias-Regeln für Profile (`title` **und** `T`, linkester Eintrag gewinnt) wendet
    /// `pida-sigblock` an (Plan §6.5 Kompatibilitätsregister).
    pub short_name: Option<String>,
    /// Wert als Text (UTF8String, PrintableString, IA5String, TeletexString, BMPString).
    pub value: String,
}

impl std::fmt::Debug for RdnAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RdnAttr")
            .field("oid", &self.oid)
            .field("short_name", &self.short_name)
            .field("value_len", &self.value.chars().count())
            .finish()
    }
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
    /// Nur der SHA-256-Fingerprint (gekürzt) – Subject/Seriennummer sind Personendaten und gehören
    /// nicht in Logs (ADR 0001 §4 Regel 9).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fp = self.fingerprint(DigestAlg::Sha256).to_hex();
        f.debug_struct("Certificate").field("sha256", &&fp[..16]).finish_non_exhaustive()
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

    /// Aus PEM (`-----BEGIN CERTIFICATE-----`). Text vor dem ersten Block (z. B. Ausgabe von
    /// `openssl x509 -text`) wird ignoriert; bei mehreren Blöcken zählt der erste. Akzeptiert auch
    /// `X509 CERTIFICATE` und `TRUSTED CERTIFICATE` (OpenSSL-Zusatzdaten hinter dem DER werden über die
    /// DER-Länge abgeschnitten).
    pub fn from_pem(pem: &str) -> Result<Self> {
        Self::from_pem_all(pem)?
            .into_iter()
            .next()
            .ok_or_else(|| CmsError::Certificate("kein PEM-Block CERTIFICATE gefunden".into()))
    }

    /// Alle Zertifikate einer PEM-Datei in Reihenfolge (z. B. Kette `chain.pem`).
    pub fn from_pem_all(pem: &str) -> Result<Vec<Self>> {
        use base64::Engine as _;
        const LABELS: [&str; 3] = ["CERTIFICATE", "X509 CERTIFICATE", "TRUSTED CERTIFICATE"];
        let mut out = Vec::new();
        let mut rest = pem;
        while let Some(start) = rest.find("-----BEGIN ") {
            let after = &rest[start + "-----BEGIN ".len()..];
            let Some(label_end) = after.find("-----") else { break };
            let label = after[..label_end].trim();
            let body_start = &after[label_end + 5..];
            let end_marker = format!("-----END {label}-----");
            let Some(end) = body_start.find(&end_marker) else {
                return Err(CmsError::Certificate(format!("PEM-Block {label} ohne Ende")));
            };
            let body = &body_start[..end];
            rest = &body_start[end + end_marker.len()..];
            if !LABELS.contains(&label) {
                continue;
            }
            // Header-Zeilen (RFC 1421, `Proc-Type:` …) gibt es bei Zertifikaten nicht; Leerraum weg.
            let b64: String = body.chars().filter(|c| !c.is_whitespace()).collect();
            let der = base64::engine::general_purpose::STANDARD
                .decode(b64.as_bytes())
                .map_err(|e| CmsError::Certificate(format!("PEM/Base64: {e}")))?;
            let tlv = crate::der_util::read_tlv(&der).map_err(|e| CmsError::Certificate(e.to_string()))?;
            out.push(Self::from_der(tlv.raw)?);
        }
        Ok(out)
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
    ///
    /// RSA (`rsaEncryption`) mit Modullänge in Bit; EC (`id-ecPublicKey`) mit benannter Kurve
    /// (P-256/384/521, brainpoolP256/384/512r1, sonst [`EcCurve::Other`] mit OID; explizite
    /// Kurvenparameter → `Other("explicit")`); DSA; alles andere (RSASSA-PSS-Schlüssel, Ed25519,
    /// Ed448 …) → [`KeyType::Other`] – PDF-AS lehnt diese mit `NoSuchAlgorithmException` ab.
    pub fn key_type(&self) -> Result<KeyType> {
        let spki = &self.parsed.tbs_certificate.subject_public_key_info;
        let oid = spki.algorithm.oid.to_string();
        Ok(match oid.as_str() {
            oids::RSA_ENCRYPTION => KeyType::Rsa { bits: rsa_modulus_bits(self.public_key_bits()?)? },
            oids::EC_PUBLIC_KEY => KeyType::Ec { curve: ec_curve_from_params(spki.algorithm.parameters.as_ref())? },
            oids::DSA => KeyType::Dsa,
            oids::RSASSA_PSS => KeyType::Other("RSASSA-PSS".into()),
            oids::ED25519 => KeyType::Other("Ed25519".into()),
            oids::ED448 => KeyType::Other("Ed448".into()),
            _ => KeyType::Other(oid),
        })
    }

    /// Inhalt des `subjectPublicKey`-BIT-STRINGs (ohne „unused bits“-Oktett).
    pub(crate) fn public_key_bits(&self) -> Result<&[u8]> {
        self.parsed
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .as_bytes()
            .ok_or_else(|| CmsError::Certificate("subjectPublicKey mit ungenutzten Bits".into()))
    }

    /// Rohe DER-Kodierungen von `issuer` (Name) und `serialNumber` (INTEGER), unverändert aus dem
    /// Zertifikat – für IssuerAndSerialNumber und ESS IssuerSerial.
    pub(crate) fn issuer_and_serial_raw(&self) -> Result<(&[u8], &[u8])> {
        use crate::der_util::{Reader, read_tlv, tag};
        let cert = read_tlv(&self.der)?.expect(tag::SEQUENCE, "Certificate")?;
        let tbs = cert.reader().read_expect(tag::SEQUENCE, "TBSCertificate")?;
        let mut r: Reader<'_> = tbs.reader();
        r.read_optional(0xA0)?; // version [0] EXPLICIT
        let serial = r.read_expect(tag::INTEGER, "serialNumber")?;
        r.read_expect(tag::SEQUENCE, "signature")?;
        let issuer = r.read_expect(tag::SEQUENCE, "issuer")?;
        Ok((issuer.raw, serial.raw))
    }

    /// Wert der Erweiterung SubjectKeyIdentifier (2.5.29.14), falls vorhanden.
    pub(crate) fn subject_key_identifier(&self) -> Option<Vec<u8>> {
        let exts = self.parsed.tbs_certificate.extensions.as_ref()?;
        let ext = exts.iter().find(|e| e.extn_id.to_string() == "2.5.29.14")?;
        let t = crate::der_util::read_tlv(ext.extn_value.as_bytes()).ok()?;
        (t.tag == crate::der_util::tag::OCTET_STRING).then(|| t.content.to_vec())
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

/// Modullänge in Bit aus `RSAPublicKey ::= SEQUENCE { modulus INTEGER, publicExponent INTEGER }`.
pub(crate) fn rsa_modulus_bits(rsa_public_key: &[u8]) -> Result<u32> {
    use crate::der_util::{integer_magnitude, read_tlv, tag};
    let seq = read_tlv(rsa_public_key)?.expect(tag::SEQUENCE, "RSAPublicKey")?;
    let n = seq.reader().read_expect(tag::INTEGER, "modulus")?;
    let m = integer_magnitude(n.content);
    if m.is_empty() || m == [0] {
        return Err(CmsError::Certificate("RSA-Modul ist 0".into()));
    }
    Ok((m.len() as u32 - 1) * 8 + (8 - m[0].leading_zeros()))
}

/// Kurve aus den Parametern von `id-ecPublicKey` (`namedCurve` OID).
fn ec_curve_from_params(params: Option<&der::Any>) -> Result<EcCurve> {
    let Some(params) = params else {
        // implicitlyCA ohne Parameter (RFC 5480 verbietet das, kommt aber vor)
        return Ok(EcCurve::Other("implicitlyCA".into()));
    };
    if params.tag() != Tag::ObjectIdentifier {
        return Ok(EcCurve::Other("explicit".into()));
    }
    let oid = crate::der_util::oid_to_string(params.value())?;
    Ok(match oid.as_str() {
        oids::P256 => EcCurve::P256,
        oids::P384 => EcCurve::P384,
        oids::P521 => EcCurve::P521,
        oids::BRAINPOOL_P256R1 => EcCurve::BrainpoolP256r1,
        oids::BRAINPOOL_P384R1 => EcCurve::BrainpoolP384r1,
        oids::BRAINPOOL_P512R1 => EcCurve::BrainpoolP512r1,
        _ => EcCurve::Other(oid),
    })
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
    fn debug_hides_personal_data() {
        let der = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.crt.der"))
            .expect("Testzertifikat");
        let cert = super::Certificate::from_der(&der).unwrap();
        assert!(cert.subject_string().contains("NUR TEST"));
        let dbg = format!("{cert:?}");
        assert!(!dbg.contains("NUR TEST"), "{dbg}");
        assert!(!dbg.contains(&cert.serial_decimal()), "{dbg}");
    }

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
