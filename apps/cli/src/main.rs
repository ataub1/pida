//! `pdfover-cli` – Kommandozeile von PDF-Over NG (Plan §7.4; docs/analyse/01 §5).
//!
//! Kompatibel zu PDF-Over: `-h/-?/--help -i -o -b -n -p -ks -kstype -kspass -ksalias -kskeypass
//! -proxy -proxyport -proxyuser -proxypass -e -a -s -c -v`, inklusive der Quirks
//! - **zweiphasiges `-c`**: Pass 1 wertet nur `-c` aus (relativ zum Konfigurationsordner; Altdateien
//!   unter `~/.pdf-over/<name>` werden mit Java-Properties-Semantik gelesen, ohne zurückzuschreiben),
//!   Pass 2 alle Argumente als Overlay,
//! - **unbekanntes Token = Eingabedatei** (davon leben „Öffnen mit“ und „Senden an“), auch `-x`,
//! - Werte werden nicht auf führendes `-` geprüft.
//!
//! Korrekturen: `-v` wertet `true`/`false` aus (B-07), Hilfetexte von `-kspass`/`-kskeypass`
//! richtig (B-08), fehlende `-c`-Datei ist ein Fehler statt Werksreset, abweichendes `-kskeypass`
//! ergibt einen klaren Fehler. Neu: Subcommands, `--headless`, `--json`, `--kspass-stdin`,
//! `--kspass-env`. Die Ausführungslogik ist Teil der Integration (`run`, `todo!()`).

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Legacy-Optionen mit mehrbuchstabigem Einzelstrich (`-ks`) → `--ks`.
pub const LEGACY_LONG_OPTIONS: [&str; 9] =
    ["ks", "kstype", "kspass", "ksalias", "kskeypass", "proxy", "proxyport", "proxyuser", "proxypass"];

/// Legacy-Kurzoptionen mit Wert → Langform.
pub const LEGACY_SHORT_WITH_VALUE: [(&str, &str); 8] = [
    ("-i", "--input"),
    ("-o", "--output"),
    ("-b", "--bku"),
    ("-n", "--number"),
    ("-p", "--password"),
    ("-e", "--emblem"),
    ("-c", "--config"),
    ("-v", "--invisible"),
];

/// Legacy-Flags ohne Wert.
pub const LEGACY_FLAGS: [&str; 3] = ["-a", "-s", "-h"];

/// Moderne Optionen mit Wert (werden unverändert durchgereicht).
pub const MODERN_WITH_VALUE: [&str; 2] = ["--kspass-env", "--log-level"];

/// Moderne Flags.
pub const MODERN_FLAGS: [&str; 5] = ["--headless", "--json", "--kspass-stdin", "--help", "--version"];

/// Subcommands.
pub const SUBCOMMANDS: [&str; 5] = ["sign", "verify", "profiles", "migrate", "render-block"];

/// PDF-Over-Signiermittel (`-b`, case-sensitive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LegacyBku {
    /// lokale BKU
    #[value(name = "LOCAL")]
    Local,
    /// ID Austria (Szenario A)
    #[value(name = "MOBILE")]
    Mobile,
    /// Keystore
    #[value(name = "KS")]
    Ks,
    /// keine Vorgabe (persistente Einstellung gilt)
    #[value(name = "NONE")]
    None,
}

/// Optionen von PDF-Over (alle `global`, damit sie auch nach einem Subcommand gelten).
#[derive(Clone, Default, PartialEq, Eq, Args)]
pub struct LegacyArgs {
    /// Eingabedokument; erstes = aktuelles, weitere = Warteschlange (muss existieren)
    #[arg(short = 'i', long = "input", value_name = "DATEI", global = true, action = clap::ArgAction::Append)]
    pub input: Vec<PathBuf>,
    /// Ausgabeordner oder -datei (Overlay)
    #[arg(short = 'o', long = "output", value_name = "ORDNER|DATEI", global = true)]
    pub output: Option<PathBuf>,
    /// Signiermittel
    #[arg(short = 'b', long = "bku", value_enum, global = true)]
    pub bku: Option<LegacyBku>,
    /// ID-Austria-Benutzername bzw. Telefonnummer (normalisiert auf +43…)
    #[arg(short = 'n', long = "number", value_name = "NUMMER|BENUTZER", global = true)]
    pub number: Option<String>,
    /// ID-Austria-Passwort (Szenario A, nur RAM; 5–200 Zeichen)
    #[arg(short = 'p', long = "password", value_name = "PASSWORT", global = true)]
    pub password: Option<String>,
    /// Keystore-Datei (aktiviert den Keystore)
    #[arg(long = "ks", value_name = "DATEI", global = true)]
    pub keystore: Option<PathBuf>,
    /// Keystore-Typ (PKCS12; JKS/JCEKS werden erkannt und mit Konvertierungshinweis abgelehnt)
    #[arg(long = "kstype", value_name = "TYP", global = true)]
    pub keystore_type: Option<String>,
    /// Keystore-Passwort (besser `--kspass-stdin`/`--kspass-env`)
    #[arg(long = "kspass", value_name = "PASSWORT", global = true)]
    pub keystore_password: Option<String>,
    /// Schlüssel-Alias
    #[arg(long = "ksalias", value_name = "ALIAS", global = true)]
    pub keystore_alias: Option<String>,
    /// Schlüssel-Passwort (bei PKCS#12 gleich dem Keystore-Passwort)
    #[arg(long = "kskeypass", value_name = "PASSWORT", global = true)]
    pub keystore_key_password: Option<String>,
    /// Proxy-Host
    #[arg(long = "proxy", value_name = "HOST", global = true)]
    pub proxy_host: Option<String>,
    /// Proxy-Port (1–65535)
    #[arg(long = "proxyport", value_name = "PORT", global = true, value_parser = clap::value_parser!(u16).range(1..))]
    pub proxy_port: Option<u16>,
    /// Proxy-Benutzer
    #[arg(long = "proxyuser", value_name = "BENUTZER", global = true)]
    pub proxy_user: Option<String>,
    /// Proxy-Passwort
    #[arg(long = "proxypass", value_name = "PASSWORT", global = true)]
    pub proxy_password: Option<String>,
    /// Bildmarke
    #[arg(short = 'e', long = "emblem", value_name = "BILD", global = true)]
    pub emblem: Option<PathBuf>,
    /// automatische Positionierung
    #[arg(short = 'a', long = "auto", global = true)]
    pub auto_position: bool,
    /// Ergebnis-Screen überspringen, nach dem Speichern beenden
    #[arg(short = 's', long = "skip-finish", global = true)]
    pub skip_finish: bool,
    /// alternative Konfigurationsdatei (relativ zum Konfigurationsordner; zweiphasig)
    #[arg(short = 'c', long = "config", value_name = "DATEI", global = true)]
    pub config: Option<PathBuf>,
    /// unsichtbare Signatur (`true`/`false`; B-07 korrigiert)
    #[arg(short = 'v', long = "invisible", value_name = "true|false", global = true, value_parser = parse_bool)]
    pub invisible: Option<bool>,
}

impl std::fmt::Debug for LegacyArgs {
    /// Passwörter (`-p`, `-kspass`, `-kskeypass`, `-proxypass`) werden nie ausgegeben.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |o: &Option<String>| o.as_ref().map(|_| "***");
        f.debug_struct("LegacyArgs")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("bku", &self.bku)
            .field("number", &self.number)
            .field("password", &redact(&self.password))
            .field("keystore", &self.keystore)
            .field("keystore_type", &self.keystore_type)
            .field("keystore_password", &redact(&self.keystore_password))
            .field("keystore_alias", &self.keystore_alias)
            .field("keystore_key_password", &redact(&self.keystore_key_password))
            .field("proxy_host", &self.proxy_host)
            .field("proxy_port", &self.proxy_port)
            .field("proxy_user", &self.proxy_user)
            .field("proxy_password", &redact(&self.proxy_password))
            .field("emblem", &self.emblem)
            .field("auto_position", &self.auto_position)
            .field("skip_finish", &self.skip_finish)
            .field("config", &self.config)
            .field("invisible", &self.invisible)
            .finish()
    }
}

/// Neue Optionen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct ModernArgs {
    /// ohne GUI (nur Keystore und lokale BKU)
    #[arg(long = "headless", global = true)]
    pub headless: bool,
    /// maschinenlesbare Ausgabe
    #[arg(long = "json", global = true)]
    pub json: bool,
    /// Keystore-Passwort von stdin lesen
    #[arg(long = "kspass-stdin", global = true)]
    pub kspass_stdin: bool,
    /// Keystore-Passwort aus Umgebungsvariable
    #[arg(long = "kspass-env", value_name = "VARIABLE", global = true)]
    pub kspass_env: Option<String>,
    /// Log-Stufe (error, warn, info, debug, trace)
    #[arg(long = "log-level", value_name = "STUFE", global = true)]
    pub log_level: Option<String>,
}

/// Subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Dokumente signieren (Dateien, Ordner, Globs)
    Sign {
        /// Eingaben
        #[arg(value_name = "EINGABE")]
        inputs: Vec<PathBuf>,
    },
    /// Signaturen prüfen (QA-Werkzeug, Selbstprüfung wie I3/I4)
    Verify {
        /// Datei
        file: PathBuf,
    },
    /// Profile auflisten
    Profiles,
    /// Konfiguration aus `~/.pdf-over` importieren
    Migrate {
        /// Altordner (Default `~/.pdf-over`)
        #[arg(long = "from")]
        from: Option<PathBuf>,
        /// nur Bericht, nichts schreiben
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Signaturblock als PDF/PNG rendern
    RenderBlock {
        /// Profil-ID
        #[arg(long = "profile")]
        profile: String,
        /// Zieldatei (`.pdf` oder `.png`)
        output: PathBuf,
    },
}

/// Kommandozeile.
#[derive(Debug, Clone, PartialEq, Eq, Parser)]
#[command(name = "pdfover-cli", version, about = "PDF-Over NG – PDF-Dokumente elektronisch signieren")]
pub struct Cli {
    /// PDF-Over-Optionen
    #[command(flatten)]
    pub legacy: LegacyArgs,
    /// neue Optionen
    #[command(flatten)]
    pub modern: ModernArgs,
    /// Subcommand
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Fehler der Kommandozeile.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// `-c` ohne Wert
    #[error("Option -c verlangt einen Dateinamen")]
    MissingConfigValue,
    /// Syntaxfehler
    #[error(transparent)]
    Clap(#[from] clap::Error),
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "ja" => Ok(true),
        "false" | "0" | "no" | "nein" => Ok(false),
        other => Err(format!("erwartet true oder false, erhalten: {other}")),
    }
}

/// Übersetzt PDF-Over-Argumente in eine clap-verständliche Form (Werte immer als `--opt=WERT`,
/// damit Werte mit führendem `-` erhalten bleiben). Unbekannte Tokens werden zu `--input=TOKEN`,
/// solange kein Subcommand aktiv ist.
pub fn normalize_args<I, T>(raw: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut it = raw.into_iter().map(Into::into);
    let mut out: Vec<OsString> = Vec::new();
    if let Some(argv0) = it.next() {
        out.push(argv0);
    }
    let tokens: Vec<OsString> = it.collect();
    let mut subcommand_seen = false;
    let mut i = 0usize;
    while i < tokens.len() {
        let tok = &tokens[i];
        let s = tok.to_string_lossy();
        let next = tokens.get(i + 1);
        let with_value = |long: &str, next: Option<&OsString>| -> (Vec<OsString>, usize) {
            match next {
                Some(v) => {
                    let mut joined = OsString::from(format!("{long}="));
                    joined.push(v);
                    (vec![joined], 2)
                }
                None => (vec![OsString::from(long)], 1),
            }
        };
        let (emit, consumed): (Vec<OsString>, usize) = if s == "-?" {
            (vec!["--help".into()], 1)
        } else if let Some(name) = s.strip_prefix('-').filter(|n| LEGACY_LONG_OPTIONS.contains(n)) {
            with_value(&format!("--{name}"), next)
        } else if let Some((_, long)) = LEGACY_SHORT_WITH_VALUE.iter().find(|(short, _)| *short == s) {
            with_value(long, next)
        } else if LEGACY_FLAGS.contains(&s.as_ref()) || MODERN_FLAGS.contains(&s.as_ref()) {
            (vec![tok.clone()], 1)
        } else if MODERN_WITH_VALUE.contains(&s.as_ref()) {
            with_value(&s, next)
        } else if s.starts_with("--") && s.contains('=') {
            (vec![tok.clone()], 1)
        } else if !subcommand_seen && SUBCOMMANDS.contains(&s.as_ref()) {
            subcommand_seen = true;
            (vec![tok.clone()], 1)
        } else if subcommand_seen && !s.starts_with('-') {
            (vec![tok.clone()], 1)
        } else {
            // Quirk: unbekanntes Token (auch `-x`) = Eingabedatei (docs/analyse/01 §5.1 Punkt 4).
            let mut joined = OsString::from("--input=");
            joined.push(tok);
            (vec![joined], 1)
        };
        out.extend(emit);
        i += consumed;
    }
    out
}

/// Pass 1: nur `-c` auswerten (alles andere wird ignoriert).
pub fn extract_config_file(normalized: &[OsString]) -> Result<Option<PathBuf>, CliError> {
    let mut found = None;
    for tok in normalized.iter().skip(1) {
        let s = tok.to_string_lossy();
        if s == "--config" {
            return Err(CliError::MissingConfigValue);
        }
        if let Some(v) = s.strip_prefix("--config=") {
            found = Some(PathBuf::from(v));
        }
    }
    Ok(found)
}

/// Pass 2: vollständiges Parsen.
pub fn parse<I, T>(raw: I) -> Result<(Option<PathBuf>, Cli), CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let normalized = normalize_args(raw);
    let config = extract_config_file(&normalized)?;
    let cli = Cli::try_parse_from(normalized)?;
    Ok((config, cli))
}

/// Overlay aus den Argumenten (nie persistiert).
pub fn to_overlay(cli: &Cli) -> pdfover_config::Overlay {
    todo!("Integration: inkl. Normalisierung -n, --kspass-stdin/-env, Prüfung -kskeypass == -kspass")
}

/// Führt die Kommandozeile aus.
pub fn run(config_file: Option<PathBuf>, cli: Cli) -> pdfover_workflow::ExitCode {
    todo!("Integration: Core starten; headless (Keystore/lokale BKU) oder Übergabe an laufende GUI")
}

fn main() {
    let (config_file, cli) = match parse(std::env::args_os()) {
        Ok(v) => v,
        Err(CliError::Clap(e)) => e.exit(),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(pdfover_workflow::ExitCode::Usage as i32);
        }
    };
    let filter = cli.modern.log_level.clone().unwrap_or_else(|| "warn".into());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_writer(std::io::stderr)
        .init();
    let code = run(config_file, cli);
    std::process::exit(code as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Cli {
        let mut v = vec!["pdfover-cli"];
        v.extend_from_slice(args);
        parse(v).expect("parse").1
    }

    #[test]
    fn unknown_tokens_are_input_files() {
        let cli = p(&["a.pdf", "-x", "b.pdf"]);
        assert_eq!(cli.legacy.input, vec![PathBuf::from("a.pdf"), PathBuf::from("-x"), PathBuf::from("b.pdf")]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn legacy_long_options() {
        let cli =
            p(&["-ks", "k.p12", "-kspass", "-secret", "-ksalias", "me", "-proxyport", "8080", "-b", "KS", "-a", "-s"]);
        assert_eq!(cli.legacy.keystore, Some(PathBuf::from("k.p12")));
        assert_eq!(cli.legacy.keystore_password.as_deref(), Some("-secret"));
        assert_eq!(cli.legacy.keystore_alias.as_deref(), Some("me"));
        assert_eq!(cli.legacy.proxy_port, Some(8080));
        assert_eq!(cli.legacy.bku, Some(LegacyBku::Ks));
        assert!(cli.legacy.auto_position && cli.legacy.skip_finish);
    }

    #[test]
    fn bku_is_case_sensitive() {
        assert!(parse(["pdfover-cli", "-b", "mobile"]).is_err());
        assert_eq!(p(&["-b", "MOBILE"]).legacy.bku, Some(LegacyBku::Mobile));
    }

    #[test]
    fn invisible_value_is_evaluated() {
        assert_eq!(p(&["-v", "false"]).legacy.invisible, Some(false));
        assert_eq!(p(&["-v", "true"]).legacy.invisible, Some(true));
    }

    #[test]
    fn two_phase_config() {
        let (cfg, cli) = parse(["pdfover-cli", "x.pdf", "-c", "other.config"]).unwrap();
        assert_eq!(cfg, Some(PathBuf::from("other.config")));
        assert_eq!(cli.legacy.input, vec![PathBuf::from("x.pdf")]);
    }

    #[test]
    fn subcommand_with_legacy_options() {
        let cli = p(&["sign", "-ks", "k.p12", "--headless", "a.pdf", "b.pdf"]);
        assert_eq!(cli.command, Some(Command::Sign { inputs: vec![PathBuf::from("a.pdf"), PathBuf::from("b.pdf")] }));
        assert!(cli.modern.headless);
        assert_eq!(cli.legacy.keystore, Some(PathBuf::from("k.p12")));
    }

    #[test]
    fn question_mark_is_help() {
        let n = normalize_args(["pdfover-cli", "-?"]);
        assert_eq!(n[1], OsString::from("--help"));
    }
}
