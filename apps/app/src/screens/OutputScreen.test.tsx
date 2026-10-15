import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../test/render';
import { DOC, outputResult, viewOf } from '../test/views';

describe('Ergebnis', () => {
  it('gespeichert: Erfolgsmeldung, Öffnen per Link und Knopf, im Ordner anzeigen', async () => {
    const { mock, user } = renderApp({
      view: viewOf({ step: 'output', result: outputResult({ path: '/Users/demo/Vertrag_signed.pdf', save_pending: false }), next: null }),
    });
    expect(await screen.findByRole('heading', { level: 1, name: 'Signatur war erfolgreich' })).toBeInTheDocument();
    expect(screen.getByText('Vertrag_signed.pdf')).toBeInTheDocument();
    expect(screen.getByText('/Users/demo')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'hier' }));
    await user.click(screen.getByRole('button', { name: 'Öffnen' }));
    await user.click(screen.getByRole('button', { name: 'Im Ordner anzeigen' }));
    expect(mock.dispatched).toEqual([{ type: 'open_result' }, { type: 'open_result' }]);
    await waitFor(() => expect(mock.nativeCalls).toEqual(['revealOutput']));
  });

  it('Speichern fehlgeschlagen/abgebrochen: Knopf öffnet den Speichern-Dialog', async () => {
    const { mock, user } = renderApp({
      view: viewOf({ step: 'output', result: outputResult({ path: null, save_pending: true }), next: null }),
      pickSaveResult: '/Users/demo/ziel.pdf',
    });
    expect(await screen.findByText('Sie können das signierte Dokument speichern')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Speichern' }));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'save', path: '/Users/demo/ziel.pdf' }]));
  });

  it('abgebrochener Speichern-Dialog sendet save(null)', async () => {
    const { mock, user } = renderApp({
      view: viewOf({ step: 'output', result: outputResult({ path: null, save_pending: true }), next: null }),
      pickSaveResult: null,
    });
    await user.click(await screen.findByRole('button', { name: 'Speichern' }));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'save', path: null }]));
  });

  it('Speichern-Dialog des Kerns (save_dialog) öffnet sich einmal mit Vorschlag', async () => {
    const view = viewOf({
      step: 'output',
      result: outputResult({ save_dialog: { directory: '/Users/demo', file_name: 'Vertrag_signed.pdf' } }),
      next: null,
    });
    const { mock } = renderApp({ view, pickSaveResult: null });
    await waitFor(() => expect(mock.nativeCalls).toEqual(['pickSavePath:/Users/demo/Vertrag_signed.pdf']));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'save', path: null }]));
    act(() => mock.setView({ ...view }));
    expect(mock.nativeCalls).toHaveLength(1);
  });

  it('Überschreiben-Rückfrage (ask_overwrite)', async () => {
    const view = viewOf({
      step: 'output',
      result: outputResult({ ask_overwrite: '/Users/demo/Vertrag_signed.pdf' }),
      next: null,
    });
    const { mock, user } = renderApp({ view });
    const dialog = await screen.findByRole('alertdialog', { name: 'Warnung' });
    expect(dialog).toHaveTextContent('Datei /Users/demo/Vertrag_signed.pdf existiert bereits');
    await user.click(within(dialog).getByRole('button', { name: 'OK' }));
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([
      { type: 'confirm_overwrite', overwrite: true },
      { type: 'confirm_overwrite', overwrite: false },
    ]);
  });

  it('nächstes Dokument der Warteschlange', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'output', result: outputResult({ path: null, save_pending: true }), next: { ...DOC, name: 'Angebot.pdf' } }) });
    expect(await screen.findByText('Nächstes Dokument: Angebot.pdf')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Nächstes Dokument signieren' }));
    expect(mock.dispatched).toEqual([{ type: 'navigate', step: 'Open' }]);
  });

  it('Speichern-Dialog nicht verfügbar: Hinweis und save(null)', async () => {
    const view = viewOf({
      step: 'output',
      result: outputResult({ save_dialog: { directory: '/Users/demo', file_name: 'Vertrag_signed.pdf' } }),
      next: null,
    });
    const { mock } = renderApp({ view });
    mock.pickSavePath = () => Promise.reject(new Error('kaputt'));
    act(() => mock.setView({ ...view, step: { ...view.step } }));
    await waitFor(() => expect(mock.dispatched).toContainEqual({ type: 'save', path: null }));
  });

  it('abgeschlossene Dokumente der Sitzung', async () => {
    renderApp({
      view: viewOf(
        { step: 'output', result: outputResult({ path: '/x/c_signed.pdf' }), next: null },
        {
          done: [
            { id: 1, name: 'a.pdf', input: '/x/a.pdf', result: { kind: 'signed', output: { path: '/x/a_signed.pdf', len: 1, sha256: '00' } } },
            { id: 2, name: 'b.pdf', input: '/x/b.pdf', result: { kind: 'failed', error: { Backend: { code: 'backend-wrong-password', detail: '' } } } },
          ],
        },
      ),
    });
    const panel = await screen.findByRole('complementary', { name: 'Erledigt (2)' });
    expect(panel).toHaveTextContent('a.pdf');
    expect(panel).toHaveTextContent('a_signed.pdf');
    expect(panel).toHaveTextContent('Das Passwort ist falsch.');
  });

  it('Ablegen auf dem Ergebnis hängt Dokumente an (F-89)', async () => {
    const { mock } = renderApp({ view: viewOf({ step: 'output', result: outputResult({ path: '/x/a_signed.pdf', save_pending: false }), next: null }) });
    await screen.findByRole('heading', { level: 1 });
    act(() => mock.simulateFileDrag({ type: 'drop', paths: ['/x/b.pdf'], x: 1, y: 1 }));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'open_files', paths: ['/x/b.pdf'] }]));
  });
});

describe('Fehler', () => {
  it('Meldung aus dem Katalog, Details, Wiederholen/Abbrechen', async () => {
    const { mock, user } = renderApp({ view: viewOf({ step: 'error', code: 'error-PDFPwdProtected', params: {}, retry: true }) });
    expect(await screen.findByRole('heading', { level: 1, name: 'Fehler' })).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Passwortgeschützt');
    await user.click(screen.getByRole('button', { name: 'Wiederholen' }));
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([{ type: 'retry' }, { type: 'cancel' }]);
  });

  it('Codes mit Punkt und Parameter; unbekannte Codes', async () => {
    renderApp({ view: viewOf({ step: 'error', code: 'output.save_failed', params: { arg1: 'a.pdf', arg2: 'Datenträger voll' }, retry: false }) });
    expect(await screen.findByRole('alert')).toHaveTextContent('Datei a.pdf konnte nicht gespeichert werden: Datenträger voll');
    expect(screen.queryByRole('button', { name: 'Wiederholen' })).not.toBeInTheDocument();
  });

  it('unbekannter Code → allgemeine Meldung mit Code', async () => {
    renderApp({ view: viewOf({ step: 'error', code: 'pades.something_new', params: {}, retry: false }, { language: 'en' }) });
    expect(await screen.findByRole('alert')).toHaveTextContent('Unexpected error (pades.something_new).');
  });

  it('Kern nicht verfügbar (Shell-Fehlerschritt)', async () => {
    renderApp({ view: viewOf({ step: 'error', code: 'ng-ui-core-unavailable', params: { detail: 'not yet implemented: Integration' }, retry: true }) });
    expect(await screen.findByRole('alert')).toHaveTextContent('Der Signaturkern konnte nicht gestartet werden');
    expect(screen.getByText('not yet implemented: Integration')).toBeInTheDocument();
  });

  it('Programmende', async () => {
    renderApp({ view: viewOf({ step: 'finished', exit_code: 4 }) });
    expect(await screen.findByRole('heading', { level: 1, name: 'PDF-Over NG wurde beendet.' })).toBeInTheDocument();
    expect(screen.getByText('Beendet mit Code 4.')).toBeInTheDocument();
  });
});
