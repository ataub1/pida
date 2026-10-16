/**
 * Mock-Kern, Bereich Einstellungen (Paket UI-B; ADR 0004 §3.11).
 *
 * Hält die Einstellungen, das Overlay (`SettingsView`) und das Ergebnis des PKCS#12-Imports. Behandelt
 * `open_settings`/`close_settings`/`save_settings`/`reset_settings` wie bisher und die sofort wirksamen
 * Einstellungen `apply_settings` wie K3: nur die geänderten Pfade, Teilübernahme (ein Feld mit Problem behält
 * den bisherigen Wert und erscheint in `problems`, bis es gültig übernommen wird; alle anderen werden
 * übernommen), unbekannte Pfade → `ng-settings-field-unknown`, `applied_revision` = Revision des Ereignisses.
 * `problems` = Prüfung der aktuellen Einstellungen (Hinweise wie `settings_apply::validate`, z. B. manueller
 * Proxy ohne Host) plus abgelehnte Felder – mit denselben Codes wie der Kern. Dazu `set_ui_language`, `select_signer{make_default}`
 * (Standard-Signiermittel) und – wie `BackendEvent::LoginSucceeded` im Kern – der gemerkte ID-Austria-Benutzer
 * nach einer Anmeldung mit „merken“. Ersteinrichtung wie K4: `update_first_run` (Standard-Signiermittel, Profil,
 * Blocksprache), `accept_migration` (Werte der Altkonfiguration, `migratedSettings`), `finish_first_run`
 * (`general.first_run_completed`); bei einer Neuinstallation ohne Altkonfiguration „neben dem Original“
 * (`output.folder = "."`, ADR 0004 §8 F4). Jede Änderung meldet `settings-changed` (Vorschau, Arbeitsbereich,
 * „Zuletzt signiert“, Ersteinrichtung).
 *
 * `SettingsView::locked` (per Richtlinie festgelegte Pfade, Nachtrag 5) kommt aus der Option `policy`
 * (`SettingsMockOptions`).
 */
import { defaultSettings } from '../defaults';
import type { BackendChoice, BackendId, KeystoreImportView, Settings, SettingsView } from '../types';
import { migratedSettings, type FirstRunMockOptions } from './firstrun';
import { fileNameOf, outputFileName, type MockArea, type MockCtx, type MockOptions } from './shared';

/** Richtlinie im Mock (Teilmenge von `pida_config::Policy`). */
export interface PolicyMock {
  /** „Zuletzt signiert“ erzwingen (`true`) bzw. verbieten (`false`) */
  recent_documents?: boolean;
  /** Update-Prüfung erzwingen bzw. verbieten */
  update_check?: boolean;
}

/** Optionen des Mock-Kerns mit Richtlinie (Erweiterung des Bereichs Einstellungen). */
export type SettingsMockOptions = MockOptions & { policy?: PolicyMock };

/** Per Richtlinie festgelegte Pfade → Grund (i18n-ID). */
export function policyLocks(policy: PolicyMock | undefined): Record<string, string> {
  const locked: Record<string, string> = {};
  if (policy?.recent_documents !== undefined) locked['general.recent_documents'] = 'ng-recent-policy';
  if (policy?.update_check !== undefined) locked['general.update_check'] = 'ng-settings-policy';
  return locked;
}

export interface SettingsState {
  settings: Settings;
  open: boolean;
  /** von `apply_settings` abgelehnte Felder (bis sie gültig übernommen werden bzw. die Ansicht schließt) */
  problems: Record<string, string>;
  previewRevision: number;
  appliedRevision: number;
  readOnly: boolean;
  keychainAvailable: boolean;
  fido2Available: boolean;
  keystoreImport: KeystoreImportView | null;
  locked: Record<string, string>;
  /** Altkonfiguration (Migrationsangebot) und ihre eigene Bildmarke */
  legacy: string | null;
  legacyEmblem: string | null;
}

/** Einstellungen beim Start: Ersteinrichtung (fehlende Datei) wie K4 (ADR 0004 §3.12, §8 F4). */
function initialSettings(o: MockOptions): Settings {
  const base = o.settings ? structuredClone(o.settings) : defaultSettings();
  if (!o.firstRun) return base;
  const next: Settings = { ...base, general: { ...base.general, first_run_completed: false } };
  // Neuinstallation ohne Altkonfiguration: „neben dem Original“; mit Migration gilt der übernommene Wert
  if (!o.migrationOffer && !o.settings) next.output = { ...base.output, folder: '.' };
  return next;
}

const KEYSTORE_DIR = '/var/mobile/Containers/Data/Application/DEMO/Library/Application Support/data/keystores';

/**
 * Prüfung je Feld wie `settings_apply::validate` im Kern (Pfad → Code); `blocking` = nur die Fälle, die das
 * klassische „Speichern“ ablehnt. Dateien prüft der Mock nicht (Bildmarke, Ordner).
 */
function validate(next: Settings, blocking = false): Record<string, string> {
  const problems: Record<string, string> = {};
  if (/[\\/\0]/.test(next.output.postfix)) problems['output.postfix'] = 'ng-settings-invalid-postfix';
  if (next.proxy.mode === 'manual' && !next.proxy.host?.trim()) problems['proxy.host'] = 'ng-settings-proxy-host-missing';
  if (next.proxy.port !== null && !(Number.isInteger(next.proxy.port) && next.proxy.port >= 1 && next.proxy.port <= 65535)) problems['proxy.port'] = 'ng-settings-invalid-port';
  if (blocking) return problems;
  const size = next.signature.logo_only_size_mm;
  if (!(Number.isFinite(size) && size > 0 && size <= 120)) problems['signature.logo_only_size_mm'] = 'ng-settings-invalid-logo-size';
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
function applyPath(settings: Settings, path: string, value: unknown, problems: Record<string, string>, locked: Record<string, string>): Settings {
  const lock = locked[path];
  if (lock) {
    problems[path] = lock;
    return settings;
  }
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
    settings: initialSettings(o),
    open: false,
    problems: {},
    previewRevision: 1,
    appliedRevision: 0,
    readOnly: o.readOnly ?? false,
    keychainAvailable: o.keychainAvailable ?? true,
    fido2Available: o.fido2Available ?? true,
    keystoreImport: null,
    locked: policyLocks((o as SettingsMockOptions).policy),
    legacy: o.migrationOffer ?? null,
    legacyEmblem: (o as FirstRunMockOptions).legacyEmblem ?? null,
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
        const problems = validate(e.settings, true);
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
        // Nur bei geöffnetem Overlay (wie SaveSettings im Kern); veraltete Revisionen ohne Wirkung
        if (!state.open || e.revision <= state.appliedRevision) return true;
        // abgelehnte Felder neu bewerten: die gesendeten Pfade zählen neu, die übrigen bleiben abgelehnt
        const rejected: Record<string, string> = { ...state.problems };
        for (const path of Object.keys(e.changes)) delete rejected[path];
        let next = state.settings;
        for (const [path, value] of Object.entries(e.changes)) next = applyPath(next, path, value, rejected, state.locked);
        state.problems = rejected;
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
      case 'update_first_run':
        changed(state, ctx, {
          ...state.settings,
          backend: { default: e.default_backend },
          signature: { ...state.settings.signature, profile: e.profile, block_language: e.block_language },
        });
        return true;
      case 'accept_migration':
        if (!state.legacy) return false;
        changed(state, ctx, migratedSettings(state.settings, state.legacyEmblem, ctx.platform().mobile));
        return true;
      case 'finish_first_run':
        if (state.settings.general.first_run_completed) return false;
        state.settings = { ...state.settings, general: { ...state.settings.general, first_run_completed: true } };
        return true;
      default:
        return false;
    }
  },

  project(state) {
    const view: SettingsView | null = state.open
      ? {
          settings: structuredClone(state.settings),
          // wie der Kern: Hinweise der aktuellen Einstellungen plus abgelehnte Felder
          problems: { ...validate(state.settings), ...state.problems },
          keychain_available: state.keychainAvailable,
          fido2_available: state.fido2Available,
          keystore_ready: state.settings.keystore.enabled && !!state.settings.keystore.file,
          read_only: state.readOnly,
          preview_revision: state.previewRevision,
          applied_revision: state.appliedRevision,
          examples: { 'output.postfix': outputFileName('Vertrag.pdf', state.settings.output.postfix || '_signed') },
          locked: { ...state.locked },
        }
      : null;
    return { settings: view, keystore_import: state.keystoreImport ? { ...state.keystoreImport } : null };
  },
};
