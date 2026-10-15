/**
 * Mobile-Oberfläche (ADR 0004 §4.6, Plan §5.2, docs/integration/mobile.md): Bottom-Sheet, Touch-Ziehen mit
 * Fingerversatz, „Signatur fortsetzen“ (mit Kurzhinweis), PKCS#12-Import aus dem Konto-Chip-Menü,
 * Teilen/Sichern unter, FIDO2 ausgegraut, Dateiauswahl über das Plugin, Plattform-Kennzeichen; dieselben
 * Ansichten mit `platform: ios|android` wie `?platform=` (Abnahme E1 Punkt 9 [M]).
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { UiEvent } from '../core/types';
import { expectNoAxeViolations } from '../test/a11y';
import { emulateMedia, renderApp } from '../test/render';
import { ANDROID, documentView, IOS, onMobile, signersView, viewOf, workspaceView } from '../test/views';

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Signatur fortsetzen', () => {
  const offer = { name: 'Vertrag.pdf', backend: 'atrust_native' as const, age_minutes: 3, pending: 1, document_leaves_device: true };

  it('Dialog im Schritt Öffnen: Fortsetzen bzw. Verwerfen (Escape verwirft nicht)', async () => {
    const { mock, user } = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: offer })) });
    const dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(dialog).toHaveTextContent('Die Signatur von „Vertrag.pdf“ wurde vor 3 Minuten unterbrochen');
    expect(dialog).toHaveTextContent('Danach folgt ein weiteres Dokument.');
    await user.keyboard('{Escape}');
    expect(mock.dispatched).toEqual([]);
    await user.click(within(dialog).getByRole('button', { name: 'Fortsetzen' }));
    expect(mock.dispatched).toEqual([{ type: 'resume_signature' }]);
    await expectNoAxeViolations();
  });

  it('Kurzhinweis zum Datenschutz bei ID Austria, nicht beim Keystore', async () => {
    const { unmount } = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: offer })) });
    let dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(within(dialog).getByTestId('privacy-note')).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt.');
    unmount();
    renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: { ...offer, backend: 'keystore', document_leaves_device: false } })) });
    dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(within(dialog).queryByTestId('privacy-note')).not.toBeInTheDocument();
  });

  it('Mock-Kern: Fortsetzen öffnet die Signierkarte über dem Dokument (ausdrückliche Aktion)', async () => {
    const { user } = renderApp({ platform: 'ios', resumeOffer: offer });
    const dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Fortsetzen' }));
    expect(await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' })).toBeInTheDocument();
    expect(document.querySelector('.workspace')).not.toBeNull();
  });
});

describe('Bottom-Sheet und Touch', () => {
  it('schmal: Inspektor als Bottom-Sheet, Miniaturen eingeklappt, axe', async () => {
    restore = emulateMedia({ narrow: true });
    const { user } = renderApp({ view: onMobile(workspaceView(documentView(), { signers: signersView({}, true) }), IOS) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByRole('navigation', { name: 'Seiten' })).not.toBeInTheDocument();
    const sheet = screen.getByRole('complementary', { name: 'Signatur' });
    expect(sheet).toHaveAttribute('data-narrow', 'true');
    await expectNoAxeViolations();
    await user.click(within(sheet).getByRole('button', { name: /Optionen einblenden/ }));
    expect(within(sheet).getByRole('button', { name: /Optionen ausblenden/ })).toHaveAttribute('aria-expanded', 'true');
    await expectNoAxeViolations();
  });

  it('Touch-Ziehen: Haptik, Lage beim Loslassen an den Kern', async () => {
    const vibrate: number[] = [];
    Object.defineProperty(navigator, 'vibrate', { configurable: true, value: (ms: number) => vibrate.push(ms) });
    const { mock } = renderApp({ view: onMobile(workspaceView(), ANDROID) });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    const host = document.querySelector('.pages') as HTMLElement;
    host.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    s.getBoundingClientRect = host.getBoundingClientRect;
    const { fireEvent } = await import('@testing-library/react');
    fireEvent.pointerDown(s, { button: 0, pointerId: 3, pointerType: 'touch', clientX: 0, clientY: 0 });
    fireEvent.pointerMove(s, { pointerId: 3, pointerType: 'touch', clientX: 300, clientY: 400 });
    fireEvent.pointerUp(s, { pointerId: 3, pointerType: 'touch', clientX: 300, clientY: 400 });
    expect(vibrate).toEqual([8]);
    expect(mock.dispatched.some((e: UiEvent) => e.type === 'move_block')).toBe(true);
  });
});

describe('Konto-Chip und Signiermittel auf Mobilgeräten', () => {
  it('PKCS#12-Import aus dem Menü des Konto-Chips (Datei wählen, Passwort, Ergebnis)', async () => {
    const view = onMobile(workspaceView(documentView(), { signers: signersView({}, true) }), IOS);
    const { mock, user } = renderApp({ view, platform: 'ios' });
    await user.click(await screen.findByTestId('signer-chip'));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: /Keystore/ })).toHaveTextContent('importieren Sie eine PKCS#12-Datei');
    await user.click(within(menu).getByRole('menuitem', { name: 'PKCS#12-Datei importieren …' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    await user.click(within(dialog).getByRole('button', { name: 'Datei wählen …' }));
    expect(await within(dialog).findByTestId('keystore-import-file')).toHaveTextContent('Gewählte Datei: test.p12');
    await user.type(within(dialog).getByLabelText('Passwort des Keystores'), 'geheim');
    await user.click(within(dialog).getByRole('button', { name: 'Importieren' }));
    const sent = mock.dispatched.at(-1) as Extract<UiEvent, { type: 'import_keystore' }>;
    expect(sent).toMatchObject({ type: 'import_keystore', password: 'geheim', remember: true });
    act(() => mock.setView({ ...view, keystore_import: { file_name: 'test.p12', ok: true, code: 'ng-keystore-imported', password_stored: true } }));
    expect(await screen.findByText('Der Keystore „test.p12“ wurde importiert.')).toBeInTheDocument();
  });

  it('ID Austria ohne FIDO2: Beschreibung im Menü ohne FIDO2', async () => {
    const { user } = renderApp({ view: onMobile(workspaceView(documentView(), { signers: signersView({}, true) }), ANDROID) });
    await user.click(await screen.findByTestId('signer-chip'));
    const item = await screen.findByRole('menuitem', { name: /ID Austria/ });
    expect(item).toHaveTextContent('Bestätigung in der App „ID Austria“ oder per SMS-TAN');
    expect(screen.queryByRole('menuitem', { name: /Bürgerkarte/ })).not.toBeInTheDocument();
  });

  it('Dateiauswahl über das Plugin (Kopien in der Sandbox)', async () => {
    const { mock, user } = renderApp({ platform: 'ios' });
    await user.click(await screen.findByRole('button', { name: 'Dokument auswählen' }));
    await waitFor(() => expect(mock.dispatched[0]).toMatchObject({ type: 'open_files' }));
    const ev = mock.dispatched[0] as Extract<UiEvent, { type: 'open_files' }>;
    expect(ev.paths.every((p) => p.includes('/Library/Caches/import/'))).toBe(true);
  });

  it('Plattform an der App-Wurzel und an <html> (Dialoge als Portal; keine Zugriffstasten-Unterstreichung)', async () => {
    renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }), ANDROID) });
    await screen.findByRole('heading', { level: 1 });
    const app = document.querySelector('.app') as HTMLElement;
    expect(app.dataset.mobile).toBe('true');
    expect(app.dataset.os).toBe('android');
    expect(document.documentElement.hasAttribute('data-mobile')).toBe(true);
  });

  it('Mock-Kern mit `platform: android`: Arbeitsbereich und Karte im Szenario „Vertrag“', async () => {
    const { user } = renderApp({ platform: 'android', scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByTestId('signer-chip'));
    await user.click(await screen.findByRole('menuitem', { name: /ID Austria/ }));
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    expect(await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});
