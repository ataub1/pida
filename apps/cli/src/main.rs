//! Einstieg der Kommandozeile `pida-cli` (Logik in der Bibliothek `pida_cli`).

use std::ffi::OsString;

use pida_cli::{CliError, normalize_args, parse, run, text, wants_help};

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let lang = pida_i18n::Lang::system();
    if wants_help(&normalize_args(args.clone())) {
        print!("{}", text::help(lang));
        std::process::exit(pida_workflow::ExitCode::Success.code());
    }
    let (config_file, cli) = match parse(args) {
        Ok(v) => v,
        Err(CliError::Clap(e))
            if matches!(e.kind(), clap::error::ErrorKind::DisplayVersion | clap::error::ErrorKind::DisplayHelp) =>
        {
            e.exit()
        }
        Err(e) => {
            eprintln!("{}\n{e}", text::tr(lang, "error-CmdLineArgs", &[]));
            std::process::exit(pida_workflow::ExitCode::Usage.code());
        }
    };
    let level = cli.modern.log_level.clone().unwrap_or_else(|| "warn".into());
    let _log = match pida_config::Paths::platform() {
        Ok(paths) => pida_core_api::logging::init(
            &paths,
            &pida_core_api::logging::LogOptions { level, stderr: true, file: true },
        ),
        Err(_) => pida_core_api::logging::init(
            &pida_config::Paths::under(&std::env::temp_dir()),
            &pida_core_api::logging::LogOptions { level, stderr: true, file: false },
        ),
    };
    let code = run(config_file, cli);
    drop(_log);
    std::process::exit(code.code());
}
