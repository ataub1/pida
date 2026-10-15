//! Suche nach der PDFium-Bibliothek (Plan §2.1: dynamisch auf Desktop/Android, xcframework auf iOS).
//!
//! Reihenfolge: Umgebungsvariable `PDFOVER_PDFIUM_DIR` (**nur Debug-Builds**: Entwicklung, Tests),
//! Ordner der Binary, `../Frameworks` im macOS-Bundle, Ressourcenordner (auch `pdfium/` darunter).
//! Wird nichts gefunden, entscheidet der Kern (`CoreConfig::pdfium_dir = None` → neben der
//! Binary/System).
//!
//! Sicherheit: Ein Release-Build lädt keine Bibliothek aus einem per Umgebungsvariable genannten
//! Ordner, solange die mitgelieferte gefunden wird. Sonst könnte jeder, der die Umgebung der App
//! setzen kann, Code mit den Rechten der (signierten) App ausführen – z. B. deren macOS-TCC-Freigaben
//! für Dokumente/Downloads erben (Bibliotheks-Injektion trotz Hardened Runtime).

use std::path::{Path, PathBuf};

/// Dateiname der PDFium-Bibliothek auf dieser Plattform.
pub fn library_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

/// Name der Umgebungsvariable (wie `pdfover_render::ENV_PDFIUM_DIR`).
pub const ENV_PDFIUM_DIR: &str = "PDFOVER_PDFIUM_DIR";

/// Ordner aus [`ENV_PDFIUM_DIR`] – nur in Debug-Builds (siehe Moduldoku), sonst `None`.
pub fn env_override() -> Option<PathBuf> {
    if cfg!(debug_assertions) { std::env::var_os(ENV_PDFIUM_DIR).map(PathBuf::from) } else { None }
}

/// Kandidaten in Suchreihenfolge.
pub fn candidates(env_dir: Option<PathBuf>, exe: Option<&Path>, resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = env_dir.filter(|d| !d.as_os_str().is_empty()) {
        out.push(dir);
    }
    if let Some(exe_dir) = exe.and_then(Path::parent) {
        out.push(exe_dir.to_path_buf());
        out.push(exe_dir.join("../Frameworks"));
    }
    if let Some(res) = resource_dir {
        out.push(res.to_path_buf());
        out.push(res.join("pdfium"));
    }
    out
}

/// Erster Kandidat, der die Bibliothek enthält.
pub fn find(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|dir| dir.join(library_file_name()).is_file()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_order_and_hit() {
        let root = std::env::temp_dir().join(format!("pdfover-app-pdfium-{}", std::process::id()));
        let exe_dir = root.join("MacOS");
        let frameworks = root.join("Frameworks");
        std::fs::create_dir_all(&exe_dir).expect("exe dir");
        std::fs::create_dir_all(&frameworks).expect("frameworks");
        std::fs::write(frameworks.join(library_file_name()), b"").expect("lib");
        let exe = exe_dir.join("pdf-over-ng");
        let list = candidates(Some(PathBuf::new()), Some(&exe), None);
        // leere Umgebungsvariable zählt nicht
        assert_eq!(list[0], exe_dir);
        let hit = find(&list).expect("gefunden");
        assert_eq!(hit, exe_dir.join("../Frameworks"));
        let env = root.join("env");
        let list = candidates(Some(env.clone()), Some(&exe), Some(&root));
        assert_eq!(list.first(), Some(&env));
        assert_eq!(list.last(), Some(&root.join("pdfium")));
        assert!(find(&[root.join("nichts")]).is_none());
    }

    #[test]
    fn env_override_only_in_debug_builds() {
        // Der Test läuft im Debug-Profil (cargo test); im Release-Profil bleibt die Variable wirkungslos.
        let value = env_override();
        if cfg!(debug_assertions) {
            assert_eq!(value, std::env::var_os(ENV_PDFIUM_DIR).map(PathBuf::from));
        } else {
            assert_eq!(value, None);
        }
    }
}
