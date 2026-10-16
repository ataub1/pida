/**
 * Android: Farben der Systemleisten folgen dem Farbschema der Oberfläche (Emulator-Abnahme 16.10.2026).
 *
 * Die Activity legt Status- und Navigationsleiste als Abstand um die WebView (`MainActivity`,
 * `applySystemBarInsets` – die WebView meldet sie nicht als `env(safe-area-inset-*)`); die Flächen darunter
 * zeichnet Android selbst. Ohne Abgleich blieben sie im Dunkelmodus hell und die Statusleiste über der weißen
 * Kopfzeile grau. Die Oberfläche meldet deshalb nach jedem Wechsel (Hell/Dunkel/System) die Farben der
 * Kopfzeile (`--c-surface`, oben) und des Hintergrunds (`--c-bg`, unten) über die Brücke `PidaSystemBars`
 * (nur Farben, keine Daten; die Activity prüft das Format `#rrggbb`).
 *
 * Andere Plattformen haben keine Brücke: iOS zeichnet die WebView unter die Leisten, der Desktop hat keine.
 */

/** Brücke der Android-Activity (`addJavascriptInterface`). */
interface SystemBarsBridge {
  update(top: string, bottom: string, dark: boolean): void;
}

function bridge(): SystemBarsBridge | null {
  const b = (window as unknown as { PidaSystemBars?: Partial<SystemBarsBridge> }).PidaSystemBars;
  return b && typeof b.update === 'function' ? (b as SystemBarsBridge) : null;
}

/** Dunkler Hintergrund (`#rrggbb`, relative Luminanz unter 0,5) → helle Symbole in den Leisten. */
export function isDark(hex: string): boolean {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!m) return false;
  const [r, g, b] = [m[1], m[2], m[3]].map((x) => parseInt(x ?? '0', 16) / 255) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5;
}

/** Meldet die aktuellen Farben an die Activity (ohne Brücke: nichts). */
export function syncSystemBars(root: HTMLElement = document.documentElement): void {
  const b = bridge();
  if (!b) return;
  const style = getComputedStyle(root);
  const top = style.getPropertyValue('--c-surface').trim();
  const bottom = style.getPropertyValue('--c-bg').trim();
  if (!top || !bottom) return;
  const dark = isDark(bottom);
  try {
    b.update(top, bottom, dark);
  } catch {
    /* nur Anzeige */
  }
}

/** Folgt das Schema dem System, ändert ein Wechsel des Systems die Farben ebenfalls; Rückgabe = abmelden. */
export function watchSystemBars(): () => void {
  if (!bridge() || typeof window.matchMedia !== 'function') return () => undefined;
  const query = window.matchMedia('(prefers-color-scheme: dark)');
  const on = () => syncSystemBars();
  query.addEventListener?.('change', on);
  return () => query.removeEventListener?.('change', on);
}
