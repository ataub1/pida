//! Einstellungen „sofort wirksam“ und „Zuletzt signiert“ gegen den echten Kern (ADR 0004 §3.10, §3.11,
//! Etappe E3, Paket K3): Teilübernahme mit Feldfehlern, unbekannte Pfade, veraltete Revisionen, keine
//! verlorene Kernänderung, Neustart, genau ein Neubau der Vorschau je Änderung am Signaturbild (in derselben
//! Projektion wie `applied_revision`), der Block im offenen Dokument passt sich an, Einstellungen › Dokument
//! (PDF/A, Platzhaltersuche QR | leere Felder), schreibgeschützt (`-c`), Mobile, Richtlinie und „Zuletzt
//! signiert“ (nur Name/Pfad/Zeit, Abschalten löscht, ohne Oberfläche nichts). PDFium-Fälle laufen nur mit
//! `PIDA_PDFIUM_DIR` (sonst übersprungen).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::*;
use pida_backend_api::BackendId;
use pida_config::{Overlay, Policy, SignatureProfile};
use pida_core_api::{
    CoreOptions, DocumentPhase, PdfaDecision, PlacementTarget, SettingsView, StepView, UiEvent, ViewState,
};
use pida_workflow::PlacementOrigin;
use serde_json::{Value, json};
use zeroize::Zeroizing;

const TEXT_PDF: &str = "pdf/pdfas/pdf-as-legacy/simple.pdf";
const FIELDS_PDF: &str = "pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf";
const QR_PDF: &str = "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf";
const PDFA_PDF: &str = "pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf";

fn pdfium_dir() -> Option<PathBuf> {
    std::env::var_os("PIDA_PDFIUM_DIR").filter(|d| !d.is_empty()).map(Into::into)
}

/// Kern mit Oberfläche im Arbeitsbereich-Modus.
fn start(
    fx: &Fixture,
    overlay: Overlay,
    pdfium: bool,
    adjust: impl FnOnce(&mut CoreOptions),
) -> Option<pida_core_api::Core> {
    let mut cfg = fx.config(overlay, false);
    if pdfium {
        cfg.pdfium_dir = Some(pdfium_dir()?);
    }
    let mut opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as std::sync::Arc<dyn pida_core_api::engine::PadesEngine>),
        use_pdfium: pdfium,
        ..CoreOptions::isolated()
    };
    adjust(&mut opts);
    Some(pida_core_api::Core::new_with(cfg, opts).expect("Kern startet"))
}

fn keystore_overlay(inputs: Vec<PathBuf>, out: &Path) -> Overlay {
    Overlay {
        input_files: inputs,
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        output: Some(out.to_path_buf()),
        ..Overlay::default()
    }
}

fn changes(list: &[(&str, Value)]) -> BTreeMap<String, Value> {
    list.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect()
}

fn settings(v: &ViewState) -> &SettingsView {
    v.settings.as_ref().expect("Einstellungsansicht")
}

async fn open_settings(core: &pida_core_api::Core) -> ViewState {
    core.dispatch(UiEvent::OpenSettings);
    wait_for(core, "Einstellungen offen", |v| v.settings.is_some()).await
}

/// `ApplySettings` senden und auf die Bestätigung (`applied_revision`) warten.
async fn apply(core: &pida_core_api::Core, revision: u64, list: &[(&str, Value)]) -> ViewState {
    core.dispatch(UiEvent::ApplySettings { changes: changes(list), revision });
    wait_for(core, "übernommen", |v| v.settings.as_ref().is_some_and(|s| s.applied_revision >= revision)).await
}

/// Alle beobachteten Zustände, bis `pred` gilt.
async fn collect_until(core: &pida_core_api::Core, what: &str, pred: impl Fn(&ViewState) -> bool) -> Vec<ViewState> {
    let mut rx = core.subscribe();
    let mut seen = Vec::new();
    let r = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let v = rx.borrow_and_update().clone();
            let done = pred(&v);
            seen.push(v);
            if done {
                return;
            }
            if rx.changed().await.is_err() {
                panic!("Kern beendet");
            }
        }
    })
    .await;
    if r.is_err() {
        panic!("Zeitüberschreitung beim Warten auf {what}: {:#?}", core.view());
    }
    seen
}

fn editing(v: &ViewState) -> bool {
    v.document.as_ref().is_some_and(|d| d.phase == DocumentPhase::Editing)
}

fn saved(v: &ViewState) -> bool {
    matches!(&v.step, StepView::Output { result, .. } if result.path.is_some())
}

fn read_settings_file(fx: &Fixture) -> pida_config::Settings {
    let text = std::fs::read_to_string(fx.paths.settings_file()).expect("settings.toml");
    pida_config::Settings::from_toml(&text).unwrap()
}

// ------------------------------------------------------------------------------------------------
// §3.11 sofort wirksam (Abnahme E3 Punkt 1 und 2)
// ------------------------------------------------------------------------------------------------

/// Ungültiger Proxy-Port zeigt den Feldfehler, die übrigen Änderungen werden trotzdem übernommen und
/// gespeichert; unbekannter Pfad → Problem; veraltete Revision → ohne Wirkung; eine zwischenzeitliche
/// Kernänderung (`last_source_directory` nach `OpenFiles`) geht nicht verloren; ohne geöffnete Ansicht keine
/// Wirkung; Neustart zeigt die übernommenen Werte; Beispiel für den Dateinamen vom Kern.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn partial_apply_field_errors_revisions_and_restart() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(TEXT_PDF, &input, "a.pdf");
    let core = start(&fx, Overlay::default(), false, |_| {}).unwrap();
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    // ohne geöffnete Ansicht: nichts
    core.dispatch(UiEvent::ApplySettings { changes: changes(&[("output.postfix", json!("_x"))]), revision: 1 });
    let v = open_settings(&core).await;
    let s = settings(&v);
    assert_eq!((s.applied_revision, s.settings.output.postfix.as_str()), (0, "_signed"));
    assert_eq!(s.examples["output.postfix"], "Vertrag_signed.pdf");
    assert_eq!(s.locked.as_ref().map(BTreeMap::len), Some(0), "keine Richtlinie: nichts gesperrt");

    // ungültiger Port + gültiges Profil in einer Übernahme
    let v = apply(&core, 1, &[("proxy.port", json!(70000)), ("signature.profile", json!("AMTSSIGNATURBLOCK"))]).await;
    let s = settings(&v);
    assert_eq!(s.problems.get("proxy.port").map(String::as_str), Some("ng-settings-invalid-port"));
    assert_eq!(s.settings.proxy.port, None, "Feld mit Problem behält den bisherigen Wert");
    assert_eq!(s.settings.signature.profile, SignatureProfile::Amtssignaturblock);
    assert_eq!(read_settings_file(&fx).signature.profile, SignatureProfile::Amtssignaturblock, "gespeichert");

    // unbekannter Pfad; das Problem am Port bleibt, bis der Port erneut gesendet wird
    let v = apply(&core, 2, &[("signature.gibts_nicht", json!(1))]).await;
    let s = settings(&v);
    assert_eq!(s.problems["signature.gibts_nicht"], "ng-settings-field-unknown");
    assert_eq!(s.problems["proxy.port"], "ng-settings-invalid-port");

    // Kernänderung zwischen zwei Übernahmen: letzter Ordner nach „Öffnen“
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
    wait_for(&core, "Dokument offen", |v| v.document.is_some()).await;
    let v = apply(&core, 3, &[("output.postfix", json!("_signiert")), ("proxy.port", json!(3128))]).await;
    let s = settings(&v);
    assert_eq!(s.settings.general.last_source_directory.as_deref(), Some(input.as_path()), "Kernänderung sichtbar");
    assert!(!s.problems.contains_key("proxy.port"), "gültiger Port: Problem weg");
    assert_eq!(s.examples["output.postfix"], "Vertrag_signiert.pdf");
    let file = read_settings_file(&fx);
    assert_eq!(file.general.last_source_directory.as_deref(), Some(input.as_path()), "nicht überschrieben");
    assert_eq!((file.output.postfix.as_str(), file.proxy.port), ("_signiert", Some(3128)));

    // veraltete Revision: ohne Wirkung
    core.dispatch(UiEvent::ApplySettings { changes: changes(&[("output.postfix", json!("_alt"))]), revision: 2 });
    let v = apply(&core, 4, &[("general.update_check", json!(false))]).await;
    let s = settings(&v);
    assert_eq!(s.applied_revision, 4);
    assert_eq!(s.settings.output.postfix, "_signiert", "Revision 2 nach 3 verworfen");
    assert!(!s.settings.general.update_check);

    // Schließen verwirft nichts; Neustart zeigt die übernommenen Werte
    core.dispatch(UiEvent::CloseSettings);
    wait_for(&core, "geschlossen", |v| v.settings.is_none()).await;
    drop(core);
    let core = start(&fx, Overlay::default(), false, |_| {}).unwrap();
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    let v = open_settings(&core).await;
    let s = settings(&v);
    assert_eq!(s.settings.signature.profile, SignatureProfile::Amtssignaturblock);
    assert_eq!((s.settings.output.postfix.as_str(), s.settings.proxy.port), ("_signiert", Some(3128)));
    assert!(!s.settings.general.update_check);
    assert!(s.problems.is_empty(), "Feldfehler gelten nur bis zum Schließen: {:?}", s.problems);
}

/// Je Änderung am Signaturbild genau ein Neubau der Vorschau; die neue `preview_revision` erscheint in
/// derselben Projektion wie `applied_revision` (nie eine bestätigte Änderung mit alter Vorschau). Änderungen
/// ohne Bezug zum Signaturbild bauen nichts neu und werden sofort bestätigt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exactly_one_preview_per_signature_change_in_the_same_projection() {
    let fx = Fixture::new();
    let core = start(&fx, Overlay::default(), false, |_| {}).unwrap();
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    let v = open_settings(&core).await;
    drop(v);
    let v = wait_for(&core, "erste Vorschau", |v| v.settings.as_ref().is_some_and(|s| s.preview_revision > 0)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(core.view().settings.unwrap().preview_revision, settings(&v).preview_revision, "Ruhe");
    let mut preview = settings(&v).preview_revision;
    let mut revision = 0;
    let cases: Vec<Vec<(&str, Value)>> = vec![
        vec![("signature.profile", json!("AMTSSIGNATURBLOCK"))],
        vec![("signature.block_language", json!("en")), ("signature.note", json!({ "mode": "none" }))],
        vec![("signature.pdfa_force", json!(true))],
        vec![("signature.note", json!({ "mode": "custom", "text": "Geprüft" }))],
    ];
    for list in cases {
        revision += 1;
        core.dispatch(UiEvent::ApplySettings { changes: changes(&list), revision });
        let seen =
            collect_until(&core, "bestätigt", |v| v.settings.as_ref().is_some_and(|s| s.applied_revision >= revision))
                .await;
        for v in &seen {
            let s = settings(v);
            if s.applied_revision >= revision {
                assert_eq!(s.preview_revision, preview + 1, "{list:?}: neue Vorschau in derselben Projektion");
            } else {
                assert!(s.preview_revision <= preview + 1);
            }
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(core.view().settings.unwrap().preview_revision, preview + 1, "{list:?}: genau ein Neubau");
        preview += 1;
    }
    // ohne Bezug zum Signaturbild: kein Neubau, sofort bestätigt
    revision += 1;
    let v = apply(&core, revision, &[("proxy.mode", json!("none")), ("placeholder.search", json!(true))]).await;
    assert_eq!(settings(&v).preview_revision, preview);
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(core.view().settings.unwrap().preview_revision, preview, "keine Vorschau ohne Änderung am Signaturbild");
    // abgelehnte Änderung am Signaturbild: nichts geändert, nichts neu gebaut
    revision += 1;
    let v = apply(&core, revision, &[("signature.logo_only_size_mm", json!(500))]).await;
    assert_eq!(settings(&v).problems["signature.logo_only_size_mm"], "ng-settings-invalid-logo-size");
    assert_eq!(settings(&v).preview_revision, preview);
}

/// Der Block im offenen Dokument passt sich sofort an: automatisch platziert → neu nach der PDF-AS-Regel
/// gesetzt (Maße der neuen Vorschau), von Hand platziert → Lage bleibt (nur die Maße ändern sich).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn open_document_block_follows_signature_changes() {
    let fx = Fixture::new();
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "doc.pdf");
    let Some(core) = start(&fx, keystore_overlay(vec![a], &fx.dir("out")), true, |_| {}) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    assert_eq!(d.origin, PlacementOrigin::Auto);
    assert!(d.detection.as_ref().is_some_and(|x| x.auto_placed));
    open_settings(&core).await;
    let before = core.view().document.unwrap();
    apply(&core, 1, &[("signature.profile", json!("AMTSSIGNATURBLOCK"))]).await;
    let v = wait_for(&core, "neu platziert", |v| {
        v.document.as_ref().is_some_and(|d| {
            (d.block.height - before.block.height).abs() > 1.0
                && (d.placement.height - d.block.height).abs() < 0.01
                && d.block.preview_revision > before.block.preview_revision
        })
    })
    .await;
    let d = v.document.unwrap();
    assert_eq!((d.origin, d.target), (PlacementOrigin::Auto, PlacementTarget::Free), "weiter automatisch");
    assert!(d.detection.as_ref().is_some_and(|x| x.auto_placed));
    // von Hand verschieben, dann wieder Standardprofil: Lage bleibt, Maße folgen
    core.dispatch(UiEvent::MoveBlock { page: 0, x: 60.0, y: 500.0 });
    let moved =
        wait_for(&core, "verschoben", |v| v.document.as_ref().is_some_and(|d| d.origin == PlacementOrigin::User))
            .await
            .document
            .unwrap();
    assert!(moved.detection.as_ref().is_some_and(|x| !x.auto_placed));
    apply(&core, 2, &[("signature.profile", json!("SIGNATURBLOCK_SMALL"))]).await;
    let v = wait_for(&core, "Maße neu", |v| {
        v.document.as_ref().is_some_and(|d| (d.block.height - moved.block.height).abs() > 1.0)
    })
    .await;
    let d = v.document.unwrap();
    assert_eq!(d.origin, PlacementOrigin::User);
    assert_eq!((d.placement.page_index, d.placement.x, d.placement.y), (0, moved.placement.x, moved.placement.y));
}

// ------------------------------------------------------------------------------------------------
// Einstellungen › Dokument: PDF/A und Platzhaltersuche
// ------------------------------------------------------------------------------------------------

/// PDF/A: eine erkannte Kennung gilt immer („PDF/A-Dokument erkannt“), ohne Kennung erzwingt
/// `signature.pdfa_force` eine PDF/A-kompatible Signatur – nur mit einem Profil, das eine PDF/A-Variante hat.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pdfa_detection_and_forced_compatibility() {
    let fx = Fixture::new();
    let a = copy_pdf(PDFA_PDF, &fx.dir("in"), "pdfa.pdf");
    let core = start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }, false, |o| o.engine = None).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let det = v.document.unwrap().detection.expect("Erkennung");
    assert_eq!(det.pdfa_decision, PdfaDecision::Detected);
    assert_eq!(det.pdfa.as_ref().map(|p| p.part), Some(1), "{det:?}");
    drop(core);

    let fx = Fixture::new();
    let b = copy_pdf(TEXT_PDF, &fx.dir("in"), "plain.pdf");
    let core = start(&fx, Overlay { input_files: vec![b], ..Overlay::default() }, false, |_| {}).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let det = v.document.unwrap().detection.unwrap();
    assert_eq!((det.pdfa, det.pdfa_decision), (None, PdfaDecision::None));
    open_settings(&core).await;
    apply(&core, 1, &[("signature.pdfa_force", json!(true))]).await;
    let v = wait_for(&core, "erzwungen", |v| {
        v.document.as_ref().and_then(|d| d.detection.as_ref()).is_some_and(|x| x.pdfa_decision == PdfaDecision::Forced)
    })
    .await;
    assert!(v.settings.unwrap().settings.signature.pdfa_force);
    // Amtssignatur hat keine PDF/A-Variante: nichts erzwungen
    apply(&core, 2, &[("signature.profile", json!("AMTSSIGNATURBLOCK"))]).await;
    wait_for(&core, "Amtssignatur", |v| {
        v.document.as_ref().and_then(|d| d.detection.as_ref()).is_some_and(|x| x.pdfa_decision == PdfaDecision::None)
    })
    .await;
}

/// Platzhaltersuche „Leere Signaturfelder“: erstes leeres Feld automatisch (Hinweis über `auto_placed`),
/// sofort wirksam in beide Richtungen; ein von Hand gewähltes Feld bleibt; signiert wird ins Feld.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn field_search_places_into_the_first_empty_field_immediately() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(FIELDS_PDF, &fx.dir("in"), "fields.pdf");
    let engine = FakeEngine::new();
    let e2 = engine.clone();
    let core = start(&fx, keystore_overlay(vec![a], &out), false, move |o| {
        o.engine = Some(e2 as std::sync::Arc<dyn pida_core_api::engine::PadesEngine>)
    })
    .unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    assert!(d.fields.len() >= 2, "Testdokument mit mehreren Feldern");
    assert_eq!(d.target, PlacementTarget::Free, "ohne Suche nur angeboten");
    open_settings(&core).await;
    apply(&core, 1, &[("placeholder.search", json!(true)), ("placeholder.kind", json!("fields"))]).await;
    let v = wait_for(&core, "im ersten Feld", |v| {
        v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Field { index: 0 })
    })
    .await;
    let d = v.document.unwrap();
    assert_eq!(d.origin, PlacementOrigin::Field);
    assert!(d.detection.as_ref().is_some_and(|x| x.auto_placed), "Hinweis „Signaturfeld gefunden …“");
    assert_eq!(d.sticker, d.fields[0].rect);
    // Suche aus: wieder frei
    apply(&core, 2, &[("placeholder.search", json!(false))]).await;
    wait_for(&core, "frei", |v| v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Free)).await;
    // von Hand ins zweite Feld: bleibt beim Einschalten der Suche
    core.dispatch(UiEvent::UseTarget { target: PlacementTarget::Field { index: 1 } });
    wait_for(&core, "zweites Feld", |v| {
        v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Field { index: 1 })
    })
    .await;
    apply(&core, 3, &[("placeholder.search", json!(true))]).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let d = core.view().document.unwrap();
    assert_eq!(d.target, PlacementTarget::Field { index: 1 });
    assert!(d.detection.as_ref().is_some_and(|x| !x.auto_placed), "von Hand: kein Hinweis");
    let second = d.fields[1].name.clone();
    core.dispatch(UiEvent::CloseSettings);
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    wait_for(&core, "gespeichert", saved).await;
    let plan = engine.last_plan();
    assert!(
        matches!(&plan.field, pida_pades::FieldTarget::Existing { qualified_name, .. } if *qualified_name == second),
        "{:?}",
        plan.field
    );
}

/// Platzhaltersuche „QR-Code-Platzhalter“ nachträglich eingeschaltet: die Suche wird nachgeholt, der Block
/// kommt in den QR-Platzhalter; aus → frei; wieder an → ohne neue Suche zurück (PDFium nötig).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn qr_search_toggled_after_opening() {
    let fx = Fixture::new();
    let a = copy_pdf(QR_PDF, &fx.dir("in"), "qr.pdf");
    let Some(core) = start(&fx, Overlay { input_files: vec![a], ..Overlay::default() }, true, |_| {}) else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – übersprungen");
        return;
    };
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    let d = v.document.unwrap();
    assert_eq!((d.target, d.qr.is_none()), (PlacementTarget::Free, true), "Suche aus (Standard)");
    open_settings(&core).await;
    apply(&core, 1, &[("placeholder.search", json!(true))]).await;
    let v =
        wait_for(&core, "im QR-Platzhalter", |v| v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Qr))
            .await;
    let d = v.document.unwrap();
    assert_eq!(d.origin, PlacementOrigin::Qr);
    assert!(d.detection.as_ref().is_some_and(|x| x.auto_placed), "Hinweis „QR-Platzhalter gefunden …“");
    assert_eq!(d.sticker, d.qr.map(|q| q.rect));
    apply(&core, 2, &[("placeholder.search", json!(false))]).await;
    wait_for(&core, "frei", |v| {
        v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Free && d.qr.is_none())
    })
    .await;
    apply(&core, 3, &[("placeholder.search", json!(true))]).await;
    wait_for(&core, "wieder im QR", |v| v.document.as_ref().is_some_and(|d| d.target == PlacementTarget::Qr)).await;
}

// ------------------------------------------------------------------------------------------------
// Schreibgeschützt, Mobile, Richtlinie (Abnahme E3 Punkt 6 und 7)
// ------------------------------------------------------------------------------------------------

/// `-c` mit einer PDF-Over-Altdatei: schreibgeschützt; Änderungen wirken bis Programmende im Speicher, es
/// entsteht keine Einstellungsdatei.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn read_only_applies_in_memory() {
    let fx = Fixture::new();
    let legacy = fx.dir("legacy").join("PDF-Over.config");
    std::fs::write(&legacy, "SIGNATURE_PROFILE=SIGNATURBLOCK_SMALL\n").unwrap();
    let overlay = Overlay { config_file: Some(legacy), ..Overlay::default() };
    let core = start(&fx, overlay, false, |_| {}).unwrap();
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    let v = open_settings(&core).await;
    assert!(settings(&v).read_only);
    let v = apply(&core, 1, &[("output.postfix", json!("_ro"))]).await;
    assert_eq!(settings(&v).settings.output.postfix, "_ro", "im Speicher wirksam");
    assert!(settings(&v).read_only);
    assert!(!fx.paths.settings_file().exists(), "keine Datei geschrieben");
}

/// Mobile: Proxy, Ausgabeordner, Updates und Keystore-Datei sind nicht einstellbar (Problem), alles andere
/// wirkt; „Zuletzt signiert“ ohne Ordneranzeige.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mobile_restrictions() {
    let fx = Fixture::new();
    // vorhandene Liste (wie nach einer Signatur)
    let store = pida_config::RecentStore::new(&fx.paths);
    let signed = copy_pdf(TEXT_PDF, &fx.dir("docs"), "Vertrag_signed.pdf");
    store
        .push(pida_config::RecentEntry {
            id: 0,
            file_name: "Vertrag_signed.pdf".into(),
            path: signed,
            signed_at: "2026-10-16T10:00:00+02:00".into(),
        })
        .unwrap();
    let mut cfg = fx.config(Overlay::default(), false);
    cfg.platform = pida_backend_api::Platform::Android;
    let opts = CoreOptions { workspace_ui: Some(true), ..CoreOptions::isolated() };
    let core = pida_core_api::Core::new_with(cfg, opts).unwrap();
    let v = wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    let recent = v.recent.expect("Liste");
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].folder_display, None, "Mobile: ohne Ordner");
    open_settings(&core).await;
    let v = apply(
        &core,
        1,
        &[
            ("proxy.mode", json!("none")),
            ("output.folder", json!("/sdcard")),
            ("general.update_check", json!(false)),
            ("keystore.file", json!("/sdcard/x.p12")),
            ("signature.profile", json!("AMTSSIGNATURBLOCK")),
        ],
    )
    .await;
    let s = settings(&v);
    for p in ["proxy.mode", "output.folder", "general.update_check", "keystore.file"] {
        assert_eq!(s.problems[p], "ng-settings-unavailable-mobile", "{p}");
    }
    assert_eq!(s.settings.signature.profile, SignatureProfile::Amtssignaturblock);
}

// ------------------------------------------------------------------------------------------------
// §3.10 Zuletzt signiert (Abnahme E3 Punkt 5)
// ------------------------------------------------------------------------------------------------

/// Nach dem Speichern ein Eintrag mit **nur** Name, Pfad und Zeit (Datei feldgenau, 0600); höchstens zehn,
/// neueste zuerst; fehlende Datei → Hinweis, Eintrag bleibt; Entfernen, Leeren; Abschalten löscht sofort
/// (auch die Datei), Wiedereinschalten beginnt leer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recent_documents_are_recorded_minimal_and_switch_off_deletes() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let input = fx.dir("in");
    // zehn ältere Einträge
    let store = pida_config::RecentStore::new(&fx.paths);
    for i in 0..10 {
        store
            .push(pida_config::RecentEntry {
                id: 0,
                file_name: format!("alt{i}.pdf"),
                path: fx.dir("alt").join(format!("alt{i}.pdf")),
                signed_at: "2026-10-01T09:00:00+02:00".into(),
            })
            .unwrap();
    }
    let a = copy_pdf(TEXT_PDF, &input, "Vertrag.pdf");
    let core = start(&fx, keystore_overlay(vec![a], &out), false, |_| {}).unwrap();
    let v = wait_for(&core, "Arbeitsbereich", editing).await;
    assert_eq!(v.recent.as_ref().map(Vec::len), Some(10));
    assert!(v.recent.as_ref().unwrap().iter().all(|r| !r.exists), "Dateien der alten Einträge fehlen");
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
    wait_for(&core, "gespeichert", saved).await;
    let v =
        wait_for(&core, "Eintrag", |v| v.recent.as_ref().is_some_and(|r| r[0].file_name == "Vertrag_signed.pdf")).await;
    let recent = v.recent.unwrap();
    assert_eq!(recent.len(), 10, "höchstens zehn");
    assert_eq!(recent[0].folder_display.as_deref(), Some(out.display().to_string().as_str()));
    assert!(recent[0].exists);
    assert!(time::OffsetDateTime::parse(&recent[0].signed_at, &time::format_description::well_known::Rfc3339).is_ok());
    assert!(recent.iter().all(|r| r.file_name != "alt0.pdf"), "ältester Eintrag fällt weg");
    // Datei feldgenau: nur id, file_name, path, signed_at
    let file = store.file().to_path_buf();
    let json: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    for entry in json.as_array().unwrap() {
        let keys: Vec<&str> = entry.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["file_name", "id", "path", "signed_at"]);
    }
    assert_eq!(json[0]["path"], json!(out.join("Vertrag_signed.pdf")));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("Keystore") && !text.contains("NUR TEST"), "kein Signiermittel, kein Unterzeichner");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    }
    // fehlende Datei: Hinweis, Eintrag bleibt
    let missing = recent.iter().find(|r| r.file_name == "alt9.pdf").unwrap().id;
    core.dispatch(UiEvent::VerifyRecent { id: missing });
    let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-recent-missing")).await;
    assert!(v.recent.unwrap().iter().any(|r| r.id == missing && !r.exists));
    // vorhandene Datei: kein Hinweis (Prüfmodus folgt mit K4)
    core.dispatch(UiEvent::VerifyRecent { id: recent[0].id });
    // entfernen, leeren
    core.dispatch(UiEvent::RemoveRecent { id: missing });
    let v = wait_for(&core, "entfernt", |v| v.recent.as_ref().is_some_and(|r| r.len() == 9)).await;
    assert!(v.recent.unwrap().iter().all(|r| r.id != missing));
    core.dispatch(UiEvent::ClearRecent);
    wait_for(&core, "leer", |v| v.recent.as_ref().is_some_and(Vec::is_empty)).await;
    assert!(!file.exists(), "Liste leeren löscht die Datei");
    // erneut ein Eintrag (nächstes Dokument), dann abschalten → sofort weg samt Datei
    store
        .push(pida_config::RecentEntry {
            id: 0,
            file_name: "x.pdf".into(),
            path: out.join("x.pdf"),
            signed_at: "2026-10-16T10:00:00+02:00".into(),
        })
        .unwrap();
    open_settings(&core).await;
    let v = apply(&core, 1, &[("general.recent_documents", json!(false))]).await;
    assert_eq!(v.recent, None);
    assert!(!file.exists(), "Abschalten löscht sofort");
    let v = apply(&core, 2, &[("general.recent_documents", json!(true))]).await;
    assert_eq!(v.recent, Some(Vec::new()), "wieder an: leer");
}

/// Abgeschaltet (Einstellung) bzw. per Richtlinie aus: nichts gespeichert, Liste `None`, vorhandene Datei
/// beim Start gelöscht; die Richtlinie sperrt den Schalter (`locked`) und lehnt Änderungen ab.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recent_documents_off_by_setting_or_policy_store_nothing() {
    for policy in [false, true] {
        let fx = Fixture::new();
        let out = fx.dir("out");
        let store = pida_config::RecentStore::new(&fx.paths);
        store
            .push(pida_config::RecentEntry {
                id: 0,
                file_name: "alt.pdf".into(),
                path: out.join("alt.pdf"),
                signed_at: "2026-10-01T09:00:00+02:00".into(),
            })
            .unwrap();
        if !policy {
            fx.write_settings("[general]\nrecent_documents = false\n");
        }
        let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "doc.pdf");
        let core = start(&fx, keystore_overlay(vec![a], &out), false, |o| {
            if policy {
                o.policy = Some(Policy { recent_documents: Some(false), ..Policy::default() });
            }
        })
        .unwrap();
        let v = wait_for(&core, "Arbeitsbereich", editing).await;
        assert_eq!(v.recent, None, "policy={policy}");
        assert!(!store.file().exists(), "policy={policy}: vorhandene Liste beim Start gelöscht");
        core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
        wait_for(&core, "gespeichert", saved).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!store.file().exists(), "policy={policy}: nichts gespeichert");
        if policy {
            let v = open_settings(&core).await;
            assert_eq!(
                settings(&v).locked.as_ref().and_then(|l| l.get("general.recent_documents")).map(String::as_str),
                Some("ng-recent-policy")
            );
            let v = apply(&core, 1, &[("general.recent_documents", json!(true))]).await;
            assert_eq!(settings(&v).problems["general.recent_documents"], "ng-recent-policy");
            assert_eq!(v.recent, None);
        }
    }
}

/// Ohne Oberfläche (CLI) wird nichts gespeichert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn headless_does_not_record_recent_documents() {
    let fx = Fixture::new();
    let out = fx.dir("out");
    let a = copy_pdf(TEXT_PDF, &fx.dir("in"), "doc.pdf");
    let overlay = Overlay {
        backend: Some(pida_config::BackendChoice::Keystore),
        auto_position: true,
        ..keystore_overlay(vec![a], &out)
    };
    let opts = CoreOptions {
        engine: Some(FakeEngine::new() as std::sync::Arc<dyn pida_core_api::engine::PadesEngine>),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(fx.config(overlay, true), opts).unwrap();
    let exit = tokio::time::timeout(Duration::from_secs(30), core.wait_finished()).await.expect("Ende");
    assert_eq!(exit, pida_workflow::ExitCode::Success);
    assert!(out.join("doc_signed.pdf").is_file());
    assert_eq!(core.view().recent, None);
    assert!(!pida_config::RecentStore::new(&fx.paths).file().exists(), "ohne Oberfläche nichts gespeichert");
}
