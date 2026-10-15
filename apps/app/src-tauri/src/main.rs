//! Desktop-Einstieg von PDF-Over NG (die Logik liegt in `pdfover_app_lib`, damit Mobile dieselbe nutzt).

// Unter Windows im Release kein zusätzliches Konsolenfenster (die Kommandozeile ist `pdfover-cli`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pdfover_app_lib::run();
}
