import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../test/render';
import { QUEUE, viewOf } from '../test/views';

describe('Öffnen', () => {
  it('zeigt Drop-Zone, Durchsuchen und Hauptleiste (DE)', async () => {
    renderApp({ view: viewOf({ step: 'open', queue: [] }) });
    expect(await screen.findByRole('heading', { level: 1, name: 'Dokument öffnen' })).toBeInTheDocument();
    expect(screen.getByText('Dokument hierher ziehen')).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Ablagefläche für PDF-Dokumente' })).toBeInTheDocument();
    const nav = screen.getByRole('navigation', { name: 'Arbeitsschritte' });
    const steps = within(nav).getAllByRole('button');
    // Einstellungen + 4 Schritte
    expect(steps).toHaveLength(5);
    expect(within(nav).getByRole('button', { name: 'Öffnen' })).toHaveAttribute('aria-current', 'step');
    expect(within(nav).getByRole('button', { name: 'Positionierung' })).toBeDisabled();
    expect(within(nav).getByRole('button', { name: 'Fertig' })).toBeDisabled();
    expect(document.title).toBe('PDF-Over NG – Dokument öffnen');
  });

  it('Durchsuchen öffnet den Dateidialog (Mehrfachauswahl) und sendet open_files', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'open', queue: [] }) });
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    await waitFor(() => expect(mock.dispatched).toHaveLength(1));
    expect(mock.nativeCalls).toContain('pickFiles:multiple');
    expect(mock.dispatched[0]).toEqual({
      type: 'open_files',
      paths: ['/Users/demo/Dokumente/Vertrag.pdf', '/Users/demo/Dokumente/Angebot 2026.pdf'],
    });
  });

  it('abgebrochener Dialog sendet nichts', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'open', queue: [] }), pickFilesResult: null });
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    await waitFor(() => expect(mock.nativeCalls).toContain('pickFiles:multiple'));
    expect(mock.dispatched).toEqual([]);
  });

  it('Zugriffstaste Alt+D (DE) und Strg+O lösen den Dateidialog aus', async () => {
    const { mock } = renderApp({ view: viewOf({ step: 'open', queue: [] }) });
    const button = await screen.findByRole('button', { name: /Durchsuchen/ });
    expect(button).toHaveAttribute('aria-keyshortcuts', 'Alt+D');
    fireEvent.keyDown(window, { key: 'd', code: 'KeyD', altKey: true });
    await waitFor(() => expect(mock.nativeCalls).toEqual(['pickFiles:multiple']));
    fireEvent.keyDown(window, { key: 'o', code: 'KeyO', ctrlKey: true });
    await waitFor(() => expect(mock.nativeCalls).toEqual(['pickFiles:multiple', 'pickFiles:multiple']));
  });

  it('Ziehen hebt die Drop-Zone hervor, Ablegen sendet open_files', async () => {
    const { mock } = renderApp({ view: viewOf({ step: 'open', queue: [] }) });
    const zone = await screen.findByRole('region', { name: 'Ablagefläche für PDF-Dokumente' });
    expect(zone).not.toHaveAttribute('data-drop-target');
    act(() => mock.simulateFileDrag({ type: 'enter', paths: ['/x/a.pdf'], x: 10, y: 10 }));
    expect(zone).toHaveAttribute('data-drop-target', 'true');
    expect(screen.getByText('Loslassen, um die Dokumente zu öffnen')).toBeInTheDocument();
    act(() => mock.simulateFileDrag({ type: 'leave' }));
    expect(zone).not.toHaveAttribute('data-drop-target');
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/x/a.pdf', '/x/b.pdf'], x: 10, y: 10 }));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'open_files', paths: ['/x/a.pdf', '/x/b.pdf'] }]));
  });

  it('zeigt die Warteschlange mit aktuellem Dokument', async () => {
    renderApp({ view: viewOf({ step: 'open', queue: QUEUE }) });
    const queue = await screen.findByRole('complementary', { name: 'Warteschlange (2)' });
    const items = within(queue).getAllByRole('listitem');
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveAttribute('aria-current', 'true');
    expect(within(items[0] as HTMLElement).getByText('aktuell')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Dokument wird geprüft');
  });

  it('englische Oberfläche', async () => {
    renderApp({ view: viewOf({ step: 'open', queue: [] }, { language: 'en' }) });
    expect(await screen.findByRole('heading', { level: 1, name: 'Open document' })).toBeInTheDocument();
    expect(screen.getByText('Drag document here')).toBeInTheDocument();
    const button = screen.getByRole('button', { name: 'Browse for PDF on your computer' });
    expect(button).toHaveAttribute('aria-keyshortcuts', 'Alt+B');
    expect(document.documentElement.lang).toBe('en');
  });
});

describe('Hauptleiste', () => {
  it('klickbare Schritte navigieren, „Fertig“ nie', async () => {
    const { mock, user } = renderApp({
      view: viewOf({ step: 'output', result: { path: '/x/a_signed.pdf', save_pending: false }, next: null }),
    });
    const nav = await screen.findByRole('navigation', { name: 'Arbeitsschritte' });
    await user.click(within(nav).getByRole('button', { name: /^Positionierung/ }));
    await user.click(within(nav).getByRole('button', { name: /^Signatur/ }));
    expect(within(nav).getByRole('button', { name: 'Fertig' })).toBeDisabled();
    expect(within(nav).getByRole('button', { name: 'Fertig' })).toHaveAttribute('aria-current', 'step');
    expect(mock.dispatched).toEqual([
      { type: 'navigate', step: 'Positioning' },
      { type: 'navigate', step: 'Signing' },
    ]);
  });

  it('Einstellungen per Knopf und Strg+,', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'open', queue: [] }) });
    await user.click(await screen.findByRole('button', { name: 'Konfiguration' }));
    fireEvent.keyDown(window, { key: ',', ctrlKey: true });
    expect(mock.dispatched).toEqual([{ type: 'open_settings' }, { type: 'open_settings' }]);
  });

  it('während des Signierens ist nichts klickbar', async () => {
    renderApp({ view: viewOf({ step: 'signing', interaction: { kind: 'waiting', progress: 'Connecting' }, queue: [] }) });
    const nav = await screen.findByRole('navigation', { name: 'Arbeitsschritte' });
    for (const b of within(nav).getAllByRole('button')) expect(b).toBeDisabled();
  });
});
