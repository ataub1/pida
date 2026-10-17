//! Mehrfachauswahl gegen den echten Kern (ADR 0004 Nachtrag 17, Fehler 2 der Abnahme): `open_files` übernimmt
//! **alle** gewählten Dateien in der Reihenfolge der Auswahl (ab zwei als Stapel) – von der Startseite, mit
//! geparkter Arbeit („Zum Stapel hinzufügen“, „Offene schließen und neu beginnen“), aus dem Arbeitsbereich
//! („Weitere PDFs hinzufügen“) und in die Stapelübersicht. Was nicht dazukommt, meldet der Kern statt es still
//! zu verwerfen: schon offene bzw. doppelt gewählte Dokumente (`ng-open-duplicates`) und Pfade ohne Datei
//! (`ng-open-missing`). Auf iOS/Android sind gewählte Dateien Sandbox-Kopien mit eigenem Pfad – dort gilt eine
//! Kopie mit gleichem Namen und Inhalt wie ein offenes Dokument als Duplikat; auf dem Desktop bleibt es beim
//! Pfadvergleich (F-89).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::*;
use pida_backend_api::{BackendId, Platform, SignatureBackend};
use pida_config::Overlay;
use pida_core_api::{BatchItemStatusView, Core, CoreConfig, CoreOptions, OpenFilesMode, StepView, UiEvent, ViewState};
use pida_workflow::ExistingPolicy;

const PDF: &str = "pdf/generated/simple-xref-table.pdf";

fn start(fx: &Fixture, platform: Platform) -> Core {
    start_with(fx, platform, None)
}

fn start_with(fx: &Fixture, platform: Platform, backends: Option<Vec<Arc<dyn SignatureBackend>>>) -> Core {
    let overlay = Overlay { output: Some(fx.dir("out")), ..Overlay::default() };
    let config = CoreConfig { platform, ..fx.config(overlay, false) };
    let options = CoreOptions {
        workspace_ui: Some(true),
        batch_overview: Some(true),
        backends,
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        default_output_dir: (!platform.is_desktop()).then(|| fx.dir("docs")),
        ..CoreOptions::isolated()
    };
    Core::new_with(config, options).expect("Kern startet")
}

/// PDF mit eigenem Inhalt (`tag`) unter `dir/name`.
fn pdf(dir: &Path, name: &str, tag: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = copy_pdf(PDF, dir, name);
    std::fs::write(&p, [std::fs::read(&p).unwrap().as_slice(), format!("\n%{tag}\n").as_bytes()].concat()).unwrap();
    p
}

fn names(v: &ViewState) -> Vec<String> {
    v.batch.as_ref().map(|b| b.items.iter().map(|i| i.name.clone()).collect()).unwrap_or_default()
}

async fn overview(core: &Core, n: usize) -> ViewState {
    wait_for(core, "Übersicht voranalysiert", |v| {
        matches!(v.step, StepView::Batch)
            && v.parked.is_none()
            && v.batch.as_ref().is_some_and(|b| {
                b.items.len() == n && b.items.iter().all(|i| !matches!(i.status, BatchItemStatusView::Analyzing))
            })
    })
    .await
}

async fn workspace(core: &Core, name: &str) -> ViewState {
    wait_for(core, name, |v| v.parked.is_none() && v.document.as_ref().is_some_and(|d| d.doc.name == name)).await
}

async fn parked_start(core: &Core) -> ViewState {
    wait_for(core, "Startansicht mit geparkter Arbeit", |v| {
        matches!(v.step, StepView::Open { .. }) && v.parked.is_some()
    })
    .await
}

/// Hinweis `code` (Parameter), falls vorhanden.
fn notice(v: &ViewState, code: &str) -> Option<Vec<(String, String)>> {
    v.notices.iter().find(|n| n.code == code).map(|n| n.params.clone().into_iter().collect())
}

fn params(pairs: &[(&str, &str)]) -> Option<Vec<(String, String)>> {
    Some(pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect())
}

fn dismiss_all(core: &Core) {
    for n in core.view().notices {
        core.dispatch(UiEvent::DismissNotice { id: n.id });
    }
}

const DUPLICATES: &str = "ng-open-duplicates";
const MISSING: &str = "ng-open-missing";

/// Startseite, Dateiauswahl mit drei PDFs: alle drei als Stapel, in der Reihenfolge der Auswahl, ohne Hinweis.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn all_chosen_files_open_as_batch_in_selection_order() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let files = vec![pdf(&dir, "c.pdf", "c"), pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b")];
    let core = start(&fx, Platform::MacOs);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: files, mode: None });
    let v = overview(&core, 3).await;
    assert_eq!(names(&v), ["c.pdf", "a.pdf", "b.pdf"], "Reihenfolge der Auswahl");
    assert!(notice(&v, DUPLICATES).is_none() && notice(&v, MISSING).is_none(), "{:?}", v.notices);
}

/// Dieselbe Datei zweimal in einer Auswahl: einmal im Stapel, Hinweis mit Dateinamen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_file_twice_in_one_selection_is_added_once_with_notice() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let (a, b) = (pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b"));
    let core = start(&fx, Platform::MacOs);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b, a], mode: None });
    let v = overview(&core, 2).await;
    assert_eq!(names(&v), ["a.pdf", "b.pdf"]);
    assert_eq!(notice(&v, DUPLICATES), params(&[("count", "1"), ("file", "a.pdf")]));
}

/// Geparkter Stapel (Startseite mit „In Bearbeitung“), Auswahl aus zwei offenen und einem neuen PDF, „Zum Stapel
/// hinzufügen“: das neue kommt dazu, die zwei offenen meldet der Kern (vorher still verworfen – so sah es aus,
/// als übernähme die Auswahl nur eine Datei). Danach dasselbe in der Übersicht ohne Parken.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn adding_to_parked_batch_reports_already_open_documents() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let (a, b, c, d) =
        (pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b"), pdf(&dir, "c.pdf", "c"), pdf(&dir, "d.pdf", "d"));
    let core = start(&fx, Platform::MacOs);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b.clone()], mode: None });
    overview(&core, 2).await;
    core.dispatch(UiEvent::GoHome);
    parked_start(&core).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b, c.clone()], mode: Some(OpenFilesMode::Add) });
    let v = overview(&core, 3).await;
    assert_eq!(names(&v), ["a.pdf", "b.pdf", "c.pdf"]);
    assert_eq!(notice(&v, DUPLICATES), params(&[("count", "2")]));
    // Stapelübersicht „Hinzufügen“ (ohne `mode`): d neu, c schon offen
    dismiss_all(&core);
    core.dispatch(UiEvent::OpenFiles { paths: vec![c, d], mode: None });
    let v = overview(&core, 4).await;
    assert_eq!(names(&v), ["a.pdf", "b.pdf", "c.pdf", "d.pdf"]);
    assert_eq!(notice(&v, DUPLICATES), params(&[("count", "1"), ("file", "c.pdf")]));
    // nur Duplikate: Stapel unverändert, Hinweis
    dismiss_all(&core);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    let a_open = params(&[("count", "1"), ("file", "a.pdf")]);
    let v = wait_for(&core, "Hinweis a", |v| notice(v, DUPLICATES) == a_open).await;
    assert_eq!(names(&v).len(), 4);
}

/// Arbeitsbereich „Weitere PDFs hinzufügen“: nur das offene Dokument gewählt → bleibt offen, Hinweis; mit einem
/// weiteren → beide als Stapel, Hinweis für das offene.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workspace_add_reports_the_open_document() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let (a, b) = (pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b"));
    let core = start(&fx, Platform::MacOs);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    workspace(&core, "a.pdf").await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: Some(OpenFilesMode::Add) });
    let v = wait_for(&core, "Hinweis", |v| notice(v, DUPLICATES).is_some()).await;
    assert_eq!(notice(&v, DUPLICATES), params(&[("count", "1"), ("file", "a.pdf")]));
    assert!(
        v.document.as_ref().is_some_and(|d| d.doc.name == "a.pdf") && v.batch.is_none(),
        "bleibt im Arbeitsbereich"
    );
    dismiss_all(&core);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a, b], mode: Some(OpenFilesMode::Add) });
    let v = overview(&core, 2).await;
    assert_eq!(names(&v), ["a.pdf", "b.pdf"]);
    assert_eq!(notice(&v, DUPLICATES), params(&[("count", "1"), ("file", "a.pdf")]));
}

/// „Offene schließen und neu beginnen“ mit dem geparkten Dokument in der Auswahl: es ist danach wieder dabei (kein
/// Duplikat – das offene wurde geschlossen).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replace_reopens_chosen_documents_without_duplicate_notice() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let (a, b) = (pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b"));
    let core = start(&fx, Platform::MacOs);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()], mode: None });
    workspace(&core, "a.pdf").await;
    core.dispatch(UiEvent::GoHome);
    parked_start(&core).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a, b], mode: Some(OpenFilesMode::Replace) });
    let v = overview(&core, 2).await;
    assert_eq!(names(&v), ["a.pdf", "b.pdf"]);
    assert!(notice(&v, DUPLICATES).is_none(), "{:?}", v.notices);
}

/// Abgelegter Ordner bzw. gelöschte Datei: die übrigen öffnen, Hinweis mit Anzahl.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_files_and_folders_are_reported() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let (a, b) = (pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b"));
    let core = start(&fx, Platform::MacOs);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![dir.join("weg.pdf"), a, fx.dir("ordner"), b], mode: None });
    let v = overview(&core, 2).await;
    assert_eq!(names(&v), ["a.pdf", "b.pdf"]);
    assert_eq!(notice(&v, MISSING), params(&[("count", "2")]));
    // nur fehlende: nichts öffnet, Hinweis
    dismiss_all(&core);
    core.dispatch(UiEvent::OpenFiles { paths: vec![dir.join("auch-weg.pdf")], mode: None });
    let v = wait_for(&core, "Hinweis", |v| notice(v, MISSING) == params(&[("count", "1")])).await;
    assert_eq!(v.notices.len(), 1, "der frühere Hinweis ist geschlossen: {:?}", v.notices);
    assert_eq!(names(&v).len(), 2);
}

/// iOS/Android: jede Auswahl legt neue Sandbox-Kopien an (`import/<n>/<Name>`). Eine Kopie mit gleichem Namen und
/// Inhalt wie ein offenes Dokument (bzw. wie eine frühere in derselben Auswahl) ist ein Duplikat; gleicher Name mit
/// anderem Inhalt ist ein eigenes Dokument. Reihenfolge der Auswahl.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mobile_copies_of_open_documents_are_duplicates() {
    for platform in [Platform::Ios, Platform::Android] {
        let fx = Fixture::new();
        let import = fx.dir("import");
        let first = vec![pdf(&import.join("1"), "Vertrag.pdf", "v"), pdf(&import.join("2"), "Angebot.pdf", "a")];
        let core = start(&fx, platform);
        wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
        assert!(core.view().platform.mobile);
        core.dispatch(UiEvent::OpenFiles { paths: first, mode: None });
        let v = overview(&core, 2).await;
        assert_eq!(names(&v), ["Vertrag.pdf", "Angebot.pdf"], "{platform:?}");
        // zweite Auswahl: Vertrag noch einmal (neue Kopie, gleicher Inhalt), Rechnung zweimal (zwei Kopien in derselben
        // Auswahl), ein anderes „Angebot.pdf“
        let second = vec![
            pdf(&import.join("3"), "Vertrag.pdf", "v"),
            pdf(&import.join("4"), "Rechnung.pdf", "r"),
            pdf(&import.join("5"), "Rechnung.pdf", "r"),
            pdf(&import.join("6"), "Angebot.pdf", "anders"),
        ];
        core.dispatch(UiEvent::OpenFiles { paths: second, mode: None });
        let v = overview(&core, 4).await;
        assert_eq!(names(&v), ["Vertrag.pdf", "Angebot.pdf", "Rechnung.pdf", "Angebot.pdf"], "{platform:?}");
        assert_eq!(notice(&v, DUPLICATES), params(&[("count", "2")]), "{platform:?}");
    }
}

/// Desktop: zwei Dateien mit gleichem Namen und Inhalt in verschiedenen Ordnern sind zwei Dokumente (Pfadvergleich,
/// F-89) – nur Mobile vergleicht Kopien nach Inhalt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desktop_compares_paths_not_contents() {
    let fx = Fixture::new();
    let files = vec![pdf(&fx.dir("x"), "Vertrag.pdf", "v"), pdf(&fx.dir("y"), "Vertrag.pdf", "v")];
    let core = start(&fx, Platform::Windows);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: files, mode: None });
    let v = overview(&core, 2).await;
    assert_eq!(names(&v), ["Vertrag.pdf", "Vertrag.pdf"]);
    assert!(notice(&v, DUPLICATES).is_none());
}

/// Viele Dateien in einer Auswahl (24 PDFs und eine Datei, die kein PDF ist – unter Windows/Linux über „Alle
/// Dateien“ wählbar): alle kommen in der Reihenfolge der Auswahl in den Stapel; das Nicht-PDF bleibt als
/// abgelehnter Eintrag sichtbar statt still zu verschwinden.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_files_and_a_non_pdf_all_arrive_in_selection_order() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let mut files: Vec<PathBuf> = (1..=24).map(|i| pdf(&dir, &format!("dok-{i:02}.pdf"), &i.to_string())).collect();
    let text = dir.join("notiz.txt");
    std::fs::write(&text, "kein PDF").unwrap();
    files.insert(10, text);
    let core = start(&fx, Platform::Linux);
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: files.clone(), mode: None });
    let v = overview(&core, 25).await;
    let chosen: Vec<String> = files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names(&v), chosen, "alle, Reihenfolge der Auswahl");
    let items = &v.batch.as_ref().unwrap().items;
    let rejected: Vec<&str> = items
        .iter()
        .filter(|i| matches!(i.status, BatchItemStatusView::Rejected { .. }))
        .map(|i| i.name.as_str())
        .collect();
    assert_eq!(rejected, ["notiz.txt"]);
    assert!(notice(&v, DUPLICATES).is_none() && notice(&v, MISSING).is_none(), "{:?}", v.notices);
}

/// Laufender Stapel („Alle signieren“ wartet auf die Anmeldung): „Öffnen mit“ bzw. eine zweite Instanz liefern
/// trotzdem `open_files` (die Einstiege der Oberfläche sind gesperrt). Das neue PDF kommt als Eintrag dazu
/// (`ng-batch-added`, nicht in den laufenden Lauf), das schon offene meldet der Kern statt es still zu verwerfen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn running_batch_reports_already_open_documents() {
    let fx = Fixture::new();
    let dir = fx.dir("in");
    let (a, b, c) = (pdf(&dir, "a.pdf", "a"), pdf(&dir, "b.pdf", "b"), pdf(&dir, "c.pdf", "c"));
    let atrust = FakeAtrust::new();
    let core = start_with(&fx, Platform::MacOs, Some(vec![atrust.backend()]));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b], mode: None });
    let v = overview(&core, 2).await;
    core.dispatch(UiEvent::SelectSigner { backend: Some(BackendId::AtrustNative), make_default: false });
    let docs = v.batch.as_ref().unwrap().items.iter().map(|i| i.doc).collect();
    core.dispatch(UiEvent::StartBatch {
        backend: BackendId::AtrustNative,
        docs,
        output_dir: Some(fx.dir("signiert")),
        on_existing: ExistingPolicy::KeepBoth,
    });
    wait_for(&core, "Lauf wartet auf die Anmeldung", |v| {
        matches!(v.step, StepView::Signing { .. }) && v.batch.as_ref().is_some_and(|b| b.running)
    })
    .await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a, c], mode: None });
    let v = wait_for(&core, "Hinweise", |v| notice(v, DUPLICATES).is_some()).await;
    assert_eq!(notice(&v, DUPLICATES), params(&[("count", "1"), ("file", "a.pdf")]));
    assert_eq!(notice(&v, "ng-batch-added"), params(&[("count", "1")]));
    assert_eq!(names(&v), ["a.pdf", "b.pdf", "c.pdf"]);
    assert!(v.batch.as_ref().unwrap().running, "der Lauf geht weiter");
}
