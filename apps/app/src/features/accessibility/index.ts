/**
 * Barrierefreie Bedienung (docs/analyse/06): Einstellungen › Barrierefreiheit aus `ViewState::accessibility`.
 * Der Schalter „Barrierefreie Bedienung“ stellt mehrere Werte gemeinsam ein (`withMode`); Funktionen schaltet er
 * nicht frei – alle Wege der Platzierung gibt es immer. Gesetzt wird ohne Einstellungsansicht mit
 * `set_accessibility` (Kopfzeile, Inspektor) bzw. in den Einstellungen mit `apply_settings`.
 */
import { defaultAccessibility } from '../../core/defaults';
import type { AccessibilitySettings, ViewState } from '../../core/types';

/** Aktuelle Werte (ältere Kerne ohne das Feld: Standard). */
export function accessibilityOf(view: Pick<ViewState, 'accessibility'>): AccessibilitySettings {
  return view.accessibility ?? defaultAccessibility();
}

/**
 * Schalter „Barrierefreie Bedienung“: ein = Platzierung beginnt mit der Methodenwahl, ausführliche Ansagen erst
 * nach dem Verschieben, Prüfung vor dem Signieren; aus = diese Werte wieder wie im Standard. Die Schrittweite der
 * Pfeiltasten bleibt, wie sie ist.
 */
export function withMode(a: AccessibilitySettings, on: boolean): AccessibilitySettings {
  const d = defaultAccessibility();
  return on
    ? { ...a, mode: true, start: 'methods', announce: 'detailed', announce_after_move: true, check_before_sign: true }
    : { ...a, mode: false, start: d.start, announce: d.announce, announce_after_move: d.announce_after_move, check_before_sign: d.check_before_sign };
}

/** Pfade für `apply_settings` (Einstellungen › Barrierefreiheit). */
export function accessibilityChanges(a: AccessibilitySettings): Record<string, unknown> {
  return Object.fromEntries(Object.entries(a).map(([k, v]) => [`accessibility.${k}`, v]));
}

/** Millimeter aus pt. */
export const PT_PER_MM = 72 / 25.4;

/** Zahl in mm für Anzeige und Felder (eine Nachkommastelle, Komma im Deutschen). */
export function formatMm(pt: number, lang: 'de' | 'en'): string {
  return new Intl.NumberFormat(lang === 'de' ? 'de-AT' : 'en-GB', { maximumFractionDigits: 1, useGrouping: false }).format(pt / PT_PER_MM);
}

/** Eingabe in mm lesen (Komma oder Punkt); `null` = keine Zahl. */
export function parseMm(text: string): number | null {
  const s = text.trim().replace(',', '.');
  if (!/^\d+(\.\d+)?$/.test(s)) return null;
  const v = Number(s);
  return Number.isFinite(v) ? v : null;
}

/** URL-Parameter `?barrierefrei=1` (Direktlink, z. B. von PDF-AS-WAI); auch `a11y=1`. */
export function modeRequestedByUrl(search: string): boolean {
  const p = new URLSearchParams(search);
  return ['barrierefrei', 'a11y'].some((k) => p.get(k) === '1' || p.get(k) === 'true' || p.get(k) === '');
}
