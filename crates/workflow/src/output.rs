//! Ausgaberegeln (F-81…F-86) und Exit-Codes (Plan §7.4).

use std::path::{Path, PathBuf};

/// Exit-Codes der CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum ExitCode {
    /// erfolgreich
    Success = 0,
    /// allgemeiner Fehler
    General = 1,
    /// Aufruf-/Syntaxfehler
    Usage = 2,
    /// Eingabe abgelehnt (K5, Rechte)
    Rejected = 3,
    /// Abbruch durch Nutzerin (u. a. SL 6000/6001)
    Cancelled = 4,
    /// Backend-/Netzfehler
    Backend = 5,
    /// Invariantenverletzung (nichts geschrieben)
    Invariant = 6,
    /// Schreibfehler Ausgabe
    Write = 7,
}

/// Dateisystem-Abfragen (testbar).
pub trait PathProbe {
    /// Pfad ist ein vorhandener Ordner.
    fn is_dir(&self, path: &Path) -> bool;
    /// Pfad existiert.
    fn exists(&self, path: &Path) -> bool;
}

/// Entscheidung über das Ausgabeziel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutputDecision {
    /// Automatisch speichern unter diesem Pfad (Überschreib-Rückfrage, falls vorhanden).
    Save {
        /// Ziel
        path: PathBuf,
        /// existiert bereits
        exists: bool,
    },
    /// Speichern-Dialog: Startordner = Ordner der Quelle, Vorschlag nach F-81.
    Ask {
        /// Startordner
        directory: PathBuf,
        /// Dateiname
        file_name: String,
    },
}

/// `<Basisname><Postfix>.<Erweiterung>`; ohne Erweiterung kein abschließender Punkt (B-10 behoben).
pub fn output_file_name(input: &Path, postfix: &str) -> String {
    todo!("Integration")
}

/// F-82/F-83: Ausgabeordner existiert als Ordner → `<Ordner>/<Name>`; sonst als vollständiger
/// Dateipfad; relativ → relativ zum Ordner der Quelle; ohne Konfiguration → Dialog.
pub fn decide_output(input: &Path, configured: Option<&Path>, postfix: &str, probe: &dyn PathProbe) -> OutputDecision {
    todo!("Integration")
}
