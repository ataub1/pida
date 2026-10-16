/**
 * Integration K4 × B4 (ADR 0004 §6.2 Punkt 4, Abnahme E4 Punkte 5, 6): Die Oberfläche rendert **Zustände des
 * echten Kerns** für die Ersteinrichtung – `core/fixtures/real-core-tools-views.json` entsteht im Rust-Test
 * `crates/core-api/tests/first_run.rs::real_core_tools_views_for_the_ui` (frisch, mit Übernahmeangebot, nach der
 * Übernahme, fertig). Geprüft werden die Übernahmeliste mit den Details des Kerns und dass kein Zustand von sich aus
 * eine Sitzung anstößt (F-54). Die Zustände des Prüfmodus sind mit ihm entfallen (ADR 0004 Nachtrag 15).
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { UiEvent, ViewState } from '../core/types';
import { expectNoAxeViolations } from '../test/a11y';
import { lastEvent, renderApp } from '../test/render';
import { mainView } from './App';

const FIXTURE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../core/fixtures/real-core-tools-views.json');
const states = JSON.parse(readFileSync(FIXTURE, 'utf8')) as Record<string, ViewState>;

function state(name: string): ViewState {
  const v = states[name];
  if (!v) throw new Error(`Zustand ${name} fehlt in real-core-tools-views.json`);
  return v;
}

/** Ereignisse, die eine Backend-Sitzung öffnen können (Invariante A1, §3.2). */
const SESSION_EVENTS: UiEvent['type'][] = ['sign_with', 'choose_backend', 'retry', 'resume_signature', 'start_batch', 'confirm_position', 'choose_invisible'];

async function renderState(name: string) {
  const r = renderApp({ view: state(name) });
  await screen.findByRole('main');
  return r;
}

describe('Zustände des echten Kerns: Ansicht und keine Sitzung', () => {
  it('jeder Zustand zeigt die Startansicht (kein Prüfmodus mehr); keiner stößt eine Sitzung an (F-54)', async () => {
    expect(Object.keys(states).sort()).toEqual(['first_run_done', 'first_run_fresh', 'first_run_imported', 'first_run_migration']);
    for (const [name, v] of Object.entries(states)) {
      expect([name, mainView(v)]).toEqual([name, 'start']);
      expect('verify' in v).toBe(false);
      const { mock, unmount } = renderApp({ view: v });
      await screen.findByRole('main');
      await new Promise((r) => setTimeout(r, 50));
      expect([name, mock.dispatched.filter((e) => SESSION_EVENTS.includes(e.type))]).toEqual([name, []]);
      unmount();
    }
  });
});

describe('Ersteinrichtung: Zustände des echten Kerns', () => {
  it('mit PDF-Over-Einstellungen: Liste mit den Details des Kerns → Übernehmen → Schritte 2/3 mit den Werten → Fertig', async () => {
    const { mock, user } = await renderState('first_run_migration');
    const dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expect(dialog).toHaveTextContent('Schritt 1 von 3');
    const offer = within(dialog).getByTestId('migration-offer');
    for (const text of [
      'Signaturbild (Profil, Sprache, Hinweistext)',
      'Benutzername für ID Austria: +43 664 ···567',
      'Speicherort und Dateiname: /Users/test/home/Dokumente/Signiert',
      'Keystore: test-rsa2048.p12',
      'Proxy: proxy.example.test:3128',
      'Platzhaltersuche',
      'Sprache: Deutsch',
    ]) {
      expect(within(offer).getByText(text)).toBeInTheDocument();
    }
    // eigene Bildmarke der Altkonfiguration steht in der Liste – dann kein Hinweis „PIDA-Siegel bleibt“
    expect(within(offer).getByText(/^Bildmarke: /)).toBeInTheDocument();
    expect(within(offer).queryByTestId('seal-stays')).not.toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(offer).getByRole('button', { name: 'Übernehmen' }));
    expect(lastEvent(mock)).toEqual({ type: 'accept_migration', store_secrets: false });

    act(() => mock.setView(state('first_run_imported')));
    const step2 = await screen.findByRole('heading', { name: 'Womit signieren Sie meistens?' });
    expect(step2).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /^ID Austria/ })).toBeChecked();
    await user.click(screen.getByRole('button', { name: 'Weiter' }));
    await screen.findByRole('heading', { name: 'So sieht Ihre Signatur aus' });
    const img = within(screen.getByTestId('ob-preview')).getByRole('img');
    expect(img.getAttribute('src')).toContain('2');
    await user.click(screen.getByRole('button', { name: 'Los geht’s' }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'finish_first_run' }));
    expect(mock.dispatched.some((e) => e.type === 'decline_migration')).toBe(false);

    act(() => mock.setView(state('first_run_done')));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Willkommen bei PIDA' })).not.toBeInTheDocument());
    expect(await screen.findByText('Einrichtung abgeschlossen. Sie können alles jederzeit in den Einstellungen ändern.')).toBeInTheDocument();
  });

  it('frisch ohne PDF-Over: direkt das Standard-Signiermittel („Jedes Mal fragen“), „Später einrichten“ = finish_first_run', async () => {
    const { mock, user } = await renderState('first_run_fresh');
    const dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expect(dialog).toHaveTextContent('Schritt 1 von 2');
    expect(within(dialog).getByRole('heading', { name: 'Womit signieren Sie meistens?' })).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /^Jedes Mal fragen/ })).toBeChecked();
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Später einrichten' }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'finish_first_run' }));
    expect(mock.dispatched.some((e) => e.type === 'decline_migration')).toBe(false);
  });
});
