//! Einstellungen › Signiermittel › Keystore (Nachtrag ADR 0004, Wünsche des Auftraggebers):
//!
//! - **Format am Inhalt** statt Typauswahl: [`SettingsView::keystore_format`](crate::SettingsView) aus den ersten
//!   Bytes der gewählten Datei (`pida_backend_keystore::detect_format`); signiert wird nur mit PKCS#12
//!   (`keystore_ready`). `keystore.declared_type` bleibt nur für die Kompatibilität mit PDF-Over gespeichert.
//! - **Alias-Auswahl:** die Einträge einer PKCS#12-Datei sind meist mit dem Passwort verschlüsselt. Ist ein
//!   Passwort bekannt (Overlay, gemerkt gemäß `keystore.password_storage`), liest der Kern sie automatisch,
//!   sobald die Einstellungen offen sind und die Datei gewählt ist; sonst nach `UiEvent::InspectKeystore`. Ein
//!   richtiges Passwort wird danach wie beim Signieren gemäß Speicherart behalten (keine eigene Speicherregel).
//!   Die Angaben gelten für einen Stand der Datei (Pfad, Größe, Änderungszeit) und werden bei einer anderen
//!   Datei bzw. einem neuen Stand verworfen.
//! - **„Passwort merken?“** bei der ersten Verwendung (Neuinstallation, `keystore.storage_chosen = false`):
//!   die Keystore-Passwortabfrage beim Signieren ([`InteractionView::Secret::ask_storage`]) und
//!   „Schlüssel anzeigen“ bieten unter dem Passwortfeld die Speicherart an; die Antwort wird gespeichert und
//!   gilt schon für dieses Passwort ([`Registry::choose_keystore_storage`]).
//!
//! Passwörter liegen nur als `Zeroizing` im RAM und erscheinen nie in Logs; Namen und Aliasse der Einträge
//! (Personendaten) ebenfalls nicht (Regel 9).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pida_backend_api::SecretKind;
use pida_backend_keystore::{AliasInfo, KeystoreFormat};
use zeroize::Zeroizing;

use crate::backends::Registry;
use crate::runtime::CoreTask;
use crate::view::{InteractionView, KeystoreEntryView, KeystoreFormatView};

/// Meldungs-ID: „Schlüssel anzeigen“ ohne Passwort und ohne gemerktes Passwort.
pub(crate) const MSG_KEYSTORE_PASSWORD_NEEDED: &str = "ng-keystore-password-needed";
/// Meldungs-ID: die gewählte Keystore-Datei fehlt.
const MSG_KEYSTORE_FILE_NOT_FOUND: &str = "ng-keystore-file-not-found";
/// Obergrenze wie beim Signieren (`pida_backend_keystore`): PKCS#12 sind wenige KiB groß.
const MAX_KEYSTORE_FILE: u64 = 16 << 20;

/// Stand einer Datei (Pfad, Größe, Änderungszeit).
#[derive(Clone, PartialEq, Eq)]
struct FileStamp {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    fn of(path: &Path) -> Option<FileStamp> {
        let meta = std::fs::metadata(path).ok().filter(std::fs::Metadata::is_file)?;
        Some(FileStamp { path: path.to_path_buf(), len: meta.len(), modified: meta.modified().ok() })
    }
}

/// Angaben zur gewählten Keystore-Datei für die Einstellungsansicht.
#[derive(Default)]
pub(crate) struct KeystoreInfo {
    /// Stand, für den die Angaben gelten (`None` = keine bzw. fehlende Datei)
    stamp: Option<FileStamp>,
    /// Format am Inhalt
    format: Option<KeystoreFormatView>,
    /// Einträge (nach Alias sortiert), sobald das Passwort bekannt war
    entries: Option<Vec<KeystoreEntryView>>,
    /// Problem der letzten Prüfung (`InspectKeystore`)
    problem: Option<String>,
    /// automatisches Lesen mit einem bekannten Passwort für diesen Stand schon versucht
    auto_read: bool,
}

impl std::fmt::Debug for KeystoreInfo {
    /// ohne Pfad, Aliasse und Namen (Regel 9)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeystoreInfo")
            .field("file", &self.stamp.as_ref().map(|_| "<Pfad>"))
            .field("format", &self.format)
            .field("entries", &self.entries.as_ref().map(Vec::len))
            .field("problem", &self.problem)
            .finish_non_exhaustive()
    }
}

impl KeystoreInfo {
    /// Auf die Datei `file` umstellen: bei anderem Pfad bzw. neuem Stand Format neu erkennen und Einträge
    /// verwerfen.
    fn update(&mut self, file: Option<&Path>) {
        let stamp = file.and_then(FileStamp::of);
        if stamp == self.stamp {
            return;
        }
        let format = stamp.as_ref().and_then(|s| detect_format(&s.path));
        *self = KeystoreInfo { stamp, format, ..KeystoreInfo::default() };
    }

    /// Projektion für `file` (Format, Einträge, Problem); für eine andere als die zuletzt geprüfte Datei
    /// (abgelehnte Arbeitskopie im klassischen Speichern) nur das Format.
    pub(crate) fn view_for(
        &self,
        file: Option<&Path>,
    ) -> (Option<KeystoreFormatView>, Option<Vec<KeystoreEntryView>>, Option<String>) {
        match (file, &self.stamp) {
            (Some(f), Some(s)) if s.path == f => (self.format, self.entries.clone(), self.problem.clone()),
            (Some(f), _) => (detect_format(f), None, None),
            (None, _) => (None, None, None),
        }
    }

    /// Einträge aus einer Prüfung außerhalb (PKCS#12-Import) übernehmen.
    pub(crate) fn adopt(&mut self, file: &Path, entries: Vec<AliasInfo>) {
        self.update(Some(file));
        if self.stamp.is_some() {
            self.entries = Some(entries.iter().map(entry_view).collect());
            self.problem = None;
            self.auto_read = true;
        }
    }
}

/// Format der Datei am Inhalt (erste Bytes, ohne Passwort); `None`, wenn sie fehlt bzw. nicht lesbar ist.
pub(crate) fn detect_format(path: &Path) -> Option<KeystoreFormatView> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut head = Vec::with_capacity(4);
    file.take(4).read_to_end(&mut head).ok()?;
    Some(match pida_backend_keystore::detect_format(&head) {
        KeystoreFormat::Pkcs12 => KeystoreFormatView::Pkcs12,
        KeystoreFormat::Jks => KeystoreFormatView::Jks,
        KeystoreFormat::Jceks => KeystoreFormatView::Jceks,
        KeystoreFormat::Unknown => KeystoreFormatView::Unknown,
    })
}

/// Eintrag → Anzeige (Gültigkeitsende als RFC 3339 in UTC).
fn entry_view(e: &AliasInfo) -> KeystoreEntryView {
    KeystoreEntryView {
        alias: e.alias.clone(),
        subject_cn: e.subject_cn.clone(),
        not_after: e.not_after.map(|t| crate::workspace::rfc3339(t.to_offset(time::UtcOffset::UTC))),
        is_key_entry: e.is_key_entry,
    }
}

/// Einträge der PKCS#12-Datei mit dem Passwort lesen; `Err` = Meldungs-ID (`keystore-wrong-password` …).
fn read_entries(file: &Path, password: &str) -> Result<Vec<KeystoreEntryView>, String> {
    use std::io::Read;
    let f = std::fs::File::open(file).map_err(|_| MSG_KEYSTORE_FILE_NOT_FOUND.to_owned())?;
    let mut bytes = Zeroizing::new(Vec::new());
    f.take(MAX_KEYSTORE_FILE + 1).read_to_end(&mut bytes).map_err(|_| "keystore-io".to_owned())?;
    if bytes.len() as u64 > MAX_KEYSTORE_FILE {
        return Err("keystore-malformed".into());
    }
    let list = pida_backend_keystore::list_aliases(&bytes, password).map_err(|e| e.code().to_owned())?;
    Ok(list.iter().map(entry_view).collect())
}

impl CoreTask {
    /// Nach jeder Nachricht bei offener Einstellungsansicht: Format der gewählten Datei aktuell halten und die
    /// Einträge einmal je Stand automatisch lesen, wenn ein Passwort bekannt ist.
    pub(crate) fn refresh_keystore_info(&mut self) {
        // iOS/Android nur mit ID Austria (ADR 0004 Nachtrag 11): keine Keystore-Angaben
        if self.settings_view.is_none() || !self.cfg.keystore_offered {
            return;
        }
        let file = self.cfg.settings.keystore.file.clone();
        self.keystore_info.update(file.as_deref());
        let Some(file) = file else { return };
        let info = &mut self.keystore_info;
        if info.format != Some(KeystoreFormatView::Pkcs12) || info.entries.is_some() || info.auto_read {
            return;
        }
        info.auto_read = true;
        // nie den Schlüsselbund anfassen (macOS-Systemabfrage beim bloßen Öffnen der Einstellungen)
        let Some(password) = self.registry.known_keystore_password_without_keychain(&self.cfg, &file) else { return };
        match read_entries(&file, &password) {
            Ok(entries) => {
                tracing::debug!(entries = entries.len(), "Keystore-Einträge mit bekanntem Passwort gelesen");
                self.keystore_info.entries = Some(entries);
            }
            // kein Feldfehler: die Ansicht bietet „Schlüssel anzeigen“ an
            Err(code) => tracing::debug!(code = %code, "Keystore-Einträge mit bekanntem Passwort nicht lesbar"),
        }
    }

    /// `InspectKeystore` („Schlüssel anzeigen“): Einträge mit dem Passwort lesen; ein richtiges, eingegebenes
    /// Passwort gemäß Speicherart behalten. `storage` = Antwort auf „Passwort merken?“ (erste Verwendung).
    pub(crate) fn inspect_keystore(
        &mut self,
        password: Option<Zeroizing<String>>,
        storage: Option<pida_config::PasswordStorage>,
    ) {
        if self.settings_view.is_none() {
            tracing::debug!("inspect_keystore ohne geöffnete Einstellungsansicht verworfen");
            return;
        }
        let Some(file) = self.cfg.settings.keystore.file.clone() else {
            tracing::debug!("inspect_keystore ohne gewählte Keystore-Datei verworfen");
            return;
        };
        self.keystore_info.update(Some(&file));
        let problem = match self.keystore_info.format {
            None => Some(MSG_KEYSTORE_FILE_NOT_FOUND),
            Some(KeystoreFormatView::Pkcs12) => None,
            Some(KeystoreFormatView::Jks | KeystoreFormatView::Jceks) => Some("keystore-conversion-needed"),
            Some(KeystoreFormatView::Unknown) => Some("keystore-malformed"),
        };
        if let Some(code) = problem {
            self.keystore_info.entries = None;
            self.keystore_info.problem = Some(code.to_owned());
            return;
        }
        if let Some(storage) = storage {
            self.answer_storage_question(storage);
        }
        let entered = password.is_some();
        let Some(password) = password.or_else(|| self.registry.known_keystore_password(&self.cfg, &file)) else {
            self.keystore_info.entries = None;
            self.keystore_info.problem = Some(MSG_KEYSTORE_PASSWORD_NEEDED.to_owned());
            return;
        };
        match read_entries(&file, &password) {
            Ok(entries) => {
                tracing::info!(entries = entries.len(), "Keystore-Einträge gelesen");
                if entered {
                    self.registry.remember_keystore_password(&self.cfg, &file, &password);
                }
                self.keystore_info.entries = Some(entries);
                self.keystore_info.problem = None;
            }
            Err(code) => {
                tracing::info!(code = %code, "Keystore-Einträge nicht lesbar");
                self.keystore_info.entries = None;
                self.keystore_info.problem = Some(code);
            }
        }
        self.keystore_info.auto_read = true;
    }

    /// Antwort auf „Passwort merken?“ (erste Verwendung): als `keystore.password_storage` speichern, danach
    /// nicht mehr fragen; gilt schon für das gerade eingegebene Passwort. Ohne offene Frage ohne Wirkung.
    pub(crate) fn answer_storage_question(&mut self, storage: pida_config::PasswordStorage) -> bool {
        if !Registry::asks_storage(&self.cfg) {
            tracing::debug!("Speicherart bereits gewählt – Antwort ignoriert");
            return false;
        }
        let storage = if storage == pida_config::PasswordStorage::Keychain && !self.cfg.secrets.is_persistent() {
            tracing::debug!("kein Schlüsselbund – Speicherart „bis Programmende“");
            pida_config::PasswordStorage::Memory
        } else {
            storage
        };
        self.registry.choose_keystore_storage(storage);
        let ks = &mut self.cfg.settings.keystore;
        ks.password_storage = storage;
        ks.storage_chosen = true;
        tracing::info!(storage = ?storage, "Speicherart des Keystore-Passworts gewählt");
        if let Err(e) = self.cfg.save() {
            tracing::warn!(id = e.message_id(), "Einstellungen nicht gespeichert");
            self.notify_code("error-FailedToSaveSettings");
        }
        self.registry.configure(&self.cfg);
        true
    }

    /// Antwort auf „Passwort merken?“ aus der Passwortabfrage beim Signieren: nur für die offene
    /// Keystore-Passwortabfrage `request`, die gefragt hat.
    pub(crate) fn answer_storage_for_prompt(&mut self, request: u64, storage: pida_config::PasswordStorage) {
        let asked = matches!(
            self.interaction_view(),
            InteractionView::Secret { request: r, ask_storage: true, .. } if r == request
        );
        if asked {
            self.answer_storage_question(storage);
        } else {
            tracing::debug!(request, "Speicherart ohne Rückfrage – ignoriert");
        }
    }

    /// Aktuelle Interaktion für die Projektion: die Keystore-Passwortabfrage fragt bei der ersten Verwendung
    /// „Passwort merken?“ (mit Angabe, ob ein Schlüsselbund verfügbar ist).
    pub(crate) fn interaction_view(&self) -> InteractionView {
        let mut view = self.shared.broker.current();
        if let InteractionView::Secret { prompt, ask_storage, keychain_available, .. } = &mut view
            && prompt.kind == SecretKind::KeystorePassword
        {
            *ask_storage = Registry::asks_storage(&self.cfg);
            *keychain_available = self.cfg.secrets.is_persistent();
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn testdata(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(rel)
    }

    #[test]
    fn format_from_content() {
        let tmp = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let p = tmp.path().join(name);
            std::fs::write(&p, bytes).unwrap();
            p
        };
        assert_eq!(detect_format(&testdata("keystores/test-rsa2048.p12")), Some(KeystoreFormatView::Pkcs12));
        assert_eq!(detect_format(&testdata("backend-keystore/java-rsa2048.jks")), Some(KeystoreFormatView::Jks));
        assert_eq!(detect_format(&testdata("backend-keystore/java-rsa2048.jceks")), Some(KeystoreFormatView::Jceks));
        // die Endung zählt nicht: JKS-Inhalt mit Endung .p12, PKCS#12-Inhalt mit Endung .jks
        assert_eq!(detect_format(&write("falsch.p12", &[0xFE, 0xED, 0xFE, 0xED, 0])), Some(KeystoreFormatView::Jks));
        assert_eq!(detect_format(&write("p12.jks", &[0x30, 0x82, 0x01, 0x00])), Some(KeystoreFormatView::Pkcs12));
        assert_eq!(detect_format(&write("j.ks", &[0xCE, 0xCE, 0xCE, 0xCE])), Some(KeystoreFormatView::Jceks));
        assert_eq!(detect_format(&write("text.p12", b"kein Keystore")), Some(KeystoreFormatView::Unknown));
        assert_eq!(detect_format(&write("leer.p12", b"")), Some(KeystoreFormatView::Unknown));
        assert_eq!(detect_format(&tmp.path().join("fehlt.p12")), None);
        assert_eq!(detect_format(tmp.path()), None, "Ordner");
    }

    #[test]
    fn entries_with_password() {
        let two = testdata("backend-keystore/java-two-keys.p12");
        let entries = read_entries(&two, "test").unwrap();
        let aliases: Vec<_> = entries.iter().map(|e| (e.alias.as_str(), e.is_key_entry)).collect();
        assert_eq!(aliases, [("erster", true), ("vertrauensanker", false), ("zweiter", true)]);
        let first = &entries[0];
        assert_eq!(first.subject_cn.as_deref(), Some("PDF-Over NG Test RSA (NUR TEST)"));
        let not_after = first.not_after.as_deref().unwrap();
        assert!(not_after.ends_with('Z') && not_after.len() == 20, "RFC 3339 in UTC: {not_after}");
        assert_eq!(read_entries(&two, "falsch").unwrap_err(), "keystore-wrong-password");
        let jks = testdata("backend-keystore/java-rsa2048.jks");
        assert_eq!(read_entries(&jks, "test").unwrap_err(), "keystore-conversion-needed");
        // Debug ohne Alias und Namen (Regel 9)
        let dbg = format!("{entries:?}");
        assert!(!dbg.contains("erster") && !dbg.contains("NUR TEST"), "{dbg}");
    }

    #[test]
    fn info_follows_file_and_stamp() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("k.p12");
        std::fs::copy(testdata("keystores/test-rsa2048.p12"), &p).unwrap();
        let mut info = KeystoreInfo::default();
        info.update(Some(&p));
        assert_eq!(info.format, Some(KeystoreFormatView::Pkcs12));
        info.entries = Some(Vec::new());
        info.update(Some(&p));
        assert!(info.entries.is_some(), "gleicher Stand: Einträge bleiben");
        std::fs::write(&p, [0xFE, 0xED, 0xFE, 0xED, 0, 0]).unwrap();
        info.update(Some(&p));
        assert_eq!(info.format, Some(KeystoreFormatView::Jks));
        assert!(info.entries.is_none(), "neuer Stand: Einträge verworfen");
        info.update(None);
        assert_eq!(info.view_for(None), (None, None, None));
        assert_eq!(info.view_for(Some(&p)).0, Some(KeystoreFormatView::Jks));
        assert!(!format!("{info:?}").contains("k.p12"));
    }
}
