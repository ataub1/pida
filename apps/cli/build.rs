//! Drittlizenz-Hinweise für `pida-cli licenses` (Lizenzbericht Befund 3) komprimiert einbetten.
//!
//! Quelle wie bei den Desktop-Paketen (`apps/app/scripts/bundle-config.mjs`): `$PIDA_NOTICES_DIR`, sonst
//! `<repo>/target/notices` mit `THIRD-PARTY-NOTICES.md` und `RUST-STD-COPYRIGHT.html` aus `tools/third-party-notices.sh`.
//! Fehlen sie (Entwicklungs-Build), bettet das Skript leere Daten ein und `licenses` nennt nur den EUPL-Text samt
//! Hinweis; mit `PIDA_REQUIRE_NOTICES=1` (Release) bricht der Build stattdessen ab. Ein relativer `PIDA_NOTICES_DIR`
//! gilt ab der Wurzel des Repositorys. Ohne Feature `notices` (in `run` enthalten; die App schaltet es für ihre
//! Lizenzanzeige ein) liest niemand die Daten – dann tut das Skript nichts.

use std::io::Write;
use std::path::PathBuf;

const FILES: [&str; 2] = ["THIRD-PARTY-NOTICES.md", "RUST-STD-COPYRIGHT.html"];

fn main() {
    println!("cargo:rerun-if-env-changed=PIDA_NOTICES_DIR");
    println!("cargo:rerun-if-env-changed=PIDA_REQUIRE_NOTICES");
    if std::env::var_os("CARGO_FEATURE_NOTICES").is_none() {
        return;
    }
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo = manifest.join("../..");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let explicit = std::env::var_os("PIDA_NOTICES_DIR").filter(|d| !d.is_empty()).map(|d| repo.join(d));
    let dir = explicit.clone().unwrap_or_else(|| repo.join("target/notices"));
    let require = std::env::var("PIDA_REQUIRE_NOTICES").is_ok_and(|v| v == "1");
    // Ordner beobachten, damit später erzeugte Hinweise eingebettet werden. Den Standardordner (in `target/`) dafür
    // anlegen: ein fehlender Pfad ließe Cargo das Skript – und damit pida-cli – bei jedem Build neu bauen.
    if explicit.is_none() && !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut missing = Vec::new();
    for name in FILES {
        let src = dir.join(name);
        let data = match std::fs::read(&src) {
            Ok(data) => {
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
            panic!(
                "PIDA_REQUIRE_NOTICES=1, aber Drittlizenz-Hinweise fehlen (tools/third-party-notices.sh): {missing:?}"
            );
        }
        println!(
            "cargo:warning=ohne Drittlizenz-Hinweise gebaut (fehlten beim Build: {}; tools/third-party-notices.sh)",
            missing.join(", ")
        );
    }
}
