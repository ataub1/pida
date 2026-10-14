//! Vorprüfung eines PKCS#12 **ohne Passwort**: Struktur (PFX v3, DER), MAC-Verfahren und die
//! verwendeten Verschlüsselungsverfahren (EncryptedData der Zertifikate, Shrouded Key Bags).
//!
//! Zweck: Nicht lesbare Varianten werden **vor** der Passwortabfrage mit einer verständlichen
//! Meldung abgelehnt ([`KeystoreError::UnsupportedPbe`]), und ein Fehlschlag von `p12-keystore`
//! lässt sich sauber einordnen (MAC vorhanden → falsches Passwort vs. abweichendes
//! Schlüsselpasswort, siehe `lib.rs`).
//!
//! Unterstützt (mit den in `Cargo.toml` aktivierten `pkcs5`-Features):
//! - MAC: HMAC mit SHA-1/224/256/384/512/512-224/512-256 (PKCS#12-KDF); **nicht** PBMAC1 (RFC 9579).
//! - PBES2 mit PBKDF2 (PRF HMAC-SHA-1/224/256/384/512) oder scrypt, Chiffre AES-128/192/256-CBC,
//!   AES-256-GCM oder DES-EDE3-CBC.
//! - PKCS#12-PBE `pbeWithSHAAnd3-KeyTripleDES-CBC` und `pbeWithSHAAnd40BitRC2-CBC`.
//!
//! Nicht unterstützt: PKCS#5-PBES1 (MD5/MD2/SHA-1 mit DES/RC2), PKCS#12-PBE mit RC4 bzw. RC2-128 bzw.
//! 2-Key-3DES, PBES2 mit DES-CBC/RC2, Public-Key-Modi (signedData/envelopedData), BER-Kodierung.

use der::asn1::{AnyRef, ObjectIdentifier};
use der::{Decode, Reader, SliceReader, Tag, TagNumber, Tagged};

use crate::KeystoreError;

const DATA: &str = "1.2.840.113549.1.7.1";
const SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
const ENVELOPED_DATA: &str = "1.2.840.113549.1.7.3";
const ENCRYPTED_DATA: &str = "1.2.840.113549.1.7.6";
const SHROUDED_KEY_BAG: &str = "1.2.840.113549.1.12.10.1.2";
const PBES2: &str = "1.2.840.113549.1.5.13";
const PBKDF2: &str = "1.2.840.113549.1.5.12";
const SCRYPT: &str = "1.3.6.1.4.1.11591.4.11";
const PBMAC1: &str = "1.2.840.113549.1.5.14";

/// Ergebnis der Vorprüfung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Pkcs12Info {
    /// MacData vorhanden (Passwortprüfung über den MAC möglich).
    pub has_mac: bool,
    /// Beschreibung nicht unterstützter Verfahren (leer = alles lesbar).
    pub unsupported: Vec<String>,
    /// Anzahl Shrouded Key Bags im Klartextteil (Diagnose).
    pub shrouded_keys: usize,
}

fn malformed(what: impl std::fmt::Display) -> KeystoreError {
    KeystoreError::Malformed(format!("PKCS#12-Struktur: {what}"))
}

fn children<'a>(any: &AnyRef<'a>) -> Result<Vec<AnyRef<'a>>, KeystoreError> {
    let mut r = SliceReader::new(any.value()).map_err(malformed)?;
    let mut out = Vec::new();
    while !r.is_finished() {
        out.push(r.decode::<AnyRef<'a>>().map_err(malformed)?);
    }
    Ok(out)
}

fn expect_tag<'a>(any: AnyRef<'a>, tag: Tag, what: &str) -> Result<AnyRef<'a>, KeystoreError> {
    if any.tag() == tag { Ok(any) } else { Err(malformed(format!("{what}: {} statt {tag}", any.tag()))) }
}

fn oid(any: &AnyRef<'_>) -> Result<String, KeystoreError> {
    if any.tag() != Tag::ObjectIdentifier {
        return Err(malformed("OID erwartet"));
    }
    ObjectIdentifier::from_bytes(any.value()).map(|o| o.to_string()).map_err(malformed)
}

fn context0(tag: Tag) -> bool {
    matches!(tag, Tag::ContextSpecific { number, .. } if number == TagNumber::N0)
}

/// Inhalt von `[0] EXPLICIT` (erstes Kind).
fn explicit0<'a>(any: &AnyRef<'a>) -> Result<AnyRef<'a>, KeystoreError> {
    if !context0(any.tag()) {
        return Err(malformed("[0] erwartet"));
    }
    children(any)?.into_iter().next().ok_or_else(|| malformed("[0] leer"))
}

/// Liest die PFX-Struktur.
pub(crate) fn inspect(bytes: &[u8]) -> Result<Pkcs12Info, KeystoreError> {
    let pfx = AnyRef::from_der(bytes).map_err(|e| {
        if bytes.get(1) == Some(&0x80) {
            KeystoreError::Malformed("PKCS#12 ist BER-kodiert (unbestimmte Längen) – bitte neu exportieren".into())
        } else {
            malformed(e)
        }
    })?;
    let pfx = expect_tag(pfx, Tag::Sequence, "PFX")?;
    let items = children(&pfx)?;
    let version = items.first().ok_or_else(|| malformed("PFX leer"))?;
    if version.tag() != Tag::Integer || version.value() != [3] {
        return Err(malformed("PFX-Version ≠ 3"));
    }
    let auth_safe = expect_tag(*items.get(1).ok_or_else(|| malformed("authSafe fehlt"))?, Tag::Sequence, "authSafe")?;
    let mut info = Pkcs12Info::default();

    if let Some(mac) = items.get(2) {
        info.has_mac = true;
        let mac = children(&expect_tag(*mac, Tag::Sequence, "MacData")?)?;
        let digest_info = children(&expect_tag(
            *mac.first().ok_or_else(|| malformed("MacData leer"))?,
            Tag::Sequence,
            "DigestInfo",
        )?)?;
        let alg = children(&expect_tag(
            *digest_info.first().ok_or_else(|| malformed("DigestInfo leer"))?,
            Tag::Sequence,
            "AlgorithmIdentifier",
        )?)?;
        let mac_oid = oid(alg.first().ok_or_else(|| malformed("MAC-Algorithmus fehlt"))?)?;
        if !matches!(
            mac_oid.as_str(),
            "1.3.14.3.2.26"
                | "2.16.840.1.101.3.4.2.4"
                | "2.16.840.1.101.3.4.2.1"
                | "2.16.840.1.101.3.4.2.2"
                | "2.16.840.1.101.3.4.2.3"
                | "2.16.840.1.101.3.4.2.5"
                | "2.16.840.1.101.3.4.2.6"
        ) {
            let name = if mac_oid == PBMAC1 { "PBMAC1 (RFC 9579)".to_owned() } else { format!("MAC {mac_oid}") };
            info.unsupported.push(name);
        }
    }

    let ai = children(&auth_safe)?;
    let content_type = oid(ai.first().ok_or_else(|| malformed("authSafe leer"))?)?;
    match content_type.as_str() {
        DATA => {}
        SIGNED_DATA => {
            info.unsupported.push("Public-Key-Integritätsmodus (signedData)".into());
            return Ok(info);
        }
        other => return Err(malformed(format!("authSafe-Inhaltstyp {other}"))),
    }
    let octets = expect_tag(
        explicit0(ai.get(1).ok_or_else(|| malformed("authSafe ohne Inhalt"))?)?,
        Tag::OctetString,
        "authSafe",
    )?;
    let safes = expect_tag(AnyRef::from_der(octets.value()).map_err(malformed)?, Tag::Sequence, "AuthenticatedSafe")?;
    for ci in children(&safes)? {
        let ci = children(&expect_tag(ci, Tag::Sequence, "ContentInfo")?)?;
        let ct = oid(ci.first().ok_or_else(|| malformed("ContentInfo leer"))?)?;
        match ct.as_str() {
            DATA => {
                let Some(content) = ci.get(1) else { continue };
                let octets = expect_tag(explicit0(content)?, Tag::OctetString, "SafeContents")?;
                let bags =
                    expect_tag(AnyRef::from_der(octets.value()).map_err(malformed)?, Tag::Sequence, "SafeContents")?;
                for bag in children(&bags)? {
                    let bag = children(&expect_tag(bag, Tag::Sequence, "SafeBag")?)?;
                    let bag_id = oid(bag.first().ok_or_else(|| malformed("SafeBag leer"))?)?;
                    if bag_id == SHROUDED_KEY_BAG {
                        info.shrouded_keys += 1;
                        let epki = expect_tag(
                            explicit0(bag.get(1).ok_or_else(|| malformed("Bag ohne Wert"))?)?,
                            Tag::Sequence,
                            "EncryptedPrivateKeyInfo",
                        )?;
                        let alg = *children(&epki)?.first().ok_or_else(|| malformed("EncryptedPrivateKeyInfo leer"))?;
                        check_scheme(&alg, "Schlüssel", &mut info.unsupported)?;
                    }
                }
            }
            ENCRYPTED_DATA => {
                let Some(content) = ci.get(1) else { continue };
                let ed = children(&expect_tag(explicit0(content)?, Tag::Sequence, "EncryptedData")?)?;
                let eci = children(&expect_tag(
                    *ed.get(1).ok_or_else(|| malformed("EncryptedData ohne Inhalt"))?,
                    Tag::Sequence,
                    "EncryptedContentInfo",
                )?)?;
                let alg = *eci.get(1).ok_or_else(|| malformed("contentEncryptionAlgorithm fehlt"))?;
                check_scheme(&alg, "Zertifikate", &mut info.unsupported)?;
            }
            ENVELOPED_DATA => info.unsupported.push("Public-Key-Privacy-Modus (envelopedData)".into()),
            other => info.unsupported.push(format!("Inhaltstyp {other}")),
        }
    }
    Ok(info)
}

/// Prüft ein Verschlüsselungsverfahren (AlgorithmIdentifier) auf Lesbarkeit.
fn check_scheme(alg: &AnyRef<'_>, what: &str, unsupported: &mut Vec<String>) -> Result<(), KeystoreError> {
    let parts = children(&expect_tag(*alg, Tag::Sequence, "AlgorithmIdentifier")?)?;
    let scheme = oid(parts.first().ok_or_else(|| malformed("AlgorithmIdentifier leer"))?)?;
    let name = |n: &str| format!("{what}: {n}");
    match scheme.as_str() {
        // PKCS#12-PBE (Anhang C von RFC 7292)
        "1.2.840.113549.1.12.1.3" | "1.2.840.113549.1.12.1.6" => {}
        "1.2.840.113549.1.12.1.1" => unsupported.push(name("pbeWithSHAAnd128BitRC4")),
        "1.2.840.113549.1.12.1.2" => unsupported.push(name("pbeWithSHAAnd40BitRC4")),
        "1.2.840.113549.1.12.1.4" => unsupported.push(name("pbeWithSHAAnd2-KeyTripleDES-CBC")),
        "1.2.840.113549.1.12.1.5" => unsupported.push(name("pbeWithSHAAnd128BitRC2-CBC")),
        // PKCS#5 PBES1
        "1.2.840.113549.1.5.1" => unsupported.push(name("pbeWithMD2AndDES-CBC")),
        "1.2.840.113549.1.5.3" => unsupported.push(name("pbeWithMD5AndDES-CBC")),
        "1.2.840.113549.1.5.4" => unsupported.push(name("pbeWithMD2AndRC2-CBC")),
        "1.2.840.113549.1.5.6" => unsupported.push(name("pbeWithMD5AndRC2-CBC")),
        "1.2.840.113549.1.5.10" => unsupported.push(name("pbeWithSHA1AndDES-CBC")),
        "1.2.840.113549.1.5.11" => unsupported.push(name("pbeWithSHA1AndRC2-CBC")),
        PBES2 => {
            let params = children(&expect_tag(
                *parts.get(1).ok_or_else(|| malformed("PBES2 ohne Parameter"))?,
                Tag::Sequence,
                "PBES2-params",
            )?)?;
            let kdf = children(&expect_tag(
                *params.first().ok_or_else(|| malformed("PBES2 ohne KDF"))?,
                Tag::Sequence,
                "keyDerivationFunc",
            )?)?;
            let kdf_oid = oid(kdf.first().ok_or_else(|| malformed("KDF leer"))?)?;
            match kdf_oid.as_str() {
                PBKDF2 => {
                    let p = children(&expect_tag(
                        *kdf.get(1).ok_or_else(|| malformed("PBKDF2 ohne Parameter"))?,
                        Tag::Sequence,
                        "PBKDF2-params",
                    )?)?;
                    // salt, iterationCount, keyLength OPTIONAL, prf DEFAULT hmacWithSHA1
                    if let Some(prf) = p.iter().skip(2).find(|a| a.tag() == Tag::Sequence) {
                        let prf_oid = oid(children(prf)?.first().ok_or_else(|| malformed("PRF leer"))?)?;
                        if !matches!(
                            prf_oid.as_str(),
                            "1.2.840.113549.2.7"
                                | "1.2.840.113549.2.8"
                                | "1.2.840.113549.2.9"
                                | "1.2.840.113549.2.10"
                                | "1.2.840.113549.2.11"
                        ) {
                            unsupported.push(name(&format!("PBKDF2-PRF {prf_oid}")));
                        }
                    }
                }
                SCRYPT => {}
                other => unsupported.push(name(&format!("PBES2-KDF {other}"))),
            }
            let enc = children(&expect_tag(
                *params.get(1).ok_or_else(|| malformed("PBES2 ohne Chiffre"))?,
                Tag::Sequence,
                "encryptionScheme",
            )?)?;
            let enc_oid = oid(enc.first().ok_or_else(|| malformed("Chiffre leer"))?)?;
            match enc_oid.as_str() {
                "2.16.840.1.101.3.4.1.2"
                | "2.16.840.1.101.3.4.1.22"
                | "2.16.840.1.101.3.4.1.42"
                | "2.16.840.1.101.3.4.1.46"
                | "1.2.840.113549.3.7" => {}
                "1.3.14.3.2.7" => unsupported.push(name("PBES2 mit DES-CBC")),
                "1.2.840.113549.3.2" => unsupported.push(name("PBES2 mit RC2-CBC")),
                other => unsupported.push(name(&format!("PBES2-Chiffre {other}"))),
            }
        }
        other => unsupported.push(name(&format!("Verfahren {other}"))),
    }
    Ok(())
}
