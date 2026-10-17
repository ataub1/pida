//! Build-Skript von `pida-core-api`: bettet das **Versionsdatum** ein (`PIDA_RELEASE_DATE`, `JJJJ-MM-TT`;
//! „Über PIDA“, ADR 0004 Nachtrag 21). Die Auswahl steht in `build_support/release_date.rs` (dort beschrieben und von
//! `tests/release_date.rs` geprüft): `SOURCE_DATE_EPOCH`, sonst das Commit-Datum des gebauten Stands (nur aus dem
//! Repository, das dieses Crate verwaltet), ohne Git die Dateizeit von `Cargo.toml`, zuletzt das Datum des Bauens;
//! Angaben vor 2026 zählen nicht.
//!
//! Eingebettet wird **nur** das Datum – keine Pfade, Namen oder Commit-Kennungen. Cargo führt das Skript neu
//! aus, wenn sich `SOURCE_DATE_EPOCH` oder der ausgecheckte Commit ändert (`HEAD`, Zweig, `packed-refs`) bzw. ohne
//! Git `Cargo.toml`.

#[path = "build_support/release_date.rs"]
mod release_date;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support/release_date.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let source_date_epoch = std::env::var("SOURCE_DATE_EPOCH").ok();
    let git = release_date::git_commit_date(&manifest);
    let cargo_toml = manifest.join("Cargo.toml");
    let file_time = if git.is_none() { release_date::file_time(&cargo_toml) } else { None };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (date, source) = release_date::select(
        source_date_epoch.as_deref(),
        git.as_ref().map(|(d, _)| d.as_str()),
        file_time,
        i64::try_from(now).unwrap_or(0),
    );
    match &git {
        Some((_, rerun)) => {
            for path in rerun {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
        None => println!("cargo:rerun-if-changed=Cargo.toml"),
    }
    if source == release_date::Origin::BuildTime {
        println!(
            "cargo:warning=Versionsdatum: weder SOURCE_DATE_EPOCH noch Git noch Dateizeit – Datum des Bauens ({date})"
        );
    }
    println!("cargo:rustc-env=PIDA_RELEASE_DATE={date}");
}
