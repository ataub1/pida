/**
 * Geparkte Arbeit auf der Startansicht (ADR 0004 Nachtrag 10): Karte „In Bearbeitung“ mit Dokumentname bzw.
 * „Stapel · n Dokumente“, „Weiter bearbeiten“, „Schließen“ ohne bzw. mit kurzer Rückfrage (Standard „Abbrechen“),
 * Rückfrage beim Wählen und Ablegen neuer PDFs (Standard „Zum Stapel hinzufügen“, „Offene schließen und neu
 * beginnen“, Abbrechen ändert nichts), „Öffnen mit“ ohne Rückfrage mit Hinweis, axe. „Weiter bearbeiten“ eines Stapels
 * führt zur Stapelübersicht – auch aus einem geöffneten Eintrag, mit dessen Platzierung (Nachtrag 13).
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings, noActions } from '../../core/defaults';
import type { ParkedWorkView } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { lastEvent, renderApp } from '../../test/render';
import { viewOf } from '../../test/views';

const START = { level: 1, name: 'Was möchten Sie signieren?' } as const;

function parkedStart(parked: Partial<ParkedWorkView> = {}) {
  return viewOf(
    { step: 'open', queue: [] },
    {
      actions: { ...noActions(), open_files: true, settings: true, home: true, verify: true },
      parked: { doc_name: 'Vertrag.pdf', documents: 1, placed_by_hand: false, unsaved: false, ...parked },
    },
  );
}

function card() {
  return screen.getByRole('region', { name: 'In Bearbeitung' });
}

function settings() {
  const s = defaultSettings();
  return { ...s, backend: { default: 'id_austria' as const }, output: { ...s.output, folder: '.' } };
}

/** Zeile der Stapelliste mit diesem Dokument. */
function row(list: HTMLElement, name: string): HTMLElement {
  const r = within(list)
    .getAllByRole('listitem', { hidden: true })
    .find((x) => within(x).queryByText(name));
  if (!r) throw new Error(`Zeile ${name} fehlt`);
  return r;
}

const BATCH_LIST = { name: 'Dokumente im Stapel' } as const;

describe('Karte „In Bearbeitung“', () => {
  it('Dokumentname, „Weiter bearbeiten“, „Schließen“ ohne Rückfrage, wenn nichts verloren geht', async () => {
    const { mock, user } = renderApp({ view: parkedStart() });
    await screen.findByRole('heading', START);
    expect(within(card()).getByRole('heading', { level: 2, name: 'Vertrag.pdf' })).toBeInTheDocument();
    expect(card()).toHaveTextContent('In Bearbeitung');
    await expectNoAxeViolations();
    await user.click(within(card()).getByRole('button', { name: 'Weiter bearbeiten' }));
    expect(lastEvent(mock)).toEqual({ type: 'resume_parked' });
    await user.click(within(card()).getByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(lastEvent(mock)).toEqual({ type: 'discard_parked' });
  });

  it('von Hand platziert: „Schließen“ fragt kurz nach – Standard „Abbrechen“, Escape ändert nichts', async () => {
    const { mock, user } = renderApp({ view: parkedStart({ placed_by_hand: true }) });
    await screen.findByRole('heading', START);
    expect(card()).toHaveTextContent('Ihre Platzierung bleibt erhalten');
    await user.click(within(card()).getByRole('button', { name: 'Schließen' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Offene Arbeit schließen?' });
    expect(dialog).toHaveTextContent('„Vertrag.pdf“ wird geschlossen; Ihre Platzierung geht verloren. Bereits signierte Dateien bleiben erhalten.');
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Abbrechen' })).toHaveFocus());
    await expectNoAxeViolations();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(mock.dispatched).toEqual([]);
    await user.click(within(card()).getByRole('button', { name: 'Schließen' }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Schließen' }));
    expect(mock.dispatched).toEqual([{ type: 'discard_parked' }]);
  });

  it('Stapel mit mehreren Dokumenten: „Stapel · 3 Dokumente“, „Schließen“ mit Rückfrage', async () => {
    const { mock, user } = renderApp({ view: parkedStart({ doc_name: null, documents: 3 }) });
    await screen.findByRole('heading', START);
    expect(within(card()).getByRole('heading', { level: 2, name: 'Stapel · 3 Dokumente' })).toBeInTheDocument();
    await user.click(within(card()).getByRole('button', { name: 'Schließen' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Offene Arbeit schließen?' });
    expect(dialog).toHaveTextContent('Der Stapel wird geschlossen');
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([]);
  });

  it('signiert, nicht gespeichert: Hinweis und Rückfrage nennen die Signatur', async () => {
    const { user } = renderApp({ view: parkedStart({ unsaved: true }) });
    await screen.findByRole('heading', START);
    expect(card()).toHaveTextContent('Signiert, aber noch nicht gespeichert.');
    await user.click(within(card()).getByRole('button', { name: 'Schließen' }));
    expect(await screen.findByRole('alertdialog')).toHaveTextContent('ist signiert, aber noch nicht gespeichert');
  });

  it('ohne geparkte Arbeit keine Karte', async () => {
    renderApp({ view: viewOf({ step: 'open', queue: [] }, { actions: { ...noActions(), open_files: true, home: true } }) });
    await screen.findByRole('heading', START);
    expect(screen.queryByRole('region', { name: 'In Bearbeitung' })).toBeNull();
  });
});

describe('Neue PDFs bei geparkter Arbeit', () => {
  it('Wählen: kurze Rückfrage, Standard „Zum Stapel hinzufügen“ (Eingabe)', async () => {
    const { mock, user } = renderApp({ view: parkedStart(), pickFilesResult: ['/Users/demo/a.pdf', '/Users/demo/b.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    expect(dialog).toHaveTextContent('„Vertrag.pdf“ ist noch in Bearbeitung. Sollen die 2 gewählten Dokumente dazukommen?');
    const add = within(dialog).getByRole('button', { name: 'Zum Stapel hinzufügen' });
    await waitFor(() => expect(add).toHaveFocus());
    expect(mock.dispatched).toEqual([]);
    await expectNoAxeViolations();
    await user.keyboard('{Enter}');
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'open_files', paths: ['/Users/demo/a.pdf', '/Users/demo/b.pdf'], mode: 'add' }]));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('Ablegen: „Offene schließen und neu beginnen“ ersetzt, Abbrechen und Escape ändern nichts', async () => {
    const { mock, user } = renderApp({ view: parkedStart({ doc_name: null, documents: 2 }) });
    await screen.findByRole('heading', START);
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/x.pdf'], x: 1, y: 1 }));
    let dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    expect(dialog).toHaveTextContent('Ein Stapel ist noch in Bearbeitung. Soll das gewählte Dokument dazukommen?');
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/x.pdf'], x: 1, y: 1 }));
    await screen.findByRole('alertdialog');
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(mock.dispatched).toEqual([]);
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/x.pdf'], x: 1, y: 1 }));
    dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Offene schließen und neu beginnen' }));
    expect(mock.dispatched).toEqual([{ type: 'open_files', paths: ['/x.pdf'], mode: 'replace' }]);
  });

  it('⌘/Strg+O fragt ebenso', async () => {
    const { mock, user } = renderApp({ view: parkedStart(), pickFilesResult: ['/Users/demo/a.pdf'] });
    await screen.findByRole('heading', START);
    await user.keyboard('{Control>}o{/Control}');
    expect(await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' })).toHaveTextContent('Soll das gewählte Dokument dazukommen?');
    expect(mock.dispatched).toEqual([]);
  });
});

describe('Mit dem Mock-Kern', () => {
  it('Zur Startseite → Karte, neue PDFs zum Stapel: das geparkte Dokument behält seinen Plan', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    const sticker = await screen.findByRole('application', { name: 'Signaturblock' });
    sticker.focus();
    fireEvent.keyDown(sticker, { key: 'ArrowUp' });
    await waitFor(async () => expect((await mock.view()).document?.origin).toBe('user'));
    const placed = (await mock.view()).document?.placement;
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await screen.findByRole('heading', START);
    expect(card()).toHaveTextContent('Vertrag.pdf');
    expect(card()).toHaveTextContent('Ihre Platzierung bleibt erhalten');
    // „Weiter bearbeiten“: Platzierung unverändert
    await user.click(within(card()).getByRole('button', { name: 'Weiter bearbeiten' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect((await mock.view()).document?.placement).toEqual(placed);
    // erneut parken, neue PDFs wählen → Rückfrage → Stapel mit dem geparkten Dokument als „Positioniert“
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await screen.findByRole('region', { name: 'In Bearbeitung' });
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/Users/demo/Angebot.pdf'], x: 1, y: 1 }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Zum Stapel hinzufügen' }));
    const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(2);
    const v = await mock.view();
    expect(v.batch?.items.map((i) => [i.name, i.position.kind])).toEqual([
      ['Vertrag.pdf', 'manual'],
      ['Angebot.pdf', 'auto'],
    ]);
    expect(v.parked).toBeNull();
  });

  it('„Öffnen mit“ (zweite Instanz) bei geparkter Arbeit: ohne Rückfrage dazu, mit Hinweis', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await screen.findByRole('region', { name: 'In Bearbeitung' });
    // die Shell sendet „Öffnen mit“ direkt an den Kern (ohne `mode`)
    await act(() => mock.dispatch({ type: 'open_files', paths: ['/Users/demo/Angebot.pdf'] }));
    await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(await screen.findByText('Das Dokument wurde zur offenen Arbeit hinzugefügt.')).toBeInTheDocument();
  });

  it('Stapel (Nachtrag 13): Eintrag geöffnet und verschoben, „Weiter bearbeiten“ → Stapelübersicht mit seinem Plan, Fokus auf der Überschrift', async () => {
    const { mock, user } = renderApp({ scenario: 'batch', settings: settings() });
    const list = await screen.findByRole('list', BATCH_LIST);
    await waitFor(() => expect(within(list).queryAllByText('Wird geprüft')).toHaveLength(0));
    await user.click(within(row(list, 'Angebot_2026-118.pdf')).getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' }));
    const sticker = await screen.findByRole('application', { name: 'Signaturblock' });
    fireEvent.keyDown(sticker, { key: 'ArrowUp' });
    await waitFor(async () => expect((await mock.view()).document?.origin).toBe('user'));
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await screen.findByRole('heading', START);
    // die Karte nennt den Stapel, nicht das gerade bearbeitete Dokument
    expect(within(card()).getByRole('heading', { level: 2, name: 'Stapel · 4 Dokumente' })).toBeInTheDocument();
    expect(card()).toHaveTextContent('Pläne und Status der Dokumente bleiben erhalten.');
    expect(card()).not.toHaveTextContent('Angebot_2026-118.pdf');
    await user.click(within(card()).getByRole('button', { name: 'Weiter bearbeiten' }));
    const back = await screen.findByRole('list', BATCH_LIST);
    expect(screen.queryByRole('application', { name: 'Signaturblock' })).toBeNull();
    const offer = row(back, 'Angebot_2026-118.pdf');
    expect(within(offer).getByText('Positioniert')).toBeInTheDocument();
    expect(within(offer).getByText('Seite 2, selbst platziert')).toBeInTheDocument();
    expect(within(back).getAllByRole('listitem')).toHaveLength(4);
    // Fokus und Titel wie beim Öffnen der Übersicht: die Überschrift wird vorgelesen
    await waitFor(() => expect(screen.getByRole('heading', { level: 1, name: '4 Dokumente' })).toHaveFocus());
    expect(document.title).toBe('PIDA – Stapel');
    const v = await mock.view();
    expect([v.parked, v.document, v.batch?.open_item]).toEqual([null, null, null]);
    await expectNoAxeViolations();
  });

  it('Einzeldokument mit wartendem Dokument (Nachtrag 13): „Stapel · 2 Dokumente“, „Weiter bearbeiten“ → Übersicht', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', settings: settings() });
    await screen.findByRole('application', { name: 'Signaturblock' });
    // „Öffnen mit“ im Arbeitsbereich: das Dokument wartet
    await act(() => mock.dispatch({ type: 'open_files', paths: ['/Users/demo/Angebot.pdf'] }));
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await screen.findByRole('heading', START);
    expect(within(card()).getByRole('heading', { level: 2, name: 'Stapel · 2 Dokumente' })).toBeInTheDocument();
    await user.click(within(card()).getByRole('button', { name: 'Weiter bearbeiten' }));
    const list = await screen.findByRole('list', BATCH_LIST);
    expect(within(list).getAllByRole('listitem')).toHaveLength(2);
    expect(row(list, 'Vertrag.pdf')).toBeInTheDocument();
    expect(row(list, 'Angebot.pdf')).toBeInTheDocument();
    expect(screen.queryByRole('application', { name: 'Signaturblock' })).toBeNull();
  });

  it('„Zum Stapel hinzufügen“ macht ein geparktes Einzeldokument zum Stapel: danach führt „Weiter bearbeiten“ zur Übersicht', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/Users/demo/Angebot.pdf'], x: 1, y: 1 }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Zum Stapel hinzufügen' }));
    let list = await screen.findByRole('list', BATCH_LIST);
    await waitFor(() => expect(within(list).queryAllByText('Wird geprüft')).toHaveLength(0));
    await user.click(within(row(list, 'Angebot.pdf')).getByRole('button', { name: '„Angebot.pdf“ öffnen und positionieren' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    expect(within(await screen.findByRole('region', { name: 'In Bearbeitung' })).getByRole('heading', { level: 2 })).toHaveTextContent('Stapel · 2 Dokumente');
    await user.click(within(card()).getByRole('button', { name: 'Weiter bearbeiten' }));
    list = await screen.findByRole('list', BATCH_LIST);
    expect(within(row(list, 'Vertrag.pdf')).getByText('Positioniert')).toBeInTheDocument();
    expect(screen.queryByRole('application', { name: 'Signaturblock' })).toBeNull();
  });

  it('„Schließen“ verwirft die geparkte Arbeit: leere Startansicht', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await user.click(within(await screen.findByRole('region', { name: 'In Bearbeitung' })).getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('region', { name: 'In Bearbeitung' })).toBeNull());
    const v = await mock.view();
    expect(v.parked).toBeNull();
    expect(v.done.map((d) => [d.name, d.result.kind])).toEqual([['Vertrag.pdf', 'cancelled']]);
    // ohne geparkte Arbeit öffnet ein neues PDF wieder direkt
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});
