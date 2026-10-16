//! Ersteinrichtung gegen den echten Kern (ADR 0004 §3.12, Etappe E4 Punkt 5, Paket K4): nur bei fehlender
//! Einstellungsdatei (Bestandsnutzer nie, ohne Oberfläche nichts, klassischer Ablauf unverändert), mit
//! Altkonfiguration die Liste der Übernahmen (ohne direkt Schritt 2), „Später einrichten“, Abbruch vor
//! „Fertig“ → beim nächsten Start erneut, erneut anbieten über Einstellungen › Erweitert (`OfferMigration`),
//! Neuinstallation speichert „neben dem Original“ (§8 F4), mit Übernahme gilt der PDF-Over-Wert; die
//! Bildmarke nur, wenn in PDF-Over eine **eigene** gesetzt war (sonst bleibt das PIDA-Siegel, ADR 0005).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::*;
use pida_config::{BackendChoice, Overlay, Settings, SignatureProfile};
use pida_core_api::{CoreOptions, StepView, UiEvent, ViewState};
use pida_i18n::Lang;

/// Kern mit Oberfläche im Arbeitsbereich-Modus; `legacy` = Altordner `~/.pdf-over`.
fn start(fx: &Fixture, legacy: Option<&Path>, adjust: impl FnOnce(&mut CoreOptions)) -> pida_core_api::Core {
    let mut opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        legacy_dir: legacy.map(Path::to_path_buf),
        ..CoreOptions::isolated()
    };
    adjust(&mut opts);
    pida_core_api::Core::new_with(fx.config(Overlay::default(), false), opts).expect("Kern startet")
}

fn started(v: &ViewState) -> bool {
    !matches!(v.step, StepView::Starting)
}

fn file_settings(fx: &Fixture) -> Option<Settings> {
    let text = std::fs::read_to_string(fx.paths.settings_file()).ok()?;
    Some(Settings::from_toml(&text).expect("settings.toml lesbar"))
}

async fn current_settings(core: &pida_core_api::Core) -> Settings {
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_for(core, "Einstellungen", |v| v.settings.is_some()).await;
    core.dispatch(UiEvent::CloseSettings);
    wait_for(core, "geschlossen", |v| v.settings.is_none()).await;
    v.settings.unwrap().settings
}

/// Altordner mit `PDF-Over.config` (Unix-Pfade im Temp-Ordner, erfundene Werte).
fn legacy_home(fx: &Fixture, emblem: Option<&Path>) -> (PathBuf, PathBuf) {
    let dir = fx.dir("home/.pdf-over");
    let out = fx.dir("home/Dokumente/Signiert");
    let mut config = format!(
        "#Configuration file was generated!\nBKU=MOBILE\nMOBILE_NUMBER=+436641234567\nOUTPUT_FOLDER={}\n\
         SIGNATURE_PROFILE=AMTSSIGNATURBLOCK\nSIGNLOCALE=en\nPROXY_HOST=proxy.example.test\nPROXY_PORT=3128\n\
         SAVE_FILE_POSTFIX=_signed\n",
        out.display()
    );
    if let Some(e) = emblem {
        config.push_str(&format!("EMBLEM={}\n", e.display()));
    }
    std::fs::write(dir.join("PDF-Over.config"), config).unwrap();
    (dir, out)
}

fn item_codes(v: &ViewState) -> Vec<String> {
    let m = v.first_run.as_ref().and_then(|f| f.migration.as_ref()).expect("Übernahmeangebot in der Ersteinrichtung");
    m.items.iter().map(|i| i.code.clone()).collect()
}

/// Neuinstallation ohne Altkonfiguration: Ersteinrichtung ohne Schritt 1, „neben dem Original“; Schritte 2/3
/// wirken sofort (Vorschau neu); Abbruch vor „Fertig“ → beim nächsten Start erneut; „Fertig“ speichert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fresh_install_runs_first_run_and_saves_next_to_the_original() {
    let fx = Fixture::new();
    let core = start(&fx, None, |_| {});
    let v = wait_for(&core, "Ersteinrichtung mit Vorschau", |v| {
        started(v) && v.first_run.as_ref().is_some_and(|f| f.preview_revision > 0)
    })
    .await;
    let f = v.first_run.clone().unwrap();
    assert!(f.migration.is_none() && v.migration.is_none(), "ohne Altkonfiguration kein Schritt 1");
    assert_eq!((f.default_backend, f.profile, f.block_language), (None, SignatureProfile::default(), None));
    assert_eq!(current_settings(&core).await.output.folder.as_deref(), Some(Path::new(".")), "F4");
    drop(core);

    // App vor „Fertig“ beendet → nächster Start erneut
    let core = start(&fx, None, |_| {});
    let v = wait_for(&core, "erneut", started).await;
    assert!(v.first_run.is_some(), "Ersteinrichtung nach Abbruch erneut");
    let preview = wait_for(&core, "Vorschau", |v| v.first_run.as_ref().is_some_and(|f| f.preview_revision > 0))
        .await
        .first_run
        .unwrap()
        .preview_revision;

    // Schritte 2/3: sofort übernommen und gespeichert, Vorschau neu
    core.dispatch(UiEvent::UpdateFirstRun {
        default_backend: Some(BackendChoice::Keystore),
        profile: SignatureProfile::Amtssignaturblock,
        block_language: Some(Lang::En),
    });
    let v = wait_for(&core, "übernommen", |v| {
        v.first_run
            .as_ref()
            .is_some_and(|f| f.profile == SignatureProfile::Amtssignaturblock && f.preview_revision > preview)
    })
    .await;
    let f = v.first_run.unwrap();
    assert_eq!((f.default_backend, f.block_language), (Some(BackendChoice::Keystore), Some(Lang::En)));
    let saved = file_settings(&fx).expect("gespeichert");
    assert_eq!(saved.signature.profile, SignatureProfile::Amtssignaturblock);
    assert!(!saved.general.first_run_completed, "noch nicht abgeschlossen");
    assert_eq!(saved.output.folder.as_deref(), Some(Path::new(".")));

    // „Fertig“
    core.dispatch(UiEvent::FinishFirstRun);
    let v = wait_for(&core, "abgeschlossen", |v| v.first_run.is_none()).await;
    assert!(v.notices.iter().any(|n| n.code == "ng-firstrun-done"));
    let saved = file_settings(&fx).unwrap();
    assert!(saved.general.first_run_completed);
    assert_eq!(saved.backend.default, Some(BackendChoice::Keystore));
    drop(core);
    let core = start(&fx, None, |_| {});
    let v = wait_for(&core, "Start", started).await;
    assert!(v.first_run.is_none(), "nach „Fertig“ nie wieder");
    // ohne Ersteinrichtung ohne Wirkung
    core.dispatch(UiEvent::UpdateFirstRun {
        default_backend: None,
        profile: SignatureProfile::Invisible,
        block_language: None,
    });
    core.dispatch(UiEvent::FinishFirstRun);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(file_settings(&fx).unwrap().signature.profile, SignatureProfile::Amtssignaturblock);
}

/// Bestandsnutzer (Datei ohne Schlüssel) sehen die Ersteinrichtung nie, `output.folder` bleibt `None`
/// (Speichern-Dialog); ohne Oberfläche und im klassischen Ablauf setzt der Kern nichts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn existing_users_headless_and_classic_never_see_it() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nprofile = \"SIGNATURBLOCK_SMALL\"\n");
    let (legacy, _) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Start", started).await;
    assert!(v.first_run.is_none() && v.migration.is_none());
    assert_eq!(current_settings(&core).await.output.folder, None);
    drop(core);

    // ohne Oberfläche (CLI): keine Ersteinrichtung, nichts gesetzt
    let fx = Fixture::new();
    let core = fx.start(Overlay::default(), true, CoreOptions { workspace_ui: Some(true), ..CoreOptions::isolated() });
    core.wait_finished().await;
    let v = core.view();
    assert!(v.first_run.is_none());
    if let Some(s) = file_settings(&fx) {
        assert!(s.general.first_run_completed && s.output.folder.is_none(), "headless setzt nichts");
    }
    drop(core);

    // klassischer Ablauf (ohne Arbeitsbereich): unverändert
    let fx = Fixture::new();
    let core = fx.start(Overlay::default(), false, CoreOptions::isolated());
    let v = wait_for(&core, "Start", started).await;
    assert!(v.first_run.is_none());
    assert_eq!(current_settings(&core).await.output.folder, None, "klassisch: Speichern-Dialog wie bisher");
}

/// Mit Altkonfiguration: Schritt 1 mit der Liste der Übernahmen; bis zur Entscheidung wird nichts gespeichert
/// (das Angebot bleibt für den nächsten Start); „Übernehmen“ → PDF-Over-Werte (auch der Ausgabeordner),
/// Einrichtung bleibt offen und erscheint nach einem Abbruch ohne Schritt 1 erneut.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn legacy_config_lists_the_items_and_accept_keeps_pdf_over_values() {
    let fx = Fixture::new();
    let (legacy, out) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Schritt 1", |v| started(v) && v.first_run.as_ref().is_some_and(|f| f.migration.is_some()))
        .await;
    assert_eq!(v.migration, v.first_run.as_ref().unwrap().migration, "gleich ViewState::migration");
    let code = |c: &str| format!("ng-migration-item-{c}");
    assert_eq!(item_codes(&v), vec![code("block"), code("username"), code("output"), code("proxy")]);
    let items = &v.first_run.as_ref().unwrap().migration.as_ref().unwrap().items;
    assert_eq!(items[1].detail.as_deref(), Some("+43 664 ···567"), "Telefonnummer maskiert");
    assert_eq!(items[2].detail.as_deref(), Some(out.display().to_string().as_str()));
    assert_eq!(items[3].detail.as_deref(), Some("proxy.example.test:3128"));
    assert_eq!(current_settings(&core).await.output.folder, None, "mit Angebot kein „neben dem Original“");
    // Kernänderungen bis zur Entscheidung nur im Speicher
    core.dispatch(UiEvent::SetUiLanguage { language: Some(Lang::En) });
    wait_for(&core, "Sprache", |v| v.language == Lang::En).await;
    assert!(!fx.paths.settings_file().exists(), "keine settings.toml vor der Entscheidung");
    drop(core);

    // Neustart ohne Entscheidung: das Angebot ist wieder da
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Schritt 1 erneut", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    assert_eq!(item_codes(&v).len(), 4);
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    assert!(v.notices.iter().any(|n| n.code == "ng-migrate-done"));
    let f = v.first_run.unwrap();
    assert_eq!(
        (f.default_backend, f.profile, f.block_language),
        (Some(BackendChoice::IdAustria), SignatureProfile::Amtssignaturblock, Some(Lang::En)),
        "Schritte 2/3 zeigen die übernommenen Werte"
    );
    let saved = file_settings(&fx).expect("Übernahme geschrieben");
    assert_eq!(saved.output.folder.as_deref(), Some(out.as_path()), "PDF-Over-Wert statt „neben dem Original“");
    assert!(!saved.general.first_run_completed);
    assert_eq!(saved.signature.emblem, None, "ohne EMBLEM bleibt das PIDA-Siegel");
    drop(core);

    // Abbruch vor „Fertig“: erneut, aber ohne Schritt 1 (direkt Schritt 2)
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Schritt 2", started).await;
    let f = v.first_run.expect("Ersteinrichtung erneut");
    assert!(f.migration.is_none() && v.migration.is_none());
    core.dispatch(UiEvent::FinishFirstRun);
    wait_for(&core, "fertig", |v| v.first_run.is_none()).await;
    assert_eq!(file_settings(&fx).unwrap().output.folder.as_deref(), Some(out.as_path()));
}

/// „Nicht übernehmen“ = Neuinstallation („neben dem Original“); „Später einrichten“ mit offenem Angebot
/// übernimmt nichts; Einstellungen › Erweitert bietet die Altkonfiguration erneut an und übernimmt sie über die
/// vorhandene Datei (Sicherung `settings.toml.before-import`); ohne Altkonfiguration ein Hinweis.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decline_later_and_offer_again() {
    // ablehnen
    let fx = Fixture::new();
    let (legacy, _) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    wait_for(&core, "Schritt 1", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::DeclineMigration);
    let v = wait_for(&core, "Schritt 2", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    assert!(v.migration.is_none());
    let saved = file_settings(&fx).expect("Entscheidung gespeichert");
    assert_eq!(saved.output.folder.as_deref(), Some(Path::new(".")), "abgelehnt = Neuinstallation");
    assert!(!saved.general.first_run_completed);
    drop(core);

    // „Später einrichten“ mit offenem Angebot
    let fx = Fixture::new();
    let (legacy, out) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    wait_for(&core, "Schritt 1", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::FinishFirstRun);
    let v = wait_for(&core, "später", |v| v.first_run.is_none()).await;
    assert!(v.migration.is_none(), "nichts übernommen, kein separates Fenster");
    let saved = file_settings(&fx).unwrap();
    assert!(saved.general.first_run_completed);
    assert_eq!((saved.output.folder.as_deref(), saved.backend.default), (Some(Path::new(".")), None));

    // erneut über Einstellungen › Erweitert
    core.dispatch(UiEvent::OfferMigration);
    let v = wait_for(&core, "Angebot", |v| v.migration.as_ref().is_some_and(|m| !m.items.is_empty())).await;
    assert!(v.first_run.is_none(), "keine Ersteinrichtung mehr");
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    wait_for(&core, "übernommen", |v| v.notices.iter().any(|n| n.code == "ng-migrate-done")).await;
    let saved = file_settings(&fx).unwrap();
    assert_eq!(saved.output.folder.as_deref(), Some(out.as_path()));
    assert_eq!(saved.backend.default, Some(BackendChoice::IdAustria));
    assert!(saved.general.first_run_completed, "Bestand: keine neue Ersteinrichtung");
    assert!(fx.paths.config_dir.join("settings.toml.before-import").is_file(), "vorher gesichert");
    let v = core.view();
    assert!(v.first_run.is_none() && v.migration.is_none());
    drop(core);

    // ohne Altkonfiguration: Hinweis
    let fx = Fixture::new();
    fx.write_settings("[general]\n");
    let core = start(&fx, None, |_| {});
    wait_for(&core, "Start", started).await;
    core.dispatch(UiEvent::OfferMigration);
    let v = wait_for(&core, "Hinweis", |v| v.notices.iter().any(|n| n.code == "ng-migration-not-found")).await;
    assert!(v.migration.is_none());
}

/// Bildmarke (Entscheidung des Auftraggebers, ADR 0005): nur eine **eigene** wird übernommen (und in den
/// Datenordner kopiert); zeigt `EMBLEM` auf die Standard-Bildmarke von PDF-AS, bleibt das PIDA-Siegel.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_an_own_emblem_is_imported() {
    let seal_png = Path::new(TESTDATA).join("../crates/sigblock/assets/images/signatur-logo_de.png");
    // Standard-Bildmarke aus der PDF-AS-Konfiguration von PDF-Over
    let fx = Fixture::new();
    let images = fx.dir("home/.pdf-over/cfg/images");
    let default_logo = images.join("signatur-logo_de.png");
    std::fs::copy(&seal_png, &default_logo).unwrap();
    let (legacy, _) = legacy_home(&fx, Some(&default_logo));
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Schritt 1", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    assert!(!item_codes(&v).contains(&"ng-migration-item-emblem".to_owned()), "{:?}", item_codes(&v));
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    wait_for(&core, "übernommen", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    assert_eq!(file_settings(&fx).unwrap().signature.emblem, None, "PIDA-Siegel bleibt");
    drop(core);

    // eigene Bildmarke: übernommen (Kopie im Datenordner), in der Liste mit Dateinamen
    let fx = Fixture::new();
    let own = fx.dir("home/Bilder").join("firmenlogo.png");
    std::fs::copy(testdata("config/legacy-home/.pdf-over/.emblem.png"), &own).unwrap();
    let (legacy, _) = legacy_home(&fx, Some(&own));
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Schritt 1", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    let items = v.first_run.unwrap().migration.unwrap().items;
    let emblem = items.iter().find(|i| i.code == "ng-migration-item-emblem").expect("Bildmarke in der Liste");
    assert_eq!(emblem.detail.as_deref(), Some("firmenlogo.png"));
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    wait_for(&core, "übernommen", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    let copied = file_settings(&fx).unwrap().signature.emblem.expect("eigene Bildmarke übernommen");
    assert!(copied.starts_with(fx.paths.emblem_dir()) && copied.is_file(), "{copied:?}");
    assert_eq!(current_settings(&core).await.signature.emblem.as_deref(), Some(copied.as_path()));
}

/// Mobile: Ersteinrichtung ja (Signiermittel, Signaturbild), aber kein „neben dem Original“ (die App speichert
/// in die App-Dokumente).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mobile_first_run_keeps_the_app_documents() {
    let fx = Fixture::new();
    let mut cfg = fx.config(Overlay::default(), false);
    cfg.platform = pida_backend_api::Platform::Android;
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        default_output_dir: Some(fx.dir("Dokumente")),
        persist_prepared: Some(false),
        ..CoreOptions::isolated()
    };
    let core = pida_core_api::Core::new_with(cfg, opts).expect("Kern");
    let v = wait_for(&core, "Ersteinrichtung", |v| started(v) && v.first_run.is_some()).await;
    assert!(v.first_run.unwrap().migration.is_none());
    assert_eq!(current_settings(&core).await.output.folder, None);
}
