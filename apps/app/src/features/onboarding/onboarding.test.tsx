/**
 * Ersteinrichtung (Paket UI-B, ADR 0004 §3.12, Abnahme E4 Punkte 5 und 6 [M]): bei fehlender Einstellungsdatei
 * mit Altkonfiguration die Liste der Übernahmen (Schritt 1), ohne Altkonfiguration direkt Schritt 2;
 * Standard-Signiermittel und Signaturbild wirken sofort (`update_first_run`), „Los geht’s“ (`finish_first_run`)
 * bzw. „Später einrichten“ (`postpone_first_run`); Bestandsnutzer sehen sie nie; erneut über Einstellungen › Erweitert
 * (`offer_migration`). Das PIDA-Siegel bleibt, nur eine echte eigene Bildmarke der Altkonfiguration wird
 * übernommen (Entscheidung des Auftraggebers, ADR 0005). Proben des Kerns, Mobile, axe je Schritt.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ViewState } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { emulateMedia, lastEvent, renderApp } from '../../test/render';

const LEGACY = '/Users/demo/.pdf-over/PDF-Over.config';
const FIXTURE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../core/fixtures/view-states.json');
const PROBES = JSON.parse(readFileSync(FIXTURE, 'utf8')) as { name: string; view: ViewState }[];
const probe = (name: string) => structuredClone(PROBES.find((p) => p.name === name)?.view as ViewState);

const welcome = () => screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Ersteinrichtung', () => {
  it('mit Altkonfiguration: Liste der Übernahmen, Siegel bleibt; Übernehmen → Signiermittel → Signaturbild → Los geht’s', async () => {
    const { mock, user } = renderApp({ firstRun: true, migrationOffer: LEGACY });
    const dialog = await welcome();
    expect(within(dialog).getByText('Schritt 1 von 3')).toBeInTheDocument();
    expect(within(dialog).getByRole('heading', { level: 3, name: 'PDF-Over-Einstellungen gefunden' })).toBeInTheDocument();
    expect(within(dialog).getByText(LEGACY)).toBeInTheDocument();
    const list = within(dialog).getByRole('list', { name: 'Übernommen werden:' });
    const items = within(list).getAllByRole('listitem').map((li) => li.textContent);
    expect(items).toEqual([
      'Signaturbild (Profil, Sprache, Hinweistext)',
      'Benutzername für ID Austria: +43 660 ···123',
      'Speicherort und Dateiname: ~/Dokumente/Signiert',
      'Keystore: max.p12',
      'Proxy',
      'Platzhaltersuche',
      'Sprache: Deutsch',
      'Bildmarke: Es bleibt das PIDA-Siegel.',
    ]);
    // keine erfundene Bildmarke: kein Eintrag „Bildmarke: …“ außer dem Hinweis auf das Siegel
    expect(items.filter((i) => i?.startsWith('Bildmarke'))).toEqual(['Bildmarke: Es bleibt das PIDA-Siegel.']);
    await expectNoAxeViolations();

    await user.click(within(dialog).getByRole('button', { name: 'Übernehmen' }));
    expect(mock.dispatched).toContainEqual({ type: 'accept_migration', store_secrets: false });
    expect(await screen.findByText('Übernommen: 7 Einstellungen; nicht übernommen: 0.')).toBeInTheDocument();
    expect(await within(dialog).findByText('Schritt 2 von 3')).toBeInTheDocument();
    const title = within(dialog).getByRole('heading', { level: 3, name: 'Womit signieren Sie meistens?' });
    await waitFor(() => expect(title).toHaveFocus());
    // nach der Übernahme ist der Keystore eingerichtet und wählbar
    const keystore = within(dialog).getByRole('radio', { name: /Keystore/ });
    expect(keystore).toBeEnabled();
    await expectNoAxeViolations();
    await user.click(keystore);
    expect(lastEvent(mock)).toEqual({ type: 'update_first_run', default_backend: 'keystore', profile: 'SIGNATURBLOCK_SMALL', block_language: 'de' });
    expect(keystore).toBeChecked();
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));

    expect(within(dialog).getByText('Schritt 3 von 3')).toBeInTheDocument();
    expect(within(dialog).getByRole('heading', { level: 3, name: 'So sieht Ihre Signatur aus' })).toBeInTheDocument();
    const before = within(dialog).getByRole('img', { name: /^Vorschau des Signaturblocks/ }).getAttribute('src');
    await user.click(within(dialog).getByRole('radio', { name: 'Amtssignatur' }));
    expect(lastEvent(mock)).toEqual({ type: 'update_first_run', default_backend: 'keystore', profile: 'AMTSSIGNATURBLOCK', block_language: 'de' });
    await waitFor(() => expect(within(dialog).getByRole('img', { name: /^Vorschau des Signaturblocks: Amtssignatur/ }).getAttribute('src')).not.toBe(before));
    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(lastEvent(mock)).toMatchObject({ type: 'update_first_run', block_language: 'en' });
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Zurück' }));
    expect(within(dialog).getByRole('radio', { name: /Keystore/ })).toBeChecked();
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));
    await user.click(within(dialog).getByRole('button', { name: 'Los geht’s' }));
    expect(lastEvent(mock)).toEqual({ type: 'finish_first_run' });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(await screen.findByText('Einrichtung abgeschlossen. Sie können alles jederzeit in den Einstellungen ändern.')).toBeInTheDocument();
    // die Wahl gilt: Konto-Chip im Arbeitsbereich zeigt den Keystore
    const settings = (await mock.view()).signers;
    expect(settings.default).toBe('keystore');
  });

  it('ohne Altkonfiguration direkt Schritt 2 (von 2); Keystore erst nach dem Einrichten wählbar', async () => {
    const { mock, user } = renderApp({ firstRun: true });
    const dialog = await welcome();
    expect(within(dialog).getByText('Schritt 1 von 2')).toBeInTheDocument();
    expect(within(dialog).queryByText('PDF-Over-Einstellungen gefunden')).not.toBeInTheDocument();
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Womit signieren Sie meistens?' })).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
    expect(within(dialog).getByText('Später in den Einstellungen einrichten.')).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Chipkarte/ })).toBeEnabled();
    expect(within(dialog).getByRole('radio', { name: /Jedes Mal fragen/ })).toBeChecked();
    await user.click(within(dialog).getByRole('radio', { name: /ID Austria/ }));
    expect(lastEvent(mock)).toEqual({ type: 'update_first_run', default_backend: 'id_austria', profile: 'SIGNATURBLOCK_SMALL', block_language: null });
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));
    expect(within(dialog).getByText('Schritt 2 von 2')).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: 'Wie System' })).toBeChecked();
    // Barrierefreie Bedienung: Standard „Nein“, „Ja“ setzt die Voreinstellungen sofort (docs/analyse/06)
    const a11y = within(dialog).getByRole('radiogroup', { name: 'Barrierefreie Bedienung verwenden?' });
    expect(within(a11y).getByRole('radio', { name: 'Nein' })).toBeChecked();
    await user.click(within(a11y).getByRole('radio', { name: 'Ja' }));
    expect(lastEvent(mock)).toMatchObject({ type: 'set_accessibility', accessibility: { mode: true, start: 'methods', check_before_sign: true } });
    await waitFor(() => expect(within(a11y).getByRole('radio', { name: 'Ja' })).toBeChecked());
    // die Vorschau ist die des Kerns; ohne eigene Bildmarke zeigt sie das PIDA-Siegel (keine erfundene)
    expect((await mock.view()).settings).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: 'Los geht’s' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('„Später einrichten“ (bzw. Esc) lehnt ein offenes Angebot ab und beendet die Einrichtung ohne „Einrichtung abgeschlossen“; kein zweiter Dialog', async () => {
    const { mock, user } = renderApp({ firstRun: true, migrationOffer: LEGACY });
    const dialog = await welcome();
    await user.click(within(dialog).getByRole('button', { name: 'Später einrichten' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(mock.dispatched.slice(-2)).toEqual([{ type: 'decline_migration' }, { type: 'postpone_first_run' }]);
    expect((await mock.view()).migration).toBeNull();
    expect(screen.getByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
    // Mobile-Check 16.10.2026: nach „Später einrichten“ nicht „Einrichtung abgeschlossen“, sondern wo es weitergeht
    expect(await screen.findByText('Signiermittel und Signaturbild können Sie jederzeit in den Einstellungen einrichten.')).toBeInTheDocument();
    expect(screen.queryByText(/Einrichtung abgeschlossen/)).not.toBeInTheDocument();

    const second = renderApp({ firstRun: true });
    await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    await second.user.keyboard('{Escape}');
    await waitFor(() => expect(lastEvent(second.mock)).toEqual({ type: 'postpone_first_run' }));
  });

  it('Bestandsnutzer sehen sie nie', async () => {
    renderApp({ examples: true });
    expect(await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('eigene Bildmarke der Altkonfiguration: eigener Eintrag, kein Siegel-Hinweis; Klartext-Passwörter nur mit Zustimmung', async () => {
    const { mock, user } = renderApp({ firstRun: true, migrationOffer: LEGACY, migrationSecrets: true, legacyEmblem: '/Users/demo/.pdf-over/eigenes-logo.png' } as Parameters<typeof renderApp>[0]);
    const dialog = await welcome();
    expect(within(dialog).getByText('Bildmarke: eigenes-logo.png')).toBeInTheDocument();
    expect(within(dialog).queryByTestId('seal-stays')).not.toBeInTheDocument();
    const consent = within(dialog).getByRole('checkbox', { name: /Schlüsselbund/ });
    expect(consent).not.toBeChecked();
    await user.click(consent);
    await user.click(within(dialog).getByRole('button', { name: 'Übernehmen' }));
    expect(mock.dispatched).toContainEqual({ type: 'accept_migration', store_secrets: true });
    await within(dialog).findByText('Schritt 2 von 3');
  });

  it('„Nicht übernehmen“: weiter mit Schritt 2, Werte bleiben', async () => {
    const { mock, user } = renderApp({ firstRun: true, migrationOffer: LEGACY });
    const dialog = await welcome();
    await user.click(within(dialog).getByRole('button', { name: 'Nicht übernehmen' }));
    expect(lastEvent(mock)).toEqual({ type: 'decline_migration' });
    expect(await within(dialog).findByText('Schritt 2 von 3')).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
  });

  it('Mobile: nur ID Austria – ohne Signiermittel-Schritt, gleich das Signaturbild, als Sheet; axe', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { mock, user } = renderApp({ firstRun: true, platform: 'ios' });
    const dialog = await welcome();
    expect(within(dialog).getByText('Schritt 1 von 1')).toBeInTheDocument();
    expect(within(dialog).queryByRole('radio', { name: /Chipkarte|Keystore|ID Austria/ })).not.toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Amtssignatur/ })).toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: 'Zurück' })).not.toBeInTheDocument();
    expect(dialog.textContent).not.toMatch(/Keystore|Chipkarte|PKCS/);
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    expect(lastEvent(mock)).toMatchObject({ type: 'update_first_run', profile: 'AMTSSIGNATURBLOCK' });
    // der Konto-Chip zeigt danach ID Austria (das einzige Signiermittel, als Standard)
    expect((await mock.view()).signers).toMatchObject({ default: 'atrust_native', options: [{ backend: 'atrust_native' }] });
  });

  it('Mobile mit Altkonfiguration: Übernahme erklärt „nur ID Austria“, danach gleich das Signaturbild', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { user } = renderApp({ firstRun: true, platform: 'android', migrationOffer: '/Users/demo/.pdf-over/PDF-Over.config' });
    const dialog = await welcome();
    expect(within(dialog).getByText('Schritt 1 von 2')).toBeInTheDocument();
    expect(within(dialog).getByTestId('migration-note')).toHaveTextContent('Signiermittel: ID Austria – andere Signiermittel aus PDF-Over gibt es auf diesem Gerät nicht');
    expect(dialog.textContent).not.toMatch(/Keystore|PKCS/);
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: /^Übernehmen/ }));
    expect(await within(dialog).findByText('Schritt 2 von 2')).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Amtssignatur/ })).toBeInTheDocument();
  });

  it('Proben des Kerns: Schritt 1 mit allen acht Einträgen (eigene Bildmarke), ohne Migration Schritt 2', async () => {
    const withMigration = probe('ws_first_run');
    const r = renderApp({ view: withMigration });
    let dialog = await welcome();
    expect(within(dialog).getAllByRole('listitem')).toHaveLength(8);
    expect(within(dialog).getByText('Bildmarke: logo.png')).toBeInTheDocument();
    await expectNoAxeViolations();
    r.unmount();
    renderApp({ view: probe('ws_first_run_without_migration') });
    dialog = await welcome();
    expect(within(dialog).getByText('Schritt 1 von 2')).toBeInTheDocument();
    // Vorschau erst ab Revision 1 (Revision 0 = noch keine Vorschau, Nachtrag 3)
    expect(within(dialog).queryByRole('img')).not.toBeInTheDocument();
  });
});

describe('Migrationsangebot ohne Ersteinrichtung', () => {
  it('eigener Dialog im neuen Stil, nur ausdrücklich zu beantworten', async () => {
    const { mock, user } = renderApp({ migrationOffer: LEGACY });
    const dialog = await screen.findByRole('dialog', { name: 'PDF-Over-Einstellungen übernehmen' });
    expect(within(dialog).queryByTestId('seal-stays')).not.toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.getByRole('dialog', { name: 'PDF-Over-Einstellungen übernehmen' })).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Nicht übernehmen' }));
    expect(lastEvent(mock)).toEqual({ type: 'decline_migration' });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('erneut über Einstellungen › Erweitert: Angebot im Bereich, Übernehmen wirkt sofort', async () => {
    const { mock, user } = renderApp({ migrationOffer: LEGACY });
    await user.click(within(await screen.findByRole('dialog', { name: 'PDF-Over-Einstellungen übernehmen' })).getByRole('button', { name: 'Nicht übernehmen' }));
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Einstellungen' }));
    const settings = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(within(settings).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Erweitert' }));
    await user.click(within(settings).getByRole('button', { name: 'Suchen …' }));
    expect(lastEvent(mock)).toEqual({ type: 'offer_migration' });
    const offer = await within(settings).findByTestId('migration-offer');
    expect(within(offer).getByRole('heading', { level: 4, name: 'PDF-Over-Einstellungen gefunden' })).toBeInTheDocument();
    expect(within(offer).getByText('Benutzername für ID Austria: +43 660 ···123')).toBeInTheDocument();
    // kein zweiter Dialog über den Einstellungen
    expect(screen.queryByRole('dialog', { name: 'PDF-Over-Einstellungen übernehmen' })).not.toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(offer).getByRole('button', { name: 'Übernehmen' }));
    expect(lastEvent(mock)).toEqual({ type: 'accept_migration', store_secrets: false });
    await waitFor(() => expect(within(settings).queryByTestId('migration-offer')).not.toBeInTheDocument());
    await user.click(within(within(settings).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Signiermittel' }));
    expect(within(settings).getByLabelText('ID Austria: Telefonnummer oder Benutzername')).toHaveValue('+43 660 1234123');
  });

  it('Einstellungen › Erweitert ohne Altkonfiguration: Hinweis „keine gefunden“; Suche findet die Zeile', async () => {
    const { user } = renderApp();
    await user.click(within(await screen.findByRole('banner')).getByRole('button', { name: 'Einstellungen' }));
    const settings = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.type(within(settings).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'pdf-over');
    await user.click(within(settings).getByRole('button', { name: 'Suchen …' }));
    // eine Quelle: der Hinweis des Kerns (`ng-migration-not-found`), über dem Dialog und für Screenreader
    // erreichbar (Top Layer), kein zweiter Text im Bereich
    const toast = await screen.findByText('Auf diesem Gerät wurden keine PDF-Over-Einstellungen gefunden.');
    expect(toast.closest('[data-notice-code]')).toHaveAttribute('data-notice-code', 'ng-migration-not-found');
    expect(toast.closest('[data-react-aria-top-layer]')).not.toBeNull();
    expect(within(settings).queryByText('Auf diesem Gerät wurden keine PDF-Over-Einstellungen gefunden.')).toBeNull();
    expect(settings.closest('[aria-hidden="true"]')).toBeNull();
  });
});
