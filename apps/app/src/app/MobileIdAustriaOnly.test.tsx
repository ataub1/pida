/**
 * Mobile nur mit ID Austria (Entscheidung des Auftraggebers 16.10.2026, ADR 0004 Nachtrag 11): auf iOS und
 * Android erscheint in keiner Ansicht ein Keystore – kein „Keystore“, keine „Datei mit Zertifikat“, kein
 * „Passwort merken?“, kein PKCS#12-Import, keine Schlüsselauswahl, kein „Sicherer Speicher“ für
 * Keystore-Passwörter. Durchgespielt mit dem Mock-Kern (der dem Kern folgt: nur ID Austria) – Start,
 * Arbeitsbereich, Signierkarte, Einstellungen (jeder Bereich und Suche), Ersteinrichtung (mit und ohne
 * Altkonfiguration), Stapel und „Fertig“; jeweils mit axe. Desktop-Gegenprobe: dort bleibt der Keystore.
 *
 * Seit Nachtrag 20 („es gibt nur ID Austria als einziges Mittel“) gibt es auf Mobilgeräten auch keine
 * Signiermittel-Auswahl: nirgends „Signiermittel wählen“/„Signiermittel wechseln“/„Signieren mit“ als Auswahl,
 * kein Konto-Chip, kein (leeres) Menü – weder im Arbeitsbereich (schmal und breit), im Stapel, in der Signierkarte,
 * der Ersteinrichtung, der Befehlspalette noch in der Kürzelübersicht. Einstellungen › Signiermittel bleibt der Ort
 * für das ID-Austria-Konto.
 */
import { screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import { expectNoAxeViolations } from '../test/a11y';
import { emulateMedia, renderApp } from '../test/render';
import { doneView, onMobile, outputResult, ANDROID, IOS } from '../test/views';

const FORBIDDEN = /Keystore|PKCS|Datei mit Zertifikat|Zertifikatsdatei|Passwort merken|Sicherer Speicher|sicheren Speicher|Schlüssel anzeigen/i;
/** Signiermittel-Auswahl (Nachtrag 20): Konto-Chip, Menü, Abschnitt „Signieren mit“, Rückfall „Signiermittel wählen“. */
const CHOICE = /Signiermittel wählen|Signiermittel wechseln|Signieren mit\b(?! ID Austria)|Als Standard verwenden|Jedes Mal fragen/;

/** Sichtbarer Text der ganzen Seite (inkl. Dialoge als Portal) ohne Keystore-Begriffe. */
function expectNoKeystoreText(where: string) {
  const text = document.body.textContent ?? '';
  const hit = text.match(FORBIDDEN);
  expect(hit?.[0] ?? null, `${where}: „${hit?.[0]}“ in ${text.slice(Math.max(0, (hit?.index ?? 0) - 60), (hit?.index ?? 0) + 60)}`).toBeNull();
}

/**
 * Keine Signiermittel-Auswahl (Nachtrag 20): kein Konto-Chip, kein Menü (außer der Befehlsliste der Palette, `palette`),
 * keine Auswahltexte (auch nicht als zugänglicher Name).
 */
function expectNoSignerChoice(where: string, palette = false) {
  expect(screen.queryByTestId('signer-chip'), where).toBeNull();
  if (!palette) expect(screen.queryByRole('menu'), where).toBeNull();
  const text = document.body.textContent ?? '';
  const names = [...document.querySelectorAll('[aria-label]')].map((e) => e.getAttribute('aria-label') ?? '').join(' | ');
  for (const t of [text, names]) {
    const hit = t.match(CHOICE);
    expect(hit?.[0] ?? null, `${where}: „${hit?.[0]}“ in ${t.slice(Math.max(0, (hit?.index ?? 0) - 60), (hit?.index ?? 0) + 60)}`).toBeNull();
  }
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
  it('Start, Arbeitsbereich ohne Konto-Chip (breit und schmal) und Signierkarte', async () => {
    const { user, unmount } = renderApp({ platform, scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expectNoKeystoreText('Arbeitsbereich');
    expectNoSignerChoice('Arbeitsbereich breit');
    await expectNoAxeViolations();
    unmount();
    restore = emulateMedia({ narrow: true, coarse: true });
    const narrow = renderApp({ platform, scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expectNoSignerChoice('Arbeitsbereich schmal');
    await narrow.user.click(screen.getByRole('button', { name: /Optionen einblenden/ }));
    expectNoSignerChoice('Arbeitsbereich schmal, ausgeklappt');
    await expectNoAxeViolations();
    // „Signieren“ signiert direkt mit ID Austria (ausdrückliche Aktion, kein Menü)
    await narrow.user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const card = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(card).toBeInTheDocument();
    expectNoKeystoreText('Signierkarte');
    expect(within(card).queryByText(/Signiermittel wählen|Signiermittel wechseln/)).toBeNull();
    await expectNoAxeViolations();
    void user;
  });

  it('Befehlspalette, Suche darin und Kürzelübersicht: keine Signiermittel-Auswahl', async () => {
    const { user } = renderApp({ platform, scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByRole('button', { name: 'Befehle und Suche' }));
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    expectNoSignerChoice('Befehlspalette', true);
    const search = within(palette).getByRole('searchbox', { name: 'Befehl suchen …' });
    for (const q of ['signiermittel', 'wechseln', 'konto', 'id austria']) {
      await user.clear(search);
      await user.type(search, q);
      expectNoSignerChoice(`Befehlspalette „${q}“`, true);
    }
    await user.clear(search);
    await user.click(within(palette).getByRole('menuitem', { name: /^Tastenkürzel anzeigen/ }));
    await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    expectNoSignerChoice('Kürzelübersicht');
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
    expectNoSignerChoice('Ersteinrichtung');
    expect(within(dialog).getByText('Schritt 1 von 1')).toBeInTheDocument();
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
    expectNoSignerChoice('Ersteinrichtung › Signaturbild');
  });

  it('Stapel: Übersicht ohne Konto-Chip, „Alle signieren“ startet mit ID Austria', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { user, mock } = renderApp({ platform, scenario: 'batch' });
    const signAll = await screen.findByRole('button', { name: /Alle signieren/ });
    expectNoKeystoreText('Stapel');
    expectNoSignerChoice('Stapel');
    expect(screen.getByTestId('privacy-note')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(signAll);
    expect(screen.queryByRole('menu')).toBeNull();
    // Startdialog (Ordner/vorhandene Dateien) bzw. direkt `start_batch` – immer mit ID Austria, ohne Wahl
    const dialog = screen.queryByRole('dialog');
    if (dialog) expectNoSignerChoice('Stapel › Startdialog');
    else expect(mock.dispatched.find((e) => e.type === 'start_batch')).toMatchObject({ backend: 'atrust_native' });
  });

  it('Einstellungen eines früheren Builds (Keystore als Standard, Datei, Schlüsselbund): nirgends ein Keystore', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const base = defaultSettings();
    const settings = {
      ...base,
      backend: { default: 'keystore' as const },
      keystore: { ...base.keystore, enabled: true, file: '/var/mobile/Containers/Data/Application/ALT/data/keystores/max.p12', alias: 'max', password_storage: 'keychain' as const, storage_chosen: false },
    };
    const { user, mock } = renderApp({ platform, scenario: 'contract', settings });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expectNoKeystoreText('Arbeitsbereich (früherer Build)');
    expectNoSignerChoice('Arbeitsbereich (früherer Build)');
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const nav = within(dialog).getByRole('navigation', { name: 'Bereiche' });
    for (const section of within(nav).getAllByRole('button')) {
      await user.click(section);
      expectNoKeystoreText(`Einstellungen › ${section.textContent} (früherer Build)`);
    }
    await expectNoAxeViolations();
    expect((await mock.view()).signers).toMatchObject({ default: 'atrust_native', selected: 'atrust_native', choice_offered: false, options: [{ backend: 'atrust_native' }] });
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
