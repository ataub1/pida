//! Dateizugriff für PIDA (Konzept Webdienst, Etappe 1).
//!
//! **Nativ** ist dieses Crate nur ein Durchreichen von [`std::fs`] – Verhalten, Fehler und Typen bleiben exakt die
//! der Standardbibliothek. **Im Browser** (`wasm32-unknown-unknown`) gibt es kein Dateisystem: Dort liegt hinter
//! denselben Funktionen ein Dateisystem im Speicher. Die Web-Shell legt hochgeladene PDFs und Keystores hinein, holt
//! signierte Dokumente heraus und sichert die Einstellungen (Ordner `config`) im Browser ([`snapshot`], [`restore`]).
//!
//! Prüfungen am Pfad (`Path::exists`, `is_file`, `is_dir`) fragen in der Standardbibliothek immer das echte
//! Dateisystem; im Kern laufen sie deshalb über [`PathExt`] (`fs_exists`, `fs_is_file`, `fs_is_dir`).

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub use std::fs::*;

// nativ nur für die Tests übersetzt (dieselbe Logik wie im Browser; dort ungenutzte Teile erlaubt)
#[cfg(any(test, all(target_arch = "wasm32", target_os = "unknown")))]
#[cfg_attr(test, allow(dead_code))]
mod mem;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use mem::*;

use std::path::Path;
use std::time::SystemTime;

/// Jetzt als [`std::time::SystemTime`] – im Browser aus JavaScript (`Date.now`), weil die Standardbibliothek dort keine
/// Uhr hat (`SystemTime::now` bricht ab). Gleicher Typ wie `Metadata::modified`, damit Vergleiche nativ wie im Browser
/// gehen.
pub fn system_now() -> SystemTime {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        let since = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).unwrap_or_default();
        SystemTime::UNIX_EPOCH + since
    }
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        SystemTime::now()
    }
}

/// Prozess-ID für eindeutige Namen temporärer Dateien – im Browser gibt es keine (`std::process::id` bricht ab), dort
/// `0` (ein Kern je Seite, eindeutig über den Zähler des Aufrufers).
pub fn process_id() -> u32 {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        0
    }
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        std::process::id()
    }
}

/// Prüfungen am Pfad über dieses Crate (im Browser gegen das Dateisystem im Speicher).
pub trait PathExt {
    /// wie [`Path::exists`]
    fn fs_exists(&self) -> bool;
    /// wie [`Path::is_file`]
    fn fs_is_file(&self) -> bool;
    /// wie [`Path::is_dir`]
    fn fs_is_dir(&self) -> bool;
}

impl<P: AsRef<Path> + ?Sized> PathExt for P {
    fn fs_exists(&self) -> bool {
        metadata(self.as_ref()).is_ok()
    }
    fn fs_is_file(&self) -> bool {
        metadata(self.as_ref()).is_ok_and(|m| m.is_file())
    }
    fn fs_is_dir(&self) -> bool {
        metadata(self.as_ref()).is_ok_and(|m| m.is_dir())
    }
}
