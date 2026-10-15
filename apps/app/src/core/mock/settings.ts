/**
 * Mock-Kern, Bereich Einstellungen (Paket UI-B, in U0 aus dem bisherigen Mock übernommen; ADR 0004 §3.11).
 *
 * Hält die Einstellungen, das Overlay (`SettingsView`) und das Ergebnis des PKCS#12-Imports. Behandelt
 * `open_settings`/`close_settings`/`save_settings`/`reset_settings` wie bisher, dazu vereinfacht
 * `apply_settings` (Pfad → Wert, ungültiger Proxy-Port bleibt als Problem), `set_ui_language`,
 * `select_signer{make_default}` (Standard-Signiermittel) und – wie `BackendEvent::LoginSucceeded` im Kern
 * – den gemerkten ID-Austria-Benutzer nach einer Anmeldung mit „merken“. Jede Änderung meldet
 * `settings-changed`. B3 baut darauf die sofort wirksamen Einstellungen.
 */
import { defaultSettings } from '../defaults';
import type { BackendChoice, BackendId, KeystoreImportView, Settings, SettingsView } from '../types';
import { fileNameOf, outputFileName, type MockArea, type MockCtx } from './shared';

export interface SettingsState {
  settings: Settings;
  open: boolean;
  problems: Record<string, string>;
  previewRevision: number;
  appliedRevision: number;
  readOnly: boolean;
  keychainAvailable: boolean;
  fido2Available: boolean;
  keystoreImport: KeystoreImportView | null;
}

const KEYSTORE_DIR = '/var/mobile/Containers/Data/Application/DEMO/Library/Application Support/data/keystores';

function validate(next: Settings): Record<string, string> {
  const problems: Record<string, string> = {};
  if (next.proxy.mode === 'manual' && !next.proxy.host) problems['proxy.host'] = 'ng-ui-problem-required';
  if (next.proxy.port !== null && (next.proxy.port < 1 || next.proxy.port > 65535)) problems['proxy.port'] = 'ng-ui-problem-port';
  if (next.keystore.enabled && !next.keystore.file) problems['keystore.file'] = 'ng-ui-problem-required';
  return problems;
}

function changed(state: SettingsState, ctx: MockCtx, settings: Settings): void {
  state.settings = settings;
  state.previewRevision += 1;
  ctx.emit({ kind: 'settings-changed', settings: structuredClone(settings) });
}

function choiceOf(backend: BackendId | null): BackendChoice | null {
  switch (backend) {
    case 'atrust_native':
      return 'id_austria';
    case 'local_bku':
      return 'local_bku';
    case 'keystore':
      return 'keystore';
    default:
      return null;
  }
}

/** Setzt `section.feld` = Wert (unbekannte Pfade → Problem `ng-settings-field-unknown`). */
function applyPath(settings: Settings, path: string, value: unknown, problems: Record<string, string>): Settings {
  const [section, field, ...rest] = path.split('.');
  const target = section ? (settings as Record<string, unknown>)[section] : undefined;
  if (!section || !field || rest.length > 0 || !target || typeof target !== 'object' || !(field in (target as object))) {
    problems[path] = 'ng-settings-field-unknown';
    return settings;
  }
  const next = { ...settings, [section]: { ...(target as Record<string, unknown>), [field]: value } } as Settings;
  const p = validate(next);
  if (p[path]) {
    problems[path] = p[path] as string;
    return settings;
  }
  return next;
}

export const settingsArea: MockArea<SettingsState> = {
  init: (o) => ({
    settings: o.settings ? structuredClone(o.settings) : defaultSettings(),
    open: false,
    problems: {},
    previewRevision: 1,
    appliedRevision: 0,
    readOnly: o.readOnly ?? false,
    keychainAvailable: o.keychainAvailable ?? true,
    fido2Available: o.fido2Available ?? true,
    keystoreImport: null,
  }),

  handle(e, state, ctx) {
    switch (e.type) {
      case 'open_settings':
        state.open = true;
        state.problems = {};
        return true;
      case 'close_settings':
        state.open = false;
        state.problems = {};
        return true;
      case 'save_settings': {
        const problems = validate(e.settings);
        if (Object.keys(problems).length > 0) {
          state.problems = problems;
          return true;
        }
        const next = structuredClone(e.settings);
        if (!(next.signature.logo_only_size_mm > 0)) next.signature.logo_only_size_mm = 23;
        state.open = false;
        state.problems = {};
        changed(state, ctx, next);
        return true;
      }
      case 'reset_settings':
        state.open = false;
        state.problems = {};
        changed(state, ctx, defaultSettings());
        return true;
      case 'apply_settings': {
        const problems: Record<string, string> = {};
        let next = state.settings;
        for (const [path, value] of Object.entries(e.changes)) next = applyPath(next, path, value, problems);
        state.problems = problems;
        state.appliedRevision = e.revision;
        if (next !== state.settings) changed(state, ctx, next);
        return true;
      }
      case 'set_ui_language':
        changed(state, ctx, { ...state.settings, general: { ...state.settings.general, ui_language: e.language } });
        return true;
      case 'select_signer':
        if (!e.make_default) return false;
        changed(state, ctx, { ...state.settings, backend: { default: choiceOf(e.backend) } });
        return true;
      case 'submit_form': {
        // wie `BackendEvent::LoginSucceeded`: Benutzername merken (nie das Passwort)
        const a = e.answer;
        if (a.kind !== 'login' || a.password.length < 5 || a.password.length > 200 || a.password === 'falsch') return false;
        const ida = state.settings.id_austria;
        state.settings = { ...state.settings, id_austria: { ...ida, remember: a.remember, username: a.remember ? a.username : ida.username } };
        return true;
      }
      case 'import_keystore': {
        const fileName = fileNameOf(e.path);
        if (!/\.(p12|pfx|pkcs12)$/i.test(fileName)) {
          state.keystoreImport = { file_name: fileName, ok: false, code: 'ng-keystore-import-format', password_stored: false };
          return true;
        }
        if (e.password === 'falsch') {
          state.keystoreImport = { file_name: fileName, ok: false, code: 'keystore-wrong-password', password_stored: false };
          return true;
        }
        const stored = e.remember && !!e.password && state.keychainAvailable;
        const ks = state.settings.keystore;
        changed(state, ctx, {
          ...state.settings,
          keystore: {
            ...ks,
            enabled: true,
            file: `${KEYSTORE_DIR}/${fileName}`,
            declared_type: 'PKCS12',
            password_storage: stored ? 'keychain' : ks.password_storage === 'keychain' ? 'memory' : ks.password_storage,
          },
        });
        state.keystoreImport = { file_name: fileName, ok: true, code: 'ng-keystore-imported', password_stored: stored };
        return true;
      }
      case 'dismiss_keystore_import':
        state.keystoreImport = null;
        return true;
      default:
        return false;
    }
  },

  project(state) {
    const view: SettingsView | null = state.open
      ? {
          settings: structuredClone(state.settings),
          problems: { ...state.problems },
          keychain_available: state.keychainAvailable,
          fido2_available: state.fido2Available,
          read_only: state.readOnly,
          preview_revision: state.previewRevision,
          applied_revision: state.appliedRevision,
          examples: { 'output.postfix': outputFileName('Vertrag.pdf', state.settings.output.postfix || '_signed') },
        }
      : null;
    return { settings: view, keystore_import: state.keystoreImport ? { ...state.keystoreImport } : null };
  },
};
