//! Versionsdatum aus `build.rs` (ADR 0004 Nachtrag 21): Auswahl der Quelle (`build_support/release_date.rs`) –
//! `SOURCE_DATE_EPOCH`, Git (nur das Repository, das das Crate verwaltet), Dateizeit, Datum des Bauens –, unbrauchbare
//! Angaben vor 2026 und ein ausgepacktes Quellarchiv in einem fremden Git-Repository.

#[path = "../build_support/release_date.rs"]
mod release_date;

use std::path::Path;
use std::process::Command;

use release_date::{Origin, civil_date, git_commit_date, is_iso_date, select};

/// 17.10.2026 12:00 UTC
const OCT_17_2026: i64 = 1_792_238_400;

#[test]
fn calendar_dates_in_utc() {
    for (secs, date) in [
        (0, "1970-01-01"),
        (-86_400, "1969-12-31"),
        (315_532_800, "1980-01-01"),
        (951_782_400, "2000-02-29"),
        (1_760_659_200, "2025-10-17"),
        (OCT_17_2026, "2026-10-17"),
        (4_102_444_800, "2100-01-01"),
    ] {
        assert_eq!(civil_date(secs), date, "{secs}");
    }
    assert!(is_iso_date("2026-10-17") && !is_iso_date("2026-13-01") && !is_iso_date("17.10.2026"));
}

#[test]
fn source_selection_order_and_implausible_values() {
    let now = OCT_17_2026 + 30 * 86_400;
    // SOURCE_DATE_EPOCH vor Git
    assert_eq!(
        select(Some(" 1792238400\n"), Some("2026-09-01"), None, now),
        ("2026-10-17".into(), Origin::SourceDateEpoch)
    );
    // unbrauchbares SOURCE_DATE_EPOCH (0, 1980 als Vorgabe mancher Paketbauer, kein Zahlwert) → Git
    for sde in ["0", "315532800", "", "gestern", "-1"] {
        assert_eq!(select(Some(sde), Some("2026-09-01"), None, now), ("2026-09-01".into(), Origin::Git), "{sde:?}");
    }
    // Git unbrauchbar → Dateizeit → Datum des Bauens
    assert_eq!(select(None, Some("1999-01-01"), Some(OCT_17_2026), now), ("2026-10-17".into(), Origin::FileTime));
    assert_eq!(select(None, Some("kaputt"), None, now).1, Origin::BuildTime);
    assert_eq!(
        select(None, None, Some(1), now),
        (civil_date(now), Origin::BuildTime),
        "Dateizeit 1970 (z. B. Nix-Store)"
    );
    assert_eq!(select(None, None, None, now), ("2026-11-16".into(), Origin::BuildTime));
}

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn run_git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_DATE", "2026-03-04T10:00:00+01:00")
        .env("GIT_COMMITTER_DATE", "2026-03-04T10:00:00+01:00")
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Ein Quellarchiv (ohne `.git`), ausgepackt in einem fremden Repository: nicht dessen Commit-Datum übernehmen; erst
/// wenn das Crate dort eingecheckt ist, gilt das Commit-Datum (mit den Dateien für einen Neubau).
#[test]
fn git_date_only_from_the_repository_that_tracks_the_crate() {
    if !have_git() {
        eprintln!("git nicht verfügbar – übersprungen");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    run_git(repo, &["init", "-q"]);
    std::fs::write(repo.join("README"), "fremdes Repository").unwrap();
    run_git(repo, &["add", "README"]);
    run_git(repo, &["commit", "-q", "-m", "fremd"]);
    let krate = repo.join("downloads").join("pida-0.2.0").join("crates").join("core-api");
    std::fs::create_dir_all(&krate).unwrap();
    std::fs::write(krate.join("Cargo.toml"), "[package]\nname = \"pida-core-api\"\n").unwrap();
    assert_eq!(git_commit_date(&krate), None, "nicht eingechecktes Crate: kein fremdes Commit-Datum");

    run_git(repo, &["add", "downloads"]);
    run_git(repo, &["commit", "-q", "-m", "pida"]);
    let (date, rerun) = git_commit_date(&krate).expect("eingechecktes Crate");
    assert_eq!(date, "2026-03-04");
    assert!(rerun.iter().any(|p| p.ends_with("HEAD")), "{rerun:?}");
    assert!(rerun.iter().all(|p| p.is_file()), "nur vorhandene Dateien: {rerun:?}");

    // ganz ohne Repository
    let plain = tempfile::tempdir().unwrap();
    std::fs::write(plain.path().join("Cargo.toml"), "").unwrap();
    assert_eq!(git_commit_date(plain.path()), None);
    // dann die Dateizeit von `Cargo.toml` (hier: eben geschrieben)
    let mtime = release_date::file_time(&plain.path().join("Cargo.toml")).expect("Dateizeit");
    assert!(release_date::plausible(&civil_date(mtime)));
}

/// Der gebaute Stand: aus Git (wie in der CI) bzw. `SOURCE_DATE_EPOCH` – nie vor 2026.
#[test]
fn embedded_date_matches_the_selection() {
    let date = pida_core_api::update::RELEASE_DATE;
    assert!(is_iso_date(date) && release_date::plausible(date), "{date}");
}
