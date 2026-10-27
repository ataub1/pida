//! Anbindung an JavaScript (nur im Browser).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pida_backend_api::Platform;
use pida_config::{BackendChoice, MemorySecretStore, Overlay, Paths, Policy};
use pida_core_api::{Core, CoreConfig, CoreOptions, ShellHooks, UiEvent};
use pida_i18n::Lang;
use wasm_bindgen::prelude::*;

use crate::relay::WebSlRelay;
use crate::{OUTPUT_DIR, ROOT, SESSION_SETTINGS, UPLOAD_DIR};

thread_local! {
    /// Shell-Dienste der Seite: `(art, wert)` mit art = `open` (Pfad), `reveal` (Pfad), `url` (Adresse).
    static SHELL: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

pub use crate::relay::sl_resolve;

fn call_shell(kind: &str, value: &str) -> Result<(), String> {
    SHELL.with(|s| match s.borrow().as_ref() {
        Some(f) => f
            .call2(&JsValue::NULL, &JsValue::from_str(kind), &JsValue::from_str(value))
            .map(|_| ())
            .map_err(|_| "ng-ui-open-failed".to_owned()),
        None => Err("ng-ui-open-failed".to_owned()),
    })
}

/// Shell-Dienste des Kerns: die Seite lädt Ergebnisse herunter bzw. öffnet Links in einem neuen Tab.
struct WebHooks;

impl ShellHooks for WebHooks {
    fn open_path(&self, path: &Path) -> Result<(), String> {
        call_shell("open", &path.to_string_lossy())
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        call_shell("url", url)
    }
    fn reveal_path(&self, path: &Path) -> Result<(), String> {
        call_shell("reveal", &path.to_string_lossy())
    }
}

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Beim Laden: Panics mit Meldung in die Browser-Konsole.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Index der „nie pausieren“-Funktion in PDFiums Funktionstabelle (`Module.addFunction(() => 0, 'ii')`).
#[wasm_bindgen]
pub fn set_pdfium_never_pause(table_index: usize) {
    pida_render::set_wasm_never_pause(table_index);
}

/// Datei ins Dateisystem im Speicher legen (hochgeladenes PDF, Bildmarke, im Testbuild Keystore); liefert den Pfad
/// unter [`UPLOAD_DIR`]`/<n>/<name>` (eigener Ordner je Datei, der Name bleibt erhalten).
#[wasm_bindgen]
pub fn put_file(name: &str, bytes: &[u8]) -> Result<String, JsError> {
    thread_local! {
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    }
    let n = NEXT.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    // nur der letzte Teil des Namens (kein Pfad aus der Seite)
    let file = Path::new(name).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "datei".into());
    let path = PathBuf::from(UPLOAD_DIR).join(n.to_string()).join(file);
    pida_fs::write(&path, bytes).map_err(js_err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Datei aus dem Dateisystem im Speicher lesen (signiertes Ergebnis); nur unter [`ROOT`].
#[wasm_bindgen]
pub fn read_file(path: &str) -> Result<Vec<u8>, JsError> {
    if !Path::new(path).starts_with(ROOT) {
        return Err(JsError::new("Pfad außerhalb von /pida"));
    }
    pida_fs::read(path).map_err(js_err)
}

/// Der Kern im Browser.
#[derive(Debug)]
#[wasm_bindgen]
pub struct WebCore {
    core: Core,
}

#[wasm_bindgen]
impl WebCore {
    /// Startet den Kern. `language` = `de`/`en` (Browser-Sprache), `on_view(json)` bei jeder Änderung des Zustands,
    /// `shell(art, wert)` für Ergebnis öffnen/zeigen und Links, `sl(art, id, xml)` für die SL-Befehle an A-Trust
    /// (Overlay und Rückkanal, Antwort über [`sl_resolve`]); `test_keystore` = verstecktes Testmerkmal Keystore.
    #[wasm_bindgen(constructor)]
    pub fn new(
        language: &str,
        version: &str,
        on_view: js_sys::Function,
        shell: js_sys::Function,
        sl: js_sys::Function,
        test_keystore: bool,
    ) -> Result<WebCore, JsError> {
        SHELL.with(|s| *s.borrow_mut() = Some(shell));
        crate::relay::install(sl);
        let lang = if language.starts_with("en") { Lang::En } else { Lang::De };
        let paths = Paths::under(Path::new(ROOT));
        pida_fs::create_dir_all(OUTPUT_DIR).map_err(js_err)?;
        let settings = paths.settings_file();
        if let Some(dir) = settings.parent() {
            pida_fs::create_dir_all(dir).map_err(js_err)?;
        }
        pida_fs::write(&settings, SESSION_SETTINGS).map_err(js_err)?;
        // Signiermittel: ID Austria, als Testmerkmal zusätzlich der Keystore; Update-Prüfung und „Zuletzt signiert“ aus
        let mut allowed = vec![BackendChoice::IdAustria];
        if test_keystore {
            allowed.push(BackendChoice::Keystore);
        }
        let policy = Policy {
            allowed_backends: Some(allowed),
            update_check: Some(false),
            recent_documents: Some(false),
            ..Policy::default()
        };
        let config = CoreConfig {
            paths,
            overlay: Overlay::default(),
            // vorerst wie ein Desktop (Arbeitsbereich, Stapel, Keystore); eine eigene Plattform „Web“ folgt
            platform: Platform::Linux,
            pdfium_dir: None,
            headless: false,
            fido2: None,
            app_version: version.to_owned(),
        };
        let options = CoreOptions {
            secret_store: Some(Arc::new(MemorySecretStore::default())),
            use_pdfium: true,
            shell: Some(Arc::new(WebHooks)),
            system_language: Some(lang),
            init_logging: false,
            detect_legacy_dir: false,
            load_system_policy: false,
            probe_local_bku: false,
            default_output_dir: Some(PathBuf::from(OUTPUT_DIR)),
            workspace_ui: Some(true),
            batch_overview: Some(true),
            update_transport: None,
            policy: Some(policy),
            id_austria_backend: Some(Arc::new(pida_backend_atrust_web::AtrustWebBackend::new(Arc::new(WebSlRelay)))),
            ..CoreOptions::default()
        };
        let core = Core::new_with(config, options).map_err(js_err)?;
        let mut rx = core.subscribe();
        wasm_bindgen_futures::spawn_local(async move {
            while rx.changed().await.is_ok() {
                let json = serde_json::to_string(&*rx.borrow_and_update()).unwrap_or_default();
                let _ = on_view.call1(&JsValue::NULL, &JsValue::from_str(&json));
            }
        });
        Ok(WebCore { core })
    }

    /// Aktueller Zustand (JSON, `ViewState`).
    pub fn view(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.core.view()).map_err(js_err)
    }

    /// Ereignis der Oberfläche (JSON, `UiEvent`).
    pub fn dispatch(&self, event: &str) -> Result<(), JsError> {
        let event: UiEvent = serde_json::from_str(event).map_err(|_| JsError::new("ng-ui-event-invalid"))?;
        self.core.dispatch(event);
        Ok(())
    }

    /// PNG einer Seitenkachel (`pida-render://page/<doc>/<page>?scale=`).
    pub fn page_tile(&self, doc: u64, page: usize, scale: f32) -> Result<Vec<u8>, JsError> {
        self.core.page_tile(pida_workflow::DocId(doc), page, scale).map_err(js_err)
    }

    /// PNG der Blockvorschau.
    pub fn block_preview(&self, scale: f32) -> Result<Vec<u8>, JsError> {
        self.core.block_preview(scale).map_err(js_err)
    }

    /// PNG-Miniatur eines Stapeleintrags.
    pub fn thumbnail(&self, doc: u64) -> Result<Vec<u8>, JsError> {
        self.core.thumbnail(pida_workflow::DocId(doc)).map_err(js_err)
    }
}

/// EUPL-1.2-Text (Über PIDA › Lizenzen).
#[wasm_bindgen]
pub fn license_eupl() -> String {
    include_str!("../../../LICENSE").to_owned()
}

/// Ausgabeordner (Ergebnisse; ein Stapel wird von dort als ZIP geladen).
#[wasm_bindgen]
pub fn output_dir() -> String {
    OUTPUT_DIR.to_owned()
}

/// Version von PIDA (Workspace-Version).
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
