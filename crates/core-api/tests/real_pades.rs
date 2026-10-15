//! Ende-zu-Ende mit der echten PAdES-Engine (`pdfover-pades`, Paket I1) und externen Orakeln
//! (qpdf, pyHanko, openssl; ohne Werkzeug überspringt sich das jeweilige Orakel). Seit Welle 3 laufen
//! die Tests im Standardlauf; die Abläufe selbst prüfen zusätzlich `tests/flow.rs` mit der Fake-Engine.

mod common;

use std::process::Command;

use common::*;
use pdfover_config::Overlay;
use pdfover_core_api::CoreOptions;
use pdfover_workflow::{DocResult, ExitCode};
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
        backend: Some(pdfover_config::BackendChoice::Keystore),
        invisible: Some(invisible),
        ..Overlay::default()
    };
    let mut cfg = fx.config(overlay, true);
    cfg.pdfium_dir = std::env::var_os("PDFOVER_PDFIUM_DIR").map(Into::into);
    let opts =
        CoreOptions { interactive: Some(false), use_pdfium: cfg.pdfium_dir.is_some(), ..CoreOptions::isolated() };
    let core = pdfover_core_api::Core::new_with(cfg, opts).unwrap();
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
