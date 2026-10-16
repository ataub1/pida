//! Prüfmodus (ADR 0004 §3.9): alle Signaturen eines Dokuments mit dem eigenen Verifier prüfen und die
//! Angaben für die Anzeige liefern.
//!
//! Geprüft wird wie bei Vorsignaturen (Invariante I4, `cades::verify_detached` mit
//! [`AlgorithmPolicy::PriorSignature`](pida_cades::AlgorithmPolicy)): message-digest über die
//! ByteRange-Daten, Signaturwert, signing-certificate(-v2), falls vorhanden. **Nicht** geprüft werden
//! Vertrauenskette, Sperrstatus und Vertrauensliste – der Prüfmodus behauptet deshalb nie
//! „gültig“/„rechtsgültig“ (ADR 0004 §8 F3), sondern „unverändert und kryptografisch intakt“.
//!
//! | Befund | [`Integrity`] | Code |
//! |---|---|---|
//! | Prüfung bestanden (auch SHA-1, kurze RSA-Schlüssel – dann mit Hinweis in `warnings`) | `Intact` | – |
//! | message-digest bzw. Signaturwert passt nicht, signing-certificate verweist auf ein anderes Zertifikat | `Broken` | `cms-…` |
//! | `/ByteRange` nicht `[0 a b c]`, überlappend oder über das Dateiende | `Broken` | `pdf-signature-byterange` |
//! | kein lesbares Signaturdictionary | `Broken` | `pdf-signature-dictionary` |
//! | Verfahren nicht prüfbar (`adbe.pkcs7.sha1`, Dokument-Zeitstempel `ETSI.RFC3161`, unbekannter `/SubFilter`) | `NotVerifiable` | `pdf-signature-subfilter` |
//! | Algorithmus nicht unterstützt (z. B. DSA, unbekannte Kurve) | `NotVerifiable` | `cms-unsupported-algorithm` |
//!
//! Reihenfolge der Signaturen: nach dem Ende des signierten Bereichs (zuerst signiert = zuerst), bei
//! gleichem Ende in Feldreihenfolge.

use pida_cades::{Certificate, CmsWarning, DigestAlg, SignatureAlg};
use pida_pdf_cos::{Rect, SigField, SourcePdf, TailKind};
use time::OffsetDateTime;

use crate::PadesError;
use crate::signed::PriorStatus;
use crate::sigs::{self, SigDictInfo};

/// [`Integrity::Broken`]: kein lesbares Signaturdictionary.
pub const CODE_SIGNATURE_DICTIONARY: &str = "pdf-signature-dictionary";
/// [`Integrity::Broken`]: `/ByteRange` ungültig.
pub const CODE_SIGNATURE_BYTERANGE: &str = "pdf-signature-byterange";

/// Ergebnis der Prüfung einer Signatur.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Integrity {
    /// unverändert und kryptografisch intakt
    Intact,
    /// verändert bzw. ungültig (Code `cms-…`, `pdf-signature-…`)
    Broken {
        /// stabiler Code
        code: String,
    },
    /// nicht prüfbar (Code `pdf-signature-subfilter`, `cms-unsupported-algorithm`)
    NotVerifiable {
        /// stabiler Code
        code: String,
    },
}

/// Bericht über alle Signaturen eines Dokuments.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifyReport {
    /// Revisionen der Datei (≥ 1)
    pub revisions: usize,
    /// Signaturen in Signierreihenfolge
    pub signatures: Vec<SignatureVerification>,
}

impl VerifyReport {
    /// Die Datei wurde nach der letzten Signatur (inkl. Dokument-Zeitstempel) noch ergänzt bzw. geändert:
    /// keine Signatur deckt die ganze Datei ab (ohne Signaturen `false`).
    pub fn modified_after_last_signature(&self) -> bool {
        !self.signatures.is_empty() && !self.signatures.iter().any(|s| s.covers_whole_file)
    }
}

/// Eine geprüfte Signatur. `Debug` ohne Feldnamen und Zertifikatsinhalte (Personendaten, Regel 9).
#[derive(Clone, PartialEq)]
pub struct SignatureVerification {
    /// voll qualifizierter Feldname
    pub field_name: String,
    /// `/SubFilter`
    pub sub_filter: Option<String>,
    /// Dokument-Zeitstempel (`/Type /DocTimeStamp`)
    pub doc_timestamp: bool,
    /// Prüfergebnis
    pub integrity: Integrity,
    /// der signierte Bereich reicht bis zum Ende der Datei (letzte Revision, ohne Fremdinhalt danach)
    pub covers_whole_file: bool,
    /// Revision, an deren Ende der signierte Bereich endet
    pub covers_revision: Option<usize>,
    /// Ende des signierten Bereichs (Byte-Offset), falls lesbar
    pub covered_end: Option<u64>,
    /// Revisionen nach dem signierten Bereich
    pub later_revisions: usize,
    /// Signaturen in späteren Revisionen
    pub later_signatures: usize,
    /// Signerzertifikat (bei bestandener Prüfung das geprüfte, sonst das im CMS über die SignerInfo
    /// referenzierte bzw. – ohne Zuordnung – das einzige Nicht-CA-Zertifikat)
    pub signer: Option<Certificate>,
    /// `signing-time` (Angabe des Signierenden, kein geprüfter Zeitstempel)
    pub signing_time: Option<OffsetDateTime>,
    /// Hashverfahren
    pub digest_alg: Option<DigestAlg>,
    /// Signaturverfahren
    pub signature_alg: Option<SignatureAlg>,
    /// Hinweise zu einer intakten Signatur (SHA-1, signing-certificate v1, kurzer RSA-Schlüssel …)
    pub warnings: Vec<CmsWarning>,
    /// Seite und `/Rect` des Widgets (Benutzerraum der Seite); `None` = unsichtbar bzw. ohne Seite
    pub widget: Option<(usize, Rect)>,
}

impl std::fmt::Debug for SignatureVerification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureVerification")
            .field("field_name_len", &self.field_name.chars().count())
            .field("sub_filter", &self.sub_filter)
            .field("doc_timestamp", &self.doc_timestamp)
            .field("integrity", &self.integrity)
            .field("covers_whole_file", &self.covers_whole_file)
            .field("covers_revision", &self.covers_revision)
            .field("later_revisions", &self.later_revisions)
            .field("later_signatures", &self.later_signatures)
            .field("signer", &self.signer)
            .field("signing_time", &self.signing_time)
            .field("digest_alg", &self.digest_alg)
            .field("signature_alg", &self.signature_alg)
            .field("warnings", &self.warnings)
            .field("widget", &self.widget)
            .finish()
    }
}

/// Prüft alle Signaturen eines geöffneten Dokuments (hasht je Signatur die abgedeckten Bytes).
pub fn verify_document(src: &SourcePdf) -> Result<VerifyReport, PadesError> {
    let fields = src.signature_fields()?;
    let data = src.bytes();
    let len = src.len();
    let revisions = src.revisions();
    let tail_garbage = src.tail().kind == TailKind::Garbage;
    let mut out: Vec<(usize, SignatureVerification)> = Vec::new();
    for (order, f) in fields.iter().filter(|f| f.signed).enumerate() {
        let widget = visible_widget(f);
        let Some(info) = sigs::read_sig_dict(src, f) else {
            out.push((order, unreadable(f, widget)));
            continue;
        };
        let covered_end = info.covered_end();
        let covers_revision = covered_end.and_then(|e| crate::analyze::revision_ending_at(src, e));
        let last = revisions.len().saturating_sub(1);
        let covers_whole_file =
            covered_end == Some(len) || (covers_revision == Some(last) && !tail_garbage && !revisions.is_empty());
        let later_revisions = match (covers_revision, covered_end) {
            (Some(i), _) => last.saturating_sub(i),
            (None, Some(end)) => revisions.iter().filter(|r| r.end_offset > end).count(),
            (None, None) => 0,
        };
        let mut v = SignatureVerification {
            field_name: f.qualified_name.clone(),
            sub_filter: info.sub_filter.clone(),
            doc_timestamp: info.kind.as_deref() == Some(b"DocTimeStamp"),
            integrity: Integrity::Intact,
            covers_whole_file,
            covers_revision,
            covered_end,
            later_revisions,
            later_signatures: 0,
            signer: None,
            signing_time: None,
            digest_alg: None,
            signature_alg: None,
            warnings: Vec::new(),
            widget,
        };
        check(data, &info, &mut v);
        out.push((order, v));
    }
    // Signierreihenfolge: Ende des signierten Bereichs, unlesbare zuletzt
    out.sort_by_key(|(order, v)| (v.covered_end.unwrap_or(u64::MAX), *order));
    let ends: Vec<Option<u64>> = out.iter().map(|(_, v)| v.covered_end).collect();
    let mut signatures: Vec<SignatureVerification> = out.into_iter().map(|(_, v)| v).collect();
    for v in &mut signatures {
        if let Some(end) = v.covered_end {
            v.later_signatures = ends.iter().filter(|e| e.is_some_and(|e| e > end)).count();
        }
    }
    Ok(VerifyReport { revisions: revisions.len().max(1), signatures })
}

/// `/V` ohne lesbares Signaturdictionary.
fn unreadable(f: &SigField, widget: Option<(usize, Rect)>) -> SignatureVerification {
    SignatureVerification {
        field_name: f.qualified_name.clone(),
        sub_filter: None,
        doc_timestamp: false,
        integrity: Integrity::Broken { code: CODE_SIGNATURE_DICTIONARY.into() },
        covers_whole_file: false,
        covers_revision: None,
        covered_end: None,
        later_revisions: 0,
        later_signatures: 0,
        signer: None,
        signing_time: None,
        digest_alg: None,
        signature_alg: None,
        warnings: Vec::new(),
        widget,
    }
}

/// `/ByteRange [0 a b c]` mit genau zwei Abschnitten, Beginn bei 0, ohne Überlappung, innerhalb der Datei.
/// Strenger als die Prüfung von Vorsignaturen beim Signieren: ein Bereich, der nicht bei 0 beginnt, ließe
/// den Anfang der Datei ungeschützt.
fn byte_range_well_formed(info: &SigDictInfo, len: u64) -> bool {
    let Some(br) = info.byte_range() else { return false };
    let [a, b, c, d] = br.0;
    a == 0 && b.checked_add(a).is_some_and(|e| e <= c) && c.checked_add(d).is_some_and(|e| e <= len)
}

/// Kryptografische Prüfung und Angaben für die Anzeige.
fn check(data: &[u8], info: &SigDictInfo, v: &mut SignatureVerification) {
    let d = sigs::signature_details(data, info);
    let structure_ok = byte_range_well_formed(info, data.len() as u64);
    match (&d.report, &d.status) {
        (Some(report), PriorStatus::Valid) if structure_ok => {
            v.integrity = Integrity::Intact;
            v.signer = Some(report.signer.clone());
            v.signing_time = report.signing_time;
            v.digest_alg = Some(report.digest_alg);
            v.signature_alg = Some(report.signature_alg);
            v.warnings = report.warnings.clone();
            return;
        }
        (Some(report), _) => {
            // kryptografisch passend, aber der signierte Bereich lässt Teile der Datei offen
            v.integrity = Integrity::Broken { code: CODE_SIGNATURE_BYTERANGE.into() };
            v.signer = Some(report.signer.clone());
            v.signing_time = report.signing_time;
            v.digest_alg = Some(report.digest_alg);
            v.signature_alg = Some(report.signature_alg);
            return;
        }
        (None, _) => {}
    }
    let code = d.code.clone().unwrap_or_else(|| CODE_SIGNATURE_BYTERANGE.into());
    v.integrity = match d.status {
        PriorStatus::NotVerifiable => Integrity::NotVerifiable { code },
        _ => Integrity::Broken { code },
    };
    if let Some(cms) = d.info {
        v.signer = cms.signer.clone().or_else(|| single_end_entity(&cms.certificates));
        v.signing_time = cms.signing_time;
        v.digest_alg = cms.digest_alg;
        v.signature_alg = cms.signature_alg;
    }
}

/// Ohne Zuordnung zum SignerInfo: das Zertifikat nur, wenn genau eines ohne CA-Eigenschaft enthalten ist.
fn single_end_entity(certs: &[Certificate]) -> Option<Certificate> {
    let mut ee = certs.iter().filter(|c| !c.is_ca());
    match (ee.next(), ee.next()) {
        (Some(one), None) => Some(one.clone()),
        _ => None,
    }
}

/// Sichtbares Widget: erstes mit Seite, Fläche ≥ 1 pt × 1 pt und ohne `Hidden`/`NoView`.
fn visible_widget(f: &SigField) -> Option<(usize, Rect)> {
    const HIDDEN: u32 = 1 << 1;
    const NO_VIEW: u32 = 1 << 5;
    f.widgets.iter().find_map(|w| {
        let page = w.page_index?;
        let r = w.rect.normalized();
        let finite = [r.llx, r.lly, r.urx, r.ury].iter().all(|v| v.is_finite());
        (finite && r.width() >= 1.0 && r.height() >= 1.0 && w.annot_flags & (HIDDEN | NO_VIEW) == 0)
            .then_some((page, r))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_pdf_cos::SigWidget;

    fn field(widgets: Vec<SigWidget>) -> SigField {
        SigField {
            qualified_name: "Signatur".into(),
            partial_name: Some("Signatur".into()),
            field_id: (1, 0),
            depth: 0,
            widgets,
            signed: true,
            value_id: None,
            has_lock: false,
            has_seed_value: false,
            field_flags: 0,
        }
    }

    fn widget(page: Option<usize>, rect: Rect, flags: u32) -> SigWidget {
        SigWidget { id: (2, 0), page_index: page, rect, annot_flags: flags }
    }

    #[test]
    fn widgets_without_area_page_or_visibility_are_invisible() {
        let r = Rect { llx: 300.0, lly: 100.0, urx: 100.0, ury: 50.0 };
        let visible = field(vec![widget(Some(1), r, 4)]);
        assert_eq!(visible_widget(&visible), Some((1, r.normalized())));
        let zero = Rect { llx: 0.0, lly: 0.0, urx: 0.0, ury: 0.0 };
        assert_eq!(visible_widget(&field(vec![widget(Some(0), zero, 4)])), None);
        assert_eq!(visible_widget(&field(vec![widget(None, r, 4)])), None);
        assert_eq!(visible_widget(&field(vec![widget(Some(0), r, 2)])), None, "Hidden");
        assert_eq!(visible_widget(&field(vec![widget(Some(0), r, 32)])), None, "NoView");
        let nan = Rect { llx: f32::NAN, ..r };
        assert_eq!(visible_widget(&field(vec![widget(Some(0), nan, 4)])), None);
        // das erste sichtbare von mehreren Widgets
        assert_eq!(
            visible_widget(&field(vec![widget(Some(0), zero, 4), widget(Some(2), r, 0)])),
            Some((2, r.normalized()))
        );
    }

    #[test]
    fn byte_range_must_start_at_zero_and_stay_inside() {
        let info = |br: Vec<i64>| SigDictInfo {
            id: None,
            kind: Some(b"Sig".to_vec()),
            sub_filter: Some("ETSI.CAdES.detached".into()),
            byte_range_raw: Some(br),
            contents: Vec::new(),
            location: None,
        };
        assert!(byte_range_well_formed(&info(vec![0, 10, 20, 5]), 25));
        assert!(!byte_range_well_formed(&info(vec![1, 10, 20, 5]), 25), "Anfang ungeschützt");
        assert!(!byte_range_well_formed(&info(vec![0, 21, 20, 5]), 25), "überlappend");
        assert!(!byte_range_well_formed(&info(vec![0, 10, 20, 6]), 25), "über das Dateiende");
        assert!(!byte_range_well_formed(&info(vec![0, 10, 20, 5, 30, 1]), 31), "drei Abschnitte");
        assert!(!byte_range_well_formed(&info(vec![0, i64::MAX, i64::MAX, 1]), 25));
    }

    #[test]
    fn single_end_entity_needs_exactly_one_non_ca() {
        let read = |n: &str| {
            let p = format!("{}/../../testdata/keystores/{n}", env!("CARGO_MANIFEST_DIR"));
            Certificate::from_der(&std::fs::read(p).unwrap()).unwrap()
        };
        let a = read("test-rsa2048.crt.der");
        let b = read("test-ecp256.crt.der");
        assert_eq!(single_end_entity(std::slice::from_ref(&a)), Some(a.clone()));
        assert_eq!(single_end_entity(&[a, b]), None);
        assert_eq!(single_end_entity(&[]), None);
    }
}
