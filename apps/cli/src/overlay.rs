//! CLI-Argumente → [`Overlay`] (nie persistiert) mit den Prüfungen von PDF-Over und den Korrekturen
//! aus Plan §7.4 (docs/analyse/01 §5.2/§5.3).

use std::path::{Path, PathBuf};

use pdfover_config::{BackendChoice, Overlay};
use zeroize::Zeroizing;

#[cfg(feature = "run")]
use crate::term::Terminal;
use crate::{Cli, Command, LegacyBku};

/// Fehler der Argumente (Exit 2), mit Meldungs-ID und Parametern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgError {
    /// Meldungs-ID
    pub id: &'static str,
    /// Parameter
    pub args: Vec<(&'static str, String)>,
}

impl ArgError {
    fn new(id: &'static str, args: Vec<(&'static str, String)>) -> ArgError {
        ArgError { id, args }
    }
}

/// `-b` → Standard-Backend (`NONE` = persistente Einstellung gilt).
pub fn backend(bku: Option<LegacyBku>) -> Option<BackendChoice> {
    match bku? {
        LegacyBku::Local => Some(BackendChoice::LocalBku),
        LegacyBku::Mobile => Some(BackendChoice::IdAustria),
        LegacyBku::Ks => Some(BackendChoice::Keystore),
        LegacyBku::None => None,
    }
}

/// Overlay aus den Argumenten (reine Abbildung; Prüfungen in [`check`], Passwortquellen in
/// [`keystore_password`]). `-n` wird wie PDF-Over normalisiert (`0…` → `+43…`).
pub fn to_overlay(cli: &Cli) -> Overlay {
    let l = &cli.legacy;
    let pw = |o: &Option<String>| o.as_ref().map(|p| Zeroizing::new(p.clone()));
    Overlay {
        input_files: l.input.clone(),
        output: l.output.clone(),
        backend: backend(l.bku),
        username: l.number.as_deref().map(pdfover_config::normalize_mobile_number),
        password: pw(&l.password),
        keystore_file: l.keystore.clone(),
        keystore_type: l.keystore_type.clone(),
        keystore_store_password: pw(&l.keystore_password),
        keystore_alias: l.keystore_alias.clone(),
        keystore_key_password: pw(&l.keystore_key_password),
        proxy_host: l.proxy_host.clone(),
        proxy_port: l.proxy_port,
        proxy_user: l.proxy_user.clone(),
        proxy_password: pw(&l.proxy_password),
        emblem: l.emblem.clone(),
        auto_position: l.auto_position,
        skip_finish: l.skip_finish,
        invisible: l.invisible,
        config_file: l.config.clone(),
    }
}

/// Prüfungen wie PDF-Over (Dateien müssen existieren, `-p` 5–200 Zeichen) plus Plan §7.4:
/// abweichendes `-kskeypass` ist ein klarer Fehler.
pub fn check(cli: &Cli) -> Result<(), ArgError> {
    let l = &cli.legacy;
    let missing = |p: &Path| ArgError::new("error-FileNotExist", vec![("arg1", p.display().to_string())]);
    if let Some(p) = &l.password
        && let Some(id) = pdfover_config::overlay::password_problem(p)
    {
        return Err(ArgError::new(id, vec![]));
    }
    if let (Some(a), Some(b)) = (&l.keystore_password, &l.keystore_key_password)
        && a != b
    {
        return Err(ArgError::new("ng-cli-keypass-mismatch", vec![]));
    }
    if let Some(k) = &l.keystore
        && !k.is_file()
    {
        return Err(missing(k));
    }
    if let Some(e) = &l.emblem
        && !e.is_file()
    {
        return Err(missing(e));
    }
    for i in &l.input {
        if !i.is_file() {
            return Err(missing(i));
        }
    }
    Ok(())
}

/// Keystore-Passwort aus `-kspass`, `--kspass-env` bzw. `--kspass-stdin` (erste Quelle gewinnt).
#[cfg(feature = "run")]
pub fn keystore_password(cli: &Cli, term: &dyn Terminal) -> Result<Option<Zeroizing<String>>, ArgError> {
    if let Some(p) = &cli.legacy.keystore_password {
        return Ok(Some(Zeroizing::new(p.clone())));
    }
    if let Some(var) = &cli.modern.kspass_env {
        return match std::env::var(var) {
            Ok(v) => Ok(Some(Zeroizing::new(v))),
            Err(_) => Err(ArgError::new("ng-cli-kspass-env-missing", vec![("name", var.clone())])),
        };
    }
    if cli.modern.kspass_stdin {
        let prompt = crate::text::tr(pdfover_i18n::Lang::system(), "ng-cli-password", &[]);
        return match term.secret(&prompt) {
            Some(p) => Ok(Some(p)),
            None => Err(ArgError::new("ng-cli-stdin-failed", vec![("detail", "EOF".into())])),
        };
    }
    Ok(None)
}

/// `*`/`?` im Dateinamen (ohne Pfadtrenner).
fn wildcard(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn is_pdf(p: &Path) -> bool {
    p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Eingaben von `sign`: Dateien, Ordner (alle `*.pdf`, nicht rekursiv), Muster im Dateinamen
/// (`*.pdf`, `rechnung-??.pdf`); sortiert, ohne Duplikate.
pub fn expand_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, ArgError> {
    let mut out = Vec::new();
    for input in inputs {
        let name = input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.contains(['*', '?']) {
            let dir = match input.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => PathBuf::from("."),
            };
            let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.filter_map(Result::ok)
                        .map(|e| e.path())
                        .filter(|p| p.is_file())
                        .filter(|p| p.file_name().is_some_and(|n| wildcard(&name, &n.to_string_lossy())))
                        .collect()
                })
                .unwrap_or_default();
            found.sort();
            if found.is_empty() {
                return Err(ArgError::new("error-FileNotExist", vec![("arg1", input.display().to_string())]));
            }
            out.extend(found);
        } else if input.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(input)
                .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| is_pdf(p)).collect())
                .unwrap_or_default();
            found.sort();
            out.extend(found);
        } else if input.is_file() {
            out.push(input.clone());
        } else {
            return Err(ArgError::new("error-FileNotExist", vec![("arg1", input.display().to_string())]));
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    Ok(out)
}

/// Alle Eingabedokumente (`-i`, nackte Tokens, `sign …`).
pub fn inputs(cli: &Cli) -> Result<Vec<PathBuf>, ArgError> {
    let mut all = cli.legacy.input.clone();
    if let Some(Command::Sign { inputs }) = &cli.command {
        all.extend(expand_inputs(inputs)?);
    }
    let mut seen = std::collections::HashSet::new();
    all.retain(|p| seen.insert(p.clone()));
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn cli(args: &[&str]) -> Cli {
        let mut v = vec!["pdfover-cli"];
        v.extend_from_slice(args);
        parse(v).expect("parse").1
    }

    #[test]
    fn overlay_mapping() {
        let c = cli(&[
            "-n",
            "0664 1234567",
            "-p",
            "geheim",
            "-b",
            "NONE",
            "-v",
            "nein",
            "-a",
            "-s",
            "-proxy",
            "proxy.local",
            "-proxyport",
            "3128",
            "-kstype",
            "PKCS12",
        ]);
        let o = to_overlay(&c);
        assert_eq!(o.username.as_deref(), Some("+436641234567"));
        assert_eq!(o.password.as_ref().unwrap().as_str(), "geheim");
        assert_eq!(o.backend, None);
        assert_eq!(o.invisible, Some(false));
        assert!(o.auto_position && o.skip_finish);
        assert_eq!(o.proxy_host.as_deref(), Some("proxy.local"));
        assert_eq!(o.proxy_port, Some(3128));
        assert!(!format!("{o:?}").contains("geheim"));
        assert_eq!(to_overlay(&cli(&["-b", "KS"])).backend, Some(BackendChoice::Keystore));
        assert_eq!(to_overlay(&cli(&["-b", "MOBILE"])).backend, Some(BackendChoice::IdAustria));
        // Testnummern und Benutzernamen bleiben
        assert_eq!(to_overlay(&cli(&["-n", "max.muster"])).username.as_deref(), Some("max.muster"));
    }

    #[test]
    fn checks_like_pdf_over() {
        assert_eq!(check(&cli(&["-p", "abc"])).unwrap_err().id, "exception-PasswordTooShort");
        let long = "x".repeat(201);
        assert_eq!(check(&cli(&["-p", &long])).unwrap_err().id, "exception-PasswordTooLong");
        assert_eq!(check(&cli(&["-kspass", "a", "-kskeypass", "b"])).unwrap_err().id, "ng-cli-keypass-mismatch");
        assert!(check(&cli(&["-kspass", "a", "-kskeypass", "a"])).is_ok());
        let e = check(&cli(&["/nicht/da.pdf"])).unwrap_err();
        assert_eq!(e.id, "error-FileNotExist");
        assert_eq!(check(&cli(&["-ks", "/nicht/da.p12"])).unwrap_err().id, "error-FileNotExist");
        assert_eq!(check(&cli(&["-e", "/nicht/da.png"])).unwrap_err().id, "error-FileNotExist");
    }

    #[cfg(feature = "run")]
    #[test]
    fn keystore_password_sources() {
        let term = crate::term::Scripted::new(false, &["vom-stdin"]);
        assert_eq!(keystore_password(&cli(&["-kspass", "a"]), &term).unwrap().unwrap().as_str(), "a");
        assert_eq!(keystore_password(&cli(&["--kspass-stdin"]), &term).unwrap().unwrap().as_str(), "vom-stdin");
        let e = keystore_password(&cli(&["--kspass-env", "PDFOVER_TEST_GIBT_ES_NICHT_42"]), &term).unwrap_err();
        assert_eq!(e.id, "ng-cli-kspass-env-missing");
        assert!(keystore_password(&cli(&[]), &term).unwrap().is_none());
        let empty = crate::term::Scripted::new(false, &[]);
        assert_eq!(keystore_password(&cli(&["--kspass-stdin"]), &empty).unwrap_err().id, "ng-cli-stdin-failed");
    }

    #[test]
    fn wildcards() {
        assert!(wildcard("*.pdf", "a.pdf"));
        assert!(wildcard("r-??.pdf", "r-01.pdf"));
        assert!(!wildcard("r-??.pdf", "r-1.pdf"));
        assert!(wildcard("*", ""));
        assert!(!wildcard("*.pdf", "a.pdfx"));
        assert!(wildcard("a*b*c", "aXXbYYc"));
    }

    #[test]
    fn input_expansion() {
        let tmp = tempfile::tempdir().unwrap();
        for n in ["b.pdf", "a.PDF", "c.txt", "r-01.pdf"] {
            std::fs::write(tmp.path().join(n), b"x").unwrap();
        }
        let dir = expand_inputs(&[tmp.path().to_path_buf()]).unwrap();
        let names: Vec<String> = dir.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a.PDF", "b.pdf", "r-01.pdf"]);
        let glob =
            expand_inputs(&[tmp.path().join("r-??.pdf"), tmp.path().join("b.pdf"), tmp.path().join("*.pdf")]).unwrap();
        assert_eq!(glob.len(), 2, "{glob:?}");
        assert!(expand_inputs(&[tmp.path().join("x-*.pdf")]).is_err());
        assert!(expand_inputs(&[tmp.path().join("fehlt.pdf")]).is_err());
        let c = cli(&["sign", tmp.path().to_str().unwrap()]);
        assert_eq!(inputs(&c).unwrap().len(), 3);
    }
}
