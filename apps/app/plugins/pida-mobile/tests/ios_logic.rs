//! Reine Dateilogik der iOS-Seite (`ios/Sources/SandboxFiles.swift`) auf dem Mac geprüft, ohne Simulator: übersetzt
//! mit `xcrun swiftc` zusammen mit `ios/Tests/SandboxFilesCheck.swift` und führt sie aus (Mehrfachauswahl, ADR 0004
//! Nachtrag 17: alle gewählten Dateien in der Reihenfolge der Auswahl, gleiche Namen in eigenen Unterordnern,
//! nicht kopierbare als `skipped`). Ohne macOS bzw. Xcode übersprungen.

use std::path::Path;
use std::process::Command;

#[test]
fn ios_sandbox_files_copy_all_in_order_and_count_skipped() {
    let swiftc = Command::new("xcrun").args(["--find", "swiftc"]).output().is_ok_and(|o| o.status.success());
    if !cfg!(target_os = "macos") || !swiftc {
        eprintln!("übersprungen: kein macOS mit Xcode");
        return;
    }
    let ios = Path::new(env!("CARGO_MANIFEST_DIR")).join("ios");
    let tmp = tempdir();
    let exe = tmp.join("sandbox-files-check");
    let out = Command::new("xcrun")
        .args(["swiftc", "-parse-as-library", "-o"])
        .arg(&exe)
        .arg(ios.join("Sources/SandboxFiles.swift"))
        .arg(ios.join("Sources/SecureStore.swift"))
        .arg(ios.join("Tests/SandboxFilesCheck.swift"))
        .output()
        .expect("swiftc");
    assert!(out.status.success(), "swiftc: {}", String::from_utf8_lossy(&out.stderr));
    let run = Command::new(&exe).arg(tmp.join("daten")).output().expect("Prüfung");
    assert!(run.status.success(), "{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    assert!(String::from_utf8_lossy(&run.stdout).contains("SandboxFilesCheck ok"));
    let _ = std::fs::remove_dir_all(tmp);
}

fn tempdir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pida-ios-logic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
