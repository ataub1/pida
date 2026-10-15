//! Signieren über die Kommandozeile: der Kern läuft ohne grafische Oberfläche
//! (`CoreConfig::headless`), die CLI ist die „UI“ und beantwortet Masken, Passwortabfragen,
//! Meldungen, Datenschutzhinweis und Überschreib-Rückfragen im Terminal (Terminal-Broker).
//!
//! - `--headless` bzw. kein Terminal: keine Rückfragen (Keystore mit Passwort aus `-kspass`,
//!   `--kspass-stdin`, `--kspass-env`; lokale BKU); ID Austria ist dann nicht verfügbar.
//! - Mit Terminal: ID Austria (Anmeldung, SMS-TAN verdeckt, QR-Code als Datei, App-Bestätigung mit
//!   Countdown, FIDO2), Keystore-Passwort verdeckt.
//! - Strg+C bricht den laufenden Vorgang ab (zweimal: Programmende).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use pdfover_backend_api::{NativeForm, NoticeAnswer, NoticeButtons, Progress, SecretKind};
use pdfover_config::{BackendChoice, Paths};
use pdfover_core_api::messages::{form_problem_message, notice_message, progress_message};
use pdfover_core_api::{
    Core, CoreConfig, CoreOptions, DoneItemView, FormAnswerView, InteractionView, StepView, UiEvent,
};
use pdfover_i18n::Lang;
use pdfover_workflow::{DocResult, ExitCode};
use zeroize::Zeroizing;

use crate::overlay::{self, ArgError};
use crate::term::Terminal;
use crate::text::{is_yes, tr, tr_map};
use crate::{Cli, LegacyBku};

/// Laufzeitumgebung (Tests setzen Pfade, Terminal und Kernoptionen).
pub struct RunEnv {
    /// Terminal
    pub term: Arc<dyn Terminal>,
    /// Pfade (`None` = Plattform)
    pub paths: Option<Paths>,
    /// Kernoptionen (Engine, Secrets …)
    pub options: CoreOptions,
}

impl std::fmt::Debug for RunEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunEnv").field("paths", &self.paths).field("options", &self.options).finish_non_exhaustive()
    }
}

/// Fehler der Argumente ausgeben (Exit 2).
pub fn usage_error(term: &dyn Terminal, lang: Lang, e: &ArgError) -> ExitCode {
    let msg = tr(lang, e.id, &e.args);
    term.info(&format!("{}\n{msg}", tr(lang, "error-CmdLineArgs", &[])));
    ExitCode::Usage
}

/// Signieren (`sign` bzw. Dateien ohne Befehl).
pub fn sign(config_file: Option<PathBuf>, cli: &Cli, env: RunEnv) -> ExitCode {
    let lang = Lang::system();
    let term = env.term.clone();
    if let Err(e) = overlay::check(cli) {
        return usage_error(term.as_ref(), lang, &e);
    }
    let mut ov = overlay::to_overlay(cli);
    ov.config_file = config_file;
    match overlay::keystore_password(cli, term.as_ref()) {
        Ok(Some(p)) => ov.keystore_store_password = Some(p),
        Ok(None) => {}
        Err(e) => return usage_error(term.as_ref(), lang, &e),
    }
    match overlay::inputs(cli) {
        Ok(inputs) if !inputs.is_empty() => ov.input_files = inputs,
        Ok(_) => return usage_error(term.as_ref(), lang, &ArgError { id: "ng-cli-no-input", args: vec![] }),
        Err(e) => return usage_error(term.as_ref(), lang, &e),
    }
    // In der CLI wählt `-ks` ohne `-b` den Keystore (sonst wäre die Wahl ohne Oberfläche mehrdeutig).
    if ov.keystore_file.is_some() && ov.backend.is_none() && cli.legacy.bku != Some(LegacyBku::None) {
        ov.backend = Some(BackendChoice::Keystore);
    }
    let paths = match env.paths.clone().map(Ok).unwrap_or_else(Paths::platform) {
        Ok(p) => p,
        Err(e) => {
            term.info(&e.localized(lang));
            return ExitCode::General;
        }
    };
    let interactive = !cli.modern.headless && term.interactive();
    let temp_dir = paths.temp_dir.clone();
    // Erreichbarkeit der lokalen BKU nur prüfen, wenn sie in Frage kommt (`-b LOCAL` oder ohne Vorgabe)
    let probe_local_bku = env.options.probe_local_bku
        && !matches!(ov.backend, Some(BackendChoice::Keystore) | Some(BackendChoice::IdAustria));
    let config = CoreConfig {
        paths,
        overlay: ov,
        platform: pdfover_backend_api::Platform::current(),
        pdfium_dir: None,
        headless: true,
        fido2: pdfover_core_api::platform_fido2(),
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    let options = CoreOptions { interactive: Some(interactive), init_logging: false, probe_local_bku, ..env.options };
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            term.info(&e.to_string());
            return ExitCode::General;
        }
    };
    let json = cli.modern.json;
    rt.block_on(async move {
        let core = match Core::new_with(config, options) {
            Ok(c) => Arc::new(c),
            Err(e) => {
                let mut p = BTreeMap::new();
                p.insert("detail".to_owned(), e.to_string());
                if let pdfover_core_api::CoreError::Config(c) = &e {
                    term.info(&c.localized(lang));
                } else {
                    term.info(&tr_map(lang, e.message_id(), &p));
                }
                return if matches!(&e, pdfover_core_api::CoreError::Config(c) if c.missing_path().is_some()) {
                    ExitCode::Usage
                } else {
                    ExitCode::General
                };
            }
        };
        spawn_ctrl_c(core.clone());
        ui_loop(&core, term.clone(), lang, interactive, temp_dir).await;
        let code = core.wait_finished().await;
        report(term.as_ref(), lang, &core.results(), code, json);
        code
    })
}

/// Strg+C: erst Abbruch des Vorgangs, dann Programmende.
fn spawn_ctrl_c(core: Arc<Core>) {
    tokio::spawn(async move {
        let mut count = 0u32;
        while tokio::signal::ctrl_c().await.is_ok() {
            count += 1;
            match count {
                1 => core.dispatch(UiEvent::Cancel),
                2 => core.dispatch(UiEvent::Quit),
                _ => std::process::exit(ExitCode::Cancelled.code()),
            }
        }
    });
}

/// Eine Rückfrage im Terminal (blockierend, im Blocking-Pool).
enum Prompt {
    Login { request: u64, prefill: Option<String>, remember: bool },
    Tan { request: u64, reference: String },
    Secret { request: u64, kind: SecretKind, subject: String },
    Notice { request: u64, text: String, buttons: NoticeButtons },
    Privacy,
    Overwrite(PathBuf),
}

fn ask(term: &dyn Terminal, lang: Lang, prompt: &Prompt) -> Option<UiEvent> {
    match prompt {
        Prompt::Login { request, prefill, remember } => {
            let username = match prefill {
                Some(u) => u.clone(),
                None => term.line(&tr(lang, "ng-cli-username", &[]))?,
            };
            let password = term.secret(&tr(lang, "ng-cli-password", &[]))?;
            let answer = FormAnswerView::Login { username, password: password.to_string(), remember: *remember };
            Some(UiEvent::SubmitForm { request: *request, answer })
        }
        Prompt::Tan { request, reference } => {
            let tan = term.secret(&tr(lang, "ng-cli-tan", &[("reference", reference.clone())]))?;
            Some(UiEvent::SubmitForm { request: *request, answer: FormAnswerView::Tan { tan: tan.to_string() } })
        }
        Prompt::Secret { request, kind, subject } => {
            let id = match kind {
                SecretKind::KeystorePassword => "ng-cli-keystore-password",
                SecretKind::KeyPassword => "ng-cli-key-password",
                SecretKind::ProxyPassword => "ng-cli-proxy-password",
            };
            let secret: Zeroizing<String> = term.secret(&tr(lang, id, &[("name", subject.clone())]))?;
            Some(UiEvent::SubmitSecret { request: *request, secret: secret.to_string() })
        }
        Prompt::Notice { request, text, buttons } => {
            term.info(text);
            let answer = match buttons {
                NoticeButtons::Ok => NoticeAnswer::Ok,
                NoticeButtons::OkCancel => match term.line(&tr(lang, "ng-cli-continue", &[])) {
                    Some(a) if is_yes(&a) => NoticeAnswer::Ok,
                    _ => NoticeAnswer::Cancel,
                },
                NoticeButtons::RetryCancel => match term.line(&tr(lang, "ng-cli-retry", &[])) {
                    Some(a) if is_yes(&a) => NoticeAnswer::Retry,
                    _ => NoticeAnswer::Cancel,
                },
            };
            Some(UiEvent::AnswerNotice { request: *request, answer })
        }
        Prompt::Privacy => {
            let ok = term.line(&tr(lang, "ng-cli-privacy", &[])).is_some_and(|a| is_yes(&a));
            Some(UiEvent::AcknowledgePrivacy { accepted: ok })
        }
        Prompt::Overwrite(p) => {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let ok = term.line(&tr(lang, "ng-cli-overwrite", &[("name", name)])).is_some_and(|a| is_yes(&a));
            Some(UiEvent::ConfirmOverwrite { overwrite: ok })
        }
    }
}

/// Was im aktuellen Zustand zu fragen bzw. anzuzeigen ist.
struct Ui {
    lang: Lang,
    interactive: bool,
    temp_dir: PathBuf,
    handled: HashSet<u64>,
    notices: HashSet<u64>,
    progress: Option<Progress>,
    privacy_asked: bool,
    overwrite_asked: Option<PathBuf>,
    countdown: Option<tokio::task::JoinHandle<()>>,
}

impl Ui {
    fn stop_countdown(&mut self) {
        if let Some(h) = self.countdown.take() {
            h.abort();
        }
    }

    fn start_countdown(&mut self, term: Arc<dyn Terminal>, secs: Option<u32>) {
        self.stop_countdown();
        let Some(total) = secs else { return };
        let lang = self.lang;
        self.countdown = Some(tokio::spawn(async move {
            let mut left = total;
            while left > 0 {
                let step = left.min(30);
                tokio::time::sleep(Duration::from_secs(u64::from(step))).await;
                left -= step;
                if left > 0 {
                    term.info(&tr(lang, "ng-cli-countdown", &[("seconds", left.to_string())]));
                }
            }
        }));
    }

    /// Wertet den ViewState aus; liefert eine blockierende Rückfrage oder ein sofortiges Ereignis.
    fn next(&mut self, step: &StepView, term: &Arc<dyn Terminal>) -> (Option<Prompt>, Option<UiEvent>) {
        let lang = self.lang;
        if !matches!(step, StepView::Backend { privacy_notice: Some(_), .. }) {
            self.privacy_asked = false;
        }
        match step {
            StepView::Signing { interaction, .. } => {
                if !matches!(
                    interaction,
                    InteractionView::Form { form: NativeForm::QrCode { .. } | NativeForm::WaitForApp { .. }, .. }
                ) {
                    self.stop_countdown();
                }
                match interaction {
                    InteractionView::Waiting { progress } => {
                        if self.progress.as_ref() != Some(progress) {
                            self.progress = Some(progress.clone());
                            let (id, p) = progress_message(progress);
                            tracing::info!(message = %tr_map(lang, &id, &p));
                        }
                        (None, None)
                    }
                    InteractionView::Form { request, form } if self.handled.insert(*request) => {
                        self.form(*request, form, term)
                    }
                    InteractionView::Secret { request, prompt } if self.handled.insert(*request) => {
                        if !self.interactive {
                            return (None, Some(UiEvent::Cancel));
                        }
                        if prompt.retry {
                            term.info(&tr(lang, "ng-cli-wrong-password", &[]));
                        }
                        let p = Prompt::Secret {
                            request: *request,
                            kind: prompt.kind.clone(),
                            subject: prompt.subject.clone(),
                        };
                        (Some(p), None)
                    }
                    InteractionView::Notice { request, notice } if self.handled.insert(*request) => {
                        let (id, params) = notice_message(notice);
                        let mut text = tr_map(lang, &id, &params);
                        if id == "common-info"
                            && let Some(t) = &notice.text
                        {
                            text = format!("{text}: {t}");
                        }
                        if !self.interactive {
                            term.info(&text);
                            let answer = if notice.buttons == NoticeButtons::Ok {
                                NoticeAnswer::Ok
                            } else {
                                NoticeAnswer::Cancel
                            };
                            return (None, Some(UiEvent::AnswerNotice { request: *request, answer }));
                        }
                        (Some(Prompt::Notice { request: *request, text, buttons: notice.buttons }), None)
                    }
                    _ => (None, None),
                }
            }
            StepView::Backend { privacy_notice: Some(_), .. } if !self.privacy_asked => {
                self.privacy_asked = true;
                if self.interactive {
                    (Some(Prompt::Privacy), None)
                } else {
                    (None, Some(UiEvent::AcknowledgePrivacy { accepted: false }))
                }
            }
            StepView::Output { result, .. } => match &result.ask_overwrite {
                Some(p) if self.overwrite_asked.as_ref() != Some(p) => {
                    self.overwrite_asked = Some(p.clone());
                    if self.interactive {
                        (Some(Prompt::Overwrite(p.clone())), None)
                    } else {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        term.info(&tr(lang, "output-file_ask_overwrite", &[("arg1", name)]));
                        (None, Some(UiEvent::ConfirmOverwrite { overwrite: false }))
                    }
                }
                Some(_) => (None, None),
                None => {
                    self.overwrite_asked = None;
                    (None, None)
                }
            },
            StepView::Error { .. } => (None, Some(UiEvent::Cancel)),
            _ => (None, None),
        }
    }

    fn form(&mut self, request: u64, form: &NativeForm, term: &Arc<dyn Terminal>) -> (Option<Prompt>, Option<UiEvent>) {
        let lang = self.lang;
        let ctx = match form {
            NativeForm::Login { ctx, .. }
            | NativeForm::SmsTan { ctx, .. }
            | NativeForm::QrCode { ctx, .. }
            | NativeForm::WaitForApp { ctx, .. }
            | NativeForm::Fido2 { ctx, .. } => ctx,
        };
        if let Some(problem) = &ctx.problem {
            let (id, p) = form_problem_message(problem);
            term.info(&tr_map(lang, &id, &p));
        }
        if !self.interactive {
            return (None, Some(UiEvent::Cancel));
        }
        match form {
            NativeForm::Login { prefill_username, remember, .. } => {
                (Some(Prompt::Login { request, prefill: prefill_username.clone(), remember: *remember }), None)
            }
            NativeForm::SmsTan { reference_value, .. } => {
                (Some(Prompt::Tan { request, reference: reference_value.clone() }), None)
            }
            NativeForm::QrCode { reference_value, image, timeout_secs, .. } => {
                let path = self.temp_dir.join(format!("pdfover-qr-{}-{request}.png", std::process::id()));
                let shown = match image {
                    Some(img) => {
                        match std::fs::create_dir_all(&self.temp_dir).and_then(|_| std::fs::write(&path, &img.bytes)) {
                            Ok(()) => path.display().to_string(),
                            Err(_) => "-".into(),
                        }
                    }
                    None => tr(lang, "error-FailedToLoadQRCode", &[]),
                };
                term.info(&tr(lang, "ng-cli-qr", &[("reference", reference_value.clone()), ("path", shown)]));
                self.start_countdown(term.clone(), *timeout_secs);
                (None, None)
            }
            NativeForm::WaitForApp { reference_value, timeout_secs, .. } => {
                let reference = reference_value.clone().unwrap_or_else(|| "-".into());
                term.info(&tr(lang, "ng-cli-app", &[("reference", reference)]));
                self.start_countdown(term.clone(), *timeout_secs);
                (None, None)
            }
            NativeForm::Fido2 { in_progress, .. } => {
                term.info(&tr(lang, "ng-cli-fido2", &[]));
                if *in_progress {
                    (None, None)
                } else {
                    (None, Some(UiEvent::SubmitForm { request, answer: FormAnswerView::StartFido2 }))
                }
            }
        }
    }
}

/// Schleife der Terminal-„UI“ bis zum Programmende.
pub async fn ui_loop(core: &Arc<Core>, term: Arc<dyn Terminal>, lang: Lang, interactive: bool, temp_dir: PathBuf) {
    let mut rx = core.subscribe();
    let mut ui = Ui {
        lang,
        interactive,
        temp_dir,
        handled: HashSet::new(),
        notices: HashSet::new(),
        progress: None,
        privacy_asked: false,
        overwrite_asked: None,
        countdown: None,
    };
    loop {
        let view = rx.borrow_and_update().clone();
        for n in &view.notices {
            if ui.notices.insert(n.id) {
                term.info(&tr_map(lang, &n.code, &n.params));
                core.dispatch(UiEvent::DismissNotice { id: n.id });
            }
        }
        if matches!(view.step, StepView::Finished { .. }) {
            break;
        }
        let (prompt, event) = ui.next(&view.step, &term);
        if let Some(e) = event {
            core.dispatch(e);
        }
        if let Some(prompt) = prompt {
            let t = term.clone();
            let mut handle = tokio::task::spawn_blocking(move || ask(t.as_ref(), lang, &prompt));
            loop {
                tokio::select! {
                    answer = &mut handle => {
                        match answer {
                            Ok(Some(e)) => core.dispatch(e),
                            // EOF/Abbruch am Terminal
                            _ => core.dispatch(UiEvent::Cancel),
                        }
                        break;
                    }
                    changed = rx.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        // Die Rückfrage hat sich erledigt (z. B. Abbruch per Strg+C)?
                        let v = rx.borrow().clone();
                        if matches!(v.step, StepView::Finished { .. }) {
                            restore_echo();
                            return;
                        }
                    }
                }
            }
        }
        if rx.changed().await.is_err() {
            break;
        }
    }
    ui.stop_countdown();
}

/// Nach einer abgebrochenen verdeckten Eingabe das Echo des Terminals wieder einschalten.
fn restore_echo() {
    #[cfg(unix)]
    {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() {
            let _ = std::process::Command::new("stty").arg("echo").stdin(std::process::Stdio::inherit()).status();
        }
    }
}

/// Ergebnis ausgeben (Text bzw. `--json`).
pub fn report(term: &dyn Terminal, lang: Lang, results: &[DoneItemView], code: ExitCode, json: bool) {
    if json {
        let docs: Vec<serde_json::Value> = results
            .iter()
            .map(|r| match &r.result {
                DocResult::Signed { output } => serde_json::json!({
                    "input": r.input,
                    "result": "signed",
                    "output": output.path,
                    "len": output.len,
                    "sha256": output.sha256,
                }),
                DocResult::Cancelled => serde_json::json!({ "input": r.input, "result": "cancelled" }),
                DocResult::Failed { error } => {
                    let params = error.params();
                    serde_json::json!({
                        "input": r.input,
                        "result": "failed",
                        "error": error.code(),
                        "message": tr_map(lang, &error.code(), &params),
                        "params": params,
                    })
                }
            })
            .collect();
        let v = serde_json::json!({ "exit_code": code.code(), "result": code.as_str(), "documents": docs });
        term.out(&serde_json::to_string_pretty(&v).unwrap_or_default());
        return;
    }
    for r in results {
        let input = r.input.display().to_string();
        match &r.result {
            DocResult::Signed { output } => {
                term.out(&tr(
                    lang,
                    "ng-cli-signed",
                    &[("input", input), ("output", output.path.display().to_string())],
                ));
            }
            DocResult::Cancelled => term.info(&tr(lang, "ng-cli-cancelled", &[("input", input)])),
            DocResult::Failed { error } => {
                let message = tr_map(lang, &error.code(), &error.params());
                term.info(&tr(lang, "ng-cli-failed", &[("input", input), ("message", message)]));
            }
        }
    }
}
