/**
 * Geparkte Arbeit auf der Startansicht (ADR 0004 Nachtrag 10): Karte „In Bearbeitung“ mit Dokumentname bzw.
 * „Stapel · n Dokumente“, „Weiter bearbeiten“, „Schließen“ ohne bzw. mit kurzer Rückfrage (Standard „Abbrechen“),
 * Rückfrage beim Wählen und Ablegen neuer PDFs (Standard „Zum Stapel hinzufügen“, „Offene schließen und neu
 * beginnen“, Abbrechen ändert nichts), „Öffnen mit“ ohne Rückfrage mit Hinweis, axe.
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
