/**
 * Integration K2 × A2 (ADR 0004 §6.2 Punkt 4, Abnahme E2 Punkte 1, 2, 3, 4, 5): Die Stapeloberfläche rendert
 * **Zustände des echten Kerns** statt des Mocks – `core/fixtures/real-core-batch-views.json` entsteht im Rust-Test
 * `crates/core-api/tests/batch.rs::real_core_batch_views_for_the_ui` (vier Dateien, eine kennwortgeschützt;
 * ID-Austria-artiges Skript-Backend mit einer Anmeldung, Bestätigung je Dokument mit eigenem Vergleichswert,
 * Netzfehler mit Pause, „Erneut versuchen“, „Dokument überspringen“, Zusammenfassung). Geprüft wird, dass jeder
 * Zustand zur Stapelübersicht gehört, die Karte Dokument und Zähler nennt, die Pause die richtigen Aktionen
 * anbietet, die Zusammenfassung alle Einträge zählt und kein Zustand von sich aus eine Sitzung anstößt (F-54).
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { UiEvent, ViewState } from '../core/types';
import { expectNoAxeViolations } from '../test/a11y';
import { renderApp } from '../test/render';
import { mainView } from './App';

const FIXTURE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../core/fixtures/real-core-batch-views.json');
const states = JSON.parse(readFileSync(FIXTURE, 'utf8')) as Record<string, ViewState>;

function state(name: string): ViewState {
  const v = states[name];
  if (!v) throw new Error(`Zustand ${name} fehlt in real-core-batch-views.json`);
  return v;
}

/** Ereignisse, die eine Backend-Sitzung öffnen können (Invariante A1, §3.2). */
const SESSION_EVENTS: UiEvent['type'][] = ['sign_with', 'choose_backend', 'retry', 'resume_signature', 'start_batch', 'confirm_position', 'choose_invisible'];

function sessionEvents(dispatched: UiEvent[]): UiEvent[] {
  return dispatched.filter((e) => SESSION_EVENTS.includes(e.type));
}

/** Zeile der Stapelliste (auch hinter der modalen Karte). */
function row(name: string): HTMLElement {
  const list = screen.getByRole('list', { name: 'Dokumente im Stapel', hidden: true });
  const r = within(list)
    .getAllByRole('listitem', { hidden: true })
    .find((x) => within(x).queryByText(name));
  if (!r) throw new Error(`Zeile ${name} fehlt`);
  return r;
}

function refValue(dialog: HTMLElement): string {
  return Array.from(dialog.querySelectorAll('.refvalue-char'))
    .map((c) => c.textContent)
    .join('');
}

describe('Stapel: Zustände des echten Kerns', () => {
  it('jeder Zustand gehört zur Stapelübersicht (die Karte liegt über der Liste); geparkt zur Startansicht', () => {
    for (const [name, v] of Object.entries(states)) expect([name, mainView(v)]).toEqual([name, name === 'batch_parked' ? 'start' : 'batch']);
  });

  it('Zur Startseite (Nachtrag 10): der Stapel ist „In Bearbeitung“, „Schließen“ fragt bei mehreren Dokumenten nach', async () => {
    const { mock, user } = renderApp({ view: state('batch_parked') });
    await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' });
    const card = screen.getByRole('region', { name: 'In Bearbeitung' });
    expect(within(card).getByRole('heading', { level: 2, name: 'Stapel · 4 Dokumente' })).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: 'Dokumente im Stapel' })).not.toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Schließen' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Offene Arbeit schließen?' });
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    await user.click(within(card).getByRole('button', { name: 'Weiter bearbeiten' }));
    expect(mock.dispatched).toEqual([{ type: 'resume_parked' }]);
    expect(sessionEvents(mock.dispatched)).toEqual([]);
  });

  it('kein Zustand stößt von sich aus eine Sitzung an (F-54)', async () => {
    for (const v of Object.values(states)) {
      const { mock, unmount } = renderApp({ view: v });
      await screen.findByRole('main');
      await new Promise((r) => setTimeout(r, 200));
      expect(sessionEvents(mock.dispatched)).toEqual([]);
      unmount();
    }
  });

  it('Übersicht (E2.1): Seiten, Größe, Status, abgelehnte Datei, Speicherort; „Alle signieren“ ohne Signiermittel öffnet das Menü', async () => {
    const { mock, user } = renderApp({ view: state('batch_overview') });
    expect(await screen.findByRole('heading', { level: 1, name: '4 Dokumente' })).toBeInTheDocument();
    for (const name of ['Vertrag.pdf', 'Antrag.pdf', 'Beilage.pdf']) {
      expect(within(row(name)).getByText('Bereit')).toBeInTheDocument();
      expect(within(row(name)).getByText(/^1 Seite · /)).toBeInTheDocument();
    }
    expect(within(row('Geheim.pdf')).getByText('Abgelehnt')).toBeInTheDocument();
    expect(within(row('Geheim.pdf')).getByText('Mit Kennwort geschützt')).toBeInTheDocument();
    expect(within(row('Geheim.pdf')).getByRole('button', { name: /Geheim\.pdf/ })).toHaveTextContent('Entfernen');
    expect(screen.getByTestId('batch-output')).toHaveTextContent('Speichern in /Users/test/Dokumente');
    const signAll = screen.getByRole('button', { name: 'Alle signieren (3)' });
    expect(signAll).toBeEnabled();
    await expectNoAxeViolations();
    // ohne gewähltes Signiermittel: Menü des Konto-Chips, nichts startet (§8 F12, A1)
    expect(state('batch_overview').signers.selected).toBeNull();
    await user.click(signAll);
    expect(await screen.findByRole('menu')).toBeInTheDocument();
    expect(sessionEvents(mock.dispatched)).toEqual([]);
  });

  it('Schätzung (E2.1) und Start (E2.2): „3 Dokumente · 1 Anmeldung · 3 Bestätigungen (ggf. +1 für das Zertifikat)“, start_batch mit genau den signierbaren', async () => {
    const v = state('batch_overview_estimate');
    const { mock, user } = renderApp({ view: v });
    expect(await screen.findByTestId('batch-estimate')).toHaveTextContent('Voraussichtlich: 3 Dokumente · 1 Anmeldung · 3 Bestätigungen (ggf. +1 für das Zertifikat)');
    expect(screen.getByTestId('privacy-note')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const docs = v.batch!.items.filter((i) => i.name !== 'Geheim.pdf').map((i) => i.doc);
    expect(sessionEvents(mock.dispatched)).toEqual([{ type: 'start_batch', backend: 'atrust_native', docs, output_dir: null, on_existing: 'keep_both' }]);
  });

  it('„Für diesen Stapel automatisch positionieren“ (Nachtrag 8): Schalter an, alle „Bereit“, startet nichts; axe', async () => {
    const v = state('batch_auto_once');
    expect(v.batch?.auto_position_once).toBe(true);
    const { mock, user } = renderApp({ view: v });
    const toggle = await screen.findByRole('switch', { name: 'Für diesen Stapel automatisch positionieren' });
    expect(toggle).toBeChecked();
    expect(within(row('Vertrag.pdf')).getByText('Bereit')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(toggle);
    expect(mock.dispatched.at(-1)).toEqual({ type: 'set_batch_auto_position', enabled: false, replace_manual: false });
    expect(sessionEvents(mock.dispatched)).toEqual([]);
  });

  it('Lauf (E2.2–E2.4): eine Anmeldung, je Dokument „Bestätigung k von n · Name“ mit eigenem Vergleichswert, Pause Verbindung, Fortschritt in der Liste', async () => {
    const { mock, user } = renderApp({ view: state('batch_login') });
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(within(dialog).getByText('3 Dokumente')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Signatur-Passwort')).toBeInTheDocument();
    expect(within(row('Vertrag.pdf')).getByText('Wird signiert')).toBeInTheDocument();
    expect(within(row('Antrag.pdf')).getByText('Wartet')).toBeInTheDocument();
    await expectNoAxeViolations();

    act(() => mock.setView(state('batch_confirm_1')));
    expect(await within(dialog).findByText('Bestätigung 1 von 3 · Vertrag.pdf')).toBeInTheDocument();
    expect(within(dialog).queryByLabelText('Signatur-Passwort')).not.toBeInTheDocument();
    expect(within(dialog).getByText('0 von 3')).toBeInTheDocument();
    expect(screen.getByText('0 von 3 signiert')).toBeInTheDocument();
    const first = refValue(dialog);
    expect(first).toBe('V1');
    expect(within(dialog).getByRole('button', { name: 'Dokument überspringen' })).toBeInTheDocument();
    await expectNoAxeViolations();

    act(() => mock.setView(state('batch_paused_connection')));
    const pause = await within(dialog).findByTestId('batch-pause');
    expect(pause).toHaveAttribute('data-scope', 'session');
    expect(within(pause).queryByText(/neue Anmeldung ist möglich/)).not.toBeInTheDocument();
    expect(within(pause).getByRole('button', { name: 'Überspringen' })).toBeInTheDocument();
    expect(within(pause).getByRole('button', { name: 'Stapel abbrechen' })).toBeInTheDocument();
    expect(within(row('Vertrag.pdf')).getByText('Signiert')).toBeInTheDocument();
    // die Pause entscheidet nie von selbst
    expect(sessionEvents(mock.dispatched)).toEqual([]);
    await expectNoAxeViolations();
    await user.click(within(pause).getByRole('button', { name: 'Erneut versuchen' }));
    expect(sessionEvents(mock.dispatched)).toEqual([{ type: 'retry' }]);

    act(() => mock.setView(state('batch_confirm_2')));
    expect(await within(dialog).findByText('Bestätigung 2 von 3 · Antrag.pdf')).toBeInTheDocument();
    expect(within(dialog).queryByLabelText('Signatur-Passwort')).not.toBeInTheDocument();
    expect(within(dialog).getByText('1 von 3')).toBeInTheDocument();
    expect(refValue(dialog)).toBe('V3');
    expect(refValue(dialog)).not.toBe(first);
    await user.click(within(dialog).getByRole('button', { name: 'Dokument überspringen' }));
    expect(mock.dispatched.filter((e) => e.type === 'skip_batch_document')).toHaveLength(1);
  });

  it('Zusammenfassung (E2.5): „2 von 4 signiert, 1 übersprungen, 1 abgelehnt“ mit Speicherort und „Im Ordner zeigen“', async () => {
    const { mock, user } = renderApp({ view: state('batch_summary') });
    expect(await screen.findByText('Stapel abgeschlossen')).toBeInTheDocument();
    expect(screen.getByText('2 von 4 signiert, 1 übersprungen, 1 abgelehnt')).toBeInTheDocument();
    expect(screen.getByText('Gespeichert in /Users/test/Dokumente.')).toBeInTheDocument();
    expect(within(row('Antrag.pdf')).getByText('Übersprungen')).toBeInTheDocument();
    expect(within(row('Beilage.pdf')).getByText('Signiert')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: 'Im Ordner zeigen' }));
    const beilage = state('batch_summary').batch!.items.find((i) => i.name === 'Beilage.pdf')!.doc;
    await waitFor(() => expect(mock.nativeCalls).toContain(`revealOutput:${beilage}`));
    expect(sessionEvents(mock.dispatched)).toEqual([]);
  });
});
