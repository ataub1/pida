//! Einstellungen › Signiermittel › Keystore gegen den echten Kern (Nachtrag ADR 0004, Wünsche des
//! Auftraggebers): Format am Dateiinhalt statt Typauswahl (`keystore_format`, `keystore_ready` nur mit
//! PKCS#12), Alias-Auswahl über `inspect_keystore` (richtiges/falsches Passwort, automatisches Lesen mit
//! gemerktem Passwort gemäß Speicherart) und „Passwort merken?“ bei der ersten Verwendung (fragt genau
//! einmal, Bestandsdatei und Übernahme fragen nie, die Wahl wirkt sofort). Passwörter, Aliasse und Namen
//! erscheinen weder im Log noch in `Debug` (Regel 9). Alle Schlüssel: `testdata` („NUR TEST“, Passwort
//! `test`).

mod common;

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use common::*;
use pida_backend_api::{BackendId, SecretKind};
use pida_config::{Overlay, PasswordStorage, SecretId, SecretStore};
use pida_core_api::{
    Core, CoreOptions, InteractionView, KeystoreFormatView, SettingsView, StepView, UiEvent, ViewState,
};
use serde_json::{Value, json};
use zeroize::Zeroizing;

const PDF: &str = "pdf/generated/simple-xref-table.pdf";
const TWO_KEYS: &str = "backend-keystore/java-two-keys.p12";
const RSA: &str = "keystores/test-rsa2048.p12";
const PASSWORD: &str = "test";

// ------------------------------------------------------------------------------------------------
// Log-Mitschnitt (ein globaler Subscriber je Testprozess; der Kern loggt aus Tokio-Threads)
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn logs() -> &'static LogBuffer {
    static LOGS: OnceLock<LogBuffer> = OnceLock::new();
    LOGS.get_or_init(|| {
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .try_init();
        buffer
    })
}

fn log_text() -> String {
    let mut buf = logs().clone();
    let _ = buf.flush();
    String::from_utf8_lossy(&buf.0.lock().unwrap()).into_owned()
}

// ------------------------------------------------------------------------------------------------
// Bausteine
// ------------------------------------------------------------------------------------------------

/// Schlüsselbund für Tests (RAM, meldet sich aber als dauerhaft).
#[derive(Debug, Default)]
struct TestKeychain(pida_config::MemorySecretStore);

impl SecretStore for TestKeychain {
    fn get(&self, id: &SecretId) -> pida_config::Result<Option<Zeroizing<String>>> {
        self.0.get(id)
    }
    fn set(&self, id: &SecretId, value: &str) -> pida_config::Result<()> {
        self.0.set(id, value)
    }
    fn delete(&self, id: &SecretId) -> pida_config::Result<()> {
        self.0.delete(id)
    }
    fn is_persistent(&self) -> bool {
        true
    }
}

fn copy_keystore(fx: &Fixture, rel: &str, name: &str) -> PathBuf {
    let p = fx.dir("ks").join(name);
    std::fs::copy(testdata(rel), &p).unwrap();
    p
}

/// `settings.toml` mit Keystore; `extra` = weitere Zeilen im Abschnitt `[keystore]`.
fn keystore_toml(file: &Path, extra: &str) -> String {
    format!("[keystore]\nenabled = true\nfile = {file:?}\n{extra}\n")
}

/// Kern mit Oberfläche (Arbeitsbereich), eigener Secret-Speicher.
fn start(fx: &Fixture, secrets: Arc<dyn SecretStore>, overlay: Overlay) -> Core {
    logs();
    let opts = CoreOptions {
        workspace_ui: Some(true),
        engine: Some(FakeEngine::new() as Arc<dyn pida_core_api::engine::PadesEngine>),
        secret_store: Some(secrets),
        ..CoreOptions::isolated()
    };
    fx.start(overlay, false, opts)
}

fn settings(v: &ViewState) -> &SettingsView {
    v.settings.as_ref().expect("Einstellungsansicht")
}

async fn open_settings(core: &Core) -> ViewState {
    core.dispatch(UiEvent::OpenSettings);
    wait_for(core, "Einstellungen offen", |v| v.settings.is_some()).await
}

async fn apply(core: &Core, revision: u64, list: &[(&str, Value)]) -> ViewState {
    let changes: BTreeMap<String, Value> = list.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
    core.dispatch(UiEvent::ApplySettings { changes, revision });
    wait_for(core, "übernommen", |v| v.settings.as_ref().is_some_and(|s| s.applied_revision >= revision)).await
}

fn inspect(core: &Core, password: Option<&str>, storage: Option<PasswordStorage>) {
    core.dispatch(UiEvent::InspectKeystore { password: password.map(str::to_owned), password_storage: storage });
}

async fn wait_entries(core: &Core, what: &str) -> ViewState {
    wait_for(core, what, |v| v.settings.as_ref().is_some_and(|s| s.keystore_entries.is_some())).await
}

fn read_settings_file(fx: &Fixture) -> pida_config::Settings {
    pida_config::Settings::from_toml(&std::fs::read_to_string(fx.paths.settings_file()).unwrap()).unwrap()
}

// ------------------------------------------------------------------------------------------------
// 1. Format am Inhalt
// ------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn format_is_detected_by_content_and_only_pkcs12_is_ready() {
    let fx = Fixture::new();
    // Endungen absichtlich irreführend: erkannt wird am Inhalt
    let p12 = copy_keystore(&fx, RSA, "schluessel.jks");
    let jks = copy_keystore(&fx, "backend-keystore/java-rsa2048.jks", "java.p12");
    let jceks = copy_keystore(&fx, "backend-keystore/java-rsa2048.jceks", "java.pfx");
    let text = fx.dir("ks").join("notiz.p12");
    std::fs::write(&text, "kein Keystore").unwrap();
    fx.write_settings(&keystore_toml(&p12, ""));
    let core = start(&fx, Arc::new(TestKeychain::default()), Overlay::default());
    let v = open_settings(&core).await;
    let s = settings(&v);
    assert_eq!(s.keystore_format, Some(KeystoreFormatView::Pkcs12));
    assert!(s.keystore_ready);
    assert_eq!(s.keystore_entries, None, "ohne bekanntes Passwort noch unbekannt");
    assert_eq!(s.keystore_inspect_problem, None);

    let cases = [
        (&jks, Some(KeystoreFormatView::Jks)),
        (&jceks, Some(KeystoreFormatView::Jceks)),
        (&text, Some(KeystoreFormatView::Unknown)),
        (&p12, Some(KeystoreFormatView::Pkcs12)),
    ];
    for (i, (file, format)) in cases.iter().enumerate() {
        let v = apply(&core, i as u64 + 1, &[("keystore.file", json!(file))]).await;
        let s = settings(&v);
        assert_eq!(s.keystore_format, *format, "{}", file.display());
        assert_eq!(s.keystore_ready, *format == Some(KeystoreFormatView::Pkcs12), "{}", file.display());
        // der Typ wird nicht mehr aus der Endung gesetzt (nur PDF-Over-Kompatibilität)
        assert_eq!(s.settings.keystore.declared_type, None);
    }
    // Datei verschwunden: kein Format, nicht wählbar
    std::fs::remove_file(&p12).unwrap();
    let v = apply(&core, 9, &[("keystore.alias", json!("egal"))]).await;
    assert_eq!(settings(&v).keystore_format, None);
    assert!(!settings(&v).keystore_ready);
    std::fs::copy(testdata(RSA), &p12).unwrap();
    // ausgeschaltet: Format bleibt sichtbar, als Standard nicht wählbar
    let v = apply(&core, 10, &[("keystore.enabled", json!(false))]).await;
    assert_eq!(settings(&v).keystore_format, Some(KeystoreFormatView::Pkcs12));
    assert!(!settings(&v).keystore_ready);
    // „Schlüssel anzeigen“ bei JKS: verständliches Problem statt Passwortprüfung
    apply(&core, 11, &[("keystore.enabled", json!(true)), ("keystore.file", json!(jks))]).await;
    inspect(&core, Some(PASSWORD), None);
    let v = wait_for(&core, "Problem", |v| {
        v.settings.as_ref().is_some_and(|s| s.keystore_inspect_problem.as_deref() == Some("keystore-conversion-needed"))
    })
    .await;
    assert_eq!(settings(&v).keystore_entries, None);
}

// ------------------------------------------------------------------------------------------------
// 2. Alias-Auswahl: Prüfen mit Passwort, automatisches Lesen
// ------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn inspect_with_wrong_and_right_password_then_reads_automatically() {
    let fx = Fixture::new();
    let two = copy_keystore(&fx, TWO_KEYS, "zwei.p12");
    let rsa = copy_keystore(&fx, RSA, "rsa.p12");
    // Bestandsdatei ohne `storage_chosen`: Speicherart „bis Programmende“ gilt als gewählt
    fx.write_settings(&keystore_toml(&two, "password_storage = \"memory\""));
    let core = start(&fx, Arc::new(TestKeychain::default()), Overlay::default());
    let v = open_settings(&core).await;
    assert!(settings(&v).settings.keystore.storage_chosen);
    assert_eq!(settings(&v).keystore_entries, None);

    inspect(&core, Some("falsch-geheim-1"), None);
    let v = wait_for(&core, "falsches Passwort", |v| {
        v.settings.as_ref().is_some_and(|s| s.keystore_inspect_problem.as_deref() == Some("keystore-wrong-password"))
    })
    .await;
    assert_eq!(settings(&v).keystore_entries, None);

    inspect(&core, Some(PASSWORD), None);
    let v = wait_entries(&core, "Einträge").await;
    let s = settings(&v);
    assert_eq!(s.keystore_inspect_problem, None);
    let entries = s.keystore_entries.as_ref().unwrap();
    let listed: Vec<_> = entries.iter().map(|e| (e.alias.as_str(), e.is_key_entry)).collect();
    assert_eq!(listed, [("erster", true), ("vertrauensanker", false), ("zweiter", true)]);
    assert_eq!(entries[0].subject_cn.as_deref(), Some("PDF-Over NG Test RSA (NUR TEST)"));
    assert_eq!(entries[2].subject_cn.as_deref(), Some("PDF-Over NG Test ECDSA P-256 (NUR TEST)"));
    let not_after = entries[0].not_after.as_deref().unwrap();
    assert!(time::OffsetDateTime::parse(not_after, &time::format_description::well_known::Rfc3339).is_ok());

    // andere Datei: Einträge unbekannt; zurück: mit dem gemerkten Passwort automatisch gelesen
    let v = apply(&core, 1, &[("keystore.file", json!(rsa))]).await;
    assert_eq!(settings(&v).keystore_entries, None);
    apply(&core, 2, &[("keystore.file", json!(two))]).await;
    let v = wait_entries(&core, "automatisch gelesen").await;
    assert_eq!(settings(&v).keystore_entries.as_ref().map(Vec::len), Some(3));
    // gewählter Alias bleibt Einstellung (`keystore.alias`)
    let v = apply(&core, 3, &[("keystore.alias", json!("zweiter"))]).await;
    assert_eq!(settings(&v).settings.keystore.alias.as_deref(), Some("zweiter"));
    // geschlossen und wieder geöffnet: weiterhin bekannt
    core.dispatch(UiEvent::CloseSettings);
    wait_for(&core, "zu", |v| v.settings.is_none()).await;
    let v = open_settings(&core).await;
    assert!(settings(&v).keystore_entries.is_some());

    // Regel 9: weder Passwort noch Alias oder Name im Log
    let log = log_text();
    assert!(!log.contains("falsch-geheim-1"), "Passwort im Log");
    assert!(!log.contains("vertrauensanker") && !log.contains("NUR TEST"), "Alias/Name im Log");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn storage_none_does_not_keep_the_password() {
    let fx = Fixture::new();
    let two = copy_keystore(&fx, TWO_KEYS, "zwei.p12");
    let rsa = copy_keystore(&fx, RSA, "rsa.p12");
    fx.write_settings(&keystore_toml(&two, "password_storage = \"none\""));
    let core = start(&fx, Arc::new(TestKeychain::default()), Overlay::default());
    open_settings(&core).await;
    inspect(&core, Some(PASSWORD), None);
    wait_entries(&core, "Einträge").await;
    apply(&core, 1, &[("keystore.file", json!(rsa))]).await;
    let v = apply(&core, 2, &[("keystore.file", json!(two))]).await;
    assert_eq!(settings(&v).keystore_entries, None, "Passwort nicht gemerkt");
    // ohne Passwort und ohne gemerktes Passwort: verständliches Problem
    inspect(&core, None, None);
    wait_for(&core, "Passwort nötig", |v| {
        v.settings
            .as_ref()
            .is_some_and(|s| s.keystore_inspect_problem.as_deref() == Some("ng-keystore-password-needed"))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keychain_password_reads_entries_automatically_and_inspect_stores_it() {
    // gespeichertes Passwort im Schlüsselbund: Einträge sofort beim Öffnen
    let fx = Fixture::new();
    let two = copy_keystore(&fx, TWO_KEYS, "zwei.p12");
    fx.write_settings(&keystore_toml(&two, "password_storage = \"keychain\""));
    let keychain = Arc::new(TestKeychain::default());
    keychain.set(&SecretId::keystore_store(&two), PASSWORD).unwrap();
    let core = start(&fx, keychain.clone(), Overlay::default());
    core.dispatch(UiEvent::OpenSettings);
    let v = wait_entries(&core, "Einträge aus dem Schlüsselbund").await;
    assert_eq!(settings(&v).keystore_entries.as_ref().map(Vec::len), Some(3));
    drop(core);

    // leerer Schlüsselbund: „Schlüssel anzeigen“ mit Passwort legt es dort ab
    let fx = Fixture::new();
    let two = copy_keystore(&fx, TWO_KEYS, "zwei.p12");
    fx.write_settings(&keystore_toml(&two, "password_storage = \"keychain\""));
    let keychain = Arc::new(TestKeychain::default());
    let core = start(&fx, keychain.clone(), Overlay::default());
    let v = open_settings(&core).await;
    assert_eq!(settings(&v).keystore_entries, None);
    inspect(&core, Some(PASSWORD), None);
    wait_entries(&core, "Einträge").await;
    let stored = keychain.get(&SecretId::keystore_store(&two)).unwrap();
    assert_eq!(stored.as_deref().map(String::as_str), Some(PASSWORD));
}

// ------------------------------------------------------------------------------------------------
// 3. „Passwort merken?“ bei der ersten Verwendung
// ------------------------------------------------------------------------------------------------

/// Einstellungen für einen Signiervorgang mit dem Keystore aus den Einstellungen (kein Overlay).
fn signing_toml(file: &Path, out: &Path, keystore_extra: &str) -> String {
    format!(
        "[signature]\nauto_position = true\nprofile = \"INVISIBLE\"\n[backend]\ndefault = \"keystore\"\n[output]\nfolder = {out:?}\n{}",
        keystore_toml(file, keystore_extra)
    )
}

fn secret(v: &ViewState) -> Option<(u64, bool, bool, bool)> {
    match &v.step {
        StepView::Signing {
            interaction: InteractionView::Secret { request, prompt, ask_storage, keychain_available },
            ..
        } if prompt.kind == SecretKind::KeystorePassword => {
            Some((*request, prompt.retry, *ask_storage, *keychain_available))
        }
        _ => None,
    }
}

fn saved(v: &ViewState) -> bool {
    matches!(&v.step, StepView::Output { result, .. } if result.path.is_some())
}

/// Dokument öffnen und im Arbeitsbereich „Signieren mit Keystore“ wählen.
async fn sign_with_keystore(core: &Core, fx: &Fixture, name: &str) {
    core.dispatch(UiEvent::OpenFiles { paths: vec![copy_pdf(PDF, &fx.dir("in"), name)], mode: None });
    wait_for(core, "Signieren möglich", |v| v.actions.sign).await;
    core.dispatch(UiEvent::SignWith { backend: BackendId::Keystore });
}

/// Erste Verwendung: genau eine Rückfrage, die Wahl wird gespeichert und gilt schon für dieses Passwort.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn first_use_asks_once_and_the_choice_applies() {
    for choice in [PasswordStorage::Memory, PasswordStorage::None, PasswordStorage::Keychain] {
        let fx = Fixture::new();
        let ks = copy_keystore(&fx, RSA, "k.p12");
        let out = fx.dir("out");
        // wie nach dem ersten Speichern einer Neuinstallation
        fx.write_settings(&signing_toml(&ks, &out, "storage_chosen = false"));
        let keychain = Arc::new(TestKeychain::default());
        let core = start(&fx, keychain.clone(), Overlay::default());
        sign_with_keystore(&core, &fx, "a.pdf").await;
        let v = wait_for(&core, "Passwort", |v| secret(v).is_some()).await;
        let (request, retry, ask, keychain_available) = secret(&v).unwrap();
        assert!(!retry);
        assert!(ask, "erste Verwendung fragt „Passwort merken?“");
        assert!(keychain_available);
        // falsches Passwort mit Wahl: die Wahl gilt, die erneute Abfrage fragt nicht mehr
        core.dispatch(UiEvent::SubmitSecret {
            request,
            secret: "falsch-geheim-2".into(),
            password_storage: Some(choice),
        });
        let v = wait_for(&core, "erneut", |v| secret(v).is_some_and(|(_, retry, ..)| retry)).await;
        let (request, _, ask, _) = secret(&v).unwrap();
        assert!(!ask, "{choice:?}: nur einmal gefragt");
        let file = read_settings_file(&fx);
        assert_eq!(file.keystore.password_storage, choice);
        assert!(file.keystore.storage_chosen);
        core.dispatch(UiEvent::SubmitSecret { request, secret: PASSWORD.into(), password_storage: None });
        wait_for(&core, "gespeichert", saved).await;

        // die Wahl wirkt: gemerkt (RAM bzw. Schlüsselbund) → Einträge sofort bekannt; „nicht speichern“ → nicht
        let in_keychain = keychain.get(&SecretId::keystore_store(&ks)).unwrap();
        assert_eq!(in_keychain.is_some(), choice == PasswordStorage::Keychain, "{choice:?}");
        core.dispatch(UiEvent::OpenSettings);
        let v = wait_for(&core, "Einstellungen", |v| v.settings.is_some()).await;
        let s = settings(&v);
        assert!(s.settings.keystore.storage_chosen);
        if choice == PasswordStorage::None {
            assert_eq!(s.keystore_entries, None, "nicht gemerkt");
        } else {
            wait_entries(&core, "mit gemerktem Passwort").await;
        }
        drop(core);

        // nächster Start: keine Rückfrage mehr
        let core = start(&fx, keychain.clone(), Overlay::default());
        sign_with_keystore(&core, &fx, "b.pdf").await;
        if choice == PasswordStorage::Keychain {
            // Passwort aus dem Schlüsselbund: ohne Abfrage signiert
            wait_for(&core, "gespeichert ohne Abfrage", saved).await;
        } else {
            let v = wait_for(&core, "Passwort", |v| secret(v).is_some()).await;
            assert!(!secret(&v).unwrap().2, "{choice:?}: nach der Wahl nie mehr gefragt");
        }
    }
    assert!(!log_text().contains("falsch-geheim-2"), "Passwort im Log");
}

/// Bestandsdatei (ohne `storage_chosen`) und Datei aus dem Overlay fragen nie; ohne Antwort (klassische
/// Oberfläche) bleibt es beim Standard „bis Programmende“ und die nächste Abfrage fragt wieder.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn existing_settings_and_overlay_files_never_ask() {
    let fx = Fixture::new();
    let ks = copy_keystore(&fx, RSA, "k.p12");
    let out = fx.dir("out");
    fx.write_settings(&signing_toml(&ks, &out, ""));
    let core = start(&fx, Arc::new(TestKeychain::default()), Overlay::default());
    sign_with_keystore(&core, &fx, "a.pdf").await;
    let v = wait_for(&core, "Passwort", |v| secret(v).is_some()).await;
    assert!(!secret(&v).unwrap().2, "Bestandsdatei: keine Rückfrage");
    // eine Antwort ohne Rückfrage ist ohne Wirkung
    core.dispatch(UiEvent::SubmitSecret {
        request: secret(&v).unwrap().0,
        secret: PASSWORD.into(),
        password_storage: Some(PasswordStorage::None),
    });
    wait_for(&core, "gespeichert", saved).await;
    assert_eq!(read_settings_file(&fx).keystore.password_storage, PasswordStorage::Memory);
    drop(core);

    // Datei aus dem Overlay (`-ks`): Passwörter werden ohnehin nie dauerhaft gemerkt
    let fx = Fixture::new();
    let ks = copy_keystore(&fx, RSA, "k.p12");
    let out = fx.dir("out");
    fx.write_settings(&signing_toml(&fx.dir("ks").join("andere.p12"), &out, "storage_chosen = false"));
    let overlay = Overlay { keystore_file: Some(ks), ..Overlay::default() };
    let core = start(&fx, Arc::new(TestKeychain::default()), overlay);
    sign_with_keystore(&core, &fx, "a.pdf").await;
    let v = wait_for(&core, "Passwort", |v| secret(v).is_some()).await;
    assert!(!secret(&v).unwrap().2, "Overlay-Datei: keine Rückfrage");
}

/// „Schlüssel anzeigen“ in den Einstellungen beantwortet die Frage ebenfalls (erste Verwendung).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn inspect_answers_the_storage_question() {
    let fx = Fixture::new();
    let two = copy_keystore(&fx, TWO_KEYS, "zwei.p12");
    fx.write_settings(&keystore_toml(&two, "storage_chosen = false"));
    let keychain = Arc::new(TestKeychain::default());
    let core = start(&fx, keychain.clone(), Overlay::default());
    let v = open_settings(&core).await;
    assert!(!settings(&v).settings.keystore.storage_chosen);
    assert!(settings(&v).keychain_available);
    inspect(&core, Some(PASSWORD), Some(PasswordStorage::Keychain));
    let v = wait_entries(&core, "Einträge").await;
    let s = &settings(&v).settings.keystore;
    assert_eq!(s.password_storage, PasswordStorage::Keychain);
    assert!(s.storage_chosen);
    assert!(keychain.get(&SecretId::keystore_store(&two)).unwrap().is_some());
    assert!(read_settings_file(&fx).keystore.storage_chosen);
    // später ohne Wirkung (bereits gewählt)
    inspect(&core, Some(PASSWORD), Some(PasswordStorage::None));
    let v = wait_entries(&core, "Einträge").await;
    assert_eq!(settings(&v).settings.keystore.password_storage, PasswordStorage::Keychain);
}

/// Neuinstallation ohne Einstellungsdatei: noch nicht gewählt; eine Wahl in den Einstellungen gilt als Wahl.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fresh_install_is_unchosen_until_settings_choice() {
    let fx = Fixture::new();
    let core = start(&fx, Arc::new(TestKeychain::default()), Overlay::default());
    let v = open_settings(&core).await;
    assert!(!settings(&v).settings.keystore.storage_chosen);
    let v = apply(&core, 1, &[("keystore.password_storage", json!("none"))]).await;
    assert!(settings(&v).settings.keystore.storage_chosen);
    assert_eq!(settings(&v).settings.keystore.password_storage, PasswordStorage::None);
}

#[test]
fn debug_and_serde_of_new_events_hide_the_password() {
    let e = UiEvent::InspectKeystore {
        password: Some("geheim-inspect".into()),
        password_storage: Some(PasswordStorage::Keychain),
    };
    let dbg = format!("{e:?}");
    assert!(!dbg.contains("geheim-inspect") && dbg.contains("Keychain"), "{dbg}");
    let e = UiEvent::SubmitSecret {
        request: 1,
        secret: "geheim-submit".into(),
        password_storage: Some(PasswordStorage::None),
    };
    assert!(!format!("{e:?}").contains("geheim-submit"));
    // additiv: ältere Aufrufer ohne `password_storage`
    let old: UiEvent = serde_json::from_value(json!({"type": "submit_secret", "request": 2, "secret": "x"})).unwrap();
    assert_eq!(old, UiEvent::SubmitSecret { request: 2, secret: "x".into(), password_storage: None });
    let parsed: UiEvent = serde_json::from_value(json!({"type": "inspect_keystore", "password": null})).unwrap();
    assert_eq!(parsed, UiEvent::InspectKeystore { password: None, password_storage: None });
    let entry = pida_core_api::KeystoreEntryView {
        alias: "max-alias".into(),
        subject_cn: Some("Max Mustermann".into()),
        not_after: None,
        is_key_entry: true,
    };
    let dbg = format!("{entry:?}");
    assert!(!dbg.contains("max-alias") && !dbg.contains("Mustermann"), "{dbg}");
}
