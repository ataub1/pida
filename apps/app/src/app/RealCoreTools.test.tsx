/**
 * Integration K4 × B4 (ADR 0004 §6.2 Punkt 4, Abnahme E4 Punkte 1, 2, 5, 6; Nachtrag 6): Die Oberfläche rendert
 * **Zustände des echten Kerns** für Prüfmodus und Ersteinrichtung – `core/fixtures/real-core-tools-views.json`
 * entsteht im Rust-Test `crates/core-api/tests/verify.rs::real_core_tools_views_for_the_ui` (Prüfung der
 * Dateien aus `testdata/verify/*`, Ersteinrichtung frisch, mit Übernahmeangebot, nach der Übernahme, fertig).
 * Geprüft werden Gesamtergebnis und Status je Signatur, Zeit als Angabe des Signierenden, QES „laut
 * Zertifikat“, SHA-1-Hinweis, spätere Revisionen, das Rechteck des Signaturbilds auf der gedrehten Seite, der
 * Wortlaut F3 (nie „gültig“), die Übernahmeliste mit den Details des Kerns und dass kein Zustand von sich aus
 * eine Sitzung anstößt (F-54).
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

/** „gültig“, „rechtsgültig“, „ungültig“ als eigenes Wort (nicht „Gültigkeitsdauer“ des Zertifikats). */
const VALID_WORD = /(^|[^a-zäöüß])(rechts|un)?gültig($|[^a-zäöüß])/i;

async function renderState(name: string) {
  const r = renderApp({ view: state(name) });
  await screen.findByRole('main');
  return r;
}

describe('Prüfmodus: Zustände des echten Kerns', () => {
  it('jeder Zustand gehört zur erwarteten Ansicht; keiner stößt eine Sitzung an (F-54)', async () => {
    for (const [name, v] of Object.entries(states)) {
      expect([name, mainView(v)]).toEqual([name, name.startsWith('verify_') ? 'verify' : 'start']);
      const { mock, unmount } = renderApp({ view: v });
      await screen.findByRole('main');
      await new Promise((r) => setTimeout(r, 50));
      expect([name, mock.dispatched.filter((e) => SESSION_EVENTS.includes(e.type))]).toEqual([name, []]);
      unmount();
    }
  });

  it.each([
    ['verify_all_intact', 'Die Signatur ist unverändert und kryptografisch intakt', ['Intakt']],
    ['verify_two_signatures', 'Alle 2 Signaturen sind unverändert und kryptografisch intakt', ['Intakt', 'Intakt']],
    ['verify_some_broken', 'Achtung: Eine Signatur ist nicht intakt', ['Verändert']],
    ['verify_some_not_verifiable', 'Nicht alle Signaturen konnten geprüft werden', ['Nicht prüfbar']],
    ['verify_modified_after_last_signature', 'Nach der letzten Signatur ergänzt oder geändert', ['Intakt']],
    ['verify_no_signatures', 'Dieses Dokument enthält keine Signatur', []],
    ['verify_sha1_warning', 'Die Signatur ist unverändert und kryptografisch intakt', ['Intakt']],
  ])('%s: Gesamtergebnis und Status je Signatur, Wortlaut F3, axe', async (name, summary, badges) => {
    await renderState(name);
    const v = state(name).verify;
    if (!v) throw new Error('verify');
    expect(screen.getByRole('heading', { level: 1, name: v.file_name })).toBeInTheDocument();
    expect(screen.getByTestId('vf-summary')).toHaveTextContent(summary);
    const cards = document.querySelectorAll('.sig-card');
    expect([...cards].map((c) => c.querySelector('.badge')?.textContent)).toEqual(badges);
    // Zeit nur als Angabe des Signierenden, Hinweis auf die vollständige Prüfung, nie „gültig“
    for (const c of cards) expect(c).toHaveTextContent(/Signiert am .+ \(Angabe des Signierenden\)/);
    expect(screen.getByRole('button', { name: /Vollständige Prüfung bei signaturpruefung\.gv\.at/ })).toBeInTheDocument();
    expect(document.body.textContent ?? '').not.toMatch(VALID_WORD);
    expect(document.body.textContent ?? '').not.toMatch(/rechtsgültig/i);
    await expectNoAxeViolations();
  });

  it('Details aus dem echten Zertifikat; spätere Revisionen benannt; SHA-1-Hinweis; verletzt mit Code', async () => {
    const two = await renderState('verify_two_signatures');
    const [first, second] = [...document.querySelectorAll<HTMLElement>('.sig-card')];
    if (!first || !second) throw new Error('zwei Karten');
    expect(first).toHaveTextContent('PDF-Over NG Test RSA (NUR TEST)');
    expect(first).toHaveTextContent('Danach eine weitere Revision, davon eine mit Signatur');
    expect(second).toHaveTextContent('PDF-Over NG Test ECDSA P-256 (NUR TEST)');
    expect(second).toHaveTextContent('Dokument seit dieser Signatur unverändert');
    expect(within(second).getByText('Nicht qualifiziert (laut Zertifikat)')).toBeInTheDocument();
    expect(second).toHaveTextContent('ECDSA P-256 · SHA-256');
    expect(second).toHaveTextContent('Nicht geprüft');
    two.unmount();

    const sha1 = await renderState('verify_sha1_warning');
    expect(screen.getByText(/veraltete Hashverfahren SHA-1/)).toBeInTheDocument();
    sha1.unmount();
    const modified = await renderState('verify_modified_after_last_signature');
    expect(document.querySelector('.sig-card')).toHaveTextContent('Danach eine weitere Revision, ohne weitere Signatur');
    modified.unmount();
    await renderState('verify_some_broken');
    expect(screen.getAllByText('cms-digest-mismatch').length).toBeGreaterThan(0);
  });

  it('QES-Testzertifikat: „Qualifizierte elektronische Signatur laut Zertifikat“', async () => {
    await renderState('verify_qualified');
    expect(screen.getByText('Qualifizierte elektronische Signatur laut Zertifikat')).toBeInTheDocument();
    expect(screen.getByText('Angabe aus dem Zertifikat, nicht gegen die EU-Vertrauensliste geprüft.')).toBeInTheDocument();
  });

  it('gedrehte Seite (/Rotate 90): Querformat, Rechteck des Signaturbilds aus PAGE_ABS liegt auf der sichtbaren Seite', async () => {
    const { user, mock } = await renderState('verify_rotated');
    const sig = state('verify_rotated').verify?.signatures[0];
    const rect = sig?.appearance?.rect;
    if (!rect) throw new Error('sichtbar');
    const page = document.querySelector<HTMLElement>('.vf-page');
    if (!page) throw new Error('Seite');
    const scale = parseFloat(page.style.width) / 792;
    expect(parseFloat(page.style.height)).toBeCloseTo(612 * scale, 3);
    const mark = screen.getByTestId('vf-mark-1');
    expect(parseFloat(mark.style.left)).toBeCloseTo(rect.left * scale, 3);
    expect(parseFloat(mark.style.top)).toBeCloseTo((612 - rect.top) * scale, 3);
    expect(parseFloat(mark.style.width)).toBeCloseTo((rect.right - rect.left) * scale, 3);
    expect(parseFloat(mark.style.left) + parseFloat(mark.style.width)).toBeLessThanOrEqual(parseFloat(page.style.width));
    expect(parseFloat(mark.style.top) + parseFloat(mark.style.height)).toBeLessThanOrEqual(parseFloat(page.style.height));
    await user.click(screen.getByRole('button', { name: 'Auf der Seite zeigen' }));
    expect(mark).toHaveClass('is-active');
    // Kachel des Prüfdokuments (eigene DocId des Kerns)
    expect(mock.dispatched.some((e) => e.type === 'open_files')).toBe(false);
    await expectNoAxeViolations();
  });

  it('unsichtbare Signatur ohne „Auf der Seite zeigen“; nicht lesbare Datei mit eigenem Zustand', async () => {
    const inv = await renderState('verify_invisible');
    expect(screen.queryByRole('button', { name: 'Auf der Seite zeigen' })).not.toBeInTheDocument();
    inv.unmount();
    await renderState('verify_failed');
    expect(screen.getByRole('alert')).toHaveTextContent('Die Datei konnte nicht geprüft werden');
    expect(screen.queryByRole('button', { name: 'Dieses Dokument signieren' })).not.toBeInTheDocument();
    await expectNoAxeViolations();
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
