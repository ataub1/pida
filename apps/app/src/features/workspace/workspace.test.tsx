/**
 * Arbeitsbereich (ADR 0004 §2.3/§2.4, §4.3, Abnahme E1 Punkte 3–5): Sticker (Tastatur 15/1/50 pt über den
 * Kern, Ziehen mit Einrasten an den Einrastlinien, Feld-Magnet, Doppelklick, Touch), Live-Region,
 * Überdeckungswarnung über „Signieren“, Signaturfelder, Optionen mit Begründung, Konto-Chip, ARIA, axe;
 * Hinweise auf Erkanntes (Einstellungen › Dokument, E3: PDF/A, QR-Platzhalter, Signaturfeld) samt sofortiger
 * Wirkung der Einstellungen (Mock wie K3).
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { DocumentView, UiEvent } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { emulateMedia, lastEvent, renderApp } from '../../test/render';
import { defaultSettings } from '../../core/defaults';
import { documentView, IOS, onMobile, settingsView, signersView, viewOf, workspaceView } from '../../test/views';
import { layoutPages, PAGE_GAP, PAGE_PAD } from './placement';

const SCALE = 1.15; // eingepasster Maßstab ohne ResizeObserver (jsdom)
const PH = 841.89;

function sticker() {
  return screen.getByRole('application', { name: 'Signaturblock' });
}

/** Seitenspalte und Sticker ohne Layout: Rechtecke für die Zeigerrechnung setzen. */
function stubRects() {
  const host = document.querySelector('.pages') as HTMLElement;
  host.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
  const s = sticker();
  s.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
  return { host, s };
}

/** Zeigerposition (px in der Seitenspalte) für eine Lage in pt auf Seite `page` mit Greifpunkt in der Blockmitte. */
function px(page: number, x: number, yTop: number) {
  const top = PAGE_PAD + page * (PH * SCALE + PAGE_GAP);
  return { clientX: (x + 115) * SCALE, clientY: top + (PH - yTop + 41) * SCALE };
}

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Sticker und Seiten', () => {
  it('zeigt Kacheln, Sticker mit Blockvorschau im Rechteck des Kerns, Hilfe und keine Zahlenfelder', async () => {
    renderApp({ view: workspaceView() });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(s).toHaveAttribute('aria-roledescription', 'verschiebbarer Signaturblock');
    expect(s).toHaveAttribute('tabindex', '0');
    expect(s).toHaveAccessibleDescription(/Pfeiltasten verschieben, Umschalt\+Pfeil verschiebt fein, Alt\+Pfeil grob/);
    // Rechteck des Kerns (PAGE_ABS) → px: links 182,64 pt, Oberkante 300 pt von unten
    const top = layoutPages([[595.28, PH], [595.28, PH], [595.28, PH]], SCALE).tops[2] ?? 0;
    expect(s.style.left).toBe(`${182.64 * SCALE}px`);
    expect(parseFloat(s.style.top)).toBeCloseTo(top + (PH - 300) * SCALE, 3);
    expect(s.querySelector('img')?.getAttribute('src')).toMatch(/^data:image\/svg\+xml/);
    expect(screen.getByRole('img', { name: 'Seite 3 von 3 – Vertrag.pdf' })).toBeInTheDocument();
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    // Lage im Inspektor
    expect(screen.getByText('Seite 3 · unten mittig')).toBeInTheDocument();
  });

  it('Pfeiltasten: nudge_block (Schritte rechnet der Kern: 15/1/50 pt), Bild↑/↓ und Pos1/Ende wechseln die Seite', async () => {
    const { mock } = renderApp({ view: workspaceView() });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    s.focus();
    fireEvent.keyDown(s, { key: 'ArrowRight' });
    fireEvent.keyDown(s, { key: 'ArrowUp', shiftKey: true });
    fireEvent.keyDown(s, { key: 'ArrowLeft', altKey: true });
    fireEvent.keyDown(s, { key: 'ArrowDown', ctrlKey: true });
    fireEvent.keyDown(s, { key: 'PageUp' });
    fireEvent.keyDown(s, { key: 'Home' });
    fireEvent.keyDown(s, { key: 'End' });
    const sent = mock.dispatched.filter((e) => e.type !== 'inspect_pages');
    expect(sent).toEqual([
      { type: 'nudge_block', dx: 1, dy: 0, fine: false, coarse: false },
      { type: 'nudge_block', dx: 0, dy: 1, fine: true, coarse: false },
      { type: 'nudge_block', dx: -1, dy: 0, fine: false, coarse: true },
      { type: 'nudge_block', dx: 0, dy: -1, fine: false, coarse: true },
      { type: 'change_page', page: 1 },
      { type: 'change_page', page: 0 },
    ]);
  });

  it('Ziehen: Einrasten an den Einrastlinien (rechte Kante, unter dem Text), Bänder freier Flächen, move_block beim Loslassen', async () => {
    const { mock } = renderApp({ view: workspaceView() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const { s } = stubRects();
    fireEvent.pointerDown(s, { button: 0, pointerId: 1, pointerType: 'mouse', clientX: 0, clientY: 0 });
    fireEvent.pointerMove(s, { pointerId: 1, pointerType: 'mouse', ...px(2, 311, 324) });
    // während des Ziehens: Einrastlinien und freie Flächen aus `layout`
    expect(document.querySelectorAll('.snap-guide')).toHaveLength(2);
    expect(document.querySelectorAll('.free-band')).toHaveLength(1);
    fireEvent.pointerUp(s, { pointerId: 1, pointerType: 'mouse', ...px(2, 311, 324) });
    const move = mock.dispatched.find((e) => e.type === 'move_block') as Extract<UiEvent, { type: 'move_block' }>;
    expect(move.page).toBe(2);
    expect(move.x).toBeCloseTo(309);
    expect(move.y).toBeCloseTo(322);
    expect(document.querySelectorAll('.snap-guide')).toHaveLength(0);
  });

  it('Ziehen über Seitengrenzen und ohne Layoutdaten (kein Einrasten)', async () => {
    const { mock } = renderApp({ view: workspaceView(documentView({ layout: [] })) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const { s } = stubRects();
    fireEvent.pointerDown(s, { button: 0, pointerId: 1, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(s, { pointerId: 1, ...px(0, 100.5, 700.5) });
    expect(document.querySelectorAll('.snap-guide')).toHaveLength(0);
    fireEvent.pointerUp(s, { pointerId: 1, ...px(0, 100.5, 700.5) });
    const move = mock.dispatched.find((e) => e.type === 'move_block') as Extract<UiEvent, { type: 'move_block' }>;
    expect(move.page).toBe(0);
    expect(move.x).toBeCloseTo(100.5);
    expect(move.y).toBeCloseTo(700.5);
  });

  it('ein zweiter Finger verschiebt nicht; kurzes Antippen ohne Bewegung sendet nichts', async () => {
    const { mock } = renderApp({ view: onMobile(workspaceView()) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const { s } = stubRects();
    fireEvent.pointerDown(s, { button: 0, pointerId: 1, pointerType: 'touch', clientX: 10, clientY: 10 });
    fireEvent.pointerDown(s, { button: 0, pointerId: 2, pointerType: 'touch', clientX: 400, clientY: 400 });
    fireEvent.pointerMove(s, { pointerId: 2, pointerType: 'touch', clientX: 500, clientY: 500 });
    fireEvent.pointerUp(s, { pointerId: 2, pointerType: 'touch', clientX: 500, clientY: 500 });
    fireEvent.pointerUp(s, { pointerId: 1, pointerType: 'touch', clientX: 10, clientY: 10 });
    expect(mock.dispatched.filter((e) => e.type === 'move_block' || e.type === 'use_target')).toEqual([]);
  });

  it('Doppelklick setzt den Block mittig an den Zeiger; Doppeltippen über den Finger (Fingerversatz)', async () => {
    const { mock } = renderApp({ view: workspaceView() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    stubRects();
    const page1 = screen.getByRole('img', { name: 'Seite 1 von 3 – Vertrag.pdf' });
    // Zeiger bei (300 pt, 400 pt von oben) auf Seite 1
    const at = { clientX: 300 * SCALE, clientY: PAGE_PAD + 400 * SCALE };
    fireEvent.doubleClick(page1, at);
    const move = mock.dispatched.find((e) => e.type === 'move_block') as Extract<UiEvent, { type: 'move_block' }>;
    expect(move.page).toBe(0);
    expect(move.x).toBeCloseTo(300 - 115);
    expect(move.y).toBeCloseTo(PH - (400 - 41));
    fireEvent.pointerUp(page1, { pointerType: 'touch', ...at });
    fireEvent.pointerUp(page1, { pointerType: 'touch', ...at });
    const tap = mock.dispatched.filter((e) => e.type === 'move_block')[1] as Extract<UiEvent, { type: 'move_block' }>;
    // Unterkante 12 pt über dem Finger
    expect(tap.y).toBeCloseTo(PH - (400 - 82 - 12));
  });

  it('Live-Region nach einer Änderung des Kerns: Lage mit „von unten“, Einrasten, Prüfergebnis', async () => {
    const { mock } = renderApp({ view: workspaceView() });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    stubRects();
    fireEvent.pointerDown(s, { button: 0, pointerId: 1, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(s, { pointerId: 1, ...px(2, 311, 324) });
    fireEvent.pointerUp(s, { pointerId: 1, ...px(2, 311, 324) });
    const next = documentView({
      revision: 2,
      placement: { page_index: 2, x: 309, y: 322, width: 230, height: 82 },
      sticker: { left: 309, right: 539, top: 322, bottom: 240 },
      check: { for_revision: 2, overlaps_content: false, overlaps_widget: false, clamped: false },
      announce: { page: '3', pages: '3', left: '109', top: '114', bottom: '85' },
    });
    act(() => mock.setView(workspaceView(next)));
    await waitFor(() => expect(screen.getByTestId('live-region')).toHaveTextContent('Seite 3 von 3, 109 mm von links, 85 mm von unten. Rastet ein: rechts unten. Freie Fläche.'));
  });
});

describe('Überdeckung, Felder, Optionen', () => {
  const overlap = (extra: Partial<DocumentView> = {}) => documentView({ check: { for_revision: 1, overlaps_content: true, overlaps_widget: false, clamped: false }, ...extra });

  it('Überdeckung: Rahmen und Warnung direkt über „Signieren“ mit „Freie Fläche suchen“ → auto_place', async () => {
    const { mock, user } = renderApp({ view: workspaceView(overlap()) });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(s).toHaveClass('is-overlap');
    const warn = screen.getByTestId('overlap-warning');
    expect(warn).toHaveTextContent('Der Signaturblock überdeckt Inhalt der Seite.');
    // Reihenfolge: Warnung steht vor dem Fuß mit „Signieren“
    const footer = document.querySelector('.inspector-footer') as HTMLElement;
    expect(warn.compareDocumentPosition(footer) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    await user.click(within(warn).getByRole('button', { name: 'Freie Fläche suchen' }));
    expect(lastEvent(mock)).toEqual({ type: 'auto_place' });
  });

  it('Überdeckung einer älteren Revision wird nicht gezeigt', async () => {
    renderApp({ view: workspaceView(overlap({ revision: 2 })) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('overlap-warning')).not.toBeInTheDocument();
  });

  it('eingeklappter Bottom-Sheet: Konto-Chip, „Signieren“ und die Warnung bleiben sichtbar', async () => {
    restore = emulateMedia({ narrow: true });
    const { user } = renderApp({ view: onMobile(workspaceView(overlap()), IOS) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const sheet = screen.getByRole('complementary', { name: 'Signatur' });
    expect(within(sheet).getByRole('button', { name: /Optionen einblenden/ })).toHaveAttribute('aria-expanded', 'false');
    expect(within(sheet).getByTestId('signer-chip')).toBeVisible();
    expect(within(sheet).getByRole('button', { name: /^Signieren/ })).toBeInTheDocument();
    expect(within(sheet).getByTestId('overlap-warning')).toBeInTheDocument();
    expect(within(sheet).queryByRole('switch', { name: 'Unsichtbar signieren' })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: /Optionen einblenden/ }));
    expect(within(sheet).getByRole('switch', { name: 'Unsichtbar signieren' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('leeres Signaturfeld: „Hier signieren“-Markierung und Hinweisleiste → use_target (auch ohne Platzhaltersuche)', async () => {
    const doc = documentView({ fields: [{ index: 0, name: 'Unterschrift_Antragsteller', page: 1, rect: { left: 299, right: 539, top: 300, bottom: 214 }, selectable: true }] });
    const { mock, user } = renderApp({ view: workspaceView(doc) });
    await user.click(await screen.findByRole('button', { name: 'Hier signieren: Signaturfeld „Unterschrift_Antragsteller“' }));
    expect(lastEvent(mock)).toEqual({ type: 'use_target', target: { kind: 'field', index: 0 } });
    await user.click(screen.getByRole('button', { name: 'Ins Feld setzen' }));
    expect(lastEvent(mock)).toEqual({ type: 'use_target', target: { kind: 'field', index: 0 } });
  });

  it('Ziehen ins Feld rastet im Feld ein → use_target', async () => {
    const doc = documentView({ fields: [{ index: 0, name: 'Feld', page: 2, rect: { left: 300, right: 540, top: 200, bottom: 114 }, selectable: true }] });
    const { mock } = renderApp({ view: workspaceView(doc) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const { s } = stubRects();
    fireEvent.pointerDown(s, { button: 0, pointerId: 1, clientX: 0, clientY: 0 });
    // Blockmitte bei (420, 157) liegt im Feld
    fireEvent.pointerMove(s, { pointerId: 1, ...px(2, 420 - 115, 157 + 41) });
    expect(s).toHaveClass('is-in-field');
    fireEvent.pointerUp(s, { pointerId: 1, ...px(2, 420 - 115, 157 + 41) });
    expect(mock.dispatched.filter((e) => e.type === 'use_target')).toEqual([{ type: 'use_target', target: { kind: 'field', index: 0 } }]);
  });

  it('im Feld: Plakette „Im Signaturfeld“, Lage „Im Signaturfeld …“ und „Frei platzieren“', async () => {
    const doc = documentView({
      target: { kind: 'field', index: 0 },
      origin: 'field',
      fields: [{ index: 0, name: 'Feld', page: 2, rect: { left: 300, right: 540, top: 200, bottom: 114 }, selectable: true }],
      sticker: { left: 300, right: 540, top: 200, bottom: 114 },
    });
    const { mock, user } = renderApp({ view: workspaceView(doc) });
    expect(await screen.findByText('Im Signaturfeld „Feld“')).toBeInTheDocument();
    expect(sticker()).toHaveClass('is-in-field');
    expect(screen.queryByRole('button', { name: /Hier signieren/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Frei platzieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'use_target', target: { kind: 'free' } });
  });

  it('Hinweisleiste erscheint bzw. verschwindet (Block aus dem Feld gezogen bzw. ins Feld gesetzt): Bildlauf um ihre Höhe ausgeglichen', async () => {
    const field = { index: 0, name: 'Feld', page: 2, rect: { left: 300, right: 540, top: 200, bottom: 114 }, selectable: true };
    const inField = documentView({ target: { kind: 'field', index: 0 }, origin: 'field', fields: [field], sticker: field.rect });
    const free = documentView({ fields: [field] });
    // jsdom ohne Layout: die Leiste ist 132 px hoch (Telefon, drei Zeilen und Knopf)
    const original = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function (this: Element) {
      const h = this.classList.contains('canvas-banner') ? 132 : 0;
      return { left: 0, top: 0, right: 0, bottom: h, width: 0, height: h, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
    };
    try {
      const { mock } = renderApp({ view: workspaceView(inField) });
      expect(await screen.findByText('Im Signaturfeld „Feld“')).toBeInTheDocument();
      const scroller = document.querySelector('.canvas-scroll') as HTMLElement;
      scroller.scrollTop = 40;
      act(() => mock.setView(workspaceView(free)));
      expect(await screen.findByText('Signaturfeld gefunden:')).toBeInTheDocument();
      expect(scroller.scrollTop).toBe(40 + 132);
      act(() => mock.setView(workspaceView(inField)));
      await waitFor(() => expect(screen.queryByText('Signaturfeld gefunden:')).not.toBeInTheDocument());
      expect(scroller.scrollTop).toBe(40);
    } finally {
      Element.prototype.getBoundingClientRect = original;
    }
  });

  it('QR-Platzhalter: automatisch positioniert (Hinweisleiste)', async () => {
    const doc = documentView({ target: { kind: 'qr' }, origin: 'qr', qr: { page: 2, id: '1', rect: { left: 309, right: 539, top: 300, bottom: 218 } } });
    renderApp({ view: workspaceView(doc) });
    expect(await screen.findByText('Positioniert im QR-Code-Platzhalter des Dokuments.')).toBeInTheDocument();
    expect(screen.getByText('Im QR-Code-Platzhalter')).toBeInTheDocument();
  });

  it('Optionen: „Unsichtbar“ als Schalter (set_invisible), „Auf neuer Seite“ (toggle_new_page)', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    await user.click(await screen.findByRole('switch', { name: 'Unsichtbar signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'set_invisible', invisible: true });
    await user.click(screen.getByRole('switch', { name: 'Auf neuer Seite' }));
    expect(lastEvent(mock)).toEqual({ type: 'toggle_new_page' });
  });

  it('„Auf neuer Seite“ bei signierten bzw. zertifizierten Dokumenten gesperrt mit Begründung des Kerns', async () => {
    const doc = documentView({
      doc: { ...documentView().doc, signed: true },
      constraints: { new_page_allowed: false, visible_allowed: true, new_visible_field_allowed: true, reasons: ['ng-constraint-signed'] },
      existing: [{ field_name: 'Signature1', signer: 'Dr. Anna Gruber', signing_time: '2026-10-12T14:03:51+02:00', status: 'valid', page: 1, rect: null }],
    });
    renderApp({ view: workspaceView(doc) });
    const sw = await screen.findByRole('switch', { name: 'Auf neuer Seite' });
    expect(sw).toBeDisabled();
    expect(sw).toHaveAccessibleDescription('Das Dokument ist bereits signiert; eine neue Seite würde die vorhandenen Signaturen verändern.');
    // vorhandene Signaturen ohne „gültig“ (§8 F3)
    const list = screen.getByRole('heading', { name: 'Vorhandene Signaturen' }).closest('section') as HTMLElement;
    expect(list).toHaveTextContent('Dr. Anna Gruber');
    expect(list).toHaveTextContent('intakt');
    expect(list).not.toHaveTextContent(/gültig/);
  });

  it('unsichtbares Profil: Schalter fest an mit Begründung; kein Sticker, Hinweisleiste', async () => {
    const doc = documentView({
      invisible: true,
      sticker: null,
      constraints: { new_page_allowed: true, visible_allowed: false, new_visible_field_allowed: true, reasons: ['ng-constraint-invisible-profile'] },
    });
    renderApp({ view: workspaceView(doc) });
    const sw = await screen.findByRole('switch', { name: 'Unsichtbar signieren' });
    expect(sw).toBeChecked();
    expect(sw).toBeDisabled();
    expect(sw).toHaveAccessibleDescription(/unsichtbar \(Einstellungen › Signaturbild\)/);
    expect(screen.queryByRole('application')).not.toBeInTheDocument();
    expect(screen.getByText('Unsichtbare Signatur – auf den Seiten erscheint kein Signaturbild.')).toBeInTheDocument();
  });

  it('Analysewarnung: Seiten sichtbar, Inspektor mit „Trotzdem signieren“/„Dokument verwerfen“', async () => {
    const doc = documentView({ phase: 'warning' });
    const view = viewOf({ step: 'warning', doc: doc.doc, codes: ['pdf-usage-rights'] }, { document: doc, signers: signersView() });
    const { mock, user } = renderApp({ view });
    expect(await screen.findByRole('alert')).toHaveTextContent('erweiterte Adobe-Reader-Rechte');
    expect(sticker()).toHaveAttribute('tabindex', '-1');
    expect(screen.queryByRole('button', { name: /^Signieren/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Trotzdem signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'answer_warning', proceed: true });
  });
});

describe('Konto-Chip und „Signieren“', () => {
  it('„Signieren“ mit gewähltem Signiermittel → sign_with (keine Kombination confirm_position/choose_backend)', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    await user.click(await screen.findByRole('button', { name: /^Signieren/ }));
    expect(mock.dispatched.filter((e) => e.type !== 'inspect_pages')).toEqual([{ type: 'sign_with', backend: 'atrust_native' }]);
  });

  it('Kurzhinweis A-Trust am Konto-Chip (ID Austria), nicht beim Keystore', async () => {
    const { unmount } = renderApp({ view: workspaceView() });
    expect(await screen.findByTestId('privacy-note')).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt.');
    unmount();
    renderApp({ view: workspaceView(documentView(), { signers: signersView({ selected: 'keystore' }) }) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('privacy-note')).not.toBeInTheDocument();
  });

  it('Wechsel im Menü → select_signer (startet nichts); „Als Standard verwenden“ → make_default', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    await user.click(await screen.findByRole('button', { name: 'Signiermittel: ID Austria · +43 660 ···123. Wechseln' }));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: /Keystore · max\.p12 · max/ })).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: /Chipkarte/ })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: /Keystore/ }));
    expect(lastEvent(mock)).toEqual({ type: 'select_signer', backend: 'keystore', make_default: false });
    expect(mock.dispatched.some((e) => e.type === 'sign_with')).toBe(false);
    act(() => mock.setView(workspaceView(documentView(), { signers: signersView({ selected: 'keystore' }) })));
    await user.click(screen.getByRole('button', { name: /Signiermittel: Keystore/ }));
    await user.click(await screen.findByRole('menuitem', { name: 'Als Standard verwenden' }));
    expect(lastEvent(mock)).toEqual({ type: 'select_signer', backend: 'keystore', make_default: true });
  });

  it('„Einstellungen …“ im Menü öffnet die Einstellungen beim Abschnitt Signiermittel', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    await user.click(await screen.findByTestId('signer-chip'));
    const menu = await screen.findByRole('menu');
    expect(within(menu).queryByRole('menuitem', { name: /Konten verwalten/ })).not.toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Einstellungen …' }));
    expect(lastEvent(mock)).toEqual({ type: 'open_settings' });
    act(() => mock.setView({ ...workspaceView(), settings: settingsView().settings }));
    // Einstellungen (B3): Abschnitt „Signiermittel“ mit dem ID-Austria-Konto
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Signiermittel' })).toBeInTheDocument();
    expect(within(dialog).getByRole('heading', { name: 'ID Austria' })).toBeInTheDocument();
  });

  it('ohne Auswahl („Jedes Mal fragen“): „Signieren“ öffnet das Menü, die Wahl dort signiert (sign_with)', async () => {
    const view = workspaceView(documentView(), { signers: signersView({ selected: null, default: null }) });
    const { mock, user } = renderApp({ view });
    expect(await screen.findByRole('button', { name: 'Signiermittel: Signiermittel wählen. Wechseln' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const menu = await screen.findByRole('menu');
    expect(menu).toHaveTextContent('Signieren mit');
    expect(mock.dispatched.some((e) => e.type === 'sign_with')).toBe(false);
    await user.click(within(menu).getByRole('menuitem', { name: /ID Austria/ }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_with', backend: 'atrust_native' });
  });

  it('nicht verfügbares Signiermittel: im Menü gesperrt mit Grund des Kerns', async () => {
    const s = signersView();
    const options = s.options.map((o) => (o.backend === 'local_bku' ? { ...o, available: false, capabilities: null, unavailable_reason: 'ng-signer-localbku-not-running' } : o));
    const { user } = renderApp({ view: workspaceView(documentView(), { signers: { ...s, options } }) });
    await user.click(await screen.findByTestId('signer-chip'));
    const item = await screen.findByRole('menuitem', { name: /Chipkarte/ });
    expect(item).toHaveAttribute('aria-disabled', 'true');
    expect(item).toHaveTextContent('Die Chipkarten-Software (lokale BKU) läuft nicht.');
  });

  it('⌘/Strg+↵ signiert (ausdrückliche Aktion), nicht ohne Freigabe', async () => {
    const { mock, user, unmount } = renderApp({ view: workspaceView() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.keyboard('{Control>}{Enter}{/Control}');
    expect(mock.dispatched.filter((e) => e.type === 'sign_with')).toEqual([{ type: 'sign_with', backend: 'atrust_native' }]);
    unmount();
    const r = renderApp({ view: workspaceView(documentView(), { actions: { ...workspaceView().actions, sign: false } }) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await r.user.keyboard('{Control>}{Enter}{/Control}');
    expect(r.mock.dispatched.filter((e) => e.type === 'sign_with')).toEqual([]);
    expect(screen.getByRole('button', { name: /^Signieren/ })).toBeDisabled();
  });

  it('„Bearbeiten“ am Signaturbild öffnet die Einstellungen (Abschnitt Signaturbild)', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    await user.click(await screen.findByRole('button', { name: 'Signaturbild bearbeiten' }));
    expect(lastEvent(mock)).toEqual({ type: 'open_settings' });
  });
});

describe('Seitenansicht, Zoom und Barrierefreiheit', () => {
  it('Werkzeugleiste: Zoom-Stufen, An Breite anpassen, Seitenleiste; ⌘/Strg+Plus zoomt', async () => {
    const { user } = renderApp({ view: workspaceView() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const toolbar = screen.getByRole('toolbar', { name: 'Ansicht' });
    expect(within(toolbar).getByLabelText('Zoom 86 %')).toBeInTheDocument();
    await user.click(within(toolbar).getByRole('button', { name: 'Vergrößern' }));
    expect(within(toolbar).getByLabelText('Zoom 90 %')).toBeInTheDocument();
    await user.keyboard('{Control>}={/Control}');
    expect(within(toolbar).getByLabelText('Zoom 100 %')).toBeInTheDocument();
    await user.click(within(toolbar).getByRole('button', { name: 'An Breite anpassen' }));
    expect(within(toolbar).getByLabelText('Zoom 86 %')).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Seiten' })).toBeInTheDocument();
    await user.click(within(toolbar).getByRole('button', { name: 'Seitenleiste ein-/ausblenden' }));
    expect(screen.queryByRole('navigation', { name: 'Seiten' })).not.toBeInTheDocument();
  });

  it('sichtbare Seiten gehen als inspect_pages an den Kern', async () => {
    const { mock } = renderApp({ view: workspaceView() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await waitFor(() => expect(mock.dispatched.some((e) => e.type === 'inspect_pages')).toBe(true));
    const ev = mock.dispatched.find((e) => e.type === 'inspect_pages') as Extract<UiEvent, { type: 'inspect_pages' }>;
    expect(ev.pages.length).toBeGreaterThan(0);
    expect(ev.pages.length).toBeLessThanOrEqual(8);
  });

  it('axe: Arbeitsbereich, Felder, Überdeckung, Englisch', async () => {
    const doc = documentView({
      fields: [{ index: 0, name: 'Feld', page: 1, rect: { left: 300, right: 540, top: 200, bottom: 114 }, selectable: true }],
      check: { for_revision: 1, overlaps_content: true, overlaps_widget: false, clamped: false },
    });
    const { unmount } = renderApp({ view: workspaceView(doc) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await expectNoAxeViolations();
    unmount();
    renderApp({ view: workspaceView(documentView(), {}, 'en') });
    expect(await screen.findByRole('application', { name: 'Signature block' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});

describe('Erkennung beim Öffnen: PDF/A und Platzhalter (Einstellungen › Dokument, E3)', () => {
  const inspector = () => screen.getByRole('complementary', { name: 'Signatur' });
  const FIELD = 'Unterschrift_Antragsteller';
  const withSearch = (kind: 'marker' | 'fields', search = true) => ({ ...defaultSettings(), placeholder: { search, kind } });

  async function documentSettings(user: ReturnType<typeof renderApp>['user']) {
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Dokument' }));
    return dialog;
  }

  async function closeSettings(user: ReturnType<typeof renderApp>['user']) {
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  }

  it('nennt ein erkanntes PDF/A-Dokument bzw. die erzwungene PDF/A-Option (sofort wirksam)', async () => {
    const { unmount } = renderApp({ scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(within(inspector()).getByText('PDF/A-Dokument erkannt – wird PDF/A-konform signiert')).toBeInTheDocument();
    expect(within(inspector()).queryByText(/erzwungen/)).not.toBeInTheDocument();
    unmount();

    const r = renderApp({ scenario: 'blank' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(within(inspector()).queryByText(/PDF\/A/)).not.toBeInTheDocument();
    const dialog = await documentSettings(r.user);
    await r.user.click(within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' }));
    await closeSettings(r.user);
    expect(within(inspector()).getByText('PDF/A-kompatible Signatur (erzwungen)')).toBeInTheDocument();
  });

  it('„Leere Signaturfelder“: Signatur beim Öffnen im ersten leeren Feld, Hinweis und Sprechblase nennen das Ziel', async () => {
    const { user } = renderApp({ scenario: 'field', settings: withSearch('fields') });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(within(inspector()).getByText(`Im Signaturfeld „${FIELD}“`)).toBeInTheDocument();
    expect(within(inspector()).getByText('Signaturfeld gefunden – Signatur im Feld platziert')).toBeInTheDocument();
    expect(sticker()).toHaveClass('is-in-field');
    expect(screen.getByText('Signaturblock automatisch im Signaturfeld platziert')).toBeInTheDocument();
    // Feld belegt: keine Hinweisleiste, keine „Hier signieren“-Markierung
    expect(screen.queryByText('Signaturfeld gefunden:')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Hier signieren/ })).not.toBeInTheDocument();
    await expectNoAxeViolations();

    // „Frei platzieren“: der Hinweis entfällt, das Feld wird wieder angeboten
    await user.click(within(inspector()).getByRole('button', { name: 'Frei platzieren' }));
    await waitFor(() => expect(within(inspector()).queryByText(/Signaturfeld gefunden/)).not.toBeInTheDocument());
    expect(screen.getByRole('button', { name: `Hier signieren: Signaturfeld „${FIELD}“` })).toBeInTheDocument();
    // von Hand ins Feld: Lage genannt, aber nicht als automatisch erkannt
    await user.click(screen.getByRole('button', { name: 'Ins Feld setzen' }));
    expect(await within(inspector()).findByText(`Im Signaturfeld „${FIELD}“`)).toBeInTheDocument();
    expect(within(inspector()).queryByTestId('detect-note')).not.toBeInTheDocument();
  });

  it('Umschalten in den Einstellungen wirkt sofort; ohne Suche wird das Feld nur angeboten', async () => {
    const { user } = renderApp({ scenario: 'field' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    // Standard (Suche aus): das Signaturfeld wird nur angeboten
    expect(screen.getByText('Signaturfeld gefunden:')).toBeInTheDocument();
    expect(within(inspector()).getByText(/^Seite \d · /)).toBeInTheDocument();
    expect(within(inspector()).queryByTestId('detect-note')).not.toBeInTheDocument();

    let dialog = await documentSettings(user);
    await user.click(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' }));
    await user.click(within(dialog).getByRole('radio', { name: /Leere Signaturfelder/ }));
    await closeSettings(user);
    expect(await within(inspector()).findByText('Signaturfeld gefunden – Signatur im Feld platziert')).toBeInTheDocument();
    expect(screen.getByText('Signaturblock automatisch im Signaturfeld platziert')).toBeInTheDocument();
    expect(screen.queryByText('Signaturfeld gefunden:')).not.toBeInTheDocument();

    dialog = await documentSettings(user);
    await user.click(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' }));
    await closeSettings(user);
    await waitFor(() => expect(within(inspector()).queryByText(/Signaturfeld gefunden/)).not.toBeInTheDocument());
    expect(within(inspector()).getByText(/^Seite \d · /)).toBeInTheDocument();
    // Markierung und Hinweisleiste bleiben als Angebot
    expect(screen.getByText('Signaturfeld gefunden:')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `Hier signieren: Signaturfeld „${FIELD}“` })).toBeInTheDocument();
  });

  it('von Hand platziert bleibt beim Umschalten der Platzhaltersuche liegen', async () => {
    const { user, mock } = renderApp({ scenario: 'field' });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    s.focus();
    await user.keyboard('{ArrowLeft}');
    await waitFor(() => expect(mock.dispatched.some((e) => e.type === 'nudge_block')).toBe(true));
    const before = (await mock.view()).document?.placement;
    const dialog = await documentSettings(user);
    await user.click(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' }));
    await user.click(within(dialog).getByRole('radio', { name: /Leere Signaturfelder/ }));
    await closeSettings(user);
    const after = (await mock.view()).document;
    expect(after?.target).toEqual({ kind: 'free' });
    expect(after?.placement).toEqual(before);
    expect(within(inspector()).queryByTestId('detect-note')).not.toBeInTheDocument();
    expect(screen.getByText('Signaturfeld gefunden:')).toBeInTheDocument();
  });

  it('QR-Code-Platzhalter: automatisch genutzt, nicht bei „Leere Signaturfelder“ oder ausgeschalteter Suche', async () => {
    const open = async () => {
      const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
      await waitFor(() => expect(within(list).queryAllByText('Wird geprüft')).toHaveLength(0));
      await userOf.current!.click(screen.getByRole('button', { name: '„Auftragsbestaetigung_4711.pdf“ öffnen und positionieren' }));
      await screen.findByRole('application', { name: 'Signaturblock' });
    };
    const userOf: { current: ReturnType<typeof renderApp>['user'] | null } = { current: null };
    let r = renderApp({ scenario: 'batch', settings: withSearch('marker') });
    userOf.current = r.user;
    await open();
    expect(within(inspector()).getByText('Im QR-Code-Platzhalter')).toBeInTheDocument();
    expect(within(inspector()).getByText('QR-Platzhalter gefunden – Signatur dort platziert')).toBeInTheDocument();
    expect(screen.getByText('Signaturblock automatisch im QR-Platzhalter platziert')).toBeInTheDocument();
    r.unmount();

    for (const settings of [withSearch('fields'), withSearch('marker', false)]) {
      r = renderApp({ scenario: 'batch', settings });
      userOf.current = r.user;
      await open();
      expect(within(inspector()).queryByText(/QR-Platzhalter gefunden/)).not.toBeInTheDocument();
      expect(screen.getByText('Signaturblock automatisch auf freier Fläche platziert')).toBeInTheDocument();
      r.unmount();
    }
  });
});

describe('Automatische Positionierung (Einstellung, ADR 0004 Nachtrag 8)', () => {
  const inspector = () => screen.getByRole('complementary', { name: 'Signatur' });
  const FIELD = { index: 0, name: 'Unterschrift_Antragsteller', page: 1, rect: { left: 299, right: 539, top: 300, bottom: 214 }, selectable: true };
  const lockedDoc = (extra: Partial<DocumentView> = {}) =>
    documentView({ placement_locked: true, detection: { pdfa: null, pdfa_decision: 'none', auto_placed: true }, ...extra });
  const SESSION: UiEvent['type'][] = ['sign_with', 'choose_backend', 'retry', 'confirm_position', 'choose_invisible', 'start_batch'];
  const moves = (dispatched: UiEvent[]) => dispatched.filter((e) => ['move_block', 'nudge_block', 'change_page', 'use_target', 'auto_place'].includes(e.type));
  const withAutoPosition = () => ({ ...defaultSettings(), signature: { ...defaultSettings().signature, auto_position: true } });

  it('gesperrter Block: kein Ziehen, keine Tastatur, kein Doppelklick, keine Feld-Angebote; Fokus auf „Signieren“; axe', async () => {
    const { mock } = renderApp({ view: workspaceView(lockedDoc({ fields: [FIELD] })) });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(s).toHaveAttribute('aria-roledescription', 'automatisch platzierter Signaturblock');
    expect(s).toHaveAttribute('aria-disabled', 'true');
    expect(s).toHaveAttribute('tabindex', '-1');
    expect(s).toHaveClass('is-locked');
    expect(s).toHaveAccessibleDescription(/Automatisch platziert laut Einstellung „Automatische Positionierung“.*„Diesmal selbst platzieren“/);
    // nach dem Öffnen bereit: Fokus auf „Signieren“
    await waitFor(() => expect(screen.getByRole('button', { name: /^Signieren/ })).toHaveFocus());
    stubRects();
    fireEvent.pointerDown(s, { button: 0, pointerId: 1, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(s, { pointerId: 1, ...px(2, 311, 324) });
    fireEvent.pointerUp(s, { pointerId: 1, ...px(2, 311, 324) });
    fireEvent.keyDown(s, { key: 'ArrowRight' });
    fireEvent.keyDown(s, { key: 'PageUp' });
    fireEvent.doubleClick(screen.getByRole('img', { name: 'Seite 2 von 3 – Vertrag.pdf' }), px(1, 200, 400));
    expect(moves(mock.dispatched)).toEqual([]);
    // leeres Feld: weder Markierung noch Hinweisleiste „Ins Feld setzen“
    expect(screen.queryByRole('button', { name: /Hier signieren/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Ins Feld setzen' })).not.toBeInTheDocument();
    // Inspektor: „Automatisch (laut Einstellung)“, kein „Neu platzieren“
    expect(within(inspector()).getByTestId('placement-auto')).toHaveTextContent('Automatisch (laut Einstellung)');
    expect(within(inspector()).getByText('Bereit zum Signieren. Die Lage bestimmt die Einstellung „Automatische Positionierung“.')).toBeInTheDocument();
    expect(within(inspector()).queryByRole('button', { name: 'Neu platzieren' })).not.toBeInTheDocument();
    expect(mock.dispatched.filter((e) => SESSION.includes(e.type))).toEqual([]);
    await expectNoAxeViolations();
  });

  it('„Unsichtbar signieren“ und „Auf neuer Seite“ bleiben bedienbar; „Diesmal selbst platzieren“ → unlock_placement', async () => {
    const { mock, user } = renderApp({ view: workspaceView(lockedDoc()) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByRole('switch', { name: 'Unsichtbar signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'set_invisible', invisible: true });
    await user.click(screen.getByRole('switch', { name: 'Auf neuer Seite' }));
    expect(lastEvent(mock)).toEqual({ type: 'toggle_new_page' });
    await user.click(within(inspector()).getByRole('button', { name: 'Diesmal selbst platzieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'unlock_placement' });
    expect(mock.dispatched.filter((e) => SESSION.includes(e.type))).toEqual([]);
  });

  it('Überdeckung bei gesperrtem Block: „Diesmal selbst platzieren“ statt „Freie Fläche suchen“', async () => {
    const doc = lockedDoc({ check: { for_revision: 1, overlaps_content: true, overlaps_widget: false, clamped: false } });
    const { mock, user } = renderApp({ view: workspaceView(doc) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const warn = screen.getByTestId('overlap-warning');
    expect(within(warn).queryByRole('button', { name: 'Freie Fläche suchen' })).not.toBeInTheDocument();
    await user.click(within(warn).getByRole('button', { name: 'Diesmal selbst platzieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'unlock_placement' });
  });

  it('Option aus: wie bisher (verschiebbar, „Neu platzieren“, kein Fokussprung auf „Signieren“)', async () => {
    const { mock } = renderApp({ view: workspaceView() });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(s).toHaveAttribute('tabindex', '0');
    expect(s).not.toHaveClass('is-locked');
    expect(within(inspector()).queryByTestId('placement-auto')).not.toBeInTheDocument();
    expect(within(inspector()).queryByRole('button', { name: 'Diesmal selbst platzieren' })).not.toBeInTheDocument();
    expect(within(inspector()).getByRole('button', { name: 'Neu platzieren' })).toBeInTheDocument();
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.getByRole('button', { name: /^Signieren/ })).not.toHaveFocus();
    fireEvent.keyDown(s, { key: 'ArrowRight' });
    expect(moves(mock.dispatched)).toEqual([{ type: 'nudge_block', dx: 1, dy: 0, fine: false, coarse: false }]);
  });

  it('Mock wie der Kern: Option an → gesperrt; „Diesmal selbst platzieren“ gibt nur dieses Dokument frei', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', settings: withAutoPosition() });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(s).toHaveAttribute('aria-disabled', 'true');
    await waitFor(() => expect(screen.getByRole('button', { name: /^Signieren/ })).toHaveFocus());
    const before = (await mock.view()).document!;
    expect(before.placement_locked).toBe(true);
    expect(before.detection?.auto_placed).toBe(true);
    // Verschieben wird vom Kern ignoriert (Mock folgt): auch ein ausdrücklich gesendetes Ereignis
    await mock.dispatch({ type: 'move_block', page: 0, x: 40, y: 500 });
    expect((await mock.view()).document!.placement).toEqual(before.placement);
    await user.click(within(inspector()).getByRole('button', { name: 'Diesmal selbst platzieren' }));
    await waitFor(() => expect(sticker()).toHaveAttribute('tabindex', '0'));
    expect(sticker()).toHaveAttribute('aria-roledescription', 'verschiebbarer Signaturblock');
    expect(within(inspector()).getByRole('button', { name: 'Neu platzieren' })).toBeInTheDocument();
    sticker().focus();
    fireEvent.keyDown(sticker(), { key: 'ArrowLeft' });
    await waitFor(async () => expect((await mock.view()).document!.placement.x).toBeLessThan(before.placement.x));
    expect((await mock.view()).document!.placement_locked).toBe(false);
    expect(mock.dispatched.filter((e) => SESSION.includes(e.type))).toEqual([]);
  });

  it('Mock wie der Kern: Einstellung sofort wirksam – aus gibt den Block frei, an sperrt ihn wieder', async () => {
    const { user } = renderApp({ scenario: 'contract', settings: withAutoPosition() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(sticker()).toHaveAttribute('aria-disabled', 'true');
    const toggleSetting = async () => {
      await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
      const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
      await user.click(within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Dokument' }));
      await user.click(within(dialog).getByRole('switch', { name: 'Automatische Positionierung' }));
      await user.keyboard('{Escape}');
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    };
    await toggleSetting();
    await waitFor(() => expect(sticker()).toHaveAttribute('tabindex', '0'));
    expect(within(inspector()).queryByTestId('placement-auto')).not.toBeInTheDocument();
    await toggleSetting();
    await waitFor(() => expect(sticker()).toHaveAttribute('aria-disabled', 'true'));
    expect(within(inspector()).getByTestId('placement-auto')).toBeInTheDocument();
  });

  it('englisch und axe', async () => {
    renderApp({ view: workspaceView(lockedDoc(), {}, 'en') });
    const s = await screen.findByRole('application', { name: 'Signature block' });
    expect(s).toHaveAttribute('aria-roledescription', 'automatically placed signature block');
    expect(within(screen.getByRole('complementary', { name: 'Signature' })).getByText('Automatic (per setting)')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Place it myself this time' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});
