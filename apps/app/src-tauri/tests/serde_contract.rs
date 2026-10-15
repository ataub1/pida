//! Serde-Vertrag zwischen `pdfover-core-api` (Rust) und der Oberfläche (TypeScript).
//!
//! 1. **Rust → TS:** Beispiel-`ViewState`s für alle Varianten (Schritte, Interaktionen, Masken, Meldungen,
//!    Fortschritt, Formularprobleme, Einstellungen mit unbekannten Einträgen) werden serialisiert und mit
//!    `apps/app/src/core/fixtures/view-states.json` verglichen. Die Oberfläche prüft dieselbe Datei gegen
//!    ihre Typen (`src/core/contract.test.ts`). Neu schreiben: `PDFOVER_BLESS=1 cargo test -p pdfover-app
//!    --test serde_contract`.
//! 2. **TS → Rust:** Die in TypeScript typisierten Beispiel-Ereignisse
//!    (`src/core/fixtures/ui-events.ts` → `ui-events.json`) müssen sich als [`UiEvent`] lesen lassen und
//!    unverändert zurückschreiben.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pdfover_backend_api::{
    BackendId, Capabilities, CertNeed, DigestAlg, FactorSet, FormContext, FormProblem, Interaction, NativeForm, Notice,
    NoticeButtons, NoticeCode, NoticeKind, Progress, QrImage, Qualification, SecretKind, SecretPrompt, ServerNeed,
    SignInput, SignOutput,
};
use pdfover_config::{NoteSetting, Settings};
use pdfover_core_api::{
    BackendOptionView, BlockView, DocInfo, DoneItemView, FieldInfo, InteractionView, MainBarView, MigrationOfferView,
    NoticeView, OutputView, PrivacyNoticeView, QrInfo, QueueItemView, SaveDialogView, SettingsView, StepView, UiEvent,
    ViewState,
};
use pdfover_i18n::Lang;
use pdfover_placement::Placement;
use pdfover_pades::{AcceptanceClass, OutputInfo};
use pdfover_workflow::{DocId, DocResult, MainBarStep, Step, WorkflowError};

const VIEW_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/core/fixtures/view-states.json");
const EVENT_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/core/fixtures/ui-events.json");

fn bar(cells: [&str; 4], settings_enabled: bool) -> MainBarView {
    let steps = [Step::Open, Step::Positioning, Step::Signing, Step::Finish]
        .into_iter()
        .zip(cells)
        .map(|(step, c)| MainBarStep { step, active: c.contains('A'), enabled: c.contains('E') })
        .collect();
    MainBarView { steps, settings_enabled }
}

fn view(step: StepView, bar: MainBarView) -> ViewState {
    ViewState {
        step,
        main_bar: bar,
        settings: None,
        language: Lang::De,
        notices: Vec::new(),
        migration: None,
        done: Vec::new(),
    }
}

fn doc() -> DocInfo {
    DocInfo {
        id: DocId(7),
        name: "Vertrag.pdf".into(),
        page_count: 2,
        page_sizes: vec![(595.5, 842.0), (842.0, 595.5)],
        signed: false,
    }
}

fn queue() -> Vec<QueueItemView> {
    vec![
        QueueItemView { id: DocId(7), name: "Vertrag.pdf".into(), current: true },
        QueueItemView { id: DocId(8), name: "Angebot.pdf".into(), current: false },
    ]
}

fn ctx(problem: Option<FormProblem>) -> FormContext {
    FormContext {
        signature_data_url: Some("https://www.a-trust.at/mobile/ShowSigobj.aspx?sid=x".into()),
        can_switch_to_sms: true,
        can_switch_to_fido2: false,
        fido2_visible: true,
        problem,
    }
}

fn caps_native() -> Capabilities {
    Capabilities {
        input: SignInput::Document,
        output: SignOutput::Cms,
        certificate: CertNeed::OnlyForVisibleBlock,
        interaction: Interaction::NativeForms,
        server: ServerNeed::None,
        max_batch: 1,
        login_once_per_batch: true,
        document_leaves_device: true,
        qualification: Qualification::Qes,
        factors: FactorSet { app: true, sms: true, qr: true, fido2: true, card: false },
        cms_reserve: Some(16384),
    }
}

fn caps_keystore() -> Capabilities {
    Capabilities {
        input: SignInput::Digest { algs: vec![DigestAlg::Sha256, DigestAlg::Sha512] },
        output: SignOutput::RawSignature,
        certificate: CertNeed::Required,
        interaction: Interaction::Secret,
        server: ServerNeed::None,
        max_batch: 1,
        login_once_per_batch: false,
        document_leaves_device: false,
        qualification: Qualification::Unknown,
        factors: FactorSet::default(),
        cms_reserve: None,
    }
}

fn signing(interaction: InteractionView) -> ViewState {
    view(StepView::Signing { interaction, queue: queue() }, bar(["A", "A", "A", ""], false))
}

fn form(request: u64, form: NativeForm) -> ViewState {
    signing(InteractionView::Form { request, form })
}

fn notice(request: u64, kind: NoticeKind, code: NoticeCode, text: Option<&str>, buttons: NoticeButtons) -> ViewState {
    signing(InteractionView::Notice { request, notice: Notice { kind, code, text: text.map(str::to_owned), buttons } })
}

/// Alle Beispielzustände (Name → ViewState), Reihenfolge stabil.
fn samples() -> Vec<(&'static str, ViewState)> {
    let mut out: Vec<(&'static str, ViewState)> = vec![
        ("starting", view(StepView::Starting, bar(["", "", "", ""], false))),
        ("open_empty", view(StepView::Open { queue: vec![] }, bar(["AE", "", "", ""], true))),
        ("open_queue", view(StepView::Open { queue: queue() }, bar(["AE", "", "", ""], true))),
        ("loading", view(StepView::Loading { queue: queue() }, bar(["AE", "", "", ""], true))),
        (
            "warning",
            view(
                StepView::Warning { doc: doc(), codes: vec!["pdf-usage-rights".into(), "pdf-xfa-hybrid".into()] },
                bar(["AE", "", "", ""], true),
            ),
        ),
    ];
    // Hinweise ohne Rückfrage, Import-Angebot und abgeschlossene Dokumente (Querschnitt)
    let mut open = view(StepView::Open { queue: vec![] }, bar(["AE", "", "", ""], true));
    let mut p = BTreeMap::new();
    p.insert("path".to_owned(), "/Users/demo/settings.toml.old".to_owned());
    open.notices = vec![
        NoticeView { id: 1, code: "error-PositioningNotPossible".into(), params: BTreeMap::new() },
        NoticeView { id: 2, code: "ng-config-reset-done".into(), params: p },
    ];
    open.migration = Some(MigrationOfferView {
        legacy_file: PathBuf::from("/Users/demo/.pdf-over/PDF-Over.config"),
        plaintext_secrets: true,
    });
    open.done = vec![
        DoneItemView {
            id: DocId(1),
            name: "a.pdf".into(),
            input: PathBuf::from("/Users/demo/a.pdf"),
            result: DocResult::Signed {
                output: OutputInfo { path: PathBuf::from("/Users/demo/a_signed.pdf"), len: 1234, sha256: "ab".into() },
            },
        },
        DoneItemView {
            id: DocId(2),
            name: "b.pdf".into(),
            input: PathBuf::from("/Users/demo/b.pdf"),
            result: DocResult::Cancelled,
        },
        DoneItemView {
            id: DocId(3),
            name: "c.pdf".into(),
            input: PathBuf::from("/Users/demo/c.pdf"),
            result: DocResult::Failed {
                error: WorkflowError::Rejected { class: AcceptanceClass::K5Rejected, code: "ng-document-rejected".into() },
            },
        },
    ];
    out.push(("open_notices_migration_done", open));
    out.push((
        "placeholder_fields",
        view(
            StepView::Placeholder {
                doc: doc(),
                fields: vec![
                    FieldInfo { name: "Unterschrift 1".into(), page: Some(1) },
                    FieldInfo { name: "Feld.ohne.Seite".into(), page: None },
                ],
                qr: None,
            },
            bar(["AE", "", "", ""], true),
        ),
    ));
    out.push((
        "placeholder_qr",
        view(
            StepView::Placeholder { doc: doc(), fields: vec![], qr: Some(QrInfo { page: 1, id: Some("1".into()) }) },
            bar(["AE", "", "", ""], true),
        ),
    ));
    let mut announce = BTreeMap::new();
    announce.insert("page".to_owned(), "2".to_owned());
    announce.insert("pages".to_owned(), "2".to_owned());
    announce.insert("left".to_owned(), "105".to_owned());
    announce.insert("bottom".to_owned(), "74".to_owned());
    out.push((
        "position",
        view(
            StepView::Position {
                doc: doc(),
                page: 1,
                block: BlockView { width: 232.5, height: 82.0, preview_revision: 3 },
                placement: Placement { page_index: 1, x: 297.5, y: 210.5, width: 232.5, height: 82.0 },
                new_page: false,
                announce,
            },
            bar(["AE", "A", "", ""], true),
        ),
    ));
    out.push((
        "position_new_page",
        view(
            StepView::Position {
                doc: DocInfo { signed: true, ..doc() },
                page: 2,
                block: BlockView { width: 100.0, height: 40.0, preview_revision: 1 },
                placement: Placement { page_index: 2, x: 0.0, y: 595.5, width: 100.0, height: 40.0 },
                new_page: true,
                announce: BTreeMap::new(),
            },
            bar(["AE", "A", "", ""], true),
        ),
    ));
    out.push((
        "backend",
        view(
            StepView::Backend {
                options: vec![
                    BackendOptionView {
                        id: BackendId::AtrustNative,
                        available: true,
                        capabilities: Some(caps_native()),
                    },
                    BackendOptionView { id: BackendId::LocalBku, available: false, capabilities: None },
                    BackendOptionView { id: BackendId::Keystore, available: true, capabilities: Some(caps_keystore()) },
                    BackendOptionView { id: BackendId::Csc, available: false, capabilities: None },
                ],
                privacy_notice: None,
            },
            bar(["AE", "AE", "A", ""], true),
        ),
    ));
    out.push((
        "backend_privacy",
        view(
            StepView::Backend {
                options: vec![BackendOptionView {
                    id: BackendId::AtrustNative,
                    available: true,
                    capabilities: Some(caps_native()),
                }],
                privacy_notice: Some(PrivacyNoticeView {
                    backend: BackendId::AtrustNative,
                    version: 1,
                    custom_text: Some("Eigener Text der Organisation".into()),
                }),
            },
            bar(["AE", "AE", "A", ""], true),
        ),
    ));
    for (name, progress) in [
        ("waiting_connecting", Progress::Connecting),
        ("waiting_certificate", Progress::ReadingCertificate),
        ("waiting_server", Progress::WaitingForServer),
        ("waiting_signing", Progress::Signing),
        ("waiting_document", Progress::Document { index: 2, total: 3 }),
        ("waiting_done", Progress::Done),
    ] {
        out.push((name, signing(InteractionView::Waiting { progress })));
    }
    out.push((
        "form_login",
        form(1, NativeForm::Login { prefill_username: Some("+43664123456789".into()), remember: true, ctx: ctx(None) }),
    ));
    for (name, problem) in [
        ("form_login_too_short", FormProblem::PasswordTooShort),
        ("form_login_too_long", FormProblem::PasswordTooLong),
        ("form_login_rejected", FormProblem::LoginRejected),
        ("form_login_atrust", FormProblem::Atrust("Benutzer gesperrt".into())),
        ("form_login_fido2_error", FormProblem::Fido2Error { code: "fido2-timeout".into(), detail: "x".into() }),
    ] {
        out.push((
            name,
            form(2, NativeForm::Login { prefill_username: None, remember: false, ctx: ctx(Some(problem)) }),
        ));
    }
    out.push(("form_sms", form(3, NativeForm::SmsTan { reference_value: "Xk7Pq".into(), ctx: ctx(None) })));
    for (name, problem) in [
        ("form_sms_no_tan", FormProblem::NoTan),
        ("form_sms_too_long", FormProblem::TanTooLong),
        ("form_sms_reference", FormProblem::EnteredReferenceValue),
    ] {
        out.push((name, form(4, NativeForm::SmsTan { reference_value: "Xk7Pq".into(), ctx: ctx(Some(problem)) })));
    }
    out.push((
        "form_qr",
        form(
            5,
            NativeForm::QrCode {
                reference_value: "Xk7Pq".into(),
                image: Some(QrImage { bytes: vec![137, 80, 78, 71], mime: Some("image/png".into()) }),
                timeout_secs: Some(300),
                ctx: ctx(None),
            },
        ),
    ));
    out.push((
        "form_qr_failed",
        form(
            6,
            NativeForm::QrCode {
                reference_value: "Xk7Pq".into(),
                image: None,
                timeout_secs: None,
                ctx: ctx(Some(FormProblem::QrImageFailed)),
            },
        ),
    ));
    out.push((
        "form_app",
        form(
            7,
            NativeForm::WaitForApp { reference_value: Some("Xk7Pq".into()), timeout_secs: Some(300), ctx: ctx(None) },
        ),
    ));
    out.push((
        "form_app_no_reference",
        form(8, NativeForm::WaitForApp { reference_value: None, timeout_secs: None, ctx: FormContext::default() }),
    ));
    out.push(("form_fido2", form(9, NativeForm::Fido2 { in_progress: true, ctx: ctx(None) })));
    out.push((
        "form_fido2_failed",
        form(
            10,
            NativeForm::Fido2 { in_progress: false, ctx: ctx(Some(FormProblem::Fido2Failed("fido2-timeout".into()))) },
        ),
    ));
    for (name, kind, retry) in [
        ("secret_keystore", SecretKind::KeystorePassword, false),
        ("secret_key_retry", SecretKind::KeyPassword, true),
        ("secret_proxy", SecretKind::ProxyPassword, false),
        ("secret_fido2_pin", SecretKind::Fido2Pin, false),
    ] {
        out.push((
            name,
            signing(InteractionView::Secret {
                request: 11,
                prompt: SecretPrompt { kind, subject: "keystore.p12".into(), retry },
            }),
        ));
    }
    out.push((
        "notice_interstitial",
        notice(
            12,
            NoticeKind::Info,
            NoticeCode::AtrustInterstitial,
            Some("Ihr Zertifikat läuft ab."),
            NoticeButtons::Ok,
        ),
    ));
    out.push((
        "notice_atrust_error",
        notice(13, NoticeKind::Error, NoticeCode::AtrustError, Some("Fehler 123"), NoticeButtons::RetryCancel),
    ));
    out.push(("notice_proxy", notice(14, NoticeKind::Error, NoticeCode::ProxyAuthRequired, None, NoticeButtons::Ok)));
    out.push((
        "notice_too_large",
        notice(15, NoticeKind::Error, NoticeCode::DocumentTooLarge, None, NoticeButtons::Ok),
    ));
    out.push((
        "notice_http",
        notice(16, NoticeKind::Error, NoticeCode::Http { status: 503 }, Some("Service Unavailable"), NoticeButtons::Ok),
    ));
    out.push((
        "notice_connection",
        notice(17, NoticeKind::Error, NoticeCode::ConnectionFailed, Some("timeout"), NoticeButtons::Ok),
    ));
    out.push((
        "notice_local_bku",
        notice(18, NoticeKind::Warning, NoticeCode::LocalBkuNotReachable, None, NoticeButtons::RetryCancel),
    ));
    out.push((
        "notice_keystore_missing",
        notice(
            19,
            NoticeKind::Error,
            NoticeCode::KeystoreFileMissing,
            Some("keystore.p12"),
            NoticeButtons::RetryCancel,
        ),
    ));
    out.push((
        "notice_alias_missing",
        notice(20, NoticeKind::Error, NoticeCode::KeystoreAliasMissing, Some("signer"), NoticeButtons::RetryCancel),
    ));
    out.push((
        "notice_keystore",
        notice(21, NoticeKind::Error, NoticeCode::KeystoreError, None, NoticeButtons::OkCancel),
    ));
    out.push((
        "notice_conversion",
        notice(22, NoticeKind::Warning, NoticeCode::KeystoreConversionNeeded, None, NoticeButtons::OkCancel),
    ));
    out.push((
        "output_saved",
        view(
            StepView::Output {
                result: OutputView {
                    path: Some(PathBuf::from("/Users/demo/Vertrag_signed.pdf")),
                    save_pending: false,
                    ask_overwrite: None,
                    save_dialog: None,
                },
                next: None,
            },
            bar(["AE", "AE", "AE", "A"], true),
        ),
    ));
    out.push((
        "output_pending_next",
        view(
            StepView::Output {
                result: OutputView { path: None, save_pending: true, ask_overwrite: None, save_dialog: None },
                next: Some(doc()),
            },
            bar(["AE", "AE", "AE", "A"], true),
        ),
    ));
    out.push((
        "output_ask_overwrite",
        view(
            StepView::Output {
                result: OutputView {
                    path: None,
                    save_pending: false,
                    ask_overwrite: Some(PathBuf::from("/Users/demo/Vertrag_signed.pdf")),
                    save_dialog: None,
                },
                next: None,
            },
            bar(["AE", "AE", "AE", "A"], true),
        ),
    ));
    out.push((
        "output_save_dialog",
        view(
            StepView::Output {
                result: OutputView {
                    path: None,
                    save_pending: false,
                    ask_overwrite: None,
                    save_dialog: Some(SaveDialogView {
                        directory: PathBuf::from("/Users/demo"),
                        file_name: "Vertrag_signed.pdf".into(),
                    }),
                },
                next: None,
            },
            bar(["AE", "AE", "AE", "A"], true),
        ),
    ));
    let mut params = BTreeMap::new();
    params.insert("arg1".to_owned(), "Vertrag.pdf".to_owned());
    out.push((
        "error",
        view(StepView::Error { code: "error-MayNotBeAPDF".into(), params, retry: true }, bar(["AE", "", "", ""], true)),
    ));
    out.push(("finished", view(StepView::Finished { exit_code: 4 }, bar(["", "", "", ""], false))));

    // Einstellungen als Overlay, mit unbekannten Einträgen (Settings::unknown, flatten)
    let mut settings = Settings::default();
    settings.general.ui_language = Some(Lang::En);
    settings.signature.note = NoteSetting::Custom("Eigener Hinweis".into());
    settings.signature.emblem = Some(PathBuf::from("/Users/demo/logo.png"));
    settings.keystore.enabled = true;
    settings.keystore.file = Some(PathBuf::from("/Users/demo/keystore.p12"));
    settings.output.folder = Some(PathBuf::from("."));
    settings.proxy.port = Some(8080);
    settings.privacy.acknowledged.insert("atrust_native".into(), 1);
    settings.unknown.insert("signature.future_key".into(), toml::Value::String("bleibt".into()));
    settings.unknown.insert("future".into(), toml::Value::Integer(42));
    let mut problems = BTreeMap::new();
    problems.insert("proxy.host".to_owned(), "ng-ui-problem-required".to_owned());
    let mut with_settings = view(StepView::Open { queue: vec![] }, bar(["AE", "", "", ""], false));
    with_settings.settings =
        Some(SettingsView { settings, problems, keychain_available: true, fido2_available: false, read_only: false });
    with_settings.language = Lang::En;
    out.push(("settings_open", with_settings));
    let mut defaults = view(StepView::Open { queue: vec![] }, bar(["AE", "", "", ""], false));
    defaults.settings = Some(SettingsView {
        settings: Settings::default(),
        problems: BTreeMap::new(),
        keychain_available: false,
        fido2_available: true,
        read_only: true,
    });
    out.push(("settings_defaults", defaults));
    out
}

fn to_fixture() -> serde_json::Value {
    let list: Vec<serde_json::Value> = samples()
        .into_iter()
        .map(|(name, view)| serde_json::json!({ "name": name, "view": serde_json::to_value(&view).expect("serialisierbar") }))
        .collect();
    serde_json::Value::Array(list)
}

#[test]
fn view_states_match_the_typescript_fixture() {
    let actual = to_fixture();
    let text = serde_json::to_string_pretty(&actual).expect("json") + "\n";
    if std::env::var_os("PDFOVER_BLESS").is_some() {
        std::fs::write(VIEW_FIXTURE, &text).expect("Fixture schreiben");
    }
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(VIEW_FIXTURE).expect("Fixture lesen (PDFOVER_BLESS=1 erzeugt sie)"),
    )
    .expect("Fixture ist JSON");
    assert_eq!(
        actual, expected,
        "view-states.json ist veraltet: PDFOVER_BLESS=1 cargo test -p pdfover-app --test serde_contract"
    );
}

#[test]
fn view_states_round_trip() {
    for (name, view) in samples() {
        let json = serde_json::to_value(&view).expect("serialisierbar");
        let back: ViewState = serde_json::from_value(json).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back, view, "{name}");
    }
}

#[test]
fn every_step_and_interaction_variant_is_covered() {
    let fixture = to_fixture();
    let list = fixture.as_array().expect("Liste");
    let steps: std::collections::BTreeSet<&str> =
        list.iter().filter_map(|s| s["view"]["step"]["step"].as_str()).collect();
    for step in [
        "starting",
        "open",
        "loading",
        "warning",
        "placeholder",
        "position",
        "backend",
        "signing",
        "output",
        "error",
        "finished",
    ] {
        assert!(steps.contains(step), "Schritt {step} fehlt");
    }
    let kinds: std::collections::BTreeSet<&str> =
        list.iter().filter_map(|s| s["view"]["step"]["interaction"]["kind"].as_str()).collect();
    for kind in ["waiting", "form", "secret", "notice"] {
        assert!(kinds.contains(kind), "Interaktion {kind} fehlt");
    }
}

/// Zahlen einheitlich als f64 (JavaScript schreibt `23`, serde `23.0` für f32/f64).
fn normalize(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Number(n) => serde_json::json!(n.as_f64().unwrap_or_default()),
        serde_json::Value::Array(a) => serde_json::Value::Array(a.into_iter().map(normalize).collect()),
        serde_json::Value::Object(o) => {
            serde_json::Value::Object(o.into_iter().map(|(k, v)| (k, normalize(v))).collect())
        }
        other => other,
    }
}

#[test]
fn typescript_events_deserialize_into_ui_event() {
    let text = std::fs::read_to_string(EVENT_FIXTURE).expect("ui-events.json lesen (npm test erzeugt sie)");
    let samples: Vec<serde_json::Value> = serde_json::from_str(&text).expect("JSON-Liste");
    assert!(samples.len() >= UI_EVENT_VARIANTS, "alle UiEvent-Varianten erwartet, gefunden {}", samples.len());
    let mut types = std::collections::BTreeSet::new();
    for sample in samples {
        let event: UiEvent = serde_json::from_value(sample.clone()).unwrap_or_else(|e| panic!("{sample}: {e}"));
        let back = serde_json::to_value(&event).expect("serialisierbar");
        assert_eq!(normalize(back), normalize(sample.clone()), "Rundlauf");
        types.insert(sample["type"].as_str().unwrap_or_default().to_owned());
        // Regel 9: Debug ohne Passwörter/TANs
        let debug = format!("{event:?}");
        assert!(!debug.contains("geheim-123"), "Debug enthält das Passwort: {debug}");
        assert!(!debug.contains("917253"), "Debug enthält die TAN: {debug}");
    }
    assert_eq!(types.len(), UI_EVENT_VARIANTS, "alle UiEvent-Varianten: {types:?}");
}

/// Anzahl der `UiEvent`-Varianten (die TS-Beispiele müssen jede abdecken).
const UI_EVENT_VARIANTS: usize = 32;

#[test]
fn unknown_events_are_rejected() {
    let unknown = serde_json::json!({ "type": "frobnicate" });
    assert!(serde_json::from_value::<UiEvent>(unknown).is_err(), "unbekanntes Ereignis");
    let missing = serde_json::json!({ "type": "move_block", "page": 0 });
    assert!(serde_json::from_value::<UiEvent>(missing).is_err());
}
