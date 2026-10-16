//! Ende-zu-Ende-Abläufe des Kerns mit Fake-Engine (pades ist in dieser Welle noch Stub) und echten
//! bzw. Fake-Backends: Keystore (Modell A, echtes PKCS#12), Szenario-A-artiges Backend (Modell B,
//! Masken über den Broker), Ausgabe, Fehler, Einstellungen.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::*;
use pida_backend_api::{BackendId, NativeForm, SignatureBackend};
use pida_config::{Overlay, Settings};
use pida_core_api::{CoreOptions, FormAnswerView, InteractionView, StepView, UiEvent};
use pida_pades::{AcceptanceClass, FieldTarget, Reserve, Visibility};
use pida_workflow::{DocResult, ExitCode, PlaceholderAnswer};
use zeroize::Zeroizing;

const PDF: &str = "pdf/generated/simple-xref-table.pdf";

fn keystore_overlay(inputs: Vec<std::path::PathBuf>) -> Overlay {
    Overlay {
        input_files: inputs,
        keystore_file: Some(testdata("keystores/test-rsa2048.p12")),
        keystore_store_password: Some(Zeroizing::new("test".into())),
        backend: Some(pida_config::BackendChoice::Keystore),
        ..Overlay::default()
    }
}

fn options(engine: &Arc<FakeEngine>) -> CoreOptions {
    CoreOptions { engine: Some(engine.clone()), ..CoreOptions::isolated() }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn headless_keystore_batch_model_a() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let b = copy_pdf(PDF, &input, "b.pdf");
    let engine = FakeEngine::new();
    let opts = CoreOptions { interactive: Some(false), ..options(&engine) };
    let core = fx.start(keystore_overlay(vec![a.clone(), b.clone()]), true, opts);
    let code = tokio::time::timeout(std::time::Duration::from_secs(30), core.wait_finished()).await.unwrap();
    assert_eq!(code, ExitCode::Success, "{:#?}", core.view());
    let original = std::fs::read(&a).unwrap();
    for name in ["a_signed.pdf", "b_signed.pdf"] {
        let report = verify_fake_output(&input.join(name), &original);
        assert!(report.signer.subject_string().contains("NUR TEST"));
    }
    let results = core.results();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| matches!(r.result, DocResult::Signed { .. })));
    // Plan: Modell A, automatische Position (ohne PDFium: leere Seite), sichtbar auf der letzten Seite
    let plan = engine.last_plan();
    assert_eq!(plan.reserve, Reserve::ModelA);
    assert!(plan.profile_id.starts_with("SIGNATURBLOCK_SMALL_"));
    // /Reason aus dem Profil (`adobeSignReasonValue`), sonst „PAdES Signature“
    assert!(plan.reason.as_deref().is_some_and(|r| !r.is_empty()));
    assert!(plan.name.as_deref().is_some_and(|n| n.contains("NUR TEST")), "{:?}", plan.name);
    let Visibility::Visible(block) = &plan.visibility else { panic!("sichtbar erwartet") };
    assert_eq!(block.page_index, 0);
    assert!(!block.new_page);
    assert!(block.rect.width() > 100.0 && block.rect.height() > 20.0);
    assert_eq!(plan.field, FieldTarget::New { name: None });
    assert!(matches!(core.view().step, StepView::Finished { exit_code: 0 }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn headless_invisible_with_output_folder() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let mut overlay = keystore_overlay(vec![a.clone()]);
    overlay.invisible = Some(true);
    overlay.output = Some(out.clone());
    let core = fx.start(overlay, true, CoreOptions { interactive: Some(false), ..options(&engine) });
    assert_eq!(core.wait_finished().await, ExitCode::Success);
    assert!(out.join("a_signed.pdf").is_file());
    let plan = engine.last_plan();
    assert_eq!(plan.profile_id, "INVISIBLE");
    assert_eq!(plan.visibility, Visibility::Invisible);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn headless_existing_output_is_not_overwritten() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    std::fs::write(input.join("a_signed.pdf"), b"alt").unwrap();
    let engine = FakeEngine::new();
    let core = fx.start(keystore_overlay(vec![a]), true, CoreOptions { interactive: Some(false), ..options(&engine) });
    // ohne Antwort auf die Rückfrage wartet der Kern; die CLI beantwortet sie (hier: nein)
    let v = wait_for(
        &core,
        "Überschreib-Rückfrage",
        |v| matches!(&v.step, StepView::Output { result, .. } if result.ask_overwrite.is_some()),
    )
    .await;
    let StepView::Output { result, .. } = v.step else { unreachable!() };
    assert!(result.ask_overwrite.unwrap().ends_with("a_signed.pdf"));
    core.dispatch(UiEvent::ConfirmOverwrite { overwrite: false });
    assert_eq!(core.wait_finished().await, ExitCode::Cancelled);
    assert_eq!(std::fs::read(input.join("a_signed.pdf")).unwrap(), b"alt");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_keystore_password_headless_is_backend_error() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let mut overlay = keystore_overlay(vec![a]);
    overlay.keystore_store_password = Some(Zeroizing::new("falsch".into()));
    let core = fx.start(overlay, true, CoreOptions { interactive: Some(false), ..options(&engine) });
    assert_eq!(core.wait_finished().await, ExitCode::Backend);
    let r = core.results();
    assert!(matches!(&r[0].result, DocResult::Failed { error } if error.code() == "backend-wrong-password"));
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 0, "ohne Schlüssel kein prepare");
}

/// GUI-Ablauf mit Szenario-A-artigem Backend: Auswahl (kein Autostart, kein Zustimmungsdialog),
/// Anmeldung, TAN, Speichern-Dialog.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gui_model_b_with_forms_and_save_dialog() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let core = fx.start(Overlay::default(), false, CoreOptions { backends: Some(backends), ..options(&engine) });
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(core.view().main_bar.settings_enabled);
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), input.join("fehlt.pdf")] });
    // nur ID Austria verfügbar, aber ohne „Signieren“ (automatische Position): kein Autostart (F-54,
    // ADR 0003) – die Auswahl erscheint, die Option trägt `document_leaves_device` (Kurzhinweis der UI)
    let v =
        wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options } if !options.is_empty())).await;
    let StepView::Backend { options } = v.step else { unreachable!() };
    assert_eq!(options.len(), 1);
    assert!(options[0].capabilities.as_ref().is_some_and(|c| c.document_leaves_device));
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung ohne Nutzeraktion");
    // veraltetes Ereignis: ohne Wirkung, kein Fehler
    core.dispatch(UiEvent::AcknowledgePrivacy { accepted: true });
    core.dispatch(UiEvent::ChooseBackend { backend: BackendId::AtrustNative });
    // Anmeldung (ohne Zustimmungsschritt); der Signierbereich zeigt den Kurzhinweis
    let v = wait_for(&core, "Login", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
        )
    })
    .await;
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1);
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, document_leaves_device, .. } = v.step
    else {
        unreachable!()
    };
    assert!(document_leaves_device, "Kurzhinweis im Signierbereich");
    // „Signaturdaten anzeigen“ ohne Shell: Hinweis statt Absturz
    core.dispatch(UiEvent::OpenSignatureData { request });
    core.dispatch(UiEvent::SubmitForm {
        request,
        answer: FormAnswerView::Login { username: "0664 1234567".into(), password: "geheim1".into(), remember: true },
    });
    let v = wait_for(&core, "TAN", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::SmsTan { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
    // ohne Ausgabeordner: Speichern-Dialog (F-83)
    let v = wait_for(
        &core,
        "Speichern-Dialog",
        |v| matches!(&v.step, StepView::Output { result, .. } if result.save_dialog.is_some()),
    )
    .await;
    let StepView::Output { result, .. } = v.step else { unreachable!() };
    let dialog = result.save_dialog.unwrap();
    assert_eq!(dialog.directory, input);
    assert_eq!(dialog.file_name, "a_signed.pdf");
    let target = fx.dir("chosen").join("x.pdf");
    core.dispatch(UiEvent::Save { path: Some(target.clone()) });
    let v =
        wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
            .await;
    let StepView::Output { result, .. } = v.step else { unreachable!() };
    assert_eq!(result.path.as_deref(), Some(target.as_path()));
    verify_fake_output(&target, &std::fs::read(&a).unwrap());
    // Benutzername (normalisiert, „Merken“) gespeichert; keine Zustimmung (kein `[privacy]`)
    let text = std::fs::read_to_string(fx.paths.settings_file()).unwrap();
    assert!(!text.contains("privacy"), "{text}");
    let saved = Settings::from_toml(&text).unwrap();
    assert_eq!(saved.id_austria.username.as_deref(), Some("+436641234567"));
    assert!(saved.id_austria.remember);
    // Hinweis „Signaturdaten“ ohne Shell
    assert!(core.view().notices.iter().any(|n| n.code == "atrust-signature-data-link-rejected"));
    // Modell B: Reserve nach Backend-Modell, Plan mit Zertifikat → /Name
    let plan = engine.last_plan();
    assert_eq!(plan.reserve, Reserve::ModelB);
    assert!(plan.name.is_some());
    // F-91: dasselbe Dokument erneut signieren → Auswahl erzwungen (F-55)
    core.dispatch(UiEvent::Navigate { step: pida_workflow::Step::Signing });
    wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options } if !options.is_empty())).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_in_login_form_returns_to_backend_selection() {
    let fx = Fixture::new();
    // alte Einstellungen mit der Zustimmung des früheren Datenschutzdialogs laden weiter (ADR 0003)
    fx.write_settings("[signature]\nauto_position = true\n[privacy.acknowledged]\natrust_native = 1\n");
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let core = fx.start(
        Overlay { input_files: vec![a], ..Overlay::default() },
        false,
        CoreOptions { backends: Some(backends), ..options(&engine) },
    );
    // die alte Zustimmung startet nichts: Auswahl, erst die Wahl öffnet die Sitzung
    choose_id_austria(&core).await;
    wait_for(&core, "Login", |v| {
        matches!(&v.step, StepView::Signing { interaction: InteractionView::Form { .. }, .. })
    })
    .await;
    core.dispatch(UiEvent::Cancel);
    let v = wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options, .. } if !options.is_empty()))
        .await;
    let StepView::Backend { options } = v.step else { unreachable!() };
    assert!(options[0].available);
    assert_eq!(engine.prepares.load(Ordering::SeqCst), 0);
    // erneut wählen → neue Sitzung
    core.dispatch(UiEvent::ChooseBackend { backend: BackendId::AtrustNative });
    wait_for(&core, "Login 2", |v| {
        matches!(&v.step, StepView::Signing { interaction: InteractionView::Form { .. }, .. })
    })
    .await;
    assert_eq!(fake.opened.load(Ordering::SeqCst), 2);
}

/// CLI (ohne Oberfläche, mit Terminal, `-b MOBILE`): Der Aufruf ist die Nutzeraktion, ID Austria startet
/// ohne Rückfrage; statt des früheren Datenschutzhinweises ein einzeiliger Hinweis (ADR 0003).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn headless_interactive_id_austria_shows_a_one_line_notice() {
    let fx = Fixture::new();
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay =
        Overlay { input_files: vec![a], backend: Some(pida_config::BackendChoice::IdAustria), ..Overlay::default() };
    let opts = CoreOptions { interactive: Some(true), backends: Some(backends), ..options(&engine) };
    let core = fx.start(overlay, true, opts);
    let v = wait_for(&core, "Login", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
        )
    })
    .await;
    let notices: Vec<&str> = v.notices.iter().map(|n| n.code.as_str()).collect();
    assert_eq!(notices, vec![pida_workflow::MSG_DOCUMENT_LEAVES_DEVICE]);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1);
    core.dispatch(UiEvent::Quit);
    assert_eq!(core.wait_finished().await, ExitCode::Cancelled);
}

/// GUI-Start mit `-b MOBILE` (Overlay → Standard-Signiermittel, F-53) und automatischer Position: ID Austria
/// startet nicht von selbst – die Auswahl erscheint, erst die Wahl dort öffnet die Sitzung (Autostart-Befund
/// F-54, ADR 0003).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gui_start_with_b_mobile_waits_for_the_choice() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay =
        Overlay { input_files: vec![a], backend: Some(pida_config::BackendChoice::IdAustria), ..Overlay::default() };
    let core = fx.start(overlay, false, CoreOptions { backends: Some(backends), ..options(&engine) });
    wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options } if !options.is_empty())).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(matches!(core.view().step, StepView::Backend { .. }), "{:?}", core.view().step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung ohne Nutzeraktion");
    let cli_notice =
        |v: &pida_core_api::ViewState| v.notices.iter().any(|n| n.code == pida_workflow::MSG_DOCUMENT_LEAVES_DEVICE);
    assert!(!cli_notice(&core.view()), "die Oberfläche zeigt den Kurzhinweis selbst, nicht den CLI-Hinweis");
    choose_id_austria(&core).await;
    let v = wait_for(&core, "Login", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
        )
    })
    .await;
    assert!(matches!(v.step, StepView::Signing { document_leaves_device: true, .. }));
    assert!(!cli_notice(&v));
    assert_eq!(fake.opened.load(Ordering::SeqCst), 1);
    core.dispatch(UiEvent::Quit);
    assert_eq!(core.wait_finished().await, ExitCode::Cancelled);
}

/// Eigener Datenschutztext per Richtlinie erreicht die Oberfläche (Einstellungen › Datenschutz); ohne
/// Richtlinie gilt der Text aus PDF-Over.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn policy_privacy_text_reaches_the_view() {
    let fx = Fixture::new();
    let engine = FakeEngine::new();
    let policy =
        pida_config::Policy { privacy_notice_text: Some("Hinweis der Organisation".into()), ..Default::default() };
    let core = fx.start(Overlay::default(), false, CoreOptions { policy: Some(policy), ..options(&engine) });
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert_eq!(v.privacy_text.as_deref(), Some("Hinweis der Organisation"));
    let fx = Fixture::new();
    let core = fx.start(Overlay::default(), false, options(&engine));
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert_eq!(v.privacy_text, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cms_too_large_retry_with_bigger_reserve() {
    let fx = Fixture::new();
    fx.write_settings("[signature]\nauto_position = true\n");
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    engine.model_b_reserve.store(64, Ordering::SeqCst);
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay = Overlay { input_files: vec![a], output: Some(out.clone()), ..Overlay::default() };
    let core = fx.start(overlay, false, CoreOptions { backends: Some(backends), ..options(&engine) });
    // „Wiederholen“ öffnet die Sitzung neu – ohne erneute Auswahl
    choose_id_austria(&core).await;
    for round in 0..2 {
        let v = wait_for(&core, "Login", |v| {
            matches!(
                &v.step,
                StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
            )
        })
        .await;
        let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else {
            unreachable!()
        };
        core.dispatch(UiEvent::SubmitForm {
            request,
            answer: FormAnswerView::Login { username: "u".into(), password: "geheim1".into(), remember: false },
        });
        let v = wait_for(&core, "TAN", |v| {
            matches!(
                &v.step,
                StepView::Signing { interaction: InteractionView::Form { form: NativeForm::SmsTan { .. }, .. }, .. }
            )
        })
        .await;
        let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else {
            unreachable!()
        };
        core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
        if round == 0 {
            let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
            let StepView::Error { code, params, retry, .. } = v.step else { unreachable!() };
            assert_eq!(code, "ng-cms-too-large");
            assert!(retry);
            assert_eq!(params["reserve"], "64");
            core.dispatch(UiEvent::Retry);
        }
    }
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert!(out.join("a_signed.pdf").is_file());
    let plans = engine.plans.lock().unwrap().clone();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].reserve, Reserve::ModelB);
    assert!(matches!(plans[1].reserve, Reserve::Bytes(n) if n >= 16 * 1024));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn self_check_failure_writes_nothing() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    engine.fail_check.store(true, Ordering::SeqCst);
    let core = fx.start(keystore_overlay(vec![a]), false, options(&engine));
    // GUI-Modus ohne Positionierung? auto ist aus → Positionierung ohne PDFium nicht möglich → automatisch
    let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
    let StepView::Error { code, params, retry, .. } = v.step else { unreachable!() };
    assert_eq!(code, "ng-invariant-violated");
    assert_eq!(params["invariant"], "I1Prefix");
    assert!(!retry);
    assert!(!input.join("a_signed.pdf").exists());
    assert!(core.view().notices.iter().any(|n| n.code == "error-PositioningNotPossible"));
    core.dispatch(UiEvent::Cancel);
    wait_for(&core, "Auswahl", |v| matches!(v.step, StepView::Backend { .. })).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rejected_document_shows_error() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    *engine.class.lock().unwrap() = Some(AcceptanceClass::K5Rejected);
    let core = fx.start(Overlay { input_files: vec![a], ..Overlay::default() }, false, options(&engine));
    let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
    let StepView::Error { code, retry, .. } = v.step else { unreachable!() };
    assert_eq!(code, "ng-document-rejected");
    assert!(!retry);
    core.dispatch(UiEvent::Cancel);
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert_eq!(v.done.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn not_a_pdf_offers_retry() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let bad = input.join("kaputt.pdf");
    std::fs::write(&bad, b"%PDF-1.7 kaputt").unwrap();
    let engine = FakeEngine::new();
    let core = fx.start(Overlay { input_files: vec![bad.clone()], ..Overlay::default() }, false, options(&engine));
    let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
    let StepView::Error { code, retry, .. } = v.step else { unreachable!() };
    assert_eq!(code, "error-MayNotBeAPDF");
    assert!(retry);
    // Datei repariert → Wiederholen lädt neu
    std::fs::copy(testdata(PDF), &bad).unwrap();
    core.dispatch(UiEvent::Retry);
    wait_for(&core, "weiter", |v| !matches!(v.step, StepView::Error { .. } | StepView::Loading { .. })).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn placeholder_fields_question_per_signature() {
    let fx = Fixture::new();
    fx.write_settings("[placeholder]\nsearch = true\nkind = \"fields\"\n");
    let input = fx.dir("in");
    let a = copy_pdf("pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf", &input, "fields.pdf");
    let engine = FakeEngine::new();
    let mut overlay = keystore_overlay(vec![a]);
    overlay.backend = Some(pida_config::BackendChoice::Keystore);
    let core = fx.start(overlay, false, options(&engine));
    let v = wait_for(&core, "Platzhalterfrage", |v| matches!(v.step, StepView::Placeholder { .. })).await;
    let StepView::Placeholder { fields, qr, .. } = v.step else { unreachable!() };
    assert!(fields.len() > 1, "{fields:?}");
    assert!(qr.is_none());
    // Ja ohne Auswahl bei mehreren Feldern → Frage bleibt
    core.dispatch(UiEvent::PlaceholderAnswer { answer: PlaceholderAnswer::Yes, field_index: None });
    core.dispatch(UiEvent::PlaceholderAnswer { answer: PlaceholderAnswer::Yes, field_index: Some(1) });
    let v = wait_for(&core, "Ausgabe", |v| matches!(v.step, StepView::Output { .. })).await;
    assert!(matches!(v.step, StepView::Output { .. }));
    let plan = engine.last_plan();
    let FieldTarget::Existing { qualified_name, .. } = &plan.field else { panic!("{:?}", plan.field) };
    assert_eq!(qualified_name, &fields[1].name);
    assert!(matches!(plan.visibility, Visibility::Visible(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn settings_overlay_roundtrip_keeps_unknown_keys() {
    let fx = Fixture::new();
    fx.write_settings("zukunft = 1\n[signature]\nfuture_key = \"x\"\n");
    let engine = FakeEngine::new();
    let core = fx.start(Overlay::default(), false, options(&engine));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_for(&core, "Einstellungen", |v| v.settings.is_some()).await;
    assert!(!v.main_bar.settings_enabled, "Hauptleiste im Einstellungs-Overlay aus");
    // JSON-Rundlauf über die Bridge
    let json = serde_json::to_string(&v).unwrap();
    let back: pida_core_api::ViewState = serde_json::from_str(&json).unwrap();
    let mut settings = back.settings.unwrap().settings;
    assert_eq!(settings.unknown.len(), 2, "{:?}", settings.unknown);
    settings.output.postfix = "_sig".into();
    let ev: UiEvent =
        serde_json::from_str(&serde_json::to_string(&UiEvent::SaveSettings { settings: Box::new(settings) }).unwrap())
            .unwrap();
    core.dispatch(ev);
    wait_for(&core, "geschlossen", |v| v.settings.is_none()).await;
    let text = std::fs::read_to_string(fx.paths.settings_file()).unwrap();
    assert!(text.contains("zukunft = 1"), "{text}");
    assert!(text.contains("future_key = \"x\""), "{text}");
    assert!(text.contains("postfix = \"_sig\""), "{text}");
    // ungültiger Postfix wird abgelehnt
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_for(&core, "Einstellungen", |v| v.settings.is_some()).await;
    let mut s = v.settings.unwrap().settings;
    s.output.postfix = "a/b".into();
    core.dispatch(UiEvent::SaveSettings { settings: Box::new(s) });
    let v = wait_for(&core, "Problem", |v| v.settings.as_ref().is_some_and(|s| !s.problems.is_empty())).await;
    assert_eq!(v.settings.unwrap().problems["output.postfix"], "ng-settings-invalid-postfix");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unreadable_settings_offer_reset() {
    let fx = Fixture::new();
    fx.write_settings("schema_version = 99\n");
    let engine = FakeEngine::new();
    let core = fx.start(Overlay::default(), false, options(&engine));
    let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
    let StepView::Error { code, retry, params, .. } = v.step else { unreachable!() };
    assert_eq!(code, "error-Initialization");
    assert!(retry);
    assert!(params["detail"].contains("99"), "{params:?}");
    core.dispatch(UiEvent::Retry);
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(v.notices.iter().any(|n| n.code == "ng-config-reset-done"));
    let backup = fx
        .paths
        .config_dir
        .with_file_name(format!("{}.old", fx.paths.config_dir.file_name().unwrap().to_string_lossy()));
    assert!(backup.join("settings.toml").is_file());
}

/// Ohne Oberfläche: keine Rückfrage, aber der Grund erscheint als Hinweis (CLI gibt ihn aus), Exit 1.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unreadable_settings_headless_explain_and_end() {
    let fx = Fixture::new();
    fx.write_settings("schema_version = 99\n");
    let engine = FakeEngine::new();
    let core = fx.start(Overlay::default(), true, CoreOptions { interactive: Some(false), ..options(&engine) });
    assert_eq!(core.wait_finished().await, ExitCode::General);
    let v = core.view();
    let n = v.notices.iter().find(|n| n.code == "ng-config-error-unsupported-schema").expect("Hinweis");
    assert_eq!(n.params["found"], "99");
    // die Datei bleibt unverändert (kein Werksreset ohne Zustimmung)
    assert_eq!(std::fs::read_to_string(fx.paths.settings_file()).unwrap(), "schema_version = 99\n");
}

#[test]
fn missing_config_file_fails_start() {
    let fx = Fixture::new();
    let overlay = Overlay { config_file: Some("fehlt.config".into()), ..Overlay::default() };
    let err = pida_core_api::Core::new_with(fx.config(overlay, true), CoreOptions::isolated()).unwrap_err();
    assert_eq!(err.message_id(), "ng-config-file-missing");
}

#[test]
fn core_without_runtime_owns_one() {
    let fx = Fixture::new();
    let core = pida_core_api::Core::new_with(fx.config(Overlay::default(), false), CoreOptions::isolated()).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    });
    core.dispatch(UiEvent::Quit);
    let code = rt.block_on(core.wait_finished());
    assert_eq!(code, ExitCode::Success);
    drop(core);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ui_events_are_serde_tagged_and_secrets_redacted() {
    let e = UiEvent::SubmitSecret { request: 3, secret: "geheim".into(), password_storage: None };
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains("\"type\":\"submit_secret\""));
    assert!(!format!("{e:?}").contains("geheim"));
    let e = UiEvent::SubmitForm {
        request: 1,
        answer: FormAnswerView::Login { username: "u".into(), password: "pw123".into(), remember: false },
    };
    assert!(!format!("{e:?}").contains("pw123"));
    let parsed: UiEvent = serde_json::from_str(r#"{"type":"dismiss_notice","id":4}"#).unwrap();
    assert_eq!(parsed, UiEvent::DismissNotice { id: 4 });
}

/// Keystore im GUI-Modus: Passwortabfrage über den Broker (`InteractionView::Secret`), falsches
/// Passwort → erneute Abfrage mit `retry`, Ausgabeordner gesetzt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gui_keystore_password_prompt_with_retry() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let mut overlay = keystore_overlay(vec![a]);
    overlay.keystore_store_password = None;
    overlay.auto_position = true;
    overlay.output = Some(out.clone());
    let core = fx.start(overlay, false, options(&engine));
    let secret = |retry: bool| move |v: &pida_core_api::ViewState| matches!(&v.step, StepView::Signing { interaction: InteractionView::Secret { prompt, .. }, .. } if prompt.retry == retry);
    let v = wait_for(&core, "Passwort", secret(false)).await;
    let StepView::Signing { interaction: InteractionView::Secret { request, prompt, .. }, .. } = v.step else {
        unreachable!()
    };
    assert_eq!(prompt.subject, "test-rsa2048.p12", "nur Dateiname (Regel 9)");
    core.dispatch(UiEvent::SubmitSecret { request, secret: "falsch".into(), password_storage: None });
    let v = wait_for(&core, "Passwort erneut", secret(true)).await;
    let StepView::Signing { interaction: InteractionView::Secret { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitSecret { request, secret: "test".into(), password_storage: None });
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert!(out.join("a_signed.pdf").is_file());
    // Hauptleiste im Ergebnis-Screen (docs/analyse/01 §3.5)
    let bar = core.view().main_bar;
    assert!(bar.settings_enabled);
    assert!(bar.steps.iter().take(3).all(|s| s.active && s.enabled));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_password_prompt_returns_to_selection() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let mut overlay = keystore_overlay(vec![a]);
    overlay.keystore_store_password = None;
    overlay.auto_position = true;
    let core = fx.start(overlay, false, options(&engine));
    wait_for(&core, "Passwort", |v| {
        matches!(&v.step, StepView::Signing { interaction: InteractionView::Secret { .. }, .. })
    })
    .await;
    core.dispatch(UiEvent::Cancel);
    let v = wait_for(&core, "Auswahl", |v| matches!(&v.step, StepView::Backend { options, .. } if !options.is_empty()))
        .await;
    let StepView::Backend { options, .. } = v.step else { unreachable!() };
    assert!(options.iter().any(|o| o.id == BackendId::Keystore && o.available));
}

/// Import-Assistent (Plan §7.2): Angebot nur ohne `settings.toml`, Übernahme bzw. Ablehnung.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn migration_offer_accept_and_decline() {
    for accept in [true, false] {
        let fx = Fixture::new();
        let legacy = fx.dir("legacy/.pdf-over");
        std::fs::copy(testdata("config/pdf-over-generated-de.config"), legacy.join("PDF-Over.config")).unwrap();
        let engine = FakeEngine::new();
        let opts = CoreOptions { legacy_dir: Some(legacy.clone()), ..options(&engine) };
        let core = fx.start(Overlay::default(), false, opts);
        let v = wait_for(&core, "Angebot", |v| v.migration.is_some()).await;
        assert_eq!(v.migration.unwrap().legacy_file, legacy.join("PDF-Over.config"));
        core.dispatch(if accept {
            UiEvent::AcceptMigration { store_secrets: false }
        } else {
            UiEvent::DeclineMigration
        });
        let v = wait_for(&core, "erledigt", |v| v.migration.is_none()).await;
        let saved = Settings::from_toml(&std::fs::read_to_string(fx.paths.settings_file()).unwrap()).unwrap();
        if accept {
            assert!(v.notices.iter().any(|n| n.code == "ng-migrate-done"), "{:?}", v.notices);
            assert_eq!(saved.id_austria.username.as_deref(), Some("+436641234567"));
            // übernommene Speicherart (`KEYSTORE_PASS_STORE_TYPE` bzw. Auto-Migration) gilt als gewählt
            assert!(saved.keystore.storage_chosen, "Übernahme fragt nie „Passwort merken?“");
        } else {
            // Neuinstallation: die erste Keystore-Passworteingabe fragt „Passwort merken?“
            let mut expected = Settings::default();
            expected.keystore.storage_chosen = false;
            assert_eq!(saved, expected);
        }
        drop(core);
        // zweiter Start: kein Angebot mehr
        let core = fx.start(Overlay::default(), false, CoreOptions { legacy_dir: Some(legacy), ..options(&engine) });
        let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
        assert!(v.migration.is_none());
    }
}

/// Dateien, die nicht existieren, werden ignoriert (F-12); Navigation „Öffnen“ verwirft (F-16).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn open_files_ignores_missing_and_navigation_discards() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let b = copy_pdf(PDF, &input, "b.pdf");
    let engine = FakeEngine::new();
    let core = fx.start(Overlay::default(), false, options(&engine));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    core.dispatch(UiEvent::OpenFiles { paths: vec![input.join("fehlt.pdf")] });
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone(), b.clone()] });
    // ohne PDFium: keine Vorschau → automatische Position mit Hinweis → Auswahl
    let v = wait_for(&core, "Auswahl", |v| matches!(v.step, StepView::Backend { .. })).await;
    assert!(v.main_bar.steps[1].enabled, "Positionierung klickbar");
    core.dispatch(UiEvent::Navigate { step: pida_workflow::Step::Open });
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert_eq!(v.done.len(), 2);
    assert!(v.done.iter().all(|d| d.result == DocResult::Cancelled));
    // gespeicherter letzter Ordner (LAST_SOURCE_DIRECTORY)
    let saved = Settings::from_toml(&std::fs::read_to_string(fx.paths.settings_file()).unwrap()).unwrap();
    assert_eq!(saved.general.last_source_directory.as_deref(), Some(input.as_path()));
}

/// Ein spätes Analyse-Ergebnis eines bereits verworfenen Dokuments bleibt nicht im Speicher.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn discarded_document_is_not_kept_after_late_analysis() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    engine.analyze_delay_ms.store(400, Ordering::SeqCst);
    let core = fx.start(Overlay { input_files: vec![a], ..Overlay::default() }, false, options(&engine));
    wait_for(&core, "Laden", |v| matches!(v.step, StepView::Loading { .. })).await;
    // Hauptleiste „Öffnen“, während die Analyse noch läuft
    core.dispatch(UiEvent::Navigate { step: pida_workflow::Step::Open });
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    assert_eq!(core.open_document_count(), 0, "verworfenes Dokument im Speicher");
    assert!(matches!(core.view().step, StepView::Open { .. }), "{:?}", core.view().step);
    // ein neu geöffnetes Dokument wird normal geladen
    engine.analyze_delay_ms.store(0, Ordering::SeqCst);
    let b = copy_pdf(PDF, &input, "b.pdf");
    core.dispatch(UiEvent::OpenFiles { paths: vec![b] });
    wait_for(&core, "nächster Schritt", |v| matches!(v.step, StepView::Backend { .. } | StepView::Position { .. }))
        .await;
    assert_eq!(core.open_document_count(), 1);
}

/// Programmfehler im Kern-Task (F-106): mit Oberfläche Fehleranzeige statt hängender UI, danach weiter
/// mit der Dokumentauswahl; ohne Oberfläche Ende mit Exit-Code 1.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn panic_in_core_task_becomes_unexpected_error() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let opts = || CoreOptions {
        backends: Some(vec![Arc::new(PanickingBackend) as Arc<dyn SignatureBackend>]),
        ..options(&engine)
    };
    let overlay = Overlay { input_files: vec![a.clone()], auto_position: true, ..Overlay::default() };
    let core = fx.start(overlay.clone(), false, opts());
    let v = wait_for(&core, "Fehler", |v| matches!(v.step, StepView::Error { .. })).await;
    let StepView::Error { code, retry, .. } = v.step else { unreachable!() };
    assert_eq!(code, "error-Unexpected");
    assert!(!retry);
    assert_eq!(core.open_document_count(), 0);
    core.dispatch(UiEvent::Cancel);
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(matches!(&v.done[0].result, DocResult::Failed { error } if error.code() == "error-Unexpected"));
    drop(core);
    // ohne Oberfläche
    let core = fx.start(overlay, true, CoreOptions { interactive: Some(false), ..opts() });
    let code = tokio::time::timeout(std::time::Duration::from_secs(20), core.wait_finished()).await.unwrap();
    assert_eq!(code, ExitCode::General);
    assert!(matches!(&core.results()[0].result, DocResult::Failed { error } if error.code() == "error-Unexpected"));
}

/// Einstellungen werden nur aus der geöffneten Einstellungsansicht übernommen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn save_settings_requires_the_settings_overlay() {
    let fx = Fixture::new();
    let engine = FakeEngine::new();
    let core = fx.start(Overlay::default(), false, options(&engine));
    wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    let mut s = Settings::default();
    s.output.postfix = "_ohne_ansicht".into();
    core.dispatch(UiEvent::SaveSettings { settings: Box::new(s.clone()) });
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_for(&core, "Einstellungen", |v| v.settings.is_some()).await;
    assert_eq!(v.settings.unwrap().settings.output.postfix, "_signed", "ohne Ansicht nicht übernommen");
    s.output.postfix = "_mit_ansicht".into();
    core.dispatch(UiEvent::SaveSettings { settings: Box::new(s) });
    wait_for(&core, "geschlossen", |v| v.settings.is_none()).await;
    let saved = Settings::from_toml(&std::fs::read_to_string(fx.paths.settings_file()).unwrap()).unwrap();
    assert_eq!(saved.output.postfix, "_mit_ansicht");
}

fn openssl() -> bool {
    std::process::Command::new("openssl").arg("version").output().is_ok_and(|o| o.status.success())
}

/// Externes Orakel (openssl cms) für das CMS aus Modell A (Keystore): Signaturwert, message-digest über
/// den Inhalt und die signierten Attribute. Immer PDF-AS-Verfahren (mit signing-time), auch wenn eine
/// alte Einstellungsdatei noch `pades_bb` enthält.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn model_a_cms_passes_openssl() {
    if !openssl() {
        eprintln!("openssl fehlt – Orakel übersprungen");
        return;
    }
    for (profile, signing_time) in [("pdfas_compat", true), ("pades_bb", true)] {
        let fx = Fixture::new();
        fx.write_settings(&format!("[signature]\ncms_profile = \"{profile}\"\n"));
        let input = fx.dir("in");
        let a = copy_pdf(PDF, &input, "a.pdf");
        let engine = FakeEngine::new();
        let core = fx.start(
            keystore_overlay(vec![a.clone()]),
            true,
            CoreOptions { interactive: Some(false), ..options(&engine) },
        );
        assert_eq!(core.wait_finished().await, ExitCode::Success, "{:#?}", core.view());
        let original = std::fs::read(&a).unwrap();
        let signed = std::fs::read(input.join("a_signed.pdf")).unwrap();
        let dir = fx.dir("oracle");
        let (content, cms) = (dir.join("content.bin"), dir.join("sig.der"));
        std::fs::write(&content, &original).unwrap();
        std::fs::write(&cms, &signed[original.len()..]).unwrap();
        let verify = std::process::Command::new("openssl")
            .args(["cms", "-verify", "-binary", "-inform", "DER", "-noverify", "-purpose", "any", "-in"])
            .arg(&cms)
            .arg("-content")
            .arg(&content)
            .arg("-out")
            .arg(dir.join("verify-out.bin")) // kein /dev/null: Windows-CI
            .output()
            .unwrap();
        assert!(verify.status.success(), "{profile}: {}", String::from_utf8_lossy(&verify.stderr));
        let print = std::process::Command::new("openssl")
            .args(["cms", "-cmsout", "-print", "-inform", "DER", "-in"])
            .arg(&cms)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&print.stdout);
        assert!(text.contains("messageDigest"), "{profile}: {text}");
        assert!(text.contains("contentType"), "{profile}");
        assert!(
            text.contains("signingCertificateV2") || text.contains("1.2.840.113549.1.9.16.2.47"),
            "{profile}: {text}"
        );
        assert_eq!(text.contains("signingTime"), signing_time, "{profile}: signingTime");
        // genau ein SignerInfo (I3)
        assert_eq!(text.matches("signatureAlgorithm:").count(), 1, "{profile}");
    }
}

/// Plan §6.1: Warnungen der Analyse (`/Perms /UR3` …) mit Abbruchmöglichkeit; ohne Oberfläche Hinweis.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn analysis_warning_asks_before_signing() {
    let fx = Fixture::new();
    let input = fx.dir("in");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    engine.findings.lock().unwrap().push(pida_pades::Finding {
        code: "pdf-usage-rights".into(),
        class: AcceptanceClass::K1Regular,
        warning: true,
    });
    let core = fx.start(Overlay { input_files: vec![a.clone()], ..Overlay::default() }, false, options(&engine));
    let v = wait_for(&core, "Warnung", |v| matches!(v.step, StepView::Warning { .. })).await;
    let StepView::Warning { codes, doc } = v.step else { unreachable!() };
    assert_eq!(codes, vec!["pdf-usage-rights".to_owned()]);
    assert_eq!(doc.name, "a.pdf");
    assert!(v.notices.is_empty(), "mit Oberfläche keine doppelte Meldung");
    // Abbrechen verwirft das Dokument
    core.dispatch(UiEvent::AnswerWarning { proceed: false });
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert_eq!(v.done[0].result, DocResult::Cancelled);
    assert_eq!(core.open_document_count(), 0);
    // Fortfahren → weiter im Ablauf
    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
    wait_for(&core, "Warnung", |v| matches!(v.step, StepView::Warning { .. })).await;
    core.dispatch(UiEvent::AnswerWarning { proceed: true });
    wait_for(&core, "weiter", |v| matches!(v.step, StepView::Backend { .. } | StepView::Position { .. })).await;
    drop(core);
    // ohne Oberfläche: Hinweis, Signatur läuft durch
    let core = fx.start(keystore_overlay(vec![a]), true, CoreOptions { interactive: Some(false), ..options(&engine) });
    assert_eq!(core.wait_finished().await, ExitCode::Success);
    assert!(core.view().notices.iter().any(|n| n.code == "pdf-usage-rights"));
}

// ------------------------------------------------------------------------------------------------
// Zertifikats-Cache (Opt-in, Plan §4.2): ab der zweiten Signatur kein InfoboxRead; Abweichung →
// Ergebnis verwerfen, Eintrag löschen, „Wiederholen“ liest neu
// ------------------------------------------------------------------------------------------------

const CACHE_SETTINGS: &str = "[signature]\nauto_position = true\n\
[id_austria]\nusername = \"+436641234567\"\n[certificate_cache]\nenabled = true\n";

fn cache_key() -> pida_config::CertCacheKey {
    pida_config::CertCacheKey::new("atrust_native", "+436641234567")
}

async fn answer_tan(core: &pida_core_api::Core) {
    let v = wait_for(core, "TAN", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::SmsTan { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitForm { request, answer: FormAnswerView::Tan { tan: "123456".into() } });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificate_cache_skips_the_certificate_request() {
    let fx = Fixture::new();
    fx.write_settings(CACHE_SETTINGS);
    let cache = pida_config::CertificateCache::with_dir(fx.paths.certificate_cache_dir(), true);
    cache.put(&cache_key(), &std::fs::read(testdata("keystores/test-rsa2048.crt.der")).unwrap()).unwrap();
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay = Overlay { input_files: vec![a.clone()], output: Some(out.clone()), ..Overlay::default() };
    let core = fx.start(overlay, false, CoreOptions { backends: Some(backends), ..options(&engine) });
    choose_id_austria(&core).await;
    // ohne InfoboxRead (Anmeldung) direkt zur Signatur
    answer_tan(&core).await;
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert_eq!(fake.certificates.load(Ordering::SeqCst), 0, "Zertifikat kam aus dem Cache");
    verify_fake_output(&out.join("a_signed.pdf"), &std::fs::read(&a).unwrap());
    // Block mit dem Zertifikat aus dem Cache (/Name)
    assert!(engine.last_plan().name.as_deref().is_some_and(|n| n.contains("NUR TEST")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificate_cache_mismatch_discards_result_and_rereads() {
    let fx = Fixture::new();
    fx.write_settings(CACHE_SETTINGS);
    let cache = pida_config::CertificateCache::with_dir(fx.paths.certificate_cache_dir(), true);
    // im Cache ein anderes Zertifikat (z. B. vor der Erneuerung)
    let other = std::fs::read(testdata("keystores/test-ecp256.crt.der")).unwrap();
    cache.put(&cache_key(), &other).unwrap();
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay = Overlay { input_files: vec![a.clone()], output: Some(out.clone()), ..Overlay::default() };
    let core = fx.start(overlay, false, CoreOptions { backends: Some(backends), ..options(&engine) });
    choose_id_austria(&core).await;
    answer_tan(&core).await;
    // Ergebnis verworfen, nichts gespeichert; Begründung mit „Wiederholen“
    let v = wait_for(&core, "Fehler", |v| matches!(&v.step, StepView::Error { .. })).await;
    let StepView::Error { code, retry, .. } = v.step else { unreachable!() };
    assert_eq!(code, "ng-certcache-mismatch");
    assert!(retry);
    assert!(!out.join("a_signed.pdf").exists());
    assert_eq!(cache.get(&cache_key()).unwrap(), None, "Eintrag gelöscht");
    assert_eq!(fake.certificates.load(Ordering::SeqCst), 0);
    // Wiederholen: Zertifikat neu lesen (Anmeldung), Block neu bauen, zweite Bestätigung
    core.dispatch(UiEvent::Retry);
    let v = wait_for(&core, "Login", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitForm {
        request,
        answer: FormAnswerView::Login { username: "+436641234567".into(), password: "geheim1".into(), remember: true },
    });
    answer_tan(&core).await;
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert_eq!(fake.certificates.load(Ordering::SeqCst), 1);
    verify_fake_output(&out.join("a_signed.pdf"), &std::fs::read(&a).unwrap());
    // jetzt steht das bestätigte Zertifikat im Cache
    let cached = cache.get(&cache_key()).unwrap().expect("Zertifikat im Cache");
    assert_eq!(cached, std::fs::read(testdata("keystores/test-rsa2048.crt.der")).unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificate_cache_without_opt_in_stores_nothing() {
    let fx = Fixture::new();
    fx.write_settings(&CACHE_SETTINGS.replace("enabled = true", "enabled = false"));
    let input = fx.dir("in");
    let out = fx.dir("out");
    let a = copy_pdf(PDF, &input, "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let overlay = Overlay { input_files: vec![a], output: Some(out.clone()), ..Overlay::default() };
    let core = fx.start(overlay, false, CoreOptions { backends: Some(backends), ..options(&engine) });
    choose_id_austria(&core).await;
    let v = wait_for(&core, "Login", |v| {
        matches!(
            &v.step,
            StepView::Signing { interaction: InteractionView::Form { form: NativeForm::Login { .. }, .. }, .. }
        )
    })
    .await;
    let StepView::Signing { interaction: InteractionView::Form { request, .. }, .. } = v.step else { unreachable!() };
    core.dispatch(UiEvent::SubmitForm {
        request,
        answer: FormAnswerView::Login { username: "+436641234567".into(), password: "geheim1".into(), remember: false },
    });
    answer_tan(&core).await;
    wait_for(&core, "gespeichert", |v| matches!(&v.step, StepView::Output { result, .. } if result.path.is_some()))
        .await;
    assert!(!fx.paths.certificate_cache_dir().exists(), "ohne Opt-in kein Cache-Ordner");
}

/// ADR 0004: Im **klassischen** Ablauf (ohne `workspace_ui`) bleiben die Ereignisse der neuen Oberfläche
/// ohne Wirkung, ebenso die Ereignisse späterer Etappen (Stapel K2, Prüfmodus/Ersteinrichtung K4) – der Kern
/// bleibt im Zustand, öffnet **keine** Sitzung (A1, auch nicht mit `sign_with`) und liefert die neuen Felder
/// mit Standardwerten; die Freigaben (`actions`) folgen dem Zustand. In beiden Modi wirken `set_ui_language`
/// (Kopfzeile, K1) und – K3 – „Zuletzt signiert“ (leere Liste) sowie `apply_settings`, Letzteres nur mit
/// geöffneter Einstellungsansicht (hier nicht: ohne Wirkung).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn classic_mode_ignores_workspace_and_later_stage_events() {
    use pida_core_api::{PlacementTarget, SignersView};
    let fx = Fixture::new();
    let a = copy_pdf(PDF, &fx.dir("in"), "a.pdf");
    let engine = FakeEngine::new();
    let fake = FakeAtrust::new();
    let backends: Vec<Arc<dyn SignatureBackend>> = vec![fake.backend()];
    let core = fx.start(Overlay::default(), false, CoreOptions { backends: Some(backends), ..options(&engine) });
    let v = wait_for(&core, "Öffnen", |v| matches!(v.step, StepView::Open { .. })).await;
    assert!(v.actions.open_files && v.actions.home && v.actions.settings);
    assert!(!v.actions.sign && !v.actions.edit_placement && !v.actions.verify && !v.actions.start_batch);
    assert_eq!((v.document.as_ref(), v.batch.as_ref(), v.verify.as_ref()), (None, None, None));
    assert_eq!((v.recent.as_ref().map(Vec::len), v.first_run.as_ref()), (Some(0), None), "K3: Liste in beiden Modi");
    assert_eq!(v.signers, SignersView::default());
    let language = v.language;

    core.dispatch(UiEvent::OpenFiles { paths: vec![a.clone()] });
    // ohne PDFium (isolierter Kern) automatische Position → Signiermittel-Auswahl (klassischer Ablauf)
    let v = wait_for(&core, "Positionieren/Auswahl", |v| {
        matches!(&v.step, StepView::Position { .. })
            || matches!(&v.step, StepView::Backend { options } if !options.is_empty())
    })
    .await;
    let positioning = matches!(v.step, StepView::Position { .. });
    assert_eq!(v.actions.sign, positioning);
    assert!(v.actions.open_files && !v.actions.verify);
    let events = vec![
        UiEvent::SetInvisible { invisible: true },
        UiEvent::UseTarget { target: PlacementTarget::Qr },
        UiEvent::InspectPages { pages: vec![0] },
        UiEvent::SelectSigner { backend: Some(BackendId::AtrustNative), make_default: true },
        UiEvent::SignWith { backend: BackendId::AtrustNative },
        UiEvent::ShowBatch,
        UiEvent::OpenBatchItem { doc: pida_workflow::DocId(1) },
        UiEvent::BackToBatch,
        UiEvent::RemoveFromBatch { doc: pida_workflow::DocId(1) },
        UiEvent::StartBatch {
            backend: BackendId::AtrustNative,
            docs: vec![pida_workflow::DocId(1)],
            output_dir: None,
            on_existing: pida_workflow::ExistingPolicy::KeepBoth,
        },
        UiEvent::SkipBatchDocument,
        UiEvent::AbortBatch,
        UiEvent::OpenVerify { path: a },
        UiEvent::VerifyRecent { id: 1 },
        UiEvent::CloseVerify,
        UiEvent::SignVerified,
        UiEvent::RemoveRecent { id: 1 },
        UiEvent::ClearRecent,
        UiEvent::ApplySettings {
            changes: [("signature.profile".to_owned(), serde_json::json!("AMTSSIGNATURBLOCK"))].into(),
            revision: 1,
        },
        UiEvent::UpdateFirstRun {
            default_backend: Some(pida_config::BackendChoice::IdAustria),
            profile: pida_config::SignatureProfile::Amtssignaturblock,
            block_language: None,
        },
        UiEvent::FinishFirstRun,
        UiEvent::OfferMigration,
    ];
    for e in events {
        core.dispatch(e);
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let after = core.view();
    assert_eq!(std::mem::discriminant(&after.step), std::mem::discriminant(&v.step), "{:?}", after.step);
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0, "keine Sitzung ohne umgesetztes Ereignis (A1)");
    assert_eq!(after.language, language);
    assert!(after.settings.is_none() && after.document.is_none() && after.batch.is_none());
    assert!(after.verify.is_none() && after.first_run.is_none());
    assert_eq!(after.recent, Some(Vec::new()));
    let saved = std::fs::read_to_string(fx.paths.settings_file()).unwrap_or_default();
    assert!(
        !saved.contains("AMTSSIGNATURBLOCK"),
        "apply_settings ohne geöffnete Ansicht und update_first_run (K4) speichern nichts"
    );
    // Oberflächensprache aus der Kopfzeile: sofort wirksam und gespeichert
    let other = if language == pida_i18n::Lang::En { pida_i18n::Lang::De } else { pida_i18n::Lang::En };
    core.dispatch(UiEvent::SetUiLanguage { language: Some(other) });
    wait_for(&core, "Sprache", |v| v.language == other).await;
    let saved = std::fs::read_to_string(fx.paths.settings_file()).unwrap_or_default();
    assert!(saved.contains("ui_language"), "{saved}");
    core.dispatch(UiEvent::Quit);
    assert_eq!(core.wait_finished().await, ExitCode::Cancelled);
}
