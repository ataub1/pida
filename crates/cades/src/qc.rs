//! qcStatements eines Zertifikats (RFC 3739 §3.2.6, ETSI EN 319 412-5) für die Qualifikation „laut Zertifikat“
//! (Fertig-Ansicht, `pades::verify_document`; ADR 0004 §3.7/§3.17).
//!
//! Ausgewertet werden nur die Angaben **im Zertifikat**: `QcCompliance` (0.4.0.1862.1.1, „qualifiziertes
//! Zertifikat“), `QcSSCD` (0.4.0.1862.1.4, privater Schlüssel in einer qualifizierten
//! Signaturerstellungseinheit) und `QcType` (0.4.0.1862.1.6) mit `id-etsi-qct-esign` (0.4.0.1862.1.6.1).
//! Ob der Aussteller tatsächlich auf der EU-Vertrauensliste steht, prüft PIDA nicht – die Oberfläche sagt
//! deshalb „qualifiziert **laut Zertifikat**“ (ADR 0004 §8 F3).
//!
//! Fremde Zertifikate sind nicht vertrauenswürdig: Eine nicht lesbare Erweiterung gilt als „keine Angabe“
//! (alle Werte `false`), nie als Fehler.

use crate::Certificate;
use crate::der_util::{Tlv, read_tlv, tag};

/// Erweiterung `id-pe-qcStatements`.
pub(crate) const QC_STATEMENTS: &str = "1.3.6.1.5.5.7.1.3";
/// `id-etsi-qcs-QcCompliance`
const QC_COMPLIANCE: &str = "0.4.0.1862.1.1";
/// `id-etsi-qcs-QcSSCD`
const QC_SSCD: &str = "0.4.0.1862.1.4";
/// `id-etsi-qcs-QcType`
const QC_TYPE: &str = "0.4.0.1862.1.6";
/// `id-etsi-qct-esign` (Zertifikat für elektronische Signaturen, nicht Siegel/Website)
const QC_TYPE_ESIGN: &str = "0.4.0.1862.1.6.1";

/// Angaben aus `qcStatements` (laut Zertifikat, nicht gegen eine Vertrauensliste geprüft).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct QcStatements {
    /// `QcCompliance`: das Zertifikat erklärt sich als qualifiziert.
    pub compliance: bool,
    /// `QcSSCD`: privater Schlüssel in einer qualifizierten Signaturerstellungseinheit (QSCD).
    pub sscd: bool,
    /// `QcType` enthält `esign` (Zertifikat für elektronische Signaturen).
    pub esign: bool,
}

/// Liest die qcStatements eines Zertifikats (Erweiterung 1.3.6.1.5.5.7.1.3). Ohne bzw. mit nicht lesbarer
/// Erweiterung: alle Angaben `false`.
pub fn qc_statements(cert: &Certificate) -> QcStatements {
    let Some(exts) = cert.x509().tbs_certificate.extensions.as_ref() else { return QcStatements::default() };
    let Some(ext) = exts.iter().find(|e| e.extn_id.to_string() == QC_STATEMENTS) else {
        return QcStatements::default();
    };
    parse(ext.extn_value.as_bytes()).unwrap_or_default()
}

/// `QCStatements ::= SEQUENCE OF QCStatement`, `QCStatement ::= SEQUENCE { statementId OID,
/// statementInfo ANY OPTIONAL }`.
fn parse(der: &[u8]) -> Option<QcStatements> {
    let seq = read_tlv(der).ok()?;
    if seq.tag != tag::SEQUENCE || seq.raw.len() != der.len() {
        return None;
    }
    let mut out = QcStatements::default();
    for statement in seq.reader().read_all().ok()? {
        if statement.tag != tag::SEQUENCE {
            continue;
        }
        let mut r = statement.reader();
        let Ok(id) = r.read().and_then(|t| t.oid_string()) else { continue };
        match id.as_str() {
            QC_COMPLIANCE => out.compliance = true,
            QC_SSCD => out.sscd = true,
            QC_TYPE => {
                // statementInfo: QcType ::= SEQUENCE OF OBJECT IDENTIFIER
                if let Ok(info) = r.read() {
                    out.esign |= qc_types(&info).iter().any(|t| t == QC_TYPE_ESIGN);
                }
            }
            _ => {}
        }
    }
    Some(out)
}

fn qc_types(info: &Tlv<'_>) -> Vec<String> {
    if info.tag != tag::SEQUENCE {
        return Vec::new();
    }
    info.reader().read_all().map(|v| v.iter().filter_map(|t| t.oid_string().ok()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::der_util as d;

    fn statement(oid: &str, info: Option<&[u8]>) -> Vec<u8> {
        match info {
            Some(i) => d::seq(&[&d::oid(oid), i]),
            None => d::seq(&[&d::oid(oid)]),
        }
    }

    #[test]
    fn parses_compliance_sscd_and_type() {
        let types = d::seq(&[&d::oid(QC_TYPE_ESIGN)]);
        let der = d::seq(&[
            &statement(QC_COMPLIANCE, None),
            &statement(QC_SSCD, None),
            &statement(QC_TYPE, Some(&types)),
            // unbekannte Angabe (z. B. QcPDS mit Daten) wird übersprungen
            &statement("0.4.0.1862.1.5", Some(&d::seq(&[&d::null()]))),
        ]);
        assert_eq!(parse(&der), Some(QcStatements { compliance: true, sscd: true, esign: true }));
        let only = d::seq(&[&statement(QC_COMPLIANCE, None)]);
        assert_eq!(parse(&only), Some(QcStatements { compliance: true, sscd: false, esign: false }));
        // QcType mit Siegel-Typ (eseal) ist kein esign
        let seal = d::seq(&[&statement(QC_TYPE, Some(&d::seq(&[&d::oid("0.4.0.1862.1.6.2")])))]);
        assert_eq!(parse(&seal), Some(QcStatements::default()));
    }

    #[test]
    fn hostile_extensions_are_no_statements() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x30, 0x05, 0x06]), None);
        // Restdaten hinter der SEQUENCE
        let mut der = d::seq(&[&statement(QC_COMPLIANCE, None)]);
        der.push(0);
        assert_eq!(parse(&der), None);
        // Einträge, die keine SEQUENCE sind, bzw. ohne OID
        let odd = d::seq(&[&d::null(), &d::seq(&[&d::null()]), &statement(QC_SSCD, None)]);
        assert_eq!(parse(&odd), Some(QcStatements { compliance: false, sscd: true, esign: false }));
    }

    #[test]
    fn test_certificates_without_extension_are_not_qualified() {
        let der = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.crt.der"))
            .expect("Testzertifikat");
        let cert = Certificate::from_der(&der).unwrap();
        assert_eq!(qc_statements(&cert), QcStatements::default());
    }

    #[test]
    fn qes_test_certificate_declares_compliance_and_sscd() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/verify/keys/test-qes-ecp256.crt.der");
        let der = std::fs::read(path).expect("QES-Testzertifikat (testdata/verify/keys)");
        let cert = Certificate::from_der(&der).unwrap();
        assert_eq!(qc_statements(&cert), QcStatements { compliance: true, sscd: true, esign: true });
    }
}
