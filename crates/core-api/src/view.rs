//! Serde-Typen der Bridge (Plan §3.4, TypeScript-Skizze `ViewState`/`UiEvent`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use pida_backend_api::{
    BackendId, Capabilities, NativeForm, Notice, NoticeAnswer, Platform, Progress, Qualification, SecretPrompt,
};
use pida_i18n::Lang;
use pida_workflow::{DocId, DocResult, ExistingPolicy, MainBarStep, PlaceholderAnswer, PlacementOrigin, Step};

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
    /// Was übernommen würde (Ersteinrichtung Schritt 1, ADR 0004 §3.12): i18n-IDs `ng-migration-item-*`
    /// mit Detail; ab K4 per Probelauf von `pida_config::migrate` (bis dahin leer)
    #[serde(default)]
    pub items: Vec<MigrationItemView>,
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
    /// Zusammenfassung für „Fertig“, sobald gespeichert (ADR 0004 §3.7; ab K1, bis dahin `None`)
    #[serde(default)]
    pub summary: Option<SignedSummaryView>,
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
        /// Fortschritt für Stepper und Stapel (ADR 0004 §3.6; ab K1, bis dahin Standardwerte)
        #[serde(default)]
        progress: SigningProgressView,
    },
    /// Ergebnis
    Output {
        /// Ergebnis
        result: OutputView,
        /// nächstes Dokument
        next: Option<DocInfo>,
        /// Zahl der wartenden Dokumente nach diesem (ADR 0004 E1 „Fertig“: „Nächstes Dokument“ mit Anzahl,
        /// solange `batch_overview` aus ist; Nachtrag 1 im ADR)
        #[serde(default)]
        pending: usize,
    },
    /// Fehler
    Error {
        /// Fehlercode (i18n-ID)
        code: String,
        /// Parameter
        params: BTreeMap<String, String>,
        /// Wiederholen möglich
        retry: bool,
        /// Herkunft, Zeitpunkt, Dokument (ADR 0004 §3.6; ab K1, bis dahin Standardwerte)
        #[serde(default)]
        context: ErrorContextView,
    },
    /// Ende
    Finished {
        /// Exit-Code
        exit_code: u8,
    },
    /// Stapelübersicht (ADR 0004 §3.8, nur mit `batch_overview`); Inhalt in [`ViewState::batch`]. Der Kern
    /// erzeugt den Schritt ab K2.
    Batch,
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
    /// Revision des zuletzt übernommenen [`UiEvent::ApplySettings`] (ADR 0004 §3.11; ab K3, bis dahin 0)
    #[serde(default)]
    pub applied_revision: u64,
    /// Vom Kern berechnete Beispiele (ADR 0004 §3.11): Schlüssel `output.postfix` → Ausgabename für die
    /// Beispieldatei `Vertrag.pdf` (F-81); ab K3, bis dahin leer
    #[serde(default)]
    pub examples: BTreeMap<String, String>,
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
    /// (`ng-ui-privacy-statement`, PIDA-Fassung des PDF-Over-Texts `config-DataProtectionStatement`, ADR 0003)
    #[serde(default)]
    pub privacy_text: Option<String>,
    /// Aktuelles Dokument im Arbeitsbereich (Warnung, Positionieren, Signieren, Fertig; ADR 0004 §3.1,
    /// §3.3); sonst `None`. Ab K1, bis dahin `None`.
    #[serde(default)]
    pub document: Option<DocumentView>,
    /// Signiermittel für Konto-Chip, Menü und Stapel (ADR 0004 §3.4); ab K1 nach dem Start immer befüllt,
    /// bis dahin leer
    #[serde(default)]
    pub signers: SignersView,
    /// Stapel (nur mit `batch_overview`, ADR 0004 §3.8); ab K2, bis dahin `None`
    #[serde(default)]
    pub batch: Option<BatchView>,
    /// Prüfmodus – liegt über dem Workflow-Zustand, ändert ihn nicht (ADR 0004 §3.9); ab K4
    #[serde(default)]
    pub verify: Option<VerifyView>,
    /// „Zuletzt signiert“; `None` = Funktion abgeschaltet bzw. (bis K3) noch nicht umgesetzt (ADR 0004 §3.10)
    #[serde(default)]
    pub recent: Option<Vec<RecentItemView>>,
    /// Ersteinrichtung, nur beim ersten Start (ADR 0004 §3.12); ab K4
    #[serde(default)]
    pub first_run: Option<FirstRunView>,
    /// Freigaben für Knöpfe, Befehlspalette und Tastenkürzel (ADR 0004 §3.13), aus dem Zustand berechnet
    #[serde(default)]
    pub actions: ActionsView,
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

    // --- ADR 0004: neue Oberfläche „Arbeitsbereich“ (Vertrag E0). Der Kern ignoriert diese Ereignisse,
    // --- bis sie in K1–K4 umgesetzt sind (Debug-Log, kein Zustandswechsel).
    /// Schalter „Unsichtbar signieren“ (nur dieses Dokument, Profil unverändert; §3.3)
    SetInvisible {
        /// unsichtbar
        invisible: bool,
    },
    /// „Ins Feld setzen“ / QR-Platzhalter / „Frei platzieren“ (= automatische Platzierung auf freier
    /// Fläche; §3.3)
    UseTarget {
        /// Ziel
        target: PlacementTarget,
    },
    /// Sichtbare Seiten; der Kern berechnet deren Layout (Cache) – höchstens 8 je Ereignis, idempotent
    /// (§3.3)
    InspectPages {
        /// Seiten (0-basiert)
        pages: Vec<usize>,
    },
    /// Konto-Chip: Signiermittel für diese Sitzung wählen; `make_default` speichert es als Standard
    /// (`backend: None` + `make_default` = „Jedes Mal fragen“). Startet **nichts** (A1, §3.4).
    SelectSigner {
        /// Signiermittel (`None` = keine Auswahl)
        backend: Option<BackendId>,
        /// als Standard speichern
        make_default: bool,
    },
    /// „Signieren“ im Arbeitsbereich mit ausdrücklichem Signiermittel (ersetzt `confirm_position` +
    /// `choose_backend`; im Zustand der Signiermittel-Auswahl gleichbedeutend mit `choose_backend`; §3.5)
    SignWith {
        /// Signiermittel
        backend: BackendId,
    },
    /// Zur Stapelübersicht (aus einem Einzeldokument mit weiteren Dokumenten in der Warteschlange; §3.8)
    ShowBatch,
    /// Dokument aus der Stapelübersicht im Arbeitsbereich öffnen (§3.8)
    OpenBatchItem {
        /// Dokument
        doc: DocId,
    },
    /// Zurück zur Stapelübersicht; die Platzierung wird als Plan des Dokuments gemerkt (§3.8)
    BackToBatch,
    /// Dokument aus dem Stapel entfernen (§3.8)
    RemoveFromBatch {
        /// Dokument
        doc: DocId,
    },
    /// „Alle signieren (n) mit X“: eine Sitzung, eine Anmeldung, je Dokument eine Bestätigung (§3.8)
    StartBatch {
        /// Signiermittel
        backend: BackendId,
        /// genau diese Dokumente (später hinzugefügte gehören nicht zum Lauf)
        docs: Vec<DocId>,
        /// nur bei `BatchOutputView::needs_folder` (Ordnerwahl einmal vor dem Start)
        output_dir: Option<PathBuf>,
        /// Umgang mit vorhandenen Zieldateien
        on_existing: ExistingPolicy,
    },
    /// Aktuelles Dokument des Stapellaufs überspringen (Sitzung und Zugangsdaten bleiben; §3.8)
    SkipBatchDocument,
    /// Stapel abbrechen (Sitzung abbrechen, F-72; `cancel` im Lauf bedeutet dasselbe; §3.8)
    AbortBatch,
    /// Datei im Prüfmodus prüfen (Dialog/Drop; §3.9)
    OpenVerify {
        /// Datei
        path: PathBuf,
    },
    /// Eintrag aus „Zuletzt signiert“ prüfen (§3.9)
    VerifyRecent {
        /// Kennung ([`RecentItemView::id`])
        id: u64,
    },
    /// Prüfmodus schließen (§3.9)
    CloseVerify,
    /// „Dieses Dokument signieren“ (Gegenzeichnen): Prüfmodus schließen, Datei öffnen wie `open_files`
    /// (§3.9)
    SignVerified,
    /// Eintrag aus „Zuletzt signiert“ entfernen (§3.10)
    RemoveRecent {
        /// Kennung
        id: u64,
    },
    /// „Zuletzt signiert“ leeren (§3.10)
    ClearRecent,
    /// Einstellungen sofort übernehmen (§3.11): **nur die geänderten Felder** als TOML-Punktpfad →
    /// serde-JSON des Felds; Felder mit Problem behalten ihren Wert (`SettingsView::problems`), alle
    /// anderen werden übernommen und gespeichert. Nur bei geöffneter Einstellungsansicht.
    ApplySettings {
        /// geänderte Felder (Pfad → Wert)
        #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
        changes: BTreeMap<String, serde_json::Value>,
        /// Revision der UI (zurück in `SettingsView::applied_revision`)
        revision: u64,
    },
    /// Oberflächensprache aus Kopfzeile bzw. Befehlspalette (ohne Einstellungsansicht; §3.11)
    SetUiLanguage {
        /// Sprache (`None` = System)
        language: Option<Lang>,
    },
    /// Ersteinrichtung Schritte 2/3: sofort übernehmen und speichern (Vorschau neu; §3.12)
    UpdateFirstRun {
        /// Standard-Signiermittel (`None` = „Jedes Mal fragen“)
        #[cfg_attr(feature = "ts", ts(as = "Option<crate::ts::BackendChoice>"))]
        default_backend: Option<pida_config::BackendChoice>,
        /// Profil des Signaturblocks
        #[cfg_attr(feature = "ts", ts(as = "crate::ts::SignatureProfile"))]
        profile: pida_config::SignatureProfile,
        /// Sprache des Signaturblocks (`None` = System)
        block_language: Option<Lang>,
    },
    /// Ersteinrichtung „Fertig“ bzw. „Später einrichten“ (§3.12)
    FinishFirstRun,
    /// Einstellungen › Erweitert: Altkonfiguration erneut suchen und anbieten (auch nach
    /// `decline_migration`; §3.12)
    OfferMigration,
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
            // ADR 0004: ohne Pfade und Einstellungswerte (Benutzername, Ordner; Regel 9)
            UiEvent::OpenVerify { .. } => f.debug_struct("OpenVerify").field("path", &"<Pfad>").finish(),
            UiEvent::StartBatch { backend, docs, output_dir, on_existing } => f
                .debug_struct("StartBatch")
                .field("backend", backend)
                .field("docs", docs)
                .field("output_dir", &output_dir.as_ref().map(|_| "<Pfad>"))
                .field("on_existing", on_existing)
                .finish(),
            UiEvent::ApplySettings { changes, revision } => f
                .debug_struct("ApplySettings")
                .field("changes", &changes.keys().collect::<Vec<_>>())
                .field("revision", revision)
                .finish(),
            other => {
                // Alle übrigen Varianten enthalten keine Secrets.
                let s = serde_json::to_string(other).unwrap_or_else(|_| "UiEvent".into());
                f.write_str(&s)
            }
        }
    }
}

// ================================================================================================
// ADR 0004 – neue Oberfläche „Arbeitsbereich“: Bridge-Typen (Vertrag E0, Paket K0)
//
// Alle Typen entstehen hier vollständig mit ihren Varianten; der Kern liefert sie bis zur Umsetzung in
// K1–K4 mit Standardwerten (`None`/leer). Serde nach §3.0 Regel 8: Structs mit allen Feldern
// (`Option` → `null`), Einheits-Enums als `snake_case`-String, Enums mit Daten intern getaggt (`kind`,
// Status-Enums `state`), Zeiten als RFC-3339-String mit Offset. Koordinaten in pt, `PAGE_ABS` (Ursprung
// links unten, y nach oben), Seiten 0-basiert. Typen mit Personendaten (Namen, Dateinamen, Pfade,
// Kontoanzeige) haben ein redigierendes `Debug` (ADR 0001 Regel 9).
// ================================================================================================

/// Platzhalter für Personendaten in `Debug`.
struct Redacted;

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redigiert>")
    }
}

/// `Debug` mit allen Feldern; die unter `redact` genannten erscheinen als `<redigiert>`. Die vollständige
/// Destrukturierung bricht den Build, wenn ein neues Feld in keiner der beiden Listen steht.
macro_rules! redacted_debug {
    ($t:ident { $($shown:ident),* $(,)? } redact { $($hidden:ident),* $(,)? }) => {
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let $t { $($shown,)* $($hidden: _,)* } = self;
                f.debug_struct(stringify!($t))
                    $(.field(stringify!($shown), $shown))*
                    $(.field(stringify!($hidden), &Redacted))*
                    .finish()
            }
        }
    };
}

/// Rechteck in pt (`PAGE_ABS`: Ursprung links unten, y nach oben; ADR 0004 §3.0 Regel 3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RectPt {
    /// linke Kante
    pub left: f32,
    /// Unterkante
    pub bottom: f32,
    /// rechte Kante
    pub right: f32,
    /// Oberkante
    pub top: f32,
}

/// Kurzverweis auf ein Dokument (§3.1).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocRefView {
    /// Kennung
    pub id: DocId,
    /// Name
    pub name: String,
}

redacted_debug!(DocRefView { id } redact { name });

// --- §3.3 Dokument im Arbeitsbereich ------------------------------------------------------------

/// Phase des Dokuments im Arbeitsbereich (Tabelle §3.1).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentPhase {
    /// Analysewarnung (Seiten sichtbar, Inspektor gesperrt)
    Warning,
    /// Platzierung änderbar
    Editing,
    /// Signieren (Karte über dem Dokument)
    Signing,
    /// Fertig (Vorschau der signierten Seite)
    Done,
}

/// Ziel der Platzierung (§3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlacementTarget {
    /// frei platziert (bzw. „Frei platzieren“ = automatische Platzierung auf freier Fläche)
    Free,
    /// leeres Signaturfeld
    Field {
        /// Index in [`DocumentView::fields`]
        index: u32,
    },
    /// QR-Platzhalter
    Qr,
}

/// Leeres, angebotenes Signaturfeld („Hier signieren“; §3.3, §8 F2: nur angeboten, nie automatisch).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldTargetView {
    /// Index (für [`PlacementTarget::Field`])
    pub index: u32,
    /// qualifizierter Feldname
    pub name: String,
    /// Seite des Widgets
    pub page: Option<usize>,
    /// Widget-Rechteck (`PAGE_ABS`)
    pub rect: Option<RectPt>,
    /// wählbar (`false` bei gesperrten Feldern: `/Lock`, FieldMDP)
    pub selectable: bool,
}

/// QR-Platzhalter (nur mit `placeholder.search` und `kind = marker`; §3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QrTargetView {
    /// Seite
    pub page: usize,
    /// Rechteck des Platzhalters (`PAGE_ABS`)
    pub rect: RectPt,
    /// id des Platzhalters
    pub id: Option<String>,
}

/// Status einer vorhandenen Signatur (aus `pades::PriorStatus`; §3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorStatusView {
    /// unverändert und kryptografisch intakt
    Valid,
    /// verändert bzw. ungültig
    Invalid,
    /// nicht prüfbar
    NotVerifiable,
}

/// Vorhandene Signatur im Inspektor (§3.3; „Rolle“ ist nicht ermittelbar und entfällt).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExistingSignatureView {
    /// Feldname
    pub field_name: String,
    /// CN des Signerzertifikats
    pub signer: Option<String>,
    /// RFC 3339 (`signing-time` des CMS, kein geprüfter Zeitstempel)
    pub signing_time: Option<String>,
    /// Status
    pub status: PriorStatusView,
    /// Seite des Signaturbilds
    pub page: Option<usize>,
    /// Rechteck des Signaturbilds (`None` = unsichtbar)
    pub rect: Option<RectPt>,
}

redacted_debug!(ExistingSignatureView { field_name, signing_time, status, page, rect } redact { signer });

/// Einschränkungen des Dokuments mit Begründung (§3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocConstraintsView {
    /// „Auf neuer Seite“ erlaubt (`false` bei bereits signierten Dokumenten)
    pub new_page_allowed: bool,
    /// sichtbarer Block erlaubt (`false` beim unsichtbaren Profil)
    pub visible_allowed: bool,
    /// neues sichtbares Feld erlaubt (`false` bei DocMDP-Zertifizierung: nur vorhandene Felder/unsichtbar)
    pub new_visible_field_allowed: bool,
    /// Begründungen (i18n-IDs): `ng-constraint-signed`, `ng-constraint-certified`,
    /// `ng-constraint-invisible-profile`
    pub reasons: Vec<String>,
}

impl Default for DocConstraintsView {
    /// Keine Einschränkung.
    fn default() -> Self {
        DocConstraintsView {
            new_page_allowed: true,
            visible_allowed: true,
            new_visible_field_allowed: true,
            reasons: Vec::new(),
        }
    }
}

/// Achse einer Einrastlinie (§3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideAxis {
    /// senkrechte Linie bei `x = at`
    X,
    /// waagrechte Linie bei `y = at`
    Y,
}

/// Art einer Einrastlinie (§3.3): `X`-Linien gelten für die linke (`ContentLeft`, `MarginLeft`) bzw.
/// rechte Blockkante (`ContentRight`, `MarginRight`) bzw. die Blockmitte (`CenterX`); `Y`-Linien für die
/// Ober- (`ContentTop`, `BelowContent`, `MarginTop`) bzw. Unterkante (`ContentBottom`, `MarginBottom`)
/// bzw. Mitte (`CenterY`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideKind {
    /// linke Kante des Inhaltsrahmens
    ContentLeft,
    /// rechte Kante des Inhaltsrahmens
    ContentRight,
    /// Oberkante des Inhaltsrahmens
    ContentTop,
    /// Unterkante des Inhaltsrahmens
    ContentBottom,
    /// waagrechte Seitenmitte
    CenterX,
    /// senkrechte Seitenmitte
    CenterY,
    /// Oberkante „unter dem Text“ (18 pt unter `content.bottom`)
    BelowContent,
    /// linker Rand (nur ohne Inhalt, 36 pt)
    MarginLeft,
    /// rechter Rand (nur ohne Inhalt, 36 pt)
    MarginRight,
    /// oberer Rand (nur ohne Inhalt, 36 pt)
    MarginTop,
    /// unterer Rand (nur ohne Inhalt, 36 pt)
    MarginBottom,
}

/// Einrastlinie (§3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuideView {
    /// Achse
    pub axis: GuideAxis,
    /// Lage in pt
    pub at: f32,
    /// Art
    pub kind: GuideKind,
}

/// Layout einer Seite (§3.3): Inhaltsrahmen, freie Flächen, Einrastlinien.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageLayoutView {
    /// Seite
    pub page: usize,
    /// Begrenzungsrahmen des Inhalts (`None` = leere Seite)
    pub content: Option<RectPt>,
    /// freie Rechtecke, in die der Block in aktueller Größe passt (6 pt Abstand zum Inhalt), absteigend
    /// nach Eignung, höchstens 16
    pub free: Vec<RectPt>,
    /// Einrastlinien
    pub guides: Vec<GuideView>,
}

/// Ergebnis der Live-Prüfung der Platzierung (§3.3).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PlacementCheckView {
    /// Revision der Platzierung, auf die sich die Prüfung bezieht ([`DocumentView::revision`])
    pub for_revision: u64,
    /// überdeckt Inhalt; `None` = unbekannt (kein Renderer, Raster noch nicht berechnet, neue Seite)
    pub overlaps_content: Option<bool>,
    /// überdeckt ein Formularfeld
    pub overlaps_widget: bool,
    /// vom Kern an die Seite geklemmt
    pub clamped: bool,
}

/// Aktuelles Dokument im Arbeitsbereich (§3.3) – einzige Inhaltsquelle der neuen Oberfläche;
/// `StepView::Position` bleibt Phasenmarker.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentView {
    /// Dokument (id, name, page_count, page_sizes, signed)
    pub doc: DocInfo,
    /// Größe der Datei in Bytes
    pub size_bytes: u64,
    /// Phase
    pub phase: DocumentPhase,
    /// Revision der Platzierung (steigt bei jeder Änderung)
    pub revision: u64,
    /// Blockgröße und Vorschau-Revision
    pub block: BlockView,
    /// maßgebliche Lage (`PAGE_ABS`, linke obere Ecke)
    #[cfg_attr(feature = "ts", ts(as = "crate::ts::Placement"))]
    pub placement: pida_placement::Placement,
    /// Herkunft der Lage
    pub origin: PlacementOrigin,
    /// Ziel
    pub target: PlacementTarget,
    /// unsichtbar signieren (nur dieses Dokument)
    pub invisible: bool,
    /// virtuelle neue Seite
    pub new_page: bool,
    /// tatsächliches Rechteck des Blocks auf `placement.page_index` (bei Feld/QR das Ziel-Rechteck);
    /// `None` = unsichtbar bzw. keine Vorschau
    pub sticker: Option<RectPt>,
    /// leere, nicht gesperrte Signaturfelder – im Arbeitsbereich immer angeboten, nie automatisch benutzt
    pub fields: Vec<FieldTargetView>,
    /// QR-Platzhalter (nur mit Platzhaltersuche)
    pub qr: Option<QrTargetView>,
    /// vorhandene Signaturen
    pub existing: Vec<ExistingSignatureView>,
    /// Einschränkungen
    pub constraints: DocConstraintsView,
    /// Layout der zuletzt angefragten Seiten (`InspectPages`, ≤ 8) und der Seite des Stickers, aufsteigend
    /// nach `page`, höchstens 9; Seiten ohne fertiges Raster fehlen
    pub layout: Vec<PageLayoutView>,
    /// Live-Prüfung
    pub check: PlacementCheckView,
    /// Live-Region: `page`, `pages`, `left`, `top` (wie `StepView::Position`) und `bottom` (mm der
    /// Unterkante von unten)
    pub announce: BTreeMap<String, String>,
}

redacted_debug!(DocumentView {
    size_bytes, phase, revision, block, placement, origin, target, invisible, new_page, sticker, fields, qr,
    constraints, layout, check, announce,
} redact { doc, existing });

// --- §3.4 Signiermittel und Konto-Chip ----------------------------------------------------------

/// Signiermittel für Konto-Chip, Menü und Stapel (§3.4).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SignersView {
    /// Optionen (ohne reservierte Backends)
    pub options: Vec<SignerView>,
    /// Auswahl des Konto-Chips (Start = Standard, sofern verfügbar; bei Richtlinie `forced_backend` fest);
    /// mit „Jedes Mal fragen“ nur bis zum Ende des Dokuments bzw. Stapellaufs (§8 F12)
    pub selected: Option<BackendId>,
    /// Standard aus den Einstellungen (`None` = „Jedes Mal fragen“)
    pub default: Option<BackendId>,
}

/// Ein Signiermittel im Konto-Chip (§3.4).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignerView {
    /// Backend
    pub backend: BackendId,
    /// verfügbar
    pub available: bool,
    /// Kontoanzeige: ID Austria „+43 660 ···123“ (nur bei gemerktem Benutzer, im Kern maskiert),
    /// Keystore „max.p12 · alias“; lokale BKU `None`
    pub account: Option<String>,
    /// Fähigkeiten (u. a. `document_leaves_device`, Faktoren, Qualifikation)
    pub capabilities: Option<Capabilities>,
    /// Hinweise (i18n-IDs) wie [`BackendOptionView::notes`]
    pub notes: Vec<String>,
    /// Grund, falls nicht verfügbar (i18n-ID): `ng-signer-localbku-not-running`,
    /// `ng-signer-keystore-missing`, `ng-signer-keystore-import` (Mobile), `ng-signer-policy`
    pub unavailable_reason: Option<String>,
}

redacted_debug!(SignerView { backend, available, capabilities, notes, unavailable_reason } redact { account });

// --- §3.6 Signierkarte --------------------------------------------------------------------------

/// Phase der Signierkarte (§3.6).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningStage {
    /// Verbindung bzw. Sitzung wird geöffnet
    #[default]
    Connecting,
    /// Anmeldung (offene Login-Maske)
    Login,
    /// Zertifikat wird gelesen
    Certificate,
    /// Dokument wird vorbereitet
    Preparing,
    /// Bestätigung (App, QR, SMS, FIDO2, Passwort, Karte)
    Confirming,
    /// Signatur wird erstellt und geprüft
    Finishing,
    /// wird gespeichert (Stapellauf)
    Saving,
    /// Stapelpause ([`SigningProgressView::pause`])
    Paused,
}

/// Bestätigungsweg (§3.6).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmMethod {
    /// App „ID Austria“
    App,
    /// QR-Code
    Qr,
    /// SMS-TAN
    Sms,
    /// FIDO2-Schlüssel
    Fido2,
    /// Passwort (Keystore)
    Password,
    /// Karte (lokale BKU)
    Card,
}

/// Herkunft des Signaturzertifikats in dieser Sitzung (§3.6).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificateSource {
    /// wird bzw. wurde in dieser Sitzung gelesen (InfoboxRead; ggf. eigene Bestätigung)
    #[default]
    Read,
    /// aus dem Zertifikats-Cache (keine Bestätigung)
    Cache,
    /// bereits in dieser Sitzung gelesen (Stapel ab Dokument 2, Wiederholen)
    Session,
    /// nicht nötig (unsichtbar, Keystore, lokale BKU)
    NotNeeded,
}

/// Wirkungsbereich einer Stapelpause (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseScope {
    /// nur dieses Dokument
    Document,
    /// Sitzung bzw. Verbindung
    Session,
}

/// Pause eines Stapellaufs mit ausdrücklicher Entscheidung (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchPauseView {
    /// Dokument (`None` = Sitzungsfehler vor dem ersten Dokument)
    pub doc: Option<DocRefView>,
    /// Fehlercode (i18n-ID)
    pub code: String,
    /// Parameter
    pub params: BTreeMap<String, String>,
    /// Wirkungsbereich
    pub scope: PauseScope,
    /// „Erneut versuchen“ möglich
    pub can_retry: bool,
    /// „Erneut versuchen“ öffnet eine neue Sitzung (ggf. neue Anmeldung)
    pub new_session: bool,
}

redacted_debug!(BatchPauseView { doc, code, scope, can_retry, new_session } redact { params });

/// Fortschritt der Signierkarte (Stepper „Anmelden → Bestätigen k/n → Signiert“; §3.6).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SigningProgressView {
    /// Signiermittel
    pub backend: Option<BackendId>,
    /// Phase
    pub stage: SigningStage,
    /// Bestätigungsweg der offenen Maske bzw. des Backends
    pub method: Option<ConfirmMethod>,
    /// Dokument der laufenden Bestätigung
    pub doc: Option<DocRefView>,
    /// 1-basiert; Einzeldokument 1
    pub index: u32,
    /// Einzeldokument 1
    pub total: u32,
    /// Anmeldung dieser Sitzung erfolgreich; wird wieder `false`, sobald das Backend erneut die
    /// Login-Maske zeigt
    pub login_done: bool,
    /// abgeschlossene Bestätigungen
    pub confirmations_done: u32,
    /// Bestätigungen gesamt (je Dokument eine)
    pub confirmations_total: u32,
    /// Herkunft des Zertifikats
    pub certificate: CertificateSource,
    /// Stapellauf
    pub batch: bool,
    /// Stapelpause; bei Einzeldokumenten immer `None` (dort `StepView::Error`)
    pub pause: Option<BatchPauseView>,
}

/// Herkunft eines Fehlers (§3.6).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorOrigin {
    /// Laden
    Load,
    /// Analyse
    Analysis,
    /// Vorbereiten
    Prepare,
    /// Signieren (Fehlerschritt in der Karte)
    Signing,
    /// Ausgabe
    Output,
    /// Einstellungen
    Config,
    /// sonstiges
    #[default]
    Other,
}

/// Kontext eines Fehlers für „Technische Details“ (§3.6).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ErrorContextView {
    /// Herkunft
    pub origin: ErrorOrigin,
    /// RFC 3339, lokale Zeit mit Offset
    pub occurred_at: Option<String>,
    /// Signiermittel
    pub backend: Option<BackendId>,
    /// Dokument
    pub doc: Option<DocRefView>,
}

// --- §3.7 Fertig --------------------------------------------------------------------------------

/// Zusammenfassung „Fertig“ (§3.7). Wortlaut nach §8 F3: nie „gültig“, sondern „Signatur erstellt und
/// geprüft (Selbstprüfung)“, Qualifikation „laut Zertifikat“.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignedSummaryView {
    /// Dateiname der Ausgabe
    pub file_name: String,
    /// Desktop: Ordner der Ausgabe; Mobile: `None` („in den Dokumenten dieser App“)
    pub folder_display: Option<String>,
    /// CN des verwendeten Signaturzertifikats
    pub signer: Option<String>,
    /// RFC 3339 wie `/M`
    pub signing_time: String,
    /// Signiermittel
    pub backend: BackendId,
    /// bis K4 aus den Fähigkeiten des Backends, ab K4 aus den qcStatements des Signerzertifikats
    pub qualification: Qualification,
    /// Selbstprüfung I1–I6 bestanden (ohne Bestehen wird nie gespeichert – dient der Anzeige)
    pub self_check: bool,
    /// Seite des sichtbaren Blocks (`None` = unsichtbar)
    pub page: Option<usize>,
}

redacted_debug!(SignedSummaryView { signing_time, backend, qualification, self_check, page }
    redact { file_name, folder_display, signer });

// --- §3.8 Stapel --------------------------------------------------------------------------------

/// Status eines Stapeleintrags (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BatchItemStatusView {
    /// Voranalyse läuft
    Analyzing,
    /// bereit (automatische Platzierung)
    Ready,
    /// mit Plan (QR-Platzhalter oder im Arbeitsbereich platziert)
    Positioned,
    /// abgelehnt (z. B. `error-PDFPwdProtected`) – wird nie signiert
    Rejected {
        /// Code
        code: String,
    },
    /// Analysewarnung: erst einzeln öffnen und bestätigen
    NeedsReview {
        /// Codes der Warnungen
        codes: Vec<String>,
    },
    /// wartet im Lauf
    Queued,
    /// wird signiert
    Active {
        /// Phase
        stage: SigningStage,
    },
    /// signiert
    Signed {
        /// Ausgabe
        path: PathBuf,
    },
    /// fehlgeschlagen
    Failed {
        /// Code
        code: String,
        /// Parameter
        params: BTreeMap<String, String>,
    },
    /// übersprungen: durch die Nutzerin bzw. – `existing` – weil das Ziel existiert (`on_existing = skip`)
    Skipped {
        /// Ziel existierte
        existing: bool,
    },
    /// abgebrochen (Stapelabbruch)
    Cancelled,
}

impl std::fmt::Debug for BatchItemStatusView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchItemStatusView::Analyzing => f.write_str("Analyzing"),
            BatchItemStatusView::Ready => f.write_str("Ready"),
            BatchItemStatusView::Positioned => f.write_str("Positioned"),
            BatchItemStatusView::Rejected { code } => f.debug_struct("Rejected").field("code", code).finish(),
            BatchItemStatusView::NeedsReview { codes } => f.debug_struct("NeedsReview").field("codes", codes).finish(),
            BatchItemStatusView::Queued => f.write_str("Queued"),
            BatchItemStatusView::Active { stage } => f.debug_struct("Active").field("stage", stage).finish(),
            BatchItemStatusView::Signed { .. } => f.debug_struct("Signed").field("path", &Redacted).finish(),
            BatchItemStatusView::Failed { code, .. } => {
                f.debug_struct("Failed").field("code", code).field("params", &Redacted).finish()
            }
            BatchItemStatusView::Skipped { existing } => f.debug_struct("Skipped").field("existing", existing).finish(),
            BatchItemStatusView::Cancelled => f.write_str("Cancelled"),
        }
    }
}

/// Kurzfassung der Platzierung eines Stapeleintrags (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PositionSummaryView {
    /// automatisch
    Auto,
    /// manuell auf dieser Seite
    Manual {
        /// Seite
        page: usize,
    },
    /// im Signaturfeld
    Field {
        /// Feldname
        name: String,
    },
    /// im QR-Platzhalter
    Qr,
    /// unsichtbar
    Invisible,
}

/// Eintrag der Stapelübersicht (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchItemView {
    /// Dokument
    pub doc: DocId,
    /// Name
    pub name: String,
    /// Seitenzahl (nach der Voranalyse)
    pub page_count: Option<usize>,
    /// Größe in Bytes
    pub size_bytes: Option<u64>,
    /// Status
    pub status: BatchItemStatusView,
    /// Platzierung
    pub position: PositionSummaryView,
    /// Ausgabedateiname nach F-81
    pub target_name: Option<String>,
    /// Ziel existiert bereits (Vorabprüfung)
    pub target_exists: bool,
    /// Miniatur über `pida-render://thumb/<doc>` verfügbar
    pub thumbnail: bool,
}

redacted_debug!(BatchItemView { doc, page_count, size_bytes, status, position, target_exists, thumbnail }
    redact { name, target_name });

/// Ausgabeart des Stapels (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputModeView {
    /// neben dem Original
    NextToSource,
    /// fester Ordner
    Folder,
    /// jedes Mal fragen
    Ask,
    /// App-Dokumente (Mobile)
    AppDocuments,
}

/// Ausgabe des Stapels (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchOutputView {
    /// Art
    pub mode: OutputModeView,
    /// Ordner zur Anzeige
    pub folder_display: Option<String>,
    /// `StartBatch` braucht `output_dir` („jedes Mal fragen“ oder kein vorhandener Ordner)
    pub needs_folder: bool,
}

redacted_debug!(BatchOutputView { mode, needs_folder } redact { folder_display });

/// Schätzung „n Dokumente · 1 Anmeldung · n Bestätigungen“ für das gewählte Signiermittel (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchEstimateView {
    /// Signiermittel
    pub backend: BackendId,
    /// Dokumente
    pub documents: u32,
    /// erwartete Anmeldungen (0 oder 1; eine Erwartung, keine Zusage)
    pub logins: u32,
    /// Bestätigungen (= `documents` bei `max_batch == 1`)
    pub confirmations: u32,
    /// ggf. eine weitere Bestätigung für das Zertifikat
    pub certificate_confirmation_possible: bool,
    /// ab 20 Dokumenten mit ID Austria: Hinweis `ng-batch-large` (§8 F9)
    pub large: bool,
}

/// Zusammenfassung über **alle** Einträge der Übersicht nach dem letzten Lauf (§3.8).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchSummaryView {
    /// Einträge gesamt
    pub total: u32,
    /// signiert (auch vorher einzeln signierte)
    pub signed: u32,
    /// abgelehnt
    pub rejected: u32,
    /// fehlgeschlagen
    pub failed: u32,
    /// übersprungen
    pub skipped: u32,
    /// abgebrochen
    pub cancelled: u32,
    /// nicht begonnen
    pub not_started: u32,
    /// Speicherort zur Anzeige
    pub folder_display: Option<String>,
}

redacted_debug!(BatchSummaryView { total, signed, rejected, failed, skipped, cancelled, not_started }
    redact { folder_display });

/// Stapel (§3.8; nur mit `batch_overview`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchView {
    /// Einträge
    pub items: Vec<BatchItemView>,
    /// gerade im Arbeitsbereich geöffnetes Dokument (`OpenBatchItem`)
    pub open_item: Option<DocId>,
    /// Lauf aktiv (Details in `StepView::Signing::progress`)
    pub running: bool,
    /// nach dem letzten Lauf
    pub summary: Option<BatchSummaryView>,
    /// Ausgabe
    pub output: BatchOutputView,
    /// für `signers.selected` (`None` = kein Signiermittel gewählt)
    pub estimate: Option<BatchEstimateView>,
}

// --- §3.9 Prüfmodus -----------------------------------------------------------------------------

/// Ladezustand des Prüfmodus (§3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VerifyStatus {
    /// wird geladen
    Loading,
    /// geprüft
    Ready,
    /// nicht prüfbar (z. B. `ng-verify-open-failed`)
    Failed {
        /// Code
        code: String,
    },
}

/// Gesamtergebnis (§3.9; Wortlaut §8 F3 – PIDA behauptet nie Rechtsgültigkeit).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifySummaryView {
    /// keine Signaturen
    NoSignatures,
    /// alle unverändert und kryptografisch intakt
    AllIntact,
    /// mindestens eine verändert/ungültig
    SomeBroken,
    /// keine verletzt, aber mindestens eine nicht prüfbar
    SomeNotVerifiable,
    /// alle intakt, aber die Datei wurde nach der letzten Signatur noch ergänzt oder geändert
    ModifiedAfterLastSignature,
}

/// Integrität einer Signatur (§3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignatureIntegrity {
    /// unverändert und kryptografisch intakt
    Intact,
    /// verändert bzw. ungültig
    Broken {
        /// Code
        code: String,
    },
    /// nicht prüfbar
    NotVerifiable {
        /// Code
        code: String,
    },
}

/// Abdeckung einer Signatur (§3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignatureCoverage {
    /// ganze Datei
    WholeDocument,
    /// eine frühere Revision
    Revision {
        /// spätere Revisionen
        later_revisions: u32,
        /// Signaturen in späteren Revisionen
        later_signatures: u32,
    },
}

/// Qualifikation **laut Zertifikat** (qcStatements; nicht gegen die EU-Vertrauensliste geprüft; §3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QualificationClaim {
    /// QcCompliance (0.4.0.1862.1.1); `qscd` = QcSSCD (0.4.0.1862.1.4)
    Qualified {
        /// qualifizierte Signaturerstellungseinheit laut Zertifikat
        qscd: bool,
    },
    /// nicht qualifiziert
    NotQualified,
    /// unbekannt
    Unknown,
}

/// Format einer Signatur (`/SubFilter`; §3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureFormat {
    /// `ETSI.CAdES.detached`
    Pades,
    /// `adbe.pkcs7.detached`
    Pkcs7Detached,
    /// `adbe.pkcs7.sha1` (immer nicht prüfbar)
    Pkcs7Sha1,
    /// sonstiges (u. a. Dokument-Zeitstempel `ETSI.RFC3161`)
    Other,
}

/// Lage eines Signaturbilds (`PAGE_ABS`; §3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppearanceView {
    /// Seite
    pub page: usize,
    /// Rechteck
    pub rect: RectPt,
}

/// Prüfung von Vertrauenskette und Sperrstatus (§3.9): in v1 nie (Hinweis auf signaturpruefung.gv.at).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustCheck {
    /// nicht geprüft
    #[default]
    NotChecked,
}

/// Eine geprüfte Signatur (§3.9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VerifiedSignatureView {
    /// 1-basiert in Dokumentreihenfolge
    pub index: u32,
    /// Feldname
    pub field_name: String,
    /// CN des Unterzeichners
    pub signer: Option<String>,
    /// O des Subjects
    pub organization: Option<String>,
    /// CN des Ausstellers
    pub issuer: Option<String>,
    /// Seriennummer (dezimal)
    pub serial: Option<String>,
    /// Gültigkeit des Zertifikats ab (RFC 3339)
    pub cert_valid_from: Option<String>,
    /// Gültigkeit des Zertifikats bis (RFC 3339)
    pub cert_valid_to: Option<String>,
    /// Angabe des Signierenden (`signing-time`), kein geprüfter Zeitstempel
    pub signing_time: Option<String>,
    /// Zertifikat zum angegebenen Zeitpunkt gültig
    pub cert_valid_at_signing: Option<bool>,
    /// Integrität
    pub integrity: SignatureIntegrity,
    /// Abdeckung
    pub coverage: SignatureCoverage,
    /// Qualifikation laut Zertifikat
    pub qualification: QualificationClaim,
    /// Format
    pub format: SignatureFormat,
    /// Hashverfahren, z. B. „SHA-256“
    pub digest: Option<String>,
    /// Signaturverfahren, z. B. „ECDSA P-256“
    pub algorithm: Option<String>,
    /// Signaturbild (`None` = unsichtbar)
    pub appearance: Option<AppearanceView>,
    /// Hinweise zu einer intakten Signatur (i18n-IDs): `ng-verify-warning-sha1`,
    /// `ng-verify-warning-signing-certificate-v1`, `ng-verify-warning-weak-key`
    pub warnings: Vec<String>,
    /// Vertrauenskette/Sperrstatus (in v1 immer `not_checked`)
    pub trust: TrustCheck,
}

redacted_debug!(VerifiedSignatureView {
    index, field_name, issuer, cert_valid_from, cert_valid_to, signing_time, cert_valid_at_signing, integrity,
    coverage, qualification, format, digest, algorithm, appearance, warnings, trust,
} redact { signer, organization, serial });

/// Prüfmodus (§3.9) – liegt über dem Workflow-Zustand.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VerifyView {
    /// Dateiname
    pub file_name: String,
    /// Ladezustand
    pub status: VerifyStatus,
    /// für Kacheln über `pida-render://page/<doc.id>/<page>` (eigene `DocId`, nicht in der Warteschlange)
    pub doc: Option<DocInfo>,
    /// Signaturen
    pub signatures: Vec<VerifiedSignatureView>,
    /// Gesamtergebnis
    pub summary: VerifySummaryView,
    /// Revisionen der Datei
    pub revisions: u32,
}

redacted_debug!(VerifyView { status, signatures, summary, revisions } redact { file_name, doc });

// --- §3.10 Zuletzt signiert ---------------------------------------------------------------------

/// Eintrag „Zuletzt signiert“ (§3.10; gespeichert werden nur Dateiname, Pfad und Zeitpunkt).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecentItemView {
    /// Kennung
    pub id: u64,
    /// Dateiname
    pub file_name: String,
    /// Desktop: Ordner; Mobile: `None`
    pub folder_display: Option<String>,
    /// RFC 3339, lokale Zeit mit Offset
    pub signed_at: String,
    /// Datei noch vorhanden (beim Projizieren geprüft, gecacht)
    pub exists: bool,
}

redacted_debug!(RecentItemView { id, signed_at, exists } redact { file_name, folder_display });

// --- §3.12 Ersteinrichtung ----------------------------------------------------------------------

/// Eintrag der Übernahmeliste (§3.12): i18n-ID `ng-migration-item-{block,emblem,username,output,keystore,
/// proxy,placeholder,language}` mit Detail (z. B. Dateiname der Bildmarke).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationItemView {
    /// i18n-ID
    pub code: String,
    /// Detail
    pub detail: Option<String>,
}

redacted_debug!(MigrationItemView { code } redact { detail });

/// Ersteinrichtung (§3.12; nur beim ersten Start).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FirstRunView {
    /// Migrationsangebot (gleich [`ViewState::migration`]); `None` = Schritt 1 entfällt
    pub migration: Option<MigrationOfferView>,
    /// Standard-Signiermittel (`None` = „Jedes Mal fragen“)
    #[cfg_attr(feature = "ts", ts(as = "Option<crate::ts::BackendChoice>"))]
    pub default_backend: Option<pida_config::BackendChoice>,
    /// Profil des Signaturblocks
    #[cfg_attr(feature = "ts", ts(as = "crate::ts::SignatureProfile"))]
    pub profile: pida_config::SignatureProfile,
    /// Sprache des Signaturblocks (`None` = System)
    pub block_language: Option<Lang>,
    /// Revision der Blockvorschau
    pub preview_revision: u64,
}

redacted_debug!(FirstRunView { default_backend, profile, block_language, preview_revision } redact { migration });

// --- §3.13 Freigaben ----------------------------------------------------------------------------

/// Freigaben für Knöpfe, Befehlspalette und Tastenkürzel (§3.13), berechnet aus dem Zustand
/// (`actions::actions_for`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ActionsView {
    /// Dateidialog/Ablegen sinnvoll (nicht während des Signierens)
    pub open_files: bool,
    /// `sign_with` möglich (ohne `signers.selected` öffnet die UI das Signiermittel-Menü)
    pub sign: bool,
    /// Sticker und Optionen bedienbar
    pub edit_placement: bool,
    /// Stapelübersicht mit mindestens einem signierbaren Dokument
    pub start_batch: bool,
    /// weitere Dokumente in der Warteschlange (`show_batch`)
    pub show_batch: bool,
    /// `open_verify` erlaubt
    pub verify: bool,
    /// Einstellungen erreichbar (= `main_bar.settings_enabled`)
    pub settings: bool,
    /// `navigate{Open}` erlaubt (Startansicht)
    pub home: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET_NAME: &str = "Max Mustermann";
    const SECRET_FILE: &str = "Geheimvertrag";
    const SECRET_DIR: &str = "/Users/max";

    fn assert_redacted(debug: String) {
        for s in [SECRET_NAME, SECRET_FILE, SECRET_DIR, "+43 660"] {
            assert!(!debug.contains(s), "Debug enthält {s:?}: {debug}");
        }
    }

    /// ADR 0004 §3.0 Regel 4 (ADR 0001 Regel 9): neue Anzeigetypen mit Namen, Dateinamen, Pfaden oder
    /// Kontoanzeige zeigen sie in `Debug` nicht.
    #[test]
    fn new_view_types_redact_personal_data_in_debug() {
        let file = format!("{SECRET_FILE}.pdf");
        let doc = DocRefView { id: DocId(1), name: file.clone() };
        assert_redacted(format!("{doc:?}"));
        let summary = SignedSummaryView {
            file_name: file.clone(),
            folder_display: Some(SECRET_DIR.into()),
            signer: Some(SECRET_NAME.into()),
            signing_time: "2026-10-15T10:00:00+02:00".into(),
            backend: BackendId::AtrustNative,
            qualification: Qualification::Qes,
            self_check: true,
            page: Some(0),
        };
        assert_redacted(format!("{summary:?}"));
        let signer = SignerView {
            backend: BackendId::AtrustNative,
            available: true,
            account: Some("+43 660 ···123".into()),
            capabilities: None,
            notes: Vec::new(),
            unavailable_reason: None,
        };
        assert_redacted(format!("{signer:?}"));
        let item = BatchItemView {
            doc: DocId(2),
            name: file.clone(),
            page_count: Some(1),
            size_bytes: None,
            status: BatchItemStatusView::Signed { path: format!("{SECRET_DIR}/{file}").into() },
            position: PositionSummaryView::Auto,
            target_name: Some(file.clone()),
            target_exists: false,
            thumbnail: false,
        };
        assert_redacted(format!("{item:?}"));
        let mut params = BTreeMap::new();
        params.insert("arg1".to_owned(), file.clone());
        assert_redacted(format!("{:?}", BatchItemStatusView::Failed { code: "x".into(), params: params.clone() }));
        let pause = BatchPauseView {
            doc: Some(doc.clone()),
            code: "output-save_failed".into(),
            params,
            scope: PauseScope::Document,
            can_retry: true,
            new_session: false,
        };
        assert_redacted(format!("{pause:?}"));
        let recent = RecentItemView {
            id: 1,
            file_name: file.clone(),
            folder_display: Some(SECRET_DIR.into()),
            signed_at: "2026-10-15T10:00:00+02:00".into(),
            exists: true,
        };
        assert_redacted(format!("{recent:?}"));
        let existing = ExistingSignatureView {
            field_name: "Signatur1".into(),
            signer: Some(SECRET_NAME.into()),
            signing_time: None,
            status: PriorStatusView::Valid,
            page: None,
            rect: None,
        };
        assert_redacted(format!("{existing:?}"));
        assert_redacted(format!("{:?}", MigrationItemView { code: "c".into(), detail: Some(file.clone()) }));
        let output = BatchOutputView {
            mode: OutputModeView::Folder,
            folder_display: Some(SECRET_DIR.into()),
            needs_folder: false,
        };
        assert_redacted(format!("{output:?}"));
        // Ereignisse mit Pfaden bzw. Einstellungswerten
        assert_redacted(format!("{:?}", UiEvent::OpenVerify { path: format!("{SECRET_DIR}/{file}").into() }));
        assert_redacted(format!(
            "{:?}",
            UiEvent::StartBatch {
                backend: BackendId::Keystore,
                docs: vec![DocId(1)],
                output_dir: Some(SECRET_DIR.into()),
                on_existing: ExistingPolicy::KeepBoth,
            }
        ));
        let changes = BTreeMap::from([("id_austria.username".to_owned(), serde_json::json!("+43 660 1234567"))]);
        let apply = format!("{:?}", UiEvent::ApplySettings { changes, revision: 3 });
        assert_redacted(apply.clone());
        assert!(apply.contains("id_austria.username"), "Pfade der Felder bleiben sichtbar: {apply}");
    }
}
