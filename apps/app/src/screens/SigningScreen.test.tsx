import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { Notice } from '../core/types';
import { translator } from '../i18n/catalog';
import { renderApp } from '../test/render';
import { ctx, formView, signingView } from '../test/views';
import { noticeTexts, problemText, progressText, qrDataUrl } from './SigningScreen';

describe('Signieren: Warten', () => {
  it('unbestimmter Fortschritt mit Text und Abbrechen', async () => {
    const { mock, user } = renderApp({ view: signingView({ kind: 'waiting', progress: 'Connecting' }) });
    const bar = await screen.findByRole('progressbar', { name: 'Verbindung wird aufgebaut …' });
    expect(bar).not.toHaveAttribute('aria-valuenow');
    expect(screen.getByRole('heading', { name: 'Signaturerstellung läuft...' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([{ type: 'cancel' }]);
    // Warteschlange mit mehreren Dokumenten sichtbar
    expect(screen.getByRole('complementary', { name: 'Warteschlange (2)' })).toBeInTheDocument();
  });

  it('Batch-Fortschritt ist bestimmt', async () => {
    renderApp({ view: signingView({ kind: 'waiting', progress: { Document: { index: 2, total: 4 } } }) });
    const bar = await screen.findByRole('progressbar', { name: 'Dokument 2 von 4 wird signiert …' });
    expect(bar).toHaveAttribute('aria-valuenow', '2');
    expect(bar).toHaveAttribute('aria-valuemax', '4');
  });

  it('Texte aller Fortschritte', () => {
    const t = translator('en');
    expect(progressText(t, 'ReadingCertificate')).toBe('Reading certificate …');
    expect(progressText(t, 'WaitingForServer')).toBe('Waiting for the signature service …');
    expect(progressText(t, 'Signing')).toBe('Signature creation in progress...');
    expect(progressText(t, 'Done')).toBe('Signature created.');
  });
});

describe('Signieren: Kurzhinweis zum Datenschutz (ADR 0003)', () => {
  it('ID Austria: Kurzhinweis unter der Maske; „Details“ zeigt die Angaben, solange die Einstellungen gesperrt sind', async () => {
    const { mock, user } = renderApp({ view: formView({ SmsTan: { reference_value: 'ab', ctx: ctx() } }) });
    const note = await screen.findByTestId('privacy-note');
    expect(note).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt. Details');
    await user.click(within(note).getByRole('link', { name: 'Details zum Datenschutz' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Datenschutzinformation' });
    expect(dialog).toHaveTextContent('an die Firma A-Trust gesendet');
    expect(dialog).toHaveTextContent('Mit einem Keystore oder der Bürgerkarte');
    await user.click(within(dialog).getByRole('link', { name: /datenschutzerklaerung\.pdf/ }));
    expect(mock.openedUrls).toEqual(['https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf']);
    await user.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
    // weder Einstellungen (während des Signierens gesperrt) noch ein Abbruch
    expect(mock.dispatched).toEqual([]);
    expect(screen.getByLabelText('TAN:')).toBeInTheDocument();
  });

  it('auch während des Wartens auf A-Trust; Escape schließt die Angaben', async () => {
    const { user } = renderApp({ view: signingView({ kind: 'waiting', progress: 'WaitingForServer' }) });
    await user.click(await screen.findByRole('link', { name: 'Details zum Datenschutz' }));
    await screen.findByRole('alertdialog', { name: 'Datenschutzinformation' });
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
  });

  it('Keystore (lokal): kein Kurzhinweis', async () => {
    renderApp({ view: signingView({ kind: 'secret', request: 4, prompt: { kind: 'KeystorePassword', subject: 'id.p12', retry: false } }) });
    await screen.findByLabelText(/Keystore-Passwort/, { selector: 'input' });
    expect(screen.queryByTestId('privacy-note')).not.toBeInTheDocument();
    renderApp({ view: signingView({ kind: 'waiting', progress: 'Signing' }, 'de', false) });
    await screen.findAllByRole('progressbar');
    expect(screen.queryByTestId('privacy-note')).not.toBeInTheDocument();
  });
});

describe('Signieren: Anmeldung (ID Austria)', () => {
  it('lehnt die Bridge das Absenden ab, wird OK wieder frei (kein Hängenbleiben)', async () => {
    const { mock, user } = renderApp({ view: formView({ Login: { prefill_username: 'max', remember: false, ctx: ctx() } }) });
    const dispatch = vi.spyOn(mock, 'dispatch').mockRejectedValueOnce(new Error('ng-ui-bridge-error'));
    const password = await screen.findByLabelText('Passwort:');
    await user.type(password, 'geheim-123{Enter}');
    expect(await screen.findByRole('alert', { name: '' })).toHaveTextContent('Die Verbindung zum Signaturkern ist gestört.');
    await waitFor(() => expect(screen.getByRole('button', { name: 'OK' })).toBeEnabled());
    await user.type(password, 'geheim-123{Enter}');
    expect(dispatch).toHaveBeenCalledTimes(2);
    expect(screen.getByRole('button', { name: 'OK' })).toBeDisabled();
  });

  it('Keystore-Passwort: nach abgelehntem Senden erneut möglich', async () => {
    const { mock, user } = renderApp({ view: signingView({ kind: 'secret', request: 4, prompt: { kind: 'KeystorePassword', subject: 'id.p12', retry: false } }) });
    vi.spyOn(mock, 'dispatch').mockRejectedValueOnce(new Error('ng-ui-core-unavailable'));
    const field = await screen.findByLabelText(/Keystore-Passwort/, { selector: 'input' });
    await user.type(field, 'pw{Enter}');
    await waitFor(() => expect(screen.getByRole('button', { name: 'OK' })).toBeEnabled());
  });

  it('Fokus, Vorbelegung, Merken, Hinweiszeile, Absenden', async () => {
    const { mock, user } = renderApp({
      view: formView({ Login: { prefill_username: '+43664123456789', remember: true, ctx: ctx({ signature_data_url: 'https://a-trust.at/x' }) } }),
    });
    const password = await screen.findByLabelText('Passwort:');
    await waitFor(() => expect(password).toHaveFocus());
    expect(screen.getByLabelText('Benutzername:')).toHaveValue('+43664123456789');
    const remember = screen.getByRole('checkbox', { name: /Daten merken/ });
    expect(remember).toBeChecked();
    expect(screen.getByText('Dieses Service wird von der Fa. A-Trust zu Verfügung gestellt')).toBeInTheDocument();
    await user.click(remember);
    await user.type(password, 'geheim-123{Enter}');
    expect(mock.dispatched).toEqual([
      { type: 'submit_form', request: 1, answer: { kind: 'login', username: '+43664123456789', password: 'geheim-123', remember: false } },
    ]);
    // Doppelklickschutz und Passwort verworfen
    expect(screen.getByRole('button', { name: 'OK' })).toBeDisabled();
    expect(password).toHaveValue('');
  });

  it('Passwortlänge 5–200 wird vor dem Senden geprüft; leerer Benutzername', async () => {
    const { mock, user } = renderApp({ view: formView({ Login: { prefill_username: null, remember: false, ctx: ctx() } }) });
    const username = await screen.findByLabelText('Benutzername:');
    await waitFor(() => expect(username).toHaveFocus());
    await user.type(screen.getByLabelText('Passwort:'), 'abcd{Enter}');
    expect(await screen.findByRole('alert')).toHaveTextContent('Bitte geben Sie den Benutzernamen');
    await user.type(username, 'max');
    await user.type(screen.getByLabelText('Passwort:'), '{Enter}');
    expect(screen.getByRole('alert')).toHaveTextContent('Eingegebenes Passwort ist zu kurz!');
    expect(mock.dispatched).toEqual([]);
  });

  it('Problem vom Kern (erneute Login-Seite, A-Trust-Text)', async () => {
    renderApp({ view: formView({ Login: { prefill_username: 'max', remember: false, ctx: ctx({ problem: 'LoginRejected' }) } }) });
    expect(await screen.findByRole('alert')).toHaveTextContent('Die Anmeldung wurde abgelehnt');
  });

  it('Signaturdaten anzeigen sendet open_signature_data', async () => {
    const { mock, user } = renderApp({
      view: formView({ Login: { prefill_username: 'max', remember: false, ctx: ctx({ signature_data_url: 'https://a-trust.at/x' }) } }, 4),
    });
    await user.click(await screen.findByRole('link', { name: 'Signaturdaten anzeigen' }));
    expect(mock.dispatched).toEqual([{ type: 'open_signature_data', request: 4 }]);
  });

  it('Abbrechen', async () => {
    const { mock, user } = renderApp({ view: formView({ Login: { prefill_username: null, remember: false, ctx: ctx() } }) });
    await user.click(await screen.findByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([{ type: 'cancel' }]);
  });
});

describe('Signieren: SMS-TAN', () => {
  it('Vergleichswert, Live-Warnung ab 4 Zeichen, Prüfungen, Absenden', async () => {
    const { mock, user } = renderApp({ view: formView({ SmsTan: { reference_value: 'Xk7Pq9', ctx: ctx() } }, 3) });
    expect(await screen.findByText('Xk7Pq9')).toHaveAttribute('aria-label', 'X k 7 P q 9');
    const tan = screen.getByLabelText('TAN:');
    await waitFor(() => expect(tan).toHaveFocus());
    await user.type(tan, 'Xk7P');
    expect(screen.getByText('Sie haben den Vergleichswert eingegeben!')).toBeInTheDocument();
    await user.keyboard('{Enter}');
    expect(screen.getByRole('alert')).toHaveTextContent('Vergleichswert');
    await user.clear(tan);
    await user.keyboard('{Enter}');
    expect(screen.getByRole('alert')).toHaveTextContent('Keine TAN eingeben');
    await user.type(tan, '1234567{Enter}');
    expect(screen.getByRole('alert')).toHaveTextContent('zu lang');
    await user.clear(tan);
    await user.type(tan, ' 917253 {Enter}');
    expect(mock.dispatched).toEqual([{ type: 'submit_form', request: 3, answer: { kind: 'tan', tan: '917253' } }]);
  });

  it('FIDO2 sichtbar nach Kontext; Wechsel', async () => {
    const { mock, user } = renderApp({ view: formView({ SmsTan: { reference_value: 'ab', ctx: ctx({ can_switch_to_fido2: true }) } }, 5) });
    await user.click(await screen.findByRole('button', { name: 'FIDO2' }));
    expect(mock.dispatched).toEqual([{ type: 'submit_form', request: 5, answer: { kind: 'switch_to_fido2' } }]);
  });

  it('FIDO2 unsichtbar ohne Plattformunterstützung', async () => {
    renderApp({ view: formView({ SmsTan: { reference_value: 'ab', ctx: ctx({ fido2_visible: false }) } }) });
    await screen.findByLabelText('TAN:');
    expect(screen.queryByRole('button', { name: 'FIDO2' })).not.toBeInTheDocument();
  });
});

describe('Signieren: QR-Code und App', () => {
  it('QR-Bild, Countdown, SMS anfordern, Weiter', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { mock, user } = renderApp({
        view: formView({ QrCode: { reference_value: 'Xk7Pq', image: { bytes: [60, 115, 118, 103, 47, 62], mime: 'image/svg+xml' }, timeout_secs: 125, ctx: ctx() } }, 6),
      });
      const img = await screen.findByRole('img', { name: 'QR-Code zum Scannen mit der App „ID Austria“' });
      expect(img.getAttribute('src')).toBe(`data:image/svg+xml;base64,${btoa('<svg/>')}`);
      expect(screen.getByText('Verbleibende Zeit: 2:05')).toBeInTheDocument();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2100);
      });
      expect(screen.getByText('Verbleibende Zeit: 2:03')).toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'SMS anfordern' }));
      expect(mock.dispatched).toEqual([{ type: 'submit_form', request: 6, answer: { kind: 'switch_to_sms' } }]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('QR-Bild: nur Bild-MIME-Typen', () => {
    expect(qrDataUrl({ bytes: [1, 2], mime: 'image/PNG' })).toBe('data:image/png;base64,AQI=');
    expect(qrDataUrl({ bytes: [1, 2], mime: 'text/html' })).toBe('data:image/png;base64,AQI=');
    expect(qrDataUrl({ bytes: [], mime: null })).toBe('data:image/png;base64,');
  });

  it('fehlendes QR-Bild', async () => {
    renderApp({ view: formView({ QrCode: { reference_value: 'Xk7Pq', image: null, timeout_secs: null, ctx: ctx({ can_switch_to_sms: false }) } }) });
    expect(await screen.findByRole('alert')).toHaveTextContent('Konnte den QR code nicht laden');
    expect(screen.getByRole('button', { name: 'SMS anfordern' })).toBeDisabled();
  });

  it('App-Bestätigung: Text, Countdown abgelaufen, Weiter', async () => {
    const { mock, user } = renderApp({ view: formView({ WaitForApp: { reference_value: 'Xk7Pq', timeout_secs: 0, ctx: ctx() } }, 7) });
    expect(await screen.findByText(/ID Austria-App/)).toBeInTheDocument();
    expect(screen.getByText('Die Zeit ist abgelaufen.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Weiter' }));
    expect(mock.dispatched).toEqual([{ type: 'submit_form', request: 7, answer: { kind: 'continue' } }]);
  });
});

describe('Signieren: FIDO2', () => {
  it('läuft: Knopf gesperrt, Statustext', async () => {
    renderApp({ view: formView({ Fido2: { in_progress: true, ctx: ctx() } }) });
    expect(await screen.findByRole('button', { name: 'Autorisierung starten' })).toBeDisabled();
    expect(screen.getByRole('status')).toHaveTextContent('In Arbeit...');
  });

  it('fehlgeschlagen: Meldung, erneut starten', async () => {
    const { mock, user } = renderApp({ view: formView({ Fido2: { in_progress: false, ctx: ctx({ problem: { Fido2Failed: 'timeout' } }) } }, 9) });
    expect(await screen.findByRole('alert')).toHaveTextContent('FIDO2 ist fehlgeschlagen: timeout');
    await user.click(screen.getByRole('button', { name: 'Autorisierung starten' }));
    expect(mock.dispatched).toEqual([{ type: 'submit_form', request: 9, answer: { kind: 'start_fido2' } }]);
  });
});

describe('Signieren: Passwort und Meldungen', () => {
  it('Keystore-Passwort mit Wiederholung', async () => {
    const { mock, user } = renderApp({
      view: signingView({ kind: 'secret', request: 11, prompt: { kind: 'KeystorePassword', subject: 'keystore.p12', retry: true } }),
    });
    expect(await screen.findByRole('heading', { name: 'Keystore-Passwort' })).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Keystore-Passwort nicht akzeptiert');
    expect(screen.getByText('Für: keystore.p12')).toBeInTheDocument();
    const field = screen.getByLabelText('Keystore-Passwort eingeben:');
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, 'geheim{Enter}');
    expect(mock.dispatched).toEqual([{ type: 'submit_secret', request: 11, secret: 'geheim' }]);
    expect(field).toHaveValue('');
  });

  it.each([
    ['Ok', ['OK']],
    ['OkCancel', ['Abbrechen', 'OK']],
    ['RetryCancel', ['Abbrechen', 'Wiederholen']],
  ] as const)('Meldung mit %s', async (buttons, labels) => {
    const { mock, user } = renderApp({
      view: signingView({ kind: 'notice', request: 12, notice: { kind: 'Error', code: 'AtrustError', text: 'Fehler 7', buttons } }),
    });
    const dialog = await screen.findByRole('alertdialog', { name: 'Fehler' });
    expect(dialog).toHaveTextContent('a-trust.at meldet:');
    expect(dialog).toHaveTextContent('Fehler 7');
    const found = within(dialog)
      .getAllByRole('button')
      .map((b) => b.textContent);
    expect(found).toEqual(labels);
    await user.click(within(dialog).getByRole('button', { name: labels[labels.length - 1] }));
    const expected = buttons === 'RetryCancel' ? 'Retry' : 'Ok';
    expect(mock.dispatched).toEqual([{ type: 'answer_notice', request: 12, answer: expected }]);
  });

  it('Texte aller Meldungscodes (EN)', () => {
    const t = translator('en');
    const n = (code: Notice['code'], text: string | null = null, kind: Notice['kind'] = 'Error') => noticeTexts(t, { kind, code, text, buttons: 'Ok' });
    expect(n('AtrustInterstitial', 'Your certificate expires', 'Info')).toEqual({ title: 'Information', text: 'Your certificate expires' });
    expect(n('ProxyAuthRequired').text).toContain('407');
    expect(n('DocumentTooLarge').text).toContain('size limit');
    expect(n({ Http: { status: 503 } }, 'Service Unavailable').text).toContain('HTTP 503 Service Unavailable');
    expect(n('ConnectionFailed', 'timeout').text).toContain('timeout');
    expect(n('LocalBkuNotReachable', null, 'Warning')).toMatchObject({ title: 'Warning' });
    expect(n('KeystoreFileMissing', 'k.p12').text).toBe('Keystore file k.p12 does not exist!');
    expect(n('KeystoreAliasMissing', 'signer').text).toBe('Key alias signer not found in keystore');
    expect(n('KeystoreError').text).toContain('keystore');
    expect(n('KeystoreConversionNeeded').text).toContain('PKCS#12');
  });

  it('Texte aller Formularprobleme', () => {
    const t = translator('de');
    expect(problemText(t, 'PasswordTooLong')).toContain('zu lang');
    expect(problemText(t, 'NoTan')).toBe('Keine TAN eingeben');
    expect(problemText(t, 'TanTooLong')).toContain('zu lang');
    expect(problemText(t, 'EnteredReferenceValue')).toContain('Vergleichswert');
    expect(problemText(t, 'QrImageFailed')).toContain('QR');
    expect(problemText(t, { Atrust: 'Gesperrt' })).toBe('Nachricht vom Server: Gesperrt');
  });
});
