/**
 * A-Trust-Overlay der Web-Variante: Fokusführung (WCAG 2.1.2, 2.4.3) – Fokus beim Öffnen hinein, Tab bleibt im
 * Overlay (Wächter), beim Schließen zurück zum auslösenden Knopf.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createSlTransport } from './ida-overlay';

const BASE = 'https://pida.example/';

function fakeFetch() {
  return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    if (url.endsWith('api/config')) return new Response(JSON.stringify({ sl_endpoint: 'https://sl.example/https-security-layer-request' }));
    if (url.endsWith('api/sl/sessions')) return new Response(JSON.stringify({ id: 's1', token: 'tok' }));
    if (url.endsWith('/response')) {
      // Long-Poll: wartet, bis abgebrochen wird
      return new Promise<Response>((_, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError'))));
    }
    return new Response(null, { status: 204 });
  });
}

describe('A-Trust-Overlay (Web)', () => {
  let trigger: HTMLButtonElement;

  beforeEach(() => {
    vi.stubGlobal('fetch', fakeFetch());
    vi.spyOn(HTMLFormElement.prototype, 'submit').mockImplementation(() => undefined);
    trigger = document.createElement('button');
    trigger.textContent = 'Anmelden';
    document.body.append(trigger);
    trigger.focus();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('holt den Fokus, hält Tab im Overlay und gibt ihn beim Schließen zurück', async () => {
    const resolve = vi.fn();
    const onCancel = vi.fn();
    const sl = createSlTransport(BASE, resolve, () => 'de', onCancel);
    sl('exchange', 1, '<sl:CreateCMSSignatureRequest/>');

    const dialog = await vi.waitFor(() => {
      const d = document.querySelector<HTMLElement>('.ida-overlay[role="dialog"]');
      if (!d) throw new Error('kein Overlay');
      return d;
    });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    // Fokus beim Öffnen: Titel des Overlays (Screenreader nennen den Dialog)
    expect(document.activeElement).toBe(dialog.querySelector('.ida-title'));

    const [before, after] = dialog.querySelectorAll<HTMLElement>('.ida-guard');
    const buttons = dialog.querySelectorAll<HTMLButtonElement>('.ida-head button');
    // Tab aus dem iframe heraus → zurück zur ersten Schaltfläche; Umschalt+Tab davor → ins iframe
    after?.focus();
    expect(document.activeElement).toBe(buttons[0]);
    before?.focus();
    expect(document.activeElement).toBe(dialog.querySelector('iframe'));

    // Escape = Abbrechen (der Kern schließt die Kette dann mit `close`)
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(onCancel).toHaveBeenCalled();
    sl('close', 0, 'aborted');
    expect(document.querySelector('.ida-overlay')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});
