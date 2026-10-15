//! Desktop-Einstieg von PIDA (die Logik liegt in `pida_app_lib`, damit Mobile dieselbe nutzt).

// Unter Windows im Release kein zusätzliches Konsolenfenster (die Kommandozeile ist `pida-cli`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pida_app_lib::run();
}
