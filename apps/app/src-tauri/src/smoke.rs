//! Startprüfung der gebauten App (`PDFOVER_SMOKE=1`), z. B. in der CI oder bei der Abnahme
//! (docs/integration/app-abnahme.md): belegt, dass **Kern und WebView** hochfahren, ohne dass jemand
//! klickt.
//!
//! Ablauf: Die Oberfläche fragt nach dem Laden den ViewState ab (Command `core_view`, nur möglich, wenn
//! WebView, JavaScript-Bundle und IPC laufen). Danach wartet die Shell, bis der Kern einen Schritt
//! außer `starting` meldet, gibt eine Zeile
//!
//! ```text
//! pdfover-smoke: ok step=open pdfium=true webview_ms=412 core_ms=415
//! ```
//!
//! auf stdout aus und beendet die App mit Exit-Code 0. Kommt binnen [`WATCHDOG`] keine Abfrage der
//! Oberfläche bzw. kein bereiter Kern, endet sie mit Exit-Code 3 (`pdfover-smoke: timeout …`).
//!
//! Die Variable kann die App nur beenden; sie liest oder schreibt nichts zusätzlich. Für die
//! Startprüfung `HOME` (bzw. `APPDATA`) auf einen leeren Ordner setzen, damit keine Nutzerdaten
//! entstehen.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pdfover_core_api::StepView;
use tauri::{AppHandle, Manager};

use crate::shell::Shell;

/// Umgebungsvariable der Startprüfung.
pub const ENV_SMOKE: &str = "PDFOVER_SMOKE";
/// Höchstdauer bis zur ersten Abfrage der Oberfläche und einem bereiten Kern.
pub const WATCHDOG: Duration = Duration::from_secs(60);
/// Exit-Code bei Zeitüberschreitung.
pub const EXIT_TIMEOUT: i32 = 3;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static UI_SEEN: AtomicBool = AtomicBool::new(false);

/// Startprüfung aktiv?
pub fn enabled() -> bool {
    std::env::var_os(ENV_SMOKE).is_some_and(|v| v == "1")
}

/// Zeitpunkt des Prozessstarts merken (erste Zeile von `run`).
pub fn mark_process_start() {
    let _ = PROCESS_START.set(Instant::now());
}

fn elapsed_ms() -> u128 {
    PROCESS_START.get().map_or(0, |t| t.elapsed().as_millis())
}

/// Wartet nach der ersten Abfrage der Oberfläche auf einen bereiten Kern und beendet dann die App.
pub fn ui_ready(app: &AppHandle, shell: &Shell) {
    if !enabled() || UI_SEEN.swap(true, Ordering::SeqCst) {
        return;
    }
    let webview_ms = elapsed_ms();
    let app = app.clone();
    let core = shell.core();
    let pdfium = core.as_ref().is_some_and(|c| c.has_renderer());
    tauri::async_runtime::spawn(async move {
        let step = match core {
            Some(core) => {
                let mut rx = core.subscribe();
                loop {
                    let step = rx.borrow_and_update().step.clone();
                    if !matches!(step, StepView::Starting) {
                        break step_name(&step);
                    }
                    if rx.changed().await.is_err() {
                        break "stopped".to_owned();
                    }
                }
            }
            // ohne Kern (Fehlerschritt): die Oberfläche läuft trotzdem – Ergebnis „error“
            None => app.try_state::<Shell>().map_or_else(|| "error".to_owned(), |s| step_name(&s.view().step)),
        };
        println!("pdfover-smoke: ok step={step} pdfium={pdfium} webview_ms={webview_ms} core_ms={}", elapsed_ms());
        app.exit(0);
    });
}

/// Beendet die App mit [`EXIT_TIMEOUT`], wenn die Oberfläche binnen [`WATCHDOG`] nichts abfragt.
pub fn arm_watchdog(app: &AppHandle) {
    if !enabled() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(WATCHDOG);
        println!("pdfover-smoke: timeout ui_seen={} after_ms={}", UI_SEEN.load(Ordering::SeqCst), elapsed_ms());
        app.exit(EXIT_TIMEOUT);
    });
}

/// Name des Schritts wie im JSON (`open`, `error` …).
pub fn step_name(step: &StepView) -> String {
    serde_json::to_value(step)
        .ok()
        .and_then(|v| v.get("step").and_then(|s| s.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_names_match_the_bridge() {
        assert_eq!(step_name(&StepView::Starting), "starting");
        assert_eq!(step_name(&StepView::Open { queue: Vec::new() }), "open");
        assert_eq!(step_name(&StepView::Finished { exit_code: 0 }), "finished");
    }
}
