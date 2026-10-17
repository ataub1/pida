/**
 * Mobile-Oberfläche (ADR 0004 §4.6, Plan §5.2, docs/integration/mobile.md): Bottom-Sheet, Touch-Ziehen mit
 * Fingerversatz, „Signatur fortsetzen“ (mit Kurzhinweis), nur ID Austria (ADR 0004 Nachtrag 11: nirgends
 * Keystore-Texte; Nachtrag 20: keine Signiermittel-Auswahl, kein Konto-Chip), Teilen/Sichern unter, FIDO2
 * ausgegraut, Dateiauswahl über das Plugin,
 * Plattform-Kennzeichen; dieselben Ansichten mit `platform: ios|android` wie `?platform=` (Abnahme E1 Punkt 9 [M]).
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { UiEvent } from '../core/types';
import { expectNoAxeViolations } from '../test/a11y';
import { emulateMedia, lastEvent, renderApp } from '../test/render';
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
    expect(dialog).toHaveTextContent('Anmeldung und Bestätigung sind erneut nötig.');
    expect(dialog).toHaveTextContent('Danach folgt ein weiteres Dokument.');
    await user.keyboard('{Escape}');
    expect(mock.dispatched).toEqual([]);
    await user.click(within(dialog).getByRole('button', { name: 'Fortsetzen' }));
    expect(mock.dispatched).toEqual([{ type: 'resume_signature' }]);
    await expectNoAxeViolations();
  });

  // Ein Angebot mit einem Signiermittel auf dem Gerät (Keystore) kommt seit ADR 0004 Nachtrag 11 nicht mehr auf
  // Mobilgeräten vor (der Kern verwirft einen solchen Vorgang beim Start); der Dialog behandelt es weiter.
  it('Kurzhinweis zum Datenschutz bei ID Austria, nicht bei einem Signiermittel auf dem Gerät', async () => {
    const { unmount } = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: offer })) });
    let dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(within(dialog).getByTestId('privacy-note')).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt.');
    unmount();
    renderApp({ view: viewOf({ step: 'open', queue: [] }, { resume: { ...offer, backend: 'keystore', document_leaves_device: false } }) });
    dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(within(dialog).queryByTestId('privacy-note')).not.toBeInTheDocument();
  });

  it('eben unterbrochen: „gerade eben“; auf dem Gerät ohne Anmeldung/Bestätigung (Emulator 16.10.2026)', async () => {
    const keystore = { ...offer, backend: 'keystore' as const, age_minutes: 0, pending: 0, document_leaves_device: false };
    const { unmount } = renderApp({ view: viewOf({ step: 'open', queue: [] }, { resume: keystore }) });
    let dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(dialog).toHaveTextContent('Die Signatur von „Vertrag.pdf“ wurde gerade eben unterbrochen');
    expect(dialog).not.toHaveTextContent('vor 0 Minuten');
    expect(dialog).not.toHaveTextContent('Anmeldung');
    unmount();
    renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }, { resume: { ...offer, age_minutes: 1 } }), ANDROID) });
    dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    expect(dialog).toHaveTextContent('wurde vor einer Minute unterbrochen');
  });

  it('Mock-Kern: Fortsetzen öffnet die Signierkarte über dem Dokument (ausdrückliche Aktion)', async () => {
    const { user } = renderApp({ platform: 'ios', resumeOffer: offer });
    const dialog = await screen.findByRole('alertdialog', { name: 'Signatur fortsetzen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Fortsetzen' }));
    expect(await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' })).toBeInTheDocument();
    expect(document.querySelector('.workspace')).not.toBeNull();
  });
});

describe('Ansichtswechsel', () => {
  it('der Arbeitsbereich beginnt oben, auch wenn die Startseite gerollt war (Querformat)', async () => {
    const { mock } = renderApp({ view: onMobile(viewOf({ step: 'open', queue: [] }), IOS) });
    await screen.findByRole('heading', { level: 1 });
    const main = document.getElementById('main') as HTMLElement;
    main.scrollTop = 180;
    act(() => mock.setView(onMobile(workspaceView(), IOS)));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(main.scrollTop).toBe(0);
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

  it('schmal mit ID Austria: Kurzhinweis „an A-Trust übermittelt“ über „Signieren“, auch eingeklappt', async () => {
    restore = emulateMedia({ narrow: true });
    const { unmount } = renderApp({ view: onMobile(workspaceView(documentView(), { signers: signersView({}, true) }), ANDROID) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const sheet = screen.getByRole('complementary', { name: 'Signatur' });
    expect(within(sheet).getByRole('button', { name: /Optionen einblenden/ })).toHaveAttribute('aria-expanded', 'false');
    expect(within(sheet).getByTestId('privacy-note')).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt.');
    await expectNoAxeViolations();
    unmount();
    // schmal am Desktop mit Keystore: kein Kurzhinweis (auf Mobilgeräten gibt es keinen Keystore)
    renderApp({ view: workspaceView(documentView(), { signers: signersView({ selected: 'keystore' }) }) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('privacy-note')).not.toBeInTheDocument();
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

describe('Signiermittel auf Mobilgeräten: nur ID Austria, keine Auswahl (ADR 0004 Nachtrag 20)', () => {
  it.each([
    ['ios', IOS],
    ['android', ANDROID],
  ] as const)('%s schmal: kein Konto-Chip, „Signieren“ über die volle Breite, Kurzhinweis bleibt; axe', async (platform, os) => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const view = onMobile(workspaceView(documentView(), { signers: signersView({}, true) }), os);
    const { mock, user } = renderApp({ view, platform });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const sheet = screen.getByRole('complementary', { name: 'Signatur' });
    expect(screen.queryByTestId('signer-chip')).not.toBeInTheDocument();
    expect(sheet.querySelector('.inspector-footer')).toHaveClass('is-single');
    expect(sheet.querySelector('.inspector-footer-chip')).toBeNull();
    expect(within(sheet).getByTestId('privacy-note')).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt.');
    expect(sheet.textContent).not.toMatch(/Signiermittel|Signieren mit/);
    await expectNoAxeViolations();
    // ausgeklappt ebenso kein Abschnitt „Signieren mit“
    await user.click(within(sheet).getByRole('button', { name: /Optionen einblenden/ }));
    expect(within(sheet).queryByRole('heading', { name: 'Signieren mit' })).not.toBeInTheDocument();
    expect(sheet.textContent).not.toMatch(/Signiermittel/);
    await expectNoAxeViolations();
    // „Signieren“ signiert mit dem vom Kern gewählten ID Austria – kein Menü
    await user.click(within(sheet).getByRole('button', { name: /^Signieren/ }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_with', backend: 'atrust_native' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('breit (Tablet): kein Abschnitt „Signieren mit“, Kurzhinweis über „Signieren“', async () => {
    renderApp({ view: onMobile(workspaceView(documentView(), { signers: signersView({}, true) }), IOS), platform: 'ios' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const inspector = screen.getByRole('complementary', { name: 'Signatur' });
    expect(within(inspector).queryByRole('heading', { name: 'Signieren mit' })).not.toBeInTheDocument();
    expect(screen.queryByTestId('signer-chip')).not.toBeInTheDocument();
    const footer = inspector.querySelector('.inspector-footer') as HTMLElement;
    expect(within(footer).getByTestId('privacy-note')).toBeInTheDocument();
    expect(within(footer).getByRole('button', { name: /^Signieren/ })).toBeEnabled();
    await expectNoAxeViolations();
  });

  it('kein leeres Menü: ohne Signiermittel (Richtlinie schließt ID Austria aus) bleibt „Signieren“ gesperrt', async () => {
    const base = signersView({}, true);
    const signers = { ...base, options: base.options.map((o) => ({ ...o, available: false, capabilities: null, unavailable_reason: 'ng-signer-policy' })), selected: null, default: null };
    const { mock, user } = renderApp({ view: onMobile(workspaceView(documentView(), { signers }), ANDROID) });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const sign = screen.getByRole('button', { name: /^Signieren/ });
    expect(sign).toBeDisabled();
    await user.click(sign);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(mock.dispatched).toEqual([]);
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
    // ID Austria ist das einzige Signiermittel und schon gewählt (kein Konto-Chip); „Signieren“ bleibt die
    // ausdrückliche Aktion
    expect(screen.queryByTestId('signer-chip')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    expect(await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});

describe('Hinweis zum Teilen auf iOS (ADR 0004 Nachtrag 12)', () => {
  const TEXT = 'Mehrere PDFs geteilt? iOS übergibt beim Teilen nur das erste. Für einen Stapel wählen Sie die übrigen über „Weitere PDFs hinzufügen …“ – dort ist Mehrfachauswahl möglich.';

  it('schmal über „Signieren“, auch im eingeklappten Sheet: „Weitere PDFs hinzufügen …“ (Mehrfachauswahl) und „Hinweis ausblenden“; axe', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const doc = documentView();
    const { mock, user } = renderApp({ view: onMobile(workspaceView(doc, { share_hint: { doc: doc.doc.id } }), IOS), platform: 'ios' });
    const hint = await screen.findByTestId('share-hint');
    expect(hint).toHaveAttribute('role', 'note');
    expect(hint).toHaveTextContent(TEXT);
    // nicht im (eingeklappten) Inhalt des Sheets, sondern daneben – sichtbar ohne Aufklappen
    expect(hint.closest('.inspector-scroll')).toBeNull();
    expect(hint.closest('.inspector')).not.toBeNull();
    await expectNoAxeViolations();
    await user.click(within(hint).getByRole('button', { name: 'Weitere PDFs hinzufügen …' }));
    await waitFor(() => expect(lastEvent(mock)).toMatchObject({ type: 'open_files', mode: 'add' }));
    // Dokumentauswahl mit Mehrfachauswahl (seit Nachtrag 17 mit der Zahl der nicht übernommenen Dateien)
    expect(mock.nativeCalls).toContain('pickMobileDocuments:multiple');
    await user.click(within(hint).getByRole('button', { name: 'Hinweis ausblenden' }));
    expect(lastEvent(mock)).toEqual({ type: 'dismiss_share_hint' });
  });

  it('ohne `share_hint` kein Hinweis (Dateiauswahl, Android, Desktop)', async () => {
    renderApp({ view: onMobile(workspaceView(), IOS), platform: 'ios' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('share-hint')).not.toBeInTheDocument();
  });

  it('Mock-Kern wie der Kern: nur nach dem Empfang über „Teilen“, weg mit weiteren PDFs bzw. nach „Ausblenden“ (bis zum Beenden)', async () => {
    const { mock, user } = renderApp({ platform: 'ios' });
    await screen.findByRole('heading', { level: 1 });
    // Dateiauswahl: kein Hinweis
    await act(() => mock.dispatch({ type: 'open_files', paths: ['/x/Library/Caches/import/Vertrag.pdf'] }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('share-hint')).not.toBeInTheDocument();
    await act(() => mock.dispatch({ type: 'cancel' }));
    await waitFor(() => expect(screen.queryByRole('application', { name: 'Signaturblock' })).not.toBeInTheDocument());
    // über „Teilen“ empfangen (Shell): Hinweis
    await act(() => mock.dispatch({ type: 'files_received', paths: ['/x/Library/Caches/inbox/Angebot.pdf'] }));
    const hint = await screen.findByTestId('share-hint');
    expect(hint).toHaveTextContent(TEXT);
    await user.click(within(hint).getByRole('button', { name: 'Hinweis ausblenden' }));
    await waitFor(() => expect(screen.queryByTestId('share-hint')).not.toBeInTheDocument());
    // erneut empfangen: ausgeblendet bleibt ausgeblendet
    await act(() => mock.dispatch({ type: 'cancel' }));
    await act(() => mock.dispatch({ type: 'files_received', paths: ['/x/Library/Caches/inbox/Auftrag.pdf'] }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('share-hint')).not.toBeInTheDocument();
  });

  it('Mock-Kern: mit weiteren PDFs ist der Hinweis erledigt; Android und Desktop zeigen ihn nie', async () => {
    const { mock, unmount } = renderApp({ platform: 'ios' });
    await screen.findByRole('heading', { level: 1 });
    await act(() => mock.dispatch({ type: 'files_received', paths: ['/x/Library/Caches/inbox/Vertrag.pdf'] }));
    await screen.findByTestId('share-hint');
    await act(() => mock.dispatch({ type: 'open_files', paths: ['/x/Library/Caches/import/Angebot.pdf'], mode: 'add' }));
    await waitFor(() => expect(screen.queryByTestId('share-hint')).not.toBeInTheDocument());
    unmount();
    for (const platform of ['android', 'desktop'] as const) {
      const r = renderApp({ platform });
      await screen.findByRole('heading', { level: 1 });
      await act(() => r.mock.dispatch({ type: 'files_received', paths: ['/x/inbox/Vertrag.pdf'] }));
      await screen.findByRole('application', { name: 'Signaturblock' });
      expect(screen.queryByTestId('share-hint')).not.toBeInTheDocument();
      r.unmount();
    }
  });
});
