//! TypeScript-Abbild der `pida-pades`-Typen im Workflow (Feature `ts`, nur für die Erzeugung der
//! Bridge-Typen in `pida-core-api`).
//!
//! `ts_rs::TS` lässt sich für fremde Typen nicht implementieren (Orphan-Regel), und `pida-pades`
//! gehört nicht zur Bridge. Die Spiegel werden über `#[ts(as = …)]` eingesetzt. Gegen Abweichungen
//! sichern ab: die vollständige Abbildung `From<&Original>` (neue Felder oder Varianten brechen den
//! Build) und der Test, dass beide gleich serialisieren.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Spiegel von `pida_pades::AcceptanceClass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub enum AcceptanceClass {
    /// K1 regulär
    K1Regular,
    /// K2 reparierbar
    K2Repairable,
    /// K3 verschlüsselt
    K3Encrypted,
    /// K4 eingeschränkt
    K4Restricted,
    /// K5 abgelehnt
    K5Rejected,
}

impl From<&pida_pades::AcceptanceClass> for AcceptanceClass {
    fn from(v: &pida_pades::AcceptanceClass) -> Self {
        use pida_pades::AcceptanceClass as A;
        match v {
            A::K1Regular => AcceptanceClass::K1Regular,
            A::K2Repairable => AcceptanceClass::K2Repairable,
            A::K3Encrypted => AcceptanceClass::K3Encrypted,
            A::K4Restricted => AcceptanceClass::K4Restricted,
            A::K5Rejected => AcceptanceClass::K5Rejected,
        }
    }
}

/// Spiegel von `pida_pades::Invariant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub enum Invariant {
    /// I1
    I1Prefix,
    /// I2
    I2ByteRange,
    /// I3
    I3Cms,
    /// I4
    I4PriorSignatures,
    /// I5
    I5Reparse,
    /// I6
    I6AllowedChanges,
}

impl From<&pida_pades::Invariant> for Invariant {
    fn from(v: &pida_pades::Invariant) -> Self {
        use pida_pades::Invariant as I;
        match v {
            I::I1Prefix => Invariant::I1Prefix,
            I::I2ByteRange => Invariant::I2ByteRange,
            I::I3Cms => Invariant::I3Cms,
            I::I4PriorSignatures => Invariant::I4PriorSignatures,
            I::I5Reparse => Invariant::I5Reparse,
            I::I6AllowedChanges => Invariant::I6AllowedChanges,
        }
    }
}

/// Spiegel von `pida_pades::InvariantViolation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct InvariantViolation {
    /// Invariante
    pub invariant: Invariant,
    /// Detail (ohne Dokumentinhalt)
    pub detail: String,
}

impl From<&pida_pades::InvariantViolation> for InvariantViolation {
    fn from(v: &pida_pades::InvariantViolation) -> Self {
        let pida_pades::InvariantViolation { invariant, detail } = v;
        InvariantViolation { invariant: invariant.into(), detail: detail.clone() }
    }
}

/// Spiegel von `pida_pades::OutputInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct OutputInfo {
    /// Pfad
    pub path: PathBuf,
    /// Länge in Bytes
    pub len: u64,
    /// SHA-256 (hex)
    pub sha256: String,
}

impl From<&pida_pades::OutputInfo> for OutputInfo {
    fn from(v: &pida_pades::OutputInfo) -> Self {
        let pida_pades::OutputInfo { path, len, sha256 } = v;
        OutputInfo { path: path.clone(), len: *len, sha256: sha256.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same<A: Serialize, B: Serialize>(a: &A, b: &B) {
        assert_eq!(serde_json::to_value(a).unwrap(), serde_json::to_value(b).unwrap());
    }

    #[test]
    fn mirrors_serialize_like_the_originals() {
        use pida_pades::AcceptanceClass as A;
        for v in [A::K1Regular, A::K2Repairable, A::K3Encrypted, A::K4Restricted, A::K5Rejected] {
            same(&v, &AcceptanceClass::from(&v));
        }
        use pida_pades::Invariant as I;
        for v in [I::I1Prefix, I::I2ByteRange, I::I3Cms, I::I4PriorSignatures, I::I5Reparse, I::I6AllowedChanges] {
            same(&v, &Invariant::from(&v));
            let violation = pida_pades::InvariantViolation { invariant: v, detail: "x".into() };
            same(&violation, &InvariantViolation::from(&violation));
        }
        let out = pida_pades::OutputInfo { path: "/a_signed.pdf".into(), len: 7, sha256: "ab".into() };
        same(&out, &OutputInfo::from(&out));
    }
}
