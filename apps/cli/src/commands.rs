//! Subcommands `profiles`, `verify`, `migrate`, `render-block` (Plan §7.4).

use std::path::{Path, PathBuf};

use pdfover_config::Paths;
use pdfover_i18n::Lang;
use pdfover_workflow::ExitCode;

use crate::term::Terminal;
use crate::text::tr;

/// `profiles`: die 12 PDF-Over-Profil-IDs.
pub fn profiles(term: &dyn Terminal, json: bool) -> ExitCode {
    let ids = pdfover_core_api::tools::profile_ids();
    if json {
        let v: Vec<serde_json::Value> =
            ids.iter().map(|(id, ok)| serde_json::json!({ "id": id, "available": ok })).collect();
        term.out(&serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        for (id, ok) in &ids {
            term.out(if *ok { id } else { "?" });
        }
    }
    if ids.iter().all(|(_, ok)| *ok) { ExitCode::Success } else { ExitCode::General }
}

/// `verify <datei>`: alle Signaturen mit dem eigenen Verifier prüfen. Exit 0 = alle gültig,
/// 6 = mindestens eine ungültig, 3 = Datei nicht lesbar.
pub fn verify(term: &dyn Terminal, lang: Lang, file: &Path, json: bool) -> ExitCode {
    let checks = match pdfover_core_api::tools::verify_file(file) {
        Ok(c) => c,
        Err(e) => {
            term.info(&format!("{}: {e}", tr(lang, "error-MayNotBeAPDF", &[])));
            return ExitCode::Rejected;
        }
    };
    let ok = checks.iter().all(|c| c.valid);
    if json {
        let v = serde_json::json!({ "file": file, "valid": ok, "signatures": checks });
        term.out(&serde_json::to_string_pretty(&v).unwrap_or_default());
    } else if checks.is_empty() {
        term.out(&tr(lang, "ng-cli-no-signatures", &[]));
    } else {
        for c in &checks {
            if c.valid {
                let signer = c.signer.clone().unwrap_or_default();
                term.out(&tr(lang, "ng-cli-valid", &[("field", c.field.clone()), ("signer", signer)]));
            } else {
                let error = c.error.clone().unwrap_or_default();
                term.out(&tr(lang, "ng-cli-invalid", &[("field", c.field.clone()), ("error", error)]));
            }
        }
    }
    if ok { ExitCode::Success } else { ExitCode::Invariant }
}

/// `render-block --profile <ID> <datei.pdf|png>`.
pub fn render_block(term: &dyn Terminal, lang: Lang, profile: &str, output: &Path) -> ExitCode {
    let pdf = match pdfover_core_api::tools::block_pdf(profile) {
        Ok(p) => p,
        Err(e) => {
            if e.code() == "sigblock-unknown-profile" {
                term.info(&tr(lang, "ng-cli-unknown-profile", &[("id", profile.to_owned())]));
                return ExitCode::Usage;
            }
            term.info(&crate::text::tr(lang, &e.code(), &[("detail", e.to_string())]));
            return ExitCode::General;
        }
    };
    let png = output.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
    let bytes = if png {
        match pdfover_core_api::tools::render_png(&pdf, 4.0, None) {
            Ok(b) => b,
            Err(e) => {
                term.info(&tr(lang, e.message_id(), &[("detail", e.to_string())]));
                return ExitCode::General;
            }
        }
    } else {
        pdf
    };
    if let Err(e) = std::fs::write(output, bytes) {
        term.info(&tr(lang, "output-save_failed", &[("arg1", output.display().to_string()), ("arg2", e.to_string())]));
        return ExitCode::Write;
    }
    term.out(&tr(lang, "ng-cli-written", &[("path", output.display().to_string())]));
    ExitCode::Success
}

/// `migrate [--from <ordner>] [--dry-run]`: Import aus `~/.pdf-over` (Plan §7.2). Klartext-Passwörter
/// werden ohne Rückfrage nicht übernommen (Zustimmung nur in der GUI).
pub fn migrate(
    term: &dyn Terminal,
    lang: Lang,
    from: Option<PathBuf>,
    dry_run: bool,
    paths: Option<Paths>,
    json: bool,
) -> ExitCode {
    let dir = from.or_else(Paths::legacy_dir);
    let file = dir.as_deref().and_then(pdfover_config::migrate::find_legacy_config);
    let Some(file) = file else {
        let shown = dir.map(|d| d.display().to_string()).unwrap_or_else(|| "~/.pdf-over".into());
        term.info(&tr(lang, "ng-cli-no-legacy", &[("path", shown)]));
        return ExitCode::Usage;
    };
    let legacy = match pdfover_config::LegacyConfig::load(&file) {
        Ok(l) => l,
        Err(e) => {
            term.info(&e.localized(lang));
            return ExitCode::General;
        }
    };
    let legacy_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    let ctx = pdfover_config::MigrationContext { legacy_dir, system_language: lang, check_paths: true };
    let result = pdfover_config::migrate(&legacy, &ctx);
    let messages: Vec<String> = result.report.messages.iter().map(|m| m.localized(lang)).collect();
    let mut written = None;
    if !dry_run {
        let paths = match paths.map(Ok).unwrap_or_else(Paths::platform) {
            Ok(p) => p,
            Err(e) => {
                term.info(&e.localized(lang));
                return ExitCode::General;
            }
        };
        let store = pdfover_config::MemorySecretStore::default();
        let opts = pdfover_config::ApplyOptions { store_secrets: false, ..pdfover_config::ApplyOptions::default() };
        match pdfover_config::apply_migration(&result, &paths, &store, &opts) {
            Ok(a) => written = Some(a.settings_file),
            Err(e) => {
                term.info(&e.localized(lang));
                return ExitCode::General;
            }
        }
    }
    if json {
        let v = serde_json::json!({
            "legacy_file": file,
            "imported": result.report.imported.len(),
            "skipped": result.report.skipped.len(),
            "messages": messages,
            "settings_file": written,
            "dry_run": dry_run,
        });
        term.out(&serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        for m in &messages {
            term.out(m);
        }
        let done = tr(
            lang,
            "ng-migrate-done",
            &[
                ("imported", result.report.imported.len().to_string()),
                ("skipped", result.report.skipped.len().to_string()),
            ],
        );
        term.out(&done);
        match written {
            Some(p) => term.out(&tr(lang, "ng-cli-written", &[("path", p.display().to_string())])),
            None => term.out(&tr(lang, "ng-cli-migrate-dry-run", &[])),
        }
    }
    ExitCode::Success
}
