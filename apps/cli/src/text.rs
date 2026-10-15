//! Texte der Kommandozeile (DE/EN) über die Fluent-Kataloge; neue `ng-cli-*`-IDs haben bis zur
//! Aufnahme in `crates/i18n/locales/*/pdfover-ng.ftl` (Paket I4) eingebaute Ersatztexte.

use std::collections::BTreeMap;

use pdfover_i18n::{ArgValue, Catalog, Lang, markup};

/// Eingebaute Texte neuer IDs: (ID, Deutsch, Englisch). Platzhalter wie in Fluent: `{ $name }`.
pub const NG_TEXTS: &[(&str, &str, &str)] = &[
    (
        "ng-cli-help-title",
        "PDF-Over NG – PDF-Dokumente elektronisch signieren",
        "PDF-Over NG – sign PDF documents electronically",
    ),
    (
        "ng-cli-help-usage",
        "Aufruf: pdfover-cli [Optionen] [Dateien …] | pdfover-cli <Befehl> [Optionen]",
        "Usage: pdfover-cli [options] [files …] | pdfover-cli <command> [options]",
    ),
    ("ng-cli-help-options", "Optionen (kompatibel zu PDF-Over):", "Options (compatible with PDF-Over):"),
    ("ng-cli-help-new-options", "Neue Optionen:", "New options:"),
    ("ng-cli-help-commands", "Befehle:", "Commands:"),
    (
        "ng-cli-help-headless",
        "Ohne Rückfragen signieren (Keystore mit -kspass/--kspass-stdin/--kspass-env, lokale BKU); ohne Terminal automatisch",
        "Sign without prompts (keystore with -kspass/--kspass-stdin/--kspass-env, local BKU); automatic without a terminal",
    ),
    ("ng-cli-help-json", "Ergebnis maschinenlesbar (JSON) auf stdout ausgeben", "Print the result as JSON on stdout"),
    (
        "ng-cli-help-kspass-stdin",
        "Keystore-Passwort von stdin lesen (erste Zeile)",
        "Read the keystore password from stdin (first line)",
    ),
    (
        "ng-cli-help-kspass-env",
        "Keystore-Passwort aus der Umgebungsvariable lesen",
        "Read the keystore password from the environment variable",
    ),
    (
        "ng-cli-help-log-level",
        "Log-Stufe (error, warn, info, debug, trace)",
        "Log level (error, warn, info, debug, trace)",
    ),
    ("ng-cli-help-version", "Version anzeigen", "Show the version"),
    (
        "ng-cli-help-cmd-sign",
        "Dokumente signieren (Dateien, Ordner, Muster wie *.pdf)",
        "Sign documents (files, folders, patterns such as *.pdf)",
    ),
    ("ng-cli-help-cmd-verify", "Signaturen eines PDFs prüfen", "Verify the signatures of a PDF"),
    ("ng-cli-help-cmd-profiles", "Signaturprofile auflisten", "List signature profiles"),
    (
        "ng-cli-help-cmd-migrate",
        "Einstellungen aus ~/.pdf-over übernehmen (--from <Ordner>, --dry-run)",
        "Import settings from ~/.pdf-over (--from <folder>, --dry-run)",
    ),
    (
        "ng-cli-help-cmd-render-block",
        "Signaturblock als PDF/PNG ausgeben (--profile <ID> <Datei>)",
        "Write the signature block as PDF/PNG (--profile <ID> <file>)",
    ),
    (
        "ng-cli-help-exit-codes",
        "Exit-Codes: 0 erfolgreich, 1 allgemeiner Fehler, 2 Aufruf, 3 Dokument abgelehnt, 4 abgebrochen, 5 Signiermittel/Netz, 6 Selbstprüfung (nichts gespeichert), 7 Schreibfehler",
        "Exit codes: 0 success, 1 general error, 2 usage, 3 document rejected, 4 cancelled, 5 signing device/network, 6 self-check (nothing saved), 7 write error",
    ),
    (
        "ng-cli-no-input",
        "Kein Dokument angegeben (Hilfe: pdfover-cli -h).",
        "No document given (help: pdfover-cli -h).",
    ),
    (
        "ng-cli-keypass-mismatch",
        "-kskeypass weicht von -kspass ab: PKCS#12 hat nur ein Passwort. Bitte den Keystore mit einheitlichem Passwort exportieren.",
        "-kskeypass differs from -kspass: PKCS#12 has a single password. Please export the keystore with one password.",
    ),
    (
        "ng-cli-kspass-env-missing",
        "Die Umgebungsvariable { $name } ist nicht gesetzt.",
        "The environment variable { $name } is not set.",
    ),
    (
        "ng-cli-stdin-failed",
        "Passwort von stdin nicht lesbar: { $detail }",
        "Could not read the password from stdin: { $detail }",
    ),
    ("ng-cli-signed", "signiert: { $input } → { $output }", "signed: { $input } → { $output }"),
    ("ng-cli-cancelled", "abgebrochen: { $input }", "cancelled: { $input }"),
    ("ng-cli-failed", "fehlgeschlagen: { $input }: { $message }", "failed: { $input }: { $message }"),
    (
        "ng-cli-overwrite",
        "Datei { $name } existiert bereits. Überschreiben? [j/N] ",
        "File { $name } already exists. Overwrite? [y/N] ",
    ),
    (
        "ng-cli-privacy",
        "Das Dokument wird zum Signieren an A-Trust übermittelt (Datenschutzerklärung: https://www.a-trust.at/datenschutz). Fortfahren? [j/N] ",
        "The document is sent to A-Trust for signing (privacy policy: https://www.a-trust.at/datenschutz). Continue? [y/N] ",
    ),
    ("ng-cli-retry", "Wiederholen? [j/N] ", "Retry? [y/N] "),
    ("ng-cli-continue", "Fortfahren? [j/N] ", "Continue? [y/N] "),
    ("ng-cli-username", "Benutzername/Telefonnummer: ", "Username/phone number: "),
    ("ng-cli-password", "Passwort: ", "Password: "),
    ("ng-cli-tan", "TAN (Vergleichswert { $reference }): ", "TAN (reference value { $reference }): "),
    (
        "ng-cli-qr",
        "Vergleichswert { $reference }: Bitte den QR-Code mit der App „ID Austria“ scannen ({ $path }).",
        "Reference value { $reference }: please scan the QR code with the ID Austria app ({ $path }).",
    ),
    (
        "ng-cli-app",
        "Vergleichswert { $reference }: Bitte in der App „ID Austria“ bestätigen.",
        "Reference value { $reference }: please confirm in the ID Austria app.",
    ),
    ("ng-cli-countdown", "noch { $seconds } s …", "{ $seconds } s left …"),
    ("ng-cli-fido2", "FIDO2: Bitte den Sicherheitsschlüssel berühren …", "FIDO2: please touch your security key …"),
    ("ng-cli-keystore-password", "Keystore-Passwort für { $name }: ", "Keystore password for { $name }: "),
    ("ng-cli-key-password", "Schlüssel-Passwort für { $name }: ", "Key password for { $name }: "),
    ("ng-cli-proxy-password", "Proxy-Passwort für { $name }: ", "Proxy password for { $name }: "),
    ("ng-cli-fido2-pin", "PIN des Sicherheitsschlüssels: ", "Security key PIN: "),
    ("ng-cli-wrong-password", "Passwort falsch.", "Wrong password."),
    ("ng-cli-progress", "… { $step }", "… { $step }"),
    ("ng-cli-valid", "gültig: { $field } ({ $signer })", "valid: { $field } ({ $signer })"),
    ("ng-cli-invalid", "ungültig: { $field } ({ $error })", "invalid: { $field } ({ $error })"),
    ("ng-cli-no-signatures", "Keine Signaturen gefunden.", "No signatures found."),
    ("ng-cli-written", "geschrieben: { $path }", "written: { $path }"),
    (
        "ng-cli-no-legacy",
        "Keine PDF-Over-Konfiguration gefunden ({ $path }).",
        "No PDF-Over configuration found ({ $path }).",
    ),
    ("ng-cli-migrate-dry-run", "Probelauf – es wurde nichts geschrieben.", "Dry run – nothing was written."),
    (
        "ng-cli-unknown-profile",
        "Profil { $id } existiert nicht (Liste: pdfover-cli profiles).",
        "Profile { $id } does not exist (list: pdfover-cli profiles).",
    ),
    // Workflow / Kern (neue IDs der Integration, Welle 2)
    (
        "ng-invariant-violated",
        "Die Selbstprüfung der Signatur ist fehlgeschlagen ({ $invariant }: { $detail }). Es wurde nichts gespeichert.",
        "The signature self-check failed ({ $invariant }: { $detail }). Nothing was saved.",
    ),
    (
        "ng-cms-too-large",
        "Die Signatur ist größer als der reservierte Platz ({ $needed } statt { $reserve } Bytes). „Wiederholen“ bereitet das Dokument mit mehr Platz neu vor; dafür ist eine erneute Bestätigung nötig.",
        "The signature is larger than the reserved space ({ $needed } instead of { $reserve } bytes). “Retry” prepares the document again with more space; this needs another confirmation.",
    ),
    (
        "ng-backend-choice-required",
        "Mehrere Signiermittel verfügbar ({ $detail }) – bitte mit -b wählen.",
        "Several signing devices are available ({ $detail }) – please choose one with -b.",
    ),
    ("ng-no-backend-available", "Kein Signiermittel verfügbar.", "No signing device is available."),
    (
        "ng-default-backend-unavailable",
        "Das gewählte Signiermittel ({ $detail }) ist nicht verfügbar.",
        "The chosen signing device ({ $detail }) is not available.",
    ),
    ("ng-document-rejected", "Das Dokument kann nicht signiert werden.", "The document cannot be signed."),
    (
        "ng-pdf-no-sign-rights",
        "Die Rechte des Dokuments erlauben kein Signieren.",
        "The document permissions do not allow signing.",
    ),
    ("ng-field-already-signed", "Das Signaturfeld ist bereits signiert.", "The signature field is already signed."),
    ("ng-field-locked", "Das Signaturfeld ist gesperrt.", "The signature field is locked."),
    (
        "ng-certificate-not-valid",
        "Das Zertifikat ist zum Signierzeitpunkt nicht gültig.",
        "The certificate is not valid at the time of signing.",
    ),
    (
        "ng-clock-skew",
        "Die Uhr dieses Geräts weicht um etwa { $minutes } Minuten von der Zeit des Signaturdienstes ab.",
        "This device's clock differs from the signing service by about { $minutes } minutes.",
    ),
    (
        "ng-atrust-login-rejected",
        "Die Anmeldung wurde abgelehnt. Bitte das Passwort erneut eingeben.",
        "The login was rejected. Please enter the password again.",
    ),
    (
        "ng-verify-failed",
        "Die Datei konnte nicht geprüft werden: { $detail }",
        "The file could not be verified: { $detail }",
    ),
    (
        "ng-verify-no-signature-dictionary",
        "Signaturfeld ohne Signaturdaten.",
        "Signature field without signature data.",
    ),
    (
        "ng-verify-no-byte-range",
        "Signatur ohne /ByteRange bzw. /Contents.",
        "Signature without /ByteRange or /Contents.",
    ),
    ("ng-verify-byte-range", "Die /ByteRange der Signatur ist ungültig.", "The signature /ByteRange is invalid."),
    ("ng-progress-connecting", "Verbindung wird aufgebaut", "Connecting"),
    ("ng-progress-certificate", "Zertifikat wird gelesen", "Reading the certificate"),
    ("ng-progress-waiting", "Warten auf den Signaturdienst", "Waiting for the signing service"),
    ("ng-progress-document", "Dokument { $index } von { $total }", "Document { $index } of { $total }"),
    ("ng-progress-done", "Fertig", "Done"),
    // Backend-Codes (`BackendError::code`)
    ("backend-cancelled", "Der Vorgang wurde abgebrochen.", "The operation was cancelled."),
    (
        "backend-unavailable",
        "Das Signiermittel ist nicht verfügbar: { $detail }",
        "The signing device is not available: { $detail }",
    ),
    (
        "backend-network",
        "Die Verbindung zum Signaturdienst ist fehlgeschlagen: { $detail }",
        "The connection to the signing service failed: { $detail }",
    ),
    ("backend-proxy-auth", "Der Proxy verlangt eine Anmeldung.", "The proxy requires authentication."),
    (
        "backend-document-too-large",
        "Das Dokument überschreitet die Größengrenze des Signaturdienstes.",
        "The document exceeds the size limit of the signing service.",
    ),
    (
        "backend-http",
        "Der Signaturdienst antwortet mit HTTP { $detail }.",
        "The signing service answered with HTTP { $detail }.",
    ),
    (
        "backend-security-layer",
        "Vorgang vom Signaturserver abgebrochen ({ $detail }).",
        "Operation aborted by the signature server ({ $detail }).",
    ),
    (
        "backend-unknown-page",
        "Unbekannte Seite des Signaturdienstes ({ $detail }).",
        "Unknown page of the signing service ({ $detail }).",
    ),
    (
        "backend-loop",
        "Endlosschleife im Ablauf des Signaturdienstes erkannt.",
        "Infinite loop detected in the signing service flow.",
    ),
    (
        "backend-protocol",
        "Unerwartete Antwort des Signaturdienstes: { $detail }",
        "Unexpected answer from the signing service: { $detail }",
    ),
    ("backend-wrong-password", "Das Passwort ist falsch.", "The password is wrong."),
    ("backend-keystore", "Keystore: { $detail }", "Keystore: { $detail }"),
    (
        "backend-keystore-format",
        "Keystore-Format { $detail } wird nicht unterstützt – bitte nach PKCS#12 konvertieren.",
        "Keystore format { $detail } is not supported – please convert it to PKCS#12.",
    ),
    (
        "backend-certificate-mismatch",
        "Das Zertifikat weicht vom erwarteten ab.",
        "The certificate differs from the expected one.",
    ),
    ("backend-fido2", "FIDO2: { $detail }", "FIDO2: { $detail }"),
    ("backend-timeout", "Zeitüberschreitung beim Signaturdienst.", "The signing service timed out."),
    ("backend-internal", "Interner Fehler: { $detail }", "Internal error: { $detail }"),
];

/// Ersetzt `{ $name }` durch die Argumente.
fn substitute(text: &str, args: &BTreeMap<String, String>) -> String {
    let mut out = text.to_owned();
    for (k, v) in args {
        out = out.replace(&format!("{{ ${k} }}"), v);
    }
    out
}

/// Text zu einer Meldungs-ID (Katalog, sonst eingebaut, sonst `None`).
pub fn lookup(lang: Lang, id: &str, args: &BTreeMap<String, String>) -> Option<String> {
    let catalog = Catalog::global();
    let fluent_id = id.replace('.', "-");
    for candidate in [id, fluent_id.as_str()] {
        if catalog.has(lang, candidate) || catalog.has(Lang::De, candidate) {
            let fargs: Vec<(&str, ArgValue)> =
                args.iter().map(|(k, v)| (k.as_str(), ArgValue::Str(v.clone()))).collect();
            return Some(markup::to_plain(&catalog.format(lang, candidate, &fargs)));
        }
    }
    NG_TEXTS.iter().find(|(i, ..)| *i == id).map(|(_, de, en)| {
        substitute(
            match lang {
                Lang::De => de,
                Lang::En => en,
            },
            args,
        )
    })
}

/// Text zu einer ID; unbekannte IDs erscheinen als `id (detail)`.
pub fn tr(lang: Lang, id: &str, args: &[(&str, String)]) -> String {
    let map: BTreeMap<String, String> = args.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
    tr_map(lang, id, &map)
}

/// Wie [`tr`] mit Parameter-Map (Fehler des Kerns).
pub fn tr_map(lang: Lang, id: &str, args: &BTreeMap<String, String>) -> String {
    match lookup(lang, id, args) {
        Some(t) => t,
        None => match args.get("detail").filter(|d| !d.is_empty()) {
            Some(d) => format!("{id} ({d})"),
            None => id.to_owned(),
        },
    }
}

/// Antwort „ja“ (j/ja/y/yes, ohne Groß-/Kleinschreibung).
pub fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "j" | "ja" | "y" | "yes")
}

/// Zweisprachige Hilfe (`-h`, `-?`, `--help`; F-108), in der Systemsprache.
pub fn help(lang: Lang) -> String {
    let t = |id: &str| tr(lang, id, &[]);
    let legacy: &[(&str, &str)] = &[
        ("-h, -?, --help", "argument-help-help"),
        ("-i <datei>", "argument-help-input"),
        ("-o <ordner|datei>", "argument-help-output"),
        ("-b <LOCAL|MOBILE|KS|NONE>", "argument-help-bku"),
        ("-n <nummer>", "argument-help-number"),
        ("-p <passwort>", "argument-help-password"),
        ("-ks <datei>", "argument-help-keystorefile"),
        ("-kstype <typ>", "argument-help-keystoretype"),
        ("-kspass <passwort>", "argument-help-keystorestorepass"),
        ("-ksalias <alias>", "argument-help-keystorealias"),
        ("-kskeypass <passwort>", "argument-help-keystorekeypass"),
        ("-proxy <host>", "argument-help-proxyhost"),
        ("-proxyport <port>", "argument-help-proxyport"),
        ("-proxyuser <benutzer>", "argument-help-proxyuser"),
        ("-proxypass <passwort>", "argument-help-proxypass"),
        ("-e <bild>", "argument-help-emblem"),
        ("-a", "argument-help-autopos"),
        ("-s", "argument-help-skipfinish"),
        ("-c <datei>", "argument-help-config"),
        ("-v <wert>", "argument-help-vis"),
    ];
    let modern: &[(&str, &str)] = &[
        ("--headless", "ng-cli-help-headless"),
        ("--json", "ng-cli-help-json"),
        ("--kspass-stdin", "ng-cli-help-kspass-stdin"),
        ("--kspass-env <variable>", "ng-cli-help-kspass-env"),
        ("--log-level <stufe>", "ng-cli-help-log-level"),
        ("--version", "ng-cli-help-version"),
    ];
    let commands: &[(&str, &str)] = &[
        ("sign <eingaben…>", "ng-cli-help-cmd-sign"),
        ("verify <datei>", "ng-cli-help-cmd-verify"),
        ("profiles", "ng-cli-help-cmd-profiles"),
        ("migrate", "ng-cli-help-cmd-migrate"),
        ("render-block", "ng-cli-help-cmd-render-block"),
    ];
    let width = legacy.iter().chain(modern).chain(commands).map(|(o, _)| o.chars().count()).max().unwrap_or(0) + 2;
    let mut out = String::new();
    let mut section = |title: &str, rows: &[(&str, &str)]| {
        out.push_str(title);
        out.push('\n');
        for (opt, id) in rows {
            let pad = width - opt.chars().count();
            out.push_str(&format!("  {opt}{}{}\n", " ".repeat(pad), t(id).replace('\n', " ")));
        }
        out.push('\n');
    };
    let header = format!("{}\n{}\n\n", t("ng-cli-help-title"), t("ng-cli-help-usage"));
    section(&format!("{header}{}", t("ng-cli-help-options")), legacy);
    section(&t("ng-cli-help-new-options"), modern);
    section(&t("ng-cli-help-commands"), commands);
    out.push_str(&t("ng-cli-help-exit-codes"));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_in_both_languages() {
        let de = help(Lang::De);
        let en = help(Lang::En);
        assert!(de.contains("Optionen"), "{de}");
        assert!(en.contains("Options"), "{en}");
        for opt in ["-kskeypass", "-proxyport", "--kspass-env", "render-block", "-v <wert>"] {
            assert!(de.contains(opt) && en.contains(opt), "{opt}");
        }
        // B-08: Hilfetexte von -kspass/-kskeypass richtig zugeordnet
        let line = |text: &str, opt: &str| text.lines().find(|l| l.trim_start().starts_with(opt)).unwrap().to_owned();
        assert!(line(&en, "-kskeypass").to_lowercase().contains("key password"), "{}", line(&en, "-kskeypass"));
        assert!(!line(&en, "-kspass").to_lowercase().contains("key password"), "{}", line(&en, "-kspass"));
        assert!(!de.contains("!argument"), "fehlende Katalogeinträge");
    }

    #[test]
    fn translation_fallbacks() {
        assert_eq!(
            tr(Lang::En, "ng-cli-kspass-env-missing", &[("name", "X".into())]),
            "The environment variable X is not set."
        );
        assert!(tr(Lang::De, "error-FileNotExist", &[("arg1", "/a".into())]).contains("/a"));
        let mut m = BTreeMap::new();
        m.insert("detail".to_owned(), "x".to_owned());
        assert_eq!(tr_map(Lang::De, "gibt-es-nicht", &m), "gibt-es-nicht (x)");
        assert!(is_yes(" J ") && is_yes("yes") && !is_yes("n") && !is_yes(""));
    }
}
