//! Suche nach der PDFium-Bibliothek (Plan §2.1: dynamisch auf Desktop/Android, xcframework auf iOS).
//!
//! Mobile ([`mobile_dir`]): unter iOS `Frameworks` im App-Bundle (`pdfium.framework/pdfium` aus
//! `PDFium.xcframework`, `tools/fetch-pdfium.sh --xcframework`); unter Android entscheidet der Kern
//! (`None` → `dlopen("libpdfium.so")` aus `nativeLibraryDir`, gefüllt aus `jniLibs/<abi>`,
//! `tools/fetch-pdfium.sh --jnilibs`).
//!
//! Reihenfolge: Umgebungsvariable `PIDA_PDFIUM_DIR` (**nur Debug-Builds**: Entwicklung, Tests),
//! Ordner der Binary, `../Frameworks` im macOS-Bundle, Ressourcenordner (auch `pdfium/` darunter).
//! Wird nichts gefunden ([`resolve`]), entscheidet in Debug-Builds der Kern (`CoreConfig::pdfium_dir =
//! None` → `PIDA_PDFIUM_DIR`, neben der Binary, System); ein Release-Build nennt dann den Ordner der
//! Binary, sodass der Kern ohne mitgelieferte Bibliothek ohne Vorschau startet (automatische Position).
//!
//! Sicherheit: Ein Release-Build lädt nie eine Bibliothek aus einem per Umgebungsvariable genannten
//! Ordner oder aus Systempfaden (`DYLD_*`, `PATH`, Homebrew). Sonst könnte jeder, der die Umgebung der
//! App setzen kann, Code mit den Rechten der (signierten) App ausführen – z. B. deren macOS-TCC-Freigaben
//! für Dokumente/Downloads erben (Bibliotheks-Injektion trotz Hardened Runtime).

use std::path::{Path, PathBuf};

/// Dateiname der PDFium-Bibliothek auf dieser Plattform (wie `pida_render::library_file_name`).
pub fn library_file_name() -> &'static str {
    pida_render::library_file_name()
}

/// Name der Umgebungsvariable (wie `pida_render::ENV_PDFIUM_DIR`).
pub const ENV_PDFIUM_DIR: &str = "PIDA_PDFIUM_DIR";

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

/// Erster Kandidat, der die Bibliothek enthält (unter iOS auch als `pdfium.framework/pdfium`).
pub fn find(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|dir| pida_render::library_paths_in(dir).iter().any(|p| p.is_file())).cloned()
}

/// Mobile: Ordner für `CoreConfig::pdfium_dir`. iOS: `Frameworks` des App-Bundles neben der Binary
/// (`<App>.app/Frameworks`; ohne Treffer der Ordner selbst – dann ohne Vorschau, nie eine fremde
/// Bibliothek). Android: `None` – PDFium kommt aus `nativeLibraryDir` der App über den Linker
/// (`dlopen("libpdfium.so")`).
pub fn mobile_dir(exe: Option<&Path>, android: bool) -> Option<PathBuf> {
    if android {
        return None;
    }
    let exe_dir = exe.and_then(Path::parent)?;
    let frameworks = exe_dir.join("Frameworks");
    find(&[frameworks.clone(), exe_dir.to_path_buf()]).or(Some(frameworks))
}

/// Ordner für `CoreConfig::pdfium_dir`: der erste Kandidat mit der Bibliothek; ohne Treffer in einem
/// Release-Build (`release`) der Ordner der Binary – `pida-render` sucht mit einem genannten Ordner
/// nur dort, nie in `PIDA_PDFIUM_DIR` oder Systempfaden (siehe Moduldoku).
pub fn resolve(candidates: &[PathBuf], exe: Option<&Path>, release: bool) -> Option<PathBuf> {
    find(candidates).or_else(|| if release { exe.and_then(Path::parent).map(Path::to_path_buf) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_order_and_hit() {
        let root = std::env::temp_dir().join(format!("pida-app-pdfium-{}", std::process::id()));
        let exe_dir = root.join("MacOS");
        let frameworks = root.join("Frameworks");
        std::fs::create_dir_all(&exe_dir).expect("exe dir");
        std::fs::create_dir_all(&frameworks).expect("frameworks");
        std::fs::write(frameworks.join(library_file_name()), b"").expect("lib");
        let exe = exe_dir.join("pida");
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
    fn release_builds_never_fall_back_to_search_paths() {
        let root = std::env::temp_dir().join(format!("pida-app-pdfium-resolve-{}", std::process::id()));
        let exe = root.join("MacOS").join("pida");
        let none = [root.join("nichts")];
        // Debug: ohne Treffer entscheidet der Kern (Umgebung, Systempfad)
        assert_eq!(resolve(&none, Some(&exe), false), None);
        // Release: nur der Ordner der Binary (dort fehlt sie → keine Vorschau, keine fremde Bibliothek)
        assert_eq!(resolve(&none, Some(&exe), true), Some(root.join("MacOS")));
        // Treffer gehen immer vor
        let lib_dir = root.join("Frameworks");
        std::fs::create_dir_all(&lib_dir).expect("Ordner");
        std::fs::write(lib_dir.join(library_file_name()), b"").expect("lib");
        assert_eq!(resolve(std::slice::from_ref(&lib_dir), Some(&exe), true), Some(lib_dir));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mobile_resolution() {
        // Android: der Kern bindet über den Linker (jniLibs → nativeLibraryDir)
        assert_eq!(mobile_dir(Some(Path::new("/system/bin/app_process64")), true), None);
        // iOS: Frameworks im Bundle, auch ohne Treffer (dann ohne Vorschau)
        let root = std::env::temp_dir().join(format!("pida-app-pdfium-ios-{}", std::process::id()));
        let app = root.join("PIDA.app");
        let exe = app.join("pida");
        assert_eq!(mobile_dir(Some(&exe), false), Some(app.join("Frameworks")));
        std::fs::create_dir_all(&app).expect("Bundle");
        std::fs::write(app.join(library_file_name()), b"").expect("lib");
        assert_eq!(mobile_dir(Some(&exe), false), Some(app.clone()), "lose Bibliothek neben der Binary");
        assert_eq!(mobile_dir(None, false), None);
        let _ = std::fs::remove_dir_all(root);
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
