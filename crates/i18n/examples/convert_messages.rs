//! Werkzeug: konvertiert eine `messages*.properties` von PDF-Over nach Fluent.
//!
//! ```text
//! cargo run -p pida-i18n --example convert_messages -- <de|en> <messages.properties> [<ausgabe.ftl>]
//! ```
//! Ohne Ausgabedatei wird das FTL auf stdout geschrieben; Warnungen gehen nach stderr.
//! Für die eingebetteten Kataloge siehe `tests/catalogs.rs` (`PIDA_I18N_BLESS=1`).

use std::process::ExitCode;

use pida_i18n::Lang;
use pida_i18n::convert::convert_pdfover_messages;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(lang), Some(input)) = (args.first().and_then(|l| Lang::from_tag(l)), args.get(1)) else {
        eprintln!("Aufruf: convert_messages <de|en> <messages.properties> [<ausgabe.ftl>]");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let conversion = match convert_pdfover_messages(&bytes, lang) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    for w in &conversion.warnings {
        eprintln!("Warnung: {w}");
    }
    match args.get(2) {
        Some(out) => {
            if let Err(e) = std::fs::write(out, &conversion.ftl) {
                eprintln!("{out}: {e}");
                return ExitCode::FAILURE;
            }
            eprintln!("{} Nachrichten nach {out} geschrieben", conversion.messages);
        }
        None => print!("{}", conversion.ftl),
    }
    ExitCode::SUCCESS
}
