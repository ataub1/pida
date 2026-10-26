//! Durchstich „PIDA im Browser“ (Konzept Webdienst, Variante E, Etappe 1): der echte Kern als WebAssembly –
//! PDF öffnen, Signaturblock nach Profil planen und zeichnen, PAdES-konform einbetten. Signiert wird hier mit einem
//! Test-Keystore (PKCS#12) als Ersatz für A-Trust; in Variante E liefert die Signatur das A-Trust-Overlay.
//!
//! Keine Dateien, keine Uhr und keine Threads aus `std`: Eingaben kommen als Bytes, die Zeit von der Seite.

use pida_backend_keystore::LoadedKey;
use pida_cades::{RawSigner, SignedAttrsBuilder, SigningTime};
use pida_pades::{PlanOptions, cert_fields, plan_signature, prepare};
use pida_pdf_cos::{Limits, SourcePdf};
use pida_render::{PdfiumBackend, RenderBackend, Renderer};
use pida_sigblock::{FontProvider, ProfileStore, SigBlockInput};
use time::{OffsetDateTime, UtcOffset};
use wasm_bindgen::prelude::*;

thread_local! {
    /// Lage und Hinweise der letzten Planung (`last_plan`), als JSON.
    static LAST_PLAN: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Lage und Hinweise der letzten Signatur (Seite, x/y/Breite/Höhe in pt, Hinweise der Platzierung) als JSON.
#[wasm_bindgen]
pub fn last_plan() -> String {
    LAST_PLAN.with(|l| l.borrow().clone())
}

/// Beim Laden: Panics mit Meldung und Ort in die Browser-Konsole.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

fn err(stage: &str, e: impl std::fmt::Display) -> JsError {
    JsError::new(&format!("{stage}: {e}"))
}

/// Zeit der Seite (`Date.now()` und `getTimezoneOffset()`, in Minuten westlich von UTC) als lokale Zeit.
fn signing_time(now_ms: f64, tz_offset_min: i32) -> Result<OffsetDateTime, JsError> {
    let utc = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000).map_err(|e| err("Zeit", e))?;
    let offset = UtcOffset::from_whole_seconds(-tz_offset_min * 60).map_err(|e| err("Zeitzone", e))?;
    Ok(utc.to_offset(offset))
}

/// PDFium: im Browser bindet die Seite das WebAssembly-Modul vorher (`initialize_pdfium_render`); ohne → `None`.
fn renderer(src: &SourcePdf) -> Option<Box<dyn Renderer>> {
    PdfiumBackend::bind(None).ok()?.open(src.shared_bytes(), None).ok()
}

/// Index der „nie pausieren“-Funktion, die die Seite mit `Module.addFunction(() => 0, 'ii')` in PDFiums Tabelle anlegt.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_pdfium_never_pause(table_index: usize) {
    pida_render::set_wasm_never_pause(table_index);
}

/// PDFium gebunden? Beschreibung oder Fehler (Probe für die Seite).
#[wasm_bindgen]
pub fn pdfium_status() -> String {
    match PdfiumBackend::bind(None) {
        Ok(b) => b.describe(),
        Err(e) => format!("nicht verfügbar: {e}"),
    }
}

/// Vorschau einer Seite als PNG (PDFium), `scale` = Pixel je pt.
#[wasm_bindgen]
pub fn preview(pdf: &[u8], page: usize, scale: f32) -> Result<Vec<u8>, JsError> {
    let src = SourcePdf::open(pdf.to_vec(), &Limits::default()).map_err(|e| err("PDF öffnen", e))?;
    let r = renderer(&src).ok_or_else(|| JsError::new("PDFium nicht gebunden"))?;
    let img = r.render_page(page, scale).map_err(|e| err("Rendern", e))?;
    pida_render::encode_png(&img).map_err(|e| err("PNG", e))
}

/// Seitenzahl und vorhandene Signaturen – Probe, ob das PDF im Browser lesbar ist.
#[wasm_bindgen]
pub fn inspect(pdf: &[u8]) -> Result<String, JsError> {
    let src = SourcePdf::open(pdf.to_vec(), &Limits::default()).map_err(|e| err("PDF öffnen", e))?;
    let report = pida_pades::analyze(&src).map_err(|e| err("Analyse", e))?;
    let pages = src.page_count();
    Ok(serde_json::json!({ "pages": pages, "signed": report.signatures.len() }).to_string())
}

/// Signiert `pdf` sichtbar mit Profil `profile_id` und dem ersten Schlüssel aus `p12`; liefert das signierte PDF.
#[wasm_bindgen]
pub fn sign(
    pdf: &[u8],
    p12: &[u8],
    password: &str,
    profile_id: &str,
    now_ms: f64,
    tz_offset_min: i32,
) -> Result<Vec<u8>, JsError> {
    let time = signing_time(now_ms, tz_offset_min)?;
    let src = SourcePdf::open(pdf.to_vec(), &Limits::default()).map_err(|e| err("PDF öffnen", e))?;
    let report = pida_pades::analyze(&src).map_err(|e| err("Analyse", e))?;
    let key = LoadedKey::open_pkcs12(p12, password, None).map_err(|e| err("Keystore", e))?;
    let store = ProfileStore::builtin().map_err(|e| err("Profile", e))?;
    let profile = store.profile(profile_id).map_err(|e| err("Profil", e))?;
    let fonts = FontProvider::builtin();
    let opts = PlanOptions::new(time);
    let input = SigBlockInput::new(cert_fields(&key.chain().signer), time);
    // mit PDFium (falls gebunden): automatische Platzierung wie in der App, sonst Lage wie PDF-AS ohne Scan
    let r = renderer(&src);
    let planned = plan_signature(&src, &report, profile, &input, &opts, &fonts, &**store.resources(), r.as_deref())
        .map_err(|e| err("Planung", e))?;
    LAST_PLAN.with(|l| {
        *l.borrow_mut() = serde_json::json!({
            "pdfium": r.is_some(),
            "position": planned.position.as_ref().map(|p| serde_json::json!({
                "page": p.page_index, "new_page": p.new_page, "x": p.x, "y": p.y, "width": p.width, "height": p.height,
            })),
            "notices": planned.notices.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>(),
        })
        .to_string();
    });
    let prepared = prepare(&src, &planned.plan, Some(key.chain())).map_err(|e| err("Vorbereiten", e))?;
    let alg = key.algorithm().digest();
    let digest = prepared.digest(alg);
    let attrs = SignedAttrsBuilder::new(
        prepared.cms_profile(),
        &digest,
        &key.chain().signer,
        SigningTime(prepared.signing_time()),
    )
    .build()
    .map_err(|e| err("Signaturattribute", e))?;
    let raw = key.sign_digest(&attrs.digest(alg)).map_err(|e| err("Signieren", e))?;
    let signed = prepared.finish_raw(&attrs, &raw, key.chain()).map_err(|e| err("Einbetten", e))?;
    Ok(signed.bytes())
}
