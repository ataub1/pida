/** Stapelbetrieb: Liste, „Alle signieren“ mit einer Anmeldung, Fortschritt, Zusammenfassung. */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { login, renderApp } from '../../test/render';

describe('Stapel', () => {
  it('signiert alle bereiten Dokumente mit einer Anmeldung und fasst zusammen', async () => {
    const { user } = renderApp({ scenario: 'batch', privacyAcked: true });
    const list = screen.getByRole('list', { name: 'Dokumente im Stapel' });
    const rows = within(list).getAllByRole('listitem');
    expect(rows).toHaveLength(4);
    expect(within(rows[0]!).getByText('Bereit')).toBeInTheDocument();
    expect(within(rows[1]!).getByText('Positioniert')).toBeInTheDocument();
    expect(within(rows[1]!).getByText('im QR-Platzhalter')).toBeInTheDocument();
    expect(within(rows[2]!).getByText('Fehler')).toBeInTheDocument();
    expect(within(rows[0]!).getByText('2 Seiten · 96 KB')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /Alle signieren/ }));
    const dialog = await login(user);
    expect(within(dialog).getByText('3 Dokumente – eine Anmeldung')).toBeInTheDocument();
    await user.click(await within(dialog).findByRole('button', { name: 'In App bestätigt (Demo)' }));

    // Fortschritt und Zusammenfassung
    expect(await screen.findByText('Stapel abgeschlossen')).toBeInTheDocument();
    expect(screen.getByText(/3 von 4 Dokumenten signiert/)).toBeInTheDocument();
    expect(screen.getByText('1 Dokument wurde wegen eines Fehlers übersprungen.')).toBeInTheDocument();
    expect(within(list).getAllByText('Fertig')).toHaveLength(3);
    expect(within(list).getByText('als Vertrag_signiert.pdf')).toBeInTheDocument();
    // Toast-Variante von „Fertig“
    expect(await screen.findByText('3 Dokumente signiert')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Alle signieren/ })).toBeDisabled();
  });

  it('öffnet ein einzelnes Dokument im Arbeitsbereich und kehrt zurück', async () => {
    const { user } = renderApp({ scenario: 'batch' });
    await user.click(screen.getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Angebot_2026-118.pdf' })).toBeInTheDocument();
    expect(screen.getByRole('application', { name: 'Signaturblock' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Zurück zum Stapel' }));
    const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(within(list).getAllByRole('listitem')[0]!).getByText('Positioniert')).toBeInTheDocument();
  });

  it('entsperrt ein kennwortgeschütztes Dokument aus dem Stapel', async () => {
    const { user } = renderApp({ scenario: 'batch' });
    await user.click(screen.getByRole('button', { name: 'Kennwort eingeben' }));
    await user.type(await screen.findByLabelText('Kennwort'), 'demo');
    await user.click(screen.getByRole('button', { name: 'Entsperren' }));
    expect(await screen.findByRole('application', { name: 'Signaturblock' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Zurück zum Stapel' }));
    await waitFor(() => expect(screen.getByRole('button', { name: /Alle signieren/ })).toHaveTextContent('4'));
  });

  it('mehrere abgelegte Dateien werden zum Stapel', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.upload(screen.getByTestId('file-input'), [
      new File(['a'], 'Rechnung_1.pdf', { type: 'application/pdf' }),
      new File(['b'], 'Rechnung_2.pdf', { type: 'application/pdf' }),
    ]);
    const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(list).getByText('Rechnung_1.pdf')).toBeInTheDocument();
    expect(within(list).getByText('Rechnung_2.pdf')).toBeInTheDocument();
  });
});
