//! Ausgaberegeln (F-81…F-86) und Exit-Codes (Plan §7.4).
//!
//! | Regel | Umsetzung |
//! |---|---|
//! | F-81 Dateiname | [`output_file_name`]: `<Basisname><Postfix>.<Erweiterung>` wie `FilenameUtils` (Erweiterung = Text nach dem **letzten** Punkt, auch `.hidden` → Erweiterung `hidden`); ohne Erweiterung **kein** abschließender Punkt (B-10 behoben) |
//! | F-82 Ausgabeordner | [`decide_output`]: konfigurierter Wert, relativ zum Ordner der Quelle aufgelöst; ist er ein vorhandener Ordner → `<Ordner>/<Name>`, **sonst wird er als vollständiger Dateipfad benutzt** (Quirk von PDF-Over, bewusst übernommen) |
//! | F-83 Speichern-Dialog | ohne (bzw. mit leerem) Ausgabeordner: Dialog mit Startordner = Ordner der Quelle und Vorschlag nach F-81 |
//! | F-84 Überschreiben | `exists` in [`OutputDecision::Save`] → Rückfrage im Reducer |
//! | F-86 Speichern-Knopf | öffnet immer den Dialog ([`save_dialog`]) |
//!
//! Abweichung: PDF-Over prüft „ist ein Ordner“ für einen **relativen** Ausgabeordner gegen das
//! Arbeitsverzeichnis des Prozesses, speichert dann aber relativ zum Ordner der Quelle; hier gilt
//! für beides der Ordner der Quelle (sonst hinge das Ergebnis vom Startort ab).

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

impl ExitCode {
    /// Zahlenwert für `std::process::exit`.
    pub fn code(self) -> i32 {
        self as i32
    }

    /// Stabiler Name (CLI `--json`).
    pub fn as_str(self) -> &'static str {
        match self {
            ExitCode::Success => "success",
            ExitCode::General => "general",
            ExitCode::Usage => "usage",
            ExitCode::Rejected => "rejected",
            ExitCode::Cancelled => "cancelled",
            ExitCode::Backend => "backend",
            ExitCode::Invariant => "invariant",
            ExitCode::Write => "write",
        }
    }
}

/// Dateisystem-Abfragen (testbar).
pub trait PathProbe {
    /// Pfad ist ein vorhandener Ordner.
    fn is_dir(&self, path: &Path) -> bool;
    /// Pfad existiert.
    fn exists(&self, path: &Path) -> bool;
}

/// [`PathProbe`] über das echte Dateisystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsProbe;

impl PathProbe for FsProbe {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
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

/// Teilt einen Dateinamen wie Apache `FilenameUtils` in Basis und Erweiterung: Erweiterung ist der
/// Text nach dem letzten Punkt (auch an Position 0), sonst leer.
fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => (name, ""),
    }
}

/// `<Basisname><Postfix>.<Erweiterung>`; ohne Erweiterung kein abschließender Punkt (B-10 behoben).
///
/// Pfadtrenner und NUL im Postfix werden durch `_` ersetzt: Ein Postfix aus einer importierten bzw. von
/// Hand bearbeiteten Konfiguration (`SAVE_FILE_POSTFIX=/../x`) darf das Ziel nie aus dem Ausgabeordner
/// herausführen (die Einstellungsansicht lehnt solche Werte ohnehin ab).
pub fn output_file_name(input: &Path, postfix: &str) -> String {
    let name = input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let postfix: String = postfix.chars().map(|c| if matches!(c, '/' | '\\' | '\0') { '_' } else { c }).collect();
    let (base, ext) = split_name(&name);
    if ext.is_empty() { format!("{base}{postfix}") } else { format!("{base}{postfix}.{ext}") }
}

/// Ordner der Quelldatei (`""` → `.`).
fn source_dir(input: &Path) -> PathBuf {
    match input.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// F-82/F-83: Ausgabeordner existiert als Ordner → `<Ordner>/<Name>`; sonst als vollständiger
/// Dateipfad; relativ → relativ zum Ordner der Quelle; ohne Konfiguration (oder leer) → Dialog.
pub fn decide_output(input: &Path, configured: Option<&Path>, postfix: &str, probe: &dyn PathProbe) -> OutputDecision {
    let name = output_file_name(input, postfix);
    let configured = configured.filter(|c| !c.as_os_str().to_string_lossy().trim().is_empty());
    let Some(configured) = configured else {
        return OutputDecision::Ask { directory: source_dir(input), file_name: name };
    };
    let resolved = if configured.is_relative() { source_dir(input).join(configured) } else { configured.to_path_buf() };
    let path = if probe.is_dir(&resolved) { resolved.join(name) } else { resolved };
    let exists = probe.exists(&path);
    OutputDecision::Save { path, exists }
}

/// F-86: der Speichern-Knopf öffnet **immer** den Dialog (Startordner = Ordner der Quelle).
pub fn save_dialog(input: &Path, postfix: &str) -> OutputDecision {
    OutputDecision::Ask { directory: source_dir(input), file_name: output_file_name(input, postfix) }
}

/// Ohne Oberfläche (CLI) gibt es keinen Speichern-Dialog: [`OutputDecision::Ask`] wird zum
/// Speichern neben der Quelle mit dem vorgeschlagenen Namen.
pub fn resolve_without_dialog(decision: OutputDecision, probe: &dyn PathProbe) -> OutputDecision {
    match decision {
        OutputDecision::Ask { directory, file_name } => {
            let path = directory.join(file_name);
            let exists = probe.exists(&path);
            OutputDecision::Save { path, exists }
        }
        save => save,
    }
}

/// Ziel eines vom Dialog gewählten Pfads (Überschreib-Rückfrage, F-84).
pub fn chosen_path(path: PathBuf, probe: &dyn PathProbe) -> OutputDecision {
    let exists = probe.exists(&path);
    OutputDecision::Save { path, exists }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Default)]
    struct FakeFs {
        dirs: HashSet<PathBuf>,
        files: HashSet<PathBuf>,
    }

    impl PathProbe for FakeFs {
        fn is_dir(&self, path: &Path) -> bool {
            self.dirs.contains(path)
        }
        fn exists(&self, path: &Path) -> bool {
            self.dirs.contains(path) || self.files.contains(path)
        }
    }

    #[test]
    fn file_names_like_pdf_over() {
        let p = |s: &str| output_file_name(Path::new(s), "_signed");
        assert_eq!(p("/a/doc.pdf"), "doc_signed.pdf");
        assert_eq!(p("/a/doc.PDF"), "doc_signed.PDF");
        assert_eq!(p("/a/x.tar.pdf"), "x.tar_signed.pdf");
        assert_eq!(p("/a/x_signed.pdf"), "x_signed_signed.pdf"); // F-92
        // B-10 behoben: kein abschließender Punkt
        assert_eq!(p("/a/noext"), "noext_signed");
        assert_eq!(p("/a/trailing."), "trailing_signed");
        // FilenameUtils: ".hidden" ist eine Erweiterung
        assert_eq!(p("/a/.hidden"), "_signed.hidden");
        assert_eq!(output_file_name(Path::new("d.pdf"), ""), "d.pdf");
        assert_eq!(output_file_name(Path::new("d.pdf"), "-sig"), "d-sig.pdf");
    }

    #[test]
    fn postfix_cannot_leave_the_output_folder() {
        let p = |s: &str, postfix: &str| output_file_name(Path::new(s), postfix);
        assert_eq!(p("/a/doc.pdf", "/../../etc/x"), "doc_.._.._etc_x.pdf");
        assert_eq!(p("/a/doc.pdf", "..\\x"), "doc.._x.pdf");
        assert_eq!(p("/a/doc.pdf", "a\0b"), "doca_b.pdf");
        let mut fs = FakeFs::default();
        fs.dirs.insert("/out".into());
        let d = decide_output(Path::new("/in/d.pdf"), Some(Path::new("/out")), "/../../evil", &fs);
        let OutputDecision::Save { path, .. } = d else { panic!("Ordner erwartet") };
        assert_eq!(path.parent(), Some(Path::new("/out")));
        assert!(path.components().all(|c| c != std::path::Component::ParentDir));
    }

    #[test]
    fn no_configured_folder_asks() {
        let fs = FakeFs::default();
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), None, "_signed", &fs),
            OutputDecision::Ask { directory: "/in".into(), file_name: "d_signed.pdf".into() }
        );
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), Some(Path::new("  ")), "_signed", &fs),
            OutputDecision::Ask { directory: "/in".into(), file_name: "d_signed.pdf".into() }
        );
        // Quelle ohne Ordner
        assert_eq!(
            decide_output(Path::new("d.pdf"), None, "_signed", &fs),
            OutputDecision::Ask { directory: ".".into(), file_name: "d_signed.pdf".into() }
        );
    }

    #[test]
    fn folder_or_file_quirk() {
        let mut fs = FakeFs::default();
        fs.dirs.insert("/out".into());
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), Some(Path::new("/out")), "_signed", &fs),
            OutputDecision::Save { path: "/out/d_signed.pdf".into(), exists: false }
        );
        // kein Ordner → vollständiger Dateipfad (Quirk F-82)
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), Some(Path::new("/out/result.pdf")), "_signed", &fs),
            OutputDecision::Save { path: "/out/result.pdf".into(), exists: false }
        );
        fs.files.insert("/out/d_signed.pdf".into());
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), Some(Path::new("/out")), "_signed", &fs),
            OutputDecision::Save { path: "/out/d_signed.pdf".into(), exists: true }
        );
    }

    #[test]
    fn relative_folder_is_relative_to_source() {
        let mut fs = FakeFs::default();
        fs.dirs.insert("/in/signed".into());
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), Some(Path::new("signed")), "_signed", &fs),
            OutputDecision::Save { path: "/in/signed/d_signed.pdf".into(), exists: false }
        );
        assert_eq!(
            decide_output(Path::new("/in/d.pdf"), Some(Path::new("x.pdf")), "_signed", &fs),
            OutputDecision::Save { path: "/in/x.pdf".into(), exists: false }
        );
    }

    #[test]
    fn dialog_helpers() {
        let mut fs = FakeFs::default();
        assert_eq!(
            save_dialog(Path::new("/in/d.pdf"), "_s"),
            OutputDecision::Ask { directory: "/in".into(), file_name: "d_s.pdf".into() }
        );
        fs.files.insert("/in/d_s.pdf".into());
        assert_eq!(
            resolve_without_dialog(save_dialog(Path::new("/in/d.pdf"), "_s"), &fs),
            OutputDecision::Save { path: "/in/d_s.pdf".into(), exists: true }
        );
        assert_eq!(
            chosen_path("/x/y.pdf".into(), &fs),
            OutputDecision::Save { path: "/x/y.pdf".into(), exists: false }
        );
        assert_eq!(ExitCode::Write.code(), 7);
        assert_eq!(ExitCode::Cancelled.as_str(), "cancelled");
    }
}
