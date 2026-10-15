//! Logdatei mit Rotation (Plan §7.1: ≤ 2 MB, ohne Secrets und Dokumentinhalte; analyse/05 §3.2).
//!
//! `pdfover-config` liefert Ordner, Dateinamen und Rotation ([`Paths::rotate_logs`],
//! [`pdfover_config::paths::LOG_MAX_FILE_BYTES`] × [`pdfover_config::paths::LOG_KEEP`]); hier wird die
//! Datei beim Start rotiert und über `tracing-appender` (nicht blockierend) beschrieben. Die Shell
//! bzw. die CLI ruft [`init`] einmal je Prozess auf; ist bereits ein globaler Subscriber gesetzt,
//! bleibt er bestehen.

use pdfover_config::Paths;
use pdfover_config::paths::{LOG_FILE, LOG_KEEP, LOG_MAX_FILE_BYTES};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Hält den Schreib-Thread der Logdatei am Leben (beim Drop wird geleert).
pub struct LogGuard {
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
    /// Stufe bzw. Filter (`warn`, `info`, `pdfover=debug` …); `RUST_LOG` hat Vorrang
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

fn filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level))
}

/// Richtet das Logging ein: Rotation beim Start, Logdatei (ohne ANSI), optional stderr.
pub fn init(paths: &Paths, opts: &LogOptions) -> LogGuard {
    let mut guard = None;
    let mut file = None;
    let file_layer = if opts.file {
        match paths.rotate_logs(LOG_MAX_FILE_BYTES, LOG_KEEP) {
            Ok(path) => {
                let appender = tracing_appender::rolling::never(&paths.log_dir, LOG_FILE);
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

    #[test]
    fn rotates_and_creates_log_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::under(tmp.path());
        std::fs::create_dir_all(&paths.log_dir).unwrap();
        std::fs::write(paths.log_file(), vec![b'x'; LOG_MAX_FILE_BYTES as usize]).unwrap();
        let g = init(&paths, &LogOptions { level: "debug".into(), stderr: false, file: true });
        // die volle Datei wurde rotiert (auch wenn in diesem Testprozess schon ein Subscriber existiert)
        assert!(paths.log_dir.join(format!("{LOG_FILE}.1")).is_file());
        if g.installed {
            tracing::info!("Testeintrag");
            drop(g);
            assert!(std::fs::read_to_string(paths.log_file()).unwrap().contains("Testeintrag"));
        }
    }
}
