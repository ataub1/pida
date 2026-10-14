//! Hash- und Signaturalgorithmen (docs/analyse/03 §5.1 Algorithmuswahl).

use sha2::Digest as _;

/// Hashverfahren.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DigestAlg {
    /// SHA-1 – **nur Prüfung** von Vorsignaturen.
    Sha1,
    /// SHA-256 – Standard.
    Sha256,
    /// SHA-384
    Sha384,
    /// SHA-512 – EC mit Feldgröße ≥ 512.
    Sha512,
}

impl DigestAlg {
    /// OID in Punktnotation.
    pub fn oid(self) -> &'static str {
        match self {
            DigestAlg::Sha1 => "1.3.14.3.2.26",
            DigestAlg::Sha256 => "2.16.840.1.101.3.4.2.1",
            DigestAlg::Sha384 => "2.16.840.1.101.3.4.2.2",
            DigestAlg::Sha512 => "2.16.840.1.101.3.4.2.3",
        }
    }

    /// Länge des Hashwerts in Bytes.
    pub fn output_len(self) -> usize {
        match self {
            DigestAlg::Sha1 => 20,
            DigestAlg::Sha256 => 32,
            DigestAlg::Sha384 => 48,
            DigestAlg::Sha512 => 64,
        }
    }

    /// Anzeigename.
    pub fn name(self) -> &'static str {
        match self {
            DigestAlg::Sha1 => "SHA-1",
            DigestAlg::Sha256 => "SHA-256",
            DigestAlg::Sha384 => "SHA-384",
            DigestAlg::Sha512 => "SHA-512",
        }
    }

    /// Hash über `data` in einem Schritt.
    pub fn digest(self, data: &[u8]) -> Digest {
        let mut h = self.hasher();
        h.update(data);
        h.finalize()
    }

    /// Inkrementeller Hasher (für gestreamte ByteRange-Daten, Plan §6.2 „Speicher“).
    pub fn hasher(self) -> Hasher {
        Hasher(match self {
            DigestAlg::Sha1 => HasherState::Sha1(sha1::Sha1::new()),
            DigestAlg::Sha256 => HasherState::Sha256(sha2::Sha256::new()),
            DigestAlg::Sha384 => HasherState::Sha384(sha2::Sha384::new()),
            DigestAlg::Sha512 => HasherState::Sha512(sha2::Sha512::new()),
        })
    }
}

/// Hashwert mit Algorithmus.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Digest {
    /// Verfahren.
    pub alg: DigestAlg,
    /// Wert.
    pub value: Vec<u8>,
}

impl Digest {
    /// Hex in Kleinbuchstaben (Anzeige, z. B. WYSIWYS-SHA-256).
    pub fn to_hex(&self) -> String {
        hex::encode(&self.value)
    }
}

#[derive(Clone)]
enum HasherState {
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha384(sha2::Sha384),
    Sha512(sha2::Sha512),
}

/// Inkrementeller Hasher.
#[derive(Clone)]
pub struct Hasher(HasherState);

impl std::fmt::Debug for Hasher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Hasher").field(&self.alg()).finish()
    }
}

impl Hasher {
    /// Verfahren.
    pub fn alg(&self) -> DigestAlg {
        match &self.0 {
            HasherState::Sha1(_) => DigestAlg::Sha1,
            HasherState::Sha256(_) => DigestAlg::Sha256,
            HasherState::Sha384(_) => DigestAlg::Sha384,
            HasherState::Sha512(_) => DigestAlg::Sha512,
        }
    }

    /// Daten anhängen.
    pub fn update(&mut self, data: &[u8]) {
        match &mut self.0 {
            HasherState::Sha1(h) => h.update(data),
            HasherState::Sha256(h) => h.update(data),
            HasherState::Sha384(h) => h.update(data),
            HasherState::Sha512(h) => h.update(data),
        }
    }

    /// Abschließen.
    pub fn finalize(self) -> Digest {
        let alg = self.alg();
        let value = match self.0 {
            HasherState::Sha1(h) => h.finalize().to_vec(),
            HasherState::Sha256(h) => h.finalize().to_vec(),
            HasherState::Sha384(h) => h.finalize().to_vec(),
            HasherState::Sha512(h) => h.finalize().to_vec(),
        };
        Digest { alg, value }
    }
}

impl std::io::Write for Hasher {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Signaturalgorithmus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SignatureAlg {
    /// RSASSA-PKCS1-v1_5 (`sha256WithRSAEncryption` usw.).
    RsaPkcs1v15(DigestAlg),
    /// RSASSA-PSS – **nur Prüfung** von Vorsignaturen.
    RsaPss {
        /// Hash
        digest: DigestAlg,
        /// Salzlänge
        salt_len: u16,
    },
    /// ECDSA (`ecdsa-with-SHA256` usw.); Signaturwert DER `SEQUENCE { r, s }`.
    Ecdsa(DigestAlg),
}

impl SignatureAlg {
    /// Zugehöriges Hashverfahren.
    pub fn digest(self) -> DigestAlg {
        match self {
            SignatureAlg::RsaPkcs1v15(d) | SignatureAlg::Ecdsa(d) => d,
            SignatureAlg::RsaPss { digest, .. } => digest,
        }
    }

    /// OID des `signatureAlgorithm` im SignerInfo.
    pub fn oid(self) -> &'static str {
        match self {
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha1) => "1.2.840.113549.1.1.5",
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256) => "1.2.840.113549.1.1.11",
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha384) => "1.2.840.113549.1.1.12",
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha512) => "1.2.840.113549.1.1.13",
            SignatureAlg::RsaPss { .. } => "1.2.840.113549.1.1.10",
            SignatureAlg::Ecdsa(DigestAlg::Sha1) => "1.2.840.10045.4.1",
            SignatureAlg::Ecdsa(DigestAlg::Sha256) => "1.2.840.10045.4.3.2",
            SignatureAlg::Ecdsa(DigestAlg::Sha384) => "1.2.840.10045.4.3.3",
            SignatureAlg::Ecdsa(DigestAlg::Sha512) => "1.2.840.10045.4.3.4",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_answer() {
        let d = DigestAlg::Sha256.digest(b"abc");
        assert_eq!(d.to_hex(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let mut h = DigestAlg::Sha256.hasher();
        h.update(b"a");
        h.update(b"bc");
        assert_eq!(h.finalize(), d);
    }
}
