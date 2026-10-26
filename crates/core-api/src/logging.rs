//! Logdatei mit Rotation (Plan §7.1: ≤ 2 MB, ohne Secrets und Dokumentinhalte; analyse/05 §3.2).
//!
//! `pida-config` liefert Ordner, Dateinamen und Rotation ([`Paths::rotate_logs`],
//! [`pida_config::paths::LOG_MAX_FILE_BYTES`] × [`pida_config::paths::LOG_KEEP`]); hier wird die
//! Datei beim Start **und während des Laufs** rotiert (sobald sie die Größe erreicht, damit auch eine
//! lange laufende Sitzung mit Debug-Stufe die Obergrenze einhält) und über `tracing-appender` (nicht
//! blockierend) beschrieben. Die Shell bzw. die CLI ruft [`init`] einmal je Prozess auf; ist bereits
//! ein globaler Subscriber gesetzt, bleibt er bestehen.

#![cfg_attr(all(target_arch = "wasm32", target_os = "unknown"), allow(unused_imports, dead_code))]

use std::io::Write;

use pida_config::Paths;
use pida_config::paths::{LOG_KEEP, LOG_MAX_FILE_BYTES};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Hält den Schreib-Thread der Logdatei am Leben (beim Drop wird geleert).
pub struct LogGuard {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    _file: Option<tracing_appender::non_blocking::WorkerGuard>,
    /// Logdatei
    pub file: Option<std::path::PathBuf>,
    /// `false`, wenn bereits ein Subscriber gesetzt war
    pub installed: bool,
}

impl std::fmt::Debug for LogGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogGuard").field("file", &self.file).field("installed", &self.installed).finish()
    }
}

/// Optionen.
#[derive(Debug, Clone)]
pub struct LogOptions {
    /// Stufe bzw. Filter (`warn`, `info`, `pida=debug` …); `RUST_LOG` hat Vorrang
    pub level: String,
    /// zusätzlich auf stderr (CLI)
    pub stderr: bool,
    /// in die Logdatei schreiben
    pub file: bool,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self { level: "info".into(), stderr: false, file: true }
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
/// Schreiber der Logdatei mit Rotation während des Laufs (läuft im Schreib-Thread von
/// `tracing-appender`, deshalb ohne Sperren).
struct RotatingWriter {
    paths: Paths,
    file: Option<pida_fs::File>,
    written: u64,
    max: u64,
    keep: u32,
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl RotatingWriter {
    fn open(paths: Paths, max: u64, keep: u32) -> std::io::Result<RotatingWriter> {
        let mut w = RotatingWriter { paths, file: None, written: 0, max, keep };
        w.reopen()?;
        Ok(w)
    }

    fn reopen(&mut self) -> std::io::Result<()> {
        let path = self.paths.log_file();
        let file = pida_fs::OpenOptions::new().create(true).append(true).open(&path)?;
        self.written = file.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(file);
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        if let Some(mut f) = self.file.take() {
            f.flush()?;
        }
        self.paths.rotate_logs(self.max, self.keep).map_err(std::io::Error::other)?;
        self.reopen()
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl Write for RotatingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.written >= self.max {
            // Rotation fehlgeschlagen (z. B. Datei gesperrt): weiter in die bisherige Datei
            let _ = self.rotate();
        }
        if self.file.is_none() {
            self.reopen()?;
        }
        let n = self.file.as_mut().map_or(Ok(0), |f| f.write(buf))?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.as_mut().map_or(Ok(()), pida_fs::File::flush)
    }
}

fn filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level))
}

/// ANSI-Farben auf stderr nur an einem Terminal, nicht mit `NO_COLOR` und nie auf Mobilgeräten:
/// iOS leitet stderr in das System-Log um (`log show`), Android verwirft es – Farbcodes stünden dort
/// als `[2m…[0m` im Text (docs/integration/mobile-ios.md §7 Nr. 5).
fn stderr_ansi(mobile: bool, no_color: Option<&std::ffi::OsStr>, terminal: bool) -> bool {
    !mobile && no_color.is_none_or(std::ffi::OsStr::is_empty) && terminal
}

fn stderr_ansi_here() -> bool {
    use std::io::IsTerminal;
    stderr_ansi(
        cfg!(any(target_os = "ios", target_os = "android")),
        std::env::var_os("NO_COLOR").as_deref(),
        std::io::stderr().is_terminal(),
    )
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
/// Browser: keine Logdatei (kein Dateisystem, das bleibt); die Web-Shell richtet die Ausgabe in die Konsole selbst ein.
pub fn init(_paths: &Paths, _opts: &LogOptions) -> LogGuard {
    LogGuard { file: None, installed: false }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
/// Richtet das Logging ein: Rotation beim Start, Logdatei (ohne ANSI), optional stderr.
pub fn init(paths: &Paths, opts: &LogOptions) -> LogGuard {
    let mut guard = None;
    let mut file = None;
    let file_layer = if opts.file {
        match paths
            .rotate_logs(LOG_MAX_FILE_BYTES, LOG_KEEP)
            .and_then(|path| Ok((path, RotatingWriter::open(paths.clone(), LOG_MAX_FILE_BYTES, LOG_KEEP)?)))
        {
            Ok((path, appender)) => {
                let (writer, g) = tracing_appender::non_blocking(appender);
                guard = Some(g);
                file = Some(path);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_target(true)
                        .with_writer(writer)
                        .with_filter(filter(&opts.level)),
                )
            }
            Err(e) => {
                eprintln!("Logdatei nicht verfügbar: {}", e.message_id());
                None
            }
        }
    } else {
        None
    };
    let stderr_layer = opts.stderr.then(|| {
        tracing_subscriber::fmt::layer()
            .with_ansi(stderr_ansi_here())
            .with_writer(std::io::stderr)
            .with_target(false)
            .without_time()
            .with_filter(filter(&opts.level))
    });
    let installed = tracing_subscriber::registry().with(file_layer).with(stderr_layer).try_init().is_ok();
    if !installed {
        guard = None;
        file = None;
    }
    LogGuard { _file: guard, file, installed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_config::paths::LOG_FILE;
    use pida_fs::PathExt as _;

    /// Lange Sitzung: die Datei rotiert auch während des Laufs; insgesamt höchstens
    /// `(LOG_KEEP + 1) × LOG_MAX_FILE_BYTES` (+ ein Eintrag) – Plan §7.1 (≤ 2 MB).
    #[test]
    fn rotates_while_running() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::under(tmp.path());
        pida_fs::create_dir_all(&paths.log_dir).unwrap();
        let mut w = RotatingWriter::open(paths.clone(), 1000, 2).unwrap();
        let line = [b'x'; 99];
        for _ in 0..100 {
            w.write_all(&line).unwrap();
            w.write_all(b"\n").unwrap();
        }
        w.flush().unwrap();
        let size = |p: &std::path::Path| pida_fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        assert!(size(&paths.log_file()) <= 1000 + 100, "{}", size(&paths.log_file()));
        assert!(paths.log_dir.join(format!("{LOG_FILE}.1")).fs_is_file());
        assert!(paths.log_dir.join(format!("{LOG_FILE}.2")).fs_is_file());
        assert!(!paths.log_dir.join(format!("{LOG_FILE}.3")).fs_exists(), "nur `keep` alte Dateien");
        let total: u64 = pida_fs::read_dir(&paths.log_dir).unwrap().map(|e| size(&e.unwrap().path())).sum();
        assert!(total <= 3 * 1100, "{total}");
    }

    /// Farbcodes nur an einem Terminal des Desktops (iOS-System-Log ohne `ESC[…m`).
    #[test]
    fn stderr_ansi_only_on_desktop_terminals() {
        use std::ffi::OsStr;
        assert!(stderr_ansi(false, None, true));
        assert!(stderr_ansi(false, Some(OsStr::new("")), true), "leeres NO_COLOR zählt nicht");
        assert!(!stderr_ansi(false, Some(OsStr::new("1")), true));
        assert!(!stderr_ansi(false, None, false), "umgeleitetes stderr (Datei, Pipe, System-Log)");
        assert!(!stderr_ansi(true, None, true), "iOS/Android: nie");
    }

    #[test]
    fn rotates_and_creates_log_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::under(tmp.path());
        pida_fs::create_dir_all(&paths.log_dir).unwrap();
        pida_fs::write(paths.log_file(), vec![b'x'; LOG_MAX_FILE_BYTES as usize]).unwrap();
        let g = init(&paths, &LogOptions { level: "debug".into(), stderr: false, file: true });
        // die volle Datei wurde rotiert (auch wenn in diesem Testprozess schon ein Subscriber existiert)
        assert!(paths.log_dir.join(format!("{LOG_FILE}.1")).fs_is_file());
        if g.installed {
            tracing::info!("Testeintrag");
            drop(g);
            assert!(pida_fs::read_to_string(paths.log_file()).unwrap().contains("Testeintrag"));
        }
    }
}
