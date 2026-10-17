//! Herausgeber und Urheberrecht der Installer und Pakete (ADR 0004 Nachtrag 21, Entscheidung des Auftraggebers:
//! A-SIT). Tauri übernimmt die Werte aus `tauri.conf.json`:
//!
//! - `bundle.publisher` → Windows-Versionsressource `CompanyName` (tauri-build), NSIS `Publisher` in „Apps &
//!   Features“, MSI `Manufacturer`,
//! - `bundle.copyright` → Windows `LegalCopyright` (Versionsressource und NSIS-Installer), macOS
//!   `NSHumanReadableCopyright`,
//! - `bundle.homepage` → deb `Homepage`, rpm `URL`, NSIS/MSI „Info-Link“,
//! - Cargo `authors` → deb `Maintainer`.
//!
//! iOS: das versionierte Xcode-Projekt (`gen/apple`) trägt `NSHumanReadableCopyright` selbst (Info.plist und
//! `project.yml` für xcodegen). Android kennt kein Copyright-Feld im Manifest; dort steht der Vermerk in „Über
//! PIDA“.

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("JSON")
}

const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const IOS_INFO_PLIST: &str = include_str!("../gen/apple/pida-app_iOS/Info.plist");
const IOS_PROJECT: &str = include_str!("../gen/apple/project.yml");
const WORKSPACE: &str = include_str!("../../../../Cargo.toml");

#[test]
fn installers_name_a_sit_as_publisher_and_copyright_holder() {
    let conf = json(TAURI_CONF);
    let bundle = &conf["bundle"];
    assert_eq!(bundle["publisher"], "A-SIT", "Herausgeber (CompanyName, Publisher, Manufacturer)");
    let copyright = bundle["copyright"].as_str().expect("bundle.copyright");
    assert!(
        copyright.starts_with("© ") && copyright.contains(" A-SIT") && copyright.ends_with("EUPL-1.2"),
        "{copyright}"
    );
    let year: u16 = copyright[3..7].parse().expect("Jahr im Urheberrechtsvermerk");
    assert!(year >= 2026, "{copyright}");
    assert_eq!(bundle["license"], "EUPL-1.2");
    assert_eq!(bundle["homepage"], "https://github.com/ataub1/pida");
    assert!(!TAURI_CONF.contains("PIDA contributors") && !WORKSPACE.contains("PIDA contributors"));

    // iOS: dieselbe Angabe im Projekt (Info.plist und xcodegen-Vorlage)
    assert!(
        IOS_INFO_PLIST.contains(&format!("<key>NSHumanReadableCopyright</key>\n\t<string>{copyright}</string>")),
        "Info.plist ohne NSHumanReadableCopyright „{copyright}“"
    );
    assert!(IOS_PROJECT.contains(&format!("NSHumanReadableCopyright: \"{copyright}\"")), "project.yml");
}

/// Cargo-Metadaten (deb `Maintainer`, `cargo metadata`): Urheber A-SIT, Repository auf GitHub.
#[test]
fn cargo_metadata_names_a_sit() {
    // `authors.workspace = true` in der Shell (wie in allen Crates)
    assert_eq!(env!("CARGO_PKG_AUTHORS"), "A-SIT");
    assert_eq!(env!("CARGO_PKG_LICENSE"), "EUPL-1.2");
    assert!(WORKSPACE.contains("\nauthors = [\"A-SIT\"]\n"), "[workspace.package] authors");
    assert!(
        WORKSPACE.contains("\nrepository = \"https://github.com/ataub1/pida\"\n"),
        "[workspace.package] repository"
    );
}
