//! `pdfover-app` – Tauri-2-Shell von PDF-Over NG (Desktop; Mobile folgt in einer späteren Welle).
//!
//! Die Shell ist **dünn** (Plan §3.1): Sie startet `pdfover-core-api`, leitet den [`ViewState`] als
//! Ereignis [`VIEW_EVENT`] an das Fenster, nimmt [`pdfover_core_api::UiEvent`]s über den Command
//! `core_dispatch` entgegen und liefert Seitenkacheln und die Blockvorschau über das Custom Protocol
//! `pdfover-render://` ([`protocol`]). Fachlogik liegt nur im Kern; die Oberfläche (React, React Aria
//! Components) projiziert nur den Zustand.
//!
//! Plattformintegration:
//! - **Startargumente** mit der vollständigen PDF-Over-Semantik ([`args`], derselbe Parser wie
//!   `pdfover-cli`): `-i -o -b -n -p -ks… -proxy… -e -a -s -c -v` als Overlay des Kerns; ungültige
//!   Argumente zeigen den Fehlerschritt (`error-CmdLineArgs`), `-h` gibt die Hilfe aus.
//! - **Single-Instance** (Desktop): eine zweite Instanz übergibt ihre Dokumente an die erste (F-13,
//!   [`args::second_instance_documents`]); unter macOS nur mit eigenem Socket ([`instance`]).
//! - **„Öffnen mit“**: Startargumente unter Windows/Linux und `RunEvent::Opened`
//!   (macOS-Apple-Events zur Datei-Zuordnung `.pdf` im Bundle; iOS/Android „Öffnen in“). Dokumente, die
//!   vor `setup` eintreffen (macOS-Kaltstart über ein Dokument), merkt [`instance::EarlyDocuments`] vor.
//! - **Shell-Funktionen** des Kerns ([`hooks`]): Ergebnis öffnen (F-87), „Signaturdaten anzeigen“ im
//!   Browser (F-68), Elternfenster für Windows-Hello/FIDO2.
//! - **Sprache**: `Lang::set_system` aus `sys-locale` (unter Windows/macOS-GUI fehlen POSIX-Variablen). Das Plugin `deep-link` kommt erst mit eigenen URL-Schemata bzw. App Links (Szenario C/D,
//!   Mobile) hinzu; in v1 gäbe es nur tote Abhängigkeiten (u. a. `tiny-keccak`, CC0, im Lizenz-Gate).
//! - **Drag & Drop** über die Fenster-Ereignisse von Tauri (Oberfläche: `onDragDropEvent`).
//! - **Fensterzustand** (Plugin `window-state`), Dialoge (`dialog`), Öffnen im System (`opener`).
//! - Logs: Datei im Log-Ordner der Plattform ([`pdfover_config::Paths`]) über
//!   `pdfover_core_api::logging` (rotierend auch während des Laufs), ohne Secrets und Dokumentinhalte
//!   (Regel 9); Filter über `--log-level`, sonst `PDFOVER_LOG`.
//! - **Kernfehler**: Startfehler und Paniken erscheinen als Fehlerschritt `ng-ui-core-unavailable`, ein
//!   unerwartet beendeter Kern-Task als [`CORE_STOPPED`] – jeweils mit „Wiederholen“ (Neustart des Kerns).
//! - **Startprüfung** (`PDFOVER_SMOKE=1`): beendet die App, sobald die Oberfläche den ersten
//!   ViewState abgefragt hat und der Kern bereit ist; gibt die Startzeiten auf stdout aus ([`smoke`]).
//! - **Beenden** (Fenster schließen, ⌘Q): läuft über den Kern (`UiEvent::Quit` → `Finished`), damit
//!   laufende Dialoge abgebrochen (F-73) und Einstellungen gesichert werden (F-105); antwortet der Kern
//!   nicht binnen [`QUIT_GRACE`], endet die App trotzdem.
//! - **Berechtigungen** der Oberfläche (capabilities/default.json) sind minimal: Ereignisse empfangen,
//!   die eigenen Commands, Datei-/Speichern-Dialog und `https:`/`mailto:`-Links – kein `core:default`
//!   (Menüs, Tray, Fenster-/Pfad-APIs bleiben gesperrt).
//!
//! Ownership: Paket **I3** (Welle 2), danach Integration.

pub mod args;
pub mod files;
pub mod hooks;
pub mod instance;
pub mod pdfium;
pub mod protocol;
pub mod shell;
pub mod smoke;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pdfover_backend_api::Platform;
use pdfover_config::{Overlay, Paths};
use pdfover_core_api::{CoreConfig, ViewState};
use pdfover_i18n::Lang;
use tauri::{AppHandle, Emitter, Manager, State};

use args::GuiArgs;
use instance::EarlyDocuments;
use shell::{DispatchOutcome, QuitRequest, Shell, Started};

/// Meldungs-ID: der Kern-Task hat unerwartet geendet (Detail im Fehlerschritt).
pub const CORE_STOPPED: &str = "ng-core-stopped";

/// Name des Ereignisses mit dem jeweils aktuellen [`ViewState`].
pub const VIEW_EVENT: &str = "pdfover:view";

/// Wartezeit auf `Finished` nach einem Beenden-Wunsch, bevor die App ohne Kern endet.
pub const QUIT_GRACE: Duration = Duration::from_secs(5);

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
fn core_view(app: AppHandle, shell: State<'_, Shell>) -> ViewState {
    // Startprüfung: die Oberfläche läuft und spricht mit der Shell
    smoke::ui_ready(&app, &shell);
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
    let dir = shell.log_dir().ok_or_else(|| "ng-ui-open-failed".to_owned())?;
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

/// Name der Drittlizenz-Hinweise im Ressourcenordner (scripts/bundle-config.mjs, Plan §2.5).
pub const NOTICES_FILE: &str = "THIRD-PARTY-NOTICES.md";

/// Öffnet die Drittlizenz-Hinweise des Pakets (Über-Dialog); fehlen sie (Entwicklungsbuild ohne
/// `npm run notices`), liefert der Command `ng-ui-open-failed`.
#[tauri::command]
fn open_notices(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let file = app.path().resource_dir().map(|d| d.join(NOTICES_FILE)).map_err(|_| "ng-ui-open-failed".to_owned())?;
    if !file.is_file() {
        tracing::info!("Drittlizenz-Hinweise nicht im Paket");
        return Err("ng-ui-open-failed".to_owned());
    }
    app.opener().open_path(file.to_string_lossy(), None::<&str>).map_err(|_| "ng-ui-open-failed".to_owned())
}

/// Startet den Kern (bzw. erneut nach „Wiederholen“) und abonniert den ViewState.
fn start_core(app: &AppHandle, shell: &Shell) {
    // Der Kern darf `tokio::spawn` im Kontext der Tauri-Laufzeit nutzen.
    let runtime = tauri::async_runtime::handle();
    let _guard = runtime.inner().enter();
    match shell.start() {
        Started::Running(core) => {
            let Ok(mut rx) = catch_unwind(AssertUnwindSafe(|| core.subscribe())) else {
                tracing::error!("ViewState-Abo fehlgeschlagen (Panik)");
                shell.mark_unavailable("subscribe");
                let _ = app.emit(VIEW_EVENT, shell.view());
                return;
            };
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
                        // Kern-Task beendet, ohne `Finished` zu melden: Fehlerschritt mit „Wiederholen“
                        tracing::error!("Kern-Task unerwartet beendet");
                        if let Some(shell) = app.try_state::<Shell>() {
                            shell.mark_stopped(CORE_STOPPED);
                            let _ = app.emit(VIEW_EVENT, shell.view());
                        }
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

/// Beenden über den Kern anfordern (Fenster schließen, ⌘Q). `true` = der Kern beendet die App über
/// `Finished` (Schließen/Beenden jetzt verhindern); nach [`QUIT_GRACE`] endet sie in jedem Fall.
fn quit_via_core(app: &AppHandle) -> bool {
    let Some(shell) = app.try_state::<Shell>() else {
        return false;
    };
    match shell.request_quit() {
        QuitRequest::NoCore => false,
        QuitRequest::AlreadyRequested => true,
        QuitRequest::Requested => {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(QUIT_GRACE);
                tracing::warn!("Kern hat das Beenden nicht bestätigt; Programmende");
                app.exit(0);
            });
            true
        }
    }
}

fn core_config(app: &AppHandle, overlay: Overlay) -> Result<CoreConfig, pdfover_config::ConfigError> {
    let paths = Paths::platform()?;
    let env_dir = pdfium::env_override();
    let exe = std::env::current_exe().ok();
    let resources = app.path().resource_dir().ok();
    let candidates = pdfium::candidates(env_dir, exe.as_deref(), resources.as_deref());
    let pdfium_dir = pdfium::resolve(&candidates, exe.as_deref(), !cfg!(debug_assertions));
    Ok(CoreConfig {
        paths,
        overlay,
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

/// Hält den Schreib-Thread der Logdatei bis zum Programmende (beim Drop wird geleert).
struct LogState(Mutex<Option<pdfover_core_api::logging::LogGuard>>);

/// Logdatei im Log-Ordner der Plattform (rotierend, Plan §7.1); Filter aus `--log-level`, sonst
/// `PDFOVER_LOG`, sonst `info`. In Debug-Builds zusätzlich stderr.
fn init_logging(paths: &Paths, log_level: Option<String>) -> pdfover_core_api::logging::LogGuard {
    let level = log_level
        .or_else(|| std::env::var("PDFOVER_LOG").ok().filter(|l| !l.trim().is_empty()))
        .unwrap_or_else(|| "info".into());
    pdfover_core_api::logging::init(
        paths,
        &pdfover_core_api::logging::LogOptions { level, stderr: cfg!(debug_assertions), file: true },
    )
}

/// Logs nur auf stderr (ohne Konfiguration kein Log-Ordner).
fn init_logging_stderr() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_env("PDFOVER_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(std::io::stderr).try_init();
}

/// Systemsprache für `Lang::system` (unter Windows/macOS-GUI fehlen die POSIX-Variablen).
fn init_language() -> Option<Lang> {
    let lang = sys_locale::get_locale().as_deref().and_then(Lang::from_tag);
    if lang.is_some() {
        Lang::set_system(lang);
    }
    lang
}

/// Dokumente aus „Öffnen mit“ bzw. einer zweiten Instanz: vor `setup` vormerken, danach an die Shell.
fn deliver_documents(app: &AppHandle, docs: Vec<std::path::PathBuf>) {
    let early = app.try_state::<EarlyDocuments>();
    instance::deliver(early.as_deref(), docs, |docs| match app.try_state::<Shell>() {
        Some(shell) => {
            shell.open_files(docs);
            true
        }
        None => false,
    });
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
    smoke::mark_process_start();
    let system_language = init_language();

    // Startargumente wie PDF-Over (Hilfe/Version ohne Fenster).
    let cwd = std::env::current_dir().unwrap_or_default();
    let start_args = args::parse_gui_args(std::env::args_os(), &cwd, &|name| std::env::var_os(name));
    let start_args = match start_args {
        GuiArgs::Help(text) => {
            print!("{text}");
            return;
        }
        GuiArgs::Version(text) => {
            println!("{text}");
            return;
        }
        other => other,
    };

    // vor `build` registriert: `RunEvent::Opened` kann unter macOS vor `setup` eintreffen
    let mut builder = tauri::Builder::default().manage(EarlyDocuments::default());

    let single_instance = cfg!(desktop) && instance::single_instance_allowed();
    #[cfg(desktop)]
    {
        // Muss als erstes Plugin registriert werden (macOS: nur mit eigenem Socket, siehe `instance`).
        if single_instance {
            builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
                let docs = args::second_instance_documents(&argv, std::path::Path::new(&cwd));
                deliver_documents(app, docs);
                focus_main_window(app);
            }));
        }
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
                let response = match app.try_state::<Shell>() {
                    Some(shell) => shell.render(&path, query.as_deref()),
                    None => protocol::error(503, shell::CORE_UNAVAILABLE),
                };
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            core_view,
            core_dispatch,
            app_info,
            open_log_dir,
            reveal_output,
            open_notices
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let (overlay, log_level, args_error) = match start_args {
                GuiArgs::Start { overlay, log_level } => (*overlay, log_level, None),
                GuiArgs::Invalid { code, params } => (Overlay::default(), None, Some((code, params))),
                GuiArgs::Help(_) | GuiArgs::Version(_) => (Overlay::default(), None, None),
            };
            let shell = match core_config(&handle, overlay) {
                Ok(config) => {
                    app.manage(LogState(Mutex::new(Some(init_logging(&config.paths, log_level)))));
                    match args_error {
                        Some((code, params)) => {
                            tracing::warn!(code = %code, "Startargumente ungültig – Kern startet nicht");
                            Shell::start_error(Some(config), &code, params)
                        }
                        None => Shell::new(config),
                    }
                }
                Err(e) => {
                    // ohne Benutzerordner: Fehlerschritt statt Panik beim Start
                    init_logging_stderr();
                    tracing::error!(id = e.message_id(), "Konfiguration nicht ermittelbar");
                    Shell::without_config(&e.to_string())
                }
            };
            tracing::info!(version = %handle.package_info().version, "PDF-Over NG startet");
            if cfg!(desktop) && !single_instance {
                tracing::warn!("Single-Instance-Socket gehört nicht diesem Konto – Start ohne Single-Instance");
            }
            shell.set_system_language(system_language);
            shell.set_hooks(Arc::new(hooks::TauriHooks::new(handle.clone())));
            app.manage(shell);
            let shell = handle.state::<Shell>();
            // vor `setup` eingetroffene Dokumente (macOS-Kaltstart über ein Dokument) gehen mit dem Start
            // an den Kern; ab hier liefert `deliver_documents` direkt an die Shell
            shell.open_files(handle.state::<EarlyDocuments>().close());
            start_core(&handle, &shell);
            smoke::arm_watchdog(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri-Anwendung konnte nicht erstellt werden");

    app.run(|app, event| match event {
        tauri::RunEvent::WindowEvent { event: tauri::WindowEvent::CloseRequested { api, .. }, .. } => {
            // Fenster schließen während eines Dialogs gilt als Abbruch (F-73); der Kern räumt auf, sichert
            // die Einstellungen und meldet `Finished` → `app.exit(code)`.
            if quit_via_core(app) {
                api.prevent_close();
            }
        }
        // Beenden durch den Nutzer (⌘Q, Menü); `Some(code)` stammt von `app.exit` und wird nicht gehalten.
        tauri::RunEvent::ExitRequested { code: None, api, .. } => {
            if quit_via_core(app) {
                api.prevent_exit();
            }
        }
        tauri::RunEvent::Exit => {
            // Logdatei leeren (nicht blockierender Schreiber)
            if let Some(log) = app.try_state::<LogState>() {
                drop(log.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take());
            }
        }
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        tauri::RunEvent::Opened { urls } => {
            deliver_documents(app, files::documents_from_urls(&urls));
            focus_main_window(app);
        }
        _ => {}
    });
}
