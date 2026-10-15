//! Vollständige Matrix (Modell A, Keystore-Backend): `simple.pdf` × {RSA-2048, P-256, P-384} ×
//! {INVISIBLE, SIGNATURBLOCK_SMALL_DE, SIGNATURBLOCK_SMALL_EN_NOTE,
//! AMTSSIGNATURBLOCK_DE_RECOMMENDED, SIGNATURBLOCK_SMALL_DE_PDFA} × {pdfas-compat, pades-b-b};
//! PDF/A-1b-Eingabe × alle Schlüssel × `SIGNATURBLOCK_SMALL_DE_PDFA` × beide CMS-Profile; alle 12
//! PDF-Over-Profil-IDs einmal. Orakel wie im Korpus-Test.

mod common;

use common::*;
use pdfover_cades::CmsProfile;

const PROFILES: [&str; 5] = [
    "INVISIBLE",
    "SIGNATURBLOCK_SMALL_DE",
    "SIGNATURBLOCK_SMALL_EN_NOTE",
    "AMTSSIGNATURBLOCK_DE_RECOMMENDED",
    "SIGNATURBLOCK_SMALL_DE_PDFA",
];

fn cms_name(p: CmsProfile) -> &'static str {
    match p {
        CmsProfile::PdfasCompat => "pdfas-compat",
        CmsProfile::PadesBB => "pades-b-b",
    }
}

#[test]
fn full_matrix() {
    let or = Oracles::detect();
    let work = tempfile::tempdir().unwrap();
    let simple = testdata().join("pdf/pdfas/pdf-as-legacy/simple.pdf");
    let pdfa = testdata().join("pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf");
    let simple_bytes = std::fs::read(&simple).unwrap();
    let pdfa_bytes = std::fs::read(&pdfa).unwrap();
    let mut outcomes = Vec::new();
    let none = |_: &mut pdfover_pades::PlanOptions, _: &pdfover_pades::DocReport, _: &pdfover_pdf_cos::SourcePdf| {};
    for (k, (key_label, stem)) in KEYS.iter().enumerate() {
        let mut signer = KeystoreSigner::open(stem);
        for profile in PROFILES {
            for cms in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
                let name = format!("simple-{k}-{profile}-{}", cms_name(cms));
                let label = format!("`simple.pdf` {key_label} {profile} {}", cms_name(cms));
                outcomes.push(run_case(
                    &or,
                    &mut signer,
                    &simple,
                    &simple_bytes,
                    profile,
                    cms,
                    &none,
                    work.path(),
                    &name,
                    &label,
                ));
            }
        }
        for cms in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
            let name = format!("pdfa-{k}-{}", cms_name(cms));
            let label = format!(
                "`TestGhostscriptPdfA.pdf` (PDF/A-1b) {key_label} SIGNATURBLOCK_SMALL_DE_PDFA {}",
                cms_name(cms)
            );
            outcomes.push(run_case(
                &or,
                &mut signer,
                &pdfa,
                &pdfa_bytes,
                "SIGNATURBLOCK_SMALL_DE_PDFA",
                cms,
                &none,
                work.path(),
                &name,
                &label,
            ));
        }
    }
    // alle 12 Profil-IDs von PDF-Over (RSA-2048, pdfas-compat)
    let mut rsa = KeystoreSigner::open("test-rsa2048");
    for id in pdfover_sigblock::PDFOVER_PROFILE_IDS {
        let name = format!("profile-{id}");
        let label = format!("`simple.pdf` RSA-2048 {id} pdfas-compat");
        outcomes.push(run_case(
            &or,
            &mut rsa,
            &simple,
            &simple_bytes,
            id,
            CmsProfile::PdfasCompat,
            &none,
            work.path(),
            &name,
            &label,
        ));
    }
    let trust: Vec<_> = KEYS.iter().map(|(_, s)| key_cert_pem(s)).collect();
    pyhanko_all(&or, &mut outcomes, &trust);

    let ok = outcomes.iter().filter(|o| o.problems.is_empty()).count();
    let mut md = String::from("## Matrix (Modell A, Keystore-Backend)\n\n");
    md.push_str("Erzeugt von `cargo test -p pdfover-pades --test e2e_matrix`: `simple.pdf` × 3 Schlüssel × 5 Profile × 2 CMS-Profile, PDF/A-1b-Eingabe × 3 Schlüssel × `SIGNATURBLOCK_SMALL_DE_PDFA` × 2 CMS-Profile, alle 12 PDF-Over-Profil-IDs (BASE_LOGO mit dem Laufzeit-Overlay 23 mm). Orakel und Spalten wie im Korpus.\n\n");
    md.push_str(&format!("Werkzeuge: {}.\n\n**{ok} von {} Fällen bestanden.**\n\n", or.versions(), outcomes.len()));
    md.push_str(&table(&outcomes));
    write_results_section("matrix", &md);
    assert_all_ok(&outcomes);
}
