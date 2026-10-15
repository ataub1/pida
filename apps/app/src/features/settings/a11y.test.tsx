/**
 * Barrierefreiheit der Einstellungen (ADR 0004 §4.7, Abnahme E3 Punkt 8): axe ohne Verstöße je Bereich und im
 * Suchmodus (Treffer und ohne Treffer), mit Feldfehler, mit gesperrter Platzhalterart, schreibgeschützt, auf
 * Mobilgeräten (Vollbild) und im PKCS#12-Import-Assistenten.
 */
import { screen, within } from '@testing-library/react';
import { afterEach, describe, it } from 'vitest';
import { defaultSettings } from '../../core/defaults';
import { expectNoAxeViolations } from '../../test/a11y';
import { emulateMedia, renderApp } from '../../test/render';
import { IOS, onMobile, settingsView } from '../../test/views';

const SECTIONS_DE = ['Signaturbild', 'Dokument', 'Signiermittel', 'Speichern', 'Darstellung', 'Datenschutz', 'Erweitert', 'Über PIDA'];

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

async function eachSection(dialog: HTMLElement, user: ReturnType<typeof renderApp>['user']) {
  const nav = within(dialog).getByRole('navigation', { name: 'Bereiche' });
  for (const name of SECTIONS_DE) {
    await user.click(within(nav).getByRole('button', { name }));
    within(dialog).getByRole('heading', { level: 3, name });
    await expectNoAxeViolations();
  }
}

describe('Einstellungen: axe', () => {
  it('Desktop: jeder Bereich (mit Beispielen, Keystore eingerichtet)', async () => {
    const base = defaultSettings();
    const settings = { ...base, keystore: { ...base.keystore, enabled: true, file: '/Users/demo/max.p12' }, output: { ...base.output, folder: '/Users/demo/Signiert' } };
    const { user } = renderApp({ examples: true, settings, scenario: 'contract' });
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await eachSection(dialog, user);
  });

  it('Suchmodus: Treffer mit Vorschau, mehrere Bereiche, ohne Treffer', async () => {
    const { user } = renderApp();
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const search = within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' });
    for (const q of ['logo', 'sprache', 'pdf/a', 'xyz']) {
      await user.clear(search);
      await user.type(search, q);
      await expectNoAxeViolations();
    }
  });

  it('Feldfehler (Proxy-Port), Platzhalterart gesperrt bzw. frei, Weitere Profile', async () => {
    const { user } = renderApp();
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const nav = within(dialog).getByRole('navigation', { name: 'Bereiche' });
    await user.click(within(dialog).getByRole('button', { name: 'Weitere Profile' }));
    await expectNoAxeViolations();
    await user.click(within(nav).getByRole('button', { name: 'Dokument' }));
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' }));
    await expectNoAxeViolations();
    await user.click(within(nav).getByRole('button', { name: 'Erweitert' }));
    await user.click(within(dialog).getByRole('radio', { name: 'Manuell' }));
    await user.type(within(dialog).getByLabelText('Port'), '99999');
    await user.tab();
    await within(dialog).findByText('Der Port muss zwischen 1 und 65535 liegen.');
    await expectNoAxeViolations();
  });

  it('schreibgeschützt und englisch', async () => {
    renderApp({ view: settingsView(defaultSettings(), { read_only: true }, 'en') });
    await screen.findByRole('dialog', { name: 'Settings' });
    await expectNoAxeViolations();
  });

  it('Mobile (Vollbild, schmal): jeder Bereich und Suche', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { user } = renderApp({ view: onMobile(settingsView(defaultSettings(), { fido2_available: false }), IOS) });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await eachSection(dialog, user);
    await user.type(within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'keystore');
    await expectNoAxeViolations();
  });

  it('PKCS#12-Import-Assistent (Mobile)', async () => {
    const { user } = renderApp({ platform: 'android' });
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Signiermittel' }));
    await user.click(within(dialog).getByRole('button', { name: 'PKCS#12-Datei importieren …' }));
    const wizard = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    await expectNoAxeViolations();
    await user.click(within(wizard).getByRole('button', { name: 'Datei wählen …' }));
    await within(wizard).findByTestId('keystore-import-file');
    await expectNoAxeViolations();
  });
});
