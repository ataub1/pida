/**
 * Mehrfachauswahl (ADR 0004 Nachtrag 17, Fehler 2 der Abnahme): jeder Einstieg in die Dateiauswahl – Startseite
 * („PDF auswählen“, ⌘/Strg+O), Befehlspalette, Stapel „PDF hinzufügen“, Miniaturleiste, „+ PDF › Weitere PDFs
 * hinzufügen“, Rückfrage bei geparkter Arbeit, Mobile-Startseite – öffnet den Dialog mit Mehrfachauswahl und schickt
 * **alle** gewählten Pfade in der Reihenfolge der Auswahl an den Kern. Die Hinweiszeile des Dialogs und die Zeile
 * auf der Startseite sagen je Plattform, wie mehrere markiert werden (macOS: ein Doppelklick übernimmt nur die
 * angeklickte Datei). Was der Kern nicht übernimmt (schon offen), meldet er; nicht kopierbare Dateien der
 * Mobile-Auswahl meldet die Oberfläche.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { UiEvent } from '../core/types';
import { expectNoAxeViolations } from '../test/a11y';
import { emulateMedia, renderApp } from '../test/render';
import { ANDROID, batchData, batchItem, batchOverview, documentView, editActions, onMobile, viewOf, workspaceView } from '../test/views';
import { noActions } from '../core/defaults';

const DIR = '/Users/demo/Dokumente';
const PICKED = [`${DIR}/Rechnung.pdf`, `${DIR}/Angebot.pdf`, `${DIR}/Vertrag.pdf`];
const MAC_TITLE = 'PDF-Dokumente wählen – mehrere mit ⌘-Klick markieren, dann „Öffnen“';
const START = { level: 1, name: 'Was möchten Sie signieren?' } as const;

const opened = (events: UiEvent[]) => events.filter((e) => e.type === 'open_files');

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Mehrfachauswahl: alle Einstiege schicken alle Pfade', () => {
  it('Startseite „PDF auswählen“: Dialog mit Mehrfachauswahl und Hinweiszeile, alle drei als Stapel in Auswahlreihenfolge', async () => {
    const { mock, user } = renderApp({ pickFilesResult: PICKED });
    await screen.findByRole('heading', START);
    expect(screen.getByText('Mehrere PDFs auf einmal: im Dialog mit ⌘-Klick markieren, dann „Öffnen“.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    expect(await screen.findByRole('heading', { level: 1, name: '3 Dokumente' })).toBeInTheDocument();
    expect(opened(mock.dispatched)).toEqual([{ type: 'open_files', paths: PICKED }]);
    expect(mock.pickFilesCalls).toEqual([{ title: MAC_TITLE, multiple: true }]);
    const list = screen.getByRole('list', { name: 'Dokumente im Stapel' });
    const names = within(list)
      .getAllByRole('listitem', { hidden: true })
      .map((r) => ['Rechnung.pdf', 'Angebot.pdf', 'Vertrag.pdf'].find((n) => within(r).queryByText(n)))
      .filter(Boolean);
    expect(names).toEqual(['Rechnung.pdf', 'Angebot.pdf', 'Vertrag.pdf']);
    await expectNoAxeViolations();
  });

  it('⌘/Strg+O und Befehlspalette „Dokument öffnen …“', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'open', queue: [] }, { actions: editActions({ sign: false, edit_placement: false }) }), pickFilesResult: PICKED });
    await screen.findByRole('heading', START);
    await user.keyboard('{Control>}o{/Control}');
    await waitFor(() => expect(opened(mock.dispatched)).toEqual([{ type: 'open_files', paths: PICKED }]));
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    await user.type(within(palette).getByRole('searchbox', { name: 'Befehl suchen …' }), 'Dokument öffnen');
    await user.keyboard('{ArrowDown}{Enter}');
    await waitFor(() => expect(opened(mock.dispatched)).toHaveLength(2));
    expect(opened(mock.dispatched)[1]).toEqual({ type: 'open_files', paths: PICKED });
    expect(mock.pickFilesCalls.map((c) => c.multiple)).toEqual([true, true]);
  });

  it('Stapel „PDF hinzufügen“, Miniaturleiste „PDF hinzufügen“ und „+ PDF › Weitere PDFs hinzufügen …“', async () => {
    const batch = renderApp({ view: batchOverview(batchData({ items: [batchItem()] })), pickFilesResult: PICKED });
    await batch.user.click(await screen.findByRole('button', { name: 'PDF hinzufügen' }));
    await waitFor(() => expect(opened(batch.mock.dispatched)).toEqual([{ type: 'open_files', paths: PICKED }]));
    expect(batch.mock.pickFilesCalls).toEqual([{ title: MAC_TITLE, multiple: true }]);
    batch.unmount();

    const ws = renderApp({ view: workspaceView(documentView()), pickFilesResult: PICKED });
    await ws.user.click(await screen.findByTestId('thumbs-add'));
    await waitFor(() => expect(opened(ws.mock.dispatched)).toEqual([{ type: 'open_files', paths: PICKED, mode: 'add' }]));
    await ws.user.click(screen.getByRole('button', { name: 'PDF hinzufügen oder öffnen' }));
    const menu = await screen.findByRole('menu', { name: 'PDF hinzufügen oder öffnen' });
    await ws.user.click(within(menu).getByRole('menuitem', { name: 'Weitere PDFs hinzufügen …' }));
    await waitFor(() => expect(opened(ws.mock.dispatched)).toHaveLength(2));
    expect(opened(ws.mock.dispatched)[1]).toEqual({ type: 'open_files', paths: PICKED, mode: 'add' });
    expect(ws.mock.pickFilesCalls).toEqual([
      { title: MAC_TITLE, multiple: true },
      { title: MAC_TITLE, multiple: true },
    ]);
  });

  it('geparkte Arbeit: Rückfrage mit der Zahl, beide Antworten schicken alle Pfade', async () => {
    const parked = viewOf(
      { step: 'open', queue: [] },
      { parked: { doc_name: 'Vertrag.pdf', documents: 1, placed_by_hand: false, unsaved: false }, actions: { ...noActions(), open_files: true, home: true } },
    );
    const { mock, user } = renderApp({ view: parked, pickFilesResult: PICKED });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    let dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    expect(dialog).toHaveTextContent('Sollen die 3 gewählten Dokumente dazukommen?');
    await user.click(within(dialog).getByRole('button', { name: 'Offene schließen und neu beginnen' }));
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Zum Stapel hinzufügen' }));
    await waitFor(() =>
      expect(opened(mock.dispatched)).toEqual([
        { type: 'open_files', paths: PICKED, mode: 'replace' },
        { type: 'open_files', paths: PICKED, mode: 'add' },
      ]),
    );
  });

  it('„Anderes PDF öffnen“ bleibt Einzelauswahl mit dem bisherigen Titel', async () => {
    const { mock, user } = renderApp({ view: workspaceView(documentView()), pickFilesResult: PICKED });
    await user.click(await screen.findByRole('button', { name: 'PDF hinzufügen oder öffnen' }));
    const menu = await screen.findByRole('menu', { name: 'PDF hinzufügen oder öffnen' });
    await user.click(within(menu).getByRole('menuitem', { name: 'Anderes PDF öffnen …' }));
    await waitFor(() => expect(opened(mock.dispatched)).toEqual([{ type: 'open_files', paths: [PICKED[0]], mode: 'replace' }]));
    expect(mock.pickFilesCalls).toEqual([{ title: 'Durchsuchen um ein Dokument zu wählen', multiple: false }]);
  });

  it('Windows/Linux: Hinweise mit Strg-Klick', async () => {
    const view = viewOf({ step: 'open', queue: [] }, { actions: editActions({ sign: false, edit_placement: false }), platform: { os: 'windows', mobile: false } });
    const { mock, user } = renderApp({ view, pickFilesResult: PICKED });
    await screen.findByRole('heading', START);
    expect(screen.getByText('Mehrere PDFs auf einmal: im Dialog mit Strg-Klick markieren, dann „Öffnen“.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    await waitFor(() => expect(mock.pickFilesCalls).toEqual([{ title: 'PDF-Dokumente wählen – mehrere mit Strg-Klick markieren, dann „Öffnen“', multiple: true }]));
  });
});

describe('Mehrfachauswahl: Hinweise statt stillem Verwerfen', () => {
  it('schon offene Dokumente: der Kern (hier der Mock wie der Kern) meldet sie, der Stapel bleibt', async () => {
    const { mock, user } = renderApp({ pickFilesResult: PICKED });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    expect(await screen.findByRole('heading', { level: 1, name: '3 Dokumente' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'PDF hinzufügen' }));
    expect(await screen.findByText('3 der gewählten Dokumente sind bereits geöffnet und wurden nicht noch einmal hinzugefügt.')).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: '3 Dokumente' })).toBeInTheDocument();
    expect((await mock.view()).notices.map((n) => n.code)).toEqual(['ng-open-duplicates']);
  });

  it('ein Dokument doppelt in der Auswahl: Hinweis mit Namen', async () => {
    const { user } = renderApp({ pickFilesResult: [PICKED[0] ?? '', PICKED[1] ?? '', PICKED[0] ?? ''] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    expect(await screen.findByRole('heading', { level: 1, name: '2 Dokumente' })).toBeInTheDocument();
    expect(await screen.findByText('„Rechnung.pdf“ ist bereits geöffnet und wurde nicht noch einmal hinzugefügt.')).toBeInTheDocument();
  });
});

describe('Mehrfachauswahl auf Mobilgeräten', () => {
  it('iOS: Hinweis „antippen, dann Öffnen“, alle Kopien an den Kern, nicht kopierbare als Hinweis; axe', async () => {
    const { mock, user } = renderApp({ platform: 'ios', pickMobileSkipped: 1 });
    const button = await screen.findByRole('button', { name: 'Dokument auswählen' });
    expect(screen.getByText('Mehrere PDFs auf einmal: in der Dateiauswahl die PDFs antippen (Häkchen), dann „Öffnen“.')).toBeInTheDocument();
    await user.click(button);
    await waitFor(() => expect(opened(mock.dispatched)).toHaveLength(1));
    const sent = opened(mock.dispatched)[0] as Extract<UiEvent, { type: 'open_files' }>;
    expect(sent.paths.map((p) => p.split('/').at(-1))).toEqual(['Vertrag.pdf', 'Angebot.pdf']);
    expect(mock.nativeCalls).toEqual(['pickMobileDocuments:multiple']);
    expect(await screen.findByText('Eine gewählte Datei konnte nicht übernommen werden (nicht lesbar oder zu groß).')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Android: Hinweis „lange drücken“; „+ PDF“ im Arbeitsbereich schickt alle Kopien', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const start = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { actions: editActions({ sign: false, edit_placement: false }) }), ANDROID), platform: 'android' });
    expect(await screen.findByText('Mehrere PDFs auf einmal: in der Dateiauswahl ein PDF lange drücken, weitere antippen, dann „Auswählen“.')).toBeInTheDocument();
    start.unmount();
    const files = ['/data/user/0/at.asit.pida/cache/import/1/A.pdf', '/data/user/0/at.asit.pida/cache/import/2/B.pdf', '/data/user/0/at.asit.pida/cache/import/3/C.pdf'];
    const { mock, user } = renderApp({ view: onMobile(workspaceView(), ANDROID), platform: 'android', pickMobileFilesResult: files });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(await screen.findByRole('button', { name: 'PDF hinzufügen oder öffnen' }));
    const menu = await screen.findByRole('menu', { name: 'PDF hinzufügen oder öffnen' });
    await user.click(within(menu).getByRole('menuitem', { name: 'Weitere PDFs hinzufügen …' }));
    await waitFor(() => expect(opened(mock.dispatched)).toEqual([{ type: 'open_files', paths: files, mode: 'add' }]));
    expect(mock.nativeCalls).toEqual(['pickMobileDocuments:multiple']);
  });
});
