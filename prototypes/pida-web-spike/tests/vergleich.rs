//! Gegenstück zu `window.vergleich()` in web/main.js: dieselben Dokumente nativ (PDFium aus PIDA_PDFIUM_DIR) mit
//! gleicher Zeit und gleichem Keystore signieren; Lage nach target/pruefung/nativ.json und die PDFs daneben.
//!   PIDA_PDFIUM_DIR=… cargo test --test vergleich -- --nocapture

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

#[test]
fn nativ() {
    let Some(dir) = std::env::var_os("PIDA_PDFIUM_DIR") else {
        eprintln!("PIDA_PDFIUM_DIR fehlt – übersprungen");
        return;
    };
    pida_render::PdfiumBackend::bind(Some(Path::new(&dir))).expect("PDFium");
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let samples = here.join("web/samples/vergleich");
    let liste: Vec<String> =
        serde_json::from_slice(&std::fs::read(samples.join("liste.json")).expect("build.sh zuerst")).unwrap();
    let key = std::fs::read(here.join("web/samples/test-rsa2048.p12")).unwrap();
    let out_dir = here.join("target/pruefung");
    std::fs::create_dir_all(&out_dir).unwrap();
    // wie VERGLEICH_ZEIT_MS (26.10.2026 09:00 UTC) und getTimezoneOffset() = -60 (MEZ)
    let zeit_ms = 1_793_005_200_000.0;
    let mut out = serde_json::Map::new();
    for name in liste {
        let bytes = std::fs::read(samples.join(&name)).unwrap();
        let t = std::time::Instant::now();
        let v = match pida_web_spike::sign(&bytes, &key, "test", "SIGNATURBLOCK_SMALL_DE", zeit_ms, -60) {
            Ok(signed) => {
                std::fs::write(out_dir.join(format!("nativ_{name}")), &signed).unwrap();
                let mut v: serde_json::Value = serde_json::from_str(&pida_web_spike::last_plan()).unwrap();
                v["ms"] = (t.elapsed().as_millis() as u64).into();
                v["bytes"] = signed.len().into();
                v
            }
            Err(_) => serde_json::json!({ "fehler": "siehe Ausgabe" }),
        };
        println!("{name}: {v}");
        out.insert(name, v);
    }
    std::fs::write(out_dir.join("nativ.json"), serde_json::to_string_pretty(&out).unwrap()).unwrap();
}
