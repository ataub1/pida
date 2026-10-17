//! Build-Skript von `pida-core-api`: bettet das **Versionsdatum** ein (`PIDA_RELEASE_DATE`, `JJJJ-MM-TT`;
//! „Über PIDA“, ADR 0004 Nachtrag 21).
//!
//! Reproduzierbar – dasselbe Datum für denselben Stand, egal wann und wo gebaut wird:
//! 1. `SOURCE_DATE_EPOCH` (Sekunden seit 1970, reproducible-builds.org), als Kalenderdatum in UTC,
//! 2. sonst das Commit-Datum des gebauten Stands (`git log -1 --format=%cs`),
//! 3. sonst (Quellen ohne Git) das Datum des Bauens (UTC).
//!
//! Eingebettet wird **nur** das Datum – keine Pfade, Namen oder Commit-Kennungen. Cargo führt das Skript neu
//! aus, wenn sich `SOURCE_DATE_EPOCH` oder der ausgecheckte Commit ändert (`HEAD`, Zweig, `packed-refs`).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let date = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .map(civil_date)
        .or_else(|| commit_date(&manifest))
        .unwrap_or_else(|| {
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            civil_date(i64::try_from(secs).unwrap_or(0))
        });
    println!("cargo:rustc-env=PIDA_RELEASE_DATE={date}");
}

/// Commit-Datum (`%cs`, Zeitzone des Commits) samt Auslösern für einen Neubau; `None` ohne Git.
fn commit_date(dir: &Path) -> Option<String> {
    let date = git(dir, &["log", "-1", "--format=%cs"])?;
    if !is_iso_date(&date) {
        return None;
    }
    // Neu ausführen, sobald ein anderer Commit ausgecheckt bzw. auf dem Zweig committet wird
    if let Some(git_dir) = git(dir, &["rev-parse", "--absolute-git-dir"]) {
        let git_dir = Path::new(&git_dir);
        rerun_if_exists(&git_dir.join("HEAD"));
        // jede Bewegung von HEAD (Commit, Checkout, Reset) – auch wenn der Zweig nur in `packed-refs` steht
        rerun_if_exists(&git_dir.join("logs").join("HEAD"));
    }
    if let (Some(common), Some(head_ref)) =
        (git(dir, &["rev-parse", "--git-common-dir"]), git(dir, &["rev-parse", "--symbolic-full-name", "HEAD"]))
    {
        let common = if Path::new(&common).is_absolute() { PathBuf::from(common) } else { dir.join(common) };
        if head_ref.starts_with("refs/") {
            rerun_if_exists(&common.join(&head_ref));
        }
        rerun_if_exists(&common.join("packed-refs"));
    }
    Some(date)
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

/// Nur vorhandene Dateien melden: eine fehlende hielte Cargo für „geändert“ und führte das Skript jedes Mal aus.
fn rerun_if_exists(path: &Path) {
    if path.is_file() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// `JJJJ-MM-TT` mit plausiblen Werten.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && (1..=12).contains(&s[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&s[8..10].parse::<u8>().unwrap_or(0))
}

/// Sekunden seit 1970 → Kalenderdatum (UTC, proleptischer gregorianischer Kalender; H. Hinnant, `civil_from_days`).
fn civil_date(secs: i64) -> String {
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
