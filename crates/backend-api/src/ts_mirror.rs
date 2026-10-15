//! TypeScript-Abbild fremder Typen (Feature `ts`, nur für die Erzeugung der Bridge-Typen).
//!
//! `ts_rs::TS` lässt sich für Typen anderer Crates nicht implementieren (Orphan-Regel); `pida-cades`
//! gehört nicht zur Bridge. Die Spiegel stehen deshalb hier und werden über `#[ts(as = …)]` eingesetzt.
//! Gegen Abweichungen sichern ab: die vollständige Abbildung `From<&Original>` (ein neuer Variantenname
//! bricht den Build) und der Test, dass beide serde-JSON gleich serialisieren.

use serde::{Deserialize, Serialize};

/// Spiegel von `pida_cades::DigestAlg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub enum DigestAlg {
    /// SHA-1
    Sha1,
    /// SHA-256
    Sha256,
    /// SHA-384
    Sha384,
    /// SHA-512
    Sha512,
}

impl From<&pida_cades::DigestAlg> for DigestAlg {
    fn from(v: &pida_cades::DigestAlg) -> Self {
        match v {
            pida_cades::DigestAlg::Sha1 => DigestAlg::Sha1,
            pida_cades::DigestAlg::Sha256 => DigestAlg::Sha256,
            pida_cades::DigestAlg::Sha384 => DigestAlg::Sha384,
            pida_cades::DigestAlg::Sha512 => DigestAlg::Sha512,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_alg_mirror_serializes_like_the_original() {
        use pida_cades::DigestAlg as D;
        for v in [D::Sha1, D::Sha256, D::Sha384, D::Sha512] {
            let a = serde_json::to_value(v).unwrap();
            let b = serde_json::to_value(DigestAlg::from(&v)).unwrap();
            assert_eq!(a, b);
        }
    }
}
