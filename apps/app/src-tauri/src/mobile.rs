//! Mobile-Integration der Shell (iOS/Android; Plan §5.2/§5.3, docs/integration/mobile.md).
//!
//! Plattformneutral und auf dem Desktop testbar; die Tauri-Ereignisse bindet `lib.rs` unter
//! `cfg(mobile)` an:
//!
//! - **Lebenszyklus:** `WindowEvent::Suspended`/`Resumed` (iOS `applicationWillResignActive` bzw.
//!   `applicationWillEnterForeground`, Android `onPause`/`onResume`) → [`lifecycle_event`] →
//!   `UiEvent::Backgrounded`/`Foregrounded`. Ein App-Wechsel ist **kein Abbruch**; Fenster schließen bzw.
//!   Beenden geht auf Mobile nie als `UiEvent::Quit` an den Kern (das System verwaltet den Prozess).
//! - **Dokument-Eingang:** `RunEvent::Opened` liefert URLs, keine Pfade. [`ingest_urls`] liest `file:`-URLs
//!   (iOS „Öffnen in“ legt Kopien in `Documents/Inbox` ab) und kopiert die **Bytes** in den Eingangsordner
//!   der Sandbox; alles andere (Android `content://`, Security-Scoped URLs) löst die native Seite über
//!   `take_pending_open_documents` auf, die auch Intents `VIEW`/`SEND` vormerkt.
//! - **Sicherer Speicher:** [`PluginSecureStorage`] (`secure_store_*` des Plugins, mit Frist) unter
//!   `pdfover_config::ShellSecretStore`; ohne native Umsetzung nur RAM.
//! - **Pfade:** App-Sandbox (`Paths::sandbox`), Ausgabe in die App-Dokumente.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use pdfover_config::{SecretStore, SecureStorage, ShellSecretStore};
use pdfover_core_api::UiEvent;
use tauri_plugin_pdfover_mobile::{Error as MobileError, PdfoverMobile};
use zeroize::Zeroizing;

/// Eingangsordner für Dokumente aus „Öffnen in“/Share/Intents (im Cache; der Kern kopiert ein
/// Dokument für „Signatur fortsetzen“ selbst in die Sandbox).
pub const INBOX_DIR: &str = "inbox";
/// Ordner für Dateien aus der Dateiauswahl (PDF, PKCS#12) im Cache.
pub const IMPORT_DIR: &str = "import";
/// Unterordner der App-Dokumente für signierte Dokumente.
pub const OUTPUT_DIR: &str = "PDF-Over NG";
/// Frist für Aufrufe des sicheren Speichers (die native Seite antwortet sonst sofort).
pub const SECURE_STORE_TIMEOUT: Duration = Duration::from_secs(5);
/// Höchstgröße eines eingehenden Dokuments (Schutz vor Speicherüberlauf; Plan §7.7).
pub const MAX_DOCUMENT_BYTES: u64 = 512 * 1024 * 1024;
/// MIME-Typ der Ausgabe.
pub const PDF_MIME: &str = "application/pdf";

/// Lebenszyklus-Ereignis an den Kern (`suspended` = Hintergrund).
pub fn lifecycle_event(suspended: bool) -> UiEvent {
    if suspended { UiEvent::Backgrounded } else { UiEvent::Foregrounded }
}

/// Ergebnis von [`ingest_urls`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Ingested {
    /// Kopien im Eingangsordner
    pub files: Vec<PathBuf>,
    /// URLs, die nur die native Seite lesen kann (`content://`, Security-Scoped)
    pub unresolved: Vec<String>,
}

/// Dateiname ohne Pfadanteile und Steuerzeichen; leer → `Dokument.pdf`.
pub fn safe_name(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '\0') { '_' } else { c }).collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() { "Dokument.pdf".to_owned() } else { trimmed.to_owned() }
}

/// Freier Zielpfad `dir/<n>/<name>` (eigener Unterordner je Dokument, damit der Dateiname – und damit der
/// Ausgabename `<name>_signed.pdf` – unverändert bleibt).
pub fn unique_target(dir: &Path, name: &str) -> PathBuf {
    let name = safe_name(name);
    let mut n = 1u32;
    loop {
        let candidate = dir.join(n.to_string()).join(&name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Liest die **Bytes** eingehender Dokumente und legt Kopien in `inbox` ab (nur `file:`-URLs mit
/// regulärer Datei bis [`MAX_DOCUMENT_BYTES`]); andere bzw. nicht lesbare URLs gehen an die native Seite.
pub fn ingest_urls(urls: &[url::Url], inbox: &Path) -> Ingested {
    let mut out = Ingested::default();
    for url in urls {
        let readable = (url.scheme() == "file")
            .then(|| url.to_file_path().ok())
            .flatten()
            .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_DOCUMENT_BYTES));
        let copied = readable.and_then(|path| {
            let bytes = std::fs::read(&path).ok()?;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let target = unique_target(inbox, &name);
            std::fs::create_dir_all(target.parent()?).ok()?;
            std::fs::write(&target, bytes).ok()?;
            Some(target)
        });
        match copied {
            Some(target) => out.files.push(target),
            // Regel 9: keine URL im Log
            None => out.unresolved.push(url.as_str().to_owned()),
        }
    }
    out
}

/// Sicherer Speicher über das Plugin (`secure_store_get/set/delete`). Jeder Aufruf läuft in einem eigenen
/// Thread mit Frist [`SECURE_STORE_TIMEOUT`]: Die native Seite führt Methoden auf dem Hauptthread aus;
/// würde der Kern ihn blockierend fragen, während der Hauptthread selbst wartet, verklemmte die App.
#[derive(Clone)]
pub struct PluginSecureStorage {
    mobile: PdfoverMobile,
    timeout: Duration,
}

impl std::fmt::Debug for PluginSecureStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PluginSecureStorage")
    }
}

/// Schlüssel der Verfügbarkeitsprüfung (kein Secret).
const PROBE_KEY: &str = "PDF-Over NG/.probe";

impl PluginSecureStorage {
    /// Speicher über `mobile`.
    pub fn new(mobile: PdfoverMobile) -> Self {
        Self { mobile, timeout: SECURE_STORE_TIMEOUT }
    }

    fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&PdfoverMobile) -> Result<T, MobileError> + Send + 'static,
    ) -> Result<T, MobileError> {
        let (tx, rx) = mpsc::channel();
        let mobile = self.mobile.clone();
        std::thread::Builder::new()
            .name("pdfover-secure-store".into())
            .spawn(move || {
                let _ = tx.send(f(&mobile));
            })
            .map_err(|_| MobileError::Native("thread".into()))?;
        rx.recv_timeout(self.timeout).unwrap_or(Err(MobileError::Timeout))
    }
}

impl SecureStorage for PluginSecureStorage {
    fn get(&self, key: &str) -> Result<Option<Zeroizing<String>>, String> {
        let key = key.to_owned();
        self.call(move |m| m.secure_store_get(&key)).map(|v| v.map(Zeroizing::new)).map_err(|e| e.code().to_owned())
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let (key, value) = (key.to_owned(), Zeroizing::new(value.to_owned()));
        self.call(move |m| m.secure_store_set(&key, &value)).map_err(|e| e.code().to_owned())
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        let key = key.to_owned();
        self.call(move |m| m.secure_store_delete(&key)).map_err(|e| e.code().to_owned())
    }
    fn is_available(&self) -> bool {
        match self.call(|m| m.secure_store_get(PROBE_KEY)) {
            Ok(_) => true,
            Err(e) => {
                tracing::info!(code = e.code(), "sicherer Speicher des Plugins nicht verfügbar");
                false
            }
        }
    }
}

/// `SecretStore` für den Kern: Keychain bzw. Android Keystore über das Plugin, sonst RAM.
pub fn secret_store(mobile: PdfoverMobile) -> Arc<dyn SecretStore> {
    Arc::new(ShellSecretStore::new(Arc::new(PluginSecureStorage::new(mobile))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tauri_plugin_pdfover_mobile::Native;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pdfover-app-mobile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lifecycle_maps_to_background_and_foreground() {
        assert!(matches!(lifecycle_event(true), UiEvent::Backgrounded));
        assert!(matches!(lifecycle_event(false), UiEvent::Foregrounded));
    }

    #[test]
    fn opened_urls_are_read_as_bytes_into_the_inbox() {
        let root = tmp("ingest");
        let src = root.join("Documents/Inbox");
        std::fs::create_dir_all(&src).unwrap();
        let a = src.join("Vertrag 1.pdf");
        std::fs::write(&a, b"%PDF-1.7 a").unwrap();
        let inbox = root.join("cache/inbox");
        let urls = vec![
            url::Url::from_file_path(&a).unwrap(),
            url::Url::from_file_path(&a).unwrap(),
            url::Url::parse("content://com.android.providers.downloads.documents/document/42").unwrap(),
            url::Url::parse("file:///gibt/es/nicht.pdf").unwrap(),
        ];
        let got = ingest_urls(&urls, &inbox);
        assert_eq!(got.files.len(), 2, "jede Übergabe eine eigene Kopie");
        assert_eq!(got.files[0], inbox.join("1/Vertrag 1.pdf"));
        assert_eq!(got.files[1], inbox.join("2/Vertrag 1.pdf"));
        assert_eq!(std::fs::read(&got.files[0]).unwrap(), b"%PDF-1.7 a");
        assert_eq!(got.unresolved.len(), 2, "content:// und Unlesbares an die native Seite");
        assert!(got.unresolved[0].starts_with("content://"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn names_never_leave_the_folder() {
        assert_eq!(safe_name("../x.pdf"), "_x.pdf");
        assert_eq!(safe_name(""), "Dokument.pdf");
        let root = tmp("names");
        assert_eq!(unique_target(&root, "a/b.pdf"), root.join("1/a_b.pdf"));
        let _ = std::fs::remove_dir_all(root);
    }

    struct FakeNative {
        map: Mutex<std::collections::HashMap<String, String>>,
        delay: Duration,
        not_implemented: bool,
    }

    impl Native for FakeNative {
        fn call(&self, method: &str, p: serde_json::Value) -> Result<serde_json::Value, MobileError> {
            std::thread::sleep(self.delay);
            if self.not_implemented {
                return Err(MobileError::NotImplemented);
            }
            let key = p["key"].as_str().unwrap_or_default().to_owned();
            let mut map = self.map.lock().unwrap();
            Ok(match method {
                "secureStoreGet" => serde_json::json!({ "value": map.get(&key) }),
                "secureStoreSet" => {
                    map.insert(key, p["value"].as_str().unwrap_or_default().to_owned());
                    serde_json::Value::Null
                }
                _ => {
                    map.remove(&key);
                    serde_json::Value::Null
                }
            })
        }
    }

    fn storage(delay: Duration, not_implemented: bool) -> PluginSecureStorage {
        let native = Arc::new(FakeNative { map: Mutex::new(Default::default()), delay, not_implemented });
        PluginSecureStorage { mobile: PdfoverMobile::with_native(native), timeout: Duration::from_millis(300) }
    }

    #[test]
    fn plugin_storage_roundtrip_and_availability() {
        let s = storage(Duration::ZERO, false);
        assert!(s.is_available());
        s.set("PDF-Over NG/k", "test").unwrap();
        assert_eq!(s.get("PDF-Over NG/k").unwrap().as_deref().map(String::as_str), Some("test"));
        s.delete("PDF-Over NG/k").unwrap();
        assert_eq!(s.get("PDF-Over NG/k").unwrap(), None);
    }

    #[test]
    fn empty_native_shell_means_ram_only() {
        let s = storage(Duration::ZERO, true);
        assert!(!s.is_available());
        assert_eq!(s.set("k", "x").unwrap_err(), "ng-mobile-not-implemented");
        let store = ShellSecretStore::new(Arc::new(s));
        assert!(!store.is_persistent());
    }

    #[test]
    fn hanging_native_side_times_out() {
        let s = storage(Duration::from_secs(2), false);
        assert_eq!(s.get("k").unwrap_err(), "ng-mobile-timeout");
        assert!(!s.is_available());
    }
}
