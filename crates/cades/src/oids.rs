//! OIDs (Punktnotation) für CMS, ESS, X.509 und die unterstützten Algorithmen.

// CMS-Inhaltstypen (RFC 5652)
pub(crate) const DATA: &str = "1.2.840.113549.1.7.1";
pub(crate) const SIGNED_DATA: &str = "1.2.840.113549.1.7.2";

// Signierte Attribute
pub(crate) const CONTENT_TYPE: &str = "1.2.840.113549.1.9.3";
pub(crate) const MESSAGE_DIGEST: &str = "1.2.840.113549.1.9.4";
pub(crate) const SIGNING_TIME: &str = "1.2.840.113549.1.9.5";
/// ETSI `id-aa-ets-mimeType` (ETSI TS 101 733), von PDF-AS gesetzt (`PKS:298-306`).
pub(crate) const ETSI_MIME_TYPE: &str = "0.4.0.1733.2.1";
/// ESS `id-aa-signingCertificate` (RFC 2634).
pub(crate) const SIGNING_CERTIFICATE: &str = "1.2.840.113549.1.9.16.2.12";
/// ESS `id-aa-signingCertificateV2` (RFC 5035).
pub(crate) const SIGNING_CERTIFICATE_V2: &str = "1.2.840.113549.1.9.16.2.47";

// Schlüsselalgorithmen (SubjectPublicKeyInfo)
pub(crate) const RSA_ENCRYPTION: &str = "1.2.840.113549.1.1.1";
pub(crate) const RSASSA_PSS: &str = "1.2.840.113549.1.1.10";
pub(crate) const MGF1: &str = "1.2.840.113549.1.1.8";
pub(crate) const EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
pub(crate) const DSA: &str = "1.2.840.10040.4.1";
pub(crate) const ED25519: &str = "1.3.101.112";
pub(crate) const ED448: &str = "1.3.101.113";

// Kurven
pub(crate) const P256: &str = "1.2.840.10045.3.1.7";
pub(crate) const P384: &str = "1.3.132.0.34";
pub(crate) const P521: &str = "1.3.132.0.35";
pub(crate) const BRAINPOOL_P256R1: &str = "1.3.36.3.3.2.8.1.1.7";
pub(crate) const BRAINPOOL_P384R1: &str = "1.3.36.3.3.2.8.1.1.11";
pub(crate) const BRAINPOOL_P512R1: &str = "1.3.36.3.3.2.8.1.1.13";

/// Feldgröße in Bit für Kurven, die [`crate::EcCurve`] nur als `Other(oid)` führt – Grundlage der
/// PDF-AS-Hashwahl (`CertificateUtils.getAlgorithmIDs`: `getField().getFieldSize()`).
pub(crate) fn other_curve_field_bits(oid: &str) -> Option<u32> {
    Some(match oid {
        // SEC 2 / X9.62 Primkörper
        "1.2.840.10045.3.1.1" => 192, // secp192r1 / prime192v1
        "1.3.132.0.33" => 224,        // secp224r1
        "1.3.132.0.10" => 256,        // secp256k1
        "1.3.132.0.31" => 192,        // secp192k1
        "1.3.132.0.32" => 224,        // secp224k1
        "1.3.132.0.8" => 160,         // secp160r1
        // Brainpool (RFC 5639)
        "1.3.36.3.3.2.8.1.1.1" => 160,  // brainpoolP160r1
        "1.3.36.3.3.2.8.1.1.3" => 192,  // brainpoolP192r1
        "1.3.36.3.3.2.8.1.1.5" => 224,  // brainpoolP224r1
        "1.3.36.3.3.2.8.1.1.9" => 320,  // brainpoolP320r1
        "1.3.36.3.3.2.8.1.1.2" => 160,  // brainpoolP160t1
        "1.3.36.3.3.2.8.1.1.8" => 256,  // brainpoolP256t1
        "1.3.36.3.3.2.8.1.1.12" => 384, // brainpoolP384t1
        "1.3.36.3.3.2.8.1.1.14" => 512, // brainpoolP512t1
        // Binärkörper (Feldgröße = Grad m)
        "1.3.132.0.1" => 163,  // sect163k1
        "1.3.132.0.15" => 163, // sect163r2
        "1.3.132.0.26" => 233, // sect233k1
        "1.3.132.0.27" => 233, // sect233r1
        "1.3.132.0.16" => 283, // sect283k1
        "1.3.132.0.17" => 283, // sect283r1
        "1.3.132.0.36" => 409, // sect409k1
        "1.3.132.0.37" => 409, // sect409r1
        "1.3.132.0.38" => 571, // sect571k1
        "1.3.132.0.39" => 571, // sect571r1
        _ => return None,
    })
}

/// Kurzname bekannter Kurven-OIDs (Fehlermeldungen).
pub(crate) fn curve_name(oid: &str) -> &str {
    match oid {
        "1.2.840.10045.3.1.1" => "secp192r1",
        "1.3.132.0.33" => "secp224r1",
        "1.3.132.0.10" => "secp256k1",
        "1.3.36.3.3.2.8.1.1.9" => "brainpoolP320r1",
        _ => oid,
    }
}
