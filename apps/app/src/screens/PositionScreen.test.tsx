import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { renderApp } from '../test/render';
import { DOC, positionView } from '../test/views';
import { clampToPage, wheelDelta } from './PositionScreen';

async function block() {
  return screen.findByRole('application', { name: 'Signatur' });
}

describe('Positionieren', () => {
  it('zeigt Seite, Block, Seitenanzeige und Hilfe', async () => {
    renderApp({ view: positionView() });
    expect(await screen.findByRole('heading', { level: 1, name: 'Signatur positionieren' })).toBeInTheDocument();
    const b = await block();
    expect(b).toHaveAttribute('aria-roledescription', 'verschiebbarer Signaturblock');
    expect(b).toHaveAttribute('tabindex', '0');
    expect(screen.getByRole('img', { name: 'Seite 3 von 3 – Vertrag.pdf' })).toBeInTheDocument();
    expect(screen.getByText('Seite 3 von 3', { selector: '.pager-label' })).toBeInTheDocument();
    // Status nennt linke Kante und Oberkante: 297,64 pt ≈ 105 mm (linke Kante), 210,47 pt ≈ 74 mm (Oberkante von unten);
    // die früheren Zahlenfelder entfallen (Entscheidung des Auftraggebers)
    expect(document.getElementById('position-status')).toHaveTextContent(
      'Seite 3 von 3, linke Kante 105 mm von links, Oberkante 74 mm von unten',
    );
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(b.getAttribute('aria-describedby')).toContain('position-help');
  });

  it('Pfeiltasten: 15/1/50 pt über NudgeBlock (fein = Shift, grob = Strg/⌘)', async () => {
    const { mock } = renderApp({ view: positionView() });
    const b = await block();
    b.focus();
    fireEvent.keyDown(b, { key: 'ArrowRight' });
    fireEvent.keyDown(b, { key: 'ArrowUp', shiftKey: true });
    fireEvent.keyDown(b, { key: 'ArrowLeft', ctrlKey: true });
    fireEvent.keyDown(b, { key: 'ArrowDown', metaKey: true });
    expect(mock.dispatched).toEqual([
      { type: 'nudge_block', dx: 1, dy: 0, fine: false, coarse: false },
      { type: 'nudge_block', dx: 0, dy: 1, fine: true, coarse: false },
      { type: 'nudge_block', dx: -1, dy: 0, fine: false, coarse: true },
      { type: 'nudge_block', dx: 0, dy: -1, fine: false, coarse: true },
    ]);
  });

  it('Bild↑/↓, Pos1/Ende wechseln die Seite, Enter bestätigt', async () => {
    const { mock } = renderApp({ view: positionView({ page: 1, placement: { page_index: 1, x: 10, y: 300, width: 232, height: 82 } }) });
    const b = await block();
    fireEvent.keyDown(b, { key: 'PageUp' });
    fireEvent.keyDown(b, { key: 'PageDown' });
    fireEvent.keyDown(b, { key: 'Home' });
    fireEvent.keyDown(b, { key: 'End' });
    fireEvent.keyDown(b, { key: 'Enter' });
    expect(mock.dispatched).toEqual([
      { type: 'change_page', page: 0 },
      { type: 'change_page', page: 2 },
      { type: 'change_page', page: 0 },
      { type: 'change_page', page: 2 },
      { type: 'confirm_position' },
    ]);
  });

  it('Seitenknöpfe sind an den Rändern gesperrt', async () => {
    const { mock, user } = renderApp({ view: positionView() });
    await block();
    expect(screen.getByRole('button', { name: 'Nächste Seite' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Vorherige Seite' }));
    expect(mock.dispatched).toEqual([{ type: 'change_page', page: 1 }]);
  });

  it('Mausrad blättert (Shift: 5 Seiten)', async () => {
    const { mock } = renderApp({ view: positionView({ page: 0, placement: { page_index: 0, x: 10, y: 300, width: 232, height: 82 } }) });
    await block();
    const stage = document.querySelector('.page-stage') as HTMLElement;
    const now = vi.spyOn(Date, 'now');
    now.mockReturnValue(1000);
    fireEvent.wheel(stage, { deltaY: 120 });
    now.mockReturnValue(2000);
    fireEvent.wheel(stage, { deltaY: 120, shiftKey: true });
    expect(mock.dispatched).toEqual([
      { type: 'change_page', page: 1 },
      { type: 'change_page', page: 2 },
    ]);
  });

  it('Mausrad: Shift unter macOS (deltaX) und Zeilenmodus zählen ebenfalls', async () => {
    const { mock } = renderApp({ view: positionView({ page: 1, placement: { page_index: 1, x: 10, y: 300, width: 232, height: 82 } }) });
    await block();
    const stage = document.querySelector('.page-stage') as HTMLElement;
    const now = vi.spyOn(Date, 'now');
    now.mockReturnValue(1000);
    // WebKit: Shift+Rad liefert horizontales Scrollen
    fireEvent.wheel(stage, { deltaY: 0, deltaX: -120, shiftKey: true });
    now.mockReturnValue(2000);
    // Zeilenmodus (deltaMode 1): 3 Zeilen je Raste
    fireEvent.wheel(stage, { deltaY: 3, deltaMode: 1 });
    expect(mock.dispatched).toEqual([
      { type: 'change_page', page: 0 },
      { type: 'change_page', page: 2 },
    ]);
  });

  it('wheelDelta normalisiert Pixel, Zeilen, Seiten und Shift', () => {
    expect(wheelDelta({ deltaX: 0, deltaY: 100, deltaMode: 0, shiftKey: false })).toBe(100);
    expect(wheelDelta({ deltaX: 0, deltaY: -3, deltaMode: 1, shiftKey: false })).toBe(-120);
    expect(wheelDelta({ deltaX: 0, deltaY: 1, deltaMode: 2, shiftKey: false })).toBe(800);
    expect(wheelDelta({ deltaX: 90, deltaY: 0, deltaMode: 0, shiftKey: true })).toBe(90);
    // ohne Shift ist horizontales Scrollen kein Blättern
    expect(wheelDelta({ deltaX: 90, deltaY: 0, deltaMode: 0, shiftKey: false })).toBe(0);
  });

  it('Ziehen mit dem Zeiger sendet MoveBlock (Greifversatz bleibt)', async () => {
    const { mock } = renderApp({ view: positionView() });
    const b = await block();
    const surface = document.querySelector('.page-surface') as HTMLElement;
    // Fallback-Maßstab ohne Layout: min(560/595.28, 760/841.89) ≈ 0.9027
    const scale = Math.min(560 / DOC.page_sizes[2]![0], 760 / DOC.page_sizes[2]![1]);
    surface.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    const ph = DOC.page_sizes[2]![1];
    // Block bei x=297.64, Oberkante y=210.47 → 10 pt innerhalb des Blocks greifen
    const grabX = (297.64 + 10) * scale;
    const grabY = (ph - 210.47 + 10) * scale;
    fireEvent.pointerDown(b, { button: 0, pointerId: 1, clientX: grabX, clientY: grabY });
    fireEvent.pointerMove(surface, { pointerId: 1, clientX: grabX - 100 * scale, clientY: grabY - 50 * scale });
    // vorläufige Lage während des Ziehens sichtbar
    expect(b).toHaveAttribute('data-dragging');
    fireEvent.pointerUp(surface, { pointerId: 1, clientX: grabX - 100 * scale, clientY: grabY - 50 * scale });
    await waitFor(() => expect(mock.dispatched.length).toBeGreaterThan(0));
    const last = mock.dispatched[mock.dispatched.length - 1];
    expect(last).toMatchObject({ type: 'move_block', page: 2 });
    if (last?.type !== 'move_block') throw new Error('move_block erwartet');
    expect(last.x).toBeCloseTo(197.64, 1);
    expect(last.y).toBeCloseTo(260.47, 1);
    // Der (feste) Testkern meldet keinen neuen Zustand: kurz danach gilt wieder seine Position.
    await waitFor(() => expect(b).not.toHaveAttribute('data-dragging'));
    expect(b.style.left).toBe(`${297.64 * scale}px`);
  });

  it('nach dem Ziehen gilt die Position des Kerns, auch wenn er anders klemmt', async () => {
    const { mock } = renderApp({ view: positionView() });
    const b = await block();
    const surface = document.querySelector('.page-surface') as HTMLElement;
    const scale = Math.min(560 / DOC.page_sizes[2]![0], 760 / DOC.page_sizes[2]![1]);
    surface.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    fireEvent.pointerDown(surface, { button: 0, pointerId: 3, clientX: 50 * scale, clientY: 100 * scale });
    fireEvent.pointerMove(surface, { pointerId: 3, clientX: 60 * scale, clientY: 100 * scale });
    // Kern antwortet während des Ziehens (z. B. auf ganze pt gerundet) – die Vorschau bleibt am Zeiger
    act(() => mock.setView(positionView({ placement: { page_index: 2, x: 60, y: 742, width: 232, height: 82 } })));
    expect(b).toHaveAttribute('data-dragging');
    fireEvent.pointerUp(surface, { pointerId: 3, clientX: 60 * scale, clientY: 100 * scale });
    // Antwort auf die letzte Bewegung: andere Klemmung als die Vorschau
    act(() => mock.setView(positionView({ placement: { page_index: 2, x: 61, y: 700, width: 232, height: 82 } })));
    await waitFor(() => expect(b).not.toHaveAttribute('data-dragging'));
    expect(b.style.left).toBe(`${61 * scale}px`);
    // Statuszeile nennt dieselben Kanten wie früher die Felder: linke Kante von links, Oberkante von unten (246,9 mm)
    expect(document.getElementById('position-status')).toHaveTextContent(/Seite 3 von 3, linke Kante 22 mm von links, Oberkante 24[67](,\d)? mm von unten/);
  });

  it('Klick auf den Block ohne Bewegung lässt keinen Ziehzustand zurück', async () => {
    renderApp({ view: positionView() });
    const b = await block();
    const surface = document.querySelector('.page-surface') as HTMLElement;
    const scale = Math.min(560 / DOC.page_sizes[2]![0], 760 / DOC.page_sizes[2]![1]);
    surface.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    const ph = DOC.page_sizes[2]![1];
    const x = (297.64 + 5) * scale;
    const y = (ph - 210.47 + 5) * scale;
    fireEvent.pointerDown(b, { button: 0, pointerId: 4, clientX: x, clientY: y });
    fireEvent.pointerUp(b, { pointerId: 4, clientX: x, clientY: y });
    await waitFor(() => expect(b).not.toHaveAttribute('data-dragging'));
    expect(b).toHaveFocus();
  });

  it('Klick neben den Block setzt die linke obere Ecke an den Zeiger (geklemmt)', async () => {
    const { mock } = renderApp({ view: positionView() });
    await block();
    const surface = document.querySelector('.page-surface') as HTMLElement;
    surface.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    fireEvent.pointerDown(surface, { button: 0, pointerId: 2, clientX: 5000, clientY: 1 });
    fireEvent.pointerUp(surface, { pointerId: 2, clientX: 5000, clientY: 1 });
    await waitFor(() => expect(mock.dispatched.length).toBeGreaterThan(0));
    const last = mock.dispatched[mock.dispatched.length - 1];
    if (last?.type !== 'move_block') throw new Error('move_block erwartet');
    expect(last.x).toBeCloseTo(595.28 - 232, 1);
    expect(last.y).toBeCloseTo(841.89 - 1 / Math.min(560 / 595.28, 760 / 841.89), 0);
  });

  it('ohne manuelle Positionsfelder (kein Abschnitt „Position“, keine mm/pt-Umschaltung)', async () => {
    renderApp({ view: positionView() });
    await block();
    expect(screen.queryByRole('group', { name: 'Position' })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: 'mm' })).not.toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: 'pt' })).not.toBeInTheDocument();
    expect(screen.queryByText(/X von links|Oberkante von unten/)).not.toBeInTheDocument();
    // Seitennavigation bleibt
    expect(screen.getByRole('group', { name: 'Seiten' })).toBeInTheDocument();
  });

  it('Neue Seite, Automatisch, Unsichtbar, Signieren', async () => {
    const { mock, user } = renderApp({ view: positionView() });
    await block();
    await user.click(screen.getByRole('button', { name: 'Neue Seite anlegen' }));
    await user.click(screen.getByRole('button', { name: 'Automatisch' }));
    await user.click(screen.getByRole('button', { name: 'Unsichtbar signieren' }));
    await user.click(screen.getByRole('button', { name: 'Signieren' }));
    expect(mock.dispatched).toEqual([{ type: 'toggle_new_page' }, { type: 'auto_place' }, { type: 'choose_invisible' }, { type: 'confirm_position' }]);
  });

  it('virtuelle neue Seite und signiertes Dokument', async () => {
    renderApp({
      view: positionView({
        doc: { ...DOC, signed: true },
        page: 3,
        new_page: true,
        placement: { page_index: 3, x: 0, y: 841.89, width: 232, height: 82 },
      }),
    });
    await block();
    expect(screen.getByRole('img', { name: 'Neue leere Seite' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Neue Seite rückgängig' })).toBeEnabled();
    expect(screen.getByText('Seite 4 von 4', { selector: '.pager-label' })).toBeInTheDocument();
    expect(screen.getByText(/bereits signiert/)).toBeInTheDocument();
  });

  it('Live-Region meldet die neue Position', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { mock } = renderApp({ view: positionView() });
      await block();
      act(() => mock.setView(positionView({ placement: { page_index: 2, x: 56.69, y: 200, width: 232, height: 82 }, announce: { left: '20', top: '71' } })));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(500);
      });
      expect(screen.getByTestId('live-region')).toHaveTextContent('Seite 3 von 3, linke Kante 20 mm von links, Oberkante 71 mm von unten');
    } finally {
      vi.useRealTimers();
    }
  });

  it('englisch: Beschriftungen und Zugriffstasten', async () => {
    renderApp({ view: positionView({}, 'en') });
    await screen.findByRole('application', { name: 'Signature' });
    const sign = screen.getByRole('button', { name: 'Sign' });
    expect(sign).toHaveAttribute('aria-keyshortcuts', 'Alt+S');
    const toolbar = document.querySelector('.position-actions') as HTMLElement;
    expect(within(toolbar).getByRole('button', { name: 'Create new Page' })).toBeInTheDocument();
  });
});

describe('clampToPage', () => {
  it('klemmt wie F-35; Block größer als Seite liegt links/oben an (KR-F11)', () => {
    expect(clampToPage(-5, 10, 100, 40, 595, 842)).toEqual({ x: 0, y: 40 });
    expect(clampToPage(600, 900, 100, 40, 595, 842)).toEqual({ x: 495, y: 842 });
    expect(clampToPage(10, 10, 700, 900, 595, 842)).toEqual({ x: 0, y: 842 });
  });
});
