//! Build-Skript des Mobile-Plugins: bindet unter iOS das Swift-Paket `ios/` und unter Android die
//! Gradle-Bibliothek `android/` ein (tauri-plugin kopiert dafür die Tauri-API nach `<pfad>/.tauri`).
//!
//! Das Plugin hat **keine** Befehle für die Oberfläche (`COMMANDS` leer): die nativen Methoden ruft nur
//! die Rust-Shell auf. Insbesondere der sichere Speicher darf nie aus der WebView erreichbar sein.

const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").ios_path("ios").build();
}
