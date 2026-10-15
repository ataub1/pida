//! Shell-Funktionen für den Kern ([`pdfover_core_api::ShellHooks`]): Ergebnis mit der
//! Standardanwendung öffnen (F-87), „Signaturdaten anzeigen“ im Systembrowser (F-68), das
//! Elternfenster für modale Systemdialoge (Windows-Hello/FIDO2 über `webauthn.dll`) und auf Mobile
//! Teilen/Exportieren über das Plugin `pdfover-mobile` (Share-Sheet, Dokument-Picker bzw. SAF).
//!
//! Der Kern prüft bereits, was geöffnet werden darf (nur die Ergebnisdatei, nur `https://`-Links von
//! A-Trust); die Shell prüft zusätzlich das Schema, damit kein anderer Pfad über diese Naht einen
//! beliebigen Handler startet. Fehler enthalten keine Pfade oder URLs ([`opener_error_kind`]; der Kern
//! loggt sie und meldet einen Hinweis).

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

/// Fehlerangabe ohne Pfad/URL (Regel 9): Der Kern loggt sie. `opener`-Fehler nennen sonst u. a. den
/// gestarteten Befehl samt Dateipfad (`Launcher "open" "/…/Vertrag_signed.pdf" failed …`) bzw. die URL.
pub fn opener_error_kind(e: &tauri_plugin_opener::Error) -> String {
    match e {
        tauri_plugin_opener::Error::Io(io) => format!("opener-io-{:?}", io.kind()),
        tauri_plugin_opener::Error::ForbiddenPath { .. } | tauri_plugin_opener::Error::ForbiddenUrl { .. } => {
            "opener-forbidden".to_owned()
        }
        _ => "opener-failed".to_owned(),
    }
}

impl<R: Runtime> ShellHooks for TauriHooks<R> {
    fn open_path(&self, path: &Path) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        if !path.is_file() {
            return Err("keine Datei".into());
        }
        // Mobile: Anzeige über das Plugin (Android: ACTION_VIEW mit FileProvider-URI – `opener` übergibt
        // file://-Pfade, die Android anderen Apps verweigert). Ohne native Umsetzung (iOS bis M-iOS) wie
        // bisher über `opener`.
        #[cfg(mobile)]
        {
            use tauri_plugin_pdfover_mobile::PdfoverMobileExt;
            match self.app.pdfover_mobile().open_file(path, crate::mobile::PDF_MIME) {
                Ok(()) => return Ok(()),
                Err(e) if cfg!(target_os = "android") => return Err(e.code().to_owned()),
                Err(_) => {}
            }
        }
        self.app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| opener_error_kind(&e))
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        if !allowed_url(url) {
            return Err("nur https".into());
        }
        self.app.opener().open_url(url, None::<&str>).map_err(|e| opener_error_kind(&e))
    }

    fn parent_window(&self) -> Option<WindowHandle> {
        parent_window(&self.app)
    }

    fn share_path(&self, path: &Path) -> Result<(), String> {
        use tauri_plugin_pdfover_mobile::PdfoverMobileExt;
        if !path.is_file() {
            return Err("keine Datei".into());
        }
        self.app.pdfover_mobile().share_file(path, crate::mobile::PDF_MIME).map_err(|e| e.code().to_owned())
    }

    fn export_path(&self, path: &Path, file_name: &str) -> Result<bool, String> {
        use tauri_plugin_pdfover_mobile::PdfoverMobileExt;
        if !path.is_file() {
            return Err("keine Datei".into());
        }
        self.app.pdfover_mobile().export_file(path, file_name, crate::mobile::PDF_MIME).map_err(|e| e.code().to_owned())
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
    fn opener_errors_carry_no_paths() {
        let io = std::io::Error::other("Launcher \"open\" \"/geheim/Vertrag_signed.pdf\" failed");
        let text = opener_error_kind(&tauri_plugin_opener::Error::Io(io));
        assert_eq!(text, "opener-io-Other");
        let forbidden = tauri_plugin_opener::Error::ForbiddenPath { path: "/geheim/a.pdf".into(), with: None };
        assert_eq!(opener_error_kind(&forbidden), "opener-forbidden");
        let unknown = tauri_plugin_opener::Error::UnknownProgramName("/geheim/prog".into());
        assert!(!opener_error_kind(&unknown).contains("geheim"));
    }

    #[test]
    fn only_https_urls_with_host_are_opened() {
        assert!(allowed_url("https://www.a-trust.at/ShowSigobj.aspx?sid=1"));
        for bad in ["http://a-trust.at", "file:///etc/passwd", "javascript:alert(1)", "https://", "", "mailto:x@y"] {
            assert!(!allowed_url(bad), "{bad}");
        }
    }
}
