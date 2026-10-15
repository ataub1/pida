//! Shell-Funktionen für den Kern ([`pdfover_core_api::ShellHooks`]): Ergebnis mit der
//! Standardanwendung öffnen (F-87), „Signaturdaten anzeigen“ im Systembrowser (F-68) und das
//! Elternfenster für modale Systemdialoge (Windows-Hello/FIDO2 über `webauthn.dll`).
//!
//! Der Kern prüft bereits, was geöffnet werden darf (nur die Ergebnisdatei, nur `https://`-Links von
//! A-Trust); die Shell prüft zusätzlich das Schema, damit kein anderer Pfad über diese Naht einen
//! beliebigen Handler startet. Fehler enthalten keine Pfade (Regel 9; der Kern meldet sie als Hinweis).

use std::path::Path;

use pdfover_backend_api::WindowHandle;
use pdfover_core_api::ShellHooks;
use tauri::{AppHandle, Runtime};

/// Prüft ein Ziel für [`ShellHooks::open_url`]: nur `https://` mit Host.
pub fn allowed_url(url: &str) -> bool {
    match url::Url::parse(url) {
        Ok(u) => u.scheme() == "https" && u.host_str().is_some_and(|h| !h.is_empty()),
        Err(_) => false,
    }
}

/// Shell-Funktionen über die Tauri-Plugins `opener` (Öffnen) und das Hauptfenster.
pub struct TauriHooks<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriHooks<R> {
    /// Hooks für diese App.
    pub fn new(app: AppHandle<R>) -> Self {
        TauriHooks { app }
    }
}

impl<R: Runtime> std::fmt::Debug for TauriHooks<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TauriHooks")
    }
}

impl<R: Runtime> ShellHooks for TauriHooks<R> {
    fn open_path(&self, path: &Path) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        if !path.is_file() {
            return Err("keine Datei".into());
        }
        self.app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        if !allowed_url(url) {
            return Err("nur https".into());
        }
        self.app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }

    fn parent_window(&self) -> Option<WindowHandle> {
        parent_window(&self.app)
    }
}

#[cfg(windows)]
fn parent_window<R: Runtime>(app: &AppHandle<R>) -> Option<WindowHandle> {
    use tauri::Manager;
    let window = app.get_webview_window("main")?;
    let hwnd = window.hwnd().ok()?;
    Some(WindowHandle(hwnd.0 as usize as u64))
}

#[cfg(not(windows))]
fn parent_window<R: Runtime>(_app: &AppHandle<R>) -> Option<WindowHandle> {
    // Nur `webauthn.dll` (Windows) braucht ein Elternfenster; CTAP-HID zeigt keinen Systemdialog.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_urls_with_host_are_opened() {
        assert!(allowed_url("https://www.a-trust.at/ShowSigobj.aspx?sid=1"));
        for bad in ["http://a-trust.at", "file:///etc/passwd", "javascript:alert(1)", "https://", "", "mailto:x@y"] {
            assert!(!allowed_url(bad), "{bad}");
        }
    }
}
