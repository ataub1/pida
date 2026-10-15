// Automatisch erzeugt aus den Rust-Typen der Bridge (crates/core-api/src/view.rs und abhängige
// Crates) mit ts-rs – NICHT VON HAND ÄNDERN.
// Neu erzeugen: PDFOVER_BLESS=1 cargo test -p pdfover-core-api --test ts_bindings
//
// Abbildung (serde_json): Option<T> → T | null, PathBuf → string, u64/usize → number,
// Enums ohne serde-Attribut extern getaggt ({ "Variante": … } bzw. "Variante"),
// #[serde(tag = …)] intern getaggt.

/**
 * Unterstützte Sprachen.
 */
export type Lang = "de" | "en";

/**
 * Kennung eines Dokuments innerhalb einer Sitzung.
 */
export type DocId = number;

/**
 * Schritte der Hauptleiste (F-100).
 */
export type Step = "Open" | "Positioning" | "Signing" | "Finish";

/**
 * Zustand eines Schritts in der Hauptleiste (Tabelle docs/analyse/01 §3.5).
 */
export type MainBarStep = { 
/**
 * Schritt
 */
step: Step, 
/**
 * erreicht („A“)
 */
active: boolean, 
/**
 * klickbar („E“)
 */
enabled: boolean, };

/**
 * Antwort auf „Platzhalter verwenden?“.
 */
export type PlaceholderAnswer = "Yes" | "No" | "Cancel";

/**
 * Spiegel von `pdfover_pades::AcceptanceClass`.
 */
export type AcceptanceClass = "K1Regular" | "K2Repairable" | "K3Encrypted" | "K4Restricted" | "K5Rejected";

/**
 * Spiegel von `pdfover_pades::Invariant`.
 */
export type Invariant = "I1Prefix" | "I2ByteRange" | "I3Cms" | "I4PriorSignatures" | "I5Reparse" | "I6AllowedChanges";

/**
 * Spiegel von `pdfover_pades::InvariantViolation`.
 */
export type InvariantViolation = { 
/**
 * Invariante
 */
invariant: Invariant, 
/**
 * Detail (ohne Dokumentinhalt)
 */
detail: string, };

/**
 * Spiegel von `pdfover_pades::OutputInfo`.
 */
export type OutputInfo = { 
/**
 * Pfad
 */
path: string, 
/**
 * Länge in Bytes
 */
len: number, 
/**
 * SHA-256 (hex)
 */
sha256: string, };

/**
 * Fehler im Workflow (Fehlercode + Parameter für i18n).
 */
export type WorkflowError = { "Rejected": { 
/**
 * Klasse
 */
class: AcceptanceClass, 
/**
 * Code
 */
code: string, } } | { "Backend": { 
/**
 * `BackendError::code()`
 */
code: string, 
/**
 * Detail
 */
detail: string, } } | { "Invariant": InvariantViolation } | { "CmsTooLarge": { 
/**
 * benötigt
 */
needed: number, 
/**
 * vorhanden
 */
reserve: number, } } | { "Config": string } | { "Save": string } | { "Prepare": { 
/**
 * Code (`sigblock-…`, `placement.…`, `ng-certificate-not-valid`, `error-PrepareDocument`)
 */
code: string, 
/**
 * Detail
 */
detail: string, } } | { "Unexpected": string };

/**
 * Ergebnis eines abgeschlossenen Dokuments.
 */
export type DocResult = { "kind": "signed", 
/**
 * Ausgabe
 */
output: OutputInfo, } | { "kind": "cancelled" } | { "kind": "failed", 
/**
 * Fehler (Code und Parameter für i18n)
 */
error: WorkflowError, };

/**
 * Kennung eines Backends.
 */
export type BackendId = "atrust_native" | "atrust_browser" | "csc" | "local_bku" | "keystore" | "wallet";

/**
 * Spiegel von `pdfover_cades::DigestAlg`.
 */
export type DigestAlg = "Sha1" | "Sha256" | "Sha384" | "Sha512";

/**
 * Was das Backend signiert.
 */
export type SignInput = "Document" | { "Digest": { 
/**
 * unterstützte Hashverfahren
 */
algs: Array<DigestAlg>, } };

/**
 * Was das Backend liefert.
 */
export type SignOutput = "Cms" | "RawSignature";

/**
 * Wann ein Zertifikat vorab benötigt wird.
 */
export type CertNeed = "Required" | "OnlyForVisibleBlock" | "No";

/**
 * Art der Interaktion.
 */
export type Interaction = "None" | "Secret" | "NativeForms" | "EmbeddedWeb" | "SystemBrowser" | "External" | "ExternalApp";

/**
 * Benötigter Server.
 */
export type ServerNeed = "None" | "DataUrlRelay" | "TokenProxy";

/**
 * Qualifikation der Signatur (Anzeige).
 */
export type Qualification = "Qes" | "AdES" | "Unknown";

/**
 * Faktoren (nur Anzeige).
 */
export type FactorSet = { 
/**
 * ID-Austria-App
 */
app: boolean, 
/**
 * SMS-TAN
 */
sms: boolean, 
/**
 * QR-Code
 */
qr: boolean, 
/**
 * FIDO2
 */
fido2: boolean, 
/**
 * Signaturkarte
 */
card: boolean, };

/**
 * Fähigkeiten eines Backends in einer Umgebung.
 */
export type Capabilities = { 
/**
 * Eingabe
 */
input: SignInput, 
/**
 * Ausgabe
 */
output: SignOutput, 
/**
 * Zertifikat vorab
 */
certificate: CertNeed, 
/**
 * Interaktion
 */
interaction: Interaction, 
/**
 * Server
 */
server: ServerNeed, 
/**
 * Höchstzahl Dokumente je Freigabe (1 = je Dokument eine Freigabe)
 */
max_batch: number, 
/**
 * Eine Anmeldung für den ganzen Batch (Szenario A wie PDF-Over)
 */
login_once_per_batch: boolean, 
/**
 * Dokument verlässt das Gerät → Pflicht-Datenschutzhinweis vor dem ersten Upload
 */
document_leaves_device: boolean, 
/**
 * Qualifikation
 */
qualification: Qualification, 
/**
 * Faktoren
 */
factors: FactorSet, 
/**
 * Empfohlene CMS-Reserve in Bytes (Modell B: 16 KiB, Plan §6.2)
 */
cms_reserve: number | null, };

/**
 * Validierungs- bzw. Fehlerhinweis in einer Maske (i18n-Keys aus PDF-Over).
 */
export type FormProblem = "PasswordTooShort" | "PasswordTooLong" | "NoTan" | "TanTooLong" | "EnteredReferenceValue" | "QrImageFailed" | { "Fido2Failed": string } | "LoginRejected" | { "Atrust": string } | { "Fido2Error": { 
/**
 * stabiler Code (`fido2-…`)
 */
code: string, 
/**
 * Diagnosetext
 */
detail: string, } };

/**
 * Gemeinsamer Kontext der A-Trust-Masken.
 */
export type FormContext = { 
/**
 * Link „Signaturdaten anzeigen“ (`ShowSigobj.aspx`), nur wenn A-Trust ihn liefert (F-68).
 */
signature_data_url: string | null, 
/**
 * „SMS anfordern“ möglich (`#SmsButton`).
 */
can_switch_to_sms: boolean, 
/**
 * „FIDO2“ angeboten (`#FidoButton`).
 */
can_switch_to_fido2: boolean, 
/**
 * FIDO2 auf dieser Plattform verfügbar (Button sichtbar).
 */
fido2_visible: boolean, 
/**
 * Meldungszeile (Disclaimer `mobileBKU.aTrustDisclaimer` bzw. Fehler).
 */
problem: FormProblem | null, };

/**
 * QR-Bild (per HTTP von A-Trust geladen, R8).
 */
export type QrImage = { 
/**
 * Bildbytes (typisch PNG)
 */
bytes: Array<number>, 
/**
 * MIME-Typ, falls bekannt
 */
mime: string | null, };

/**
 * Native Maske (Szenario A).
 */
export type NativeForm = { "Login": { 
/**
 * Vorbelegung (gemerkt bzw. `-n`; in der UI nicht normalisiert)
 */
prefill_username: string | null, 
/**
 * Zustand „Merken“ (`MOBILE_REMEMBER_PASSWORD`)
 */
remember: boolean, 
/**
 * Kontext
 */
ctx: FormContext, } } | { "SmsTan": { 
/**
 * `#vergleichswert`
 */
reference_value: string, 
/**
 * Kontext
 */
ctx: FormContext, } } | { "QrCode": { 
/**
 * Vergleichswert
 */
reference_value: string, 
/**
 * Bild (fehlt, wenn das Laden scheiterte)
 */
image: QrImage | null, 
/**
 * Restzeit des Long-Polls in Sekunden beim Anzeigen (Countdown, Plan §4.4; Default 300 s
 * aus `PollConfig`); `None` = unbekannt. Die UI zählt selbst herunter.
 */
timeout_secs: number | null, 
/**
 * Kontext
 */
ctx: FormContext, } } | { "WaitForApp": { 
/**
 * Vergleichswert
 */
reference_value: string | null, 
/**
 * Restzeit des Long-Polls in Sekunden (Countdown, Plan §4.4); `None` = unbekannt.
 */
timeout_secs: number | null, 
/**
 * Kontext
 */
ctx: FormContext, } } | { "Fido2": { 
/**
 * Vorgang läuft (`common.working`, Button deaktiviert)
 */
in_progress: boolean, 
/**
 * Kontext
 */
ctx: FormContext, } };

/**
 * Art der Passwortabfrage (Keystore, Proxy).
 */
export type SecretKind = "KeystorePassword" | "KeyPassword" | "ProxyPassword" | "Fido2Pin";

/**
 * Passwortabfrage.
 */
export type SecretPrompt = { 
/**
 * Art
 */
kind: SecretKind, 
/**
 * Bezug (Keystore-Dateiname, Alias, Proxy-Host) – ohne Pfad zu Nutzerdaten in Logs
 */
subject: string, 
/**
 * Vorheriger Versuch war falsch (`error.KeyStoreStorePass` …)
 */
retry: boolean, };

/**
 * Art einer Meldung.
 */
export type NoticeKind = "Info" | "Warning" | "Error";

/**
 * Knöpfe einer Meldung.
 */
export type NoticeButtons = "Ok" | "OkCancel" | "RetryCancel";

/**
 * Inhalt einer Meldung (i18n über den Code; Texte von A-Trust roh in [`Notice::text`]).
 */
export type NoticeCode = "AtrustInterstitial" | "AtrustError" | "ProxyAuthRequired" | "DocumentTooLarge" | { "Http": { 
/**
 * Status
 */
status: number, } } | "ConnectionFailed" | "LocalBkuNotReachable" | "KeystoreFileMissing" | "KeystoreAliasMissing" | "KeystoreError" | "KeystoreConversionNeeded";

/**
 * Meldung.
 */
export type Notice = { 
/**
 * Art
 */
kind: NoticeKind, 
/**
 * Code
 */
code: NoticeCode, 
/**
 * Freitext (A-Trust-Meldung, HTTP-Reason)
 */
text: string | null, 
/**
 * Knöpfe
 */
buttons: NoticeButtons, };

/**
 * Antwort auf eine Meldung.
 */
export type NoticeAnswer = "Ok" | "Retry" | "Cancel";

/**
 * Fortschritt (Warte-Screen, Live-Region).
 */
export type Progress = "Connecting" | "ReadingCertificate" | "WaitingForServer" | "Signing" | { "Document": { 
/**
 * 1-basiert
 */
index: number, 
/**
 * gesamt
 */
total: number, } } | "Done";

/**
 * Spiegel von `pdfover_placement::Placement` (PAGE_ABS, linke obere Ecke, y = Oberkante von unten, pt).
 */
export type Placement = { 
/**
 * 0-basierter Seitenindex; `== Seitenzahl` = virtuelle neue Seite
 */
page_index: number, 
/**
 * linke Kante in pt
 */
x: number, 
/**
 * Oberkante in pt (von unten)
 */
y: number, 
/**
 * Breite in pt
 */
width: number, 
/**
 * Höhe in pt
 */
height: number, };

/**
 * Spiegel von `pdfover_config::SignatureProfile`.
 */
export type SignatureProfile = "SIGNATURBLOCK_SMALL" | "AMTSSIGNATURBLOCK" | "BASE_LOGO" | "INVISIBLE";

/**
 * Spiegel von `pdfover_config::NoteSetting`.
 */
export type NoteSetting = { "mode": "default" } | { "mode": "none" } | { "mode": "custom", "text": string };

/**
 * Spiegel von `pdfover_config::WindowSize`.
 */
export type WindowSize = { 
/**
 * Breite
 */
width: number, 
/**
 * Höhe
 */
height: number, };

/**
 * Spiegel von `pdfover_config::GeneralSettings`.
 */
export type GeneralSettings = { 
/**
 * UI-Sprache (`None` = System)
 */
ui_language: Lang | null, 
/**
 * Update-Prüfung
 */
update_check: boolean, 
/**
 * Fenstergröße
 */
window_size: WindowSize | null, 
/**
 * letzter Ordner (Dateidialog, F-11)
 */
last_source_directory: string | null, };

/**
 * Spiegel von `pdfover_config::CmsProfileChoice`.
 */
export type CmsProfileChoice = "pdfas_compat" | "pades_bb";

/**
 * Spiegel von `pdfover_config::SignatureSettings`.
 */
export type SignatureSettings = { 
/**
 * Profil
 */
profile: SignatureProfile, 
/**
 * Sprache des Signaturblocks (`None` = System)
 */
block_language: Lang | null, 
/**
 * Hinweis
 */
note: NoteSetting, 
/**
 * PDF/A erzwingen
 */
pdfa_force: boolean, 
/**
 * automatische Positionierung
 */
auto_position: boolean, 
/**
 * Bildmarke
 */
emblem: string | null, 
/**
 * Größe „nur Bildmarke“ in mm
 */
logo_only_size_mm: number, 
/**
 * CMS-Profil
 */
cms_profile: CmsProfileChoice, };

/**
 * Spiegel von `pdfover_config::PlaceholderKind`.
 */
export type PlaceholderKind = "fields" | "marker";

/**
 * Spiegel von `pdfover_config::PlaceholderSettings`.
 */
export type PlaceholderSettings = { 
/**
 * Suche aktiv
 */
search: boolean, 
/**
 * Art
 */
kind: PlaceholderKind, };

/**
 * Spiegel von `pdfover_config::BackendChoice`.
 */
export type BackendChoice = "id_austria" | "local_bku" | "keystore";

/**
 * Spiegel von `pdfover_config::BackendSettings`.
 */
export type BackendSettings = { 
/**
 * Standard-Signiermittel
 */
default: BackendChoice | null, };

/**
 * Spiegel von `pdfover_config::IdAustriaSettings` (das Passwort wird nie persistiert).
 */
export type IdAustriaSettings = { 
/**
 * Benutzername/Telefonnummer
 */
username: string | null, 
/**
 * „Merken“
 */
remember: boolean, 
/**
 * FIDO2 als Standard
 */
fido2_default: boolean, };

/**
 * Spiegel von `pdfover_config::PasswordStorage`.
 */
export type PasswordStorage = "none" | "memory" | "keychain";

/**
 * Spiegel von `pdfover_config::KeystoreSettings`.
 */
export type KeystoreSettings = { 
/**
 * aktiviert
 */
enabled: boolean, 
/**
 * Datei
 */
file: string | null, 
/**
 * angegebener Typ
 */
declared_type: string | null, 
/**
 * Alias
 */
alias: string | null, 
/**
 * Passwort-Speicherart
 */
password_storage: PasswordStorage, };

/**
 * Spiegel von `pdfover_config::OutputSettings`.
 */
export type OutputSettings = { 
/**
 * Ausgabeordner
 */
folder: string | null, 
/**
 * Dateinamenzusatz
 */
postfix: string, };

/**
 * Spiegel von `pdfover_config::ProxyMode`.
 */
export type ProxyMode = "system" | "none" | "manual";

/**
 * Spiegel von `pdfover_config::ProxySettings`.
 */
export type ProxySettings = { 
/**
 * Modus
 */
mode: ProxyMode, 
/**
 * Host
 */
host: string | null, 
/**
 * Port
 */
port: number | null, 
/**
 * Benutzer
 */
user: string | null, };

/**
 * Spiegel von `pdfover_config::PrivacySettings`.
 */
export type PrivacySettings = { 
/**
 * bestätigte Textversion je Backend
 */
acknowledged: { [key in string]: number }, };

/**
 * Spiegel von `pdfover_config::CertificateCacheSettings`.
 */
export type CertificateCacheSettings = { 
/**
 * Opt-in
 */
enabled: boolean, };

/**
 * Spiegel von `pdfover_config::Settings`. Unbekannte Einträge (`Settings::unknown`, `flatten`) stehen
 * auf oberster Ebene und gehen beim Speichern unverändert zurück.
 */
export type Settings = { 
/**
 * Schema-Version
 */
schema_version: number, 
/**
 * Allgemein
 */
general: GeneralSettings, 
/**
 * Signatur
 */
signature: SignatureSettings, 
/**
 * Platzhalter
 */
placeholder: PlaceholderSettings, 
/**
 * Signiermittel
 */
backend: BackendSettings, 
/**
 * ID Austria
 */
id_austria: IdAustriaSettings, 
/**
 * Keystore
 */
keystore: KeystoreSettings, 
/**
 * Ausgabe
 */
output: OutputSettings, 
/**
 * Proxy
 */
proxy: ProxySettings, 
/**
 * Datenschutzhinweise
 */
privacy: PrivacySettings, 
/**
 * Zertifikats-Cache
 */
certificate_cache: CertificateCacheSettings, } & ({ [key in string]: unknown });

/**
 * Hinweis ohne Rückfrage (i18n-ID mit Parametern), z. B. `error-PositioningNotPossible`,
 * `placement.notice.*`, `ng-config-reset-done`. Die UI schließt ihn mit [`UiEvent::DismissNotice`].
 */
export type NoticeView = { 
/**
 * Kennung
 */
id: number, 
/**
 * Meldungs-ID
 */
code: string, 
/**
 * Parameter
 */
params: { [key in string]: string }, };

/**
 * Speichern-Dialog (F-83/F-86): Startordner und Namensvorschlag.
 */
export type SaveDialogView = { 
/**
 * Startordner
 */
directory: string, 
/**
 * vorgeschlagener Dateiname
 */
file_name: string, };

/**
 * Angebot, die Einstellungen aus `~/.pdf-over` zu übernehmen (Plan §7.2, einmalig).
 */
export type MigrationOfferView = { 
/**
 * gefundene `PDF-Over.config`
 */
legacy_file: string, 
/**
 * die Altdatei enthält Klartext-Passwörter (Keystore/Proxy): Zustimmung zum Übernehmen in den
 * Schlüsselbund erfragen (`UiEvent::AcceptMigration::store_secrets`)
 */
plaintext_secrets: boolean, };

/**
 * Abgeschlossenes Dokument (Warteschlangen-Panel, CLI-Ergebnis).
 */
export type DoneItemView = { 
/**
 * Kennung
 */
id: DocId, 
/**
 * Name
 */
name: string, 
/**
 * Eingabepfad
 */
input: string, 
/**
 * Ergebnis
 */
result: DocResult, };

/**
 * Dokument (Anzeige).
 */
export type DocInfo = { 
/**
 * Kennung
 */
id: DocId, 
/**
 * Name
 */
name: string, 
/**
 * Seitenzahl
 */
page_count: number, 
/**
 * sichtbare Seitengrößen in pt
 */
page_sizes: Array<[number, number]>, 
/**
 * bereits signiert
 */
signed: boolean, };

/**
 * Eintrag der Warteschlange.
 */
export type QueueItemView = { 
/**
 * Kennung
 */
id: DocId, 
/**
 * Name
 */
name: string, 
/**
 * aktuell
 */
current: boolean, };

/**
 * Leeres Feld (Auswahlliste).
 */
export type FieldInfo = { 
/**
 * qualifizierter Name
 */
name: string, 
/**
 * Seite
 */
page: number | null, };

/**
 * QR-Platzhalter.
 */
export type QrInfo = { 
/**
 * Seite
 */
page: number, 
/**
 * id
 */
id: string | null, };

/**
 * Blockgröße und Vorschau.
 */
export type BlockView = { 
/**
 * Breite in pt
 */
width: number, 
/**
 * Höhe in pt
 */
height: number, 
/**
 * Vorschau-Revision (Cache-Busting für `blockPreviewUrl`)
 */
preview_revision: number, };

/**
 * Backend-Option.
 */
export type BackendOptionView = { 
/**
 * Backend
 */
id: BackendId, 
/**
 * verfügbar
 */
available: boolean, 
/**
 * Fähigkeiten
 */
capabilities: Capabilities | null, 
/**
 * Hinweise zur Option (i18n-IDs), z. B. `ng-fido2-unavailable-mobile`: ID Austria auf Mobile ohne
 * FIDO2 – die UI zeigt FIDO2 ausgegraut mit diesem Hinweis (Plan §4.7, §5.2)
 */
notes: Array<string>, };

/**
 * Betriebssystem der Shell.
 */
export type PlatformOs = "windows" | "macos" | "linux" | "ios" | "android";

/**
 * Plattform der Oberfläche (Plan §5.1): steuert plattformabhängige Bedienelemente.
 */
export type PlatformView = { 
/**
 * Betriebssystem
 */
os: PlatformOs, 
/**
 * Mobile Oberfläche (iOS/Android, Plan §5.2): Teilen und Exportieren statt „Im Ordner anzeigen“,
 * PKCS#12-Import in die App statt Dateipfad, kein FIDO2 (ausgegraut mit Hinweis), keine lokale
 * BKU, keine Proxy-Einstellungen (System-Proxy), keine Ausgabeordner-Wahl (App-Dokumente)
 */
mobile: boolean, };

/**
 * Angebot „Signatur fortsetzen“ (Mobile, Plan §3.5): Beim letzten Lauf wurde die App während eines
 * Signiervorgangs vom System beendet (z. B. beim Wechsel in die App „ID Austria“). Antwort über
 * [`UiEvent::ResumeSignature`] bzw. [`UiEvent::DiscardResume`]; nur im Schritt `open` gesetzt.
 */
export type ResumeOfferView = { 
/**
 * Name des Dokuments
 */
name: string, 
/**
 * Signiermittel des unterbrochenen Vorgangs
 */
backend: BackendId, 
/**
 * vor wie vielen Minuten vorbereitet (Anzeige)
 */
age_minutes: number, 
/**
 * weitere Dokumente des unterbrochenen Batches
 */
pending: number, };

/**
 * Ergebnis des PKCS#12-Imports in die App-Sandbox (Mobile, Import-Assistent; Plan §5.2) – schließen
 * mit [`UiEvent::DismissKeystoreImport`].
 */
export type KeystoreImportView = { 
/**
 * Dateiname (ohne Pfad)
 */
file_name: string, 
/**
 * importiert (Einstellungen › Keystore zeigen auf die Kopie in der Sandbox)
 */
ok: boolean, 
/**
 * Meldungs-ID: `ng-keystore-imported` bzw. der Fehler (`keystore-wrong-password`,
 * `ng-keystore-import-format`, …)
 */
code: string, 
/**
 * Passwort im sicheren Speicher abgelegt (Keychain bzw. Android Keystore)
 */
password_stored: boolean, };

/**
 * Datenschutzhinweis.
 */
export type PrivacyNoticeView = { 
/**
 * Backend
 */
backend: BackendId, 
/**
 * Textversion
 */
version: number, 
/**
 * eigener Text per Richtlinie
 */
custom_text: string | null, };

/**
 * Interaktion während des Signierens.
 */
export type InteractionView = { "kind": "waiting", 
/**
 * Fortschritt
 */
progress: Progress, } | { "kind": "form", 
/**
 * Anfrage-ID (Antwort über [`UiEvent::SubmitForm`])
 */
request: number, 
/**
 * Maske
 */
form: NativeForm, } | { "kind": "secret", 
/**
 * Anfrage-ID
 */
request: number, 
/**
 * Abfrage
 */
prompt: SecretPrompt, } | { "kind": "notice", 
/**
 * Anfrage-ID
 */
request: number, 
/**
 * Meldung
 */
notice: Notice, };

/**
 * Ergebnis.
 */
export type OutputView = { 
/**
 * gespeichert unter
 */
path: string | null, 
/**
 * Speichern fehlgeschlagen/abgebrochen (Knopf „Speichern“, F-86)
 */
save_pending: boolean, 
/**
 * Rückfrage „Datei existiert, überschreiben?“ (F-84) – Antwort über [`UiEvent::ConfirmOverwrite`]
 */
ask_overwrite: string | null, 
/**
 * Speichern-Dialog öffnen (F-83/F-86) – Antwort über [`UiEvent::Save`]
 */
save_dialog: SaveDialogView | null, };

/**
 * Hauptleiste.
 */
export type MainBarView = { 
/**
 * Schritte
 */
steps: Array<MainBarStep>, 
/**
 * Einstellungen erreichbar
 */
settings_enabled: boolean, };

/**
 * Schritt-spezifische Sicht.
 */
export type StepView = { "step": "starting" } | { "step": "open", 
/**
 * Warteschlange
 */
queue: Array<QueueItemView>, } | { "step": "loading", 
/**
 * Warteschlange
 */
queue: Array<QueueItemView>, } | { "step": "warning", 
/**
 * Dokument
 */
doc: DocInfo, 
/**
 * Meldungs-IDs der Warnungen (`pades::Finding::code`)
 */
codes: Array<string>, } | { "step": "placeholder", 
/**
 * Dokument
 */
doc: DocInfo, 
/**
 * leere Felder
 */
fields: Array<FieldInfo>, 
/**
 * QR-Platzhalter
 */
qr: QrInfo | null, } | { "step": "position", 
/**
 * Dokument
 */
doc: DocInfo, 
/**
 * Seite
 */
page: number, 
/**
 * Block
 */
block: BlockView, 
/**
 * Position (PAGE_ABS, linke obere Ecke)
 */
placement: Placement, 
/**
 * virtuelle neue Seite
 */
new_page: boolean, 
/**
 * Parameter des Live-Region-Texts `ng-ui-position-announce`: `page`, `pages`, `left` (mm der
 * linken Kante von links), `top` (mm der Oberkante von unten – wie das Zahlenfeld Y), ganzzahlig
 */
announce: { [key in string]: string }, } | { "step": "backend", 
/**
 * Optionen
 */
options: Array<BackendOptionView>, 
/**
 * Datenschutzhinweis
 */
privacy_notice: PrivacyNoticeView | null, } | { "step": "signing", 
/**
 * Interaktion
 */
interaction: InteractionView, 
/**
 * Warteschlange
 */
queue: Array<QueueItemView>, } | { "step": "output", 
/**
 * Ergebnis
 */
result: OutputView, 
/**
 * nächstes Dokument
 */
next: DocInfo | null, } | { "step": "error", 
/**
 * Fehlercode (i18n-ID)
 */
code: string, 
/**
 * Parameter
 */
params: { [key in string]: string }, 
/**
 * Wiederholen möglich
 */
retry: boolean, } | { "step": "finished", 
/**
 * Exit-Code
 */
exit_code: number, };

/**
 * Einstellungen (Overlay über jedem Schritt).
 */
export type SettingsView = { 
/**
 * Einstellungen (Schema `pdfover-config`)
 */
settings: Settings, 
/**
 * Validierungsprobleme je Feld: Schlüssel = TOML-Punktpfad des Felds (`proxy.host`,
 * `output.folder`, `keystore.file` …), Wert = i18n-ID
 */
problems: { [key in string]: string }, 
/**
 * Schlüsselbund verfügbar
 */
keychain_available: boolean, 
/**
 * FIDO2 verfügbar
 */
fido2_available: boolean, 
/**
 * Einstellungen werden nicht gespeichert (Altdatei per `-c`, unlesbare Datei)
 */
read_only: boolean, 
/**
 * Revision der Blockvorschau (Cache-Busting für `blockPreviewUrl` im Einstellungsdialog);
 * der Kern baut die Vorschau beim Öffnen auch ohne Dokument (A4, Beispielzertifikat) wie PDF-Over
 */
preview_revision: number, };

/**
 * Gesamtzustand für die UI.
 */
export type ViewState = { 
/**
 * Schritt
 */
step: StepView, 
/**
 * Hauptleiste
 */
main_bar: MainBarView, 
/**
 * Einstellungen, falls geöffnet
 */
settings: SettingsView | null, 
/**
 * UI-Sprache
 */
language: Lang, 
/**
 * Hinweise ohne Rückfrage (älteste zuerst)
 */
notices: Array<NoticeView>, 
/**
 * Import-Angebot aus `~/.pdf-over`
 */
migration: MigrationOfferView | null, 
/**
 * abgeschlossene Dokumente dieser Sitzung
 */
done: Array<DoneItemView>, 
/**
 * Plattform (Desktop/Mobile-Bedienelemente)
 */
platform: PlatformView, 
/**
 * Angebot „Signatur fortsetzen“ (nur Mobile, nur im Schritt `open`)
 */
resume: ResumeOfferView | null, 
/**
 * Ergebnis des PKCS#12-Imports (Import-Assistent)
 */
keystore_import: KeystoreImportView | null, };

/**
 * Antwort auf eine Maske (serde-Variante von `FormAnswer`; Passwort wird sofort in `Zeroizing`
 * überführt und nie geloggt).
 */
export type FormAnswerView = { "kind": "login", 
/**
 * Benutzername
 */
username: string, 
/**
 * Passwort
 */
password: string, 
/**
 * Merken
 */
remember: boolean, } | { "kind": "tan", 
/**
 * TAN
 */
tan: string, } | { "kind": "switch_to_sms" } | { "kind": "switch_to_fido2" } | { "kind": "start_fido2" } | { "kind": "continue" };

/**
 * UI-Ereignis.
 */
export type UiEvent = { "type": "open_files", 
/**
 * Pfade
 */
paths: Array<string>, } | { "type": "answer_warning", 
/**
 * `true` = fortfahren
 */
proceed: boolean, } | { "type": "placeholder_answer", 
/**
 * Antwort
 */
answer: PlaceholderAnswer, 
/**
 * gewähltes Feld
 */
field_index: number | null, } | { "type": "move_block", 
/**
 * Seite
 */
page: number, 
/**
 * x
 */
x: number, 
/**
 * y
 */
y: number, } | { "type": "nudge_block", 
/**
 * dx in Schritten
 */
dx: number, 
/**
 * dy in Schritten
 */
dy: number, 
/**
 * fein (Shift)
 */
fine: boolean, 
/**
 * grob (Strg/⌘)
 */
coarse: boolean, } | { "type": "change_page", 
/**
 * Seite
 */
page: number, } | { "type": "toggle_new_page" } | { "type": "auto_place" } | { "type": "confirm_position" } | { "type": "choose_invisible" } | { "type": "choose_backend", 
/**
 * Backend
 */
backend: BackendId, } | { "type": "acknowledge_privacy", 
/**
 * angenommen
 */
accepted: boolean, } | { "type": "submit_form", 
/**
 * Anfrage-ID
 */
request: number, 
/**
 * Antwort
 */
answer: FormAnswerView, } | { "type": "submit_secret", 
/**
 * Anfrage-ID
 */
request: number, 
/**
 * Passwort
 */
secret: string, } | { "type": "answer_notice", 
/**
 * Anfrage-ID
 */
request: number, 
/**
 * Antwort
 */
answer: NoticeAnswer, } | { "type": "open_signature_data", 
/**
 * Anfrage-ID der offenen Maske
 */
request: number, } | { "type": "cancel" } | { "type": "retry" } | { "type": "save", 
/**
 * Ziel (`None` = Dialog abgebrochen)
 */
path: string | null, } | { "type": "confirm_overwrite", 
/**
 * ja
 */
overwrite: boolean, } | { "type": "open_result" } | { "type": "share_result" } | { "type": "export_result" } | { "type": "navigate", 
/**
 * Schritt
 */
step: Step, } | { "type": "open_settings" } | { "type": "save_settings", 
/**
 * neue Einstellungen
 */
settings: Settings, } | { "type": "close_settings" } | { "type": "reset_settings" } | { "type": "backgrounded" } | { "type": "foregrounded" } | { "type": "resume_signature" } | { "type": "discard_resume" } | { "type": "import_keystore", 
/**
 * Datei (von der Shell bereits in die Sandbox kopiert, `import_file`)
 */
path: string, 
/**
 * Passwort (nur RAM, sofort in `Zeroizing`; nie geloggt)
 */
password: string | null, 
/**
 * Passwort im sicheren Speicher merken
 */
remember: boolean, } | { "type": "dismiss_keystore_import" } | { "type": "quit" } | { "type": "dismiss_notice", 
/**
 * Kennung ([`NoticeView::id`])
 */
id: number, } | { "type": "accept_migration", 
/**
 * Klartext-Passwörter in den Schlüsselbund übernehmen (Zustimmung)
 */
store_secrets: boolean, } | { "type": "decline_migration" };
