//! Durchstich „PIDA im Browser“ (Konzept Webdienst, Variante E, Etappe 1): der echte Kern als WebAssembly –
//! PDF öffnen, Signaturblock nach Profil planen und zeichnen, PAdES-konform einbetten. Signiert wird hier mit einem
//! Test-Keystore (PKCS#12) als Ersatz für A-Trust; in Variante E liefert die Signatur das A-Trust-Overlay.
//!
//! Keine Dateien, keine Uhr und keine Threads aus `std`: Eingaben kommen als Bytes, die Zeit von der Seite.

use pida_backend_keystore::LoadedKey;
use pida_cades::{RawSigner, SignedAttrsBuilder, SigningTime};
use pida_pades::{PlanOptions, cert_fields, plan_signature, prepare};
use pida_pdf_cos::{Limits, SourcePdf};
use pida_sigblock::{FontProvider, ProfileStore, SigBlockInput};
use time::{OffsetDateTime, UtcOffset};
use wasm_bindgen::prelude::*;

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
    // ohne Renderer: Lage wie PDF-AS ohne Scan (PDFium als WebAssembly folgt im zweiten Schritt)
    let planned = plan_signature(&src, &report, profile, &input, &opts, &fonts, &**store.resources(), None)
        .map_err(|e| err("Planung", e))?;
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
