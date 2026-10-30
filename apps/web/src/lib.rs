//! Web-Shell: der PIDA-Kern als WebAssembly im Browser (Konzept Webdienst, Variante E, Etappe 1).
//!
//! Gegenstück zur Tauri-Shell (`apps/app/src-tauri`) für die Web-Brücke der Oberfläche (`apps/app/src/core/web.ts`):
//! - **Kern:** `WebCore` (nur im Browser-Build) startet `pida_core_api::Core` mit Ordnern im Dateisystem im
//!   Speicher ([`ROOT`], `pida-fs`), ohne eigene Laufzeit (Aufgaben im Ereignis-Takt der Seite), mit PDFium als WebAssembly-Modul der Seite.
//! - **Zustand:** `view()` und `dispatch()` als JSON (dieselbe serde-Form wie die Tauri-Commands `core_view` und
//!   `core_dispatch`), Änderungen über den Rückruf aus dem Konstruktor.
//! - **Bilder:** Kacheln, Blockvorschau und Miniaturen als PNG (wie das Protokoll `pida-render://`).
//! - **Dateien:** `put_file` legt hochgeladene PDFs (und im Testbuild Keystores) ab, `read_file` holt das
//!   signierte Ergebnis.
//! - **Shell-Dienste** (Ergebnis öffnen, im Ordner zeigen, Links): an einen JavaScript-Rückruf (Download, neuer Tab).
//!
//! **Nichts wird gespeichert:** Jede Sitzung beginnt mit denselben Einstellungen ([`SESSION_SETTINGS`]: keine
//! Ersteinrichtung, Ergebnisse nach [`OUTPUT_DIR`], ohne „Zuletzt signiert“ und Update-Prüfung); Änderungen gelten bis
//! zum Schließen der Seite. Signiermittel legt eine Richtlinie fest: ID Austria über das A-Trust-Overlay
//! ([`pida_backend_atrust_web`]; Overlay und Rückkanal stellt die Seite), keine Chipkarte, kein Netz im Kern.
//! **Verstecktes Testmerkmal:** Mit `?test-keystore` in der Adresse erlaubt die Richtlinie zusätzlich den Keystore
//! (internes Testen ohne A-Trust); die Oberfläche bietet das nirgends an.

/// Wurzel aller Ordner im Dateisystem im Speicher (Einstellungen unter `config`, Ergebnisse unter `Ausgabe`).
pub const ROOT: &str = "/pida";
/// Ordner für Ergebnisse (Standard-Ausgabeordner).
pub const OUTPUT_DIR: &str = "/pida/Ausgabe";
/// Ordner für hochgeladene Dateien (nur bis zum Schließen der Seite).
pub const UPLOAD_DIR: &str = "/pida/Hochgeladen";

/// Einstellungen zu Beginn jeder Sitzung (`config/settings.toml`; fehlende Werte = Standard).
pub const SESSION_SETTINGS: &str = r#"schema_version = 1

[general]
first_run_completed = true
recent_documents = false
update_check = false

[output]
folder = "/pida/Ausgabe"

# QR-Code-Platzhalter immer suchen (ADR 0004 Nachtrag 24): Dokumente, die mit einem Platzhalter vorbereitet sind
# (PIDA, PDF-AS-WAI), bekommen die Signatur ohne weiteren Schritt an die richtige Stelle
[placeholder]
search = true
kind = "marker"

[id_austria]
remember = false

[keystore]
password_storage = "memory"
storage_chosen = true
"#;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod relay;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod web;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use web::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Sitzungseinstellungen sind gültig und suchen QR-Code-Platzhalter.
    #[test]
    fn session_settings_search_qr_placeholders() {
        let s = pida_config::Settings::from_toml(SESSION_SETTINGS).expect("gültige Einstellungen");
        assert!(s.placeholder.search);
        assert_eq!(s.placeholder.kind, pida_config::PlaceholderKind::Marker);
        assert_eq!(s.output.folder.as_deref(), Some(std::path::Path::new(OUTPUT_DIR)));
        assert!(!s.general.recent_documents && !s.general.update_check);
    }
}
