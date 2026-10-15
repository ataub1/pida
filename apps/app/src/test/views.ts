/** Bausteine für Test-Zustände (serde-Format wie `pida-core-api`). */
import { defaultSettings, mainBar } from '../core/defaults';
import type {
  BackendOptionView,
  DocInfo,
  PlatformView,
  FormContext,
  InteractionView,
  Lang,
  NativeForm,
  OutputView,
  QueueItemView,
  Settings,
  SettingsView,
  StepView,
  ViewState,
} from '../core/types';

export const DOC: DocInfo = {
  id: 7,
  name: 'Vertrag.pdf',
  page_count: 3,
  page_sizes: [
    [595.28, 841.89],
    [595.28, 841.89],
    [595.28, 841.89],
  ],
  signed: false,
};

export const QUEUE: QueueItemView[] = [
  { id: 7, name: 'Vertrag.pdf', current: true },
  { id: 8, name: 'Angebot.pdf', current: false },
];

const BARS: Record<StepView['step'], [string, string, string, string]> = {
  starting: ['', '', '', ''],
  open: ['AE', '', '', ''],
  loading: ['AE', '', '', ''],
  warning: ['AE', '', '', ''],
  placeholder: ['AE', '', '', ''],
  position: ['AE', 'A', '', ''],
  backend: ['AE', 'AE', 'A', ''],
  signing: ['A', 'A', 'A', ''],
  output: ['AE', 'AE', 'AE', 'A'],
  error: ['AE', '', '', ''],
  finished: ['', '', '', ''],
};

/** Plattform der Tests (Standard: Desktop). */
export const DESKTOP: PlatformView = { os: 'macos', mobile: false };
export const IOS: PlatformView = { os: 'ios', mobile: true };
export const ANDROID: PlatformView = { os: 'android', mobile: true };

export function viewOf(step: StepView, extra: Partial<ViewState> = {}): ViewState {
  const settingsEnabled = !['starting', 'signing', 'finished'].includes(step.step);
  return {
    step,
    main_bar: mainBar(BARS[step.step], settingsEnabled),
    settings: null,
    language: 'de',
    notices: [],
    migration: null,
    done: [],
    platform: DESKTOP,
    resume: null,
    keystore_import: null,
    ...extra,
  };
}

/** Derselbe Zustand auf einer Mobile-Plattform. */
export function onMobile(view: ViewState, platform: PlatformView = IOS): ViewState {
  return { ...view, platform };
}

export function positionView(extra: Partial<Extract<StepView, { step: 'position' }>> = {}, lang: Lang = 'de'): ViewState {
  // Live-Region: linke Kante 297,64 pt ≈ 105 mm, Oberkante 210,47 pt ≈ 74 mm (wie das Feld Y)
  return viewOf(
    {
      step: 'position',
      doc: DOC,
      page: 2,
      block: { width: 232, height: 82, preview_revision: 1 },
      placement: { page_index: 2, x: 297.64, y: 210.47, width: 232, height: 82 },
      new_page: false,
      announce: {},
      ...extra,
    },
    { language: lang },
  );
}

export function ctx(extra: Partial<FormContext> = {}): FormContext {
  return {
    signature_data_url: null,
    can_switch_to_sms: true,
    can_switch_to_fido2: true,
    fido2_visible: true,
    problem: null,
    ...extra,
  };
}

export function signingView(interaction: InteractionView, lang: Lang = 'de'): ViewState {
  return viewOf({ step: 'signing', interaction, queue: QUEUE }, { language: lang });
}

export function formView(form: NativeForm, request = 1, lang: Lang = 'de'): ViewState {
  return signingView({ kind: 'form', request, form }, lang);
}

export function backendView(options: BackendOptionView[], privacy = false): ViewState {
  return viewOf({
    step: 'backend',
    options,
    privacy_notice: privacy ? { backend: 'atrust_native', version: 1, custom_text: null } : null,
  });
}

export function settingsView(settings: Settings = defaultSettings(), extra: Partial<SettingsView> = {}, lang: Lang = 'de'): ViewState {
  return viewOf(
    { step: 'open', queue: [] },
    {
      language: lang,
      settings: { settings, problems: {}, keychain_available: true, fido2_available: true, read_only: false, preview_revision: 0, ...extra },
      main_bar: mainBar(['AE', '', '', ''], false),
    },
  );
}

/** Ergebnis des Ausgabe-Schritts (ohne Rückfragen, sofern nicht angegeben). */
export function outputResult(extra: Partial<OutputView> = {}): OutputView {
  return { path: null, save_pending: false, ask_overwrite: null, save_dialog: null, ...extra };
}
