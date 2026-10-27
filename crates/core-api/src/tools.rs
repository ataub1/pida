//! Werkzeuge für die CLI-Subcommands `verify`, `profiles`, `render-block` (Plan §7.4).
//!
//! `verify` ist ein Entwickler-Werkzeug (QA, Golden- und Interop-Tests): Es prüft, ob eine Signatur unverändert
//! und kryptografisch intakt ist, nicht Vertrauenskette und Sperrstatus. Die App hat keinen Prüfmodus mehr
//! (ADR 0004 Nachtrag 15); wie PDF-Over verweist sie auf signaturpruefung.gv.at.

use std::path::Path;

use pida_cades::{AlgorithmPolicy, Expect, ExpectedSigner};
use pida_pdf_cos::{Limits, Object, SourcePdf};
use pida_render::RenderBackend;
use pida_sigblock::{FontProvider, ProfileStore};

use crate::CoreError;
use crate::plan::{self, PlanError};

/// Ergebnis der Prüfung einer Signatur (`verify`). `Debug` ohne Signernamen (Regel 9).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignatureCheck {
    /// Feldname
    pub field: String,
    /// unverändert und kryptografisch intakt (message-digest, Signaturwert, signing-certificate, signierter
    /// Bereich); Vertrauenskette und Sperrstatus sind **nicht** geprüft
    pub intact: bool,
    /// Fehlercode (`cms-…`, `ng-verify-…`), wenn nicht intakt
    pub error: Option<String>,
    /// Common Name des Signers
    pub signer: Option<String>,
    /// `signing-time` (RFC 3339), falls vorhanden
    pub signing_time: Option<String>,
    /// ByteRange deckt die ganze Datei ab (letzte Signatur)
    pub covers_whole_file: bool,
}

impl std::fmt::Debug for SignatureCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureCheck")
            .field("field", &self.field)
            .field("intact", &self.intact)
            .field("error", &self.error)
            .field("signer", &self.signer.as_ref().map(|s| s.chars().count()))
            .field("covers_whole_file", &self.covers_whole_file)
            .finish_non_exhaustive()
    }
}

fn as_i64(o: &Object) -> Option<i64> {
    match o {
        Object::Integer(i) => Some(*i),
        _ => None,
    }
}

/// `/ByteRange [a b c d]` → die beiden signierten Bereiche, wenn sie in der Datei liegen und sich nicht
/// überlappen (`a + b ≤ c`). Überlaufsicher: Werte aus einem feindlichen PDF (z. B. `i64::MAX`) führen zu
/// `None` statt zu einer Panik beim Slicen.
fn byte_range_slices(br: &[i64], len: usize) -> Option<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let [a, b, c, d] = br else { return None };
    let conv = |v: i64| usize::try_from(v).ok();
    let (a, b, c, d) = (conv(*a)?, conv(*b)?, conv(*c)?, conv(*d)?);
    let first_end = a.checked_add(b)?;
    let second_end = c.checked_add(d)?;
    (first_end <= c && second_end <= len).then_some((a..first_end, c..second_end))
}

/// Prüft alle Signaturen eines PDFs mit dem eigenen Verifier (Algorithmuspolitik für
/// Vorsignaturen, ohne Vertrauensanker – QA-Werkzeug, Selbstprüfung wie I3/I4).
pub fn verify_file(path: &Path) -> Result<Vec<SignatureCheck>, CoreError> {
    let src = SourcePdf::open_file(path, &Limits::default()).map_err(|e| CoreError::Verify(e.to_string()))?;
    let bytes = src.bytes();
    let fields = src.signature_fields().map_err(|e| CoreError::Verify(e.to_string()))?;
    let mut out = Vec::new();
    for f in fields.iter().filter(|f| f.signed) {
        let mut check = SignatureCheck {
            field: f.qualified_name.clone(),
            intact: false,
            error: None,
            signer: None,
            signing_time: None,
            covers_whole_file: false,
        };
        let dict = f.value_id.and_then(|id| src.get_object(id).ok()).and_then(|o| o.as_dict().ok());
        let Some(dict) = dict else {
            check.error = Some("ng-verify-no-signature-dictionary".into());
            out.push(check);
            continue;
        };
        let br: Option<Vec<i64>> = dict
            .get(b"ByteRange")
            .ok()
            .and_then(|o| src.resolve(o).ok())
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(as_i64).collect());
        let contents = dict.get(b"Contents").ok().and_then(|o| src.resolve(o).ok()).and_then(|o| match o {
            Object::String(s, _) => Some(s.clone()),
            _ => None,
        });
        let (Some(br), Some(contents)) = (br, contents) else {
            check.error = Some("ng-verify-no-byte-range".into());
            out.push(check);
            continue;
        };
        let Some((first, second)) = byte_range_slices(&br, bytes.len()) else {
            check.error = Some("ng-verify-byte-range".into());
            out.push(check);
            continue;
        };
        check.covers_whole_file = first.start == 0 && second.end == bytes.len();
        let cms = match pida_cades::trim_contents_padding(&contents) {
            Ok(c) => c,
            Err(e) => {
                check.error = Some(e.code().into());
                out.push(check);
                continue;
            }
        };
        let mut data = std::io::Read::chain(&bytes[first.clone()], &bytes[second.clone()]);
        let expect = Expect {
            signer: ExpectedSigner::Any,
            require_signing_certificate: false,
            max_len: None,
            policy: AlgorithmPolicy::PriorSignature,
        };
        match pida_cades::verify_detached(cms, &mut data, &expect) {
            // der signierte Bereich lässt Teile der Datei offen (Anfang bzw. Bytes neben /Contents in der Lücke):
            // wie der Inspektor nicht intakt (`pades::gap_is_hex_string`)
            Ok(_) if first.start != 0 || !pida_pades::gap_is_hex_string(&bytes[first.end..second.start], &contents) => {
                check.error = Some("ng-verify-byte-range".into());
            }
            Ok(report) => {
                check.intact = true;
                check.signer = report.signer.subject_cn();
                check.signing_time =
                    report.signing_time.and_then(|t| t.format(&time::format_description::well_known::Rfc3339).ok());
            }
            Err(e) => check.error = Some(e.code().into()),
        }
        out.push(check);
    }
    Ok(out)
}

/// Die 12 Profil-IDs von PDF-Over mit Verfügbarkeit im eingebauten Profilspeicher.
pub fn profile_ids() -> Vec<(String, bool)> {
    let store = ProfileStore::builtin().ok();
    pida_sigblock::PDFOVER_PROFILE_IDS
        .iter()
        .map(|id| ((*id).to_owned(), store.as_ref().is_some_and(|s| s.get(id).is_some())))
        .collect()
}

/// Signaturblock eines Profils als eigenständiges Mini-PDF (Beispielzertifikat, A4-Breite).
pub fn block_pdf(profile_id: &str) -> Result<Vec<u8>, PlanError> {
    let store = ProfileStore::builtin()?;
    let fonts = FontProvider::builtin();
    let profile = store.profile(profile_id)?.clone();
    let mut input = pida_sigblock::SigBlockInput::new(pida_sigblock::CertFields::example(), plan::local_now());
    let pdfa = plan::pdfa_mode(None, &profile);
    input.pdfa = pdfa;
    let spec = plan::BlockSpec { profile_id: profile_id.to_owned(), profile, input, pdfa };
    let pages = [plan::a4_page()];
    let geom = plan::block_geometry(&spec, &pages, 0, &store, &fonts)?;
    let app = spec.build(geom.width, 0, &store, &fonts)?;
    Ok(app.to_standalone_pdf()?)
}

/// Rendert die erste Seite eines PDFs als PNG (PDFium aus `pdfium_dir` bzw. Standardsuche).
pub fn render_png(pdf: &[u8], scale: f32, pdfium_dir: Option<&Path>) -> Result<Vec<u8>, CoreError> {
    let backend = pida_render::PdfiumBackend::bind(pdfium_dir)?;
    let r = backend.open(std::sync::Arc::from(pdf), None)?;
    let img = r.render_page(0, scale)?;
    Ok(pida_render::encode_png(&img)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata");

    #[test]
    fn byte_range_is_checked_without_overflow() {
        assert_eq!(byte_range_slices(&[0, 10, 20, 5], 25), Some((0..10, 20..25)));
        // Überlauf, negative Werte, außerhalb der Datei, überlappend, falsche Anzahl
        assert_eq!(byte_range_slices(&[i64::MAX, 1, 0, 0], 25), None);
        assert_eq!(byte_range_slices(&[0, i64::MAX, i64::MAX, i64::MAX], 25), None);
        assert_eq!(byte_range_slices(&[1, i64::MAX, 0, 0], usize::MAX), None);
        assert_eq!(byte_range_slices(&[-1, 10, 20, 5], 25), None);
        assert_eq!(byte_range_slices(&[0, 10, 20, 6], 25), None);
        assert_eq!(byte_range_slices(&[0, 21, 20, 5], 25), None);
        assert_eq!(byte_range_slices(&[0, 10, 20], 25), None);
    }

    /// Ein PDF mit absurd großer `/ByteRange` bringt `verify` nicht zum Absturz (vorher Überlauf
    /// `a + b` bzw. Slice außerhalb der Datei).
    #[test]
    fn verify_survives_hostile_byte_range() {
        let p = Path::new(TESTDATA).join("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf");
        let mut bytes = pida_fs::read(&p).unwrap();
        // PDF-AS füllt `/ByteRange [...]` mit Leerzeichen auf: gleiche Länge, xref-Offsets bleiben gültig
        let at = bytes.windows(10).position(|w| w == b"/ByteRange").unwrap();
        let open = at + bytes[at..].iter().position(|b| *b == b'[').unwrap();
        let mut end = open + bytes[open..].iter().position(|b| *b == b']').unwrap() + 1;
        while bytes[end] == b' ' {
            end += 1;
        }
        let hostile = b"[1 9223372036854775807 192702 32395]";
        assert!(hostile.len() <= end - open, "Platz für den Testwert");
        let mut replacement = hostile.to_vec();
        replacement.resize(end - open, b' ');
        bytes[open..end].copy_from_slice(&replacement);
        let tmp = tempfile::tempdir().unwrap();
        let t = tmp.path().join("t.pdf");
        pida_fs::write(&t, &bytes).unwrap();
        let checks = verify_file(&t).expect("Struktur bleibt lesbar");
        assert!(checks.iter().any(|c| !c.intact && c.error.as_deref() == Some("ng-verify-byte-range")), "{checks:?}");
    }

    /// Signature Wrapping (Merge E4): Bytes neben `/Contents` in der Lücke der `/ByteRange` – kryptografisch
    /// passt die Signatur, `pida verify` nennt sie aber wie der Inspektor nicht intakt.
    #[test]
    fn verify_rejects_bytes_hidden_in_the_gap() {
        let p = Path::new(TESTDATA).join("verify/pdfas-signed.pdf");
        let mut bytes = pida_fs::read(&p).unwrap();
        assert!(verify_file(&p).unwrap().iter().all(|c| c.intact));
        let at = bytes.windows(10).position(|w| w == b"/ByteRange").unwrap();
        let open = at + bytes[at..].iter().position(|b| *b == b'[').unwrap() + 1;
        let close = open + bytes[open..].iter().position(|b| *b == b']').unwrap();
        let nums: Vec<usize> =
            std::str::from_utf8(&bytes[open..close]).unwrap().split_whitespace().map(|n| n.parse().unwrap()).collect();
        let gap_end = nums[2];
        let inject = b">/X 1";
        assert!(bytes[gap_end - inject.len()..gap_end - 1].iter().all(|b| *b == b'0'), "Padding am Ende der Lücke");
        bytes[gap_end - inject.len()..gap_end].copy_from_slice(inject);
        let tmp = tempfile::tempdir().unwrap();
        let t = tmp.path().join("t.pdf");
        pida_fs::write(&t, &bytes).unwrap();
        let checks = verify_file(&t).unwrap();
        assert!(checks.iter().all(|c| !c.intact && c.error.as_deref() == Some("ng-verify-byte-range")), "{checks:?}");
    }

    #[test]
    fn profiles_are_all_builtin() {
        let ids = profile_ids();
        assert_eq!(ids.len(), 12);
        assert!(ids.iter().all(|(_, ok)| *ok));
    }

    #[test]
    fn block_pdf_for_visible_profiles() {
        let pdf = block_pdf("SIGNATURBLOCK_SMALL_DE").unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(block_pdf("INVISIBLE").is_err());
        assert!(block_pdf("GIBT_ES_NICHT").is_err());
    }

    #[test]
    fn verify_pdfas_signed_document() {
        let p = Path::new(TESTDATA).join("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf");
        let checks = verify_file(&p).unwrap();
        assert!(!checks.is_empty());
        assert!(checks.iter().all(|c| c.intact), "{checks:?}");
        assert!(checks.iter().any(|c| c.covers_whole_file));
        // unsigniert: keine Signaturen
        let p = Path::new(TESTDATA).join("pdf/generated/simple-xref-table.pdf");
        assert!(verify_file(&p).unwrap().is_empty());
    }

    #[test]
    fn verify_detects_tampering() {
        let p = Path::new(TESTDATA).join("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf");
        let mut bytes = pida_fs::read(&p).unwrap();
        // ein Byte am Anfang (im signierten Bereich) ändern
        let pos = bytes.iter().position(|b| *b == b'\n').unwrap() + 1;
        bytes[pos] ^= 0x01;
        let tmp = tempfile::tempdir().unwrap();
        let t = tmp.path().join("t.pdf");
        pida_fs::write(&t, &bytes).unwrap();
        if let Ok(checks) = verify_file(&t) {
            assert!(checks.iter().any(|c| !c.intact), "{checks:?}");
        }
    }
}
