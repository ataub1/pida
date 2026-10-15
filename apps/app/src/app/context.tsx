/**
 * App-Kontext: Bridge, aktueller `ViewState`, Übersetzer, Live-Region, Hinweise und die Dienste der Shell
 * (Dateiauswahl, Einstellungen bei einem Abschnitt öffnen, Overlays, „Signieren“-Befehl).
 * Die Oberfläche hält **keinen Fachzustand** – nur UI-Zustand (Eingaben, Ziehen, Theme, Overlays;
 * ADR 0004 §4.1).
 */
import { createContext, useContext } from 'react';
import type { Bridge } from '../core/bridge';
import type { UiEvent, ViewState } from '../core/types';
import type { Translator } from '../i18n/catalog';

/** Kurzer Hinweis der Oberfläche (Toast). */
export interface Toast {
  id: number;
  kind: 'info' | 'success' | 'error';
  text: string;
  /** zweite Zeile (optional) */
  detail?: string;
}

/**
 * Abschnitte der Einstellungen für `openSettings(section)` (ADR 0004 §6.1, in E0 festgelegt). Welcher
 * Abschnitt gezeigt wird, ist reiner UI-Zustand; der Kern öffnet nur das Overlay (`open_settings`).
 */
export type SettingsSectionId = 'signature' | 'signers' | 'save' | 'appearance' | 'privacy' | 'advanced' | 'about';

/** Alle Abschnitte in der Reihenfolge der Einstellungen. */
export const SETTINGS_SECTION_IDS: readonly SettingsSectionId[] = ['signature', 'signers', 'save', 'appearance', 'privacy', 'advanced', 'about'];

/** Tabs des bisherigen Einstellungsdialogs (`settings/SettingsDialog.tsx`, bis B3). */
export type SettingsTab = 'simple' | 'advanced' | 'keystore' | 'fido2' | 'privacy' | 'about';

/** Overlays der Shell, die die Oberfläche selbst öffnet (Palette und Kürzelübersicht kommen mit B4). */
export type ShellOverlay = 'palette' | 'shortcuts' | null;

/** Werte des Kontexts. */
export interface AppContextValue {
  bridge: Bridge;
  view: ViewState;
  /**
   * sendet ein Ereignis; Fehler erscheinen als Hinweis. Das Ergebnis meldet, ob der Kern das Ereignis
   * angenommen hat (Masken geben ihren Doppelklickschutz bei `false` wieder frei).
   */
  dispatch: (event: UiEvent) => Promise<boolean>;
  /** Übersetzer der UI-Sprache (`ViewState::language`) */
  t: Translator;
  /** Text für Screenreader (Live-Region, höflich) */
  announce: (text: string) => void;
  /** Hinweis anzeigen */
  toast: (text: string, kind?: Toast['kind'], detail?: string) => void;
  /** Ziehen von Dateien über dem Fenster aktiv */
  dragActive: boolean;
  /** macOS (Kürzel mit ⌘, Zugriffstasten mit ⌃⌥) */
  isMac: boolean;
  /** Einstellungen öffnen, optional bei einem Abschnitt (z. B. `privacy` aus dem Kurzhinweis „Details“) */
  openSettings: (section?: SettingsSectionId) => void;
  /** zuletzt angeforderter Abschnitt der Einstellungen (für `SettingsPanel`) */
  settingsSection: SettingsSectionId;
  /** Dateiauswahl des Systems (Desktop: Dialog, Mobile: Dokument-Picker) → `open_files` */
  browse: () => Promise<void>;
  /** offenes Shell-Overlay (Palette, Kürzel) */
  overlay: ShellOverlay;
  setOverlay: (overlay: ShellOverlay) => void;
  /**
   * „Signieren“ (Knopf, ⌘↵, Palette): mit gewähltem Signiermittel `sign_with`, sonst öffnet der
   * Arbeitsbereich das Menü des Konto-Chips (ADR 0004 §3.4, §8 F12). Der Arbeitsbereich meldet dafür
   * seinen Handler an (`registerSign`); ohne Handler geschieht nichts.
   */
  requestSign: () => void;
  registerSign: (handler: (() => void) | null) => void;
}

export const AppContext = createContext<AppContextValue | null>(null);

/** Zugriff auf den App-Kontext. */
export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error('AppContext fehlt');
  return ctx;
}

/** Übersetzer. */
export function useT(): Translator {
  return useApp().t;
}
