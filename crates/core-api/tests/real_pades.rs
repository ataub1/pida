//! Ende-zu-Ende mit der echten PAdES-Engine (`pida-pades`, Paket I1) und externen Orakeln
//! (qpdf, pyHanko, openssl; ohne Werkzeug überspringt sich das jeweilige Orakel). Seit Welle 3 laufen
//! die Tests im Standardlauf; die Abläufe selbst prüfen zusätzlich `tests/flow.rs` mit der Fake-Engine.

mod common;

use std::process::Command;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::*;
use pida_backend_api::{NativeForm, SignatureBackend};
use pida_config::Overlay;
use pida_core_api::{CoreOptions, FormAnswerView, InteractionView, StepView, UiEvent};
use pida_workflow::{DocResult, ExitCode};
use zeroize::Zeroizing;

fn tool(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok_and(|o| o.status.success())
}

fn sign_headless(fx: &Fixture, rel: &str, invisible: bool) -> std::path::PathBuf {
    let input = fx.dir("in");
    let a = copy_pdf(rel, &input, "doc.pdf");
    let overlay = Overlay {
        input_files: vec![a],
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        backend: Some(pida_config::BackendChoice::Keystore),
        invisible: Some(invisible),
        ..Overlay::default()
    };
    let mut cfg = fx.config(overlay, true);
    cfg.pdfium_dir = std::env::var_os("PIDA_PDFIUM_DIR").map(Into::into);
    let opts =
        CoreOptions { interactive: Some(false), use_pdfium: cfg.pdfium_dir.is_some(), ..CoreOptions::isolated() };
    let core = pida_core_api::Core::new_with(cfg, opts).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let code = rt.block_on(core.wait_finished());
    assert_eq!(code, ExitCode::Success, "{:#?}", core.view());
    let r = core.results();
    assert!(matches!(r[0].result, DocResult::Signed { .. }));
    input.join("doc_signed.pdf")
}

fn oracles(out: &std::path::Path) {
    if tool("qpdf") {
        let o = Command::new("qpdf").arg("--check").arg(out).output().unwrap();
        assert!(o.status.success(), "qpdf --check: {}", String::from_utf8_lossy(&o.stdout));
    } else {
        eprintln!("qpdf fehlt – Orakel übersprungen");
    }
    if tool("pyhanko") {
        let o = Command::new("pyhanko").args(["sign", "validate", "--no-strict-syntax"]).arg(out).output().unwrap();
        // ohne Vertrauensanker ist die Signatur „untrusted“, aber intakt
        let text = String::from_utf8_lossy(&o.stdout);
        assert!(text.contains("INTACT") || text.contains("intact"), "pyHanko: {text}");
    } else {
        eprintln!("pyHanko fehlt – Orakel übersprungen");
    }
}

#[test]
fn real_engine_keystore_visible() {
    let fx = Fixture::new();
    let out = sign_headless(&fx, "pdf/generated/simple-xref-table.pdf", false);
    oracles(&out);
}

#[test]
fn real_engine_keystore_invisible_xref_stream() {
    let fx = Fixture::new();
    let out = sign_headless(&fx, "pdf/generated/simple-xref-stream.pdf", true);
    oracles(&out);
}

// ------------------------------------------------------------------------------------------------
// Zertifikats-Cache (Opt-in, Plan §4.2) mit der echten Engine: Treffer → Signatur ohne InfoboxRead,
// für qpdf/pyHanko intakt, Signer = Zertifikat aus dem Cache; abweichendes Zertifikat im Cache → die
// echte Selbstprüfung (I3, `cms-unexpected-signer`) verwirft das Ergebnis, „Wiederholen“ liest neu.
// ------------------------------------------------------------------------------------------------

const CACHE_SETTINGS: &str = "[signature]\nauto_position = true\n\
[id_austria]\nusername = \"+436641234567\"\n[certificate_cache]\nenabled = true\n";

fn cache_key() -> pida_config::CertCacheKey {
    pida_config::CertCacheKey::new("atrust_native", "+436641234567")
}

async fn submit_tan(core: &pida_core_api::Core) {
    let v = wait_for(core, "TAN", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::SmsTan { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
}

async fn submit_login(core: &pida_core_api::Core) {
    let v = wait_for(core, "Login", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitForm {
        request,
        answer: FormAnswerView::Login { username: "+436641234567".into(), password: "geheim1".into(), remember: false },
    });
}

/// Signer der ersten Signatur (Subject) über pyHanko; `None` ohne pyHanko.
fn pyhanko_signer(out: &std::path::Path) -> Option<String> {
    if !tool("pyhanko") {
        return None;
    }
    let o = Command::new("pyhanko")
        .args(["sign", "validate", "--pretty-print", "--no-strict-syntax"])
        .arg(out)
        .output()
        .unwrap();
    Some(String::from_utf8_lossy(&o.stdout).into_owned())
}

fn start_cached(fx: &Fixture, out: &std::path::Path, cached: &str) -> (pida_core_api::Core, Arc<FakeAtrust>) {
    fx.write_settings(CACHE_SETTINGS);
    let cache = pida_config::CertificateCache::with_dir(fx.paths.certificate_cache_dir(), true);
    cache.put(&cache_key(), &std::fs::read(testdata(cached)).unwrap()).unwrap();
    let a = copy_pdf("pdf/generated/simple-xref-table.pdf", &fx.dir("in"), "a.pdf");
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay = Overlay { input_files: vec![a], output: Some(out.to_path_buf()), ..Overlay::default() };
    let mut cfg = fx.config(overlay, false);
    cfg.pdfium_dir = std::env::var_os("PIDA_PDFIUM_DIR").map(Into::into);
    let opts =
        CoreOptions { backends: Some(backends), use_pdfium: cfg.pdfium_dir.is_some(), ..CoreOptions::isolated() };
    (pida_core_api::Core::new_with(cfg, opts).unwrap(), fake)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_engine_certificate_cache_hit_is_valid() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let (core, fake) = start_cached(&fx, &out, "keystores/test-rsa2048.crt.der");
    choose_id_austria(&core).await;
    // kein Lesen des Zertifikats (keine Anmeldung), direkt die Bestätigung
    submit_tan(&core).await;
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert_eq!(fake.certificates.load(Ordering::SeqCst), 0, "Zertifikat kam aus dem Cache");
    let signed = out.join("a_signed.pdf");
    oracles(&signed);
    if let Some(text) = pyhanko_signer(&signed) {
        assert!(text.contains("PDF-Over NG Test RSA (NUR TEST)"), "pyHanko: {text}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_engine_certificate_cache_mismatch_is_caught_and_retried() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    // im Cache ein anderes Zertifikat (z. B. vor der Erneuerung); A-Trust signiert mit dem RSA-Testschlüssel
    let (core, fake) = start_cached(&fx, &out, "keystores/test-ecp256.crt.der");
    choose_id_austria(&core).await;
    submit_tan(&core).await;
    let v = wait_for(&core, "Fehler", |v| matches!(&v.step, StepView::Error { .. })).await;
    let StepView::Error { code, params, retry } = v.step else { unreachable!() };
    assert_eq!((code.as_str(), retry), ("ng-certcache-mismatch", true), "{params:?}");
    assert_eq!(params.get("detail").map(String::as_str), Some("cms-unexpected-signer"));
    assert!(!out.join("a_signed.pdf").exists(), "nichts gespeichert");
    let cache = pida_config::CertificateCache::with_dir(fx.paths.certificate_cache_dir(), true);
    assert_eq!(cache.get(&cache_key()).unwrap(), None, "Eintrag gelöscht");
    // Wiederholen: Zertifikat neu lesen (Anmeldung), Block neu bauen, erneute Bestätigung
    core.dispatch(UiEvent::Retry);
    submit_login(&core).await;
    submit_tan(&core).await;
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert_eq!(fake.certificates.load(Ordering::SeqCst), 1);
    let signed = out.join("a_signed.pdf");
    oracles(&signed);
    if let Some(text) = pyhanko_signer(&signed) {
        assert!(text.contains("PDF-Over NG Test RSA (NUR TEST)"), "pyHanko: {text}");
    }
    // bestätigtes Zertifikat im Cache
    assert_eq!(
        cache.get(&cache_key()).unwrap().as_deref(),
        Some(std::fs::read(testdata("keystores/test-rsa2048.crt.der")).unwrap().as_slice())
    );
}
