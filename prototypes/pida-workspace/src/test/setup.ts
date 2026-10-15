// Testumgebung (Vitest + jsdom + Testing Library). jsdom hat kein Layout: ResizeObserver und
// matchMedia fehlen bewusst (die App fällt dann auf Desktop und feste Maßstäbe zurück).
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => {
  if (typeof document === 'undefined') return; // Tests mit `@vitest-environment node`
  cleanup();
  try {
    localStorage.clear();
  } catch {
    /* jsdom ohne Speicher */
  }
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.removeAttribute('lang');
});

// jsdom kennt kein PointerCapture.
if (typeof Element !== 'undefined') {
  const proto = Element.prototype as unknown as Record<string, unknown>;
  proto.setPointerCapture ??= () => undefined;
  proto.releasePointerCapture ??= () => undefined;
  proto.hasPointerCapture ??= () => false;
}

// axe-core fragt Canvas ab (Icon-Ligaturen); jsdom hat kein Canvas – still „nicht verfügbar“ melden.
if (typeof HTMLCanvasElement !== 'undefined') {
  HTMLCanvasElement.prototype.getContext = (() => null) as unknown as HTMLCanvasElement['getContext'];
}
