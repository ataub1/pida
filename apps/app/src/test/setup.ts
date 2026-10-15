// Testumgebung (Vitest + jsdom + Testing Library).
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => {
  cleanup();
  try {
    localStorage.clear();
  } catch {
    /* jsdom ohne Speicher */
  }
  document.documentElement.removeAttribute('data-theme');
});

// jsdom kennt kein PointerCapture.
if (typeof Element !== 'undefined') {
  const proto = Element.prototype as unknown as Record<string, unknown>;
  proto.setPointerCapture ??= () => undefined;
  proto.releasePointerCapture ??= () => undefined;
  proto.hasPointerCapture ??= () => false;
}
