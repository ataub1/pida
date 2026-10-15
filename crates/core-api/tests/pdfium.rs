//! Abläufe mit PDFium (Seitenkacheln, Blockvorschau, Startposition, Auto-Position, QR-Platzhalter).
//! Ohne `PIDA_PDFIUM_DIR` übersprungen (ADR 0001 Regel 6).

mod common;

use common::*;
use pida_config::Overlay;
use pida_core_api::{CoreOptions, StepView, UiEvent};
use pida_workflow::DocId;

fn pdfium_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("PIDA_PDFIUM_DIR").map(Into::into)
}

fn start(fx: &Fixture, overlay: Overlay) -> Option<pida_core_api::Core> {
    let dir = pdfium_dir()?;
    let mut cfg = fx.config(overlay, false);
    cfg.pdfium_dir = Some(dir);
    let opts = CoreOptions { engine: Some(FakeEngine::new()), use_pdfium: true, ..CoreOptions::isolated() };
    Some(pida_core_api::Core::new_with(cfg, opts).expect("Kern"))
}

fn is_png(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG\r\n\x1a\n")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn positioning_with_tiles_preview_and_auto_place() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf("pdf/pdfas/pdf-as-legacy/simple.pdf", &input, "simple.pdf");
    let Some(core) = start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let v = wait_for(&core, "Positionierung", |v| matches!(v.step, StepView::Position { .. })).await;
    let StepView::Position { doc, page, placement, block, announce, .. } = v.step else { unreachable!() };
    assert_eq!(page, doc.page_count - 1, "Startseite = letzte Seite");
    let (w, h) = doc.page_sizes[page];
    // 50 %/25 % (F-33), geklemmt
    assert!((placement.x - (0.5 * w).min(w - placement.width)).abs() < 0.5, "{placement:?}");
    assert!((placement.y - (0.25 * h).max(placement.height)).abs() < 0.5, "{placement:?}");
    assert!(placement.width > 100.0);
    assert_eq!(announce["page"], doc.page_count.to_string());
    // Kachel und Blockvorschau aus dem echten Appearance-Objekt
    let tile = core.page_tile(doc.id, page, 0.5).unwrap();
    assert!(is_png(&tile));
    assert!(core.page_tile(doc.id, page, 0.5).unwrap() == tile, "aus dem Cache");
    assert!(core.page_tile(DocId(999), 0, 1.0).is_err());
    let v = wait_for(
        &core,
        "Blockvorschau",
        |v| matches!(&v.step, StepView::Position { block, .. } if block.preview_revision > 0),
    )
    .await;
    let StepView::Position { block: b2, .. } = v.step else { unreachable!() };
    assert!((b2.width - block.width).abs() < 1.0 || block.preview_revision == 0);
    let png = core.block_preview(2.0).unwrap();
    assert!(is_png(&png));
    // Verschieben wird auf die Seite geklemmt (F-35)
    core.dispatch(UiEvent::MoveBlock { page, x: -100.0, y: 10_000.0 });
    let v =
        wait_for(&core, "geklemmt", |v| matches!(&v.step, StepView::Position { placement, .. } if placement.x == 0.0))
            .await;
    let StepView::Position { placement: p, .. } = v.step else { unreachable!() };
    assert!((p.y - h).abs() < 0.01, "{p:?}");
    // Tastatur: grob 50 pt nach rechts
    core.dispatch(UiEvent::NudgeBlock { dx: 1, dy: 0, fine: false, coarse: true });
    wait_for(
        &core,
        "Schritt",
        |v| matches!(&v.step, StepView::Position { placement, .. } if (placement.x - 50.0).abs() < 0.01),
    )
    .await;
    // „Automatisch platzieren“: Freiraumsuche mit PDFium
    core.dispatch(UiEvent::AutoPlace);
    let v = wait_for(&core, "Auto", |v| {
        matches!(&v.step, StepView::Position { placement, .. } if (placement.x - 50.0).abs() > 0.01 || placement.y < h - 1.0)
    })
    .await;
    let StepView::Position { placement: auto, .. } = v.step else { unreachable!() };
    assert!(auto.y <= h);
    // virtuelle neue Seite: weiße Kachel
    core.dispatch(UiEvent::ToggleNewPage);
    let v = wait_for(&core, "neue Seite", |v| matches!(&v.step, StepView::Position { new_page: true, .. })).await;
    let StepView::Position { page: np, .. } = v.step else { unreachable!() };
    assert_eq!(np, doc.page_count);
    assert!(is_png(&core.page_tile(doc.id, np, 0.25).unwrap()));
    core.dispatch(UiEvent::ConfirmPosition);
    wait_for(&core, "Auswahl", |v| matches!(v.step, StepView::Backend { .. })).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn qr_placeholder_is_offered() {
    let fx = Fixture::new();
    fx.write_settings("[placeholder]\nsearch = true\nkind = \"marker\"\n");
    let input = fx.dir("in");
    let a = copy_pdf("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf", &input, "qr.pdf");
    let Some(core) = start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let v =
        wait_for(&core, "Platzhalter", |v| matches!(v.step, StepView::Placeholder { .. } | StepView::Position { .. }))
            .await;
    let StepView::Placeholder { qr, fields, .. } = v.step else { panic!("kein Platzhalter gefunden: {:?}", v.step) };
    assert!(fields.is_empty());
    assert!(qr.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn block_preview_without_document_uses_a4() {
    let fx = Fixture::new();
    let Some(core) = start(&fx, Overlay::default()) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    // ohne Positionierung und ohne Einstellungen wurde noch keine Vorschau gebaut
    assert!(core.block_preview(1.0).is_err());
    // Einstellungen öffnen baut die Vorschau (A4, Beispielzertifikat) wie PDF-Over, auch ohne Dokument
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_for(&core, "Vorschau in den Einstellungen", |v| {
        v.settings.as_ref().is_some_and(|s| s.preview_revision > 0)
    })
    .await;
    assert!(v.settings.unwrap().preview_revision > 0);
    let png = core.block_preview(2.0).expect("Blockvorschau ohne Dokument");
    assert!(is_png(&png));
}

/// Minimales PDF mit zwei unterschiedlich breiten Seiten (A4 quer, A4 hoch).
fn mixed_size_pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>".to_owned(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
    );
    out
}

/// KR-F01 (Vorschau = Ergebnis): Bei Profilen ohne `w` hängt die Blockbreite von der Seitenbreite ab;
/// ein Seitenwechsel baut die Vorschau für die angezeigte Seite neu.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn block_width_follows_the_displayed_page() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nprofile = \"AMTSSIGNATURBLOCK\"\nblock_language = \"de\"\n");
    let input = fx.dir("in");
    let a = input.join("gemischt.pdf");
    std::fs::write(&a, mixed_size_pdf()).unwrap();
    let Some(core) = start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let near = |a: f32, b: f32| (a - b).abs() < 1.0;
    // Startseite = letzte (595 pt breit): Breite 595 − 2 × 50
    let v = wait_for(&core, "Vorschau letzte Seite", |v| {
        matches!(&v.step, StepView::Position { block, page: 1, .. } if block.preview_revision > 0 && near(block.width, 495.0))
    })
    .await;
    let StepView::Position { block: last, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::ChangePage { page: 0 });
    // erste Seite (842 pt breit): 742 pt, Vorschau neu gebaut, Position mitgezogen
    let v = wait_for(&core, "Vorschau erste Seite", |v| {
        matches!(&v.step, StepView::Position { block, page: 0, placement, .. }
            if block.preview_revision > last.preview_revision && near(block.width, 742.0) && near(placement.width, 742.0))
    })
    .await;
    let StepView::Position { placement, .. } = v.step else { unreachable!() };
    assert!(placement.x + placement.width <= 842.0 + 0.01, "auf die Seite geklemmt: {placement:?}");
}
