/**
 * Hell/Dunkel: folgt dem System (`prefers-color-scheme`) oder fest gewählt. Reine Anzeige-Einstellung
 * je Gerät (localStorage, nicht im Settings-Schema des Kerns); ohne Speicher gilt „System“.
 */
export type Theme = 'system' | 'light' | 'dark';

const KEY = 'pdfover-ng.theme';

export function readTheme(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' ? v : 'system';
  } catch {
    return 'system';
  }
}

export function writeTheme(theme: Theme): void {
  try {
    if (theme === 'system') localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, theme);
  } catch {
    /* nur Komfort */
  }
}

/** Setzt `data-theme` am Wurzelelement (`system` entfernt es). */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
}
