/**
 * Mobile-Oberfläche (Plan §5.2, docs/integration/mobile.md): Touch-Positionierung mit Fingerversatz,
 * „Signatur fortsetzen“, PKCS#12-Import-Assistent, Teilen/Exportieren, FIDO2 ausgegraut mit Hinweis,
 * Dateiauswahl über das Plugin, Einstellungen ohne Desktop-Bereiche.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import type { BackendOptionView, Capabilities, UiEvent, ViewState } from '../core/types';
import { grabPoint, TOUCH_OFFSET_PT } from '../screens/PositionScreen';
import { keystoreOnlyChanged } from '../settings/SettingsDialog';
import { renderApp } from '../test/render';
import { ANDROID, ctx, DOC, IOS, backendView, formView, onMobile, outputResult, positionView, settingsView, viewOf } from '../test/views';

const native: Capabilities = {
  input: 'Document',
  output: 'Cms',
  certificate: 'OnlyForVisibleBlock',
  interaction: 'NativeForms',
  server: 'None',
  max_batch: 1,
  login_once_per_batch: true,
  document_leaves_device: true,
  qualification: 'Qes',
  factors: { app: true, sms: true, qr: true, fido2: false, card: false },
  cms_reserve: 16384,
};

const MOBILE_OPTIONS: BackendOptionView[] = [
  { id: 'atrust_native', available: true, capabilities: native, notes: ['ng-fido2-unavailable-mobile'] },
  { id: 'keystore', available: false, capabilities: null, notes: [] },
];

function pointer(type: 'pointerDown' | 'pointerMove' | 'pointerUp', el: Element, init: Record<string, unknown>) {
  fireEvent[type](el, { button: 0, pointerType: 'touch', ...init });
}

describe('Touch-Positionierung', () => {
  it('grabPoint: auf dem Block bleibt der Versatz, außerhalb Maus links oben, Touch mittig über dem Finger', () => {
    const block = { x: 100, y: 500, width: 200, height: 60 };
    // Seite 842 pt hoch: Oberkante des Blocks bei 842 − 500 = 342 pt von oben
    expect(grabPoint(true, 'touch', { x: 150, top: 352 }, block, 842)).toEqual({ grabX: 50, grabTop: 10 });
    expect(grabPoint(false, 'mouse', { x: 10, top: 10 }, block, 842)).toEqual({ grabX: 0, grabTop: 0 });
    expect(grabPoint(false, 'pen', { x: 10, top: 10 }, block, 842)).toEqual({ grabX: 0, grabTop: 0 });
    expect(grabPoint(false, 'touch', { x: 10, top: 10 }, block, 842)).toEqual({ grabX: 100, grabTop: 60 + TOUCH_OFFSET_PT });
  });

  it('Antippen neben dem Block setzt ihn über den Finger (Fingerversatz), Ziehen folgt', async () => {
    const { mock } = renderApp({ view: onMobile(positionView()) });
    await screen.findByRole('application', { name: 'Signatur' });
    const surface = document.querySelector('.page-surface') as HTMLElement;
    surface.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    const [pw, ph] = DOC.page_sizes[2]!;
    const scale = Math.min(560 / pw, 760 / ph);
    // Finger bei (300 pt, 600 pt von oben)
    pointer('pointerDown', surface, { pointerId: 7, clientX: 300 * scale, clientY: 600 * scale });
    pointer('pointerUp', surface, { pointerId: 7, clientX: 300 * scale, clientY: 600 * scale });
    await waitFor(() => expect(mock.dispatched.length).toBeGreaterThan(0));
    const move = mock.dispatched[0] as Extract<UiEvent, { type: 'move_block' }>;
    expect(move.type).toBe('move_block');
    // Block mittig (x = 300 − 232/2 = 184), Unterkante 12 pt über dem Finger: Oberkante von oben = 600 − 82 − 12
    expect(move.x).toBeCloseTo(184, 0);
    expect(move.y).toBeCloseTo(ph - (600 - 82 - TOUCH_OFFSET_PT), 0);
  });

  it('ein zweiter Finger (Pinch) verschiebt den Block nicht', async () => {
    const { mock } = renderApp({ view: onMobile(positionView()) });
    const b = await screen.findByRole('application', { name: 'Signatur' });
    const surface = document.querySelector('.page-surface') as HTMLElement;
    surface.getBoundingClientRect = () => ({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    pointer('pointerDown', b, { pointerId: 1, clientX: 300, clientY: 250 });
    pointer('pointerDown', surface, { pointerId: 2, clientX: 10, clientY: 10 });
    pointer('pointerMove', surface, { pointerId: 2, clientX: 20, clientY: 20 });
    pointer('pointerUp', surface, { pointerId: 2, clientX: 20, clientY: 20 });
    pointer('pointerUp', surface, { pointerId: 1, clientX: 300, clientY: 250 });
    await waitFor(() => expect(mock.dispatched.length).toBeGreaterThan(0));
    // nur der erste Finger zählt: der Block bleibt unter ihm (Versatz), keine Bewegung zur zweiten Stelle
    for (const e of mock.dispatched) {
      const m = e as Extract<UiEvent, { type: 'move_block' }>;
      expect(m.x).toBeGreaterThan(200);
    }
  });
});

describe('Signatur fortsetzen', () => {
  const offer = { name: 'Vertrag.pdf', backend: 'atrust_native' as const, age_minutes: 3, pending: 1 };

  it('Dialog im Schritt Öffnen: Fortsetzen bzw. Verwerfen', async () => {
    const { mock, user } = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: offer })) });
    const dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(dialog).toHaveTextContent('Die Signatur von „Vertrag.pdf“ wurde vor 3 Minuten unterbrochen');
    expect(dialog).toHaveTextContent('Danach folgt ein weiteres Dokument.');
    await user.click(within(dialog).getByRole('button', { name: 'Fortsetzen' }));
    expect(mock.dispatched).toEqual([{ type: 'resume_signature' }]);
  });

  it('Verwerfen nur ausdrücklich (Escape verwirft nicht)', async () => {
    const { mock, user } = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: offer })) });
    const dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    await user.keyboard('{Escape}');
    expect(mock.dispatched).toEqual([]);
    await user.click(within(dialog).getByRole('button', { name: 'Verwerfen' }));
    expect(mock.dispatched).toEqual([{ type: 'discard_resume' }]);
  });

  it('Mock-Kern: Angebot verschwindet nach dem Fortsetzen', async () => {
    const { user } = renderApp({ platform: 'ios', resumeOffer: offer });
    const dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Fortsetzen' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog', { name: 'Signatur fortsetzen?' })).not.toBeInTheDocument());
  });
});

describe('PKCS#12-Import-Assistent', () => {
  it('aus der Signiermittel-Auswahl: Datei wählen, Passwort, Ergebnis', async () => {
    const view = onMobile(backendView(MOBILE_OPTIONS));
    const { mock, user } = renderApp({ view, platform: 'ios' });
    expect(await screen.findByText('Noch kein Keystore auf diesem Gerät.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'PKCS#12-Datei importieren …' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    const start = within(dialog).getByRole('button', { name: 'Importieren' });
    expect(start).toBeDisabled();
    await user.click(within(dialog).getByRole('button', { name: 'Datei wählen …' }));
    expect(await within(dialog).findByTestId('keystore-import-file')).toHaveTextContent('Gewählte Datei: test.p12');
    expect(mock.nativeCalls).toContain('pickMobileFiles:pkcs12:single');
    await user.type(within(dialog).getByLabelText('Passwort des Keystores'), 'falsch');
    await user.click(within(dialog).getByRole('button', { name: 'Importieren' }));
    const sent = mock.dispatched.at(-1) as Extract<UiEvent, { type: 'import_keystore' }>;
    expect(sent).toMatchObject({ type: 'import_keystore', password: 'falsch', remember: true });
    expect(sent.path).toMatch(/import\/1\/test\.p12$/);
    // Antwort des Kerns: falsches Passwort
    act(() => mock.setView({ ...view, keystore_import: { file_name: 'test.p12', ok: false, code: 'keystore-wrong-password', password_stored: false } }));
    expect(await within(dialog).findByRole('alert')).toHaveTextContent(/Passwort/);
    // Antwort des Kerns: importiert, Passwort im sicheren Speicher
    act(() => mock.setView({ ...view, keystore_import: { file_name: 'test.p12', ok: true, code: 'ng-keystore-imported', password_stored: true } }));
    const done = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    expect(done).toHaveTextContent('Der Keystore „test.p12“ wurde importiert.');
    expect(done).toHaveTextContent('Das Passwort ist im sicheren Speicher des Geräts abgelegt.');
    await user.click(within(done).getByRole('button', { name: 'Fertig' }));
    expect(mock.dispatched.at(-1)).toEqual({ type: 'dismiss_keystore_import' });
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
  });

  it('Mock-Kern: Import setzt den Keystore in den Einstellungen (Arbeitskopie bleibt sonst erhalten)', async () => {
    const { mock, user } = renderApp({ platform: 'android', settings: { ...defaultSettings(), keystore: { ...defaultSettings().keystore, enabled: true } } });
    await screen.findByRole('heading', { level: 1 });
    await act(async () => {
      await mock.dispatch({ type: 'open_settings' });
    });
    const dialog = await screen.findByRole('dialog');
    // ungespeicherte Eingabe in einem anderen Tab
    await user.click(within(dialog).getByRole('tab', { name: /Erweitert/ }));
    await user.click(within(dialog).getByRole('checkbox', { name: /Automatische Positionierung/ }));
    await user.click(within(dialog).getByRole('tab', { name: 'Keystore' }));
    expect(within(dialog).queryByRole('textbox', { name: /Keystore-Datei/ })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'PKCS#12-Datei importieren …' }));
    const wizard = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    await user.click(within(wizard).getByRole('button', { name: 'Datei wählen …' }));
    await user.type(within(wizard).getByLabelText('Passwort des Keystores'), 'test');
    await user.click(within(wizard).getByRole('button', { name: 'Importieren' }));
    await user.click(await within(await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' })).findByRole('button', { name: 'Fertig' }));
    expect(await within(dialog).findByText('Importierte Datei: test.p12')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const saved = mock.dispatched.find((e) => e.type === 'save_settings') as Extract<UiEvent, { type: 'save_settings' }>;
    expect(saved.settings.keystore.file).toMatch(/keystores\/test\.p12$/);
    expect(saved.settings.signature.auto_position).toBe(true);
  });

  it('keystoreOnlyChanged erkennt reine Keystore-Änderungen', () => {
    const a = defaultSettings();
    const b = { ...a, keystore: { ...a.keystore, file: '/x/k.p12', enabled: true } };
    expect(keystoreOnlyChanged(a, b)).toBe(true);
    expect(keystoreOnlyChanged(a, { ...b, signature: { ...b.signature, auto_position: true } })).toBe(false);
    expect(keystoreOnlyChanged(a, a)).toBe(false);
  });
});

describe('Mobile-Bedienelemente', () => {
  it('Ergebnis: Teilen und Sichern unter statt „Im Ordner anzeigen“', async () => {
    const path = '/var/mobile/Containers/Data/Application/X/Documents/Vertrag_signed.pdf';
    const { mock, user } = renderApp({ view: onMobile(viewOf({ step: 'output', result: outputResult({ path }), next: null })) });
    expect(await screen.findByText('Das signierte Dokument liegt in den Dokumenten dieser App.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Im Ordner anzeigen' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Teilen' }));
    await user.click(screen.getByRole('button', { name: 'Sichern unter …' }));
    expect(mock.dispatched).toEqual([{ type: 'share_result' }, { type: 'export_result' }]);
  });

  it('Überschreiben-Rückfrage nennt nur den Dateinamen, nicht den Sandbox-Pfad', async () => {
    const existing = '/var/mobile/Containers/Data/Application/X/Documents/Vertrag_signed.pdf';
    renderApp({ view: onMobile(viewOf({ step: 'output', result: outputResult({ ask_overwrite: existing }), next: null })) });
    const dialog = await screen.findByRole('alertdialog');
    expect(dialog).toHaveTextContent('Datei Vertrag_signed.pdf existiert bereits');
    expect(dialog).not.toHaveTextContent('/var/mobile');
  });

  it('Mobile: Zugriffstasten-Unterstreichung ist per CSS abgeschaltet (data-mobile an der Wurzel)', async () => {
    renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] })) });
    const button = await screen.findByRole('button', { name: 'Dokument wählen …' });
    // Die Markierung bleibt im DOM (Tastatur am Tablet), die Stilregel `.app[data-mobile] .mnemonic` blendet sie aus
    expect(button.closest('.app[data-mobile]')).not.toBeNull();
    // Dialoge liegen als Portal außerhalb von `.app`: das Attribut steht deshalb auch an <html>
    expect(document.documentElement.hasAttribute('data-mobile')).toBe(true);
  });

  it('Signiermittel: FIDO2 ausgegraut mit Hinweis, Beschreibung ohne FIDO2', async () => {
    renderApp({ view: onMobile(backendView(MOBILE_OPTIONS), ANDROID) });
    const card = await screen.findByRole('button', { name: /ID Austria|Handy/ });
    expect(card).toHaveTextContent('Bestätigung in der App „ID Austria“ oder per SMS-TAN.');
    expect(card).toHaveTextContent('FIDO2 nicht verfügbar');
    expect(card).toHaveTextContent('FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar.');
    expect(screen.queryByText(/Bürgerkarte|lokal installierte/)).not.toBeInTheDocument();
  });

  it('Warten auf die App: FIDO2-Knopf gesperrt mit Hinweis, wenn A-Trust FIDO2 anbietet', async () => {
    const form = { WaitForApp: { reference_value: 'AB12', timeout_secs: 300, ctx: ctx({ fido2_visible: false, can_switch_to_fido2: true }) } };
    renderApp({ view: onMobile(formView(form)) });
    const fido = await screen.findByRole('button', { name: 'FIDO2' });
    expect(fido).toBeDisabled();
    expect(fido).toHaveAccessibleDescription('FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar. Bitte bestätigen Sie in der App „ID Austria“ oder per SMS-TAN.');
  });

  it('Desktop ohne FIDO2-Provider: kein ausgegrauter Knopf', async () => {
    const form = { SmsTan: { reference_value: 'ab', ctx: ctx({ fido2_visible: false, can_switch_to_fido2: true }) } };
    renderApp({ view: formView(form) });
    await screen.findByLabelText('TAN:');
    expect(screen.queryByRole('button', { name: 'FIDO2' })).not.toBeInTheDocument();
  });

  it('Durchsuchen nutzt die Dateiauswahl des Plugins (Kopien in der Sandbox)', async () => {
    const { mock, user } = renderApp({ platform: 'ios' });
    // Mobile-Text statt „Durchsuchen um ein Dokument zu wählen“ („… on your computer“ im Englischen)
    await user.click(await screen.findByRole('button', { name: 'Dokument wählen …' }));
    await waitFor(() => expect(mock.dispatched[0]).toMatchObject({ type: 'open_files' }));
    expect(mock.nativeCalls).toEqual(['pickMobileFiles:pdf:multiple']);
    const ev = mock.dispatched[0] as Extract<UiEvent, { type: 'open_files' }>;
    expect(ev.paths.every((p) => p.includes('/Library/Caches/import/'))).toBe(true);
  });

  it('Einstellungen: ohne Ausgabeordner, Proxy, Update-Prüfung und Log-Ordner; FIDO2-Hinweis', async () => {
    const view: ViewState = onMobile(settingsView(defaultSettings(), { fido2_available: false }), IOS);
    const { user } = renderApp({ view });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: /Erweitert/ }));
    expect(within(dialog).queryByRole('textbox', { name: /Ausgabeordner|Ordner/ })).not.toBeInTheDocument();
    expect(within(dialog).queryByText('Proxy')).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('radio', { name: /Proxy/ })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('tab', { name: 'FIDO2' }));
    expect(within(dialog).getByText(/FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('tab', { name: /Über/ }));
    expect(within(dialog).queryByRole('checkbox', { name: /Updates/ })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: /Log/ })).not.toBeInTheDocument();
  });

  it('App-Wurzel kennzeichnet die Plattform (CSS: Safe Areas, Touch)', async () => {
    renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }), ANDROID) });
    await screen.findByRole('heading', { level: 1 });
    const app = document.querySelector('.app') as HTMLElement;
    expect(app.dataset.mobile).toBe('true');
    expect(app.dataset.os).toBe('android');
  });
});
