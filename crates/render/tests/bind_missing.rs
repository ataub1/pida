//! Eigener Testprozess: Bindung an ein nicht vorhandenes Verzeichnis liefert eine klare
//! Fehlermeldung (vor jeder erfolgreichen Bindung im Prozess).

use pdfover_render::{PdfiumBackend, RenderError};

#[test]
fn missing_library_reports_searched_location() {
    let dir = std::env::temp_dir().join("pdfover-render-ohne-pdfium");
    let err = PdfiumBackend::bind(Some(&dir)).unwrap_err();
    assert_eq!(err.code(), "render.library_not_found");
    match err {
        RenderError::LibraryNotFound(msg) => {
            assert!(msg.contains("pdfover-render-ohne-pdfium"), "{msg}");
            assert!(msg.contains("chromium/8100"), "{msg}");
        }
        other => panic!("unerwartet: {other:?}"),
    }
    // Danach lässt sich die echte Bibliothek noch binden (der Fehlversuch bindet nichts).
    if let Some(dir) = std::env::var_os(pdfover_render::ENV_PDFIUM_DIR).filter(|d| !d.is_empty()) {
        let backend = PdfiumBackend::bind(Some(std::path::Path::new(&dir))).expect("Bindung nach Fehlversuch");
        assert!(backend.library_path().is_some());
    }
}
