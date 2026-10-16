/**
 * Start und Zustandskarten (ADR 0004 §2.1, §2.11): Ablagefläche nur optisch (Hervorhebung beim Ziehen über
 * die Fenster-Ereignisse), Dateiauswahl, ⌘/Strg+O, Mobile-Variante, Slot „Zuletzt signiert“ leer, Laden,
 * kennwortgeschütztes PDF ohne Kennwortfeld mit „Anderes Dokument öffnen“, Fehlerkarte mit Details, axe.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultErrorContext } from '../../core/defaults';
import { expectNoAxeViolations } from '../../test/a11y';
import { renderApp } from '../../test/render';
import { editActions, IOS, onMobile, QUEUE, viewOf } from '../../test/views';

const START = () => viewOf({ step: 'open', queue: [] }, { actions: editActions({ sign: false, edit_placement: false }) });

describe('Start', () => {
  it('Titel, Ablagefläche, „PDF auswählen“ (Dateidialog mit Mehrfachauswahl), keine Seitenspalte ohne Inhalte', async () => {
    const { mock, user } = renderApp({ pickFilesResult: ['/Users/demo/a.pdf'] });
    expect(await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Ablagefläche für PDF-Dokumente' })).toBeInTheDocument();
    // seit dem Prüfmodus (B4, wie K4) gibt der Kern ihn im Leerlauf frei: die Seitenspalte zeigt nur den Hinweis darauf
    const side = screen.getByRole('complementary');
    expect(within(side).queryByTestId('recent-panel')).not.toBeInTheDocument();
    expect(within(side).getByRole('button', { name: 'Prüfmodus öffnen' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    await waitFor(() => expect(mock.dispatched[0]).toEqual({ type: 'open_files', paths: ['/Users/demo/a.pdf'] }));
    expect(mock.nativeCalls).toEqual(['pickFiles:multiple']);
    await expectNoAxeViolations();
  });

  it('Ziehen über dem Fenster hebt die Ablagefläche hervor, Ablegen öffnet (Pfade aus der Shell)', async () => {
    const { mock } = renderApp({ view: START() });
    const zone = await screen.findByRole('region', { name: 'Ablagefläche für PDF-Dokumente' });
    act(() => mock.simulateFileDrag({ type: 'enter', paths: ['/x.pdf'], x: 1, y: 1 }));
    expect(zone).toHaveAttribute('data-drop-target', 'true');
    expect(zone).toHaveTextContent('Loslassen zum Öffnen');
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/x.pdf', '/y.txt'], x: 1, y: 1 }));
    // keine Filterung nach Endung – der Kern meldet Nicht-PDFs
    expect(mock.dispatched).toEqual([{ type: 'open_files', paths: ['/x.pdf', '/y.txt'] }]);
    expect(zone).not.toHaveAttribute('data-drop-target');
  });

  it('⌘/Strg+O öffnet die Dateiauswahl (nur mit Freigabe)', async () => {
    const { mock, user } = renderApp({ view: START() });
    await screen.findByRole('heading', { level: 1 });
    await user.keyboard('{Control>}o{/Control}');
    await waitFor(() => expect(mock.nativeCalls).toEqual(['pickFiles:multiple']));
  });

  it('Mobile: keine Ablage-Texte und Kürzel, „Dokument auswählen“ über das Plugin', async () => {
    const { mock, user } = renderApp({ platform: 'ios' });
    const button = await screen.findByRole('button', { name: 'Dokument auswählen' });
    expect(screen.queryByText('PDF hierher ziehen oder auswählen')).not.toBeInTheDocument();
    expect(screen.queryByText('öffnet die Dateiauswahl')).not.toBeInTheDocument();
    await user.click(button);
    await waitFor(() => expect(mock.dispatched[0]).toMatchObject({ type: 'open_files' }));
    expect(mock.nativeCalls).toEqual(['pickMobileFiles:pdf:multiple']);
    await expectNoAxeViolations();
  });

  it('Laden mit Abbrechen', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'loading', queue: QUEUE }) });
    expect(await screen.findByRole('heading', { level: 1, name: 'Vertrag.pdf' })).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Das Dokument wird gelesen und geprüft');
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([{ type: 'cancel' }]);
    await expectNoAxeViolations();
  });
});

describe('Zustandskarten', () => {
  it('kennwortgeschütztes PDF: ohne Kennwortfeld, „Anderes Dokument öffnen“ = cancel + Dateiauswahl', async () => {
    const view = viewOf({ step: 'error', code: 'error-PDFPwdProtected', params: {}, retry: false, context: { ...defaultErrorContext(), origin: 'load' } });
    const { mock, user } = renderApp({ view, pickFilesResult: ['/Users/demo/b.pdf'] });
    expect(await screen.findByRole('heading', { level: 1, name: 'Dieses PDF ist mit einem Kennwort geschützt' })).toBeInTheDocument();
    expect(screen.getByRole('main').querySelector('input')).toBeNull();
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: 'Anderes Dokument öffnen' }));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'cancel' }, { type: 'open_files', paths: ['/Users/demo/b.pdf'] }]));
  });

  it('Fehler beim Öffnen (kein PDF) mit Erneut versuchen, Schließen und technischen Details', async () => {
    const view = viewOf({ step: 'error', code: 'error-MayNotBeAPDF', params: {}, retry: true, context: { ...defaultErrorContext(), origin: 'load', occurred_at: '2026-10-15T10:42:17+02:00' } });
    const { mock, user } = renderApp({ view });
    expect(await screen.findByRole('heading', { level: 1, name: 'Diese Datei ist kein PDF' })).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Dies ist möglicherweise keine PDF-Datei');
    await user.click(screen.getByRole('button', { name: 'Technische Details' }));
    expect(screen.getByRole('main')).toHaveTextContent('error-MayNotBeAPDF');
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: 'Erneut versuchen' }));
    const card = screen.getByRole('region', { name: 'Diese Datei ist kein PDF' });
    await user.click(within(card).getByRole('button', { name: 'Schließen' }));
    expect(mock.dispatched).toEqual([{ type: 'retry' }, { type: 'cancel' }]);
  });

  it('Mobile: Startansicht im schmalen Layout ohne Verstöße', async () => {
    renderApp({ view: onMobile(START(), IOS) });
    await screen.findByRole('heading', { level: 1 });
    await expectNoAxeViolations();
  });
});
