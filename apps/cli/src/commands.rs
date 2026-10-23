//! Subcommands `profiles`, `verify`, `migrate`, `render-block`, `licenses` (Plan §7.4).

use std::path::{Path, PathBuf};

use pida_config::Paths;
use pida_i18n::Lang;
use pida_workflow::ExitCode;

use crate::term::Terminal;
use crate::text::tr;

/// `profiles`: die 12 PDF-Over-Profil-IDs.
pub fn profiles(term: &dyn Terminal, json: bool) -> ExitCode {
    let ids = pida_core_api::tools::profile_ids();
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

/// `verify <datei>` (Entwickler-Werkzeug, ADR 0004 Nachtrag 15): alle Signaturen mit dem eigenen Verifier prüfen –
/// unverändert und kryptografisch intakt; Vertrauenskette und Sperrstatus prüft es **nicht** (der Text sagt das
/// ausdrücklich, nie „gültig“). Exit 0 = alle intakt (bzw. keine Signatur), 6 = mindestens eine nicht intakt,
/// 3 = Datei nicht lesbar.
pub fn verify(term: &dyn Terminal, lang: Lang, file: &Path, json: bool) -> ExitCode {
    let checks = match pida_core_api::tools::verify_file(file) {
        Ok(c) => c,
        Err(e) => {
            term.info(&format!("{}: {e}", tr(lang, "error-MayNotBeAPDF", &[])));
            return ExitCode::Rejected;
        }
    };
    let intact = checks.iter().all(|c| c.intact);
    if json {
        let v = serde_json::json!({ "file": file, "intact": intact, "trust_checked": false, "signatures": checks });
        term.out(&serde_json::to_string_pretty(&v).unwrap_or_default());
    } else if checks.is_empty() {
        term.out(&tr(lang, "ng-cli-no-signatures", &[]));
    } else {
        for c in &checks {
            if c.intact {
                let signer = c.signer.clone().unwrap_or_default();
                term.out(&tr(lang, "ng-cli-verify-intact", &[("field", c.field.clone()), ("signer", signer)]));
            } else {
                let error = c.error.clone().unwrap_or_default();
                term.out(&tr(lang, "ng-cli-verify-not-intact", &[("field", c.field.clone()), ("error", error)]));
            }
        }
        term.out(&tr(lang, "ng-cli-verify-scope", &[]));
    }
    if intact { ExitCode::Success } else { ExitCode::Invariant }
}

/// `render-block --profile <ID> <datei.pdf|png>`.
pub fn render_block(term: &dyn Terminal, lang: Lang, profile: &str, output: &Path) -> ExitCode {
    let pdf = match pida_core_api::tools::block_pdf(profile) {
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
        match pida_core_api::tools::render_png(&pdf, 4.0, None) {
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
    let file = dir.as_deref().and_then(pida_config::migrate::find_legacy_config);
    let Some(file) = file else {
        let shown = dir.map(|d| d.display().to_string()).unwrap_or_else(|| "~/.pdf-over".into());
        term.info(&tr(lang, "ng-cli-no-legacy", &[("path", shown)]));
        return ExitCode::Usage;
    };
    let legacy = match pida_config::LegacyConfig::load(&file) {
        Ok(l) => l,
        Err(e) => {
            term.info(&e.localized(lang));
            return ExitCode::General;
        }
    };
    let legacy_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    let ctx = pida_config::MigrationContext { legacy_dir, system_language: lang, check_paths: true };
    let result = pida_config::migrate(&legacy, &ctx);
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
        let store = pida_config::MemorySecretStore::default();
        let opts = pida_config::ApplyOptions { store_secrets: false, ..pida_config::ApplyOptions::default() };
        match pida_config::apply_migration(&result, &paths, &store, &opts) {
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

use crate::notices::{EUPL, NOTICES, notice};

/// `licenses [-o <ordner>]` (Lizenzbericht Befunde 3, 4): ohne `-o` den EUPL-1.2-Text und die Drittlizenz-Hinweise auf
/// stdout (die HTML-Hinweise der Rust-Standardbibliothek nur mit `-o`), mit `-o` `LICENSE.txt`,
/// `THIRD-PARTY-NOTICES.md` und `RUST-STD-COPYRIGHT.html` in den Ordner schreiben – wie in den Desktop-Paketen.
pub fn licenses(term: &dyn Terminal, lang: Lang, output: Option<&Path>) -> ExitCode {
    let notices: Vec<(&str, Option<Vec<u8>>)> = NOTICES.iter().map(|(name, packed)| (*name, notice(packed))).collect();
    let embedded = notices.iter().all(|(_, data)| data.is_some());
    let Some(dir) = output else {
        term.out(EUPL.trim_end());
        match &notices[0].1 {
            Some(md) => {
                term.out("");
                term.out(String::from_utf8_lossy(md).trim_end());
                term.info(&tr(lang, "ng-cli-licenses-rust-std", &[]));
            }
            None => term.info(&tr(lang, "ng-cli-licenses-missing", &[])),
        }
        return ExitCode::Success;
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        term.info(&tr(lang, "output-save_failed", &[("arg1", dir.display().to_string()), ("arg2", e.to_string())]));
        return ExitCode::Write;
    }
    let mut files: Vec<(&str, Vec<u8>)> = vec![("LICENSE.txt", EUPL.as_bytes().to_vec())];
    files.extend(notices.into_iter().filter_map(|(name, data)| data.map(|d| (name, d))));
    for (name, data) in files {
        let path = dir.join(name);
        if let Err(e) = std::fs::write(&path, data) {
            term.info(&tr(
                lang,
                "output-save_failed",
                &[("arg1", path.display().to_string()), ("arg2", e.to_string())],
            ));
            return ExitCode::Write;
        }
        term.out(&tr(lang, "ng-cli-written", &[("path", path.display().to_string())]));
    }
    if !embedded {
        term.info(&tr(lang, "ng-cli-licenses-missing", &[]));
    }
    ExitCode::Success
}
