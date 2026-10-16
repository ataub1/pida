/**
 * Haus-Symbol in der Kopfzeile (Wunsch des Auftraggebers): sichtbar außerhalb der Startansicht, führt **ohne
 * Rückfrage** zur Startseite – der Kern parkt das offene Dokument bzw. den Stapel (`go_home`, ADR 0004 Nachtrag
 * 10), die Startansicht zeigt es unter „In Bearbeitung“. Beim Signieren gesperrt wie bisher.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import { expectNoAxeViolations } from '../test/a11y';
import { lastEvent, renderApp } from '../test/render';
import { cardView } from '../test/views';

function settings() {
  const s = defaultSettings();
  return { ...s, backend: { default: 'id_austria' as const }, output: { ...s.output, folder: '.' } };
}

const START = { level: 1, name: 'Was möchten Sie signieren?' } as const;

describe('Haus-Symbol in der Kopfzeile', () => {
  it('nicht auf der Startansicht; im Arbeitsbereich ohne Rückfrage zur Startseite, das Dokument bleibt in Bearbeitung', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await screen.findByRole('heading', START);
    expect(within(screen.getByRole('banner')).queryByRole('button', { name: 'Zur Startseite' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    expect(await screen.findByRole('heading', START)).toBeInTheDocument();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(lastEvent(mock)).toEqual({ type: 'go_home' });
    expect(screen.getByRole('region', { name: 'In Bearbeitung' })).toHaveTextContent('Vertrag.pdf');
    // auf der Startansicht kein Haus-Symbol mehr
    expect(within(screen.getByRole('banner')).queryByRole('button', { name: 'Zur Startseite' })).toBeNull();
    await expectNoAxeViolations();
  });

  it('Stapel: ohne Rückfrage, „Stapel · 4 Dokumente“ in Bearbeitung, „Weiter bearbeiten“ führt zurück in die Übersicht', async () => {
    const { mock, user } = renderApp({ scenario: 'batch', settings: settings() });
    await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zur Startseite' }));
    const card = await screen.findByRole('region', { name: 'In Bearbeitung' });
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(card).toHaveTextContent('Stapel · 4 Dokumente');
    await user.click(within(card).getByRole('button', { name: 'Weiter bearbeiten' }));
    const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(4);
    expect(mock.dispatched.filter((e) => e.type === 'go_home' || e.type === 'resume_parked').map((e) => e.type)).toEqual(['go_home', 'resume_parked']);
  });

  it('Wortmarke wie das Haus-Symbol: parkt ohne Rückfrage', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'PIDA – Startseite' }));
    await screen.findByRole('region', { name: 'In Bearbeitung' });
    expect(screen.queryByRole('alertdialog')).toBeNull();
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'go_home' }));
  });

  it('beim Signieren gesperrt (kein Haus-Symbol, `actions.home` aus)', async () => {
    renderApp({ view: cardView({ kind: 'waiting', progress: 'Connecting' }) });
    await screen.findByRole('banner');
    expect(within(screen.getByRole('banner')).queryByRole('button', { name: 'Zur Startseite' })).toBeNull();
  });
});
