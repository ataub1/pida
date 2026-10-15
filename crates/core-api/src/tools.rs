//! Werkzeuge für die CLI-Subcommands `verify`, `profiles`, `render-block` (Plan §7.4).

use std::path::Path;

use pdfover_cades::{AlgorithmPolicy, Expect, ExpectedSigner};
use pdfover_pdf_cos::{Limits, Object, SourcePdf};
use pdfover_render::RenderBackend;
use pdfover_sigblock::{FontProvider, ProfileStore};

use crate::CoreError;
use crate::plan::{self, PlanError};

/// Ergebnis der Prüfung einer Signatur (`verify`). `Debug` ohne Signernamen (Regel 9).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignatureCheck {
    /// Feldname
    pub field: String,
    /// CMS gültig (message-digest, Signaturwert, signing-certificate)
    pub valid: bool,
    /// Fehlercode (`cms-…`, `ng-verify-…`), wenn ungültig
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
            .field("valid", &self.valid)
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
            valid: false,
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
        let len = bytes.len() as i64;
        let valid_range = br.len() == 4
            && br.iter().all(|v| *v >= 0)
            && br[0] + br[1] <= len
            && br[2] + br[3] <= len
            && br[0] + br[1] <= br[2];
        if !valid_range {
            check.error = Some("ng-verify-byte-range".into());
            out.push(check);
            continue;
        }
        check.covers_whole_file = br[0] == 0 && br[2] + br[3] == len;
        let (a, b, c, d) = (br[0] as usize, br[1] as usize, br[2] as usize, br[3] as usize);
        let cms = match pdfover_cades::trim_contents_padding(&contents) {
            Ok(c) => c,
            Err(e) => {
                check.error = Some(e.code().into());
                out.push(check);
                continue;
            }
        };
        let mut data = std::io::Read::chain(&bytes[a..a + b], &bytes[c..c + d]);
        let expect = Expect {
            signer: ExpectedSigner::Any,
            require_signing_certificate: false,
            max_len: None,
            policy: AlgorithmPolicy::PriorSignature,
        };
        match pdfover_cades::verify_detached(cms, &mut data, &expect) {
            Ok(report) => {
                check.valid = true;
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
    pdfover_sigblock::PDFOVER_PROFILE_IDS
        .iter()
        .map(|id| ((*id).to_owned(), store.as_ref().is_some_and(|s| s.get(id).is_some())))
        .collect()
}

/// Signaturblock eines Profils als eigenständiges Mini-PDF (Beispielzertifikat, A4-Breite).
pub fn block_pdf(profile_id: &str) -> Result<Vec<u8>, PlanError> {
    let store = ProfileStore::builtin()?;
    let fonts = FontProvider::builtin();
    let profile = store.profile(profile_id)?.clone();
    let mut input = pdfover_sigblock::SigBlockInput::new(pdfover_sigblock::CertFields::example(), plan::local_now());
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
    let backend = pdfover_render::PdfiumBackend::bind(pdfium_dir)?;
    let r = backend.open(std::sync::Arc::from(pdf), None)?;
    let img = r.render_page(0, scale)?;
    Ok(pdfover_render::encode_png(&img)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata");

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
        assert!(checks.iter().all(|c| c.valid), "{checks:?}");
        assert!(checks.iter().any(|c| c.covers_whole_file));
        // unsigniert: keine Signaturen
        let p = Path::new(TESTDATA).join("pdf/generated/simple-xref-table.pdf");
        assert!(verify_file(&p).unwrap().is_empty());
    }

    #[test]
    fn verify_detects_tampering() {
        let p = Path::new(TESTDATA).join("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf");
        let mut bytes = std::fs::read(&p).unwrap();
        // ein Byte am Anfang (im signierten Bereich) ändern
        let pos = bytes.iter().position(|b| *b == b'\n').unwrap() + 1;
        bytes[pos] ^= 0x01;
        let tmp = tempfile::tempdir().unwrap();
        let t = tmp.path().join("t.pdf");
        std::fs::write(&t, &bytes).unwrap();
        if let Ok(checks) = verify_file(&t) {
            assert!(checks.iter().any(|c| !c.valid), "{checks:?}");
        }
    }
}
