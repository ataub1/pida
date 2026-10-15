/**
 * App-Kontext: Bridge, aktueller `ViewState`, Übersetzer, Live-Region und Hinweise.
 * Die Oberfläche hält **keinen Fachzustand** – nur UI-Zustand (Eingaben, Ziehen, Theme).
 */
import { createContext, useContext } from 'react';
import type { Bridge } from '../core/bridge';
import type { UiEvent, ViewState } from '../core/types';
import type { Translator } from '../i18n/catalog';

/** Kurzer Hinweis (Toast). */
export interface Toast {
  id: number;
  kind: 'info' | 'error';
  text: string;
}

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
  toast: (text: string, kind?: Toast['kind']) => void;
  /** Ziehen von Dateien über dem Fenster aktiv */
  dragActive: boolean;
  /** macOS (Zugriffstasten mit ⌃⌥) */
  isMac: boolean;
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
