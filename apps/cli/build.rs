//! Drittlizenz-Hinweise für `pida-cli licenses` (Lizenzbericht Befund 3) komprimiert einbetten.
//!
//! Quelle wie bei den Desktop-Paketen (`apps/app/scripts/bundle-config.mjs`): `$PIDA_NOTICES_DIR`, sonst
//! `<repo>/target/notices` mit `THIRD-PARTY-NOTICES.md` und `RUST-STD-COPYRIGHT.html` aus `tools/third-party-notices.sh`.
//! Fehlen sie (Entwicklungs-Build), bettet das Skript leere Daten ein und `licenses` nennt nur den EUPL-Text samt
//! Hinweis; mit `PIDA_REQUIRE_NOTICES=1` (Release) bricht der Build stattdessen ab.

use std::io::Write;
use std::path::PathBuf;

const FILES: [&str; 2] = ["THIRD-PARTY-NOTICES.md", "RUST-STD-COPYRIGHT.html"];

fn main() {
    println!("cargo:rerun-if-env-changed=PIDA_NOTICES_DIR");
    println!("cargo:rerun-if-env-changed=PIDA_REQUIRE_NOTICES");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let dir = std::env::var_os("PIDA_NOTICES_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../target/notices"));
    let require = std::env::var("PIDA_REQUIRE_NOTICES").is_ok_and(|v| v == "1");
    // nur vorhandene Pfade beobachten: ein fehlender Pfad ließe Cargo das Skript bei jedem Build neu ausführen
    if dir.is_dir() {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
    let mut missing = Vec::new();
    for name in FILES {
        let src = dir.join(name);
        let data = match std::fs::read(&src) {
            Ok(data) => {
                println!("cargo:rerun-if-changed={}", src.display());
                let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
                enc.write_all(&data).expect("deflate");
                enc.finish().expect("deflate")
            }
            Err(_) => {
                missing.push(src.display().to_string());
                Vec::new()
            }
        };
        std::fs::write(out.join(format!("{name}.deflate")), data).expect("OUT_DIR schreiben");
    }
    if !missing.is_empty() {
        if require {
            panic!("PIDA_REQUIRE_NOTICES=1, aber Drittlizenz-Hinweise fehlen (tools/third-party-notices.sh): {missing:?}");
        }
        println!("cargo:warning=pida-cli ohne Drittlizenz-Hinweise gebaut (fehlt: {})", missing.join(", "));
    }
}
