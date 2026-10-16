/**
 * „Zuletzt signiert“ auf der Startansicht (Paket UI-B, ADR 0004 §3.10, Abnahme E3 Punkt 5 [M]): Karte nur
 * mit `ViewState::recent`, höchstens zehn Einträge, je Eintrag nur Name und Zeitpunkt (kein Signiermittel),
 * Entfernen ist eine ausdrückliche Aktion, leere Liste, axe.
 *
 * Nachtrag 15 (Prüfmodus entfernt): Ein Eintrag bietet, was „Fertig“ kann – der Eintrag und „Öffnen“ öffnen die
 * Datei (`open_recent`), im Menü „Im Ordner zeigen“ (Desktop) bzw. „Teilen“ (Mobile) und „Erneut signieren“ wie
 * „Öffnen“ (`sign_recent`, bei geparkter Arbeit mit Rückfrage); fehlende Datei → Hinweis des Kerns. Gegentest: kein
 * Einstieg in einen Prüfmodus mehr.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { noActions } from '../../core/defaults';
import type { RecentItemView } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { lastEvent, renderApp } from '../../test/render';
import { IOS, onMobile, viewOf } from '../../test/views';

function item(id: number, extra: Partial<RecentItemView> = {}): RecentItemView {
  return { id, file_name: `Dokument_${id}_signed.pdf`, folder_display: '/Users/demo/Dokumente', signed_at: '2026-10-14T09:12:00+02:00', exists: true, ...extra };
}

const START = (recent: RecentItemView[] | null) => viewOf({ step: 'open', queue: [] }, { recent });
const MIETVERTRAG = '/Users/demo/Dokumente/Wohnung/Mietvertrag_signiert.pdf';

describe('Zuletzt signiert', () => {
  it('Mock mit Beispielen: Karte neben der Ablage, neueste zuerst, nur Name und Zeitpunkt', async () => {
    renderApp({ examples: true });
    const card = await screen.findByTestId('recent-panel');
    expect(within(card).getByRole('heading', { level: 2, name: 'Zuletzt signiert' })).toBeInTheDocument();
    const buttons = within(card).getAllByRole('button', { name: /^„.+“ öffnen$/ });
    expect(buttons.map((b) => b.getAttribute('aria-label'))).toEqual([
      '„Kaufvertrag_Fahrzeug_signiert.pdf“ öffnen',
      '„Mietvertrag_signiert.pdf“ öffnen',
      '„Angebot_2026-117_signed.pdf“ öffnen',
    ]);
    expect(buttons[0]).toHaveAccessibleDescription(/^14\.10\.2026/);
    expect(card).not.toHaveTextContent(/ID Austria|Keystore|Chipkarte/);
    expect(screen.getByRole('complementary', { name: 'Zuletzt signiert' })).toContainElement(card);
    await expectNoAxeViolations();
  });

  it('Eintrag öffnet die Datei wie „Fertig“ (kein Prüfmodus, Nachtrag 15); fehlende Datei: Hinweis des Kerns, Eintrag bleibt', async () => {
    const { mock, user } = renderApp({ examples: true });
    const card = await screen.findByTestId('recent-panel');
    await user.click(within(card).getByRole('button', { name: '„Mietvertrag_signiert.pdf“ öffnen' }));
    expect(lastEvent(mock)).toEqual({ type: 'open_recent', id: 2 });
    expect(mock.nativeCalls).toContain(`openRecent:${MIETVERTRAG}`);
    // Gegentest: es öffnet sich keine Prüfansicht, die Startansicht bleibt
    expect(screen.getByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent(/Prüfmodus|verification mode/i);
    expect(mock.dispatched.map((e) => e.type)).not.toContain('open_files');
    const missing = within(card).getByRole('button', { name: '„Angebot_2026-117_signed.pdf“ öffnen' });
    expect(missing).toHaveAccessibleDescription(/Datei nicht mehr vorhanden/);
    await user.click(missing);
    expect(lastEvent(mock)).toEqual({ type: 'open_recent', id: 1 });
    expect(await screen.findByText('Die Datei ist nicht mehr am gespeicherten Ort.')).toBeInTheDocument();
    expect(within(card).getByRole('button', { name: '„Angebot_2026-117_signed.pdf“ öffnen' })).toBeInTheDocument();
  });

  it('Menü (Desktop): Öffnen, Im Ordner zeigen, Erneut signieren – kein Teilen, kein Prüfen', async () => {
    const { mock, user } = renderApp({ examples: true });
    const card = await screen.findByTestId('recent-panel');
    await user.click(within(card).getByRole('button', { name: 'Weitere Aktionen für „Mietvertrag_signiert.pdf“' }));
    const menu = await screen.findByRole('menu', { name: 'Weitere Aktionen für „Mietvertrag_signiert.pdf“' });
    expect(within(menu).getAllByRole('menuitem').map((m) => m.textContent)).toEqual(['Öffnen', 'Im Ordner zeigen', 'Erneut signieren']);
    await expectNoAxeViolations();
    await user.click(within(menu).getByRole('menuitem', { name: 'Im Ordner zeigen' }));
    expect(lastEvent(mock)).toEqual({ type: 'reveal_recent', id: 2 });
    expect(mock.nativeCalls).toContain(`revealRecent:${MIETVERTRAG}`);
    await user.click(within(card).getByRole('button', { name: 'Weitere Aktionen für „Mietvertrag_signiert.pdf“' }));
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Öffnen' }));
    expect(lastEvent(mock)).toEqual({ type: 'open_recent', id: 2 });
  });

  it('„Erneut signieren“ öffnet die Datei wie „Öffnen“ im Arbeitsbereich – ohne Sitzung (A1)', async () => {
    const { mock, user } = renderApp({ examples: true });
    const card = await screen.findByTestId('recent-panel');
    await user.click(within(card).getByRole('button', { name: 'Weitere Aktionen für „Mietvertrag_signiert.pdf“' }));
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Erneut signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_recent', id: 2 });
    const header = screen.getByRole('banner');
    await waitFor(() => expect(header).toHaveTextContent('Mietvertrag_signiert.pdf'));
    expect(mock.dispatched.some((e) => e.type === 'sign_with' || e.type === 'start_batch')).toBe(false);
  });

  it('„Erneut signieren“ bei geparkter Arbeit: dieselbe Rückfrage wie bei neuen PDFs', async () => {
    const view = START([item(4)]);
    view.actions = { ...noActions(), open_files: true, settings: true, home: true };
    view.parked = { doc_name: 'Vertrag.pdf', documents: 1, placed_by_hand: false, unsaved: false };
    const { mock, user } = renderApp({ view });
    const card = await screen.findByTestId('recent-panel');
    const more = within(card).getByRole('button', { name: 'Weitere Aktionen für „Dokument_4_signed.pdf“' });
    await user.click(more);
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Erneut signieren' }));
    let dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    expect(mock.dispatched).toEqual([]);
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([]);
    await user.click(more);
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Erneut signieren' }));
    dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Zum Stapel hinzufügen' }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_recent', id: 4, mode: 'add' });
    await user.click(more);
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Erneut signieren' }));
    dialog = await screen.findByRole('alertdialog', { name: 'Zur offenen Arbeit hinzufügen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Offene schließen und neu beginnen' }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_recent', id: 4, mode: 'replace' });
  });

  it('„Erneut signieren“ gesperrt, solange nichts geöffnet werden kann (`actions.open_files`)', async () => {
    const { mock, user } = renderApp({ view: START([item(1)]) });
    const card = await screen.findByTestId('recent-panel');
    await user.click(within(card).getByRole('button', { name: 'Weitere Aktionen für „Dokument_1_signed.pdf“' }));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Erneut signieren' })).toHaveAttribute('aria-disabled', 'true');
    expect(within(menu).getByRole('menuitem', { name: 'Öffnen' })).not.toHaveAttribute('aria-disabled');
    expect(mock.dispatched).toEqual([]);
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

  it('Mobile: ohne Ordner, Englisch mit en-GB-Datum; Menü mit Teilen statt Ordner', async () => {
    const { mock, user } = renderApp({ view: onMobile({ ...START([item(1, { folder_display: null })]), language: 'en' }, IOS) });
    const card = await screen.findByTestId('recent-panel');
    expect(within(card).getByRole('heading', { name: 'Recently signed' })).toBeInTheDocument();
    const open = within(card).getByRole('button', { name: 'Open “Dokument_1_signed.pdf”' });
    expect(open).toHaveAccessibleDescription(/^14 Oct 2026/);
    expect(open.querySelector('[title]')).toBeNull();
    await expectNoAxeViolations();
    await user.click(within(card).getByRole('button', { name: 'More actions for “Dokument_1_signed.pdf”' }));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getAllByRole('menuitem').map((m) => m.textContent)).toEqual(['Open', 'Share', 'Sign again']);
    await user.click(within(menu).getByRole('menuitem', { name: 'Share' }));
    expect(lastEvent(mock)).toEqual({ type: 'share_recent', id: 1 });
  });
});
