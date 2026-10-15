//! Dateien aus Startargumenten, „Öffnen mit“/„Senden an“ (zweite Instanz) und Datei-URLs.
//!
//! Die Shell übernimmt nur **vorhandene Dateien** (F-12) und reicht sie als
//! [`pdfover_core_api::UiEvent::OpenFiles`] an den Kern; die Prüfung auf PDF macht der Kern.
//! Die vollständige PDF-Over-Argumentsemantik (`-c`, `-b`, `-o` …) wertet die CLI aus; die GUI
//! übernimmt hier nur Dokumente (`-i <datei>` und freie Tokens wie bei PDF-Over: „unbekanntes Token =
//! Datei“, docs/analyse/01 §5.1).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// PDF-Over-Optionen mit Wert (Wert wird übersprungen, nur `-i` liefert ein Dokument).
const OPTIONS_WITH_VALUE: &[&str] = &[
    "-i",
    "-o",
    "-b",
    "-n",
    "-p",
    "-e",
    "-c",
    "-v",
    "-ks",
    "-kstype",
    "-kspass",
    "-kskeypass",
    "-ksalias",
    "-proxy",
    "-proxyhost",
    "-proxyport",
    "-proxyuser",
    "-proxypass",
];

/// Dokumente aus einer Argumentliste **ohne** Programmnamen; relative Pfade gelten relativ zu `cwd`.
/// Werte von Optionen (z. B. Passwörter nach `-p`) werden nie als Datei interpretiert.
///
/// Die Argumente sind `OsStr`-artig: Dateinamen dürfen unter Unix beliebige Bytes enthalten
/// (`std::env::args()` bräche bei Nicht-UTF-8 mit einer Panik ab, deshalb `args_os()`).
pub fn documents_from_args<I, S>(args: I, cwd: &Path) -> Vec<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut out = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_ref();
        if arg == "-i" {
            if let Some(value) = iter.next() {
                push_existing(&mut out, cwd, value.as_ref());
            }
            continue;
        }
        if arg.to_str().is_some_and(|a| OPTIONS_WITH_VALUE.contains(&a)) {
            let _ = iter.next();
            continue;
        }
        let bytes = arg.as_encoded_bytes();
        if bytes.first() == Some(&b'-') && bytes.len() > 1 {
            // Schalter ohne Wert (-a, -s, -h, -?, --help) bzw. unbekannte Optionen.
            continue;
        }
        push_existing(&mut out, cwd, arg);
    }
    out
}

/// Dokumente aus URLs (`RunEvent::Opened`, Deep Links): nur `file:`-URLs auf vorhandene Dateien.
pub fn documents_from_urls<'a, I>(urls: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = &'a url::Url>,
{
    let mut out = Vec::new();
    for url in urls {
        if url.scheme() != "file" {
            continue;
        }
        if let Ok(path) = url.to_file_path()
            && path.is_file()
            && !out.contains(&path)
        {
            out.push(path);
        }
    }
    out
}

fn push_existing(out: &mut Vec<PathBuf>, cwd: &Path, value: &OsStr) {
    if value.is_empty() {
        return;
    }
    let candidate = Path::new(value);
    let path = if candidate.is_absolute() { candidate.to_path_buf() } else { cwd.join(candidate) };
    if path.is_file() && !out.contains(&path) {
        out.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("pdfover-app-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("Temp-Ordner");
        let a = dir.join("a.pdf");
        let b = dir.join("b c.pdf");
        std::fs::write(&a, b"%PDF-1.7\n").expect("a");
        std::fs::write(&b, b"%PDF-1.7\n").expect("b");
        (dir, a, b)
    }

    #[test]
    fn bare_tokens_and_dash_i_are_documents() {
        let (dir, a, b) = fixture();
        let docs = documents_from_args(["a.pdf", "-i", b.to_str().expect("utf-8"), "-a", "-s"], &dir);
        assert_eq!(docs, vec![a, b]);
    }

    #[test]
    fn option_values_are_never_documents() {
        let (dir, a, _) = fixture();
        // Ein Passwort, das zufällig wie eine vorhandene Datei heißt, wird nicht geöffnet.
        let docs = documents_from_args(["-p", "a.pdf", "-n", "a.pdf", "-o", "a.pdf"], &dir);
        assert!(docs.is_empty(), "{docs:?}");
        let docs = documents_from_args(["-b", "MOBILE", "a.pdf"], &dir);
        assert_eq!(docs, vec![a]);
    }

    #[test]
    fn missing_files_and_duplicates_are_ignored() {
        let (dir, a, _) = fixture();
        let docs = documents_from_args(["fehlt.pdf", "a.pdf", "a.pdf", ""], &dir);
        assert_eq!(docs, vec![a]);
    }

    /// Unter Unix sind Dateinamen Bytefolgen: ein Nicht-UTF-8-Name darf weder abstürzen noch verloren
    /// gehen (Linux „Öffnen mit“ mit Latin-1-Namen).
    #[cfg(unix)]
    #[test]
    fn non_utf8_file_names_are_accepted() {
        use std::os::unix::ffi::OsStrExt;
        let (dir, a, _) = fixture();
        let name = OsStr::from_bytes(b"latin1-\xe4.pdf");
        let path = dir.join(name);
        if std::fs::write(&path, b"%PDF-1.7\n").is_err() {
            // Dateisystem ohne Bytenamen (z. B. APFS verlangt UTF-8): nur „keine Panik“ prüfen.
            let docs = documents_from_args([name, OsStr::new("a.pdf"), OsStr::from_bytes(b"-\xff")], &dir);
            assert_eq!(docs, vec![a]);
            return;
        }
        let docs = documents_from_args([name, OsStr::new("-p"), name, OsStr::new("a.pdf")], &dir);
        assert_eq!(docs, vec![path.clone(), a]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn only_file_urls_are_accepted() {
        let (_, a, b) = fixture();
        let urls = vec![
            url::Url::from_file_path(&a).expect("url"),
            url::Url::parse("https://example.invalid/x.pdf").expect("url"),
            url::Url::from_file_path(&b).expect("url"),
            url::Url::parse("file:///gibt/es/nicht.pdf").expect("url"),
        ];
        assert_eq!(documents_from_urls(&urls), vec![a, b]);
    }
}
