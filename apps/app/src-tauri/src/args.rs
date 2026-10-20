//! Startargumente der GUI mit der PDF-Over-Semantik der Kommandozeile (docs/analyse/01 §5).
//!
//! Die GUI nutzt **denselben Parser** wie `pida-cli` (Bibliothek `pida_cli` ohne Feature `run`):
//! zweiphasiges `-c`, „unbekanntes Token = Eingabedatei“, `-i -o -b -n -p -ks -kstype -kspass -ksalias
//! -kskeypass -proxy -proxyport -proxyuser -proxypass -e -a -s -c -v`. Die Werte gehen als
//! [`Overlay`] an den Kern (nie persistiert). Wie bei PDF-Over gilt:
//!
//! - `-h`/`-?`/`--help`: Hilfe auf stdout, danach Ende (ohne Fenster);
//! - ungültige Argumente bzw. fehlende Dateien (`error-CmdLineArgs`, `error-FileNotExist`): Fehleranzeige,
//!   „Abbrechen“ beendet (PDF-Over: Fehlerdialog + Ende, `PrepareConfigurationState.java:264-300`).
//!
//! PIDA-Ergänzungen: `--kspass-env <VAR>` liest das Keystore-Passwort aus der Umgebung,
//! `--log-level <filter>` setzt den Log-Filter (Vorrang vor `PIDA_LOG`), Datei-URLs (`file:///…`,
//! Linux-Starter mit `%U`) gelten als Pfade; die
//! CLI-Befehle (`verify`, `profiles`, `migrate`, `render-block`, `licenses`) und die Terminal-Optionen
//! (`--headless`, `--json`, `--kspass-stdin`) gibt es nur in `pida-cli` – in der GUI sind sie ein
//! Argumentfehler. `sign <eingaben…>` öffnet die Eingaben (Ordner, Muster) wie in der CLI.
//!
//! Regel 9: Werte von Optionen (Passwörter, Benutzername) erscheinen weder im Log noch in der
//! Fehleranzeige; Fehler nennen nur Dateipfade bzw. den Parserhinweis ohne Werte.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pida_cli::{Cli, CliError, Command};
use pida_config::Overlay;
use pida_i18n::Lang;
use zeroize::Zeroizing;

/// Meldungs-ID: ungültige Kommandozeile (PDF-Over `error.CmdLineArgs`).
pub const CMDLINE_INVALID: &str = "error-CmdLineArgs";
/// Meldungs-ID: Option nur in der Kommandozeile `pida-cli` verfügbar.
pub const CLI_ONLY: &str = "ng-ui-args-cli-only";

/// Ergebnis der Auswertung.
#[derive(Debug)]
pub enum GuiArgs {
    /// Starten mit diesem Overlay (Eingabedateien absolut, `-c` übernommen).
    Start {
        /// Overlay des Kerns
        overlay: Box<Overlay>,
        /// `--log-level` (Filter wie `PIDA_LOG`, hat Vorrang)
        log_level: Option<String>,
    },
    /// Hilfe ausgeben und beenden.
    Help(String),
    /// Version ausgeben und beenden.
    Version(String),
    /// Fehleranzeige statt Kernstart (`code` = Meldungs-ID, `params` für die Übersetzung).
    Invalid {
        /// Meldungs-ID
        code: String,
        /// Parameter (`detail`, `arg1` …)
        params: BTreeMap<String, String>,
    },
}

/// Wertet die Startargumente aus (`raw` **mit** Programmnamen; relative Pfade gelten zu `cwd`).
///
/// `env` liefert Umgebungsvariablen (`--kspass-env`); im Betrieb `std::env::var_os`.
pub fn parse_gui_args<I, T>(raw: I, cwd: &Path, env: &dyn Fn(&str) -> Option<OsString>) -> GuiArgs
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw: Vec<OsString> = without_platform_tokens(raw.into_iter().map(Into::into));
    let normalized = pida_cli::normalize_args(raw.clone());
    if pida_cli::wants_help(&normalized) {
        return GuiArgs::Help(pida_cli::text::help(Lang::system()));
    }
    let (config_file, cli) = match pida_cli::parse(raw) {
        Ok(v) => v,
        Err(CliError::Clap(e)) if e.kind() == clap::error::ErrorKind::DisplayVersion => {
            return GuiArgs::Version(format!("PIDA {}", env!("CARGO_PKG_VERSION")));
        }
        Err(CliError::Clap(e)) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
            return GuiArgs::Help(pida_cli::text::help(Lang::system()));
        }
        Err(e) => return invalid(CMDLINE_INVALID, &[("detail", parser_detail(&e))]),
    };
    if let Some(problem) = cli_only(&cli) {
        return invalid(CLI_ONLY, &[("option", problem.to_owned())]);
    }
    let cli = absolutize(cli, cwd);
    if let Err(e) = pida_cli::overlay::check(&cli) {
        let params: Vec<(&str, String)> = e.args.clone();
        return invalid(e.id, &params);
    }
    let mut overlay = pida_cli::to_overlay(&cli);
    overlay.config_file = config_file;
    if overlay.keystore_store_password.is_none()
        && let Some(var) = &cli.modern.kspass_env
    {
        match env(var) {
            Some(v) => overlay.keystore_store_password = Some(Zeroizing::new(v.to_string_lossy().into_owned())),
            None => return invalid("ng-cli-kspass-env-missing", &[("name", var.clone())]),
        }
    }
    if let (Some(key), Some(store)) = (&overlay.keystore_key_password, &overlay.keystore_store_password)
        && key.as_str() != store.as_str()
    {
        return invalid("ng-cli-keypass-mismatch", &[]);
    }
    match pida_cli::overlay::inputs(&cli) {
        Ok(inputs) => overlay.input_files = inputs,
        Err(e) => {
            let params: Vec<(&str, String)> = e.args.clone();
            return invalid(e.id, &params);
        }
    }
    let log_level = cli.modern.log_level.clone().filter(|l| !l.trim().is_empty());
    GuiArgs::Start { overlay: Box::new(overlay), log_level }
}

/// Dokumente einer zweiten Instanz („Öffnen mit“, „Senden an“): dieselbe Semantik (`-i` und freie
/// Tokens), nur vorhandene Dateien; Optionen der zweiten Instanz gelten nicht für die laufende
/// Sitzung (deren Overlay steht seit dem Start fest). Bei einem Syntaxfehler (z. B. `-o` ohne Wert)
/// bleibt die einfache Auswertung von [`crate::files::documents_from_args`].
pub fn second_instance_documents(argv: &[String], cwd: &Path) -> Vec<PathBuf> {
    let raw: Vec<OsString> = without_platform_tokens(argv.iter().map(OsString::from));
    match pida_cli::parse(raw) {
        Ok((_, cli)) => {
            let cli = absolutize(cli, cwd);
            let mut out: Vec<PathBuf> = Vec::new();
            let mut all = cli.legacy.input.clone();
            if let Some(Command::Sign { inputs }) = &cli.command
                && let Ok(expanded) = pida_cli::overlay::expand_inputs(inputs)
            {
                all.extend(expanded);
            }
            for p in all {
                if p.is_file() && !out.contains(&p) {
                    out.push(p);
                }
            }
            out
        }
        Err(_) => crate::files::documents_from_args(argv.iter().skip(1), cwd),
    }
}

/// Plattform-Eigenheiten der Starter: macOS übergibt älteren Startern `-psn_<nr>` (Process Serial
/// Number, kein Dokument); Desktop-Starter unter Linux übergeben mit `%U` Datei-URLs (`file:///…`, mit
/// Prozentkodierung) statt Pfaden – sie werden zu Pfaden (sonst hieße es `error-FileNotExist`).
fn without_platform_tokens(raw: impl Iterator<Item = OsString>) -> Vec<OsString> {
    raw.enumerate()
        .filter_map(|(i, t)| {
            if i == 0 {
                return Some(t);
            }
            let text = t.to_string_lossy();
            if text.starts_with("-psn_") {
                return None;
            }
            if text.starts_with("file://")
                && let Ok(path) = url::Url::parse(&text).map_err(|_| ()).and_then(|u| u.to_file_path())
            {
                return Some(path.into_os_string());
            }
            Some(t)
        })
        .collect()
}

/// Optionen bzw. Befehle, die es nur in der Kommandozeile gibt.
fn cli_only(cli: &Cli) -> Option<&'static str> {
    match &cli.command {
        Some(Command::Verify { .. }) => return Some("verify"),
        Some(Command::Profiles) => return Some("profiles"),
        Some(Command::Migrate { .. }) => return Some("migrate"),
        Some(Command::RenderBlock { .. }) => return Some("render-block"),
        Some(Command::Licenses) => return Some("licenses"),
        Some(Command::Sign { .. }) | None => {}
    }
    if cli.modern.headless {
        Some("--headless")
    } else if cli.modern.json {
        Some("--json")
    } else if cli.modern.kspass_stdin {
        Some("--kspass-stdin")
    } else {
        None
    }
}

/// Relative Pfade (Eingaben, `-o`, `-ks`, `-e`) gelten relativ zum Arbeitsordner beim Start; `-c`
/// bleibt relativ zum Konfigurationsordner (PDF-Over).
fn absolutize(mut cli: Cli, cwd: &Path) -> Cli {
    let abs = |p: &PathBuf| if p.is_absolute() || p.as_os_str().is_empty() { p.clone() } else { cwd.join(p) };
    let l = &mut cli.legacy;
    l.input = l.input.iter().map(abs).collect();
    l.output = l.output.as_ref().map(abs);
    l.keystore = l.keystore.as_ref().map(abs);
    l.emblem = l.emblem.as_ref().map(abs);
    if let Some(Command::Sign { inputs }) = &mut cli.command {
        *inputs = inputs.iter().map(abs).collect();
    }
    cli
}

/// Parserhinweis ohne Werte (Regel 9): clap nennt in `Display` u. a. den abgelehnten Wert, z. B. ein
/// Passwort mit führendem `-`; übernommen wird nur die Art des Fehlers und das betroffene Argument.
fn parser_detail(e: &CliError) -> String {
    match e {
        CliError::MissingConfigValue => "-c".to_owned(),
        CliError::Clap(e) => {
            let arg = e.get(clap::error::ContextKind::InvalidArg).map(|v| v.to_string());
            match arg {
                Some(arg) => format!("{:?}: {arg}", e.kind()),
                None => format!("{:?}", e.kind()),
            }
        }
    }
}

fn invalid(code: &str, params: &[(&str, String)]) -> GuiArgs {
    GuiArgs::Invalid {
        code: code.to_owned(),
        params: params.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_config::BackendChoice;

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pida-app-args-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("Temp-Ordner");
        std::fs::write(dir.join("a.pdf"), b"%PDF-1.7\n").expect("a");
        std::fs::write(dir.join("b.pdf"), b"%PDF-1.7\n").expect("b");
        std::fs::write(dir.join("ks.p12"), b"x").expect("ks");
        dir
    }

    fn start(args: &[&str], cwd: &Path) -> Overlay {
        let mut raw = vec!["pida"];
        raw.extend_from_slice(args);
        match parse_gui_args(raw, cwd, &no_env) {
            GuiArgs::Start { overlay, .. } => *overlay,
            other => panic!("erwartet Start: {other:?}"),
        }
    }

    #[test]
    fn all_pdf_over_options_become_the_overlay() {
        let dir = fixture("all");
        let o = start(
            &[
                "a.pdf",
                "-i",
                "b.pdf",
                "-o",
                "out",
                "-b",
                "MOBILE",
                "-n",
                "0664123456",
                "-p",
                "geheim1",
                "-ks",
                "ks.p12",
                "-kstype",
                "PKCS12",
                "-kspass",
                "pw123",
                "-ksalias",
                "alias",
                "-kskeypass",
                "pw123",
                "-proxy",
                "proxy.example",
                "-proxyport",
                "8080",
                "-proxyuser",
                "u",
                "-proxypass",
                "pp",
                "-a",
                "-s",
                "-c",
                "alt.config",
                "-v",
                "true",
            ],
            &dir,
        );
        assert_eq!(o.input_files, vec![dir.join("a.pdf"), dir.join("b.pdf")]);
        assert_eq!(o.output, Some(dir.join("out")));
        assert_eq!(o.backend, Some(BackendChoice::IdAustria));
        assert_eq!(o.username.as_deref(), Some("+43664123456"));
        assert_eq!(o.password.as_deref().map(String::as_str), Some("geheim1"));
        assert_eq!(o.keystore_file, Some(dir.join("ks.p12")));
        assert_eq!(o.keystore_type.as_deref(), Some("PKCS12"));
        assert_eq!(o.keystore_store_password.as_deref().map(String::as_str), Some("pw123"));
        assert_eq!(o.keystore_alias.as_deref(), Some("alias"));
        assert_eq!(o.proxy_host.as_deref(), Some("proxy.example"));
        assert_eq!(o.proxy_port, Some(8080));
        assert_eq!(o.proxy_user.as_deref(), Some("u"));
        assert!(o.auto_position && o.skip_finish);
        assert_eq!(o.config_file, Some(PathBuf::from("alt.config")));
        assert_eq!(o.invisible, Some(true));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_arguments_start_without_overlay() {
        let dir = fixture("none");
        let o = start(&[], &dir);
        assert_eq!(o, Overlay::default());
        // `-b NONE` = keine Vorgabe; `-v false` = sichtbar (B-07)
        let o = start(&["-b", "NONE", "-v", "false"], &dir);
        assert_eq!(o.backend, None);
        assert_eq!(o.invisible, Some(false));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_files_and_bad_values_are_errors_without_secrets() {
        let dir = fixture("err");
        let r = parse_gui_args(["pida", "fehlt.pdf"], &dir, &no_env);
        let GuiArgs::Invalid { code, params } = r else { panic!("{r:?}") };
        assert_eq!(code, "error-FileNotExist");
        assert!(params["arg1"].ends_with("fehlt.pdf"));
        // unbekanntes Token `-x` = Datei → fehlt
        assert!(
            matches!(parse_gui_args(["x", "-x"], &dir, &no_env), GuiArgs::Invalid { code, .. } if code == "error-FileNotExist")
        );
        // -p zu kurz
        let r = parse_gui_args(["x", "-p", "kurz"], &dir, &no_env);
        assert!(matches!(&r, GuiArgs::Invalid { code, .. } if code == "exception-PasswordTooShort"), "{r:?}");
        // ungültiger Wert: Parserfehler ohne den Wert (Regel 9)
        let r = parse_gui_args(["x", "-b", "GEHEIMWERT"], &dir, &no_env);
        let GuiArgs::Invalid { code, params } = r else { panic!("{r:?}") };
        assert_eq!(code, CMDLINE_INVALID);
        assert!(!params["detail"].contains("GEHEIMWERT"), "{params:?}");
        // -kskeypass ≠ -kspass
        let r = parse_gui_args(["x", "-kspass", "a1234", "-kskeypass", "b1234"], &dir, &no_env);
        assert!(matches!(&r, GuiArgs::Invalid { code, .. } if code == "ng-cli-keypass-mismatch"), "{r:?}");
        // -c ohne Wert
        let r = parse_gui_args(["x", "-c"], &dir, &no_env);
        assert!(matches!(&r, GuiArgs::Invalid { code, .. } if code == CMDLINE_INVALID), "{r:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn help_version_and_cli_only_options() {
        let dir = fixture("help");
        for h in ["-h", "-?", "--help"] {
            let GuiArgs::Help(text) = parse_gui_args(["x", h], &dir, &no_env) else { panic!("{h}") };
            assert!(text.contains("-kskeypass"), "{text}");
        }
        assert!(
            matches!(parse_gui_args(["x", "--version"], &dir, &no_env), GuiArgs::Version(v) if v.starts_with("PIDA"))
        );
        for (args, option) in [
            (&["x", "--headless", "a.pdf"][..], "--headless"),
            (&["x", "--json"][..], "--json"),
            (&["x", "profiles"][..], "profiles"),
            (&["x", "licenses"][..], "licenses"),
            (&["x", "verify", "a.pdf"][..], "verify"),
        ] {
            let r = parse_gui_args(args.iter().copied(), &dir, &no_env);
            let GuiArgs::Invalid { code, params } = r else { panic!("{args:?}: {r:?}") };
            assert_eq!(code, CLI_ONLY);
            assert_eq!(params["option"], option);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sign_expands_folders_and_kspass_env_is_read() {
        let dir = fixture("sign");
        let o = start(&["sign", "."], &dir);
        assert_eq!(o.input_files, vec![dir.join("./a.pdf"), dir.join("./b.pdf")]);
        let env = |name: &str| (name == "KSPW").then(|| OsString::from("pw123"));
        let r = parse_gui_args(["x", "-ks", "ks.p12", "--kspass-env", "KSPW"], &dir, &env);
        let GuiArgs::Start { overlay: o, .. } = r else { panic!("{r:?}") };
        assert_eq!(o.keystore_store_password.as_deref().map(String::as_str), Some("pw123"));
        let r = parse_gui_args(["x", "--kspass-env", "FEHLT"], &dir, &env);
        assert!(matches!(&r, GuiArgs::Invalid { code, .. } if code == "ng-cli-kspass-env-missing"), "{r:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn log_level_option_is_passed_to_the_shell() {
        let dir = fixture("log");
        let r = parse_gui_args(["x", "--log-level", "debug"], &dir, &no_env);
        assert!(matches!(&r, GuiArgs::Start { log_level: Some(l), .. } if l == "debug"), "{r:?}");
        assert!(matches!(parse_gui_args(["x"], &dir, &no_env), GuiArgs::Start { log_level: None, .. }));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn macos_process_serial_number_is_ignored() {
        let dir = fixture("psn");
        let o = start(&["-psn_0_12345", "a.pdf"], &dir);
        assert_eq!(o.input_files, vec![dir.join("a.pdf")]);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Linux-Starter mit `%U`: Datei-URLs (auch mit Leerzeichen) sind Dokumente.
    #[test]
    fn file_urls_become_paths() {
        let dir = fixture("url");
        std::fs::write(dir.join("mit leer.pdf"), b"%PDF-1.7\n").expect("Datei");
        let a = url::Url::from_file_path(dir.join("a.pdf")).expect("url").to_string();
        let spaced = url::Url::from_file_path(dir.join("mit leer.pdf")).expect("url").to_string();
        assert!(spaced.contains("%20"), "{spaced}");
        let o = start(&[a.as_str(), spaced.as_str()], &dir);
        assert_eq!(o.input_files, vec![dir.join("a.pdf"), dir.join("mit leer.pdf")]);
        let argv: Vec<String> = vec!["pida".into(), a];
        assert_eq!(second_instance_documents(&argv, &dir), vec![dir.join("a.pdf")]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn second_instance_takes_only_existing_documents() {
        let dir = fixture("second");
        let argv: Vec<String> =
            ["pida", "a.pdf", "-p", "b.pdf", "-i", "b.pdf", "fehlt.pdf", "a.pdf"].map(String::from).to_vec();
        // `-p b.pdf` ist ein Passwort (zu kurz ist egal – Optionen gelten hier nicht)
        assert_eq!(second_instance_documents(&argv, &dir), vec![dir.join("a.pdf"), dir.join("b.pdf")]);
        // Syntaxfehler (`-o` ohne Wert): einfache Auswertung
        let argv: Vec<String> = ["pida", "a.pdf", "-o"].map(String::from).to_vec();
        assert_eq!(second_instance_documents(&argv, &dir), vec![dir.join("a.pdf")]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
