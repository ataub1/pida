//! Eingaben des Signaturblocks (docs/analyse/04 §2.5).

use std::collections::BTreeMap;
use std::sync::Arc;

use time::OffsetDateTime;

use crate::emblem::Emblem;

/// Zertifikatsfelder für die Profilvariablen `subject.X`, `issuer.X`, `sn`.
///
/// Wird von der Integration aus `pida_cades::Certificate` befüllt (Attributname = Kurzname
/// wie `CN`, `O`, `C`, `title` bzw. OID). `pida-sigblock` wendet die Alias-Regeln an:
/// Titel-RDN (2.5.4.12) unter `title` **und** `T`; bei Mehrfachvorkommen gewinnt der linkeste
/// Eintrag (Plan §6.5 Kompatibilitätsregister).
/// `Debug` zeigt nur die Attributnamen (Personendaten, ADR 0001 §4 Regel 9).
#[derive(Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct CertFields {
    /// Subject-AVAs in DER-Reihenfolge.
    pub subject: Vec<(String, String)>,
    /// Issuer-AVAs in DER-Reihenfolge.
    pub issuer: Vec<(String, String)>,
    /// Seriennummer dezimal (`sn`).
    pub serial_decimal: String,
    /// Subject-DN als Text (`subject.DN`).
    pub subject_dn: String,
    /// Issuer-DN als Text (`issuer.DN`).
    pub issuer_dn: String,
}

impl std::fmt::Debug for CertFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = |v: &[(String, String)]| v.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        f.debug_struct("CertFields")
            .field("subject", &names(&self.subject))
            .field("issuer", &names(&self.issuer))
            .finish_non_exhaustive()
    }
}

impl CertFields {
    /// Beispielzertifikat für die Vorschau vor bekanntem Zertifikat (wie `example.cer` bei PDF-AS,
    /// Plan §6.6).
    pub fn example() -> CertFields {
        // Fiktive Daten (kein echtes Zertifikat, keine Personendaten).
        let subject = vec![("C".to_string(), "AT".to_string()), ("CN".to_string(), "Max Mustermann".to_string())];
        let issuer = vec![
            ("C".to_string(), "AT".to_string()),
            ("O".to_string(), "PIDA".to_string()),
            ("CN".to_string(), "Beispiel-Zertifikat (nur Vorschau)".to_string()),
        ];
        CertFields {
            subject,
            issuer,
            serial_decimal: "1234567890".to_string(),
            subject_dn: "CN=Max Mustermann,C=AT".to_string(),
            issuer_dn: "CN=Beispiel-Zertifikat (nur Vorschau),O=PIDA,C=AT".to_string(),
        }
    }
}

/// PDF/A-Behandlung des Blocks (Plan §6.9): Teile 1–3 gleich (eingebettete Fonts), Teil 1 ohne
/// Alpha, Teil 2 und 3 ohne `CIDSet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum PdfaMode {
    /// kein PDF/A
    #[default]
    None,
    /// PDF/A-1
    Part1,
    /// PDF/A-2
    Part2,
    /// PDF/A-3
    Part3,
}

/// Eingabe für Layout und Appearance. `Debug` zeigt keinen Hinweistext und keine `sbp`-Werte.
#[derive(Clone, PartialEq)]
pub struct SigBlockInput {
    /// Zertifikatsfelder (bzw. [`CertFields::example`]).
    pub cert: CertFields,
    /// Zeitpunkt für `SIG_DATE` (gleich `/M`).
    pub signing_time: OffsetDateTime,
    /// Hinweistext (`None` = Profilwert).
    pub note: Option<String>,
    /// Bildmarke (`None` = Profil-Default-Logo).
    pub emblem: Option<Arc<Emblem>>,
    /// PDF/A-Behandlung.
    pub pdfa: PdfaMode,
    /// `iui.pdfVersion`.
    pub pdf_version: String,
    /// `sbp.*` (dynamische Parameter; PDF-Over setzt keine).
    pub sbp: BTreeMap<String, String>,
    /// PDF-AS-Quirks exakt nachbilden (nur Golden-Tests, `compat.pdfas_quirks`).
    pub pdfas_quirks: bool,
}

impl std::fmt::Debug for SigBlockInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigBlockInput")
            .field("cert", &self.cert)
            .field("signing_time", &self.signing_time)
            .field("note_len", &self.note.as_ref().map(|n| n.chars().count()))
            .field("emblem", &self.emblem)
            .field("pdfa", &self.pdfa)
            .field("pdf_version", &self.pdf_version)
            .field("sbp", &self.sbp.keys().collect::<Vec<_>>())
            .field("pdfas_quirks", &self.pdfas_quirks)
            .finish()
    }
}

impl SigBlockInput {
    /// Eingabe mit Defaults: kein Hinweis-Override, Profil-Logo, kein PDF/A, `iui.pdfVersion`
    /// = `1.7`, keine `sbp`, ohne PDF-AS-Quirks.
    pub fn new(cert: CertFields, signing_time: OffsetDateTime) -> SigBlockInput {
        SigBlockInput {
            cert,
            signing_time,
            note: None,
            emblem: None,
            pdfa: PdfaMode::None,
            pdf_version: "1.7".to_string(),
            sbp: BTreeMap::new(),
            pdfas_quirks: false,
        }
    }
}
