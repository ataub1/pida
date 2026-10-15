/**
 * Signierkarte (ADR 0004 §4.4, Abnahme E1 Punkt 6): jeder Zweig aus `InteractionView` bzw.
 * `SigningProgressView` – Anmeldung, App-Bestätigung (Vergleichswert, Countdown, Vorwarnung,
 * Alternativen), QR, SMS-TAN (Vergleichswert abgewiesen), FIDO2 inkl. PIN, Keystore-Passwort, Chipkarte,
 * A-Trust-Meldung, Rückfall-Auswahl, Fehler mit Details, „abgelehnt“, Abbruch – je mit axe; die Karte liegt
 * über dem eingehängten Arbeitsbereich.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { capabilitiesFor } from '../../core/mock/shared';
import type { FormContext, InteractionView, NativeForm } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { lastEvent, renderApp } from '../../test/render';
import { cardErrorView, cardView, ctx, documentView, IOS, onMobile, outputResult, signersView, viewOf } from '../../test/views';

function form(f: NativeForm, request = 1): InteractionView {
  return { kind: 'form', request, form: f };
}

const LOGIN = form({ Login: { prefill_username: '+43 660 1234123', remember: false, ctx: ctx({ can_switch_to_sms: false, can_switch_to_fido2: false, fido2_visible: false }) } });
const APP = (c: Partial<FormContext> = {}) => form({ WaitForApp: { reference_value: 'Xk7Pq', timeout_secs: 300, ctx: ctx({ signature_data_url: 'https://www.a-trust.at/x', ...c }) } }, 2);

async function card() {
  return screen.findByRole('dialog', { name: /Signieren mit/ });
}

describe('Signierkarte: Schritte und Stepper', () => {
  it('Anmeldung: Benutzer vorbelegt, Fokus im Passwortfeld, Vorabprüfung, „Daten merken“, Kurzhinweis A-Trust', async () => {
    const { mock, user } = renderApp({ view: cardView(LOGIN, { stage: 'login' }) });
    const dialog = await card();
    expect(dialog).toHaveAccessibleName('Signieren mit ID Austria');
    expect(within(dialog).getByText('Vertrag.pdf')).toBeInTheDocument();
    // Stepper: Anmelden aktuell
    const steps = within(dialog).getByRole('list', { name: 'Fortschritt' });
    expect(within(steps).getAllByRole('listitem')[0]).toHaveAttribute('aria-current', 'step');
    const pw = within(dialog).getByLabelText('Signatur-Passwort');
    await waitFor(() => expect(pw).toHaveFocus());
    expect(within(dialog).getByLabelText('Telefonnummer oder Benutzername')).toHaveValue('+43 660 1234123');
    await user.type(pw, 'kurz');
    await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));
    expect(within(dialog).getByText('Eingegebenes Passwort ist zu kurz!')).toBeInTheDocument();
    expect(mock.dispatched).toEqual([]);
    await user.type(pw, '-lang');
    await user.click(within(dialog).getByRole('checkbox', { name: /Daten merken/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));
    expect(mock.dispatched).toEqual([{ type: 'submit_form', request: 1, answer: { kind: 'login', username: '+43 660 1234123', password: 'kurz-lang', remember: true } }]);
    expect(within(dialog).getByTestId('privacy-note')).toHaveTextContent('an A-Trust übermittelt');
    await expectNoAxeViolations();
  });

  it('abgelehnte Anmeldung (FormProblem des Kerns) als Meldung', async () => {
    const rejected = form({ Login: { prefill_username: 'max', remember: false, ctx: ctx({ problem: 'LoginRejected' }) } }, 3);
    renderApp({ view: cardView(rejected) });
    const dialog = await card();
    expect(within(dialog).getAllByRole('alert').some((a) => a.textContent?.includes('Anmeldung'))).toBe(true);
  });

  it('App-Bestätigung: Vergleichswert in Einzelzeichen, Countdown-Ring, Signaturdaten, Alternativen, Stepper', async () => {
    const { mock, user } = renderApp({ view: cardView(APP(), { stage: 'confirming', method: 'app', login_done: true }) });
    const dialog = await card();
    expect(within(dialog).getByRole('heading', { name: 'Benachrichtigung an die ID-Austria-App gesendet' })).toBeInTheDocument();
    expect(within(dialog).getByText('X k 7 P q')).toHaveClass('sr-only');
    expect(within(dialog).getByRole('timer')).toHaveAccessibleName('Verbleibende Zeit: 5:00');
    const steps = within(dialog).getAllByRole('listitem');
    expect(steps[0]).toHaveClass('is-done');
    expect(steps[1]).toHaveAttribute('aria-current', 'step');
    expect(steps[1]).toHaveTextContent('In der App bestätigen');
    await user.click(within(dialog).getByRole('button', { name: 'Signaturdaten anzeigen' }));
    expect(lastEvent(mock)).toEqual({ type: 'open_signature_data', request: 2 });
    await user.click(within(dialog).getByRole('button', { name: 'SMS-TAN' }));
    expect(lastEvent(mock)).toEqual({ type: 'submit_form', request: 2, answer: { kind: 'switch_to_sms' } });
    await expectNoAxeViolations();
  });

  it('App-Bestätigung: einmalige Vorwarnung eine Minute vor Ablauf', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderApp({ view: cardView(APP(), { stage: 'confirming', method: 'app', login_done: true }) });
      const dialog = await card();
      expect(within(dialog).queryByText('Noch eine Minute für die Bestätigung in der App.')).not.toBeInTheDocument();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(241_000);
      });
      expect(within(dialog).getByText('Noch eine Minute für die Bestätigung in der App.')).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it('Mobile: FIDO2 ausgegraut mit Hinweis, Text „in der App bestätigen, dann zurückkehren“', async () => {
    renderApp({ view: onMobile(cardView(APP({ fido2_visible: false, can_switch_to_fido2: true }), { stage: 'confirming', method: 'app', login_done: true })) });
    const dialog = await card();
    const fido = within(dialog).getByRole('button', { name: 'FIDO2-Schlüssel' });
    expect(fido).toBeDisabled();
    expect(fido).toHaveAccessibleDescription(/FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar/);
    expect(dialog).toHaveTextContent('Bestätigen Sie in der App „ID Austria“ und kehren Sie dann zu PIDA zurück.');
  });

  it('QR-Code: Bild, Vergleichswert, Weiter', async () => {
    const bytes = Array.from(new TextEncoder().encode('<svg xmlns="http://www.w3.org/2000/svg"/>'));
    const qr = form({ QrCode: { reference_value: 'Ab12', image: { bytes, mime: 'image/svg+xml' }, timeout_secs: 300, ctx: ctx() } }, 4);
    const { mock, user } = renderApp({ view: cardView(qr, { stage: 'confirming', method: 'qr', login_done: true }) });
    const dialog = await card();
    expect(within(dialog).getByRole('img', { name: /QR-Code/ }).getAttribute('src')).toMatch(/^data:image\/svg\+xml;base64,/);
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));
    expect(lastEvent(mock)).toEqual({ type: 'submit_form', request: 4, answer: { kind: 'continue' } });
    await expectNoAxeViolations();
  });

  it('SMS-TAN: Vergleichswert wird als TAN abgewiesen, TAN geht an den Kern', async () => {
    const sms = form({ SmsTan: { reference_value: 'Xk7Pq', ctx: ctx() } }, 5);
    const { mock, user } = renderApp({ view: cardView(sms, { stage: 'confirming', method: 'sms', login_done: true }) });
    const dialog = await card();
    expect(within(dialog).getAllByRole('listitem')[1]).toHaveTextContent('SMS-TAN eingeben');
    const tan = within(dialog).getByLabelText('TAN aus der SMS');
    expect(tan).toHaveAttribute('autocomplete', 'one-time-code');
    await user.type(tan, 'Xk7Pq');
    await user.click(within(dialog).getByRole('button', { name: 'Bestätigen' }));
    expect(within(dialog).getAllByText('Das ist der Vergleichswert – bitte geben Sie die TAN aus der SMS ein.').length).toBeGreaterThan(0);
    expect(mock.dispatched).toEqual([]);
    await user.clear(tan);
    await user.type(tan, '482913');
    await user.click(within(dialog).getByRole('button', { name: 'Bestätigen' }));
    expect(lastEvent(mock)).toEqual({ type: 'submit_form', request: 5, answer: { kind: 'tan', tan: '482913' } });
    await expectNoAxeViolations();
  });

  it('FIDO2: Start, „Schlüssel berühren“ mit Statusmeldung; PIN als Passwortabfrage', async () => {
    const fido = form({ Fido2: { in_progress: false, ctx: ctx({ can_switch_to_fido2: false }) } }, 6);
    const { mock, user, unmount } = renderApp({ view: cardView(fido, { stage: 'confirming', method: 'fido2', login_done: true }) });
    let dialog = await card();
    expect(within(dialog).getAllByRole('listitem')[1]).toHaveTextContent('Mit FIDO2 bestätigen');
    await user.click(within(dialog).getByRole('button', { name: 'Mit Schlüssel bestätigen' }));
    expect(lastEvent(mock)).toEqual({ type: 'submit_form', request: 6, answer: { kind: 'start_fido2' } });
    unmount();
    renderApp({ view: cardView(form({ Fido2: { in_progress: true, ctx: ctx() } }, 7), { stage: 'confirming', method: 'fido2', login_done: true }) });
    dialog = await card();
    expect(within(dialog).getByRole('status')).toHaveTextContent('Berühren Sie jetzt Ihren Sicherheitsschlüssel');
    await expectNoAxeViolations();
  });

  it('FIDO2-PIN (SecretKind::Fido2Pin) über submit_secret', async () => {
    const pin: InteractionView = { kind: 'secret', request: 8, prompt: { kind: 'Fido2Pin', subject: '', retry: false } };
    const { mock, user } = renderApp({ view: cardView(pin, { stage: 'confirming', method: 'fido2', login_done: true }) });
    const dialog = await card();
    const field = within(dialog).getByLabelText('PIN des Sicherheitsschlüssels');
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, '1234{Enter}');
    expect(mock.dispatched).toEqual([{ type: 'submit_secret', request: 8, secret: '1234' }]);
  });

  it('Keystore: Passwort mit Hinweis bei falschem Versuch; Stepper „Entsperren → Signiert“; kein Kurzhinweis A-Trust', async () => {
    const ks: InteractionView = { kind: 'secret', request: 9, prompt: { kind: 'KeystorePassword', subject: 'max.p12', retry: true } };
    const { mock, user } = renderApp({ view: cardView(ks, { backend: 'keystore', stage: 'confirming', method: 'password', certificate: 'not_needed' }) });
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    expect(within(dialog).getAllByRole('listitem').map((l) => l.textContent?.replace(/,.*/, ''))).toEqual(['1Entsperren', '2Signiert']);
    expect(within(dialog).getByText('Geben Sie das Passwort für „max.p12“ ein.')).toBeInTheDocument();
    expect(within(dialog).getAllByRole('alert').some((a) => a.textContent?.includes('Keystore-Passwort'))).toBe(true);
    await user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'geheim{Enter}');
    expect(mock.dispatched).toEqual([{ type: 'submit_secret', request: 9, secret: 'geheim' }]);
    expect(within(dialog).queryByTestId('privacy-note')).not.toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Chipkarte: „PIN in der Chipkarten-Software eingeben“', async () => {
    renderApp({ view: cardView({ kind: 'waiting', progress: 'Signing' }, { backend: 'local_bku', stage: 'confirming', method: 'card', certificate: 'not_needed' }) });
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Chipkarte' });
    expect(dialog).toHaveTextContent('Stecken Sie Ihre Chipkarte in das Lesegerät');
    expect(within(dialog).getByRole('status')).toHaveTextContent('Warten auf die Chipkarten-Software');
    await expectNoAxeViolations();
  });

  it('A-Trust-Meldung (Fehlerseite nach Ablauf): Text roh, „Neue Bestätigung anfordern“ = Retry', async () => {
    const notice: InteractionView = { kind: 'notice', request: 10, notice: { kind: 'Error', code: 'AtrustError', text: 'Zeit abgelaufen', buttons: 'RetryCancel' } };
    const { mock, user } = renderApp({ view: cardView(notice, { stage: 'confirming', method: 'app', login_done: true }) });
    const dialog = await card();
    expect(within(dialog).getByRole('alert')).toHaveTextContent('Zeit abgelaufen');
    await user.click(within(dialog).getByRole('button', { name: 'Neue Bestätigung anfordern' }));
    expect(lastEvent(mock)).toEqual({ type: 'answer_notice', request: 10, answer: 'Retry' });
    await expectNoAxeViolations();
  });

  it('erneute Login-Maske nach zuvor erfolgreicher Anmeldung: Stepper springt auf „Anmelden“ zurück', async () => {
    const { mock } = renderApp({ view: cardView(APP(), { stage: 'confirming', method: 'app', login_done: true }) });
    let dialog = await card();
    expect(within(dialog).getAllByRole('listitem')[0]).toHaveClass('is-done');
    act(() => mock.setView(cardView(LOGIN, { stage: 'login', login_done: false })));
    dialog = await card();
    expect(within(dialog).getAllByRole('listitem')[0]).toHaveAttribute('aria-current', 'step');
    expect(within(dialog).getAllByRole('listitem')[0]).not.toHaveClass('is-done');
  });

  it('Kern ohne Fortschritt (K0): Schritt aus der Maske', async () => {
    const view = cardView(APP(), {});
    const s = view.step;
    if (s.step !== 'signing') throw new Error('Schritt');
    const plain = { ...view, step: { ...s, progress: { ...s.progress, backend: null, stage: 'connecting' as const } } };
    renderApp({ view: plain });
    const dialog = await card();
    expect(within(dialog).getAllByRole('listitem')[1]).toHaveAttribute('aria-current', 'step');
  });

  it('Abbrechen (×, Knopf, Esc) sendet cancel; kein Schließen durch Klick daneben', async () => {
    const { mock, user } = renderApp({ view: cardView(APP(), { stage: 'confirming', method: 'app', login_done: true }) });
    const dialog = await card();
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    await user.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => expect(mock.dispatched.filter((e) => e.type === 'cancel').length).toBeGreaterThanOrEqual(3));
    // die Seitenansicht bleibt eingehängt
    expect(document.querySelector('.workspace')).not.toBeNull();
  });
});

describe('Signierkarte: Fehler, Rückfall, Fertigstellen', () => {
  it('Fehler „A-Trust nicht erreichbar“: Alltagstext, Details (Code, Zeitpunkt), Erneut versuchen (gleiche Sitzung)', async () => {
    const { mock, user } = renderApp({ view: cardErrorView('backend-network', { detail: 'service.a-trust.at:443' }) });
    const dialog = await card();
    expect(within(dialog).getByRole('heading', { name: 'Der Signaturdienst ist gerade nicht erreichbar' })).toBeInTheDocument();
    expect(within(dialog).getByRole('alert')).toHaveTextContent('Die Verbindung zum Signaturdienst ist fehlgeschlagen');
    // Stepper zeigt den Fehler nicht nur über die Farbe
    const failed = within(dialog).getAllByRole('listitem').find((li) => li.classList.contains('is-error'));
    expect(failed).toHaveTextContent('!');
    expect(failed).toHaveTextContent('fehlgeschlagen');
    await user.click(within(dialog).getByRole('button', { name: 'Technische Details' }));
    expect(dialog).toHaveTextContent('backend-network · ID Austria · 15.10.2026');
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Erneut versuchen' }));
    expect(lastEvent(mock)).toEqual({ type: 'retry' });
  });

  it('„In der App abgelehnt“ (ng-signing-declined): Erneut versuchen und Schließen', async () => {
    const { mock, user } = renderApp({ view: cardErrorView('ng-signing-declined') });
    const dialog = await card();
    expect(within(dialog).getByRole('heading', { name: 'In der App abgelehnt oder abgelaufen' })).toBeInTheDocument();
    expect(dialog).toHaveTextContent('es wurde nichts signiert');
    const actions = dialog.querySelector('.step-actions') as HTMLElement;
    await user.click(within(actions).getByRole('button', { name: 'Schließen' }));
    expect(lastEvent(mock)).toEqual({ type: 'cancel' });
    await user.click(within(dialog).getByRole('button', { name: 'Erneut versuchen' }));
    expect(lastEvent(mock)).toEqual({ type: 'retry' });
  });

  it('Rückfall „Signiermittel wählen“ (nach „Signatur fortsetzen“) → choose_backend, Kurzhinweis bei ID Austria', async () => {
    const options = [
      { id: 'atrust_native' as const, available: true, capabilities: capabilitiesFor('atrust_native'), notes: [] },
      { id: 'keystore' as const, available: false, capabilities: null, notes: [] },
    ];
    const view = viewOf({ step: 'backend', options }, { document: documentView({ phase: 'signing' }), signers: signersView() });
    const { mock, user } = renderApp({ view });
    const dialog = await screen.findByRole('dialog', { name: 'Signiermittel wählen' });
    expect(within(dialog).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
    expect(within(dialog).getByTestId('privacy-note')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'choose_backend', backend: 'atrust_native' });
    await expectNoAxeViolations();
  });

  it('Mobile-Rückfall ohne Keystore: Import-Assistent aus der Karte', async () => {
    const options = [{ id: 'atrust_native' as const, available: true, capabilities: capabilitiesFor('atrust_native', true), notes: ['ng-fido2-unavailable-mobile'] }];
    const view = onMobile(viewOf({ step: 'backend', options }, { document: documentView({ phase: 'signing' }), signers: signersView({}, true) }), IOS);
    const { user } = renderApp({ view, platform: 'ios' });
    const dialog = await screen.findByRole('dialog', { name: 'Signiermittel wählen' });
    await user.click(within(dialog).getByRole('button', { name: 'PKCS#12-Datei importieren …' }));
    expect(await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' })).toBeInTheDocument();
  });

  it('Signatur erstellt, Speichern läuft: Häkchen „Signiert“, kein Abbrechen, Seitenansicht bleibt', async () => {
    const view = viewOf({ step: 'output', result: outputResult(), next: null, pending: 0 }, { document: documentView({ phase: 'done' }), signers: signersView() });
    renderApp({ view });
    const dialog = await card();
    expect(within(dialog).getByRole('status')).toHaveTextContent('Signiert');
    expect(within(dialog).queryByRole('button', { name: 'Abbrechen' })).not.toBeInTheDocument();
    expect(within(dialog).getAllByRole('listitem').every((li) => li.classList.contains('is-done'))).toBe(true);
    expect(document.querySelector('.workspace')).not.toBeNull();
  });
});
