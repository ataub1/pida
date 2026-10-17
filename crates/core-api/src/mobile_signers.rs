//! Mobile nur mit ID Austria (Entscheidung des Auftraggebers vom 16.10.2026, ADR 0004 Nachtrag 11): „Signieren
//! mit Keystore brauchen wir auf mobilen Apps nicht, da reicht ID Austria.“ Der Desktop behält den Keystore
//! unverändert; die Chipkarte (Chipkarten-Software) war schon nur am Desktop.
//!
//! Auf iOS/Android gilt deshalb:
//!
//! - **Registrierung:** kein Keystore-Backend (`Registry::configure`, auch nicht unter fest vorgegebenen
//!   Backends); Signiermittel-Auswahl, Konto-Chip, Stapel und Ersteinrichtung bieten nur ID Austria an
//!   (`SignersView` ohne Platzhalter „Keystore einrichten“).
//! - **Standard-Signiermittel:** das einzige Signiermittel ist ID Austria – [`ConfigState::default_backend`]
//!   liefert es immer (sofern die Richtlinie es erlaubt). Ein gespeichertes `backend.default = keystore`
//!   (bzw. `local_bku`) eines früheren Mobile-Builds wird beim Lesen auf `id_austria` abgebildet
//!   ([`map_default_signer`]); der Abschnitt `[keystore]` bleibt in der Datei unberührt (nur ignoriert), die
//!   Desktop-Datei bleibt unverändert lesbar.
//! - **Einstellungen:** `ApplySettings` lehnt `keystore.*` und `backend.default = keystore` mit
//!   `ng-settings-unavailable-mobile` ab, `SaveSettings` übernimmt den Abschnitt `[keystore]` nicht;
//!   `SettingsView` ohne Keystore-Angaben (`keystore_ready = false`, kein Format, keine Einträge, keine
//!   Probleme unter `keystore.*`).
//! - **Ereignisse:** `import_keystore`, `inspect_keystore`, die Antwort auf „Passwort merken?“ und jede Wahl
//!   des Keystores (`choose_backend`, `select_signer`, `sign_with`, `start_batch`) sind ohne Wirkung (Notiz im
//!   Log), ebenso `select_signer` mit der Chipkarte – [`ignored_on_mobile`].
//! - **Übernahme aus PDF-Over:** ohne Keystore-Datei, Alias und Keystore-Passwörter, Standard-Signiermittel
//!   ID Austria ([`restrict_migration`]); die Übernahmeliste erklärt das mit [`MSG_MIGRATION_ITEM_SIGNER`].
//! - **Sicherer Speicher:** gespeicherte Keystore-Passwörter eines früheren Mobile-Builds entfernt der Kern
//!   beim ersten Start einmalig ([`CoreTask::remove_former_keystore_secrets`]; nur Einträge von PIDA, Merker
//!   [`SECRETS_REMOVED_MARKER`] im Datenordner).
//! - **„Signatur fortsetzen“:** ein gesicherter Keystore-Vorgang eines früheren Builds wird beim Start
//!   verworfen ([`MSG_RESUME_SIGNER_NOT_OFFERED`]) statt angeboten.

use std::path::{Path, PathBuf};

use pida_backend_api::{BackendId, Platform};
use pida_config::{
    BackendChoice, KeystoreSettings, LegacyKey, MigrationResult, ReportMessage, SecretId, SecretStore, Settings,
};
use pida_i18n::Lang;

use crate::runtime::{CoreTask, KEYSTORE_DIR};
use crate::view::UiEvent;

/// Meldungs-ID (Übernahmeliste der Ersteinrichtung bzw. des Angebots, Mobile): signiert wird mit ID Austria,
/// andere Signiermittel aus PDF-Over (Keystore, Chipkarte) gibt es auf dem Gerät nicht.
pub(crate) const MSG_MIGRATION_ITEM_SIGNER: &str = "ng-migration-item-signer-mobile";
/// Meldungs-ID (Bericht der Übernahme): ein Keystore-Schlüssel der Altdatei wird auf Mobilgeräten nicht
/// übernommen.
pub(crate) const MSG_MIGRATE_KEYSTORE_MOBILE: &str = "ng-migrate-keystore-mobile";
/// Meldungs-ID: eine unterbrochene Signatur mit einem Signiermittel, das es in der App nicht mehr gibt
/// (Keystore eines früheren Mobile-Builds), wurde verworfen.
pub(crate) const MSG_RESUME_SIGNER_NOT_OFFERED: &str = "ng-resume-signer-not-offered";
/// Merker im Datenordner: die Keystore-Passwörter eines früheren Mobile-Builds sind entfernt.
pub(crate) const SECRETS_REMOVED_MARKER: &str = "keystore-secrets-removed";

/// Schlüssel des Keystores in `PDF-Over.config`.
const KEYSTORE_KEYS: [LegacyKey; 7] = [
    LegacyKey::KeystoreEnabled,
    LegacyKey::KeystoreFile,
    LegacyKey::KeystoreType,
    LegacyKey::KeystoreAlias,
    LegacyKey::KeystorePassStoreType,
    LegacyKey::KeystoreStorepass,
    LegacyKey::KeystoreKeypass,
];

/// Signieren mit Keystore auf dieser Plattform? Desktop ja, iOS/Android nein (nur ID Austria).
pub(crate) fn keystore_offered(platform: Platform) -> bool {
    platform.is_desktop()
}

/// Standard-Signiermittel, das es auf Mobilgeräten nicht gibt (Keystore, Chipkarte), → ID Austria. Der
/// Abschnitt `[keystore]` bleibt unberührt. `true` = geändert.
pub(crate) fn map_default_signer(settings: &mut Settings) -> bool {
    match settings.backend.default {
        Some(BackendChoice::Keystore | BackendChoice::LocalBku) => {
            settings.backend.default = Some(BackendChoice::IdAustria);
            true
        }
        _ => false,
    }
}

/// Hätte die Übernahme auf Mobile etwas am Signiermittel geändert (Keystore eingerichtet bzw. Standard
/// Keystore/Chipkarte)?
fn other_signer(settings: &Settings) -> bool {
    settings.keystore.enabled
        || settings.keystore.file.is_some()
        || matches!(settings.backend.default, Some(BackendChoice::Keystore | BackendChoice::LocalBku))
}

/// Übernahme aus PDF-Over auf Mobile: ohne Keystore (Datei, Alias, Typ, Speicherart, Passwörter), Standard-
/// Signiermittel ID Austria; die Keystore-Schlüssel des Berichts gelten als nicht übernommen. `true` = am
/// Signiermittel wurde etwas verworfen (die Übernahmeliste nennt dann [`MSG_MIGRATION_ITEM_SIGNER`]).
pub(crate) fn restrict_migration(r: &mut MigrationResult, lang: Lang) -> bool {
    let dropped = other_signer(&r.settings);
    r.settings.keystore = KeystoreSettings::default();
    map_default_signer(&mut r.settings);
    r.secrets.retain(|(id, _)| matches!(id, SecretId::ProxyPassword { .. }));
    r.report.secrets_found.retain(|k| !KEYSTORE_KEYS.contains(k));
    let mut skipped = Vec::new();
    r.report.imported.retain(|k| {
        let keystore = KEYSTORE_KEYS.contains(k);
        if keystore {
            skipped.push(*k);
        }
        !keystore
    });
    for key in skipped {
        let msg = ReportMessage { id: MSG_MIGRATE_KEYSTORE_MOBILE, key: Some(key), args: Vec::new() };
        r.report.skipped.push((key, msg.localized(lang)));
        r.report.messages.push(msg);
    }
    dropped
}

/// UI-Ereignis, das auf Mobilgeräten ohne Wirkung bleibt? Liefert die Art für das Log. Das sind alle Ereignisse
/// zum Keystore und die Wahl der Chipkarte im Konto-Chip (`select_signer` – sie wählte sonst ein Signiermittel
/// bzw. speicherte mit `make_default` einen Standard, den es auf dem Gerät nicht gibt; `sign_with` und
/// `start_batch` mit der Chipkarte melden wie bisher „nicht verfügbar“).
pub(crate) fn ignored_on_mobile(e: &UiEvent) -> Option<&'static str> {
    match e {
        UiEvent::ImportKeystore { .. } => Some("import_keystore"),
        UiEvent::InspectKeystore { .. } => Some("inspect_keystore"),
        UiEvent::ChooseBackend { backend: BackendId::Keystore } => Some("choose_backend"),
        UiEvent::SignWith { backend: BackendId::Keystore } => Some("sign_with"),
        UiEvent::StartBatch { backend: BackendId::Keystore, .. } => Some("start_batch"),
        UiEvent::SelectSigner { backend: Some(BackendId::Keystore | BackendId::LocalBku), .. } => Some("select_signer"),
        _ => None,
    }
}

/// Passwort eines verworfenen Ereignisses ([`ignored_on_mobile`]) sofort überschreiben (wie beim Verarbeiten:
/// `Zeroizing`), statt den Klartext einfach freizugeben.
pub(crate) fn discard_event(e: UiEvent) {
    if let UiEvent::ImportKeystore { password: Some(p), .. } | UiEvent::InspectKeystore { password: Some(p), .. } = e {
        drop(zeroize::Zeroizing::new(p));
    }
}

/// Kennungen der Keystore-Passwörter, die ein früherer Mobile-Build im sicheren Speicher abgelegt haben kann:
/// zur Datei aus den Einstellungen (auch in der Schreibweise `/var` ↔ `/private/var`, iOS) und zu jeder
/// importierten Datei im Ordner `imported` (`data_dir/keystores`, auch nach einem Umzug der Sandbox), je
/// kanonisch und wie gespeichert, mit dem eingestellten bzw. leeren Alias und jedem Alias aus `aliases_of`
/// (Schlüssel der Datei: ein früherer Build legte zu jedem gewählten Schlüssel ein Schlüsselpasswort ab, auch
/// wenn die Einstellungen inzwischen einen anderen nennen).
fn former_keystore_secret_ids(
    keystore: &KeystoreSettings,
    imported: &Path,
    aliases_of: impl Fn(&Path) -> Vec<String>,
) -> Vec<SecretId> {
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(file) = &keystore.file {
        files.push(file.clone());
        // iOS-Pfade als Zeichenkette umschreiben (mit „/“ wie gespeichert; `Path::join` setzte unter Windows „\“)
        if let Some(s) = file.to_str() {
            if let Some(rest) = s.strip_prefix("/var/") {
                files.push(PathBuf::from(format!("/private/var/{rest}")));
            }
            if let Some(rest) = s.strip_prefix("/private/var/") {
                files.push(PathBuf::from(format!("/var/{rest}")));
            }
        }
    }
    if let Ok(dir) = std::fs::read_dir(imported) {
        files.extend(dir.flatten().map(|e| e.path()).filter(|p| p.is_file()));
    }
    let mut aliases = vec![String::new()];
    if let Some(alias) = keystore.alias.as_ref().filter(|a| !a.is_empty()) {
        aliases.push(alias.clone());
    }
    let mut ids = Vec::new();
    for file in &files {
        ids.push(SecretId::keystore_store(file));
        ids.push(SecretId::KeystoreStorePassword { keystore: file.clone() });
        for alias in aliases.iter().cloned().chain(aliases_of(file)) {
            ids.push(SecretId::keystore_key(file, &alias));
            ids.push(SecretId::KeystoreKeyPassword { keystore: file.clone(), alias });
        }
    }
    ids.sort_by_key(SecretId::account);
    ids.dedup();
    ids
}

/// Aliase der Schlüssel in `file`, sofern das Passwort der Datei noch im sicheren Speicher liegt (sonst – auch bei
/// fehlender, zu großer oder unlesbarer Datei – keine). Nur für [`former_keystore_secret_ids`]: die Datei wird
/// mit dem gespeicherten Passwort gelesen, nichts geschrieben; Passwort und Inhalt nur im RAM.
fn stored_key_aliases(secrets: &dyn SecretStore, file: &Path) -> Vec<String> {
    if !std::fs::metadata(file).is_ok_and(|m| m.is_file() && m.len() <= crate::runtime::MAX_KEYSTORE_BYTES) {
        return Vec::new();
    }
    let Ok(Some(password)) = secrets.get(&SecretId::keystore_store(file)) else { return Vec::new() };
    let Ok(bytes) = std::fs::read(file).map(zeroize::Zeroizing::new) else { return Vec::new() };
    match pida_backend_keystore::list_aliases(&bytes, &password) {
        Ok(entries) => entries.into_iter().map(|e| e.alias).collect(),
        Err(e) => {
            tracing::debug!(code = e.code(), "Schlüssel eines früheren Keystores nicht lesbar");
            Vec::new()
        }
    }
}

impl CoreTask {
    /// Mobile, beim ersten Start dieses Builds: Keystore-Passwörter eines früheren Mobile-Builds aus dem sicheren
    /// Speicher entfernen – nur Einträge von PIDA (Dienst „PIDA“, Kennungen aus [`former_keystore_secret_ids`]);
    /// das ID-Austria-Passwort und Proxy-Passwörter bleiben. Einmalig: danach steht der Merker
    /// [`SECRETS_REMOVED_MARKER`] im Datenordner. Ohne dauerhaften Speicher bzw. nach einem Fehler beim Löschen
    /// bleibt der Merker aus (nächster Start versucht es erneut). Keine Pfade und Werte im Log (Regel 9).
    pub(crate) fn remove_former_keystore_secrets(&mut self) {
        if self.cfg.keystore_offered {
            return;
        }
        let marker = self.cfg.paths.data_dir.join(SECRETS_REMOVED_MARKER);
        if marker.exists() {
            return;
        }
        if !self.cfg.secrets.is_persistent() {
            tracing::debug!("kein dauerhafter sicherer Speicher – Keystore-Passwörter später entfernen");
            return;
        }
        let imported = self.cfg.paths.data_dir.join(KEYSTORE_DIR);
        let secrets = self.cfg.secrets.as_ref();
        let mut removed = 0usize;
        let mut failed = false;
        let ids =
            former_keystore_secret_ids(&self.cfg.settings.keystore, &imported, |f| stored_key_aliases(secrets, f));
        for id in ids {
            match secrets.get(&id) {
                Ok(None) => continue,
                Ok(Some(_)) => {}
                Err(e) => {
                    tracing::warn!(id = e.message_id(), "sicherer Speicher nicht lesbar – Keystore-Passwort bleibt");
                    failed = true;
                    continue;
                }
            }
            match secrets.delete(&id) {
                Ok(()) => removed += 1,
                Err(e) => {
                    tracing::warn!(id = e.message_id(), "Keystore-Passwort nicht aus dem sicheren Speicher entfernt");
                    failed = true;
                }
            }
        }
        tracing::info!(removed, "Mobile nur mit ID Austria: Keystore-Passwörter eines früheren Builds entfernt");
        if failed {
            return;
        }
        let written = std::fs::create_dir_all(&self.cfg.paths.data_dir)
            .and_then(|()| std::fs::write(&marker, b"ADR 0004 Nachtrag 11\n"));
        if let Err(e) = written {
            tracing::warn!(kind = ?e.kind(), "Merker der entfernten Keystore-Passwörter nicht geschrieben");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_config::{LegacyConfig, MigrationContext, PasswordStorage, migrate};

    #[test]
    fn only_desktop_offers_the_keystore() {
        for p in [Platform::Windows, Platform::MacOs, Platform::Linux] {
            assert!(keystore_offered(p), "{p:?}");
        }
        for p in [Platform::Ios, Platform::Android] {
            assert!(!keystore_offered(p), "{p:?}");
        }
    }

    #[test]
    fn default_signer_becomes_id_austria_and_keeps_the_keystore_section() {
        let mut s = Settings::default();
        s.backend.default = Some(BackendChoice::Keystore);
        s.keystore.enabled = true;
        s.keystore.file = Some("/var/mobile/k/test.p12".into());
        s.keystore.alias = Some("max".into());
        let keystore = s.keystore.clone();
        assert!(map_default_signer(&mut s));
        assert_eq!(s.backend.default, Some(BackendChoice::IdAustria));
        assert_eq!(s.keystore, keystore, "Keystore-Werte nicht löschen, nur ignorieren");
        s.backend.default = Some(BackendChoice::LocalBku);
        assert!(map_default_signer(&mut s));
        assert_eq!(s.backend.default, Some(BackendChoice::IdAustria));
        for unchanged in [None, Some(BackendChoice::IdAustria)] {
            s.backend.default = unchanged;
            assert!(!map_default_signer(&mut s));
            assert_eq!(s.backend.default, unchanged);
        }
    }

    fn migrated(config: &str) -> MigrationResult {
        let legacy = LegacyConfig::parse(config.as_bytes()).unwrap();
        let ctx = MigrationContext { legacy_dir: std::env::temp_dir(), system_language: Lang::De, check_paths: false };
        migrate(&legacy, &ctx)
    }

    #[test]
    fn migration_drops_keystore_file_alias_and_passwords() {
        let mut r = migrated(
            "BKU=KS\nKEYSTORE_ENABLED=true\nKEYSTORE_FILE=/home/test/test.p12\nKEYSTORE_ALIAS=max\n\
             KEYSTORE_TYPE=PKCS12\nKEYSTORE_PASS_STORE_TYPE=disk\nKEYSTORE_STOREPASS=geheim\n\
             KEYSTORE_KEYPASS=geheim\nPROXY_HOST=proxy.example.test\nPROXY_USER=u\nPROXY_PASS=p\n\
             MOBILE_NUMBER=+436641234567\n",
        );
        assert!(r.secrets.iter().any(|(id, _)| matches!(id, SecretId::KeystoreStorePassword { .. })), "{r:?}");
        assert!(r.report.imported.contains(&LegacyKey::KeystoreFile));
        assert!(restrict_migration(&mut r, Lang::De));
        assert_eq!(r.settings.backend.default, Some(BackendChoice::IdAustria));
        assert_eq!(r.settings.keystore, KeystoreSettings::default());
        assert_eq!(r.settings.id_austria.username.as_deref(), Some("+436641234567"), "anderes bleibt");
        assert!(r.secrets.iter().all(|(id, _)| matches!(id, SecretId::ProxyPassword { .. })));
        assert!(!r.secrets.is_empty(), "Proxy-Passwort bleibt");
        assert!(r.report.secrets_found.iter().all(|k| !KEYSTORE_KEYS.contains(k)));
        assert!(r.report.imported.iter().all(|k| !KEYSTORE_KEYS.contains(k)));
        let (_, text) = r.report.skipped.iter().find(|(k, _)| *k == LegacyKey::KeystoreFile).unwrap();
        assert!(text.contains("KEYSTORE_FILE") && text.contains("ID Austria"), "{text}");
        assert!(!format!("{r:?}").contains("geheim"));
        // ohne Keystore in der Altdatei: nichts zu erklären
        let mut plain = migrated("BKU=MOBILE\nSIGNATURE_PROFILE=AMTSSIGNATURBLOCK\n");
        assert!(!restrict_migration(&mut plain, Lang::De));
        assert_eq!(plain.settings.backend.default, Some(BackendChoice::IdAustria));
        // Chipkarte als Standard (gibt es auf Mobilgeräten nicht): ID Austria, erklärt
        let mut card = migrated("BKU=LOCAL\n");
        assert!(restrict_migration(&mut card, Lang::De));
        assert_eq!(card.settings.backend.default, Some(BackendChoice::IdAustria));
    }

    #[test]
    fn keystore_events_are_recognised() {
        let ks = BackendId::Keystore;
        let card = BackendId::LocalBku;
        let yes = [
            UiEvent::ImportKeystore { path: "/x.p12".into(), password: None, remember: false },
            UiEvent::InspectKeystore { password: None, password_storage: Some(PasswordStorage::Keychain) },
            UiEvent::ChooseBackend { backend: ks },
            UiEvent::SignWith { backend: ks },
            UiEvent::SelectSigner { backend: Some(ks), make_default: true },
            // die Chipkarte im Konto-Chip: sonst ausgewählt bzw. als Standard gespeichert
            UiEvent::SelectSigner { backend: Some(card), make_default: true },
            UiEvent::StartBatch {
                backend: ks,
                docs: Vec::new(),
                output_dir: None,
                on_existing: pida_workflow::ExistingPolicy::KeepBoth,
            },
        ];
        for e in &yes {
            assert!(ignored_on_mobile(e).is_some(), "{e:?}");
        }
        let ida = BackendId::AtrustNative;
        for e in [
            UiEvent::ChooseBackend { backend: ida },
            UiEvent::SignWith { backend: ida },
            UiEvent::SelectSigner { backend: None, make_default: false },
            UiEvent::SelectSigner { backend: Some(ida), make_default: true },
            // „nicht verfügbar“ meldet der Ablauf selbst (wie bisher)
            UiEvent::SignWith { backend: card },
            UiEvent::DismissKeystoreImport,
        ] {
            assert!(ignored_on_mobile(&e).is_none(), "{e:?}");
        }
        // verworfen: das Passwort wird dabei überschrieben (kein Absturz, auch ohne Passwort)
        discard_event(UiEvent::ImportKeystore {
            path: "/x.p12".into(),
            password: Some("geheim".into()),
            remember: true,
        });
        discard_event(UiEvent::InspectKeystore { password: Some("geheim".into()), password_storage: None });
        discard_event(UiEvent::SignWith { backend: ks });
    }

    #[test]
    fn secret_ids_cover_settings_file_imported_files_and_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        let imported = tmp.path().join("keystores");
        std::fs::create_dir_all(&imported).unwrap();
        std::fs::write(imported.join("neu.p12"), b"x").unwrap();
        let ks = KeystoreSettings {
            enabled: true,
            file: Some("/var/mobile/Containers/Data/Application/ALT/data/keystores/alt.p12".into()),
            alias: Some("max".into()),
            ..KeystoreSettings::default()
        };
        // Schlüssel der Datei (aus ihr gelesen): auch ein früher gewählter, nicht mehr eingestellter Alias
        let ids = former_keystore_secret_ids(&ks, &imported, |f| {
            if f.ends_with("neu.p12") { vec!["frueher".to_owned()] } else { Vec::new() }
        });
        let accounts: Vec<String> = ids.iter().map(SecretId::account).collect();
        let has = |s: &str| accounts.iter().any(|a| a == s);
        assert!(has("keystore-store:/var/mobile/Containers/Data/Application/ALT/data/keystores/alt.p12"));
        assert!(has("keystore-store:/private/var/mobile/Containers/Data/Application/ALT/data/keystores/alt.p12"));
        assert!(has("keystore-key:/var/mobile/Containers/Data/Application/ALT/data/keystores/alt.p12:max"));
        assert!(has("keystore-key:/var/mobile/Containers/Data/Application/ALT/data/keystores/alt.p12:"));
        let neu = std::fs::canonicalize(imported.join("neu.p12")).unwrap();
        assert!(has(&format!("keystore-store:{}", neu.display())));
        assert!(has(&format!("keystore-key:{}:", neu.display())));
        assert!(has(&format!("keystore-key:{}:frueher", neu.display())));
        assert!(!has("keystore-key:/var/mobile/Containers/Data/Application/ALT/data/keystores/alt.p12:frueher"));
        assert!(accounts.iter().all(|a| a.starts_with("keystore-")), "nie Proxy oder anderes");
        let mut sorted = accounts.clone();
        sorted.dedup();
        assert_eq!(sorted, accounts, "keine Doppelten");
        let none = former_keystore_secret_ids(&KeystoreSettings::default(), &tmp.path().join("fehlt"), |_| {
            vec!["x".to_owned()]
        });
        assert!(none.is_empty());
    }

    /// Aliase nur aus einer lesbaren Datei mit gespeichertem Passwort; sonst keine (kein Fehler).
    #[test]
    fn stored_key_aliases_need_the_stored_password() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("k.p12");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.p12"), &file)
            .unwrap();
        let store = pida_config::MemorySecretStore::default();
        assert!(stored_key_aliases(&store, &file).is_empty(), "ohne Passwort");
        store.set(&SecretId::keystore_store(&file), "falsch").unwrap();
        assert!(stored_key_aliases(&store, &file).is_empty(), "falsches Passwort");
        store.set(&SecretId::keystore_store(&file), "test").unwrap();
        let bytes = std::fs::read(&file).unwrap();
        let expected: Vec<String> =
            pida_backend_keystore::list_aliases(&bytes, "test").unwrap().into_iter().map(|e| e.alias).collect();
        assert!(!expected.is_empty());
        assert_eq!(stored_key_aliases(&store, &file), expected);
        assert!(stored_key_aliases(&store, &tmp.path().join("fehlt.p12")).is_empty());
    }
}
