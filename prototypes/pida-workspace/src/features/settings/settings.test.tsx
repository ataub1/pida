/** Einstellungen: Live-Editor des Signaturbilds, Suche, sofort wirksame Änderungen. */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../test/render';

function preview(dialog: HTMLElement) {
  return within(dialog).getByRole('img', { name: /Vorschau des Signaturblocks/ });
}

describe('Einstellungen', () => {
  it('ändert die Signaturbild-Vorschau sofort (Profil, Sprache, Hinweistext)', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('button', { name: 'Signaturbild bearbeiten' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(preview(dialog)).toHaveAccessibleName(/Standard \(klein\), Deutsch/);
    expect(preview(dialog)).toHaveTextContent('Unterzeichner');
    expect(preview(dialog)).not.toHaveTextContent('Aussteller-Zertifikat');

    await user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    expect(preview(dialog)).toHaveAccessibleName(/Amtssignatur/);
    expect(preview(dialog)).toHaveTextContent('Aussteller-Zertifikat');
    expect(preview(dialog)).toHaveTextContent('amtssigniert');

    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(preview(dialog)).toHaveTextContent('Signatory');
    expect(preview(dialog)).toHaveTextContent('official signature');

    const note = within(dialog).getByLabelText('Hinweistext');
    await user.clear(note);
    await user.type(note, 'Intern – nur zur Ablage');
    expect(preview(dialog)).toHaveTextContent('Intern – nur zur Ablage');
    await user.click(within(dialog).getByRole('button', { name: 'Standardtext wiederherstellen' }));
    expect(preview(dialog)).toHaveTextContent('official signature');

    // Sofort wirksam: Inspektor im Arbeitsbereich zeigt das geänderte Signaturbild
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(document.querySelector('.insp-block-preview')).toHaveTextContent('Signatory');
  });

  it('übernimmt ein Logo per Dateiauswahl', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const input = dialog.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, new File(['<svg xmlns="http://www.w3.org/2000/svg"/>'], 'logo.svg', { type: 'image/svg+xml' }));
    expect(await within(dialog).findByText('Aktuell: logo.svg')).toBeInTheDocument();
    await waitFor(() => expect(preview(dialog).querySelector('img')).not.toBeNull());
  });

  it('durchsucht alle Bereiche', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.type(within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'proxy');
    expect(within(dialog).getByRole('heading', { name: 'Erweitert' })).toBeInTheDocument();
    expect(within(dialog).getByRole('radiogroup', { name: 'Proxy' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('heading', { name: 'Signaturbild' })).not.toBeInTheDocument();
    await user.clear(within(dialog).getByRole('searchbox'));
    await user.type(within(dialog).getByRole('searchbox'), 'xyz');
    expect(within(dialog).getByText('Keine Einstellung gefunden für „xyz“.')).toBeInTheDocument();
  });

  it('Speichern: Namensmuster mit Vorschau wirkt auf den Ausgabenamen', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const pattern = within(dialog).getByLabelText('Dateiname');
    expect(pattern).toHaveValue('{name}_signiert');
    expect(within(dialog).getByText('Vertrag_signiert.pdf')).toBeInTheDocument();
    await user.clear(pattern);
    await user.type(pattern, '{{name}-final');
    expect(within(dialog).getByText('Vertrag-final.pdf')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('radio', { name: /In festem Ordner/ }));
    expect(within(dialog).getAllByText('~/Dokumente/Signiert').length).toBeGreaterThan(0);
  });

  it('Darstellung und Sprache gelten sofort', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(dialog).getByRole('button', { name: 'Darstellung' }));
    await user.click(within(dialog).getByRole('radio', { name: 'Dunkel' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(await screen.findByRole('dialog', { name: 'Settings' })).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute('lang', 'en');
  });
});
