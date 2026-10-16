/**
 * Fertig (ADR 0004 §2.6/§3.7, Abnahme E1 Punkt 7): Unterzeichner, Zeit, Datei, Ordner, Selbstprüfung im
 * Wortlaut §8 F3 (nie „gültig“), Öffnen/Im Ordner zeigen (Desktop) bzw. Öffnen/Teilen/Sichern unter
 * (Mobile), Speichern-Dialog und Überschreiben-Rückfrage im neuen Stil, „Nächstes Dokument“, axe.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../test/a11y';
import { renderApp } from '../../test/render';
import { DOC, doneView, documentView, onMobile, outputResult, signersView, summary, viewOf } from '../../test/views';

describe('Fertig', () => {
  it('Zusammenfassung: Prüfsiegel „erstellt und geprüft“, Qualifikation laut Zertifikat, Datei, Ordner, Unterzeichner, Zeit', async () => {
    renderApp({ view: doneView() });
    const heading = await screen.findByRole('heading', { level: 1, name: 'Signiert' });
    await waitFor(() => expect(heading).toHaveFocus());
    const main = screen.getByRole('main');
    expect(main).toHaveTextContent('Signatur erstellt und geprüft (Selbstprüfung)');
    expect(main).toHaveTextContent('Qualifiziert laut Zertifikat');
    expect(main).toHaveTextContent('Vertrag_signed.pdf');
    expect(main).toHaveTextContent('Gespeichert in /Users/demo/Dokumente');
    expect(main).toHaveTextContent(/Signiert von Max Mustermann · 15\.10\.2026/);
    // §8 F3: nie „gültig“ oder „rechtsgültig“
    expect(main.textContent).not.toMatch(/gültig/i);
    expect(screen.getByRole('img', { name: 'Vorschau der signierten Seite' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Desktop: Öffnen, Im Ordner zeigen, Link zur vollständigen Prüfung, Weiteres Dokument', async () => {
    const { mock, user } = renderApp({ view: doneView() });
    await user.click(await screen.findByRole('button', { name: 'Öffnen' }));
    await user.click(screen.getByRole('button', { name: 'Im Ordner zeigen' }));
    await user.click(screen.getByRole('button', { name: /Vollständige Prüfung bei signaturpruefung\.gv\.at/ }));
    await user.click(screen.getByRole('button', { name: 'Weiteres Dokument signieren' }));
    expect(mock.dispatched).toEqual([{ type: 'open_result' }, { type: 'navigate', step: 'Open' }]);
    expect(mock.nativeCalls).toContain('revealOutput');
    expect(mock.openedUrls).toEqual(['https://www.signaturpruefung.gv.at']);
    expect(screen.queryByRole('button', { name: 'Teilen' })).not.toBeInTheDocument();
  });

  it('mit wartenden Dokumenten: „Nächstes Dokument“ mit Namen und Anzahl der wartenden', async () => {
    const { mock, user } = renderApp({ view: doneView({}, { ...DOC, id: 8, name: 'Angebot.pdf' }, {}, 2) });
    expect(await screen.findByText('Weiter mit „Angebot.pdf“ · 2 Dokumente warten')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Nächstes Dokument' }));
    expect(mock.dispatched).toEqual([{ type: 'navigate', step: 'Open' }]);
  });

  it('Stapelübersicht an (E2): mit wartenden Dokumenten zusätzlich „Zum Stapel“ (`show_batch`)', async () => {
    const base = doneView({}, { ...DOC, id: 8, name: 'Angebot.pdf' }, {}, 2);
    const { mock, user } = renderApp({ view: { ...base, actions: { ...base.actions, show_batch: true } } });
    await user.click(await screen.findByRole('button', { name: 'Zum Stapel' }));
    expect(mock.dispatched).toEqual([{ type: 'show_batch' }]);
    await expectNoAxeViolations();
  });

  it('ohne `show_batch` kein „Zum Stapel“', async () => {
    renderApp({ view: doneView({}, { ...DOC, id: 8, name: 'Angebot.pdf' }, {}, 2) });
    await screen.findByRole('button', { name: 'Nächstes Dokument' });
    expect(screen.queryByRole('button', { name: 'Zum Stapel' })).not.toBeInTheDocument();
  });

  it('Mobile: Teilen und Sichern unter statt Im Ordner zeigen, kein Sandbox-Pfad', async () => {
    const path = '/var/mobile/Containers/Data/Application/X/Documents/Vertrag_signed.pdf';
    const { mock, user } = renderApp({ view: onMobile(doneView({ path, summary: summary({ folder_display: null }) })) });
    expect(await screen.findByText('Gespeichert in den Dokumenten dieser App')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Im Ordner zeigen' })).not.toBeInTheDocument();
    expect(screen.getByRole('main').textContent).not.toContain('/var/mobile');
    await user.click(screen.getByRole('button', { name: 'Teilen' }));
    await user.click(screen.getByRole('button', { name: 'Sichern unter …' }));
    expect(mock.dispatched).toEqual([{ type: 'share_result' }, { type: 'export_result' }]);
    await expectNoAxeViolations();
  });

  it('Kern ohne Zusammenfassung (K0): Dateiname und Ordner aus dem Pfad', async () => {
    renderApp({ view: viewOf({ step: 'output', result: outputResult({ path: '/Users/demo/Out/Vertrag_signed.pdf' }), next: null, pending: 0 }) });
    expect(await screen.findByText('Vertrag_signed.pdf')).toBeInTheDocument();
    expect(screen.getByText('Gespeichert in /Users/demo/Out')).toBeInTheDocument();
  });

  it('Speichern-Dialog öffnet sich automatisch mit dem Vorschlag; abgebrochen → Knopf „Speichern unter …“', async () => {
    const view = viewOf(
      { step: 'output', result: outputResult({ save_dialog: { directory: '/Users/demo/Dokumente', file_name: 'Vertrag_signed.pdf' } }), next: null, pending: 0 },
      { document: documentView({ phase: 'done' }), signers: signersView() },
    );
    const { mock, user, unmount } = renderApp({ view, pickSaveResult: null });
    await waitFor(() => expect(mock.nativeCalls).toContain('pickSavePath:/Users/demo/Dokumente/Vertrag_signed.pdf'));
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'save', path: null }]));
    unmount();
    const pending = viewOf({ step: 'output', result: outputResult({ save_pending: true }), next: null, pending: 0 }, { document: documentView({ phase: 'done' }) });
    const r = renderApp({ view: pending, pickSaveResult: '/Users/demo/Neu.pdf' });
    expect(await screen.findByText('Das signierte Dokument ist noch nicht gespeichert.')).toBeInTheDocument();
    await r.user.click(screen.getByRole('button', { name: 'Speichern unter …' }));
    await waitFor(() => expect(r.mock.dispatched).toEqual([{ type: 'save', path: '/Users/demo/Neu.pdf' }]));
    void user;
  });

  it('Überschreiben-Rückfrage im neuen Stil (Mobile: nur der Dateiname)', async () => {
    const existing = '/var/mobile/Containers/Data/Application/X/Documents/Vertrag_signed.pdf';
    const view = onMobile(viewOf({ step: 'output', result: outputResult({ ask_overwrite: existing }), next: null, pending: 0 }, { document: documentView({ phase: 'done' }) }));
    const { mock, user } = renderApp({ view });
    const dialog = await screen.findByRole('alertdialog', { name: 'Datei überschreiben?' });
    expect(dialog).toHaveTextContent('Datei Vertrag_signed.pdf existiert bereits');
    expect(dialog).not.toHaveTextContent('/var/mobile');
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Überschreiben' }));
    expect(mock.dispatched).toEqual([{ type: 'confirm_overwrite', overwrite: true }]);
  });
});
