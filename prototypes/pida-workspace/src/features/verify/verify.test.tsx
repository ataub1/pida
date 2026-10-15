/** Prüfmodus: signiertes PDF → Signaturen-Panel mit Status und aufklappbaren Zertifikatsdetails. */
import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../test/render';

describe('Prüfmodus', () => {
  it('prüft ein Beispiel: Status je Signatur, Zertifikatsdetails aufklappbar', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Prüfmodus öffnen' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiertes PDF zum Prüfen ablegen' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Beispiel prüfen' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Kaufvertrag_Fahrzeug_signiert.pdf' })).toBeInTheDocument();
    expect(screen.getByText('Alle 2 Signaturen sind gültig')).toBeInTheDocument();
    const cards = screen.getAllByRole('listitem').filter((li) => li.classList.contains('sig-card'));
    expect(cards).toHaveLength(2);
    const first = cards[0]!;
    expect(within(first).getByRole('heading', { name: 'Max Mustermann' })).toBeInTheDocument();
    expect(within(first).getByText('Gültig')).toBeInTheDocument();
    expect(within(first).getByText(/Qualifizierte elektronische Signatur/)).toBeInTheDocument();
    expect(within(first).getByText('Dokument seit dieser Signatur unverändert')).toBeInTheDocument();

    const trigger = within(first).getByRole('button', { name: 'Zertifikatsdetails' });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    await user.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(within(first).getByText('a-sign-premium-mobile-07 (Demo-CA)')).toBeVisible();
    expect(within(first).getByText('PAdES-BASELINE-B')).toBeVisible();
    expect(within(first).getByText('1 288 004 912')).toBeVisible();
  });

  it('zeigt eine ungültige Signatur (Dokument nach dem Signieren verändert)', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: '„Angebot_2026-117_signiert.pdf“ im Prüfmodus öffnen' }));
    expect(await screen.findByText('Achtung: Eine Signatur ist ungültig')).toBeInTheDocument();
    expect(screen.getByText('Ungültig')).toBeInTheDocument();
    expect(screen.getByText('Dokument wurde nach dieser Signatur verändert')).toBeInTheDocument();
  });

  it('nimmt ein abgelegtes PDF im Prüfmodus entgegen und kann es weiter signieren', async () => {
    const { user } = renderApp({ scenario: 'verify' });
    await user.click(screen.getByRole('button', { name: 'Anderes Dokument prüfen' }));
    await user.upload(screen.getByTestId('file-input'), new File(['%PDF'], 'Vollmacht_signiert.pdf', { type: 'application/pdf' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Vollmacht_signiert.pdf' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Dieses Dokument signieren' }));
    expect(await screen.findByRole('application', { name: 'Signaturblock' })).toBeInTheDocument();
    expect(screen.getByText('Vorhandene Signaturen')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Auf neuer Seite' })).toBeDisabled();
  });
});
