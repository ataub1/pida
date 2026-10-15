//! Ende-zu-Ende über den Korpus (`testdata/pdf/`): jedes PDF wird mit dem Keystore-Backend
//! (Modell A) signiert – Schlüssel, Profil und CMS-Profil rotieren über die Dateien, sodass jede
//! Dimension vorkommt (die vollständige Matrix steht in `e2e_matrix.rs`).
//!
//! Prüfung je Datei: `analyze` (K5 → `prepare` lehnt ab), Plan über `plan_signature` (PDFium-
//! Rasterscan), `prepare` → Keystore `sign` → `finish_raw` → `self_check` (I1–I6 mit PDFium) →
//! `qpdf --check` (keine neuen Befunde), `openssl cms -verify` über die ByteRange-Daten,
//! PDFium-Render (Block sichtbar im erwarteten Rechteck, sonst nichts geändert), pyHanko
//! (neue Signatur intakt/gültig/vertraut/ganze Datei, Vorsignaturen unverändert gültig, die
//! zuvor vollständige höchstens `FORM_FILLING`).
//!
//! Fehlende Werkzeuge überspringen die jeweilige Prüfung. Ergebnis:
//! `$CARGO_TARGET_TMPDIR/pades-results-korpus.md`, mit `PIDA_UPDATE_RESULTS=1` auch
//! `testdata/pades/RESULTS.md`.

mod common;

use std::path::PathBuf;

use common::*;
use pida_cades::CmsProfile;
use pida_pades::{
    AcceptanceClass, CheckContext, FieldTarget, PadesError, SignPlan, analyze, prepare, reject_open_error,
};
use pida_pdf_cos::{Limits, SourcePdf, XrefKind};
use pida_placement::visible_block_rect;

const PROFILES: [&str; 4] =
    ["INVISIBLE", "SIGNATURBLOCK_SMALL_DE", "SIGNATURBLOCK_SMALL_EN_NOTE", "AMTSSIGNATURBLOCK_DE_RECOMMENDED"];
const PDFA_PROFILE: &str = "SIGNATURBLOCK_SMALL_DE_PDFA";

struct Row {
    file: String,
    xref: String,
    enc: String,
    prior: usize,
    class: String,
    key: String,
    profile: String,
    cms: String,
    result: String,
    qpdf: String,
    openssl: String,
    pixel: String,
    new_sig: String,
    prior_after: String,
    notices: String,
    problems: Vec<String>,
    out: Option<PathBuf>,
    original: PathBuf,
    field: String,
    key_stem: String,
}

fn xref_name(k: XrefKind) -> &'static str {
    match k {
        XrefKind::Table => "Tabelle",
        XrefKind::Stream => "xref-Stream",
        XrefKind::Hybrid => "Hybrid",
    }
}

fn cms_name(p: CmsProfile) -> &'static str {
    match p {
        CmsProfile::PdfasCompat => "pdfas-compat",
        CmsProfile::PadesBB => "pades-b-b",
    }
}

#[test]
fn corpus_model_a() {
    let or = Oracles::detect();
    let work = tempfile::tempdir().unwrap();
    let mut rows: Vec<Row> = Vec::new();
    let mut signers: Vec<KeystoreSigner> = KEYS.iter().map(|(_, stem)| KeystoreSigner::open(stem)).collect();

    for (i, path) in corpus().iter().enumerate() {
        let name = rel(path);
        let (key_label, key_stem) = KEYS[i % 3];
        let cms = if (i / 12) % 2 == 0 { CmsProfile::PdfasCompat } else { CmsProfile::PadesBB };
        let mut row = Row {
            file: name.clone(),
            xref: "–".into(),
            enc: "–".into(),
            prior: 0,
            class: "–".into(),
            key: key_label.into(),
            profile: "–".into(),
            cms: cms_name(cms).into(),
            result: String::new(),
            qpdf: "–".into(),
            openssl: "–".into(),
            pixel: "–".into(),
            new_sig: "–".into(),
            prior_after: "–".into(),
            notices: "–".into(),
            problems: Vec::new(),
            out: None,
            original: path.clone(),
            field: String::new(),
            key_stem: key_stem.into(),
        };
        let src = match SourcePdf::open_file(path, &Limits::default()) {
            Ok(s) => s,
            Err(e) => {
                let f = reject_open_error(&e);
                assert_eq!(f.class, AcceptanceClass::K5Rejected);
                row.class = "K5".into();
                row.result = format!("abgelehnt beim Öffnen ({})", f.code);
                if !name.ends_with("simple-aes256-userpw.pdf") {
                    row.problems.push(format!("unerwarteter Öffnungsfehler: {e}"));
                }
                rows.push(row);
                continue;
            }
        };
        row.xref = xref_name(src.xref_kind()).into();
        row.enc = src.encryption().map_or("–".into(), |e| format!("{:?} (R{})", e.algorithm, e.r));
        let report = analyze(&src).unwrap_or_else(|e| panic!("{name}: analyze: {e}"));
        row.prior = report.signatures.len();
        row.class = format!("{:?}", report.class).chars().take(2).collect();
        // je drei aufeinanderfolgende Dateien: dasselbe Profil mit allen drei Schlüsseln
        let profile = if report.pdfa.is_some() { PDFA_PROFILE } else { PROFILES[(i / 3) % PROFILES.len()] };
        row.profile = profile.into();

        if !report.class.is_signable() {
            // K5: prepare muss ablehnen
            let code = report.findings.iter().find(|f| f.class == AcceptanceClass::K5Rejected).unwrap().code.clone();
            let chain = signers[i % 3].chain.clone();
            match prepare(&src, &SignPlan::invisible(pida_pades::local_signing_time()), Some(&chain)) {
                Err(PadesError::Rejected(c)) if c == code => row.result = format!("abgelehnt ({code})"),
                other => row.problems.push(format!("K5 nicht abgelehnt: {other:?}")),
            }
            rows.push(row);
            continue;
        }

        let signer = &mut signers[i % 3];
        let chain = signer.chain.clone();
        let (prepared, planned) =
            match prepare_with(&src, &report, profile, &chain.signer, &chain, cms, FieldTarget::New { name: None }) {
                Ok(x) => x,
                Err(e) => {
                    row.problems.push(format!("prepare: {e}"));
                    rows.push(row);
                    continue;
                }
            };
        row.field = prepared.field_name().to_string();
        let mut notes: Vec<String> = prepared.notices().to_vec();
        notes.extend(planned.notices.iter().map(|n| n.code().to_string()));
        if !notes.is_empty() {
            row.notices = notes.join(", ");
        }
        let signed = match signer.sign(prepared) {
            Ok(s) => s,
            Err(e) => {
                row.problems.push(format!("signieren: {e}"));
                rows.push(row);
                continue;
            }
        };
        let ctx = CheckContext { source: &src, expected_signer: Some(&chain.signer), renderer: or.renderer() };
        match signed.self_check(&ctx) {
            Ok(rep) => {
                if or.pdfium.is_some() && !rep.warnings.is_empty() {
                    row.problems.push(format!("self_check-Warnungen: {:?}", rep.warnings));
                }
            }
            Err(v) => row.problems.push(format!("self_check: {v}")),
        }
        let dir = work.path().join(format!("{i:03}"));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("signed.pdf");
        std::fs::write(&out, signed.bytes()).unwrap();
        // zur Sichtprüfung: PIDA_KEEP=<verzeichnis>
        if let Some(keep) = std::env::var_os("PIDA_KEEP") {
            let _ = std::fs::copy(&out, PathBuf::from(keep).join(format!("{i:03}-{}", name.replace('/', "_"))));
        }
        let look = match &planned.position {
            Some(rp) => {
                Some(Look::Visible { page: rp.page_index, rect: visible_block_rect(rp), new_page: rp.new_page })
            }
            None if !planned.plan.is_visible() => Some(Look::Invisible { page: 0 }),
            None => None,
        };
        let (q, o, p) = check_output(&or, path, src.bytes(), &signed, &out, look, &dir, &mut row.problems);
        row.qpdf = q;
        row.openssl = o;
        row.pixel = p;
        row.result = if planned.position.as_ref().is_some_and(|p| p.new_page) {
            "signiert (neue Seite)".into()
        } else {
            "signiert".into()
        };
        row.out = Some(out);
        rows.push(row);
    }

    // pyHanko: ein Lauf für alle Ausgaben (vertraut: Testzertifikate), einer für Originale
    if let Some(py) = &or.python {
        let outs: Vec<PathBuf> = rows.iter().filter_map(|r| r.out.clone()).collect();
        let trust: Vec<PathBuf> = KEYS.iter().map(|(_, s)| key_cert_pem(s)).collect();
        let after = pyhanko_batch(py, &outs, &trust).expect("pyHanko");
        let originals: Vec<PathBuf> =
            rows.iter().filter(|r| r.prior > 0 && r.out.is_some()).map(|r| r.original.clone()).collect();
        let before = if originals.is_empty() {
            Default::default()
        } else {
            pyhanko_batch(py, &originals, &[]).expect("pyHanko")
        };
        for r in rows.iter_mut().filter(|r| r.out.is_some()) {
            let a = &after[&r.out.as_ref().unwrap().to_string_lossy().to_string()];
            let b = before.get(&r.original.to_string_lossy().to_string()).cloned().unwrap_or_default();
            let (n, p) = evaluate_pyhanko(&b, a, &r.field, &mut r.problems);
            r.new_sig = n;
            r.prior_after = p;
        }
    }

    // Ergebnis
    let ok = rows.iter().filter(|r| r.problems.is_empty()).count();
    let mut md = String::new();
    md.push_str("## Korpus (Modell A, Keystore-Backend)\n\n");
    md.push_str("Erzeugt von `cargo test -p pida-pades --test e2e_corpus`. Je Datei rotieren Schlüssel (RSA-2048, P-256, P-384), Profil (`INVISIBLE`, `SIGNATURBLOCK_SMALL_DE`, `SIGNATURBLOCK_SMALL_EN_NOTE`, `AMTSSIGNATURBLOCK_DE_RECOMMENDED`; PDF/A-Eingaben `SIGNATURBLOCK_SMALL_DE_PDFA`) und CMS-Profil (`pdfas-compat`, `pades-b-b`). Ablauf: `analyze` → `plan_signature` (PDFium-Rasterscan) → `prepare` → `KeystoreSession::sign` (Modell A) → `finish_raw` → `self_check` (I1–I6, PDFium) → Orakel.\n\n");
    md.push_str("Spalten: *qpdf* = `qpdf --check` ohne neue Befunde; *openssl* = `openssl cms -verify -noverify` über die ByteRange-Daten; *Bild* = PDFium-Render vorher/nachher (Block mit Inhalt im erwarteten Rechteck, außerhalb keine Änderung; unsichtbar: Seite unverändert); *Neue Signatur* = pyHanko Integrität / Gültigkeit / Abdeckung (Vertrauensanker = Testzertifikate); *Vorsignaturen* = pyHanko-Änderungsstufe vorher → nachher; *Hinweise* = `prepare`-/Positionierungshinweise.\n\n");
    md.push_str(&format!("Werkzeuge: {}.\n\n", or.versions()));
    md.push_str(&format!("**{ok} von {} Dateien bestanden.**\n\n", rows.len()));
    md.push_str("| Datei | xref | Verschl. | Vorsig. | Klasse | Schlüssel | Profil | CMS | Ergebnis | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for r in &rows {
        md.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.file,
            r.xref,
            r.enc,
            r.prior,
            r.class,
            r.key,
            r.profile,
            r.cms,
            r.result,
            r.qpdf,
            r.openssl,
            r.pixel,
            r.new_sig,
            r.prior_after,
            r.notices,
            if r.problems.is_empty() { "ok".to_string() } else { format!("**FEHLER**: {}", r.problems.join("; ")) }
        ));
    }
    write_results_section("korpus", &md);
    let failed: Vec<String> =
        rows.iter().filter(|r| !r.problems.is_empty()).map(|r| format!("{}: {:?}", r.file, r.problems)).collect();
    assert!(failed.is_empty(), "{} Fehler:\n{}", failed.len(), failed.join("\n"));
}
