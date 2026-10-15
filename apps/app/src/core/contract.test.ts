/**
 * Vertragstest Oberfläche ↔ Kern (serde-Format):
 * - `fixtures/view-states.json` wird vom Rust-Test `apps/app/src-tauri/tests/serde_contract.rs` aus
 *   echten `pdfover_core_api::ViewState`-Werten erzeugt; hier wird jede Probe gegen ein Laufzeit-Schema
 *   geprüft, dessen Typ der Compiler mit `ViewState` aus `types.ts` gleichsetzt.
 * - `fixtures/ui-events.json` entsteht aus den typisierten TS-Beispielen; Rust liest sie als `UiEvent`.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  arr,
  bool,
  ext,
  int,
  lit,
  nullable,
  num,
  obj,
  objOpen,
  record,
  str,
  tagged,
  tuple2,
  validate,
  type Infer,
  type Same,
} from '../test/schema';
import { UI_EVENT_EXTRA, UI_EVENT_SAMPLES } from './fixtures/ui-events';
import { defaultSettings } from './defaults';
import type { UiEvent, ViewState } from './types';

const lang = lit('de', 'en');
const step = lit('Open', 'Positioning', 'Signing', 'Finish');
const backendId = lit('atrust_native', 'atrust_browser', 'csc', 'local_bku', 'keystore', 'wallet');

const capabilities = obj({
  input: ext({ Digest: obj({ algs: arr(lit('Sha1', 'Sha256', 'Sha384', 'Sha512')) }) }, ['Document'] as const),
  output: lit('Cms', 'RawSignature'),
  certificate: lit('Required', 'OnlyForVisibleBlock', 'No'),
  interaction: lit('None', 'Secret', 'NativeForms', 'EmbeddedWeb', 'SystemBrowser', 'External', 'ExternalApp'),
  server: lit('None', 'DataUrlRelay', 'TokenProxy'),
  max_batch: int,
  login_once_per_batch: bool,
  document_leaves_device: bool,
  qualification: lit('Qes', 'AdES', 'Unknown'),
  factors: obj({ app: bool, sms: bool, qr: bool, fido2: bool, card: bool }),
  cms_reserve: nullable(int),
});

const formProblem = ext(
  { Fido2Failed: str, Atrust: str, Fido2Error: obj({ code: str, detail: str }) },
  ['PasswordTooShort', 'PasswordTooLong', 'NoTan', 'TanTooLong', 'EnteredReferenceValue', 'QrImageFailed', 'LoginRejected'] as const,
);

const formContext = obj({
  signature_data_url: nullable(str),
  can_switch_to_sms: bool,
  can_switch_to_fido2: bool,
  fido2_visible: bool,
  problem: nullable(formProblem),
});

const nativeForm = ext({
  Login: obj({ prefill_username: nullable(str), remember: bool, ctx: formContext }),
  SmsTan: obj({ reference_value: str, ctx: formContext }),
  QrCode: obj({
    reference_value: str,
    image: nullable(obj({ bytes: arr(int), mime: nullable(str) })),
    timeout_secs: nullable(int),
    ctx: formContext,
  }),
  WaitForApp: obj({ reference_value: nullable(str), timeout_secs: nullable(int), ctx: formContext }),
  Fido2: obj({ in_progress: bool, ctx: formContext }),
});

const progress = ext(
  { Document: obj({ index: int, total: int }) },
  ['Connecting', 'ReadingCertificate', 'WaitingForServer', 'Signing', 'Done'] as const,
);

const notice = obj({
  kind: lit('Info', 'Warning', 'Error'),
  code: ext(
    { Http: obj({ status: int }) },
    [
      'AtrustInterstitial',
      'AtrustError',
      'ProxyAuthRequired',
      'DocumentTooLarge',
      'ConnectionFailed',
      'LocalBkuNotReachable',
      'KeystoreFileMissing',
      'KeystoreAliasMissing',
      'KeystoreError',
      'KeystoreConversionNeeded',
    ] as const,
  ),
  text: nullable(str),
  buttons: lit('Ok', 'OkCancel', 'RetryCancel'),
});

const interaction = tagged('kind', {
  waiting: { progress },
  form: { request: int, form: nativeForm },
  secret: {
    request: int,
    prompt: obj({ kind: lit('KeystorePassword', 'KeyPassword', 'ProxyPassword', 'Fido2Pin'), subject: str, retry: bool }),
  },
  notice: { request: int, notice },
});

const docInfo = obj({ id: int, name: str, page_count: int, page_sizes: arr(tuple2(num, num)), signed: bool });
const queueItem = obj({ id: int, name: str, current: bool });

const stepView = tagged('step', {
  starting: {},
  open: { queue: arr(queueItem) },
  loading: { queue: arr(queueItem) },
  warning: { doc: docInfo, codes: arr(str) },
  placeholder: {
    doc: docInfo,
    fields: arr(obj({ name: str, page: nullable(int) })),
    qr: nullable(obj({ page: int, id: nullable(str) })),
  },
  position: {
    doc: docInfo,
    page: int,
    block: obj({ width: num, height: num, preview_revision: int }),
    placement: obj({ page_index: int, x: num, y: num, width: num, height: num }),
    new_page: bool,
    announce: record(str),
  },
  backend: {
    options: arr(obj({ id: backendId, available: bool, capabilities: nullable(capabilities) })),
    privacy_notice: nullable(obj({ backend: backendId, version: int, custom_text: nullable(str) })),
  },
  signing: { interaction, queue: arr(queueItem) },
  output: {
    result: obj({
      path: nullable(str),
      save_pending: bool,
      ask_overwrite: nullable(str),
      save_dialog: nullable(obj({ directory: str, file_name: str })),
    }),
    next: nullable(docInfo),
  },
  error: { code: str, params: record(str), retry: bool },
  finished: { exit_code: int },
});

const settings = objOpen({
  schema_version: int,
  general: obj({
    ui_language: nullable(lang),
    update_check: bool,
    window_size: nullable(obj({ width: int, height: int })),
    last_source_directory: nullable(str),
  }),
  signature: obj({
    profile: lit('SIGNATURBLOCK_SMALL', 'AMTSSIGNATURBLOCK', 'BASE_LOGO', 'INVISIBLE'),
    block_language: nullable(lang),
    note: tagged('mode', { default: {}, none: {}, custom: { text: str } }),
    pdfa_force: bool,
    auto_position: bool,
    emblem: nullable(str),
    logo_only_size_mm: num,
    cms_profile: lit('pdfas_compat', 'pades_bb'),
  }),
  placeholder: obj({ search: bool, kind: lit('fields', 'marker') }),
  backend: obj({ default: nullable(lit('id_austria', 'local_bku', 'keystore')) }),
  id_austria: obj({ username: nullable(str), remember: bool, fido2_default: bool }),
  keystore: obj({
    enabled: bool,
    file: nullable(str),
    declared_type: nullable(str),
    alias: nullable(str),
    password_storage: lit('none', 'memory', 'keychain'),
  }),
  output: obj({ folder: nullable(str), postfix: str }),
  proxy: obj({ mode: lit('system', 'none', 'manual'), host: nullable(str), port: nullable(int), user: nullable(str) }),
  privacy: obj({ acknowledged: record(int) }),
  certificate_cache: obj({ enabled: bool }),
});

const workflowError = ext({
  Rejected: obj({ class: lit('K1Regular', 'K2Repairable', 'K3Encrypted', 'K4Restricted', 'K5Rejected'), code: str }),
  Backend: obj({ code: str, detail: str }),
  Invariant: obj({
    invariant: lit('I1Prefix', 'I2ByteRange', 'I3Cms', 'I4PriorSignatures', 'I5Reparse', 'I6AllowedChanges'),
    detail: str,
  }),
  CmsTooLarge: obj({ needed: int, reserve: int }),
  Config: str,
  Save: str,
  Prepare: obj({ code: str, detail: str }),
  Unexpected: str,
});

const docResult = tagged('kind', {
  signed: { output: obj({ path: str, len: int, sha256: str }) },
  cancelled: {},
  failed: { error: workflowError },
});

const viewState = obj({
  step: stepView,
  main_bar: obj({ steps: arr(obj({ step, active: bool, enabled: bool })), settings_enabled: bool }),
  settings: nullable(
    obj({ settings, problems: record(str), keychain_available: bool, fido2_available: bool, read_only: bool, preview_revision: int }),
  ),
  language: lang,
  notices: arr(obj({ id: int, code: str, params: record(str) })),
  migration: nullable(obj({ legacy_file: str, plaintext_secrets: bool })),
  done: arr(obj({ id: int, name: str, input: str, result: docResult })),
});

const formAnswer = tagged('kind', {
  login: { username: str, password: str, remember: bool },
  tan: { tan: str },
  switch_to_sms: {},
  switch_to_fido2: {},
  start_fido2: {},
  continue: {},
});

const uiEvent = tagged('type', {
  open_files: { paths: arr(str) },
  answer_warning: { proceed: bool },
  placeholder_answer: { answer: lit('Yes', 'No', 'Cancel'), field_index: nullable(int) },
  move_block: { page: int, x: num, y: num },
  nudge_block: { dx: num, dy: num, fine: bool, coarse: bool },
  change_page: { page: int },
  toggle_new_page: {},
  auto_place: {},
  confirm_position: {},
  choose_invisible: {},
  choose_backend: { backend: backendId },
  acknowledge_privacy: { accepted: bool },
  submit_form: { request: int, answer: formAnswer },
  submit_secret: { request: int, secret: str },
  answer_notice: { request: int, answer: lit('Ok', 'Retry', 'Cancel') },
  open_signature_data: { request: int },
  cancel: {},
  retry: {},
  save: { path: nullable(str) },
  confirm_overwrite: { overwrite: bool },
  open_result: {},
  navigate: { step },
  open_settings: {},
  save_settings: { settings },
  close_settings: {},
  reset_settings: {},
  backgrounded: {},
  foregrounded: {},
  quit: {},
  dismiss_notice: { id: int },
  accept_migration: { store_secrets: bool },
  decline_migration: {},
});

// --- Typgleichheit Schema ↔ types.ts (Kompilierzeit) ------------------------------------------
const viewStateMatchesTypes: Same<Infer<typeof viewState>, ViewState> = true;
const uiEventMatchesTypes: Same<Infer<typeof uiEvent>, UiEvent> = true;

const fixtureDir = path.join(path.dirname(fileURLToPath(import.meta.url)), 'fixtures');
const fixtureUrl = (name: string) => path.join(fixtureDir, name);

describe('Vertrag ViewState (Rust → TS)', () => {
  const fixture = JSON.parse(readFileSync(fixtureUrl('view-states.json'), 'utf8')) as { name: string; view: unknown }[];

  it('Schema entspricht den TS-Typen', () => {
    expect(viewStateMatchesTypes).toBe(true);
    expect(uiEventMatchesTypes).toBe(true);
  });

  it('enthält Proben', () => {
    expect(fixture.length).toBeGreaterThan(40);
  });

  it.each(fixture.map((f) => [f.name, f.view] as const))('%s passt zu ViewState', (_name, view) => {
    expect(validate(viewState, view)).toEqual([]);
  });

  it('deckt alle Schritte, Interaktionen und Masken ab', () => {
    const views = fixture.map((f) => f.view as ViewState);
    const steps = new Set(views.map((v) => v.step.step));
    expect([...steps].sort()).toEqual([
      'backend',
      'error',
      'finished',
      'loading',
      'open',
      'output',
      'placeholder',
      'position',
      'signing',
      'starting',
      'warning',
    ]);
    const forms = new Set(
      views.flatMap((v) => (v.step.step === 'signing' && v.step.interaction.kind === 'form' ? [Object.keys(v.step.interaction.form)[0]] : [])),
    );
    expect([...forms].sort()).toEqual(['Fido2', 'Login', 'QrCode', 'SmsTan', 'WaitForApp']);
  });

  it('defaultSettings() entspricht Settings::default() aus Rust (Mock-Kern, Tests, Zurücksetzen)', () => {
    const v = fixture.find((f) => f.name === 'settings_defaults')?.view as ViewState;
    expect(v.settings?.settings).toEqual(defaultSettings());
  });

  it('unbekannte Einstellungen stehen auf oberster Ebene (flatten)', () => {
    const v = fixture.find((f) => f.name === 'settings_open')?.view as ViewState;
    expect(v.settings?.settings['signature.future_key']).toBe('bleibt');
    expect(v.settings?.settings.future).toBe(42);
  });

  it('Schema erkennt Abweichungen', () => {
    expect(
      validate(viewState, {
        step: { step: 'open' },
        main_bar: { steps: [], settings_enabled: true },
        settings: null,
        language: 'fr',
        notices: [],
        migration: null,
        done: [],
      }),
    ).toEqual(['$.step<open>.queue: fehlt', '$.language: eines von de|en erwartet, "fr" gefunden']);
    expect(validate(uiEvent, { type: 'move_block', page: 1, x: 1, y: 2, z: 3 })).toEqual(['$<move_block>.z: unbekanntes Feld']);
    expect(validate(uiEvent, { type: 'frobnicate' })).toHaveLength(1);
  });
});

describe('Vertrag UiEvent (TS → Rust)', () => {
  const samples: UiEvent[] = [...UI_EVENT_SAMPLES, ...UI_EVENT_EXTRA];

  it.each(samples.map((s) => [s.type, s] as const))('%s passt zum Schema', (_t, sample) => {
    expect(validate(uiEvent, sample)).toEqual([]);
  });

  it('alle 32 Varianten sind vertreten', () => {
    expect(new Set(UI_EVENT_SAMPLES.map((s) => s.type)).size).toBe(32);
  });

  it('ui-events.json ist aktuell (für den Rust-Test)', () => {
    const text = `${JSON.stringify(samples, null, 2)}\n`;
    const file = fixtureUrl("ui-events.json");
    if (process.env.PDFOVER_BLESS) writeFileSync(file, text);
    expect(readFileSync(file, "utf8")).toBe(text);
  });
});
