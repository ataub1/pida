/**
 * Ablauf Ende-zu-Ende mit dem Mock-Kern (ohne festen Zustand): Öffnen → Positionieren →
 * Signiermittel → Datenschutz → Anmeldung → App-Bestätigung → Speichern → nächstes Dokument;
 * außerdem Keystore-Ablauf, Abbruch, Einstellungen mit Sprachwechsel und Fokusführung.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import { renderApp } from '../test/render';
import { accessKeyFromEvent, triggerAccessKey } from './App';

describe('Ablauf mit dem Mock-Kern', () => {
  it('ID Austria (App-Bestätigung), Speichern-Dialog, Batch mit zwei Dokumenten', async () => {
    const { mock, user } = renderApp();
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    // Positionieren (letzte Seite, Block bei 50 %/25 %)
    const block = await screen.findByRole('application', { name: 'Signatur' });
    await waitFor(() => expect(document.getElementById('screen-heading')).toHaveFocus());
    expect(screen.getByText('Seite 3 von 3', { selector: '.pager-label' })).toBeInTheDocument();
    block.focus();
    await user.keyboard('{ArrowRight}');
    expect(mock.dispatched.at(-1)).toEqual({ type: 'nudge_block', dx: 1, dy: 0, fine: false, coarse: false });
    await user.keyboard('{Enter}');
    // Signiermittel: Datenschutzhinweis vor dem ersten Upload
    await user.click(await screen.findByRole('button', { name: /^ID Austria/ }));
    const privacy = await screen.findByRole('alertdialog');
    await user.click(within(privacy).getByRole('button', { name: 'Zustimmen und fortfahren' }));
    // Anmeldung
    const username = await screen.findByLabelText('Benutzername:');
    await user.type(username, 'max.muster');
    await user.type(screen.getByLabelText('Passwort:'), 'demo-passwort');
    await user.click(screen.getByRole('checkbox', { name: /Daten merken/ }));
    await user.click(screen.getByRole('button', { name: 'OK' }));
    // App-Bestätigung (Long-Poll im Mock manuell abschließen)
    expect(await screen.findByText(/ID Austria-App/)).toBeInTheDocument();
    act(() => mock.completeConfirmation());
    // Speichern-Dialog (vorgeschlagene Rückfrage), danach Batch → zweites Dokument
    await waitFor(() => expect(mock.nativeCalls).toContain('pickSavePath:/Users/demo/Dokumente/Vertrag_signed.pdf'));
    expect(await screen.findByRole('application', { name: 'Signatur' })).toBeInTheDocument();
    expect(screen.getByText('Angebot 2026.pdf')).toBeInTheDocument();
    expect(screen.getByText(/Vertrag_signed\.pdf wurde gespeichert/)).toBeInTheDocument();
    // zweites Dokument: gemerkter Benutzer → direkt App-Bestätigung (eine Anmeldung je Batch)
    await user.click(screen.getByRole('button', { name: 'Signieren' }));
    await user.click(await screen.findByRole('button', { name: /^ID Austria/ }));
    expect(await screen.findByText(/ID Austria-App/)).toBeInTheDocument();
    act(() => mock.completeConfirmation());
    expect(await screen.findByRole('heading', { level: 1, name: 'Signatur war erfolgreich' })).toBeInTheDocument();
    expect(screen.getByText('Angebot 2026_signed.pdf')).toBeInTheDocument();
    expect(mock.choices).toEqual({ position: 'manual', backend: 'atrust_native' });
  });

  it('Keystore mit falschem Passwort, Abbruch zurück zur Auswahl', async () => {
    const base = defaultSettings();
    const { mock, user } = renderApp({
      settings: { ...base, keystore: { ...base.keystore, enabled: true, file: '/Users/demo/id.p12' }, output: { folder: '/Users/demo/out', postfix: '_signed' } },
      localBkuAvailable: false,
      pickFilesResult: ['/Users/demo/Vertrag.pdf'],
    });
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    await screen.findByRole('application', { name: 'Signatur' });
    await user.click(screen.getByRole('button', { name: 'Unsichtbar signieren' }));
    await user.click(await screen.findByRole('button', { name: /^Keystore/ }));
    const pw = await screen.findByLabelText('Keystore-Passwort eingeben:');
    await user.type(pw, 'falsch{Enter}');
    expect(await screen.findByRole('alert')).toHaveTextContent('Keystore-Passwort nicht akzeptiert');
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiermittel wählen' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Keystore/ }));
    await user.type(await screen.findByLabelText('Keystore-Passwort eingeben:'), 'richtig{Enter}');
    // Ausgabeordner konfiguriert → automatisch gespeichert
    expect(await screen.findByRole('heading', { level: 1, name: 'Signatur war erfolgreich' }, { timeout: 3000 })).toBeInTheDocument();
    expect(mock.choices.position).toBe('invisible');
  });

  it('Einstellungen: Sprache wechseln wirkt sofort nach dem Speichern', async () => {
    const { user } = renderApp();
    await user.click(await screen.findByRole('button', { name: 'Konfiguration' }));
    const dialog = await screen.findByRole('dialog', { name: 'Konfiguration' });
    await user.click(within(dialog).getByRole('tab', { name: 'Erweitert' }));
    await user.click(within(dialog).getByRole('button', { name: /Interface-Sprache/ }));
    await user.click(await screen.findByRole('option', { name: 'Englisch' }));
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Open document' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('Fehler beim Öffnen (kein PDF) mit Wiederholen/Abbrechen', async () => {
    const { user } = renderApp({ pickFilesResult: ['/Users/demo/bild.png'] });
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Dies ist möglicherweise keine PDF-Datei');
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Dokument öffnen' })).toBeInTheDocument();
  });

  it('Ablehnen eines Ereignisses durch den Kern erscheint als Fehlerhinweis', async () => {
    const { mock, user } = renderApp();
    mock.dispatch = async () => {
      throw new Error('ng-ui-event-invalid');
    };
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    expect(await screen.findByRole('alert')).toHaveTextContent('noch nicht unterstützt');
  });
});

describe('Zugriffstasten', () => {
  it('Alt+Taste (Windows/Linux) bzw. Ctrl+Option (macOS); AltGr wird ignoriert', () => {
    const ev = (init: KeyboardEventInit) => new KeyboardEvent('keydown', init);
    expect(accessKeyFromEvent(ev({ code: 'KeyB', altKey: true }), false)).toBe('b');
    expect(accessKeyFromEvent(ev({ code: 'KeyB', altKey: true, ctrlKey: true }), false)).toBeNull();
    expect(accessKeyFromEvent(ev({ code: 'KeyB', altKey: true }), true)).toBeNull();
    expect(accessKeyFromEvent(ev({ code: 'KeyB', altKey: true, ctrlKey: true }), true)).toBe('b');
    expect(accessKeyFromEvent(ev({ code: 'Digit3', altKey: true }), false)).toBe('3');
    expect(accessKeyFromEvent(ev({ code: 'Space', altKey: true }), false)).toBeNull();
  });

  it('nur die oberste Ebene und keine gesperrten Elemente', () => {
    document.body.innerHTML = `
      <button id="page" data-accesskey="x">Seite</button>
      <div data-modal-layer><button id="dlg-off" disabled data-accesskey="x">Aus</button><button id="dlg" data-accesskey="x">Dialog</button></div>`;
    let clicked = '';
    for (const b of Array.from(document.querySelectorAll('button'))) b.addEventListener('click', () => (clicked = b.id));
    expect(triggerAccessKey('x')).toBe(true);
    expect(clicked).toBe('dlg');
    expect(triggerAccessKey('q')).toBe(false);
    document.body.innerHTML = '';
  });
});
