//! Versionsdatum für `build.rs` (`PIDA_RELEASE_DATE`, „Über PIDA“, ADR 0004 Nachtrag 21) – eigene Datei, damit
//! `tests/release_date.rs` die Auswahl prüfen kann (ein Build-Skript selbst hat keine Tests).
//!
//! Reihenfolge – dasselbe Datum für denselben Stand, solange eine der ersten drei Quellen greift:
//! 1. `SOURCE_DATE_EPOCH` (reproducible-builds.org; Paketbauer ohne Git), als Kalenderdatum in UTC,
//! 2. das Commit-Datum des gebauten Stands (`git log -1 --format=%cs`) – **nur**, wenn dieses Repository das Crate
//!    selbst verwaltet (`Cargo.toml` ist darin eingecheckt): ein Quellarchiv, das in einem fremden Repository
//!    ausgepackt wurde (Home-Verzeichnis unter Git, Monorepo, Vendoring), bekäme sonst dessen Datum,
//! 3. ohne Git die Dateizeit von `Cargo.toml` (UTC): Quellarchive von GitHub (`git archive`) tragen die Zeit des
//!    Commits,
//! 4. zuletzt das Datum des Bauens (UTC) – nicht reproduzierbar, nur damit überhaupt ein Datum dasteht.
//!
//! Angaben vor [`EARLIEST`] gelten als unbrauchbar und werden übersprungen (`SOURCE_DATE_EPOCH=0`, die Vorgabe
//! 1980-01-01 mancher Paketbauer, Dateizeiten aus Archiven ohne Zeitangabe) – sonst stünde „© 1970“ im
//! Urheberrechtsvermerk. Eingebettet wird **nur** das Datum: keine Pfade, Namen oder Commit-Kennungen.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Früheste zulässige Angabe (Jahr der ersten Veröffentlichung von PIDA).
pub const EARLIEST: &str = "2026-01-01";

/// Woher das Datum stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `SOURCE_DATE_EPOCH`
    SourceDateEpoch,
    /// Commit-Datum aus Git
    Git,
    /// Dateizeit von `Cargo.toml`
    FileTime,
    /// Datum des Bauens
    BuildTime,
}

/// Wählt das Datum aus den Quellen (siehe Modul); `now` = Sekunden seit 1970 beim Bauen.
pub fn select(
    source_date_epoch: Option<&str>,
    git: Option<&str>,
    file_time: Option<i64>,
    now: i64,
) -> (String, Origin) {
    let from_epoch = |secs: i64| Some(civil_date(secs)).filter(|d| plausible(d));
    if let Some(d) = source_date_epoch.and_then(|s| s.trim().parse::<i64>().ok()).and_then(from_epoch) {
        return (d, Origin::SourceDateEpoch);
    }
    if let Some(d) = git.filter(|d| is_iso_date(d) && plausible(d)) {
        return (d.to_owned(), Origin::Git);
    }
    if let Some(d) = file_time.and_then(from_epoch) {
        return (d, Origin::FileTime);
    }
    (civil_date(now), Origin::BuildTime)
}

/// Nicht vor [`EARLIEST`] (ISO-Daten lassen sich als Text vergleichen).
pub fn plausible(date: &str) -> bool {
    date >= EARLIEST
}

/// Commit-Datum (`%cs`, Zeitzone des Commits) des Repositorys, das `dir` (mit seiner `Cargo.toml`) verwaltet, samt
/// den Dateien, bei deren Änderung das Datum neu zu bestimmen ist (`HEAD`, `logs/HEAD`, Zweig, `packed-refs`); `None`
/// ohne Git, außerhalb eines Repositorys oder wenn `Cargo.toml` dort nicht eingecheckt ist.
pub fn git_commit_date(dir: &Path) -> Option<(String, Vec<PathBuf>)> {
    // gehört das Crate zu diesem Repository? (nicht: ausgepacktes Archiv in einem fremden Repository)
    git(dir, &["ls-files", "--error-unmatch", "--", "Cargo.toml"])?;
    let date = git(dir, &["log", "-1", "--format=%cs"])?;
    if !is_iso_date(&date) {
        return None;
    }
    let mut rerun = Vec::new();
    if let Some(git_dir) = git(dir, &["rev-parse", "--absolute-git-dir"]) {
        let git_dir = Path::new(&git_dir);
        rerun.push(git_dir.join("HEAD"));
        // jede Bewegung von HEAD (Commit, Checkout, Reset) – auch wenn der Zweig nur in `packed-refs` steht
        rerun.push(git_dir.join("logs").join("HEAD"));
    }
    if let (Some(common), Some(head_ref)) =
        (git(dir, &["rev-parse", "--git-common-dir"]), git(dir, &["rev-parse", "--symbolic-full-name", "HEAD"]))
    {
        let common = if Path::new(&common).is_absolute() { PathBuf::from(common) } else { dir.join(common) };
        if head_ref.starts_with("refs/") {
            rerun.push(common.join(&head_ref));
        }
        rerun.push(common.join("packed-refs"));
    }
    // nur vorhandene Dateien: eine fehlende hielte Cargo für „geändert“ und führte das Skript jedes Mal aus
    rerun.retain(|p| p.is_file());
    Some((date, rerun))
}

/// Dateizeit (Sekunden seit 1970) – für Quellen ohne Git.
pub fn file_time(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok()
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// `JJJJ-MM-TT` mit plausiblen Werten.
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && (1..=12).contains(&s[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&s[8..10].parse::<u8>().unwrap_or(0))
}

/// Sekunden seit 1970 → Kalenderdatum (UTC, proleptischer gregorianischer Kalender; H. Hinnant, `civil_from_days`).
pub fn civil_date(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
