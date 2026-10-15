//! Ende-zu-Ende (Paket A): Increment → ByteRange-Daten mit `openssl cms -sign` (Test-PKCS#12)
//! signieren → CMS einbetten → mit pyHanko validieren.
//!
//! Erwartung je Datei: neue Signatur intakt und kryptografisch gültig, deckt die ganze Datei ab;
//! jede Vorsignatur behält Integrität/Gültigkeit; die bisher letzte Vorsignatur (deckte die ganze
//! Datei ab) erreicht höchstens `FORM_FILLING`, ältere werden nicht schlechter als vorher.
//! Zusätzlich `openssl cms -verify` über die ByteRange-Daten, `qpdf --check` und die pyHanko-CLI
//! (`pyhanko sign validate --pretty-print --no-strict-syntax`, ohne Netz) als zweites Orakel.
//!
//! Fehlen openssl oder pyHanko, wird der Test übersprungen (Meldung auf stderr). Das Ergebnis je
//! Datei steht in `$CARGO_TARGET_TMPDIR/pdf-incremental-RESULTS.md`; mit
//! `PIDA_UPDATE_RESULTS=1` wird `testdata/pdf-incremental/RESULTS.md` aktualisiert.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use pida_pdf_cos::{CosError, Limits, SourcePdf, XrefKind};
use pida_pdf_incremental::{IncrementOptions, RESERVE_MIN_MODEL_A, XrefFormat};

struct Signer {
    openssl: PathBuf,
    key: PathBuf,
    cert: PathBuf,
}

fn prepare_signer(dir: &Path) -> Option<Signer> {
    let openssl = common::which("openssl")?;
    let p12 = common::testdata().join("keystores/test-rsa2048.p12");
    let key = dir.join("key.pem");
    let cert = dir.join("cert.pem");
    let run = |args: &[&str], out: &Path| {
        Command::new(&openssl)
            .args(["pkcs12", "-in"])
            .arg(&p12)
            .args(["-passin", "pass:test"])
            .args(args)
            .arg("-out")
            .arg(out)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if !run(&["-nodes", "-nocerts"], &key) || !run(&["-nokeys", "-clcerts"], &cert) {
        eprintln!("HINWEIS: Test-PKCS#12 nicht lesbar – E2E übersprungen");
        return None;
    }
    Some(Signer { openssl, key, cert })
}

impl Signer {
    /// `openssl cms -sign -binary -outform DER -nosmimecap -md sha256` über `data`.
    fn sign(&self, dir: &Path, data: &[u8]) -> Result<Vec<u8>, String> {
        let input = dir.join("data.bin");
        let output = dir.join("cms.der");
        std::fs::write(&input, data).map_err(|e| e.to_string())?;
        let o = Command::new(&self.openssl)
            .args(["cms", "-sign", "-binary", "-outform", "DER", "-nosmimecap", "-md", "sha256", "-in"])
            .arg(&input)
            .arg("-signer")
            .arg(&self.cert)
            .arg("-inkey")
            .arg(&self.key)
            .arg("-out")
            .arg(&output)
            .output()
            .map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(String::from_utf8_lossy(&o.stderr).into_owned());
        }
        std::fs::read(&output).map_err(|e| e.to_string())
    }

    /// `openssl cms -verify -binary -noverify` (Integrität ohne Vertrauenskette).
    fn verify(&self, dir: &Path, cms: &[u8], data: &[u8]) -> bool {
        let c = dir.join("verify.der");
        let d = dir.join("verify.bin");
        let o = dir.join("verify.out");
        if std::fs::write(&c, cms).is_err() || std::fs::write(&d, data).is_err() {
            return false;
        }
        Command::new(&self.openssl)
            .args(["cms", "-verify", "-binary", "-inform", "DER", "-noverify", "-in"])
            .arg(&c)
            .arg("-content")
            .arg(&d)
            .arg("-out")
            .arg(&o)
            .output()
            .is_ok_and(|r| r.status.success())
    }
}

struct WorkPath(PathBuf);
impl WorkPath {
    fn path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, Clone)]
struct Row {
    file: String,
    xref: String,
    encryption: String,
    prior: usize,
    qpdf: String,
    openssl: bool,
    new_sig: String,
    prior_after: String,
    cli: String,
    ok: bool,
    problems: Vec<String>,
}

fn xref_name(k: XrefKind) -> &'static str {
    match k {
        XrefKind::Table => "Tabelle",
        XrefKind::Stream => "xref-Stream",
        XrefKind::Hybrid => "Hybrid",
    }
}

fn run_one(
    path: &Path,
    signer: &Signer,
    python: &Path,
    qpdf: Option<&Path>,
    pyhanko: Option<&Path>,
    work: &Path,
) -> Option<Row> {
    let name = common::rel(path);
    let src = match SourcePdf::open_file(path, &Limits::default()) {
        Ok(s) => s,
        Err(CosError::PasswordRequired) if common::expected_open_error(&name) => return None,
        Err(e) => panic!("{name}: öffnen: {e}"),
    };
    let dir = work.join(name.replace(['/', '\\'], "_"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut problems = Vec::new();

    let before = common::pyhanko_report(python, path, &[]).unwrap_or_else(|e| panic!("{name}: pyHanko (vorher): {e}"));
    let mut inc = common::add_dummy_signature(&src, IncrementOptions::default(), 2 * RESERVE_MIN_MODEL_A)
        .unwrap_or_else(|e| panic!("{name}: Increment: {e}"));
    let data = inc.byte_range_data(src.bytes());
    let cms = signer.sign(&dir, &data).unwrap_or_else(|e| panic!("{name}: openssl cms -sign: {e}"));
    inc.embed_contents(&cms).unwrap_or_else(|e| panic!("{name}: einbetten: {e}"));
    let out = inc.to_vec(src.bytes());
    assert_eq!(&out[..src.bytes().len()], src.bytes(), "{name}: Präfix");
    // Die eingebettete Signatur ändert die signierten Bytes nicht.
    assert_eq!(inc.byte_range_data(src.bytes()), data, "{name}: ByteRange-Daten verändert");
    let signed = dir.join("signed.pdf");
    std::fs::write(&signed, &out).unwrap();

    let openssl = signer.verify(&dir, &cms, &data);
    if !openssl {
        problems.push("openssl cms -verify fehlgeschlagen".into());
    }

    let qpdf_s = match qpdf {
        Some(q) => {
            let b = common::qpdf_check(q, path);
            let a = common::qpdf_check(q, &signed);
            let new: Vec<_> = a.messages.iter().filter(|m| !b.messages.contains(m)).cloned().collect();
            if !(a.code == 0 || a.code == 3) || !new.is_empty() {
                problems.push(format!("qpdf Code {} neue Befunde {new:?}", a.code));
            }
            if a.code == 0 { "ok".to_string() } else { format!("Warnungen wie Original ({})", a.code) }
        }
        None => "nicht installiert".into(),
    };

    let after =
        common::pyhanko_report(python, &signed, &[&signer.cert]).unwrap_or_else(|e| panic!("{name}: pyHanko: {e}"));
    let n = common::next_field_number(&src.signature_fields().unwrap());
    let new_name = format!("PDF-AS Signatur{n}");
    let new_sig = after.iter().find(|s| s.field == new_name);
    let new_desc = match new_sig {
        Some(s) => {
            if !(s.intact && s.valid) {
                problems.push(format!("neue Signatur nicht intakt/gültig: {s:?}"));
            }
            if s.coverage.as_deref() != Some("ENTIRE_FILE") {
                problems.push(format!("neue Signatur deckt nicht die ganze Datei ab: {:?}", s.coverage));
            }
            if s.error.is_some() {
                problems.push(format!("pyHanko-Fehler: {:?}", s.error));
            }
            format!(
                "{} / {} / {}",
                if s.intact { "INTACT" } else { "BROKEN" },
                if s.valid { "VALID" } else { "INVALID" },
                s.coverage.as_deref().unwrap_or("-")
            )
        }
        None => {
            problems.push(format!("neue Signatur {new_name} nicht gefunden"));
            "fehlt".into()
        }
    };

    // Vorsignaturen: Status unverändert; die zuletzt vollständige höchstens FORM_FILLING.
    let mut prior_desc = Vec::new();
    for b in &before {
        let Some(a) = after.iter().find(|s| s.field == b.field) else {
            problems.push(format!("Vorsignatur {} verschwunden", b.field));
            continue;
        };
        if a.intact != b.intact || a.valid != b.valid {
            problems.push(format!(
                "Vorsignatur {}: Status {:?}/{:?} → {:?}/{:?}",
                b.field, b.intact, b.valid, a.intact, a.valid
            ));
        }
        let rb = common::modification_rank(b.modification_level.as_deref());
        let ra = common::modification_rank(a.modification_level.as_deref());
        let limit = rb.max(common::modification_rank(Some("FORM_FILLING")));
        if ra > limit {
            problems.push(format!(
                "Vorsignatur {}: Änderungsstufe {:?} → {:?}",
                b.field, b.modification_level, a.modification_level
            ));
        }
        prior_desc.push(format!(
            "{}: {} → {}",
            b.field,
            b.modification_level.as_deref().unwrap_or("-"),
            a.modification_level.as_deref().unwrap_or("-")
        ));
    }

    // Zweites Orakel: die pyHanko-CLI wie im Auftrag (`sign validate --pretty-print
    // --no-strict-syntax`), Textausgabe je Feld.
    let cli = match pyhanko {
        Some(ph) => match common::pyhanko_cli(ph, &signed, &signer.cert) {
            Ok(sigs) => {
                match sigs.iter().find(|s| s.field == new_name) {
                    Some(s) if s.sound && s.entire_file && s.judged_valid && !s.illegitimate_changes => {}
                    other => problems.push(format!("pyHanko-CLI: neue Signatur {other:?}")),
                }
                for b in &before {
                    let Some(c) = sigs.iter().find(|s| s.field == b.field) else {
                        problems.push(format!("pyHanko-CLI: Vorsignatur {} fehlt", b.field));
                        continue;
                    };
                    if c.sound != (b.intact && b.valid) {
                        problems.push(format!("pyHanko-CLI: Vorsignatur {}: {c:?}", b.field));
                    }
                    // die zuvor vollständig abdeckende Vorsignatur: nur Formularänderungen
                    if b.modification_level.as_deref() == Some("NONE") && c.illegitimate_changes {
                        problems.push(format!("pyHanko-CLI: Vorsignatur {}: unzulässige Änderungen", b.field));
                    }
                }
                let new = if sigs.iter().any(|s| s.field == new_name && s.judged_valid) { "VALID" } else { "FEHLER" };
                let prior: Vec<_> = sigs.iter().filter(|s| s.field != new_name).collect();
                let sound = prior.iter().filter(|s| s.sound).count();
                let clean = prior.iter().filter(|s| !s.illegitimate_changes).count();
                let n = prior.len();
                if n == 0 {
                    new.to_string()
                } else {
                    format!("{new}; Vorsig. sound {sound}/{n}, ohne „illegitimate“ {clean}/{n}")
                }
            }
            Err(e) => {
                problems.push(format!("pyHanko-CLI: {e}"));
                "Fehler".into()
            }
        },
        None => "nicht installiert".into(),
    };

    let encryption = match src.encryption() {
        Some(e) => format!("{:?} (V{} R{})", e.algorithm, e.v, e.r),
        None => "–".into(),
    };
    Some(Row {
        file: name,
        xref: xref_name(src.xref_kind()).into(),
        encryption,
        prior: before.len(),
        qpdf: qpdf_s,
        openssl,
        new_sig: new_desc,
        prior_after: if prior_desc.is_empty() { "–".into() } else { prior_desc.join("; ") },
        cli,
        ok: problems.is_empty(),
        problems,
    })
}

fn tool_versions(signer: &Signer, python: &Path, qpdf: Option<&Path>) -> String {
    let first_line = |cmd: &mut Command| -> String {
        cmd.output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string())
            .unwrap_or_default()
    };
    let openssl = first_line(Command::new(&signer.openssl).arg("version"));
    let pyhanko = first_line(Command::new(python).args([
        "-I",
        "-c",
        "import importlib.metadata as m; print(m.version('pyhanko'))",
    ]));
    let qpdf = qpdf.map(|q| first_line(Command::new(q).arg("--version"))).unwrap_or_else(|| "–".into());
    format!("Werkzeuge: {openssl}; {qpdf}; pyHanko {pyhanko}.\n\n")
}

const FURTHER_CHECKS: &str = "## Weitere Prüfungen (gleiche Testläufe)\n\n\
- `tests/corpus_increment.rs`: jede Datei → Präfix-Invariante (I1), CRLF-Beginn, ByteRange `[0 a b c]` mit \
`b + c` = Dateilänge, `<`/`>` an `a` bzw. `b − 1`, `0`-gefüllter Platzhalter, 36-Zeichen-Feld mit Leerzeichen \
(I2), Re-Parse mit pdf-cos **und** lopdf (I5), Querverweis im Format der Vorrevision mit `/Prev`, `/ID[0]` \
beibehalten, `/ID[1]` neu, `/Encrypt` übernommen, `/Contents` unverschlüsselt, Schlüsselreihenfolge des \
Signaturdictionarys wie PDF-AS, `qpdf --check` ohne neue Befunde.\n\
- Mehrfachsignatur: drei aufeinanderfolgende Increments je Tabelle, xref-Stream, Hybrid, linearisiert, AES-256, \
RC4-128 und vorsigniert (`manySignFields_signed4.pdf`); jede Runde endet an einer Revisionsgrenze.\n\
- Erzwungene Formate (Tabelle, xref-Stream, Hybrid) auf `POS_LAST_ALREADY_SIGNED/example.pdf`: neue Signatur \
intakt/gültig, Vorsignatur höchstens `FORM_FILLING`, `qpdf --check` ohne Befund.\n\
- Befüllen eines vorhandenen leeren Signaturfelds (`manySignFields.pdf` mit und ohne Object Streams): `/Rect` \
bleibt, Signatur intakt und gültig.\n\
- `tests/builder.rs`: Fehlerfälle, Längenprüfung (N passt, N+1 und 2N abgewiesen), Platzhalter-Layout, \
Verschlüsselung neuer Objekte (RC4-128, AES-128, AES-256, xref-Stream), unverschlüsselte und ungefilterte \
XMP-Metadaten bei `EncryptMetadata false`, Originalschreibweise unveränderter Reals, Vorspann vor dem Header, \
korrigierter `startxref` (K2), rekonstruierter Querverweis wird abgelehnt, `/Contents`-Reserve über \
`RESERVE_MAX`, inkonsistente `Increment`-Bereiche.\n\
- `edge_case_inputs_validate`: Leerraum bzw. Fremdinhalt nach `%%EOF`, fehlendes EOL, falscher `startxref` – \
neue Signatur für pyHanko intakt/gültig über die ganze Datei, `qpdf --check` ohne neue Befunde.\n\
- `tests/robustness.rs` (beide Crates): Mutations-Fuzzing über den Korpus (öffnen, alle Auswertungen, \
Increment bauen, erneut öffnen – keine Panics, I1/I2 gelten), `/Size` am Ende des Objektnummernraums, \
Laufzeit-/Speicherfallen (viele `%%EOF`, Streams ohne `endstream`, Form-Auffächerung, viele Bild-Aufrufe, \
wiederholte Inhalts-Streams, unzulässige Predictor-Parameter).\n\n\
Einmalig von Hand gegengeprüft (Review 14.10.2026): PDFBox 2.0.34 – der Leser von PDF-AS – lädt alle 65 \
signierten Ausgaben und findet jede neue Signatur mit konsistenter ByteRange (`[0 a b c]`, `b + c` = \
Dateilänge, `<`/`>` an den Grenzen).\n";

fn render(rows: &[Row], versions: &str) -> String {
    let mut s = String::new();
    s.push_str("# pdf-incremental – Ergebnisse Ende-zu-Ende (Paket A)\n\n");
    s.push_str("Erzeugt von `cargo test -p pida-pdf-incremental --test e2e_sign` ");
    s.push_str("(mit `PIDA_UPDATE_RESULTS=1` wird diese Datei aktualisiert).\n\n");
    s.push_str("Ablauf je Datei: `SourcePdf::open` → Increment mit Signaturdictionary (Reserve 8192 Byte) und ");
    s.push_str("leerem, unsichtbarem Widget `PDF-AS Signatur<n>` auf Seite 1 (`/F 132`, `/Rect [0 0 0 0]`) → ");
    s.push_str("ByteRange-Daten mit `openssl cms -sign -binary -outform DER -nosmimecap -md sha256` und ");
    s.push_str("`testdata/keystores/test-rsa2048.p12` signieren → `Increment::embed_contents` → Prüfung mit ");
    s.push_str("`openssl cms -verify -noverify`, `qpdf --check` und pyHanko ");
    s.push_str("(`testdata/pdf-incremental/pyhanko_report.py`: `validate_pdf_signature`, `strict=False` wie ");
    s.push_str("`--no-strict-syntax`, ohne Netzzugriff).\n\n");
    s.push_str("Spalten: *Neue Signatur* = Integrität / kryptografische Gültigkeit / Abdeckung (Vertrauen ");
    s.push_str("bewusst ohne Bedeutung, selbstsigniertes Testzertifikat); *Vorsignaturen* = pyHanko-Änderungsstufe ");
    s.push_str("vorher → nachher (Ziel: höchstens `FORM_FILLING` für die zuvor vollständige Signatur; ");
    s.push_str("`OTHER` bei älteren PDF-AS-Signaturen stammt aus den PDF-AS-Increments selbst und ist unverändert); ");
    s.push_str("*pyHanko-CLI* = zweites Orakel `pyhanko sign validate --pretty-print --no-strict-syntax ");
    s.push_str("--no-revocation-check --trust <Testzertifikat> --trust-replace` (ohne Netz): neue Signatur ");
    s.push_str("„cryptographically sound“, „covers the entire file“, „judged VALID“; Vorsignaturen weiterhin ");
    s.push_str(
        "„cryptographically sound“ und – falls sie vorher die ganze Datei abdeckten – ohne „may be illegitimate“.\n\n",
    );
    s.push_str(versions);
    let ok = rows.iter().filter(|r| r.ok).count();
    s.push_str(&format!(
        "**{ok} von {} Dateien bestanden.** `simple-aes256-userpw.pdf` (Benutzerpasswort, K5) ",
        rows.len()
    ));
    s.push_str("wird erwartungsgemäß mit `CosError::PasswordRequired` abgelehnt und ist nicht aufgeführt.\n\n");
    s.push_str("| Datei | xref (letzte Rev.) | Verschlüsselung | Vorsig. | qpdf | openssl | Neue Signatur | Vorsignaturen (pyHanko) | pyHanko-CLI | Ergebnis |\n");
    s.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
    for r in rows {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.file,
            r.xref,
            r.encryption,
            r.prior,
            r.qpdf,
            if r.openssl { "ok" } else { "FEHLER" },
            r.new_sig,
            r.prior_after,
            r.cli,
            if r.ok { "ok".to_string() } else { format!("FEHLER: {}", r.problems.join("; ")) }
        ));
    }
    s.push('\n');
    s.push_str(FURTHER_CHECKS);
    s
}

/// Arbeitsverzeichnis: temporär, oder `PIDA_E2E_OUT` (Ausgaben bleiben zur Analyse erhalten).
fn workdir() -> (Option<tempfile::TempDir>, PathBuf) {
    match std::env::var_os("PIDA_E2E_OUT") {
        Some(p) => {
            let p = PathBuf::from(p);
            std::fs::create_dir_all(&p).unwrap();
            (None, p)
        }
        None => {
            let t = tempfile::tempdir().unwrap();
            let p = t.path().to_path_buf();
            (Some(t), p)
        }
    }
}

#[test]
fn sign_with_openssl_and_validate_with_pyhanko() {
    let (_guard, work_path) = workdir();
    let work = WorkPath(work_path);
    let Some(signer) = prepare_signer(work.path()) else {
        eprintln!("HINWEIS: openssl fehlt – E2E übersprungen");
        return;
    };
    let Some(python) = common::pyhanko_python() else {
        eprintln!("HINWEIS: pyHanko fehlt – E2E übersprungen");
        return;
    };
    let qpdf = common::which("qpdf");
    let pyhanko = common::which("pyhanko");
    if pyhanko.is_none() {
        eprintln!("HINWEIS: pyhanko-CLI fehlt – nur API-Prüfung");
    }
    let files = common::corpus();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunks: Vec<Vec<PathBuf>> =
        (0..threads).map(|i| files.iter().skip(i).step_by(threads).cloned().collect()).collect();
    let mut rows: Vec<Row> = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                let (signer, python, qpdf, pyhanko, work) =
                    (&signer, &python, qpdf.as_deref(), pyhanko.as_deref(), work.path());
                scope.spawn(move || {
                    chunk.iter().filter_map(|p| run_one(p, signer, python, qpdf, pyhanko, work)).collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("Worker")).collect()
    });
    rows.sort_by(|a, b| a.file.cmp(&b.file));
    let md = render(&rows, &tool_versions(&signer, &python, qpdf.as_deref()));
    let report = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pdf-incremental-RESULTS.md");
    std::fs::write(&report, &md).unwrap();
    if std::env::var_os("PIDA_UPDATE_RESULTS").is_some_and(|v| v == "1") {
        std::fs::write(common::testdata().join("pdf-incremental/RESULTS.md"), &md).unwrap();
    }
    let failed: Vec<_> =
        rows.iter().filter(|r| !r.ok).map(|r| format!("{}: {}", r.file, r.problems.join("; "))).collect();
    assert!(rows.len() >= 50, "zu wenige Dateien: {}", rows.len());
    assert!(failed.is_empty(), "E2E-Fehler:\n{}", failed.join("\n"));
}

/// Erzwungene Formate auf einer Tabellen-Quelle: auch xref-Stream bzw. Hybrid validieren.
#[test]
fn forced_xref_formats_validate() {
    let work = tempfile::tempdir().unwrap();
    let (Some(signer), Some(python)) = (prepare_signer(work.path()), common::pyhanko_python()) else {
        eprintln!("HINWEIS: openssl/pyHanko fehlt – übersprungen");
        return;
    };
    let path = common::testdata().join("pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf");
    let src = SourcePdf::open_file(&path, &Limits::default()).unwrap();
    for (i, xref) in [XrefFormat::Table, XrefFormat::Stream, XrefFormat::Hybrid].into_iter().enumerate() {
        let opts = IncrementOptions { xref, ..IncrementOptions::default() };
        let mut inc = common::add_dummy_signature(&src, opts, 8192).unwrap();
        let data = inc.byte_range_data(src.bytes());
        let cms = signer.sign(work.path(), &data).unwrap();
        inc.embed_contents(&cms).unwrap();
        let out_path = work.path().join(format!("forced{i}.pdf"));
        std::fs::write(&out_path, inc.to_vec(src.bytes())).unwrap();
        let rep = common::pyhanko_report(&python, &out_path, &[]).unwrap();
        assert_eq!(rep.len(), 2, "{xref:?}");
        let new = rep.iter().find(|s| s.field == "PDF-AS Signatur2").unwrap();
        assert!(new.intact && new.valid && new.coverage.as_deref() == Some("ENTIRE_FILE"), "{xref:?}: {new:?}");
        let old = rep.iter().find(|s| s.field == "PDF-AS Signatur1").unwrap();
        assert!(old.intact && old.valid, "{xref:?}: {old:?}");
        assert!(common::modification_rank(old.modification_level.as_deref()) <= 2, "{xref:?}: {old:?}");
        if let Some(q) = common::which("qpdf") {
            let r = common::qpdf_check(&q, &out_path);
            assert_eq!(r.code, 0, "{xref:?}: {:?}", r.messages);
        }
    }
}

/// Vorhandenes leeres Signaturfeld befüllen (Feld + `/Kids`-Widget fortgeschrieben).
#[test]
fn fill_existing_empty_field_validates() {
    let work = tempfile::tempdir().unwrap();
    let (Some(signer), Some(python)) = (prepare_signer(work.path()), common::pyhanko_python()) else {
        eprintln!("HINWEIS: openssl/pyHanko fehlt – übersprungen");
        return;
    };
    for rel in ["pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf", "pdf/generated/manySignFields-xref-stream.pdf"] {
        let path = common::testdata().join(rel);
        let src = SourcePdf::open_file(&path, &Limits::default()).unwrap();
        let fields = src.signature_fields().unwrap();
        let empty = fields.iter().find(|f| !f.signed && !f.widgets.is_empty()).expect("leeres Feld");
        let mut inc = common::fill_empty_field(&src, empty, 8192).unwrap();
        let data = inc.byte_range_data(src.bytes());
        let cms = signer.sign(work.path(), &data).unwrap();
        inc.embed_contents(&cms).unwrap();
        let out = inc.to_vec(src.bytes());
        let out_path = work.path().join("filled.pdf");
        std::fs::write(&out_path, &out).unwrap();
        let re = SourcePdf::open(out, &Limits::default()).unwrap();
        let f = re.signature_fields().unwrap().into_iter().find(|f| f.field_id == empty.field_id).unwrap();
        assert!(f.signed, "{rel}");
        assert_eq!(f.widgets[0].rect, empty.widgets[0].rect, "{rel}: /Rect bleibt");
        let rep = common::pyhanko_report(&python, &out_path, &[]).unwrap();
        let s = rep.iter().find(|s| s.field == empty.qualified_name).unwrap_or_else(|| panic!("{rel}: {rep:?}"));
        assert!(s.intact && s.valid && s.coverage.as_deref() == Some("ENTIRE_FILE"), "{rel}: {s:?}");
    }
}

/// Randfälle der Annahmepolitik (Plan §6.1 K1/K2): Leerraum bzw. Fremdinhalt nach `%%EOF`,
/// Vorspann vor dem Header, falscher `startxref` – das Increment wird angehängt, die neue Signatur
/// ist für pyHanko intakt und deckt die ganze Datei ab, `qpdf --check` meldet nichts Neues.
#[test]
fn edge_case_inputs_validate() {
    let work = tempfile::tempdir().unwrap();
    let (Some(signer), Some(python)) = (prepare_signer(work.path()), common::pyhanko_python()) else {
        eprintln!("HINWEIS: openssl/pyHanko fehlt – übersprungen");
        return;
    };
    let qpdf = common::which("qpdf");
    let base = std::fs::read(common::testdata().join("pdf/generated/simple-xref-table.pdf")).unwrap();
    let sx_pos = base.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10;
    let sx_end = sx_pos + base[sx_pos..].iter().position(|&b| b == b'\n').unwrap();
    let sx: u64 = std::str::from_utf8(&base[sx_pos..sx_end]).unwrap().parse().unwrap();
    let shifted = {
        let mut v = base[..sx_pos].to_vec();
        v.extend_from_slice((sx + 3).to_string().as_bytes());
        v.extend_from_slice(&base[sx_end..]);
        v
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("Leerraum nach %%EOF", [base.clone(), b"\r\n\n \0\0\t".to_vec()].concat()),
        ("Fremdinhalt nach %%EOF", [base.clone(), b"NUR TEST: angehaengter Fremdinhalt\n".to_vec()].concat()),
        ("ohne EOL nach %%EOF", base[..base.len() - 1].to_vec()),
        ("falscher startxref (K2)", shifted),
    ];
    for (i, (what, input)) in cases.into_iter().enumerate() {
        let src = SourcePdf::open(input.clone(), &Limits::default()).unwrap_or_else(|e| panic!("{what}: {e}"));
        let mut inc = common::add_dummy_signature(&src, IncrementOptions::default(), 8192).unwrap();
        let data = inc.byte_range_data(src.bytes());
        let cms = signer.sign(work.path(), &data).unwrap();
        inc.embed_contents(&cms).unwrap();
        let out = inc.to_vec(src.bytes());
        assert!(out.starts_with(&input), "{what}: Präfix");
        let path = work.path().join(format!("edge{i}.pdf"));
        std::fs::write(&path, &out).unwrap();
        let rep = common::pyhanko_report(&python, &path, &[&signer.cert]).unwrap();
        let s = rep.iter().find(|s| s.field == "PDF-AS Signatur1").unwrap_or_else(|| panic!("{what}: {rep:?}"));
        assert!(s.intact && s.valid, "{what}: {s:?}");
        assert_eq!(s.coverage.as_deref(), Some("ENTIRE_FILE"), "{what}: {s:?}");
        let re = SourcePdf::open(out, &Limits::default()).unwrap();
        assert_eq!(re.tail().kind, pida_pdf_cos::TailKind::Clean, "{what}");
        if let Some(q) = &qpdf {
            let before = common::qpdf_check(q, &{
                let p = work.path().join(format!("edge{i}-in.pdf"));
                std::fs::write(&p, &input).unwrap();
                p
            });
            let after = common::qpdf_check(q, &path);
            let new: Vec<_> = after.messages.iter().filter(|m| !before.messages.contains(m)).collect();
            assert!(after.code == 0 || after.code == 3, "{what}: qpdf {}: {:?}", after.code, after.messages);
            assert!(new.is_empty(), "{what}: neue qpdf-Befunde {new:?}");
        }
    }
}
