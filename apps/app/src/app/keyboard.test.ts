/** Bildschirmtastatur (app/keyboard.ts): fokussiertes Feld im verkleinerten Dialog wieder in den Blick holen. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { keepFocusedFieldVisible, REVEAL_DELAY_MS } from './keyboard';

afterEach(() => {
  vi.useRealTimers();
  Reflect.deleteProperty(window, 'visualViewport');
  document.body.innerHTML = '';
});

describe('Bildschirmtastatur', () => {
  it('nach dem Verkleinern des sichtbaren Bereichs: Feld im Dialog in den Blick, Seitenfelder nicht', () => {
    vi.useFakeTimers();
    const vv = new EventTarget();
    Object.defineProperty(window, 'visualViewport', { configurable: true, value: vv });
    document.body.innerHTML =
      '<div class="overlay"><div class="modal"><input id="pw" type="password"><input id="sw" type="checkbox"></div></div><input id="page">';
    const pw = document.getElementById('pw') as HTMLInputElement;
    const sw = document.getElementById('sw') as HTMLInputElement;
    const page = document.getElementById('page') as HTMLInputElement;
    const reveal = { pw: vi.fn(), sw: vi.fn(), page: vi.fn() };
    pw.scrollIntoView = reveal.pw;
    sw.scrollIntoView = reveal.sw;
    page.scrollIntoView = reveal.page;
    const stop = keepFocusedFieldVisible(window);

    pw.focus();
    // mehrere Ereignisse während der Animation: einmal, nach der Pause
    vv.dispatchEvent(new Event('resize'));
    vv.dispatchEvent(new Event('resize'));
    vi.advanceTimersByTime(REVEAL_DELAY_MS - 1);
    expect(reveal.pw).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(reveal.pw).toHaveBeenCalledTimes(1);
    expect(reveal.pw).toHaveBeenCalledWith({ block: 'nearest', inline: 'nearest' });

    sw.focus();
    vv.dispatchEvent(new Event('resize'));
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(reveal.sw).not.toHaveBeenCalled();

    // außerhalb eines Overlays rollt WebKit selbst an das Feld heran
    page.focus();
    vv.dispatchEvent(new Event('resize'));
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(reveal.page).not.toHaveBeenCalled();

    stop();
    pw.focus();
    vv.dispatchEvent(new Event('resize'));
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(reveal.pw).toHaveBeenCalledTimes(1);
  });

  it('ohne visualViewport (ältere WebViews): nichts zu tun', () => {
    expect(window.visualViewport ?? null).toBeNull();
    const stop = keepFocusedFieldVisible(window);
    expect(stop).toBeTypeOf('function');
    stop();
  });
});
