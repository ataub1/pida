//! iOS/Android: Registrierung der nativen Klasse und Aufrufe über `PluginHandle::run_mobile_plugin`.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use tauri::plugin::{PluginApi, PluginHandle};
use tauri::{AppHandle, Runtime};

use crate::{Error, Native, PdfoverMobile, Result};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_pdfover_mobile);

/// Registriert `PdfoverMobilePlugin` (Swift bzw. Kotlin).
pub(crate) fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> std::result::Result<PdfoverMobile, Box<dyn std::error::Error>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(crate::ANDROID_PACKAGE, "PdfoverMobilePlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_pdfover_mobile)?;
    Ok(PdfoverMobile::with_native(Arc::new(PluginNative(handle))))
}

/// Native Seite über Tauri.
struct PluginNative<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Native for PluginNative<R> {
    fn call(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin::<serde_json::Value>(method, payload).map_err(Error::from)
    }
}
