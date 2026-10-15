//! Ein Signierfall mit allen Orakeln (Modell A) und eine Ergebnistabelle.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use pida_cades::CmsProfile;
use pida_pades::{CheckContext, DocReport, PlanOptions, PlannedSignature, SignedPdf, prepare};
use pida_pdf_cos::{Limits, SourcePdf};
use pida_placement::visible_block_rect;

use super::{
    KeystoreSigner, Look, Oracles, PyhankoSig, check_output, evaluate_pyhanko, open_renderer, plan_for, pyhanko_batch,
};

/// Ergebnis eines Falls.
pub struct Outcome {
    pub label: String,
    pub original: PathBuf,
    pub out: Option<PathBuf>,
    pub field: String,
    pub prior: usize,
    pub qpdf: String,
    pub openssl: String,
    pub pixel: String,
    pub new_sig: String,
    pub prior_after: String,
    pub notices: String,
    pub problems: Vec<String>,
    pub signed: Option<SignedPdf>,
}

impl Outcome {
    pub fn new(label: impl Into<String>, original: &Path) -> Self {
        Outcome {
            label: label.into(),
            original: original.to_path_buf(),
            out: None,
            field: String::new(),
            prior: 0,
            qpdf: "–".into(),
            openssl: "–".into(),
            pixel: "–".into(),
            new_sig: "–".into(),
            prior_after: "–".into(),
            notices: "–".into(),
            problems: Vec::new(),
            signed: None,
        }
    }
}

/// Plant (mit `adjust` für die Optionen), bereitet vor, signiert (Keystore), prüft
/// (`self_check`, qpdf, openssl, PDFium) und schreibt `work/<name>.pdf`.
#[allow(clippy::too_many_arguments)]
pub fn run_case(
    or: &Oracles,
    signer: &mut KeystoreSigner,
    original_path: &Path,
    src_bytes: &[u8],
    profile: &str,
    cms: CmsProfile,
    adjust: &dyn Fn(&mut PlanOptions, &DocReport, &SourcePdf),
    work: &Path,
    name: &str,
    label: &str,
) -> Outcome {
    let mut o = Outcome::new(label, original_path);
    let src = match SourcePdf::open(src_bytes.to_vec(), &Limits::default()) {
        Ok(s) => s,
        Err(e) => {
            o.problems.push(format!("öffnen: {e}"));
            return o;
        }
    };
    let report = match pida_pades::analyze(&src) {
        Ok(r) => r,
        Err(e) => {
            o.problems.push(format!("analyze: {e}"));
            return o;
        }
    };
    o.prior = report.signatures.len();
    let mut opts = PlanOptions::new(pida_pades::local_signing_time());
    opts.cms_profile = cms;
    adjust(&mut opts, &report, &src);
    let renderer = open_renderer(&src);
    let chain = signer.chain.clone();
    let planned: PlannedSignature = match plan_for(&src, &report, profile, &chain.signer, &opts, renderer.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            o.problems.push(format!("plan: {e} ({})", e.code()));
            return o;
        }
    };
    let prepared = match prepare(&src, &planned.plan, Some(&chain)) {
        Ok(p) => p,
        Err(e) => {
            o.problems.push(format!("prepare: {e} ({})", e.code()));
            return o;
        }
    };
    o.field = prepared.field_name().to_string();
    let mut notes: Vec<String> = prepared.notices().to_vec();
    notes.extend(planned.notices.iter().map(|n| n.code().to_string()));
    if !notes.is_empty() {
        o.notices = notes.join(", ");
    }
    let signed = match signer.sign(prepared) {
        Ok(s) => s,
        Err(e) => {
            o.problems.push(format!("signieren: {e}"));
            return o;
        }
    };
    let ctx = CheckContext { source: &src, expected_signer: Some(&chain.signer), renderer: or.renderer() };
    if let Err(v) = signed.self_check(&ctx) {
        o.problems.push(format!("self_check: {v}"));
    }
    let out = work.join(format!("{name}.pdf"));
    std::fs::write(&out, signed.bytes()).unwrap();
    if let Some(keep) = std::env::var_os("PIDA_KEEP") {
        let _ = std::fs::copy(&out, PathBuf::from(keep).join(format!("{name}.pdf")));
    }
    let look = match (&planned.position, &planned.plan.visibility) {
        (Some(rp), _) => {
            Some(Look::Visible { page: rp.page_index, rect: visible_block_rect(rp), new_page: rp.new_page })
        }
        (None, pida_pades::Visibility::Invisible) => Some(Look::Invisible { page: 0 }),
        // vorhandenes Feld: Widget-Rect (Benutzerraum) → sichtbarer Raum, nur bei ungedrehter Seite
        (None, pida_pades::Visibility::Visible(bp)) => {
            src.pages().get(bp.page_index).filter(|p| p.rotation == pida_pdf_cos::Rotation::R0).map(|p| {
                let c = p.crop_box.normalized();
                let r = bp.rect.normalized();
                Look::Visible {
                    page: bp.page_index,
                    rect: pida_pdf_cos::Rect::new(r.llx - c.llx, r.lly - c.lly, r.urx - c.llx, r.ury - c.lly),
                    new_page: false,
                }
            })
        }
    };
    // Ein Ausgangsdokument, das nur im Speicher liegt, für qpdf als Datei ablegen.
    let orig_file = work.join(format!("{name}.orig.pdf"));
    std::fs::write(&orig_file, src_bytes).unwrap();
    let (q, ssl, p) = check_output(or, &orig_file, src_bytes, &signed, &out, look, work, &mut o.problems);
    o.qpdf = q;
    o.openssl = ssl;
    o.pixel = p;
    o.out = Some(out);
    o.original = orig_file;
    o.signed = Some(signed);
    o
}

/// pyHanko für alle Fälle (ein Lauf) und Bewertung.
pub fn pyhanko_all(or: &Oracles, outcomes: &mut [Outcome], trust: &[PathBuf]) {
    let Some(py) = &or.python else { return };
    let outs: Vec<PathBuf> = outcomes.iter().filter_map(|o| o.out.clone()).collect();
    if outs.is_empty() {
        return;
    }
    let after = pyhanko_batch(py, &outs, trust).expect("pyHanko");
    let originals: Vec<PathBuf> =
        outcomes.iter().filter(|o| o.prior > 0 && o.out.is_some()).map(|o| o.original.clone()).collect();
    let before =
        if originals.is_empty() { Default::default() } else { pyhanko_batch(py, &originals, &[]).expect("pyHanko") };
    for o in outcomes.iter_mut().filter(|o| o.out.is_some()) {
        let a: &Vec<PyhankoSig> = &after[&o.out.as_ref().unwrap().to_string_lossy().to_string()];
        let b = before.get(&o.original.to_string_lossy().to_string()).cloned().unwrap_or_default();
        let (n, p) = evaluate_pyhanko(&b, a, &o.field, &mut o.problems);
        o.new_sig = n;
        o.prior_after = p;
    }
}

/// Tabelle (Markdown) der Fälle.
pub fn table(outcomes: &[Outcome]) -> String {
    let mut md = String::from(
        "| Fall | Vorsig. | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |\n|---|---|---|---|---|---|---|---|---|\n",
    );
    for o in outcomes {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            o.label,
            o.prior,
            o.qpdf,
            o.openssl,
            o.pixel,
            o.new_sig,
            o.prior_after,
            o.notices,
            if o.problems.is_empty() { "ok".to_string() } else { format!("**FEHLER**: {}", o.problems.join("; ")) }
        ));
    }
    md
}

/// Bricht mit allen Problemen ab.
pub fn assert_all_ok(outcomes: &[Outcome]) {
    let failed: Vec<String> =
        outcomes.iter().filter(|o| !o.problems.is_empty()).map(|o| format!("{}: {:?}", o.label, o.problems)).collect();
    assert!(failed.is_empty(), "{} Fehler:\n{}", failed.len(), failed.join("\n"));
}
