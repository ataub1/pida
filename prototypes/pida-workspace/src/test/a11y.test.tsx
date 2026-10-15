/** Barrierefreiheit: axe-core ohne Verstöße für alle Hauptansichten und Overlays. */
import { fireEvent, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations, login, renderApp, signingDialog } from './render';

describe('axe – Hauptansichten', () => {
  it('Start/Leerzustand', async () => {
    renderApp({ scenario: 'start' });
    expect(screen.getByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Ersteinrichtung', async () => {
    renderApp({ scenario: 'start', onboarded: false });
    expect(await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Arbeitsbereich (Vertrag)', async () => {
    renderApp({ scenario: 'contract' });
    expect(screen.getByRole('application', { name: 'Signaturblock' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Arbeitsbereich mit Signaturfeld und bereits signiertes Dokument', async () => {
    const { unmount } = renderApp({ scenario: 'field' });
    expect(screen.getByRole('button', { name: /Hier signieren: Signaturfeld/ })).toBeInTheDocument();
    await expectNoAxeViolations();
    unmount();
    renderApp({ scenario: 'signed' });
    expect(screen.getAllByText('Dr. Anna Gruber').length).toBeGreaterThan(0);
    await expectNoAxeViolations();
  });

  it('Fehler: kennwortgeschütztes PDF', async () => {
    renderApp({ scenario: 'error-password' });
    expect(screen.getByRole('heading', { name: 'Dieses PDF ist mit einem Kennwort geschützt' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Signierkarte: Datenschutz, Anmeldung, App-Bestätigung', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await signingDialog();
    expect(within(dialog).getByRole('heading', { name: 'Kurz zum Datenschutz' })).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Verstanden, weiter' }));
    expect(await within(dialog).findByLabelText('Signatur-Passwort')).toBeInTheDocument();
    await expectNoAxeViolations();
    await login(user);
    expect(await within(dialog).findByText('Benachrichtigung an die ID-Austria-App gesendet')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Signierkarte: FIDO2, SMS-TAN und Fehler', async () => {
    const { user, unmount } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    let dialog = await login(user);
    await user.click(await within(dialog).findByRole('button', { name: 'FIDO2-Schlüssel' }));
    expect(await within(dialog).findByLabelText('PIN des Sicherheitsschlüssels')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'SMS-TAN' }));
    expect(await within(dialog).findByLabelText('TAN aus der SMS')).toBeInTheDocument();
    await expectNoAxeViolations();
    unmount();

    const r = renderApp({ scenario: 'error-atrust', privacyAcked: true });
    await r.user.click(screen.getByRole('button', { name: /^Signieren/ }));
    dialog = await login(r.user);
    expect(await within(dialog).findByRole('heading', { name: 'ID Austria ist gerade nicht erreichbar' })).toBeInTheDocument();
    await r.user.click(within(dialog).getByRole('button', { name: 'Technische Details' }));
    await expectNoAxeViolations();
  });

  it('Fertig', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    await user.click(await within(dialog).findByRole('button', { name: 'In App bestätigt (Demo)' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Stapel (Liste, Fortschritt abgeschlossen)', async () => {
    const { user } = renderApp({ scenario: 'batch', privacyAcked: true });
    expect(screen.getByRole('heading', { level: 1, name: '4 Dokumente' })).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: /Alle signieren/ }));
    const dialog = await login(user);
    await user.click(await within(dialog).findByRole('button', { name: 'In App bestätigt (Demo)' }));
    expect(await screen.findByText('Stapel abgeschlossen')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Einstellungen (alle Bereiche und Suche)', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    for (const section of ['Signaturbild', 'Signiermittel', 'Speichern', 'Darstellung', 'Erweitert']) {
      await user.click(within(dialog).getByRole('button', { name: section }));
      expect(within(dialog).getByRole('heading', { level: 3, name: section })).toBeInTheDocument();
      await expectNoAxeViolations();
    }
    await user.click(within(dialog).getByRole('radio', { name: 'Manuell' }));
    await user.type(within(dialog).getByLabelText('Port'), '99999');
    expect(within(dialog).getByText('Der Port muss zwischen 1 und 65535 liegen.')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.type(within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'logo');
    await expectNoAxeViolations();
  });

  it('Prüfmodus (leer und Ergebnis mit Zertifikatsdetails)', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Prüfmodus öffnen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiertes PDF zum Prüfen ablegen' })).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: 'Beispiel prüfen' }));
    await user.click((await screen.findAllByRole('button', { name: 'Zertifikatsdetails' }))[0]!);
    await expectNoAxeViolations();
  });

  it('Befehlspalette, Tastenkürzel, Demo-Steuerung', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    expect(await screen.findByRole('dialog', { name: 'Befehle' })).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.keyboard('{Escape}');
    fireEvent.keyDown(window, { key: '?' });
    expect(await screen.findByRole('dialog', { name: 'Tastenkürzel' })).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('button', { name: 'Demo' }));
    expect(screen.getByRole('complementary', { name: 'Demo-Steuerung' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Englische Oberfläche', async () => {
    renderApp({ scenario: 'contract', settings: { uiLang: 'en' } });
    expect(screen.getByRole('application', { name: 'Signature block' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});

describe('axe – schmale Ansicht (Bottom-Sheet)', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('Seitenleiste als ausklappbares Bottom-Sheet', async () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query.includes('max-width: 760px'),
      media: query,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    const { user } = renderApp({ scenario: 'contract' });
    const handle = screen.getByRole('button', { name: 'Optionen einblenden' });
    expect(handle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('switch', { name: 'Unsichtbar signieren' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Signieren/ })).toBeInTheDocument();
    expect(screen.queryByRole('navigation', { name: 'Seiten' })).not.toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(handle);
    expect(screen.getByRole('button', { name: 'Optionen ausblenden' })).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('switch', { name: 'Unsichtbar signieren' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});

describe('axe – Gegenprobe', () => {
  it('erkennt Verstöße (die Prüfung ist nicht leer)', async () => {
    const host = document.createElement('main');
    host.innerHTML = '<img src="x.png"><button></button><input type="text">';
    document.body.appendChild(host);
    const axe = (await import('axe-core')).default;
    const results = await axe.run(host, { rules: { 'color-contrast': { enabled: false } } });
    const ids = results.violations.map((v) => v.id);
    expect(ids).toEqual(expect.arrayContaining(['image-alt', 'button-name', 'label']));
    host.remove();
  });
});
