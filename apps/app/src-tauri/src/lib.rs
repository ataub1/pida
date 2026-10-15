//! `pdfover-app` – Tauri-2-Shell von PDF-Over NG (Desktop; Mobile folgt in einer späteren Welle).
//!
//! Die Shell ist **dünn** (Plan §3.1): Sie startet `pdfover-core-api`, leitet den [`ViewState`] als
//! Ereignis [`VIEW_EVENT`] an das Fenster, nimmt [`pdfover_core_api::UiEvent`]s über den Command
//! `core_dispatch` entgegen und liefert Seitenkacheln und die Blockvorschau über das Custom Protocol
//! `pdfover-render://` ([`protocol`]). Fachlogik liegt nur im Kern; die Oberfläche (React, React Aria
//! Components) projiziert nur den Zustand.
//!
//! Plattformintegration:
//! - **Single-Instance** (Desktop): eine zweite Instanz übergibt ihre Dokumente an die erste (F-13).
//! - **„Öffnen mit“**: Startargumente ([`files::documents_from_args`]) unter Windows/Linux und
//!   `RunEvent::Opened` (macOS-Apple-Events zur Datei-Zuordnung `.pdf` im Bundle; iOS/Android „Öffnen
//!   in“). Das Plugin `deep-link` kommt erst mit eigenen URL-Schemata bzw. App Links (Szenario C/D,
//!   Mobile) hinzu; in v1 gäbe es nur tote Abhängigkeiten (u. a. `tiny-keccak`, CC0, im Lizenz-Gate).
//! - **Drag & Drop** über die Fenster-Ereignisse von Tauri (Oberfläche: `onDragDropEvent`).
//! - **Fensterzustand** (Plugin `window-state`), Dialoge (`dialog`), Öffnen im System (`opener`).
//! - Logs: Datei im Log-Ordner der Plattform ([`pdfover_config::Paths`]), rotierend, ohne Secrets und
//!   Dokumentinhalte (Regel 9).
//!
//! Ownership: Paket **I3** (Welle 2), danach Integration.

pub mod files;
pub mod pdfium;
pub mod protocol;
pub mod shell;

use std::sync::Arc;

use pdfover_backend_api::Platform;
use pdfover_config::{Overlay, Paths};
use pdfover_core_api::{CoreConfig, ViewState};
use pdfover_i18n::Lang;
use tauri::{AppHandle, Emitter, Manager, State};

use shell::{DispatchOutcome, Shell, Started};

/// Name des Ereignisses mit dem jeweils aktuellen [`ViewState`].
pub const VIEW_EVENT: &str = "pdfover:view";

/// Informationen für den Über-Dialog.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppInfo {
    /// App-Version
    pub version: String,
    /// Plattform
    pub platform: Platform,
    /// Kern verfügbar
    pub core_available: bool,
}

#[tauri::command]
fn core_view(shell: State<'_, Shell>) -> ViewState {
    shell.view()
}

#[tauri::command]
fn core_dispatch(app: AppHandle, shell: State<'_, Shell>, event: serde_json::Value) -> Result<(), String> {
    match shell.dispatch_json(event).map_err(str::to_owned)? {
        DispatchOutcome::Dispatched => {}
        DispatchOutcome::Restart => start_core(&app, &shell),
        DispatchOutcome::Quit => app.exit(0),
    }
    Ok(())
}

#[tauri::command]
fn app_info(app: AppHandle, shell: State<'_, Shell>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        platform: Platform::current(),
        core_available: shell.core().is_some(),
    }
}

#[tauri::command]
fn open_log_dir(app: AppHandle, shell: State<'_, Shell>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = shell.log_dir();
    std::fs::create_dir_all(&dir).map_err(|_| "ng-ui-open-failed".to_owned())?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|_| "ng-ui-open-failed".to_owned())
}

/// Zeigt die Ergebnisdatei im Dateimanager – nur die Datei des aktuellen Ergebnis-Schritts.
#[tauri::command]
fn reveal_output(app: AppHandle, shell: State<'_, Shell>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = shell.output_path().ok_or_else(|| "ng-ui-open-failed".to_owned())?;
    app.opener().reveal_item_in_dir(path).map_err(|_| "ng-ui-open-failed".to_owned())
}

/// Startet den Kern (bzw. erneut nach „Wiederholen“) und abonniert den ViewState.
fn start_core(app: &AppHandle, shell: &Shell) {
    // Der Kern darf `tokio::spawn` im Kontext der Tauri-Laufzeit nutzen.
    let runtime = tauri::async_runtime::handle();
    let _guard = runtime.inner().enter();
    match shell.start() {
        Started::Running(core) => {
            let mut rx = core.subscribe();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let view = rx.borrow_and_update().clone();
                    let finished = match &view.step {
                        pdfover_core_api::StepView::Finished { exit_code } => Some(i32::from(*exit_code)),
                        _ => None,
                    };
                    let _ = app.emit(VIEW_EVENT, &view);
                    if let Some(code) = finished {
                        app.exit(code);
                        break;
                    }
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
            });
        }
        Started::Unavailable => {
            let _ = app.emit(VIEW_EVENT, shell.view());
        }
    }
}

fn core_config(app: &AppHandle) -> Result<CoreConfig, pdfover_config::ConfigError> {
    let paths = Paths::platform()?;
    let env_dir = std::env::var_os("PDFOVER_PDFIUM_DIR").map(std::path::PathBuf::from);
    let exe = std::env::current_exe().ok();
    let resources = app.path().resource_dir().ok();
    let pdfium_dir = pdfium::find(&pdfium::candidates(env_dir, exe.as_deref(), resources.as_deref()));
    Ok(CoreConfig {
        paths,
        overlay: Overlay::default(),
        platform: Platform::current(),
        pdfium_dir,
        headless: false,
        fido2: fido2_provider(),
        app_version: app.package_info().version.to_string(),
    })
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
fn fido2_provider() -> Option<Arc<dyn pdfover_backend_atrust::Fido2Provider>> {
    pdfover_backend_atrust::fido2::platform_provider()
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn fido2_provider() -> Option<Arc<dyn pdfover_backend_atrust::Fido2Provider>> {
    None
}

/// Logdatei im Log-Ordner der Plattform (rotierend, Plan §7.1); Filter über `PDFOVER_LOG`.
fn init_logging(paths: &Paths) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_env("PDFOVER_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file = paths
        .rotate_logs(pdfover_config::paths::LOG_MAX_FILE_BYTES, pdfover_config::paths::LOG_KEEP)
        .ok()
        .and_then(|log| std::fs::OpenOptions::new().create(true).append(true).open(log).ok());
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_target(true);
    let _ = match file {
        Some(file) => builder.with_writer(std::sync::Mutex::new(file)).try_init(),
        None => builder.with_writer(std::io::stderr).try_init(),
    };
}

/// Systemsprache für `Lang::system` (unter Windows/macOS-GUI fehlen die POSIX-Variablen).
fn init_language() {
    if let Some(lang) = sys_locale::get_locale().as_deref().and_then(Lang::from_tag) {
        Lang::set_system(Some(lang));
    }
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Einstieg (Desktop über `main.rs`, Mobile über den Einstiegspunkt der Plattform).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_language();

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        // Muss als erstes Plugin registriert werden.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let docs = files::documents_from_args(argv.iter().skip(1), std::path::Path::new(&cwd));
            app.state::<Shell>().open_files(docs);
            focus_main_window(app);
        }));
        builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());
    }

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            let query = request.uri().query().map(str::to_owned);
            tauri::async_runtime::spawn_blocking(move || {
                let response = app.state::<Shell>().render(&path, query.as_deref());
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![core_view, core_dispatch, app_info, open_log_dir, reveal_output])
        .setup(|app| {
            let handle = app.handle().clone();
            let config = core_config(&handle)?;
            init_logging(&config.paths);
            tracing::info!(version = %handle.package_info().version, "PDF-Over NG startet");
            app.manage(Shell::new(config));
            let shell = handle.state::<Shell>();
            start_core(&handle, &shell);
            // Dokumente aus den Startargumenten (Windows/Linux „Öffnen mit“, Kommandozeile).
            let cwd = std::env::current_dir().unwrap_or_default();
            let docs = files::documents_from_args(std::env::args().skip(1), &cwd);
            shell.open_files(docs);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri-Anwendung konnte nicht erstellt werden");

    app.run(|app, event| match event {
        tauri::RunEvent::WindowEvent { event: tauri::WindowEvent::CloseRequested { .. }, .. } => {
            // Fenster schließen während eines Dialogs gilt als Abbruch (F-73); der Kern räumt auf.
            if let Some(core) = app.state::<Shell>().core() {
                core.dispatch(pdfover_core_api::UiEvent::Quit);
            }
        }
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        tauri::RunEvent::Opened { urls } => {
            app.state::<Shell>().open_files(files::documents_from_urls(&urls));
            focus_main_window(app);
        }
        _ => {}
    });
}
