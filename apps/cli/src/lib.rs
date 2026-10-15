//! `pida-cli` – Kommandozeile von PIDA (Plan §7.4; docs/analyse/01 §5), als Bibliothek
//! auch der Argument-Parser der GUI (`apps/app/src-tauri`, Feature `run` aus): dieselbe
//! PDF-Over-Semantik ([`parse`], [`to_overlay`], [`overlay::inputs`]) für beide Oberflächen.
//!
//! Kompatibel zu PDF-Over: `-h/-?/--help -i -o -b -n -p -ks -kstype -kspass -ksalias -kskeypass
//! -proxy -proxyport -proxyuser -proxypass -e -a -s -c -v`, inklusive der Quirks
//! - **zweiphasiges `-c`**: Pass 1 wertet nur `-c` aus (relativ zum Konfigurationsordner; Altdateien
//!   unter `~/.pdf-over/<name>` werden mit Java-Properties-Semantik gelesen, ohne zurückzuschreiben),
//!   Pass 2 alle Argumente als Overlay,
//! - **unbekanntes Token = Eingabedatei** (davon leben „Öffnen mit“ und „Senden an“), auch `-x`,
//! - Werte werden nicht auf führendes `-` geprüft.
//!
//! Korrekturen: `-v` wertet `false`/`0`/`no`/`nein` als sichtbar aus, jeder andere Wert bleibt wie bei
//! PDF-Over „unsichtbar“ (B-07, kompatibel zu Skripten mit `-v x`), Hilfetexte von `-kspass`/`-kskeypass`
//! richtig (B-08), fehlende `-c`-Datei ist ein Fehler statt Werksreset, abweichendes `-kskeypass`
//! ergibt einen klaren Fehler. Neu: Subcommands, `--headless`, `--json`, `--kspass-stdin`,
//! `--kspass-env`.
//!
//! Ausführung ([`run`]): Signieren läuft über `pida-core-api` ohne grafische Oberfläche; die CLI
//! beantwortet Masken und Passwortabfragen im Terminal ([`session`]). Ohne Terminal bzw. mit
//! `--headless` gibt es keine Rückfragen (Keystore mit Passwort aus `-kspass`, `--kspass-stdin`,
//! `--kspass-env`; lokale BKU). Die Hilfe (`-h`, `-?`, `--help`) erscheint in der Systemsprache
//! (DE/EN, [`text::help`]). Exit-Codes nach Plan §7.4 (`pida_workflow::ExitCode`).

#[cfg(feature = "run")]
mod commands;
pub mod overlay;
#[cfg(feature = "run")]
pub mod session;
#[cfg(feature = "run")]
pub mod term;
pub mod text;

use std::ffi::OsString;
use std::path::PathBuf;
#[cfg(feature = "run")]
use std::sync::Arc;

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

/// Optionen der Subcommands mit Wert (`migrate --from`, `render-block --profile`).
pub const SUBCOMMAND_WITH_VALUE: [&str; 2] = ["--from", "--profile"];

/// Flags der Subcommands (`migrate --dry-run`).
pub const SUBCOMMAND_FLAGS: [&str; 1] = ["--dry-run"];

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
    /// unsichtbare Signatur (`false`/`0`/`no`/`nein` = sichtbar, jeder andere Wert = unsichtbar; B-07)
    #[arg(short = 'v', long = "invisible", value_name = "true|false", global = true, value_parser = parse_invisible)]
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
#[command(name = "pida-cli", display_name = "PIDA", version, about = "PIDA – PDF-Signatur mit ID Austria")]
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

/// `-v <wert>`: PDF-Over ignoriert den Wert und signiert immer unsichtbar (B-07). Korrigiert werden nur
/// die eindeutig verneinenden Werte; alles andere bleibt „unsichtbar“, damit vorhandene Skripte
/// (`-v x`, `-v 1`) unverändert funktionieren.
fn parse_invisible(s: &str) -> Result<bool, String> {
    Ok(pida_config::overlay::invisible_from_cli_value(s))
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
        } else if MODERN_WITH_VALUE.contains(&s.as_ref())
            || (subcommand_seen && SUBCOMMAND_WITH_VALUE.contains(&s.as_ref()))
        {
            with_value(&s, next)
        } else if (subcommand_seen && SUBCOMMAND_FLAGS.contains(&s.as_ref()))
            || (s.starts_with("--") && s.contains('='))
        {
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

/// Overlay aus den Argumenten (nie persistiert): `-n` normalisiert (`0…` → `+43…`), `-b NONE` = keine
/// Vorgabe, `-v` nach B-07. Prüfungen (`-p` 5–200, `-kskeypass` = `-kspass`, Dateien vorhanden)
/// macht [`overlay::check`], Passwörter aus `--kspass-stdin`/`--kspass-env` [`overlay::keystore_password`].
pub fn to_overlay(cli: &Cli) -> pida_config::Overlay {
    overlay::to_overlay(cli)
}

/// Hilfe angefordert (`-h`, `-?` → `--help`, `--help`)?
pub fn wants_help(normalized: &[OsString]) -> bool {
    normalized.iter().skip(1).any(|t| t == "-h" || t == "--help")
}

/// Führt die Kommandozeile aus.
#[cfg(feature = "run")]
pub fn run(config_file: Option<PathBuf>, cli: Cli) -> pida_workflow::ExitCode {
    let env =
        session::RunEnv { term: Arc::new(term::Tty), paths: None, options: pida_core_api::CoreOptions::default() };
    run_with(config_file, cli, env)
}

/// Wie [`run`] mit vorgegebener Umgebung (Tests).
#[cfg(feature = "run")]
pub fn run_with(config_file: Option<PathBuf>, cli: Cli, env: session::RunEnv) -> pida_workflow::ExitCode {
    let lang = pida_i18n::Lang::system();
    let json = cli.modern.json;
    match &cli.command {
        Some(Command::Profiles) => commands::profiles(env.term.as_ref(), json),
        Some(Command::Verify { file }) => commands::verify(env.term.as_ref(), lang, file, json),
        Some(Command::RenderBlock { profile, output }) => {
            commands::render_block(env.term.as_ref(), lang, profile, output)
        }
        Some(Command::Migrate { from, dry_run }) => {
            commands::migrate(env.term.as_ref(), lang, from.clone(), *dry_run, env.paths.clone(), json)
        }
        Some(Command::Sign { .. }) | None => session::sign(config_file, &cli, env),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Cli {
        let mut v = vec!["pida-cli"];
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
        assert!(parse(["pida-cli", "-b", "mobile"]).is_err());
        assert_eq!(p(&["-b", "MOBILE"]).legacy.bku, Some(LegacyBku::Mobile));
    }

    #[test]
    fn invisible_value_is_evaluated() {
        assert_eq!(p(&["-v", "false"]).legacy.invisible, Some(false));
        assert_eq!(p(&["-v", "true"]).legacy.invisible, Some(true));
        // PDF-Over-kompatibel: beliebiger Wert bleibt „unsichtbar“.
        assert_eq!(p(&["-v", "x"]).legacy.invisible, Some(true));
        assert!(p(&[]).legacy.invisible.is_none());
    }

    #[test]
    fn two_phase_config() {
        let (cfg, cli) = parse(["pida-cli", "x.pdf", "-c", "other.config"]).unwrap();
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
        let n = normalize_args(["pida-cli", "-?"]);
        assert_eq!(n[1], OsString::from("--help"));
        assert!(wants_help(&n));
        assert!(wants_help(&normalize_args(["pida-cli", "x.pdf", "-h"])));
        assert!(!wants_help(&normalize_args(["pida-cli", "x.pdf"])));
    }
}

/// Ablauf der CLI mit Fake-Engine und echtem Keystore.
#[cfg(all(test, feature = "run"))]
mod run_tests {
    use std::path::Path;

    use pida_core_api::CoreOptions;
    use pida_core_api::testing::{FakeEngine, verify_fake_output};
    use pida_workflow::ExitCode;

    use super::*;
    use crate::session::RunEnv;
    use crate::term::Scripted;

    const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata");

    struct Fx {
        tmp: tempfile::TempDir,
    }

    impl Fx {
        fn new() -> Fx {
            pida_i18n::Lang::set_system(Some(pida_i18n::Lang::De));
            Fx { tmp: tempfile::tempdir().unwrap() }
        }

        fn pdf(&self, name: &str) -> PathBuf {
            let dir = self.tmp.path().join("in");
            std::fs::create_dir_all(&dir).unwrap();
            let p = dir.join(name);
            std::fs::copy(Path::new(TESTDATA).join("pdf/generated/simple-xref-table.pdf"), &p).unwrap();
            p
        }

        fn run(&self, args: &[&str], term: Arc<Scripted>) -> ExitCode {
            self.run_with_options(
                args,
                term,
                CoreOptions { engine: Some(FakeEngine::new()), ..CoreOptions::isolated() },
            )
        }

        fn run_with_options(&self, args: &[&str], term: Arc<Scripted>, options: CoreOptions) -> ExitCode {
            let mut v = vec!["pida-cli"];
            v.extend_from_slice(args);
            let (config_file, cli) = parse(v).expect("parse");
            let env = RunEnv { term, paths: Some(pida_config::Paths::under(&self.tmp.path().join("app"))), options };
            run_with(config_file, cli, env)
        }
    }

    /// Fake-ID-Austria ohne Netz (`document_leaves_device`): Die Sitzung öffnet, das Zertifikat wird
    /// „abgebrochen“ – so endet der Lauf ohne Kontakt zu A-Trust.
    #[derive(Debug, Default)]
    struct FakeIdAustria {
        opened: std::sync::atomic::AtomicUsize,
    }

    struct CancelledSession;

    #[async_trait::async_trait]
    impl pida_backend_api::SignatureBackend for FakeIdAustria {
        fn id(&self) -> pida_backend_api::BackendId {
            pida_backend_api::BackendId::AtrustNative
        }

        fn capabilities(&self, env: &pida_backend_api::Environment) -> Option<pida_backend_api::Capabilities> {
            use pida_backend_api as b;
            (!env.headless).then(|| b::Capabilities {
                input: b::SignInput::Document,
                output: b::SignOutput::Cms,
                certificate: b::CertNeed::OnlyForVisibleBlock,
                interaction: b::Interaction::NativeForms,
                server: b::ServerNeed::None,
                max_batch: 1,
                login_once_per_batch: true,
                document_leaves_device: true,
                qualification: b::Qualification::Qes,
                factors: b::FactorSet { app: true, sms: true, ..b::FactorSet::default() },
                cms_reserve: Some(16 * 1024),
            })
        }

        async fn open(
            &self,
            _ctx: pida_backend_api::SessionContext,
            _ui: Arc<dyn pida_backend_api::InteractionBroker>,
        ) -> Result<Box<dyn pida_backend_api::BackendSession>, pida_backend_api::BackendError> {
            self.opened.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Box::new(CancelledSession))
        }
    }

    #[async_trait::async_trait]
    impl pida_backend_api::BackendSession for CancelledSession {
        async fn certificate(&mut self) -> Result<pida_backend_api::CertChain, pida_backend_api::BackendError> {
            Err(pida_backend_api::BackendError::Cancelled)
        }

        async fn sign(
            &mut self,
            _jobs: Vec<pida_backend_api::SignJob>,
        ) -> Result<Vec<pida_backend_api::SignResult>, pida_backend_api::BackendError> {
            Err(pida_backend_api::BackendError::Cancelled)
        }

        async fn cancel(&mut self) {}
    }

    /// `-b MOBILE` im Terminal: kein Datenschutzdialog mehr, stattdessen eine einzeilige Info (nicht
    /// blockierend, keine „[j/N]“-Rückfrage); die Sitzung startet ohne weitere Eingabe (ADR 0003).
    #[test]
    fn id_austria_prints_a_one_line_privacy_info_without_asking() {
        let fx = Fx::new();
        let a = fx.pdf("a.pdf");
        let term = Arc::new(Scripted::new(true, &[]));
        let fake = Arc::new(FakeIdAustria::default());
        let options = CoreOptions {
            engine: Some(FakeEngine::new()),
            backends: Some(vec![fake.clone() as Arc<dyn pida_backend_api::SignatureBackend>]),
            ..CoreOptions::isolated()
        };
        let code = fx.run_with_options(&["-b", "MOBILE", a.to_str().unwrap()], term.clone(), options);
        assert_eq!(code, ExitCode::Cancelled, "{}", term.stderr());
        assert_eq!(fake.opened.load(std::sync::atomic::Ordering::SeqCst), 1, "Sitzung ohne Rückfrage geöffnet");
        let info: Vec<String> =
            term.err.lock().unwrap().iter().filter(|l| l.contains("an A-Trust übermittelt")).cloned().collect();
        assert_eq!(info.len(), 1, "{}", term.stderr());
        assert!(!info[0].contains('\n') && info[0].contains("datenschutzerklaerung.pdf"), "{}", info[0]);
        assert!(term.prompts.lock().unwrap().is_empty(), "keine Rückfrage: {:?}", term.prompts.lock().unwrap());
    }

    fn ks() -> String {
        format!("{TESTDATA}/keystores/test-rsa2048.p12")
    }

    #[test]
    fn headless_keystore_signing_with_json() {
        let fx = Fx::new();
        let a = fx.pdf("a.pdf");
        let b = fx.pdf("b.pdf");
        let term = Arc::new(Scripted::new(false, &[]));
        let ks = ks();
        let code = fx.run(
            &["--headless", "--json", "-ks", &ks, "-kspass", "test", a.to_str().unwrap(), "-i", b.to_str().unwrap()],
            term.clone(),
        );
        assert_eq!(code, ExitCode::Success, "{}", term.stderr());
        let original = std::fs::read(&a).unwrap();
        verify_fake_output(&a.with_file_name("a_signed.pdf"), &original);
        verify_fake_output(&b.with_file_name("b_signed.pdf"), &original);
        let json: serde_json::Value = serde_json::from_str(&term.stdout()).unwrap();
        assert_eq!(json["exit_code"], 0);
        assert_eq!(json["documents"].as_array().unwrap().len(), 2);
        assert_eq!(json["documents"][0]["result"], "signed");
    }

    #[test]
    fn sign_subcommand_with_folder_and_output() {
        let fx = Fx::new();
        let a = fx.pdf("a.pdf");
        fx.pdf("b.pdf");
        let out = fx.tmp.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let term = Arc::new(Scripted::new(false, &[]));
        let ks = ks();
        let dir = a.parent().unwrap().to_str().unwrap().to_owned();
        let code = fx.run(
            &["sign", "-ks", &ks, "-kspass", "test", "-o", out.to_str().unwrap(), "-v", "true", &dir],
            term.clone(),
        );
        assert_eq!(code, ExitCode::Success, "{}", term.stderr());
        assert!(out.join("a_signed.pdf").is_file() && out.join("b_signed.pdf").is_file());
        assert!(term.stdout().contains("signiert"), "{}", term.stdout());
    }

    #[test]
    fn interactive_keystore_password_prompt() {
        let fx = Fx::new();
        let a = fx.pdf("a.pdf");
        let term = Arc::new(Scripted::new(true, &["falsch", "test"]));
        let ks = ks();
        let code = fx.run(&["-ks", &ks, a.to_str().unwrap()], term.clone());
        assert_eq!(code, ExitCode::Success, "{}", term.stderr());
        let prompts = term.prompts.lock().unwrap().clone();
        assert_eq!(prompts.len(), 2, "{prompts:?}");
        assert!(prompts[0].contains("test-rsa2048.p12"));
        assert!(term.stderr().contains("Passwort falsch"), "{}", term.stderr());
    }

    #[test]
    fn existing_output_overwrite_question() {
        let fx = Fx::new();
        let a = fx.pdf("a.pdf");
        let target = a.with_file_name("a_signed.pdf");
        std::fs::write(&target, b"alt").unwrap();
        let ks = ks();
        // ohne Terminal: nicht überschreiben → abgebrochen
        let term = Arc::new(Scripted::new(false, &[]));
        let code = fx.run(&["-ks", &ks, "-kspass", "test", a.to_str().unwrap()], term.clone());
        assert_eq!(code, ExitCode::Cancelled);
        assert_eq!(std::fs::read(&target).unwrap(), b"alt");
        // mit Terminal: „j“ → überschrieben
        let term = Arc::new(Scripted::new(true, &["j"]));
        let code = fx.run(&["-ks", &ks, "-kspass", "test", a.to_str().unwrap()], term.clone());
        assert_eq!(code, ExitCode::Success, "{}", term.stderr());
        verify_fake_output(&target, &std::fs::read(&a).unwrap());
    }

    #[test]
    fn usage_errors() {
        let fx = Fx::new();
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&[], term.clone()), ExitCode::Usage);
        assert!(term.stderr().contains("Kein Dokument"), "{}", term.stderr());
        let a = fx.pdf("a.pdf");
        let a = a.to_str().unwrap();
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["-c", "fehlt.config", a], term.clone()), ExitCode::Usage);
        assert!(term.stderr().contains("fehlt.config"), "{}", term.stderr());
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["-kspass", "a", "-kskeypass", "b", a], term.clone()), ExitCode::Usage);
        assert!(term.stderr().contains("-kskeypass"), "{}", term.stderr());
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["-p", "kurz", a], term), ExitCode::Usage);
        // abweichendes -kskeypass auch bei Passwort aus der Umgebung bzw. stdin (Plan §4.9)
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["--kspass-env", "PATH", "-kskeypass", "anders", a], term.clone()), ExitCode::Usage);
        assert!(term.stderr().contains("-kskeypass"), "{}", term.stderr());
        let term = Arc::new(Scripted::new(false, &["vom-stdin"]));
        assert_eq!(fx.run(&["--kspass-stdin", "-kskeypass", "anders", a], term.clone()), ExitCode::Usage);
    }

    #[test]
    fn wrong_password_headless_reports_failure() {
        let fx = Fx::new();
        let a = fx.pdf("a.pdf");
        let term = Arc::new(Scripted::new(false, &[]));
        let ks = ks();
        let code = fx.run(&["--headless", "-ks", &ks, "-kspass", "falsch", a.to_str().unwrap()], term.clone());
        assert_eq!(code, ExitCode::Backend);
        assert!(term.stderr().contains("Passwort ist falsch"), "{}", term.stderr());
    }

    #[test]
    fn subcommands() {
        let fx = Fx::new();
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["profiles"], term.clone()), ExitCode::Success);
        assert_eq!(term.out.lock().unwrap().len(), 12);
        let term = Arc::new(Scripted::new(false, &[]));
        let signed = format!("{TESTDATA}/pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf");
        assert_eq!(fx.run(&["--json", "verify", &signed], term.clone()), ExitCode::Success);
        let v: serde_json::Value = serde_json::from_str(&term.stdout()).unwrap();
        assert_eq!(v["valid"], true);
        let out = fx.tmp.path().join("block.pdf");
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(
            fx.run(&["render-block", "--profile", "SIGNATURBLOCK_SMALL_EN", out.to_str().unwrap()], term.clone()),
            ExitCode::Success
        );
        assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF-"));
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["render-block", "--profile", "NEIN", out.to_str().unwrap()], term), ExitCode::Usage);
    }

    #[test]
    fn migrate_dry_run_and_apply() {
        let fx = Fx::new();
        let legacy = fx.tmp.path().join("legacy");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::copy(Path::new(TESTDATA).join("config/pdf-over-generated-de.config"), legacy.join("PDF-Over.config"))
            .unwrap();
        let settings = fx.tmp.path().join("app/config/settings.toml");
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(
            fx.run(&["migrate", "--from", legacy.to_str().unwrap(), "--dry-run"], term.clone()),
            ExitCode::Success
        );
        assert!(!settings.exists());
        assert!(term.stdout().contains("Probelauf"), "{}", term.stdout());
        let term = Arc::new(Scripted::new(false, &[]));
        assert_eq!(fx.run(&["migrate", "--from", legacy.to_str().unwrap()], term.clone()), ExitCode::Success);
        let s = pida_config::Settings::from_toml(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s.id_austria.username.as_deref(), Some("+436641234567"));
        let term = Arc::new(Scripted::new(false, &[]));
        let empty = fx.tmp.path().join("leer");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(fx.run(&["migrate", "--from", empty.to_str().unwrap()], term), ExitCode::Usage);
    }
}
