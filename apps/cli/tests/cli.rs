//! Tests der fertigen Binary (`pdfover-cli`): Hilfe zweisprachig, Quirks der PDF-Over-Argumente,
//! Exit-Codes, Subcommands, `--json`. Jeder Aufruf läuft mit eigenem `HOME` (Konfiguration,
//! Logdatei, `~/.pdf-over` isoliert) und ohne Terminal (keine Rückfragen).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata");

struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Home {
        Home { dir: tempfile::tempdir().unwrap() }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn cmd(&self, lang: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_pdfover-cli"))
            .args(args)
            .env("HOME", self.path())
            // Windows: Profilordner nur über PDFOVER_HOME umlenkbar (Known-Folder-API)
            .env("PDFOVER_HOME", self.path())
            .env("XDG_CONFIG_HOME", self.path().join(".config"))
            .env("XDG_DATA_HOME", self.path().join(".local/share"))
            .env("XDG_STATE_HOME", self.path().join(".local/state"))
            .env("XDG_CACHE_HOME", self.path().join(".cache"))
            .env("LANG", lang)
            .env("LC_ALL", lang)
            .env_remove("LC_MESSAGES")
            .env_remove("RUST_LOG")
            .stdin(Stdio::null())
            .output()
            .expect("Binary startet")
    }

    fn pdf(&self, name: &str) -> PathBuf {
        let d = self.path().join("in");
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(name);
        std::fs::copy(Path::new(TESTDATA).join("pdf/generated/simple-xref-table.pdf"), &p).unwrap();
        p
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const DE: &str = "de_AT.UTF-8";
const EN: &str = "en_US.UTF-8";

#[test]
fn help_in_german_and_english() {
    let h = Home::new();
    for flag in ["-h", "-?", "--help"] {
        let o = h.cmd(DE, &[flag]);
        assert_eq!(o.status.code(), Some(0), "{flag}");
        let s = stdout(&o);
        assert!(s.contains("Optionen"), "{s}");
        assert!(s.contains("-kskeypass") && s.contains("--kspass-env") && s.contains("render-block"));
    }
    let o = h.cmd(EN, &["x.pdf", "-h"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).contains("Options"), "{}", stdout(&o));
}

#[test]
fn version() {
    let h = Home::new();
    let o = h.cmd(EN, &["--version"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_token_is_an_input_file_quirk() {
    let h = Home::new();
    let o = h.cmd(DE, &["-x"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("-x"), "{}", stderr(&o));
    assert!(stderr(&o).contains("existiert nicht"), "{}", stderr(&o));
}

#[test]
fn usage_errors_exit_2() {
    let h = Home::new();
    let a = h.pdf("a.pdf");
    let a = a.to_str().unwrap();
    // -b ist case-sensitive
    assert_eq!(h.cmd(EN, &["-b", "mobile", a]).status.code(), Some(2));
    // -p 5–200 Zeichen
    let o = h.cmd(EN, &["-p", "abc", a]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).to_lowercase().contains("password"), "{}", stderr(&o));
    // abweichendes -kskeypass
    let o = h.cmd(DE, &["-kspass", "a", "-kskeypass", "b", a]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("PKCS#12"), "{}", stderr(&o));
    // fehlende Umgebungsvariable
    let o = h.cmd(EN, &["--kspass-env", "PDFOVER_NICHT_GESETZT_4711", a]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("PDFOVER_NICHT_GESETZT_4711"));
    // -c fehlt → Fehler statt Werksreset
    let o = h.cmd(DE, &["-c", "fehlt.config", a]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("fehlt.config"), "{}", stderr(&o));
    // -proxyport außerhalb 1–65535
    assert_eq!(h.cmd(EN, &["-proxyport", "0", a]).status.code(), Some(2));
    // -c ohne Wert
    assert_eq!(h.cmd(EN, &["-c"]).status.code(), Some(2));
    // keine Eingabe
    assert_eq!(h.cmd(EN, &[]).status.code(), Some(2));
}

#[test]
fn profiles_text_and_json() {
    let h = Home::new();
    let o = h.cmd(EN, &["profiles"]);
    assert_eq!(o.status.code(), Some(0));
    let lines: Vec<String> = stdout(&o).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 12);
    assert!(lines.iter().any(|l| l == "AMTSSIGNATURBLOCK_DE_RECOMMENDED"));
    let o = h.cmd(EN, &["--json", "profiles"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 12);
}

fn tool(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok_and(|o| o.status.success())
}

#[test]
fn render_block_pdf_and_png() {
    let h = Home::new();
    let out = h.path().join("block.pdf");
    let o = h.cmd(EN, &["render-block", "--profile", "SIGNATURBLOCK_SMALL_DE_NOTE", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF-"));
    if tool("qpdf") {
        let q = Command::new("qpdf").arg("--check").arg(&out).output().unwrap();
        assert!(q.status.success(), "qpdf: {}", String::from_utf8_lossy(&q.stdout));
    }
    let o = h.cmd(EN, &["render-block", "--profile", "UNBEKANNT", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(2));
    if let Some(dir) = std::env::var_os("PDFOVER_PDFIUM_DIR") {
        let png = h.path().join("block.png");
        let o = Command::new(env!("CARGO_BIN_EXE_pdfover-cli"))
            .args(["render-block", "--profile", "AMTSSIGNATURBLOCK_EN_RECOMMENDED", png.to_str().unwrap()])
            .env("HOME", h.path())
            .env("PDFOVER_PDFIUM_DIR", dir)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));
    }
}

#[test]
fn verify_signed_unsigned_and_broken() {
    let h = Home::new();
    let signed = format!("{TESTDATA}/pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf");
    let o = h.cmd(EN, &["--json", "verify", &signed]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["valid"], true);
    assert!(v["signatures"].as_array().unwrap().len() >= 2);
    let unsigned = h.pdf("u.pdf");
    let o = h.cmd(DE, &["verify", unsigned.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).contains("Keine Signaturen"), "{}", stdout(&o));
    let broken = h.path().join("b.pdf");
    std::fs::write(&broken, b"nope").unwrap();
    assert_eq!(h.cmd(EN, &["verify", broken.to_str().unwrap()]).status.code(), Some(3));
    if tool("openssl") {
        // Gegenprobe: dieselbe Datei mit openssl cms ist nicht Teil der CLI; der Verifier ist in
        // pdfover-cades gegen openssl geprüft.
    }
}

#[test]
fn migrate_dry_run_and_apply() {
    let h = Home::new();
    let legacy = h.path().join(".pdf-over");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::copy(
        Path::new(TESTDATA).join("config/pdf-over-generated-en-keystore.config"),
        legacy.join("PDF-Over.config"),
    )
    .unwrap();
    let o = h.cmd(EN, &["--json", "migrate", "--dry-run"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["dry_run"], true);
    assert!(v["imported"].as_u64().unwrap() > 5);
    assert!(v["settings_file"].is_null());
    let o = h.cmd(EN, &["migrate"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("settings.toml"), "{}", stdout(&o));
    // erneut: Einstellungen existieren bereits → Fehler, nichts überschrieben
    assert_eq!(h.cmd(EN, &["migrate"]).status.code(), Some(1));
}

#[test]
fn headless_without_pades_fails_cleanly_or_signs() {
    // Ohne echte PAdES-Engine (Welle 2: Stub) endet der Lauf mit einem Fehlercode statt Absturz.
    let h = Home::new();
    let a = h.pdf("a.pdf");
    let ks = format!("{TESTDATA}/keystores/test-rsa2048.p12");
    let o = h.cmd(EN, &["--headless", "--json", "-ks", &ks, "-kspass", "test", a.to_str().unwrap()]);
    let code = o.status.code();
    assert!(matches!(code, Some(0) | Some(1)), "{code:?} {}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["documents"].as_array().unwrap().len(), 1);
}

#[test]
fn headless_keystore_signing_with_real_engine() {
    let h = Home::new();
    let a = h.pdf("a.pdf");
    let ks = format!("{TESTDATA}/keystores/test-rsa2048.p12");
    let o = h.cmd(EN, &["--headless", "-ks", &ks, "-kspass", "test", a.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = a.with_file_name("a_signed.pdf");
    assert!(out.is_file());
    if tool("qpdf") {
        assert!(Command::new("qpdf").arg("--check").arg(&out).status().unwrap().success());
    }
    let v = h.cmd(EN, &["--json", "verify", out.to_str().unwrap()]);
    assert_eq!(v.status.code(), Some(0));
}
