/**
 * TypeScript-Abbild der Bridge-Typen von `pdfover-core-api` (crates/core-api/src/view.rs) im
 * **serde-JSON-Format** – exakt, ohne Ergänzungen. Vorschläge für additive Erweiterungen stehen
 * getrennt in `extensions.ts`.
 *
 * Abbildungsregeln (serde_json):
 * - `Option<T>` → `T | null` (beim Senden immer angeben), `PathBuf` → `string`, `Vec<u8>` → `number[]`,
 *   `(f32, f32)` → `[number, number]`, `BTreeMap<String, String>` → `Record<string, string>`,
 *   Zahlen (`u8`…`u64`, `usize`, `f32`, `f64`) → `number`, Newtype `DocId(u64)` → `number`.
 * - Enums **ohne** serde-Attribut sind extern getaggt: Unit-Varianten als String (`"Connecting"`),
 *   Daten-Varianten als `{ "Variante": … }`.
 * - `#[serde(tag = "…", rename_all = "snake_case")]` → intern getaggt mit snake_case-Namen.
 * - `#[serde(rename_all = …)]` auf Unit-Enums → Strings in der jeweiligen Schreibweise.
 *
 * Die Konsistenz mit Rust prüfen `src/core/contract.test.ts` (Beispiel-JSON aus serde, erzeugt vom
 * Rust-Test `apps/app/src-tauri/tests/serde_contract.rs`) und der Rust-Test selbst (TS-Beispiele →
 * `UiEvent`).
 */

// ---------------------------------------------------------------------------------------------
// pdfover-i18n, pdfover-workflow
// ---------------------------------------------------------------------------------------------

/** `pdfover_i18n::Lang` (`rename_all = "lowercase"`). */
export type Lang = 'de' | 'en';

/** `pdfover_workflow::DocId` (Newtype → Zahl). */
export type DocId = number;

/** `pdfover_workflow::Step` (extern getaggt, Unit-Varianten). */
export type Step = 'Open' | 'Positioning' | 'Signing' | 'Finish';

/** `pdfover_workflow::MainBarStep`. */
export interface MainBarStep {
  step: Step;
  /** erreicht („A“) */
  active: boolean;
  /** klickbar („E“) */
  enabled: boolean;
}

/** `pdfover_workflow::PlaceholderAnswer`. */
export type PlaceholderAnswer = 'Yes' | 'No' | 'Cancel';

// ---------------------------------------------------------------------------------------------
// pdfover-backend-api / pdfover-cades
// ---------------------------------------------------------------------------------------------

/** `pdfover_backend_api::BackendId` (`rename_all = "snake_case"`). */
export type BackendId = 'atrust_native' | 'atrust_browser' | 'csc' | 'local_bku' | 'keystore' | 'wallet';

/** `pdfover_cades::DigestAlg`. */
export type DigestAlg = 'Sha1' | 'Sha256' | 'Sha384' | 'Sha512';

/** `SignInput`. */
export type SignInput = 'Document' | { Digest: { algs: DigestAlg[] } };
/** `SignOutput`. */
export type SignOutput = 'Cms' | 'RawSignature';
/** `CertNeed`. */
export type CertNeed = 'Required' | 'OnlyForVisibleBlock' | 'No';
/** `Interaction`. */
export type Interaction = 'None' | 'Secret' | 'NativeForms' | 'EmbeddedWeb' | 'SystemBrowser' | 'External' | 'ExternalApp';
/** `ServerNeed`. */
export type ServerNeed = 'None' | 'DataUrlRelay' | 'TokenProxy';
/** `Qualification`. */
export type Qualification = 'Qes' | 'AdES' | 'Unknown';

/** `FactorSet` (nur Anzeige). */
export interface FactorSet {
  app: boolean;
  sms: boolean;
  qr: boolean;
  fido2: boolean;
  card: boolean;
}

/** `Capabilities`. */
export interface Capabilities {
  input: SignInput;
  output: SignOutput;
  certificate: CertNeed;
  interaction: Interaction;
  server: ServerNeed;
  max_batch: number;
  login_once_per_batch: boolean;
  document_leaves_device: boolean;
  qualification: Qualification;
  factors: FactorSet;
  cms_reserve: number | null;
}

/** `FormProblem`. */
export type FormProblem =
  | 'PasswordTooShort'
  | 'PasswordTooLong'
  | 'NoTan'
  | 'TanTooLong'
  | 'EnteredReferenceValue'
  | 'QrImageFailed'
  | 'LoginRejected'
  | { Fido2Failed: string }
  | { Atrust: string };

/** `FormContext`. */
export interface FormContext {
  signature_data_url: string | null;
  can_switch_to_sms: boolean;
  can_switch_to_fido2: boolean;
  fido2_visible: boolean;
  problem: FormProblem | null;
}

/** `QrImage` (`Vec<u8>` → Zahlenfeld). */
export interface QrImage {
  bytes: number[];
  mime: string | null;
}

/** `NativeForm` (extern getaggt). */
export type NativeForm =
  | { Login: { prefill_username: string | null; remember: boolean; ctx: FormContext } }
  | { SmsTan: { reference_value: string; ctx: FormContext } }
  | {
      QrCode: {
        reference_value: string;
        image: QrImage | null;
        timeout_secs: number | null;
        ctx: FormContext;
      };
    }
  | { WaitForApp: { reference_value: string | null; timeout_secs: number | null; ctx: FormContext } }
  | { Fido2: { in_progress: boolean; ctx: FormContext } };

/** `SecretKind`. */
export type SecretKind = 'KeystorePassword' | 'KeyPassword' | 'ProxyPassword';

/** `SecretPrompt`. */
export interface SecretPrompt {
  kind: SecretKind;
  subject: string;
  retry: boolean;
}

/** `NoticeKind`. */
export type NoticeKind = 'Info' | 'Warning' | 'Error';
/** `NoticeButtons`. */
export type NoticeButtons = 'Ok' | 'OkCancel' | 'RetryCancel';

/** `NoticeCode`. */
export type NoticeCode =
  | 'AtrustInterstitial'
  | 'AtrustError'
  | 'ProxyAuthRequired'
  | 'DocumentTooLarge'
  | { Http: { status: number } }
  | 'ConnectionFailed'
  | 'LocalBkuNotReachable'
  | 'KeystoreFileMissing'
  | 'KeystoreAliasMissing'
  | 'KeystoreError'
  | 'KeystoreConversionNeeded';

/** `Notice`. */
export interface Notice {
  kind: NoticeKind;
  code: NoticeCode;
  text: string | null;
  buttons: NoticeButtons;
}

/** `NoticeAnswer`. */
export type NoticeAnswer = 'Ok' | 'Retry' | 'Cancel';

/** `Progress`. */
export type Progress =
  | 'Connecting'
  | 'ReadingCertificate'
  | 'WaitingForServer'
  | 'Signing'
  | { Document: { index: number; total: number } }
  | 'Done';

// ---------------------------------------------------------------------------------------------
// pdfover-placement
// ---------------------------------------------------------------------------------------------

/** `pdfover_placement::Placement` (PAGE_ABS, linke obere Ecke, y = Oberkante von unten, pt). */
export interface Placement {
  page_index: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

// ---------------------------------------------------------------------------------------------
// pdfover-config (Settings-Schema, Plan §7.1)
// ---------------------------------------------------------------------------------------------

/** `SignatureProfile` (`SCREAMING_SNAKE_CASE`). */
export type SignatureProfile = 'SIGNATURBLOCK_SMALL' | 'AMTSSIGNATURBLOCK' | 'BASE_LOGO' | 'INVISIBLE';

/** `NoteSetting` (`tag = "mode", content = "text"`). */
export type NoteSetting = { mode: 'default' } | { mode: 'none' } | { mode: 'custom'; text: string };

/** `WindowSize`. */
export interface WindowSize {
  width: number;
  height: number;
}

/** `GeneralSettings`. */
export interface GeneralSettings {
  ui_language: Lang | null;
  update_check: boolean;
  window_size: WindowSize | null;
  last_source_directory: string | null;
}

/** `CmsProfileChoice`. */
export type CmsProfileChoice = 'pdfas_compat' | 'pades_bb';

/** `SignatureSettings`. */
export interface SignatureSettings {
  profile: SignatureProfile;
  block_language: Lang | null;
  note: NoteSetting;
  pdfa_force: boolean;
  auto_position: boolean;
  emblem: string | null;
  logo_only_size_mm: number;
  cms_profile: CmsProfileChoice;
}

/** `PlaceholderKind`. */
export type PlaceholderKind = 'fields' | 'marker';

/** `PlaceholderSettings`. */
export interface PlaceholderSettings {
  search: boolean;
  kind: PlaceholderKind;
}

/** `BackendChoice`. */
export type BackendChoice = 'id_austria' | 'local_bku' | 'keystore';

/** `BackendSettings`. */
export interface BackendSettings {
  default: BackendChoice | null;
}

/** `IdAustriaSettings` (das Passwort wird nie persistiert). */
export interface IdAustriaSettings {
  username: string | null;
  remember: boolean;
  fido2_default: boolean;
}

/** `PasswordStorage`. */
export type PasswordStorage = 'none' | 'memory' | 'keychain';

/** `KeystoreSettings`. */
export interface KeystoreSettings {
  enabled: boolean;
  file: string | null;
  declared_type: string | null;
  alias: string | null;
  password_storage: PasswordStorage;
}

/** `OutputSettings`. */
export interface OutputSettings {
  folder: string | null;
  postfix: string;
}

/** `ProxyMode`. */
export type ProxyMode = 'system' | 'none' | 'manual';

/** `ProxySettings`. */
export interface ProxySettings {
  mode: ProxyMode;
  host: string | null;
  port: number | null;
  user: string | null;
}

/** `PrivacySettings`. */
export interface PrivacySettings {
  acknowledged: Record<string, number>;
}

/** `CertificateCacheSettings`. */
export interface CertificateCacheSettings {
  enabled: boolean;
}

/**
 * `pdfover_config::Settings`. `Settings::unknown` ist `#[serde(flatten)]`: unbekannte Einträge stehen
 * **auf oberster Ebene** (Schlüssel = TOML-Pfad wie `signature.future_key`) und müssen beim Speichern
 * unverändert zurückgehen – deshalb nur per Objekt-Spread ändern (`withSettings`).
 */
export interface Settings {
  schema_version: number;
  general: GeneralSettings;
  signature: SignatureSettings;
  placeholder: PlaceholderSettings;
  backend: BackendSettings;
  id_austria: IdAustriaSettings;
  keystore: KeystoreSettings;
  output: OutputSettings;
  proxy: ProxySettings;
  privacy: PrivacySettings;
  certificate_cache: CertificateCacheSettings;
  /** unbekannte Einträge (`Settings::unknown`, flatten) */
  [unknown: string]: unknown;
}

// ---------------------------------------------------------------------------------------------
// pdfover-core-api (view.rs)
// ---------------------------------------------------------------------------------------------

/** `DocInfo`. */
export interface DocInfo {
  id: DocId;
  name: string;
  page_count: number;
  /** sichtbare Seitengrößen in pt (Breite, Höhe) */
  page_sizes: [number, number][];
  signed: boolean;
}

/** `QueueItemView`. */
export interface QueueItemView {
  id: DocId;
  name: string;
  current: boolean;
}

/** `FieldInfo`. */
export interface FieldInfo {
  name: string;
  page: number | null;
}

/** `QrInfo`. */
export interface QrInfo {
  page: number;
  id: string | null;
}

/** `BlockView`. */
export interface BlockView {
  width: number;
  height: number;
  preview_revision: number;
}

/** `BackendOptionView`. */
export interface BackendOptionView {
  id: BackendId;
  available: boolean;
  capabilities: Capabilities | null;
}

/** `PrivacyNoticeView`. */
export interface PrivacyNoticeView {
  backend: BackendId;
  version: number;
  custom_text: string | null;
}

/** `InteractionView` (`tag = "kind"`). */
export type InteractionView =
  | { kind: 'waiting'; progress: Progress }
  | { kind: 'form'; request: number; form: NativeForm }
  | { kind: 'secret'; request: number; prompt: SecretPrompt }
  | { kind: 'notice'; request: number; notice: Notice };

/** `OutputView`. */
export interface OutputView {
  path: string | null;
  save_pending: boolean;
}

/** `MainBarView`. */
export interface MainBarView {
  steps: MainBarStep[];
  settings_enabled: boolean;
}

/** `StepView` (`tag = "step"`). */
export type StepView =
  | { step: 'starting' }
  | { step: 'open'; queue: QueueItemView[] }
  | { step: 'placeholder'; doc: DocInfo; fields: FieldInfo[]; qr: QrInfo | null }
  | {
      step: 'position';
      doc: DocInfo;
      page: number;
      block: BlockView;
      placement: Placement;
      new_page: boolean;
      announce: Record<string, string>;
    }
  | { step: 'backend'; options: BackendOptionView[]; privacy_notice: PrivacyNoticeView | null }
  | { step: 'signing'; interaction: InteractionView; queue: QueueItemView[] }
  | { step: 'output'; result: OutputView; next: DocInfo | null }
  | { step: 'error'; code: string; params: Record<string, string>; retry: boolean }
  | { step: 'finished'; exit_code: number };

/** Name eines Schritts (`StepView['step']`). */
export type StepName = StepView['step'];

/** `SettingsView`. */
export interface SettingsView {
  settings: Settings;
  /** Validierungsprobleme je Feld (Schlüssel = Feldpfad, Wert = i18n-ID) */
  problems: Record<string, string>;
  keychain_available: boolean;
  fido2_available: boolean;
}

/** `ViewState`. */
export interface ViewState {
  step: StepView;
  main_bar: MainBarView;
  settings: SettingsView | null;
  language: Lang;
}

/** `FormAnswerView` (`tag = "kind"`; das Passwort geht nie in Logs). */
export type FormAnswerView =
  | { kind: 'login'; username: string; password: string; remember: boolean }
  | { kind: 'tan'; tan: string }
  | { kind: 'switch_to_sms' }
  | { kind: 'switch_to_fido2' }
  | { kind: 'start_fido2' }
  | { kind: 'continue' };

/** `UiEvent` (`tag = "type"`). */
export type UiEvent =
  | { type: 'open_files'; paths: string[] }
  | { type: 'placeholder_answer'; answer: PlaceholderAnswer; field_index: number | null }
  | { type: 'move_block'; page: number; x: number; y: number }
  | { type: 'nudge_block'; dx: number; dy: number; fine: boolean; coarse: boolean }
  | { type: 'change_page'; page: number }
  | { type: 'toggle_new_page' }
  | { type: 'auto_place' }
  | { type: 'confirm_position' }
  | { type: 'choose_backend'; backend: BackendId }
  | { type: 'acknowledge_privacy'; accepted: boolean }
  | { type: 'submit_form'; request: number; answer: FormAnswerView }
  | { type: 'submit_secret'; request: number; secret: string }
  | { type: 'answer_notice'; request: number; answer: NoticeAnswer }
  | { type: 'open_signature_data'; request: number }
  | { type: 'cancel' }
  | { type: 'retry' }
  | { type: 'save'; path: string | null }
  | { type: 'confirm_overwrite'; overwrite: boolean }
  | { type: 'open_result' }
  | { type: 'navigate'; step: Step }
  | { type: 'open_settings' }
  | { type: 'save_settings'; settings: Settings }
  | { type: 'close_settings' }
  | { type: 'reset_settings' }
  | { type: 'backgrounded' }
  | { type: 'foregrounded' }
  | { type: 'quit' };

/** Name eines Ereignisses. */
export type UiEventType = UiEvent['type'];

/** Variante einer extern getaggten Enum mit Daten (`{ "Name": … }`) bzw. der Unit-String. */
export function variantName(value: string | Record<string, unknown>): string {
  if (typeof value === 'string') return value;
  const keys = Object.keys(value);
  return keys[0] ?? '';
}
