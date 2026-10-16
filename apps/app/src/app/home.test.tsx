/**
 * Haus-Symbol in der Kopfzeile (Wunsch des Auftraggebers): sichtbar außerhalb der Startansicht, führt zur
 * Startseite; wer Arbeit verwerfen würde (Stapel mit mehreren Dokumenten), bestätigt vorher.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import { expectNoAxeViolations } from '../test/a11y';
import { renderApp } from '../test/render';

function settings() {
  const s = defaultSettings();
  return { ...s, backend: { default: 'id_austria' as const }, output: { ...s.output, folder: '.' } };
}

describe('Haus-Symbol in der Kopfzeile', () => {
  it('nicht auf der Startansicht, aber im Arbeitsbereich; führt ohne Rückfrage zur Startseite', async () => {
    const { user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' });
    const header = screen.getByRole('banner');
    expect(within(header).queryByRole('button', { name: 'Zur Startseite' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('Stapel: kurze Rückfrage, „Abbrechen“ bleibt, „Zur Startseite“ verlässt den Stapel', async () => {
    const { user } = renderApp({ scenario: 'batch', settings: settings() });
    await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    const home = within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' });
    await user.click(home);
    const confirm = await screen.findByRole('alertdialog', { name: 'Zur Startseite?' });
    await expectNoAxeViolations();
    await user.click(within(confirm).getByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(screen.getByRole('list', { name: 'Dokumente im Stapel' })).toBeInTheDocument();
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Zur Startseite' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
  });
});
