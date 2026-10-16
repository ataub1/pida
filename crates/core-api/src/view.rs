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
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationOfferView {
    /// gefundene `PDF-Over.config`
    pub legacy_file: PathBuf,
    /// die Altdatei enthält Klartext-Passwörter (Keystore/Proxy): Zustimmung zum Übernehmen in den
    /// Schlüsselbund erfragen (`UiEvent::AcceptMigration::store_secrets`)
    #[serde(default)]
    pub plaintext_secrets: bool,
    /// Was übernommen würde (Ersteinrichtung Schritt 1, ADR 0004 §3.12): i18n-IDs `ng-migration-item-*`
    /// mit Detail; per Probelauf von `pida_config::migrate` (ohne zu schreiben)
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
    /// signiert wird nur mit ID Austria (kein Keystore, ADR 0004 Nachtrag 11; keine lokale BKU), kein FIDO2
    /// (ausgegraut mit Hinweis), keine Proxy-Einstellungen (System-Proxy), keine Ausgabeordner-Wahl
    /// (App-Dokumente)
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

/// Ergebnis des PKCS#12-Imports in die App-Sandbox (früherer Mobile-Import-Assistent; Plan §5.2) – schließen
/// mit [`UiEvent::DismissKeystoreImport`]. Seit ADR 0004 Nachtrag 11 gibt es auf Mobilgeräten keinen Keystore
/// (nur ID Austria): der Kern setzt es dort nie; der Typ bleibt im Vertrag (additiv, §3.0 Regel 2).
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
        /// Erste Verwendung des Keystores: unter dem Passwortfeld „Passwort merken?“ anbieten (Antwort in
        /// [`UiEvent::SubmitSecret::password_storage`]); nur für das Keystore-Passwort einer Datei aus den
        /// Einstellungen, deren Speicherart noch nie gewählt wurde
        #[serde(default)]
        ask_storage: bool,
        /// Schlüsselbund bzw. sicherer Speicher verfügbar (sonst ist „Im Schlüsselbund“ nicht wählbar)
        #[serde(default)]
        keychain_available: bool,
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

/// Am Inhalt erkanntes Keystore-Format (Einstellungen › Signiermittel › Keystore; ersetzt die Typauswahl).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeystoreFormatView {
    /// PKCS#12 (`.p12`/`.pfx`) – unterstützt
    Pkcs12,
    /// Java KeyStore – nicht unterstützt (Umwandlung in PKCS#12 nötig)
    Jks,
    /// Java JCEKS – nicht unterstützt (Umwandlung in PKCS#12 nötig)
    Jceks,
    /// kein bekanntes Keystore-Format
    Unknown,
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
    /// Keystore ist eingerichtet (aktiviert, die gewählte Datei existiert und ist PKCS#12); erst dann ist
    /// er als Standard-Signiermittel wählbar
    #[serde(default)]
    pub keystore_ready: bool,
    /// Am Dateiinhalt erkanntes Format der gewählten Keystore-Datei (`None` = keine/fehlende Datei). Ersetzt
    /// die Typauswahl aus PDF-Over (dort aus der Dateiendung); signiert wird nur mit PKCS#12.
    #[serde(default)]
    pub keystore_format: Option<KeystoreFormatView>,
    /// Einträge der gewählten PKCS#12-Datei für die Alias-Auswahl (nach Alias sortiert), sobald das Passwort
    /// bekannt ist: automatisch mit einem gemerkten Passwort (Speicherart), sonst nach
    /// [`UiEvent::InspectKeystore`]. `None` = noch unbekannt (bzw. keine PKCS#12-Datei)
    #[serde(default)]
    pub keystore_entries: Option<Vec<KeystoreEntryView>>,
    /// Problem der letzten Prüfung über [`UiEvent::InspectKeystore`] (Meldungs-ID, z. B.
    /// `keystore-wrong-password`); `None` = keines
    #[serde(default)]
    pub keystore_inspect_problem: Option<String>,
    /// Einstellungen werden nicht gespeichert (Altdatei per `-c`, unlesbare Datei)
    #[serde(default)]
    pub read_only: bool,
    /// Revision der Blockvorschau (Cache-Busting für `blockPreviewUrl` im Einstellungsdialog);
    /// der Kern baut die Vorschau beim Öffnen auch ohne Dokument (A4, Beispielzertifikat) wie PDF-Over
    #[serde(default)]
    pub preview_revision: u64,
    /// Revision des zuletzt übernommenen [`UiEvent::ApplySettings`] (ADR 0004 §3.11). Ändert eine Übernahme das
    /// Signaturbild, steigt sie erst zusammen mit der neuen Vorschau (`preview_revision`) in derselben
    /// Projektion
    #[serde(default)]
    pub applied_revision: u64,
    /// Vom Kern berechnete Beispiele (ADR 0004 §3.11): Schlüssel `output.postfix` → Ausgabename für die
    /// Beispieldatei `Vertrag.pdf` (F-81)
    #[serde(default)]
    pub examples: BTreeMap<String, String>,
    /// Per Richtlinie festgelegte Einstellungen: TOML-Punktpfad (bzw. ganzer Abschnitt wie `proxy`) → i18n-ID
    /// des Grunds (`ng-recent-policy`, `ng-settings-policy`). Die UI zeigt diese Zeilen gesperrt mit Hinweis;
    /// `ApplySettings` lehnt sie mit dem Grund als Problem ab. Nachtrag K3 (ADR 0004 §9, Vorschlag aus B3
    /// `contract-draft.ts`): additiv, in TypeScript optional; der Kern liefert es immer (`Some`, ggf. leer).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub locked: Option<BTreeMap<String, String>>,
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
    /// Ergebnis des PKCS#12-Imports (früherer Mobile-Import-Assistent; seit ADR 0004 Nachtrag 11 immer `None`
    /// auf Mobilgeräten)
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
    /// Prüfmodus – liegt über dem Workflow-Zustand, ändert ihn nicht (ADR 0004 §3.9; nur im Arbeitsbereich)
    #[serde(default)]
    pub verify: Option<VerifyView>,
    /// „Zuletzt signiert“ (neueste zuerst, höchstens zehn); `None` = Funktion abgeschaltet (Einstellung,
    /// Richtlinie) bzw. ohne Oberfläche (ADR 0004 §3.10)
    #[serde(default)]
    pub recent: Option<Vec<RecentItemView>>,
    /// Ersteinrichtung, nur beim ersten Start (fehlende Einstellungsdatei, Arbeitsbereich; ADR 0004 §3.12)
    #[serde(default)]
    pub first_run: Option<FirstRunView>,
    /// Freigaben für Knöpfe, Befehlspalette und Tastenkürzel (ADR 0004 §3.13), aus dem Zustand berechnet
    #[serde(default)]
    pub actions: ActionsView,
    /// Geparkte Arbeit („Zur Startseite“, ADR 0004 Nachtrag 10): gesetzt, solange die Startansicht ein offenes
    /// Dokument bzw. einen Stapel verdeckt (Karte „In Bearbeitung“); dann sind `document` und `batch` `None`.
    /// Additiv, in TypeScript optional; der Kern liefert es immer (`None` ohne geparkte Arbeit).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub parked: Option<ParkedWorkView>,
    /// iOS: Hinweis nach dem Empfang über „Teilen“ bzw. „Öffnen in“ (ADR 0004 Nachtrag 12) – die Dateien-App
    /// übergibt von mehreren geteilten PDFs nur das erste; weitere für einen Stapel wählt man über „Weitere PDFs
    /// hinzufügen …“ (Mehrfachauswahl). Nur, solange das empfangene Dokument allein im Arbeitsbereich offen ist,
    /// und nicht mehr nach `dismiss_share_hint` (bis zum Beenden der App). Additiv, in TypeScript optional; sonst
    /// `None` (Desktop, Android, Dateiauswahl).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub share_hint: Option<ShareHintView>,
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

/// Wie `open_files` die gewählten Dateien zum offenen Dokument nimmt (Kopfzeile „+ PDF“, Miniaturleiste;
/// ADR 0004 Nachtrag 9). Was daraus folgt, entscheidet der Kern nach dem Zustand.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenFilesMode {
    /// „Weitere PDFs hinzufügen“: das offene Dokument bleibt; im Arbeitsbereich werden alle zusammen zum Stapel
    /// (Stapelübersicht, die Platzierung des offenen Dokuments bleibt dessen Plan). Sonst wie ohne `mode`.
    Add,
    /// „Anderes PDF öffnen“: das offene Dokument (Analysewarnung, Arbeitsbereich) wird geschlossen und nie
    /// signiert, die Dateien öffnen wie in der Dokumentauswahl; wartende Dokumente bleiben. Sonst wie ohne
    /// `mode`.
    Replace,
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
        /// Wie die Dateien zum offenen Dokument kommen (Kopfzeile „+ PDF“, Miniaturleiste; ADR 0004 Nachtrag 9);
        /// `None` = wie bisher (der Kern entscheidet nach dem Zustand: Auswahl öffnen, sonst an die
        /// Warteschlange). Nur im Arbeitsbereich-Modus, sonst ohne Bedeutung. In TypeScript optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional = nullable))]
        mode: Option<OpenFilesMode>,
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
        /// Antwort auf „Passwort merken?“ ([`InteractionView::Secret::ask_storage`]): wird als
        /// `keystore.password_storage` gespeichert und gilt schon für dieses Passwort; danach fragt PIDA nicht
        /// mehr. Ohne Rückfrage ohne Wirkung
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(as = "Option<crate::ts::PasswordStorage>", optional = nullable))]
        password_storage: Option<pida_config::PasswordStorage>,
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
    /// PKCS#12-Datei in die App-Sandbox importieren (früherer Mobile-Import-Assistent): Datei kopieren,
    /// Passwort prüfen (falls angegeben), Einstellungen › Keystore setzen; mit `remember` und
    /// verfügbarem sicheren Speicher wird das Passwort dort abgelegt. Ergebnis in
    /// [`ViewState::keystore_import`]. Auf iOS/Android seit ADR 0004 Nachtrag 11 ohne Wirkung (nur ID Austria).
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
    /// serde-JSON des Felds; Felder mit Problem (ungültig, unbekannt `ng-settings-field-unknown`, per
    /// Richtlinie festgelegt, auf Mobilgeräten nicht verfügbar) behalten ihren Wert (`SettingsView::problems`),
    /// alle anderen werden übernommen und gespeichert (schreibgeschützt: nur im Speicher). Nur bei geöffneter
    /// Einstellungsansicht; eine `revision` ≤ der zuletzt übernommenen ist veraltet und ohne Wirkung.
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
    /// Einstellungen › Signiermittel › Keystore „Schlüssel anzeigen“: die gewählte PKCS#12-Datei mit dem
    /// Passwort öffnen und ihre Einträge in [`SettingsView::keystore_entries`] zeigen (Problem in
    /// [`SettingsView::keystore_inspect_problem`]). Ein richtiges Passwort wird gemäß
    /// `keystore.password_storage` behalten (wie nach dem Signieren). Nur bei geöffneter Einstellungsansicht;
    /// auf iOS/Android ohne Wirkung (nur ID Austria, ADR 0004 Nachtrag 11).
    InspectKeystore {
        /// Passwort (nur RAM, sofort in `Zeroizing`; nie geloggt); `None` = ein bekanntes (gemerktes)
        /// Passwort verwenden
        password: Option<String>,
        /// Antwort auf „Passwort merken?“ (nur solange `keystore.storage_chosen` falsch ist, sonst ohne
        /// Wirkung)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(as = "Option<crate::ts::PasswordStorage>", optional = nullable))]
        password_storage: Option<pida_config::PasswordStorage>,
    },
    /// „Diesmal selbst platzieren“ (Nachtrag 8): hebt die Sperre der Einstellung „Automatische
    /// Positionierung“ (`DocumentView::placement_locked`) für das aktuelle Dokument auf – danach normales
    /// Ziehen und Einrasten. Startet **nichts** (A1).
    UnlockPlacement,
    /// Stapelübersicht „Für diesen Stapel automatisch positionieren“ (Nachtrag 8, `BatchView::auto_position_once`):
    /// nur für diesen Stapel, die Einstellung bleibt unverändert; `replace_manual` = auch die von Hand
    /// platzierten Einträge (`BatchView::manual_plans`) neu automatisch platzieren – nur nach ausdrücklicher
    /// Bestätigung, sonst behalten sie ihren Plan. Startet **nichts** (A1, F-54: erst „Alle signieren“).
    SetBatchAutoPosition {
        /// ein bzw. aus
        enabled: bool,
        /// von Hand platzierte Einträge ebenfalls automatisch platzieren
        #[serde(default)]
        replace_manual: bool,
    },
    /// „Zur Startseite“ (Haus-Symbol, Wortmarke, Palette; ADR 0004 Nachtrag 10): schließt einen offenen Prüfmodus
    /// und parkt das offene Dokument (mit Platzierung) bzw. den Stapel, statt sie zu verwerfen (`ViewState::parked`).
    /// Beim Signieren und im Stapellauf ohne Wirkung (`actions.home`). Startet **nichts** (A1).
    GoHome,
    /// „Weiter bearbeiten“ (Karte „In Bearbeitung“): geparkte Arbeit unverändert fortsetzen. Startet **nichts**.
    ResumeParked,
    /// „Schließen“ (Karte „In Bearbeitung“): geparkte Arbeit verwerfen (Dokumente „abgebrochen“; signierte Dateien
    /// bleiben). Startet **nichts**.
    DiscardParked,
    /// Ersteinrichtung „Später einrichten“ (auch Esc; §3.12, ADR 0004 Nachtrag 12): wie [`UiEvent::FinishFirstRun`]
    /// abgeschlossen und gespeichert, aber mit dem Hinweis `ng-firstrun-later` („jederzeit in den Einstellungen
    /// einrichten“) statt „Einrichtung abgeschlossen“. `FinishFirstRun` bleibt für „Fertig“.
    PostponeFirstRun,
    /// Shell: von außen übergebene Dokumente – Mobile „Teilen“ bzw. „Öffnen in“, Desktop „Öffnen mit“ und zweite
    /// Instanz (ADR 0004 Nachtrag 12). Wirkt wie [`UiEvent::OpenFiles`] ohne `mode`; auf iOS zeigt der Kern danach
    /// [`ViewState::share_hint`]. Die Oberfläche sendet es nicht (Dateiauswahl und Ablegen bleiben `OpenFiles`).
    FilesReceived {
        /// Pfade (Kopien in der Sandbox)
        paths: Vec<PathBuf>,
    },
    /// Hinweis zum Teilen ausblenden ([`ViewState::share_hint`], Nachtrag 12): bis zum Beenden der App nicht
    /// mehr. Startet **nichts**.
    DismissShareHint,
}

impl std::fmt::Debug for UiEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiEvent::SubmitSecret { request, password_storage, .. } => f
                .debug_struct("SubmitSecret")
                .field("request", request)
                .field("secret", &"***")
                .field("password_storage", password_storage)
                .finish(),
            UiEvent::InspectKeystore { password, password_storage } => f
                .debug_struct("InspectKeystore")
                .field("password", &password.as_ref().map(|_| "***"))
                .field("password_storage", password_storage)
                .finish(),
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
            UiEvent::OpenFiles { paths, mode } => f
                .debug_struct("OpenFiles")
                .field("paths", &format_args!("<{} Pfade>", paths.len()))
                .field("mode", mode)
                .finish(),
            UiEvent::FilesReceived { paths } => {
                f.debug_struct("FilesReceived").field("paths", &format_args!("<{} Pfade>", paths.len())).finish()
            }
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

/// Eintrag eines PKCS#12-Keystores für die Alias-Auswahl (Einstellungen › Signiermittel › Keystore; nur
/// Schlüsseleinträge sind wählbar). Name und Alias sind Personendaten (ohne friendlyName ist der Alias der
/// Subject-Text) – `Debug` redigiert sie (Regel 9).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeystoreEntryView {
    /// Alias (Wert für `keystore.alias`)
    pub alias: String,
    /// erster Common Name (CN) des Zertifikats
    pub subject_cn: Option<String>,
    /// Ende der Gültigkeit des Zertifikats (RFC 3339 mit Offset, UTC)
    pub not_after: Option<String>,
    /// Schlüsseleintrag (sonst nur ein Zertifikat, z. B. eine Zertifizierungsstelle)
    pub is_key_entry: bool,
}

redacted_debug!(KeystoreEntryView { not_after, is_key_entry } redact { alias, subject_cn });

// Pfad der Altdatei und Details der Übernahmeliste (Ordner, maskierte Telefonnummer) nicht im Log (Regel 9)
redacted_debug!(MigrationOfferView { plaintext_secrets, items } redact { legacy_file });

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
    /// leere, nicht gesperrte Signaturfelder – im Arbeitsbereich immer angeboten; automatisch benutzt wird nur
    /// das erste und nur mit Platzhaltersuche „Leere Signaturfelder“ (Einstellungen › Dokument, Nachtrag 5)
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
    /// Was der Kern erkannt bzw. automatisch entschieden hat (Einstellungen › Dokument: PDF/A,
    /// Platzhaltersuche) – für die dezenten Hinweise im Arbeitsbereich. Nachtrag K3 (ADR 0004 §9, Vorschlag):
    /// additiv; in TypeScript optional (`detection?: … | null`), damit bestehende UI-Literale gültig bleiben.
    /// Der Kern liefert es im Arbeitsbereich immer (`Some`).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub detection: Option<DocumentDetectionView>,
    /// Platzierung gesperrt durch die Einstellung „Automatische Positionierung“ (`signature.auto_position`
    /// bzw. `-a`; ADR 0004 Nachtrag 8): automatisch platziert und bereit zum Signieren – der Kern ignoriert
    /// `move_block`, `nudge_block`, `change_page`, `use_target` und `auto_place`; `set_invisible`,
    /// `toggle_new_page` und `sign_with` wirken. `unlock_placement` („Diesmal selbst platzieren“) hebt die
    /// Sperre für dieses Dokument auf. Nie für Einträge der Stapelübersicht.
    #[serde(default)]
    pub placement_locked: bool,
}

redacted_debug!(DocumentView {
    size_bytes, phase, revision, block, placement, origin, target, invisible, new_page, sticker, fields, qr,
    constraints, layout, check, announce, detection, placement_locked,
} redact { doc, existing });

/// PDF/A-Kennung eines Dokuments (XMP `pdfaid:part`, `pdfaid:conformance`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PdfaIdView {
    /// Teil (1, 2, 3, 4)
    pub part: u8,
    /// Konformitätsstufe (A, B, U …), falls angegeben
    pub conformance: Option<String>,
}

/// Wirksame PDF/A-Entscheidung beim Signieren (Einstellungen › Dokument „PDF/A-kompatibel signieren“).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfaDecision {
    /// keine PDF/A-Kennung und nicht erzwungen (bzw. das Profil hat keine PDF/A-Variante)
    #[default]
    None,
    /// PDF/A-Kennung erkannt – wird PDF/A-konform signiert („PDF/A-Dokument erkannt“)
    Detected,
    /// ohne Kennung erzwungen (`signature.pdfa_force`, Profil mit PDF/A-Variante): „PDF/A-kompatible
    /// Signatur (erzwungen)“
    Forced,
}

/// Erkanntes und automatisch Entschiedenes zum Dokument (Nachtrag K3, Einstellungen › Dokument).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocumentDetectionView {
    /// PDF/A-Kennung des Dokuments (`None` = keine)
    pub pdfa: Option<PdfaIdView>,
    /// wirksame PDF/A-Entscheidung
    pub pdfa_decision: PdfaDecision,
    /// Lage stammt aus der Automatik (Startlage nach der Analyse bzw. Nachführung nach geänderten
    /// Einstellungen). Zusammen mit [`DocumentView::target`]: frei = „automatisch platziert“, `qr` =
    /// „QR-Platzhalter gefunden – Signatur dort platziert“, `field` = „Signaturfeld gefunden – Signatur im
    /// Feld platziert“ (Platzhaltersuche „Leere Signaturfelder“). `false`, sobald die Nutzerin selbst platziert
    /// (Ziehen, Tastatur, Seite, „Ins Feld setzen“, „Frei platzieren“/„Freie Fläche suchen“).
    pub auto_placed: bool,
}

// --- §3.4 Signiermittel und Konto-Chip ----------------------------------------------------------

/// Signiermittel für Konto-Chip, Menü und Stapel (§3.4).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SignersView {
    /// Optionen (ohne reservierte Backends; auf iOS/Android nur ID Austria, ADR 0004 Nachtrag 11)
    pub options: Vec<SignerView>,
    /// Auswahl des Konto-Chips (Start = Standard, sofern verfügbar; bei Richtlinie `forced_backend` fest);
    /// mit „Jedes Mal fragen“ nur bis zum Ende des Dokuments bzw. Stapellaufs (§8 F12)
    pub selected: Option<BackendId>,
    /// Standard aus den Einstellungen (`None` = „Jedes Mal fragen“); auf iOS/Android immer ID Austria (das
    /// einzige Signiermittel, sofern die Richtlinie es erlaubt)
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
    /// `ng-signer-keystore-missing`, `ng-signer-policy` (der frühere Mobile-Grund „PKCS#12-Datei importieren“ entfiel
    /// mit ADR 0004 Nachtrag 11 – auf Mobilgeräten gibt es keinen Keystore mehr)
    pub unavailable_reason: Option<String>,
    /// Ablauf eines Stapellaufs mit diesem Signiermittel (aus den Fähigkeiten; `None` ohne Fähigkeiten, z. B.
    /// Keystore nicht eingerichtet): Wortlaut von Startfläche, Stapelübersicht, Startdialog und Stapelkarte (ADR 0004
    /// Nachtrag 12). Additiv, in TypeScript optional; der Kern liefert es immer.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub batch_flow: Option<BatchFlowView>,
}

redacted_debug!(SignerView { backend, available, capabilities, notes, unavailable_reason, batch_flow } redact { account });

/// Ablauf eines Stapellaufs mit einem Signiermittel (ADR 0004 §3.8, Nachtrag 12). Der Kern leitet ihn aus den
/// Fähigkeiten des Backends ab ([`BatchFlowView::from_capabilities`]); die Oberfläche wählt danach ihre Texte
/// („einmal anmelden, dann jedes Dokument einzeln bestätigen“ nur bei ID Austria, „Bestätigung k von n“ nur mit
/// einer Bestätigung je Dokument).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchFlowView {
    /// einmal anmelden, dann jedes Dokument einzeln bestätigen (ID Austria: eine Anmeldung je Stapel, eine
    /// Freigabe je Dokument)
    LoginThenConfirmEach,
    /// jedes Dokument einzeln bestätigen, ohne gemeinsame Anmeldung (Chipkarte: PIN in der Chipkarten-Software
    /// je Dokument)
    ConfirmEach,
    /// einmal entsperren, danach alle Dokumente ohne weitere Rückfrage (Keystore: eine Freigabe für den ganzen
    /// Stapel)
    UnlockOnce,
}

impl BatchFlowView {
    /// Aus den Fähigkeiten: mehr als ein Dokument je Freigabe (`max_batch > 1`) = einmal entsperren; sonst je
    /// Dokument eine Bestätigung, mit bzw. ohne gemeinsame Anmeldung (`login_once_per_batch`).
    pub fn from_capabilities(c: &Capabilities) -> BatchFlowView {
        if c.max_batch > 1 {
            BatchFlowView::UnlockOnce
        } else if c.login_once_per_batch {
            BatchFlowView::LoginThenConfirmEach
        } else {
            BatchFlowView::ConfirmEach
        }
    }
}

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
    /// laut qcStatements des Signerzertifikats (QcCompliance und QcSSCD → `Qes`, sonst `AdES`; nicht gegen die
    /// Vertrauensliste geprüft)
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
    /// der letzte Lauf wurde abgebrochen („Stapel abbrechen“, Esc) – Titel der Zusammenfassung; `cancelled`
    /// allein genügt nicht (Abbruch vor dem ersten Dokument bzw. Einträge aus einem früheren Lauf;
    /// Nachtrag 4 im ADR 0004)
    #[serde(default)]
    pub aborted: bool,
}

redacted_debug!(BatchSummaryView { total, signed, rejected, failed, skipped, cancelled, not_started, aborted }
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
    /// Einstellung „Automatische Positionierung“ an (`signature.auto_position` bzw. `-a`): jeder Stapel wird
    /// automatisch positioniert – die Übersicht zeigt „Automatische Positionierung (laut Einstellung)“
    /// (Nachtrag 8)
    #[serde(default)]
    pub auto_position_setting: bool,
    /// „Für diesen Stapel automatisch positionieren“ (`set_batch_auto_position`): nur für diesen Stapel bis zum
    /// Ende bzw. Abbruch des Laufs bzw. bis der Stapel geschlossen wird; die Einstellung bleibt unverändert.
    /// Mit automatischer Positionierung (Einstellung oder diese Option) gelten automatisch platzierte Einträge
    /// als „Bereit“ (Nachtrag 8)
    #[serde(default)]
    pub auto_position_once: bool,
    /// noch nicht signierte, von Hand platzierte Einträge (eigener Plan): sie behalten beim Einschalten ihren
    /// Plan, außer `set_batch_auto_position { replace_manual: true }` nach ausdrücklicher Bestätigung
    #[serde(default)]
    pub manual_plans: u32,
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

// Feldnamen sind Dokumentinhalt (wie `pades::SignatureVerification`), der Aussteller eines selbstsignierten
// Zertifikats (Keystore) ist der Unterzeichner selbst – beides nicht im Log
redacted_debug!(VerifiedSignatureView {
    index, cert_valid_from, cert_valid_to, signing_time, cert_valid_at_signing, integrity,
    coverage, qualification, format, digest, algorithm, appearance, warnings, trust,
} redact { field_name, signer, organization, issuer, serial });

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
/// proxy,placeholder,position,language}` mit Detail (z. B. Dateiname der Bildmarke); `position` =
/// „Automatische Positionierung“ (`SIGNATURE_POSITION=auto`, Nachtrag 8).
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
/// (`actions::actions_for`). Bei offenem Prüfmodus (`ViewState::verify`) sind `sign`, `edit_placement`,
/// `start_batch` und `show_batch` aus: das Dokument darunter ist verdeckt (Nachtrag 6).
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
    /// „Zur Startseite“ (`go_home`; Fertig: `navigate{Open}`) erlaubt
    pub home: bool,
}

// --- Nachtrag 10: geparkte Arbeit --------------------------------------------------------------

/// Geparkte Arbeit (ADR 0004 Nachtrag 10): Karte „In Bearbeitung“ der Startansicht mit „Weiter bearbeiten“
/// (`resume_parked`) und „Schließen“ (`discard_parked`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParkedWorkView {
    /// Name des geparkten Dokuments; `None` = Stapel („Stapel · n Dokumente“)
    pub doc_name: Option<String>,
    /// offene Dokumente (Einträge des Stapels bzw. das Dokument samt wartenden)
    pub documents: u32,
    /// Signatur bzw. Stapeleinträge von Hand platziert (`DocumentDetectionView::auto_placed == false` bzw.
    /// `BatchView::manual_plans > 0`): „Schließen“ fragt vorher nach
    pub placed_by_hand: bool,
    /// signiert, aber noch nicht gespeichert („Fertig“ mit „Speichern“): „Schließen“ verwirft die Signatur und fragt
    /// vorher nach
    pub unsaved: bool,
}
redacted_debug!(ParkedWorkView { documents, placed_by_hand, unsaved } redact { doc_name });

// --- Nachtrag 12: Hinweis zum Teilen (iOS) ---------------------------------------------------------

/// Hinweis zum Teilen auf iOS (ADR 0004 Nachtrag 12, [`ViewState::share_hint`]): ein kurzer, nicht blockierender
/// Hinweis im Arbeitsbereich mit „Weitere PDFs hinzufügen …“ und „Ausblenden“ (`dismiss_share_hint`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShareHintView {
    /// das über „Teilen“ empfangene Dokument (das offene im Arbeitsbereich)
    pub doc: DocId,
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
            batch_flow: Some(BatchFlowView::LoginThenConfirmEach),
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
        let parked =
            ParkedWorkView { doc_name: Some(file.clone()), documents: 1, placed_by_hand: true, unsaved: false };
        assert_redacted(format!("{parked:?}"));
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
