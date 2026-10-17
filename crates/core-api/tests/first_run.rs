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

/// Pfad als Wert einer Java-Properties-Datei, wie PDF-Over ihn schreibt (`Properties.store` maskiert `\` und
/// `:` – Windows-Pfade wie `C:\Users\…` stehen in echten `PDF-Over.config` als `C\:\\Users\\…`).
fn prop(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\").replace(':', "\\:")
}

/// Altordner mit `PDF-Over.config` (Unix-Pfade im Temp-Ordner, erfundene Werte).
fn legacy_home(fx: &Fixture, emblem: Option<&Path>) -> (PathBuf, PathBuf) {
    let dir = fx.dir("home/.pdf-over");
    let out = fx.dir("home/Dokumente/Signiert");
    let mut config = format!(
        "#Configuration file was generated!\nBKU=MOBILE\nMOBILE_NUMBER=+436641234567\nOUTPUT_FOLDER={}\n\
         SIGNATURE_PROFILE=AMTSSIGNATURBLOCK\nSIGNLOCALE=en\nPROXY_HOST=proxy.example.test\nPROXY_PORT=3128\n\
         SAVE_FILE_POSTFIX=_signed\n",
        prop(&out)
    );
    if let Some(e) = emblem {
        config.push_str(&format!("EMBLEM={}\n", prop(e)));
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
    // Pfade und (maskierte) Telefonnummer nie im `Debug` (Regel 9)
    let debug = format!("{v:?}");
    for secret in [".pdf-over", "PDF-Over.config", "Signiert", "567", "proxy.example.test"] {
        assert!(!debug.contains(secret), "„{secret}“ im Debug des ViewState");
    }
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
    let (legacy, out) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    wait_for(&core, "Schritt 1", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::DeclineMigration);
    let v = wait_for(&core, "Schritt 2", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    assert!(v.migration.is_none());
    let saved = file_settings(&fx).expect("Entscheidung gespeichert");
    assert_eq!(saved.output.folder.as_deref(), Some(Path::new(".")), "abgelehnt = Neuinstallation");
    assert!(!saved.general.first_run_completed);
    // noch in der Ersteinrichtung erneut angeboten und übernommen (über die eben geschriebene Datei): die
    // Einrichtung bleibt offen, obwohl die übernommenen Einstellungen „abgeschlossen“ tragen
    core.dispatch(UiEvent::OfferMigration);
    wait_for(&core, "erneut", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| v.notices.iter().any(|n| n.code == "ng-migrate-done")).await;
    assert!(v.first_run.expect("Ersteinrichtung bleibt offen").migration.is_none());
    let saved = file_settings(&fx).unwrap();
    assert!(!saved.general.first_run_completed);
    assert_eq!(saved.output.folder.as_deref(), Some(out.as_path()), "PDF-Over-Wert");
    drop(core);

    // „Später einrichten“ mit offenem Angebot (`PostponeFirstRun`): wie „Fertig“ abgeschlossen, aber ohne
    // „Einrichtung abgeschlossen“ – der Hinweis sagt, wo es weitergeht (Mobile-Check 16.10.2026)
    let fx = Fixture::new();
    let (legacy, out) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    wait_for(&core, "Schritt 1", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::PostponeFirstRun);
    let v = wait_for(&core, "später", |v| v.first_run.is_none()).await;
    assert!(v.migration.is_none(), "nichts übernommen, kein separates Fenster");
    assert!(v.notices.iter().any(|n| n.code == "ng-firstrun-later"), "{:?}", v.notices);
    assert!(!v.notices.iter().any(|n| n.code == "ng-firstrun-done"), "nicht „Einrichtung abgeschlossen“");
    let saved = file_settings(&fx).unwrap();
    assert!(saved.general.first_run_completed);
    assert_eq!((saved.output.folder.as_deref(), saved.backend.default), (Some(Path::new(".")), None));

    // erneut über Einstellungen › Erweitert (Einstellungen offen: die Live-Vorschau zeigt danach das
    // übernommene Signaturbild)
    core.dispatch(UiEvent::OpenSettings);
    wait_for(&core, "Einstellungen", |v| v.settings.as_ref().is_some_and(|s| s.preview_revision > 0)).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await; // keine Vorschau mehr unterwegs
    let preview = core.view().settings.unwrap().preview_revision;
    core.dispatch(UiEvent::OfferMigration);
    let v = wait_for(&core, "Angebot", |v| v.migration.as_ref().is_some_and(|m| !m.items.is_empty())).await;
    assert!(v.first_run.is_none(), "keine Ersteinrichtung mehr");
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| {
        v.notices.iter().any(|n| n.code == "ng-migrate-done")
            && v.settings.as_ref().is_some_and(|s| s.preview_revision > preview)
    })
    .await;
    assert_eq!(v.settings.unwrap().settings.signature.profile, SignatureProfile::Amtssignaturblock);
    core.dispatch(UiEvent::CloseSettings);
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

/// Abnahme E4 Punkt 5 [K], §8 F4 Ende-zu-Ende: Nach der Ersteinrichtung einer Neuinstallation legt PIDA das
/// signierte Dokument **neben das Original** (echte Engine, Keystore) – ohne Speichern-Dialog.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fresh_install_saves_the_signed_file_next_to_the_original() {
    let fx = Fixture::new();
    let input_dir = fx.dir("Dokumente/Verträge");
    let a = copy_pdf("pdf/pdfas/pdf-as-legacy/simple.pdf", &input_dir, "Mietvertrag.pdf");
    let overlay = Overlay {
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(zeroize::Zeroizing::new("test".into())),
        ..Overlay::default()
    };
    let opts = CoreOptions { workspace_ui: Some(true), ..CoreOptions::isolated() };
    let core = pida_core_api::Core::new_with(fx.config(overlay, false), opts).expect("Kern startet");
    wait_for(&core, "Ersteinrichtung", |v| started(v) && v.first_run.is_some()).await;
    core.dispatch(UiEvent::FinishFirstRun);
    wait_for(&core, "fertig", |v| v.first_run.is_none()).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![a], mode: None });
    wait_for(&core, "Arbeitsbereich", |v| {
        v.document.as_ref().is_some_and(|d| d.phase == pida_core_api::DocumentPhase::Editing)
    })
    .await;
    core.dispatch(UiEvent::SignWith { backend: pida_backend_api::BackendId::Keystore });
    let v = wait_for(
        &core,
        "gespeichert",
        |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some() || result.save_dialog.is_some()),
    )
    .await;
    let StepView::Output { result, .. } = &v.step else { unreachable!() };
    assert!(result.save_dialog.is_none(), "kein Speichern-Dialog");
    assert_eq!(result.path.as_deref(), Some(input_dir.join("Mietvertrag_signed.pdf").as_path()));
    assert!(input_dir.join("Mietvertrag_signed.pdf").is_file());
}

/// „Einstellungen zurücksetzen“ behält den Altordner: Einstellungen › Erweitert bietet die PDF-Over-Einstellungen
/// danach weiter an (statt „keine gefunden“).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn offer_migration_still_finds_the_legacy_config_after_a_reset() {
    let fx = Fixture::new();
    fx.write_settings("[general]\n");
    let (legacy, _) = legacy_home(&fx, None);
    let core = start(&fx, Some(&legacy), |_| {});
    wait_for(&core, "Start", started).await;
    core.dispatch(UiEvent::ResetSettings);
    wait_for(&core, "zurückgesetzt", |v| v.notices.iter().any(|n| n.code == "ng-config-reset-done")).await;
    wait_for(&core, "Start", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OfferMigration);
    let v = wait_for(&core, "Angebot", |v| {
        v.migration.is_some() || v.notices.iter().any(|n| n.code == "ng-migration-not-found")
    })
    .await;
    assert!(
        v.migration.as_ref().is_some_and(|m| !m.items.is_empty()),
        "Angebot nach dem Zurücksetzen: {:?}",
        v.notices
    );
}

// ------------------------------------------------------------------------------------------------
// Keine Sitzung (A1) und Zustände des echten Kerns für die Oberfläche (B4)
// ------------------------------------------------------------------------------------------------

fn editing(v: &ViewState) -> bool {
    v.document.as_ref().is_some_and(|d| d.phase == pida_core_api::DocumentPhase::Editing)
}

/// F-54/A1: Die Ersteinrichtung (Übernahme mit `BKU=MOBILE`, Standard-Signiermittel ID Austria, automatische
/// Position, Schritte 2/3, „Fertig“) und das erneute Angebot öffnen keine Sitzung – auch mit offenem Dokument;
/// nur „Signieren“ tut das.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn first_run_events_never_open_a_session() {
    use std::sync::atomic::Ordering;
    let fx = Fixture::new();
    let legacy = fx.dir("home/.pdf-over");
    std::fs::write(
        legacy.join("PDF-Over.config"),
        "BKU=MOBILE\nMOBILE_NUMBER=+436641234567\nSIGNATURE_POSITION=auto\n",
    )
    .unwrap();
    let a = copy_pdf("pdf/pdfas/pdf-as-legacy/simple.pdf", &fx.dir("in"), "Antrag.pdf");
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn pida_backend_api::SignatureBackend>> = vec![fake.backend()];
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        backends: Some(backends),
        legacy_dir: Some(legacy),
        ..CoreOptions::isolated()
    };
    let core =
        pida_core_api::Core::new_with(fx.config(Overlay { input_files: vec![a], ..Overlay::default() }, false), opts)
            .expect("Kern");
    wait_for(&core, "Schritt 1", |v| editing(v) && v.first_run.as_ref().is_some_and(|f| f.migration.is_some())).await;
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| v.first_run.as_ref().is_some_and(|f| f.migration.is_none())).await;
    assert_eq!(v.first_run.unwrap().default_backend, Some(BackendChoice::IdAustria));
    core.dispatch(UiEvent::UpdateFirstRun {
        default_backend: Some(BackendChoice::IdAustria),
        profile: SignatureProfile::SignaturblockSmall,
        block_language: None,
    });
    core.dispatch(UiEvent::FinishFirstRun);
    wait_for(&core, "fertig", |v| v.first_run.is_none()).await;
    core.dispatch(UiEvent::OfferMigration);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(editing(&core.view()), "{:?}", core.view().step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "Ersteinrichtung startet nie eine Sitzung");
}

/// Während des Signierens (Anmeldung offen, `actions.open_files` aus) bleiben „Erneut signieren“ aus „Zuletzt
/// signiert“ (ADR 0004 Nachtrag 15, auch mit `mode`) und das erneute Übernahmeangebot ohne Wirkung: kein
/// zusätzliches Dokument in der Warteschlange, kein Angebot, das Einstellungen und Signiermittel tauschte, und
/// nur die ausdrücklich gestartete Sitzung (A1, F-54). Bis Nachtrag 15 stand dieser Fall in `verify.rs`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recent_and_migration_events_do_nothing_while_signing() {
    use std::sync::atomic::Ordering;
    let fx = Fixture::new();
    fx.write_settings("[general]\n");
    let legacy = fx.dir("home/.pdf-over");
    std::fs::write(legacy.join("PDF-Over.config"), "BKU=KS\nSIGNATURE_PROFILE=AMTSSIGNATURBLOCK\n").unwrap();
    let a = copy_pdf("pdf/pdfas/pdf-as-legacy/simple.pdf", &fx.dir("in"), "Antrag.pdf");
    let signed = copy_pdf("pdf/pdfas/pdf-as-legacy/simple.pdf", &fx.dir("out"), "Vertrag_signed.pdf");
    let entry = pida_config::RecentStore::new(&fx.paths)
        .push(pida_config::RecentEntry {
            id: 0,
            file_name: "Vertrag_signed.pdf".into(),
            path: signed,
            signed_at: "2026-10-16T10:00:00+02:00".into(),
        })
        .unwrap();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn pida_backend_api::SignatureBackend>> = vec![fake.backend()];
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        backends: Some(backends),
        legacy_dir: Some(legacy),
        // wie die App: mit Stapelübersicht
        batch_overview: Some(true),
        ..CoreOptions::isolated()
    };
    let overlay = Overlay { input_files: vec![a], output: Some(fx.dir("out")), ..Overlay::default() };
    let core = pida_core_api::Core::new_with(fx.config(overlay, false), opts).expect("Kern");
    wait_for(&core, "Arbeitsbereich", editing).await;
    core.dispatch(UiEvent::SignWith { backend: pida_backend_api::BackendId::AtrustNative });
    let v = wait_for(&core, "Anmeldung", |v| matches!(v.step, StepView::Signing { .. })).await;
    assert!(!v.actions.open_files, "beim Signieren nichts öffnen");
    assert!(v.recent.as_ref().is_some_and(|r| r.iter().any(|i| i.id == entry.id && i.exists)));
    core.dispatch(UiEvent::SignRecent { id: entry.id, mode: None });
    core.dispatch(UiEvent::SignRecent { id: entry.id, mode: Some(pida_core_api::OpenFilesMode::Add) });
    core.dispatch(UiEvent::OfferMigration);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let v = core.view();
    assert!(matches!(v.step, StepView::Signing { .. }), "{:?}", v.step);
    assert!(v.migration.is_none(), "kein Übernahmeangebot mitten im Signieren");
    assert_eq!(v.document.as_ref().map(|d| d.doc.name.as_str()), Some("Antrag.pdf"));
    assert!(v.batch.is_none() && !v.notices.iter().any(|n| n.code == "ng-batch-added"), "{:?}", v.notices);
    assert_eq!(core.open_document_count(), 1, "kein weiteres Dokument geladen");
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1, "nur die ausdrücklich gestartete Sitzung");
    // nach dem Abbruch: dasselbe Dokument, keine weiteren in der Warteschlange („Zur Stapelübersicht“ bliebe sonst frei)
    core.dispatch(UiEvent::Cancel);
    let v = wait_for(&core, "Arbeitsbereich nach dem Abbruch", editing).await;
    assert_eq!(v.document.as_ref().map(|d| d.doc.name.as_str()), Some("Antrag.pdf"));
    assert!(!v.actions.show_batch && v.batch.is_none(), "nichts dazugekommen");
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1);
}

/// Echte `ViewState`s der Ersteinrichtung für die Oberfläche (B4, ADR 0004 §6.2 Punkt 4; eigene Datei neben
/// `real-core-views.json` und `real-core-batch-views.json`): ohne und mit Übernahmeangebot (Liste der
/// Übernahmen) und nach dem Übernehmen. Pfade und Zeitpunkte ersetzt. Die Zustände des Prüfmodus sind mit ihm
/// entfallen (ADR 0004 Nachtrag 15). Neu schreiben:
/// `PIDA_BLESS=1 cargo test -p pida-core-api --test first_run real_core_tools_views`.
const UI_FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/app/src/core/fixtures/real-core-tools-views.json");

fn fixture_state(v: &ViewState, roots: &[(String, &str)]) -> serde_json::Value {
    fn walk(value: &mut serde_json::Value, roots: &[(String, &str)]) {
        match value {
            serde_json::Value::String(s) => {
                for (from, to) in roots {
                    if s.contains(from.as_str()) {
                        *s = s.replace(from.as_str(), to);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|i| walk(i, roots)),
            serde_json::Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    if matches!(key.as_str(), "signing_time" | "occurred_at" | "signed_at") && item.is_string() {
                        *item = serde_json::Value::String("2026-10-15T10:42:17+02:00".into());
                    } else {
                        walk(item, roots);
                    }
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(v).expect("ViewState serialisierbar");
    walk(&mut value, roots);
    value["language"] = serde_json::Value::String("de".into());
    value
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_core_tools_views_for_the_ui() {
    let mut states: Vec<(String, ViewState)> = Vec::new();
    let mut roots: Vec<(String, &str)> = Vec::new();
    let mut add_roots = |fx: &Fixture| {
        let tmp = fx.tmp.path().to_path_buf();
        for p in [tmp.clone(), tmp.canonicalize().unwrap_or(tmp)] {
            roots.push((p.display().to_string(), "/Users/test"));
        }
    };

    // Ersteinrichtung ohne Übernahmeangebot (Neuinstallation)
    let fx = Fixture::new();
    add_roots(&fx);
    let core = start(&fx, None, |_| {});
    let v = wait_for(&core, "Ersteinrichtung", |v| v.first_run.as_ref().is_some_and(|f| f.preview_revision > 0)).await;
    states.push(("first_run_fresh".into(), v));
    drop(core);

    // Ersteinrichtung mit Übernahmeangebot, danach übernommen
    let fx = Fixture::new();
    add_roots(&fx);
    let legacy = fx.dir("home/.pdf-over");
    let out = fx.dir("home/Dokumente/Signiert");
    let emblem = fx.dir("home/Bilder").join("firmenlogo.png");
    std::fs::copy(testdata("config/legacy-home/.pdf-over/.emblem.png"), &emblem).unwrap();
    std::fs::write(
        legacy.join("PDF-Over.config"),
        format!(
            "BKU=MOBILE\nMOBILE_NUMBER=+436641234567\nOUTPUT_FOLDER={}\nSIGNATURE_PROFILE=SIGNATURBLOCK_SMALL\n\
             SIGNLOCALE=de\nEMBLEM={}\nKEYSTORE_ENABLED=true\nKEYSTORE_FILE={}\nPROXY_HOST=proxy.example.test\n\
             PROXY_PORT=3128\nUSE_PLACEHOLDER_SEARCH=true\nUSE_MARKER=true\nLOCALE=de\n",
            prop(&out),
            prop(&emblem),
            prop(&fx.dir("home/Dokumente").join("test-rsa2048.p12"))
        ),
    )
    .unwrap();
    let core = start(&fx, Some(&legacy), |_| {});
    let v = wait_for(&core, "Schritt 1", |v| {
        v.first_run
            .as_ref()
            .is_some_and(|f| f.preview_revision > 0 && f.migration.as_ref().is_some_and(|m| m.items.len() == 8))
    })
    .await;
    let preview = v.first_run.as_ref().unwrap().preview_revision;
    states.push(("first_run_migration".into(), v));
    core.dispatch(UiEvent::AcceptMigration { store_secrets: false });
    let v = wait_for(&core, "übernommen", |v| {
        v.first_run.as_ref().is_some_and(|f| f.migration.is_none() && f.preview_revision > preview)
    })
    .await;
    states.push(("first_run_imported".into(), v));
    core.dispatch(UiEvent::FinishFirstRun);
    let v = wait_for(&core, "fertig", |v| v.first_run.is_none()).await;
    states.push(("first_run_done".into(), v));
    drop(core);

    let fixture: serde_json::Map<String, serde_json::Value> =
        states.iter().map(|(name, v)| (name.clone(), fixture_state(v, &roots))).collect();
    if std::env::var_os("PIDA_BLESS").is_some() {
        let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
        std::fs::write(UI_FIXTURE, text).expect("Fixture schreiben");
        return;
    }
    let text = std::fs::read_to_string(UI_FIXTURE).expect("Fixture lesen (PIDA_BLESS=1 erzeugt sie)");
    let stored: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    for name in fixture.keys() {
        assert!(stored.contains_key(name), "real-core-tools-views.json ohne {name}: PIDA_BLESS=1 neu erzeugen");
    }
    for (name, value) in stored {
        let _: ViewState = serde_json::from_value(value)
            .unwrap_or_else(|e| panic!("{name}: real-core-tools-views.json passt nicht mehr zum Vertrag ({e})"));
    }
}
