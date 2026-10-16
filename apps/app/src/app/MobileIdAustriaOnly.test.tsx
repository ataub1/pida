/**
 * Mobile nur mit ID Austria (Entscheidung des Auftraggebers 16.10.2026, ADR 0004 Nachtrag 11): auf iOS und
 * Android erscheint in keiner Ansicht ein Keystore – kein „Keystore“, keine „Datei mit Zertifikat“, kein
 * „Passwort merken?“, kein PKCS#12-Import, keine Schlüsselauswahl, kein „Sicherer Speicher“ für
 * Keystore-Passwörter. Durchgespielt mit dem Mock-Kern (der dem Kern folgt: nur ID Austria) – Start,
 * Arbeitsbereich mit Konto-Chip, Signierkarte, Einstellungen (jeder Bereich und Suche), Ersteinrichtung (mit und
 * ohne Altkonfiguration), Stapel und „Fertig“; jeweils mit axe. Desktop-Gegenprobe: dort bleibt der Keystore.
 */
import { screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import { expectNoAxeViolations } from '../test/a11y';
import { emulateMedia, renderApp } from '../test/render';
import { doneView, onMobile, outputResult, ANDROID, IOS } from '../test/views';

const FORBIDDEN = /Keystore|PKCS|Datei mit Zertifikat|Zertifikatsdatei|Passwort merken|Sicherer Speicher|sicheren Speicher|Schlüssel anzeigen/i;

/** Sichtbarer Text der ganzen Seite (inkl. Dialoge als Portal) ohne Keystore-Begriffe. */
function expectNoKeystoreText(where: string) {
  const text = document.body.textContent ?? '';
  const hit = text.match(FORBIDDEN);
  expect(hit?.[0] ?? null, `${where}: „${hit?.[0]}“ in ${text.slice(Math.max(0, (hit?.index ?? 0) - 60), (hit?.index ?? 0) + 60)}`).toBeNull();
}

const PLATFORMS = [
  ['ios', IOS],
  ['android', ANDROID],
] as const;

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe.each(PLATFORMS)('Mobile nur mit ID Austria (%s)', (platform, view) => {
  it('Start, Arbeitsbereich mit Konto-Chip und Signierkarte', async () => {
    const { user } = renderApp({ platform, scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expectNoKeystoreText('Arbeitsbereich');
    await expectNoAxeViolations();
    const chip = screen.getByTestId('signer-chip');
    expect(chip).toHaveTextContent('ID Austria');
    await user.click(chip);
    const menu = await screen.findByRole('menu');
    expect(within(menu).getAllByRole('menuitem').map((i) => i.textContent)).toEqual(['Einstellungen …']);
    expectNoKeystoreText('Konto-Chip');
    await expectNoAxeViolations();
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const card = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(card).toBeInTheDocument();
    expectNoKeystoreText('Signierkarte');
    await expectNoAxeViolations();
  });

  it('Einstellungen: jeder Bereich ohne Keystore, Suche findet keinen', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { user } = renderApp({ platform });
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const nav = within(dialog).getByRole('navigation', { name: 'Bereiche' });
    for (const section of within(nav).getAllByRole('button')) {
      await user.click(section);
      expectNoKeystoreText(`Einstellungen › ${section.textContent}`);
    }
    await expectNoAxeViolations();
    const search = within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' });
    for (const q of ['keystore', 'pkcs12', 'p12', 'zertifikatsdatei', 'alias', 'schlüsselbund']) {
      await user.clear(search);
      await user.type(search, q);
      expect(dialog.querySelectorAll('[data-row]'), q).toHaveLength(0);
    }
    await expectNoAxeViolations();
  });

  it('Ersteinrichtung ohne und mit Altkonfiguration', async () => {
    const first = renderApp({ platform, firstRun: true });
    let dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expectNoKeystoreText('Ersteinrichtung');
    await expectNoAxeViolations();
    first.unmount();
    const second = renderApp({ platform, firstRun: true, migrationOffer: '/Users/demo/.pdf-over/PDF-Over.config', migrationSecrets: true });
    dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expect(within(dialog).getByTestId('migration-note')).toBeInTheDocument();
    expectNoKeystoreText('Übernahme');
    await expectNoAxeViolations();
    await second.user.click(within(dialog).getByRole('button', { name: /^Übernehmen/ }));
    await within(dialog).findByText('Schritt 2 von 2');
    expectNoKeystoreText('Signaturbild');
  });

  it('Stapel: Übersicht und „Alle signieren mit ID Austria“', async () => {
    const { user } = renderApp({ platform, scenario: 'batch' });
    await screen.findByRole('button', { name: /Alle signieren/ });
    expect(screen.getByTestId('signer-chip')).toHaveTextContent('ID Austria');
    expectNoKeystoreText('Stapel');
    await expectNoAxeViolations();
    await user.click(screen.getByTestId('signer-chip'));
    await screen.findByRole('menu');
    expectNoKeystoreText('Stapel › Konto-Chip');
  });

  it('„Fertig“ mit Teilen und Sichern', async () => {
    renderApp({ view: onMobile(doneView(outputResult({ path: '/var/mobile/X/Documents/Vertrag_signed.pdf' })), view) });
    await screen.findByRole('heading', { level: 1 });
    expectNoKeystoreText('Fertig');
    await expectNoAxeViolations();
  });
});

describe('Desktop-Gegenprobe', () => {
  it('Konto-Chip und Einstellungen bieten den Keystore weiter an', async () => {
    const base = defaultSettings();
    const settings = { ...base, keystore: { ...base.keystore, enabled: true, file: '/Users/demo/Zertifikate/max.p12' } };
    const { user } = renderApp({ scenario: 'contract', settings });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByTestId('signer-chip'));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: /Keystore/ })).toBeInTheDocument();
    expect(within(menu).getByText('Signiermittel wechseln')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Signiermittel' }));
    expect(within(dialog).getByRole('region', { name: 'Keystore' })).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Keystore/ })).toBeInTheDocument();
  });
});
