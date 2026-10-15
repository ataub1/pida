/**
 * Standardwerte wie in Rust (`pdfover_config::Settings::default()`, docs/analyse/01 §3.5) und kleine
 * Hilfen zur unveränderlichen Bearbeitung der Einstellungen.
 */
import type { MainBarStep, MainBarView, Settings, Step } from './types';

/** Entspricht `Settings::default()` (crates/config/src/settings.rs). */
export function defaultSettings(): Settings {
  return {
    schema_version: 1,
    general: { ui_language: null, update_check: true, window_size: null, last_source_directory: null },
    signature: {
      profile: 'SIGNATURBLOCK_SMALL',
      block_language: null,
      note: { mode: 'default' },
      pdfa_force: false,
      auto_position: false,
      emblem: null,
      logo_only_size_mm: 23,
      cms_profile: 'pdfas_compat',
    },
    placeholder: { search: false, kind: 'marker' },
    backend: { default: null },
    id_austria: { username: null, remember: false, fido2_default: false },
    keystore: { enabled: false, file: null, declared_type: null, alias: null, password_storage: 'memory' },
    output: { folder: null, postfix: '_signed' },
    proxy: { mode: 'system', host: null, port: null, user: null },
    privacy: { acknowledged: {} },
    certificate_cache: { enabled: false },
  };
}

/** Bekannte Abschnitte der Einstellungen (alles andere ist `Settings::unknown`). */
export const SETTINGS_SECTIONS = [
  'general',
  'signature',
  'placeholder',
  'backend',
  'id_austria',
  'keystore',
  'output',
  'proxy',
  'privacy',
  'certificate_cache',
] as const;

/** Abschnittsname. */
export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];

/**
 * Ändert einen Abschnitt; unbekannte Einträge (`Settings::unknown`, flatten) bleiben erhalten
 * (docs/integration/wave1-offene-punkte.md: JSON-Rundlauf über die Bridge).
 */
export function withSection<K extends SettingsSection>(
  settings: Settings,
  section: K,
  patch: Partial<Settings[K]>,
): Settings {
  return { ...settings, [section]: { ...settings[section], ...patch } };
}

/** Reihenfolge der Hauptleiste. */
export const MAIN_BAR_ORDER: Step[] = ['Open', 'Positioning', 'Signing', 'Finish'];

/** Hauptleiste aus „A“/„E“-Kürzeln (Tabelle docs/analyse/01 §3.5), z. B. `bar(['AE','A','',''], true)`. */
export function mainBar(cells: [string, string, string, string], settingsEnabled: boolean): MainBarView {
  const steps: MainBarStep[] = MAIN_BAR_ORDER.map((step, i) => {
    const cell = cells[i] ?? '';
    return { step, active: cell.includes('A'), enabled: cell.includes('E') };
  });
  return { steps, settings_enabled: settingsEnabled };
}

/** pt → mm. */
export function ptToMm(pt: number): number {
  return (pt * 25.4) / 72;
}

/** mm → pt. */
export function mmToPt(mm: number): number {
  return (mm * 72) / 25.4;
}

/** Dateiname eines Pfads (beide Trenner). */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Ordner eines Pfads (beide Trenner). */
export function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i > 0 ? path.slice(0, i) : '';
}
