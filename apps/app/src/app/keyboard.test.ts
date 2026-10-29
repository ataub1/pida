/** Bildschirmtastatur (app/keyboard.ts): fokussiertes Feld im verkleinerten Dialog wieder in den Blick holen. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { KEYBOARD_MIN_PX, keepFocusedFieldVisible, REVEAL_DELAY_MS } from './keyboard';

afterEach(() => {
  vi.useRealTimers();
  Reflect.deleteProperty(window, 'visualViewport');
  document.body.innerHTML = '';
  document.documentElement.removeAttribute('data-keyboard');
  document.documentElement.style.removeProperty('--vv-top');
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

  it('Fokuswechsel bei offener Tastatur (Benutzername → Passwort) und Rollen des sichtbaren Bereichs: Feld in den Blick', () => {
    vi.useFakeTimers();
    const vv = Object.assign(new EventTarget(), { height: window.innerHeight - 300, scale: 1, offsetTop: 0 });
    Object.defineProperty(window, 'visualViewport', { configurable: true, value: vv });
    document.body.innerHTML = '<div class="overlay"><div class="modal"><input id="user"><input id="pw" type="password"></div></div>';
    const user = document.getElementById('user') as HTMLInputElement;
    const pw = document.getElementById('pw') as HTMLInputElement;
    user.scrollIntoView = vi.fn();
    pw.scrollIntoView = vi.fn();
    const stop = keepFocusedFieldVisible(window);

    user.focus();
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(user.scrollIntoView).toHaveBeenCalledTimes(1);
    // kein `resize`: die Tastatur bleibt offen
    pw.focus();
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(pw.scrollIntoView).toHaveBeenCalledTimes(1);
    // iOS rollt den sichtbaren Bereich nach
    vv.offsetTop = 120;
    vv.dispatchEvent(new Event('scroll'));
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(pw.scrollIntoView).toHaveBeenCalledTimes(2);

    stop();
    user.focus();
    vi.advanceTimersByTime(REVEAL_DELAY_MS);
    expect(user.scrollIntoView).toHaveBeenCalledTimes(1);
  });

  it('`data-keyboard` am Wurzelelement, solange die Tastatur den sichtbaren Bereich verkleinert (nicht beim Zoomen)', () => {
    // iPhone 17e: Fenster 844 hoch, mit Software-Tastatur sichtbar 448 – die mitlaufende Vorschau verdeckte sonst
    // das fokussierte Feld (settings.css, signing.css)
    const vv = Object.assign(new EventTarget(), { height: window.innerHeight, scale: 1 });
    Object.defineProperty(window, 'visualViewport', { configurable: true, value: vv });
    const root = document.documentElement;
    const stop = keepFocusedFieldVisible(window);
    const resize = (height: number, scale = 1) => {
      vv.height = height;
      vv.scale = scale;
      vv.dispatchEvent(new Event('resize'));
    };
    expect(root.hasAttribute('data-keyboard')).toBe(false);
    resize(window.innerHeight - 396);
    expect(root.hasAttribute('data-keyboard')).toBe(true);
    // nur die Zubehörleiste (Hardware-Tastatur): kein Platzproblem
    resize(window.innerHeight - (KEYBOARD_MIN_PX - 1));
    expect(root.hasAttribute('data-keyboard')).toBe(false);
    // Pinch-Zoom: sichtbarer Ausschnitt kleiner, aber nicht verdeckt
    resize(window.innerHeight / 2, 2);
    expect(root.hasAttribute('data-keyboard')).toBe(false);
    resize(window.innerHeight - 300);
    expect(root.hasAttribute('data-keyboard')).toBe(true);
    stop();
    expect(root.hasAttribute('data-keyboard')).toBe(false);
  });

  it('ohne visualViewport (ältere WebViews): nichts zu tun', () => {
    expect(window.visualViewport ?? null).toBeNull();
    const stop = keepFocusedFieldVisible(window);
    expect(stop).toBeTypeOf('function');
    stop();
  });

  it('`--vv-top` folgt dem gerollten sichtbaren Bereich (iOS rollt beim Fokussieren), danach wieder entfernt', () => {
    const vv = Object.assign(new EventTarget(), { height: window.innerHeight, scale: 1, offsetTop: 0 });
    Object.defineProperty(window, 'visualViewport', { configurable: true, value: vv });
    const root = document.documentElement;
    const stop = keepFocusedFieldVisible(window);
    vv.offsetTop = 212.4;
    vv.dispatchEvent(new Event('scroll'));
    expect(root.style.getPropertyValue('--vv-top')).toBe('212px');
    vv.offsetTop = 0;
    vv.dispatchEvent(new Event('resize'));
    expect(root.style.getPropertyValue('--vv-top')).toBe('');
    vv.offsetTop = 90;
    vv.dispatchEvent(new Event('resize'));
    expect(root.style.getPropertyValue('--vv-top')).toBe('90px');
    stop();
    expect(root.style.getPropertyValue('--vv-top')).toBe('');
  });
});
