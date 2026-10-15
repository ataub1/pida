//! Serde-Typen der Bridge (Plan §3.4, TypeScript-Skizze `ViewState`/`UiEvent`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use pida_backend_api::{BackendId, Capabilities, NativeForm, Notice, NoticeAnswer, Platform, Progress, SecretPrompt};
use pida_i18n::Lang;
use pida_workflow::{DocId, DocResult, MainBarStep, PlaceholderAnswer, Step};

/// Hinweis ohne Rückfrage (i18n-ID mit Parametern), z. B. `error-PositioningNotPossible`,
/// `placement.notice.*`, `ng-config-reset-done`. Die UI schließt ihn mit [`UiEvent::DismissNotice`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NoticeView {
    /// Kennung
    pub id: u64,
    /// Meldungs-ID
    pub code: String,
    /// Parameter
    pub params: BTreeMap<String, String>,
}

/// Speichern-Dialog (F-83/F-86): Startordner und Namensvorschlag.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaveDialogView {
    /// Startordner
    pub directory: PathBuf,
    /// vorgeschlagener Dateiname
    pub file_name: String,
}

/// Angebot, die Einstellungen aus `~/.pdf-over` zu übernehmen (Plan §7.2, einmalig).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationOfferView {
    /// gefundene `PDF-Over.config`
    pub legacy_file: PathBuf,
    /// die Altdatei enthält Klartext-Passwörter (Keystore/Proxy): Zustimmung zum Übernehmen in den
    /// Schlüsselbund erfragen (`UiEvent::AcceptMigration::store_secrets`)
    #[serde(default)]
    pub plaintext_secrets: bool,
}

/// Abgeschlossenes Dokument (Warteschlangen-Panel, CLI-Ergebnis).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DoneItemView {
    /// Kennung
    pub id: DocId,
    /// Name
    pub name: String,
    /// Eingabepfad
    pub input: PathBuf,
    /// Ergebnis
    pub result: DocResult,
}

/// Dokument (Anzeige).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocInfo {
    /// Kennung
    pub id: DocId,
    /// Name
    pub name: String,
    /// Seitenzahl
    pub page_count: usize,
    /// sichtbare Seitengrößen in pt
    pub page_sizes: Vec<(f32, f32)>,
    /// bereits signiert
    pub signed: bool,
}

/// Eintrag der Warteschlange.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueueItemView {
    /// Kennung
    pub id: DocId,
    /// Name
    pub name: String,
    /// aktuell
    pub current: bool,
}

/// Leeres Feld (Auswahlliste).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldInfo {
    /// qualifizierter Name
    pub name: String,
    /// Seite
    pub page: Option<usize>,
}

/// QR-Platzhalter.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QrInfo {
    /// Seite
    pub page: usize,
    /// id
    pub id: Option<String>,
}

/// Blockgröße und Vorschau.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockView {
    /// Breite in pt
    pub width: f32,
    /// Höhe in pt
    pub height: f32,
    /// Vorschau-Revision (Cache-Busting für `blockPreviewUrl`)
    pub preview_revision: u64,
}

/// Backend-Option.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendOptionView {
    /// Backend
    pub id: BackendId,
    /// verfügbar
    pub available: bool,
    /// Fähigkeiten
    pub capabilities: Option<Capabilities>,
    /// Hinweise zur Option (i18n-IDs), z. B. `ng-fido2-unavailable-mobile`: ID Austria auf Mobile ohne
    /// FIDO2 – die UI zeigt FIDO2 ausgegraut mit diesem Hinweis (Plan §4.7, §5.2)
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Betriebssystem der Shell.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformOs {
    /// Windows
    Windows,
    /// macOS
    Macos,
    /// Linux
    Linux,
    /// iOS/iPadOS
    Ios,
    /// Android
    Android,
}

/// Plattform der Oberfläche (Plan §5.1): steuert plattformabhängige Bedienelemente.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlatformView {
    /// Betriebssystem
    pub os: PlatformOs,
    /// Mobile Oberfläche (iOS/Android, Plan §5.2): Teilen und Exportieren statt „Im Ordner anzeigen“,
    /// PKCS#12-Import in die App statt Dateipfad, kein FIDO2 (ausgegraut mit Hinweis), keine lokale
    /// BKU, keine Proxy-Einstellungen (System-Proxy), keine Ausgabeordner-Wahl (App-Dokumente)
    pub mobile: bool,
}

impl From<Platform> for PlatformView {
    fn from(p: Platform) -> Self {
        let os = match p {
            Platform::Windows => PlatformOs::Windows,
            Platform::MacOs => PlatformOs::Macos,
            Platform::Linux => PlatformOs::Linux,
            Platform::Ios => PlatformOs::Ios,
            Platform::Android => PlatformOs::Android,
        };
        PlatformView { os, mobile: !p.is_desktop() }
    }
}

impl Default for PlatformView {
    fn default() -> Self {
        PlatformView::from(Platform::current())
    }
}

/// Angebot „Signatur fortsetzen“ (Mobile, Plan §3.5): Beim letzten Lauf wurde die App während eines
/// Signiervorgangs vom System beendet (z. B. beim Wechsel in die App „ID Austria“). Antwort über
/// [`UiEvent::ResumeSignature`] bzw. [`UiEvent::DiscardResume`]; nur im Schritt `open` gesetzt.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResumeOfferView {
    /// Name des Dokuments
    pub name: String,
    /// Signiermittel des unterbrochenen Vorgangs
    pub backend: BackendId,
    /// vor wie vielen Minuten vorbereitet (Anzeige)
    pub age_minutes: u32,
    /// weitere Dokumente des unterbrochenen Batches
    pub pending: u32,
    /// Das Dokument verlässt mit diesem Signiermittel das Gerät (`document_leaves_device`, z. B. ID Austria):
    /// Kurzhinweis „wird an A-Trust übermittelt“ beim Angebot – „Fortsetzen“ startet die Sitzung (ADR 0003)
    #[serde(default)]
    pub document_leaves_device: bool,
}

/// Ergebnis des PKCS#12-Imports in die App-Sandbox (Mobile, Import-Assistent; Plan §5.2) – schließen
/// mit [`UiEvent::DismissKeystoreImport`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeystoreImportView {
    /// Dateiname (ohne Pfad)
    pub file_name: String,
    /// importiert (Einstellungen › Keystore zeigen auf die Kopie in der Sandbox)
    pub ok: bool,
    /// Meldungs-ID: `ng-keystore-imported` bzw. der Fehler (`keystore-wrong-password`,
    /// `ng-keystore-import-format`, …)
    pub code: String,
    /// Passwort im sicheren Speicher abgelegt (Keychain bzw. Android Keystore)
    pub password_stored: bool,
}

/// Interaktion während des Signierens.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionView {
    /// Warten (Fortschritt)
    Waiting {
        /// Fortschritt
        progress: Progress,
    },
    /// Native Maske (Szenario A)
    Form {
        /// Anfrage-ID (Antwort über [`UiEvent::SubmitForm`])
        request: u64,
        /// Maske
        form: NativeForm,
    },
    /// Passwortabfrage
    Secret {
        /// Anfrage-ID
        request: u64,
        /// Abfrage
        prompt: SecretPrompt,
    },
    /// Meldung
    Notice {
        /// Anfrage-ID
        request: u64,
        /// Meldung
        notice: Notice,
    },
}

/// Ergebnis.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputView {
    /// gespeichert unter
    pub path: Option<PathBuf>,
    /// Speichern fehlgeschlagen/abgebrochen (Knopf „Speichern“, F-86)
    pub save_pending: bool,
    /// Rückfrage „Datei existiert, überschreiben?“ (F-84) – Antwort über [`UiEvent::ConfirmOverwrite`]
    #[serde(default)]
    pub ask_overwrite: Option<PathBuf>,
    /// Speichern-Dialog öffnen (F-83/F-86) – Antwort über [`UiEvent::Save`]
    #[serde(default)]
    pub save_dialog: Option<SaveDialogView>,
}

/// Hauptleiste.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MainBarView {
    /// Schritte
    pub steps: Vec<MainBarStep>,
    /// Einstellungen erreichbar
    pub settings_enabled: bool,
}

/// Schritt-spezifische Sicht.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum StepView {
    /// Start
    Starting,
    /// Öffnen
    Open {
        /// Warteschlange
        queue: Vec<QueueItemView>,
    },
    /// Dokument wird geladen und analysiert (Platzhaltersuche)
    Loading {
        /// Warteschlange
        queue: Vec<QueueItemView>,
    },
    /// Warnung der Eingangsanalyse mit Abbruchmöglichkeit (Plan §6.1: `/Perms /UR3`, Hybrid-XFA, Seed
    /// Values) – Antwort über [`UiEvent::AnswerWarning`]
    Warning {
        /// Dokument
        doc: DocInfo,
        /// Meldungs-IDs der Warnungen (`pades::Finding::code`)
        codes: Vec<String>,
    },
    /// Platzhalter-Frage/Feldauswahl
    Placeholder {
        /// Dokument
        doc: DocInfo,
        /// leere Felder
        fields: Vec<FieldInfo>,
        /// QR-Platzhalter
        qr: Option<QrInfo>,
    },
    /// Positionierung
    Position {
        /// Dokument
        doc: DocInfo,
        /// Seite
        page: usize,
        /// Block
        block: BlockView,
        /// Position (PAGE_ABS, linke obere Ecke)
        #[cfg_attr(feature = "ts", ts(as = "crate::ts::Placement"))]
        placement: pida_placement::Placement,
        /// virtuelle neue Seite
        new_page: bool,
        /// Parameter des Live-Region-Texts `ng-ui-position-announce`: `page`, `pages`, `left` (mm der
        /// linken Kante von links), `top` (mm der Oberkante von unten – wie das Zahlenfeld Y), ganzzahlig
        announce: BTreeMap<String, String>,
    },
    /// Backend-Auswahl; beim Signiermittel mit `capabilities.document_leaves_device` zeigt die UI den
    /// nicht blockierenden Kurzhinweis (ADR 0003)
    Backend {
        /// Optionen
        options: Vec<BackendOptionView>,
    },
    /// Signieren
    Signing {
        /// Interaktion
        interaction: InteractionView,
        /// Warteschlange
        queue: Vec<QueueItemView>,
        /// Das Dokument verlässt mit dem laufenden Signiermittel das Gerät (`document_leaves_device`, z. B.
        /// ID Austria): Kurzhinweis „wird an A-Trust übermittelt“ im Signierbereich (ADR 0003)
        #[serde(default)]
        document_leaves_device: bool,
    },
    /// Ergebnis
    Output {
        /// Ergebnis
        result: OutputView,
        /// nächstes Dokument
        next: Option<DocInfo>,
    },
    /// Fehler
    Error {
        /// Fehlercode (i18n-ID)
        code: String,
        /// Parameter
        params: BTreeMap<String, String>,
        /// Wiederholen möglich
        retry: bool,
    },
    /// Ende
    Finished {
        /// Exit-Code
        exit_code: u8,
    },
}

/// Einstellungen (Overlay über jedem Schritt).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SettingsView {
    /// Einstellungen (Schema `pida-config`)
    #[cfg_attr(feature = "ts", ts(as = "crate::ts::Settings"))]
    pub settings: pida_config::Settings,
    /// Validierungsprobleme je Feld: Schlüssel = TOML-Punktpfad des Felds (`proxy.host`,
    /// `output.folder`, `keystore.file` …), Wert = i18n-ID
    pub problems: BTreeMap<String, String>,
    /// Schlüsselbund verfügbar
    pub keychain_available: bool,
    /// FIDO2 verfügbar
    pub fido2_available: bool,
    /// Einstellungen werden nicht gespeichert (Altdatei per `-c`, unlesbare Datei)
    #[serde(default)]
    pub read_only: bool,
    /// Revision der Blockvorschau (Cache-Busting für `blockPreviewUrl` im Einstellungsdialog);
    /// der Kern baut die Vorschau beim Öffnen auch ohne Dokument (A4, Beispielzertifikat) wie PDF-Over
    #[serde(default)]
    pub preview_revision: u64,
}

/// Gesamtzustand für die UI.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewState {
    /// Schritt
    pub step: StepView,
    /// Hauptleiste
    pub main_bar: MainBarView,
    /// Einstellungen, falls geöffnet
    pub settings: Option<SettingsView>,
    /// UI-Sprache
    pub language: Lang,
    /// Hinweise ohne Rückfrage (älteste zuerst)
    #[serde(default)]
    pub notices: Vec<NoticeView>,
    /// Import-Angebot aus `~/.pdf-over`
    #[serde(default)]
    pub migration: Option<MigrationOfferView>,
    /// abgeschlossene Dokumente dieser Sitzung
    #[serde(default)]
    pub done: Vec<DoneItemView>,
    /// Plattform (Desktop/Mobile-Bedienelemente)
    #[serde(default)]
    pub platform: PlatformView,
    /// Angebot „Signatur fortsetzen“ (nur Mobile, nur im Schritt `open`)
    #[serde(default)]
    pub resume: Option<ResumeOfferView>,
    /// Ergebnis des PKCS#12-Imports (Import-Assistent)
    #[serde(default)]
    pub keystore_import: Option<KeystoreImportView>,
    /// Eigener Datenschutztext der Organisation (Richtlinie `privacy_notice_text`) für Einstellungen ›
    /// Datenschutz und die „Details“ des Kurzhinweises; `None` = Text aus PDF-Over
    /// (`config-DataProtectionStatement`, ADR 0003)
    #[serde(default)]
    pub privacy_text: Option<String>,
}

/// Antwort auf eine Maske (serde-Variante von `FormAnswer`; Passwort wird sofort in `Zeroizing`
/// überführt und nie geloggt).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormAnswerView {
    /// Anmeldung
    Login {
        /// Benutzername
        username: String,
        /// Passwort
        password: String,
        /// Merken
        remember: bool,
    },
    /// TAN
    Tan {
        /// TAN
        tan: String,
    },
    /// SMS
    SwitchToSms,
    /// FIDO2
    SwitchToFido2,
    /// FIDO2 starten
    StartFido2,
    /// weiter
    Continue,
}

impl std::fmt::Debug for FormAnswerView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormAnswerView::Login { username, remember, .. } => f
                .debug_struct("Login")
                .field("username", username)
                .field("password", &"***")
                .field("remember", remember)
                .finish(),
            FormAnswerView::Tan { .. } => f.write_str("Tan(***)"),
            FormAnswerView::SwitchToSms => f.write_str("SwitchToSms"),
            FormAnswerView::SwitchToFido2 => f.write_str("SwitchToFido2"),
            FormAnswerView::StartFido2 => f.write_str("StartFido2"),
            FormAnswerView::Continue => f.write_str("Continue"),
        }
    }
}

/// UI-Ereignis.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    /// Dateien öffnen (Dialog, Drop, „Öffnen mit“, Share)
    OpenFiles {
        /// Pfade
        paths: Vec<PathBuf>,
    },
    /// Antwort auf die Warnung der Analyse ([`StepView::Warning`]): trotzdem signieren bzw. Dokument
    /// verwerfen
    AnswerWarning {
        /// `true` = fortfahren
        proceed: bool,
    },
    /// Platzhalter-Antwort
    PlaceholderAnswer {
        /// Antwort
        answer: PlaceholderAnswer,
        /// gewähltes Feld
        field_index: Option<usize>,
    },
    /// Block verschieben (PAGE_ABS)
    MoveBlock {
        /// Seite
        page: usize,
        /// x
        x: f32,
        /// y
        y: f32,
    },
    /// Tastatur: Schritte (15/1/50 pt)
    NudgeBlock {
        /// dx in Schritten
        dx: i32,
        /// dy in Schritten
        dy: i32,
        /// fein (Shift)
        fine: bool,
        /// grob (Strg/⌘)
        coarse: bool,
    },
    /// Seite wechseln
    ChangePage {
        /// Seite
        page: usize,
    },
    /// „Neue Seite“
    ToggleNewPage,
    /// „Automatisch platzieren“
    AutoPlace,
    /// Position bestätigen
    ConfirmPosition,
    /// „Unsichtbar signieren“ im Positionierungsschritt: dieses Dokument ohne sichtbaren Block
    /// (`PositionChoice::Invisible`, PIDA-Ergänzung; das Profil bleibt unverändert)
    ChooseInvisible,
    /// Backend wählen
    ChooseBackend {
        /// Backend
        backend: BackendId,
    },
    /// **Veraltet, ohne Wirkung** (ADR 0003): Antwort auf den früheren blockierenden Datenschutzhinweis.
    /// Wird nur noch gelesen und ignoriert, damit ältere Aufrufer nicht an `ng-ui-event-invalid`
    /// scheitern; fehlt in den TypeScript-Typen.
    #[cfg_attr(feature = "ts", ts(skip))]
    AcknowledgePrivacy {
        /// ohne Bedeutung
        #[serde(default)]
        accepted: bool,
    },
    /// Maskenantwort
    SubmitForm {
        /// Anfrage-ID
        request: u64,
        /// Antwort
        answer: FormAnswerView,
    },
    /// Passwort
    SubmitSecret {
        /// Anfrage-ID
        request: u64,
        /// Passwort
        secret: String,
    },
    /// Meldung beantwortet
    AnswerNotice {
        /// Anfrage-ID
        request: u64,
        /// Antwort
        answer: NoticeAnswer,
    },
    /// „Signaturdaten anzeigen“ (F-68): der Kern öffnet `FormContext::signature_data_url` der offenen
    /// Maske im Systembrowser (nur `https://`, A-Trust-Host); die Maske bleibt offen.
    OpenSignatureData {
        /// Anfrage-ID der offenen Maske
        request: u64,
    },
    /// Abbrechen (aktuelle Interaktion bzw. Schritt)
    Cancel,
    /// Wiederholen
    Retry,
    /// Speichern (F-86) bzw. Speichern-Dialog beantwortet
    Save {
        /// Ziel (`None` = Dialog abgebrochen)
        path: Option<PathBuf>,
    },
    /// Überschreiben bestätigt
    ConfirmOverwrite {
        /// ja
        overwrite: bool,
    },
    /// Ergebnis öffnen
    OpenResult,
    /// Ergebnis teilen (Mobile: Share-Sheet bzw. `ACTION_SEND`)
    ShareResult,
    /// Ergebnis exportieren („Sichern unter“: Dokument-Picker bzw. SAF `CREATE_DOCUMENT`)
    ExportResult,
    /// Hauptleiste
    Navigate {
        /// Schritt
        step: Step,
    },
    /// Einstellungen öffnen
    OpenSettings,
    /// Einstellungen speichern
    SaveSettings {
        /// neue Einstellungen
        #[cfg_attr(feature = "ts", ts(as = "crate::ts::Settings"))]
        settings: Box<pida_config::Settings>,
    },
    /// Einstellungen schließen (verwerfen)
    CloseSettings,
    /// „Einstellungen zurücksetzen“
    ResetSettings,
    /// Lebenszyklus: Hintergrund (kein Abbruch – z. B. Wechsel in die App „ID Austria“)
    Backgrounded,
    /// Lebenszyklus: Vordergrund (der Kern setzt einen wartenden Long-Poll sofort fort)
    Foregrounded,
    /// „Signatur fortsetzen“ annehmen ([`ViewState::resume`])
    ResumeSignature,
    /// „Signatur fortsetzen“ verwerfen (persistierter Zustand wird gelöscht)
    DiscardResume,
    /// PKCS#12-Datei in die App-Sandbox importieren (Mobile, Import-Assistent): Datei kopieren,
    /// Passwort prüfen (falls angegeben), Einstellungen › Keystore setzen; mit `remember` und
    /// verfügbarem sicheren Speicher wird das Passwort dort abgelegt. Ergebnis in
    /// [`ViewState::keystore_import`].
    ImportKeystore {
        /// Datei (von der Shell bereits in die Sandbox kopiert, `import_file`)
        path: PathBuf,
        /// Passwort (nur RAM, sofort in `Zeroizing`; nie geloggt)
        password: Option<String>,
        /// Passwort im sicheren Speicher merken
        remember: bool,
    },
    /// Ergebnis des PKCS#12-Imports schließen
    DismissKeystoreImport,
    /// Beenden
    Quit,
    /// Hinweis schließen
    DismissNotice {
        /// Kennung ([`NoticeView::id`])
        id: u64,
    },
    /// Einstellungen aus `~/.pdf-over` übernehmen (Plan §7.2)
    AcceptMigration {
        /// Klartext-Passwörter in den Schlüsselbund übernehmen (Zustimmung)
        store_secrets: bool,
    },
    /// Import ablehnen (es wird nicht wieder gefragt)
    DeclineMigration,
}

impl std::fmt::Debug for UiEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiEvent::SubmitSecret { request, .. } => {
                f.debug_struct("SubmitSecret").field("request", request).field("secret", &"***").finish()
            }
            UiEvent::SubmitForm { request, answer } => {
                f.debug_struct("SubmitForm").field("request", request).field("answer", answer).finish()
            }
            UiEvent::ImportKeystore { password, remember, .. } => f
                .debug_struct("ImportKeystore")
                .field("path", &"<Pfad>")
                .field("password", &password.as_ref().map(|_| "***"))
                .field("remember", remember)
                .finish(),
            other => {
                // Alle übrigen Varianten enthalten keine Secrets.
                let s = serde_json::to_string(other).unwrap_or_else(|_| "UiEvent".into());
                f.write_str(&s)
            }
        }
    }
}
