/**
 * „Zuletzt signiert“ auf der Startansicht (Paket UI-B, ADR 0004 §3.10, Abnahme E3 Punkt 5 [M]): Karte nur
 * mit `ViewState::recent`, höchstens zehn Einträge, je Eintrag nur Name und Zeitpunkt (kein Signiermittel),
 * Eintrag → `verify_recent` (öffnet den Prüfmodus), fehlende Datei → Hinweis des Kerns, Entfernen ist eine
 * ausdrückliche Aktion, leere Liste, axe.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { RecentItemView } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { lastEvent, renderApp } from '../../test/render';
import { IOS, onMobile, viewOf } from '../../test/views';

function item(id: number, extra: Partial<RecentItemView> = {}): RecentItemView {
  return { id, file_name: `Dokument_${id}_signed.pdf`, folder_display: '/Users/demo/Dokumente', signed_at: '2026-10-14T09:12:00+02:00', exists: true, ...extra };
}

const START = (recent: RecentItemView[] | null) => viewOf({ step: 'open', queue: [] }, { recent });

describe('Zuletzt signiert', () => {
  it('Mock mit Beispielen: Karte neben der Ablage, neueste zuerst, nur Name und Zeitpunkt', async () => {
    renderApp({ examples: true });
    const card = await screen.findByTestId('recent-panel');
    expect(within(card).getByRole('heading', { level: 2, name: 'Zuletzt signiert' })).toBeInTheDocument();
    const buttons = within(card).getAllByRole('button', { name: /im Prüfmodus öffnen$/ });
    expect(buttons.map((b) => b.getAttribute('aria-label'))).toEqual([
      '„Kaufvertrag_Fahrzeug_signiert.pdf“ im Prüfmodus öffnen',
      '„Mietvertrag_signiert.pdf“ im Prüfmodus öffnen',
      '„Angebot_2026-117_signed.pdf“ im Prüfmodus öffnen',
    ]);
    expect(buttons[0]).toHaveAccessibleDescription(/^14\.10\.2026/);
    expect(card).not.toHaveTextContent(/ID Austria|Keystore|Chipkarte/);
    expect(screen.getByRole('complementary')).toContainElement(card);
    await expectNoAxeViolations();
  });

  it('Eintrag öffnet den Prüfmodus (E4); fehlende Datei: Hinweis des Kerns, Eintrag bleibt', async () => {
    const { mock, user } = renderApp({ examples: true });
    let card = await screen.findByTestId('recent-panel');
    await user.click(within(card).getByRole('button', { name: '„Mietvertrag_signiert.pdf“ im Prüfmodus öffnen' }));
    expect(lastEvent(mock)).toEqual({ type: 'verify_recent', id: 2 });
    expect(await screen.findByRole('heading', { level: 1, name: 'Mietvertrag_signiert.pdf' })).toBeInTheDocument();
    expect(await screen.findByTestId('vf-summary')).toHaveTextContent('Alle 2 Signaturen sind unverändert und kryptografisch intakt');
    await user.click(screen.getByRole('button', { name: 'Prüfmodus verlassen' }));
    expect(lastEvent(mock)).toEqual({ type: 'close_verify' });
    card = await screen.findByTestId('recent-panel');
    const missing = within(card).getByRole('button', { name: '„Angebot_2026-117_signed.pdf“ im Prüfmodus öffnen' });
    expect(missing).toHaveAccessibleDescription(/Datei nicht mehr vorhanden/);
    await user.click(missing);
    expect(lastEvent(mock)).toEqual({ type: 'verify_recent', id: 1 });
    expect(await screen.findByText('Die Datei ist nicht mehr am gespeicherten Ort.')).toBeInTheDocument();
    expect(within(card).getByRole('button', { name: '„Angebot_2026-117_signed.pdf“ im Prüfmodus öffnen' })).toBeInTheDocument();
  });

  it('Entfernen ist eine ausdrückliche Aktion je Eintrag', async () => {
    const { mock, user } = renderApp({ examples: true });
    const card = await screen.findByTestId('recent-panel');
    await user.click(within(card).getByRole('button', { name: '„Angebot_2026-117_signed.pdf“ aus der Liste entfernen' }));
    expect(lastEvent(mock)).toEqual({ type: 'remove_recent', id: 1 });
    await waitFor(() => expect(within(card).queryByText('Angebot_2026-117_signed.pdf')).not.toBeInTheDocument());
    expect(within(card).getAllByRole('listitem')).toHaveLength(2);
  });

  it('höchstens zehn Einträge; leere Liste mit Hinweis; abgeschaltet ohne Karte', async () => {
    const many = Array.from({ length: 12 }, (_, i) => item(12 - i));
    const r = renderApp({ view: START(many) });
    expect(within(await screen.findByTestId('recent-panel')).getAllByRole('listitem')).toHaveLength(10);
    r.unmount();
    const e = renderApp({ view: START([]) });
    expect(within(await screen.findByTestId('recent-panel')).getByText('Noch keine signierten Dokumente.')).toBeInTheDocument();
    await expectNoAxeViolations();
    e.unmount();
    renderApp({ view: START(null) });
    await screen.findByRole('heading', { level: 1 });
    expect(screen.queryByTestId('recent-panel')).not.toBeInTheDocument();
  });

  it('Mobile: ohne Ordner, Englisch mit en-GB-Datum', async () => {
    renderApp({ view: onMobile({ ...START([item(1, { folder_display: null })]), language: 'en' }, IOS) });
    const card = await screen.findByTestId('recent-panel');
    expect(within(card).getByRole('heading', { name: 'Recently signed' })).toBeInTheDocument();
    const open = within(card).getByRole('button', { name: 'Open “Dokument_1_signed.pdf” in verification mode' });
    expect(open).toHaveAccessibleDescription(/^14 Oct 2026/);
    expect(open.querySelector('[title]')).toBeNull();
    await expectNoAxeViolations();
  });
});
