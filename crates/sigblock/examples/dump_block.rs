//! Entwicklerwerkzeug: Layout und Content-Stream eines Profils ausgeben.
//!
//! `cargo run -p pida-sigblock --example dump_block -- SIGNATURBLOCK_SMALL_DE 230 [out.pdf]`

use pida_pdf_cos::Object;
use pida_sigblock::layout::{CellContent, LaidOutTable};
use pida_sigblock::{Appearance, BuildOptions, CertFields, FontProvider, PdfaMode, ProfileStore, SigBlockInput};

fn dump(t: &LaidOutTable, indent: usize) {
    let pad = " ".repeat(indent);
    println!(
        "{pad}table {} {}x{} cols={:?} rows={:?} padding={} border={}",
        t.name, t.width, t.height, t.col_widths, t.row_heights, t.padding, t.border
    );
    for c in &t.cells {
        match &c.content {
            CellContent::Text { lines, role, halign, valign, .. } => {
                println!(
                    "{pad}  [{},{}] x={} top={} w={} h={} {role:?} {halign:?}/{valign:?} {lines:?}",
                    c.row, c.col, c.x, c.top, c.width, c.height
                )
            }
            CellContent::Image { key, draw_width, draw_height, .. } => {
                println!(
                    "{pad}  [{},{}] x={} top={} w={} h={} image {key} {draw_width}x{draw_height}",
                    c.row, c.col, c.x, c.top, c.width, c.height
                )
            }
            CellContent::SubTable(s) => {
                println!("{pad}  [{},{}] x={} top={} w={} h={} sub", c.row, c.col, c.x, c.top, c.width, c.height);
                dump(s, indent + 4);
            }
            CellContent::Empty => println!("{pad}  [{},{}] empty", c.row, c.col),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let id = args.get(1).map(String::as_str).unwrap_or("SIGNATURBLOCK_SMALL_DE");
    let width: f32 = args.get(2).and_then(|w| w.parse().ok()).unwrap_or(230.0);
    let store = ProfileStore::builtin().expect("store");
    let profile = store.get(id).expect("profile");
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), "Max Mustermann".into())];
    let t = time::macros::datetime!(2026-10-14 12:34:56 +2);
    let mut input = SigBlockInput::new(cert, t);
    if std::env::var("QUIRKS").is_ok() {
        input.pdfas_quirks = true;
    }
    let fonts = FontProvider::builtin();
    let opts = BuildOptions { width, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let app = Appearance::build(profile, &input, &opts, &fonts, &**store.resources()).expect("build");
    let layout = app.layout.as_ref().expect("layout");
    println!("W={} H={} tu={:?}", layout.width, layout.height, layout.tu_text);
    dump(&layout.table, 0);
    if let Object::Stream(s) = &app.objects[app.inner] {
        println!("--- inner content ---\n{}", String::from_utf8_lossy(&s.content));
    }
    if let Some(out) = args.get(3) {
        std::fs::write(out, app.to_standalone_pdf().expect("pdf")).expect("write");
    }
}
