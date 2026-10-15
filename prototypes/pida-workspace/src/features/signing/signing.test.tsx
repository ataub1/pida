/** Durchläufe: Öffnen → Signieren (App, FIDO2, SMS) → Fertig, sowie Fehlerwege der Signierkarte. */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { login, renderApp, signingDialog } from '../../test/render';

describe('Öffnen → Signieren mit App-Bestätigung → Fertig', () => {
  it('führt vom Leerzustand bis zum signierten Dokument', async () => {
    const { user } = renderApp({ scenario: 'start' });
    const input = screen.getByTestId('file-input');
    await user.upload(input, new File(['%PDF-1.7'], 'Vertrag.pdf', { type: 'application/pdf' }));

    // Arbeitsbereich: Sticker ist bereits auf der letzten Seite platziert
    expect(await screen.findByRole('heading', { level: 1, name: 'Vertrag.pdf' })).toBeInTheDocument();
    expect(screen.getByRole('application', { name: 'Signaturblock' })).toBeInTheDocument();
    expect(screen.getByText(/^Seite 3 · rechts/)).toBeInTheDocument();
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument(); // keine Positions-Zahlenfelder

    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await signingDialog();
    expect(within(dialog).getByText('Vertrag.pdf')).toBeInTheDocument();

    // Einmaliger Datenschutzhinweis
    await user.click(within(dialog).getByRole('button', { name: 'Verstanden, weiter' }));

    // Anmeldung: Benutzer vorbelegt, Passwort zu kurz → Fehler
    expect(within(dialog).getByLabelText('Telefonnummer oder Benutzername')).toHaveValue('+43 660 1234123');
    expect(within(dialog).getByRole('checkbox', { name: 'Benutzername merken' })).toBeChecked();
    await user.type(within(dialog).getByLabelText('Signatur-Passwort'), 'abc');
    await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));
    expect(await within(dialog).findByText('Das Passwort hat mindestens 5 Zeichen.')).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText('Signatur-Passwort'), 'defgh');
    await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));

    // App-Bestätigung mit Vergleichswert und Countdown
    expect(await within(dialog).findByText('Benachrichtigung an die ID-Austria-App gesendet')).toBeInTheDocument();
    expect(within(dialog).getByText('X k 7 P q')).toBeInTheDocument();
    expect(within(dialog).getByRole('timer', { name: /Verbleibende Zeit: [45]:\d\d/ })).toBeInTheDocument();
    expect(within(dialog).getByRole('listitem', { current: 'step' })).toHaveTextContent('In der App bestätigen');
    await user.click(within(dialog).getByRole('button', { name: 'In App bestätigt (Demo)' }));

    // Fertig
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
    expect(screen.getByText('Signatur gültig – geprüft')).toBeInTheDocument();
    expect(screen.getByText('Vertrag_signiert.pdf')).toBeInTheDocument();
    for (const name of ['Öffnen', 'Im Ordner zeigen', 'Teilen', 'Nächstes Dokument']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
    await user.click(screen.getByRole('button', { name: /Bei signaturpruefung\.gv\.at prüfen/ }));
    expect(await screen.findByText(/würde im Browser geöffnet/)).toBeInTheDocument();

    // Nächstes Dokument → zurück zum Start, das signierte Dokument steht unter „Zuletzt signiert“
    await user.click(screen.getByRole('button', { name: 'Nächstes Dokument' }));
    expect(await screen.findByRole('button', { name: '„Vertrag_signiert.pdf“ im Prüfmodus öffnen' })).toBeInTheDocument();
  });

  it('zeigt den Datenschutzhinweis nur beim ersten Mal', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await signingDialog();
    expect(within(dialog).queryByText('Kurz zum Datenschutz')).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('Signatur-Passwort')).toHaveFocus();
  });

  it('startet das Signieren mit Strg+Eingabe', async () => {
    renderApp({ scenario: 'contract', privacyAcked: true });
    fireEvent.keyDown(window, { key: 'Enter', ctrlKey: true });
    expect(await signingDialog()).toBeInTheDocument();
  });
});

describe('FIDO2-Zweig', () => {
  it('wechselt von der App-Bestätigung zu FIDO2 (PIN, Schlüssel berühren) und signiert', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    await user.click(await within(dialog).findByRole('button', { name: 'FIDO2-Schlüssel' }));

    expect(await within(dialog).findByRole('heading', { name: 'Sicherheitsschlüssel verwenden' })).toBeInTheDocument();
    expect(within(dialog).getByRole('listitem', { current: 'step' })).toHaveTextContent('Mit FIDO2 bestätigen');
    const pin = within(dialog).getByLabelText('PIN des Sicherheitsschlüssels');
    await user.type(pin, '12');
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));
    expect(await within(dialog).findByText('Die PIN hat mindestens 4 Zeichen.')).toBeInTheDocument();
    await user.type(pin, '34');
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));

    expect(await within(dialog).findByRole('heading', { name: 'Berühren Sie jetzt Ihren Sicherheitsschlüssel' })).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Schlüssel berührt (Demo)' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
  });

  it('geht mit der Demo-Antwort „FIDO2“ nach der Anmeldung direkt zu FIDO2', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: 'Demo' }));
    await user.click(screen.getByRole('radio', { name: 'FIDO2' }));
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    expect(await within(dialog).findByLabelText('PIN des Sicherheitsschlüssels')).toBeInTheDocument();
  });
});

describe('SMS-TAN-Zweig', () => {
  it('prüft die TAN (nicht der Vergleichswert) und signiert', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true, response: 'sms' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    const tan = await within(dialog).findByLabelText('TAN aus der SMS');
    expect(within(dialog).getByText(/an \+43 660 ···123 gesendet/)).toBeInTheDocument();
    await user.type(tan, 'Xk7');
    await user.click(within(dialog).getByRole('button', { name: 'Bestätigen' }));
    expect(await within(dialog).findByText(/Das ist der Vergleichswert/)).toBeInTheDocument();
    await user.clear(tan);
    await user.type(tan, '482913');
    await user.click(within(dialog).getByRole('button', { name: 'Bestätigen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
  });
});

describe('Fehler und Abbruch', () => {
  it('A-Trust nicht erreichbar → Details aufklappbar → Erneut versuchen', async () => {
    const { user } = renderApp({ scenario: 'error-atrust', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    expect(await within(dialog).findByRole('heading', { name: 'ID Austria ist gerade nicht erreichbar' })).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Technische Details' }));
    expect(within(dialog).getByText(/service\.a-trust\.at:443/)).toBeVisible();
    await user.click(within(dialog).getByRole('button', { name: 'Erneut versuchen' }));
    expect(await within(dialog).findByText('Benachrichtigung an die ID-Austria-App gesendet')).toBeInTheDocument();
  });

  it('Zeitablauf der App-Bestätigung → neue Bestätigung anfordern', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true, response: 'timeout' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    expect(await within(dialog).findByRole('heading', { name: 'Die Zeit ist abgelaufen' })).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Neue Bestätigung anfordern' }));
    expect(await within(dialog).findByText('Benachrichtigung an die ID-Austria-App gesendet')).toBeInTheDocument();
  });

  it('Ablehnung in der App wird verständlich gemeldet', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true, response: 'cancel' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    expect(await within(dialog).findByRole('heading', { name: 'In der App abgelehnt' })).toBeInTheDocument();
    expect(within(dialog).getByText('Die Signatur wurde in der ID-Austria-App abgelehnt. Es wurde nichts signiert.')).toBeInTheDocument();
  });

  it('Abbrechen ist jederzeit möglich (Knopf und Esc)', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    let dialog = await login(user);
    await within(dialog).findByText('Benachrichtigung an die ID-Austria-App gesendet');
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(await screen.findByText('Signieren abgebrochen – es wurde nichts signiert.')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    dialog = await signingDialog();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('Keystore- und Chipkarten-Weg', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('button', { name: /Signiermittel: ID Austria/ }));
    await user.click(await screen.findByRole('menuitemradio', { name: /Keystore · max\.mustermann\.p12/ }));
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await signingDialog(/Signieren mit Keystore/);
    await user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'pkcs12');
    await user.click(within(dialog).getByRole('button', { name: 'Signieren' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
  });
});
