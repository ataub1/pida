/**
 * Android-Systemleisten (Emulator-Abnahme 16.10.2026): Die Oberfläche meldet der Activity die Farben von
 * Kopfzeile und Hintergrund und ob die Symbole hell sein müssen; ohne Brücke (Desktop, iOS, Tests) passiert nichts.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isDark, syncSystemBars, watchSystemBars } from './systemBars';

type Win = Window & { PidaSystemBars?: { update: (top: string, bottom: string, dark: boolean) => void } };

afterEach(() => {
  delete (window as Win).PidaSystemBars;
  document.documentElement.style.removeProperty('--c-surface');
  document.documentElement.style.removeProperty('--c-bg');
});

describe('Systemleisten', () => {
  it('meldet Kopfzeilen- und Hintergrundfarbe; dunkler Hintergrund → helle Symbole', () => {
    const update = vi.fn();
    (window as Win).PidaSystemBars = { update };
    const root = document.documentElement;
    root.style.setProperty('--c-surface', '#ffffff');
    root.style.setProperty('--c-bg', '#f4f5f7');
    syncSystemBars();
    expect(update).toHaveBeenLastCalledWith('#ffffff', '#f4f5f7', false);
    root.style.setProperty('--c-surface', '#171a1f');
    root.style.setProperty('--c-bg', '#0f1114');
    syncSystemBars();
    expect(update).toHaveBeenLastCalledWith('#171a1f', '#0f1114', true);
  });

  it('ohne Brücke oder ohne Tokens: nichts', () => {
    expect(() => syncSystemBars()).not.toThrow();
    const update = vi.fn();
    (window as Win).PidaSystemBars = { update };
    syncSystemBars();
    expect(update).not.toHaveBeenCalled();
    expect(watchSystemBars()).toBeTypeOf('function');
  });

  it('Fehler der Brücke stören die Oberfläche nicht', () => {
    (window as Win).PidaSystemBars = {
      update: () => {
        throw new Error('weg');
      },
    };
    document.documentElement.style.setProperty('--c-surface', '#ffffff');
    document.documentElement.style.setProperty('--c-bg', '#f4f5f7');
    expect(() => syncSystemBars()).not.toThrow();
  });

  it('Helligkeit', () => {
    expect(isDark('#0f1114')).toBe(true);
    expect(isDark('#f4f5f7')).toBe(false);
    expect(isDark('rot')).toBe(false);
  });
});
