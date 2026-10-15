//! Hilfe zur Sichtprüfung (nicht Teil der Prüfung): rendert alle PDFs in `PIDA_KEEP` mit PDFium
//! als PNG (alle Seiten, 1 px = 1 pt). Aufruf:
//! `PIDA_KEEP=<dir> cargo test -p pida-pades --test render_keep -- --ignored`.

mod common;

use std::sync::Arc;

use pida_render::{RenderBackend, encode_png};

#[test]
#[ignore]
fn render_kept_outputs() {
    let (Some(dir), Some(backend)) = (std::env::var_os("PIDA_KEEP"), common::pdfium()) else {
        eprintln!("PIDA_KEEP oder PDFium fehlt");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "pdf") {
            continue;
        }
        let data: Arc<[u8]> = Arc::from(std::fs::read(&p).unwrap());
        let Ok(doc) = backend.open(data, None) else { continue };
        for page in 0..doc.page_count() {
            let img = doc.render_page(page, 1.0).unwrap();
            let png = encode_png(&img).unwrap();
            std::fs::write(p.with_extension(format!("p{page}.png")), png).unwrap();
        }
    }
}
