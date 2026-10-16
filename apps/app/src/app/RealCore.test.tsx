/**
 * Integration K1 × A1 (ADR 0004 §6.2 Punkt 4, Abnahme E1 Punkte 1, 2, 5, 6, 7): Die Oberfläche rendert
 * **Zustände des echten Kerns** statt des Mocks – `core/fixtures/real-core-views.json` entsteht im Rust-Test
 * `crates/core-api/tests/workspace.rs::real_core_views_for_the_ui` (Keystore mit echter Engine; ID-Austria-artiges
 * Skript-Backend mit Anmeldung, Netzfehler, „Erneut versuchen“, SMS-TAN; Abbruch). Geprüft wird, dass die
 * Seitenansicht vom Bearbeiten bis „Fertig“ eingehängt bleibt, die Karte jeden Schritt zeigt, „Fertig“ den
 * Wortlaut F3 trägt und kein Zustand von sich aus eine Sitzung anstößt (F-54).
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

const FIXTURE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../core/fixtures/real-core-views.json');
const states = JSON.parse(readFileSync(FIXTURE, 'utf8')) as Record<string, ViewState>;

function state(name: string): ViewState {
  const v = states[name];
  if (!v) throw new Error(`Zustand ${name} fehlt in real-core-views.json`);
  return v;
}

/** Ereignisse, die eine Backend-Sitzung öffnen können (Invariante A1, §3.2). */
const SESSION_EVENTS: UiEvent['type'][] = ['sign_with', 'choose_backend', 'retry', 'resume_signature', 'start_batch', 'confirm_position', 'choose_invisible'];

function sessionEvents(dispatched: UiEvent[]): UiEvent[] {
  return dispatched.filter((e) => SESSION_EVENTS.includes(e.type));
}

function sticker() {
  return screen.getByRole('application', { name: 'Signaturblock' });
}

describe('Zustände des echten Kerns', () => {
  it('jeder Zustand gehört zur erwarteten Ansicht (Seitenansicht bis „Fertig“)', () => {
    const expected: Record<string, string> = {
      keystore_editing: 'workspace',
      keystore_signing: 'workspace',
      keystore_done: 'done',
      atrust_editing: 'workspace',
      atrust_login: 'workspace',
      atrust_network_error: 'workspace',
      atrust_tan: 'workspace',
      atrust_done: 'done',
      atrust_cancelled: 'workspace',
      doc_pdfa_detected: 'workspace',
      doc_pdfa_forced: 'workspace',
      doc_field_auto: 'workspace',
      doc_qr_auto: 'workspace',
      doc_auto_locked: 'workspace',
      settings_open: 'workspace',
      settings_partial: 'workspace',
    };
    for (const [name, v] of Object.entries(states)) expect([name, mainView(v)]).toEqual([name, expected[name]]);
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

  it('Keystore: automatisch platziert → „Signieren“ (ohne Standard: Menü) → Karte → Fertig, ohne Ansichtswechsel', async () => {
    const editing = state('keystore_editing');
    const { mock, user } = renderApp({ view: editing });
    const s = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.getByRole('banner')).toHaveTextContent('Vertrag.pdf');
    expect(screen.queryByRole('heading', { name: 'Was möchten Sie signieren?' })).not.toBeInTheDocument();
    // Startlage aus dem Kern (PDF-AS-Regel) in px: Sticker steht im Rechteck `document.sticker`
    expect(s.querySelector('img')).not.toBeNull();
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
    await expectNoAxeViolations();
    // ohne Standard-Signiermittel öffnet „Signieren“ das Menü; erst die Wahl dort signiert (§8 F12)
    expect(editing.signers.selected).toBeNull();
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    expect(sessionEvents(mock.dispatched)).toEqual([]);
    const menu = await screen.findByRole('menu');
    await user.click(within(menu).getByRole('menuitem', { name: /Keystore|PKCS/ }));
    expect(sessionEvents(mock.dispatched)).toEqual([{ type: 'sign_with', backend: 'keystore' }]);

    const workspace = document.querySelector('.workspace');
    if (states.keystore_signing) {
      act(() => mock.setView(state('keystore_signing')));
      const dialog = await screen.findByRole('dialog', { name: /^Signieren mit/ });
      expect(dialog).toHaveTextContent('Vertrag.pdf');
      expect(document.querySelector('.workspace')).toBe(workspace);
      expect(s.isConnected).toBe(true);
    }
    act(() => mock.setView(state('keystore_done')));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
    expect(screen.getByText('Signatur erstellt und geprüft (Selbstprüfung)')).toBeInTheDocument();
    expect(screen.getByText('Fortgeschritten laut Zertifikat')).toBeInTheDocument();
    expect(screen.getByText('Vertrag_signed.pdf')).toBeInTheDocument();
    expect(screen.getByText('Gespeichert in /Users/test/Dokumente')).toBeInTheDocument();
    expect(screen.getByText(/^Signiert von PDF-Over NG Test RSA \(NUR TEST\) · /)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Vollständige Prüfung bei signaturpruefung\.gv\.at/ })).toBeInTheDocument();
    expect(document.body.textContent ?? '').not.toMatch(/gültig/i);
    expect(screen.getByRole('button', { name: 'Im Ordner zeigen' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('ID Austria (Skript): Anmeldung, Netzfehler mit „Erneut versuchen“, SMS-TAN, Fertig – Karte über der Seitenansicht', async () => {
    const { mock, user } = renderApp({ view: state('atrust_editing') });
    await screen.findByRole('application', { name: 'Signaturblock' });
    // Kurzhinweis Datenschutz am Konto-Chip (document_leaves_device)
    expect(screen.getByTestId('privacy-note')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    expect(sessionEvents(mock.dispatched)).toEqual([{ type: 'sign_with', backend: 'atrust_native' }]);
    const workspace = document.querySelector('.workspace');

    act(() => mock.setView(state('atrust_login')));
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(within(dialog).getByLabelText('Signatur-Passwort')).toBeInTheDocument();
    expect(within(dialog).getByText('Das Dokument wird zum Signieren an A-Trust übermittelt.', { exact: false })).toBeInTheDocument();
    expect(document.querySelector('.workspace')).toBe(workspace);
    await expectNoAxeViolations();

    act(() => mock.setView(state('atrust_network_error')));
    const err = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(await within(err).findByRole('heading', { name: 'Der Signaturdienst ist gerade nicht erreichbar' })).toBeInTheDocument();
    expect(document.querySelector('.workspace')).toBe(workspace);
    await user.click(within(err).getByRole('button', { name: 'Erneut versuchen' }));
    expect(lastEvent(mock)).toEqual({ type: 'retry' });

    act(() => mock.setView(state('atrust_tan')));
    const tan = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(await within(tan).findByLabelText('TAN aus der SMS')).toBeInTheDocument();
    // Anmeldung der Sitzung gilt weiter: Stepper „Anmelden“ erledigt
    const steps = within(tan).getAllByRole('listitem');
    expect(steps[0]).toHaveClass('is-done');
    expect(document.querySelector('.workspace')).toBe(workspace);

    act(() => mock.setView(state('atrust_done')));
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' })).toBeInTheDocument();
    expect(screen.getByText('Qualifiziert laut Zertifikat')).toBeInTheDocument();
    expect(screen.getByText('Antrag_signed.pdf')).toBeInTheDocument();
    expect(sessionEvents(mock.dispatched)).toEqual([{ type: 'sign_with', backend: 'atrust_native' }, { type: 'retry' }]);
  });

  it('Abbruch: zurück im Arbeitsbereich mit dem Hinweis des Kerns, keine Karte', async () => {
    renderApp({ view: state('atrust_cancelled') });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(sticker()).toBeInTheDocument();
    expect(await screen.findByText('Signieren abgebrochen – es wurde nichts signiert.')).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('Einstellungen › Dokument (E3): Hinweise aus `document.detection` des Kerns, Sprechblase nennt das Ziel', async () => {
    const inspector = () => screen.getByRole('complementary', { name: 'Signatur' });
    const cases: [string, RegExp | string | null, string | null, string][] = [
      ['doc_pdfa_detected', 'PDF/A-Dokument erkannt – wird PDF/A-konform signiert', null, 'Signaturblock automatisch auf freier Fläche platziert'],
      ['doc_pdfa_forced', 'PDF/A-kompatible Signatur (erzwungen)', null, 'Signaturblock automatisch auf freier Fläche platziert'],
      ['doc_field_auto', null, 'Signaturfeld gefunden – Signatur im Feld platziert', 'Signaturblock automatisch im Signaturfeld platziert'],
      ['doc_qr_auto', null, 'QR-Platzhalter gefunden – Signatur dort platziert', 'Signaturblock automatisch im QR-Platzhalter platziert'],
    ];
    for (const [name, pdfa, target, coach] of cases) {
      if (!states[name]) continue; // QR nur mit PDFium erzeugt
      const { unmount } = renderApp({ view: state(name) });
      await screen.findByRole('application', { name: 'Signaturblock' });
      if (pdfa) expect(within(inspector()).getByText(pdfa)).toBeInTheDocument();
      else expect(within(inspector()).queryByText(/PDF\/A/)).not.toBeInTheDocument();
      if (target) expect(within(inspector()).getByText(target)).toBeInTheDocument();
      else expect(within(inspector()).queryByTestId('detect-note')).not.toBeInTheDocument();
      expect(screen.getByText(coach)).toBeInTheDocument();
      // automatisch im Feld: keine Hinweisleiste „Signaturfeld gefunden:“ für das belegte Feld
      if (name === 'doc_field_auto') expect(screen.queryByText('Signaturfeld gefunden:')).not.toBeInTheDocument();
      await expectNoAxeViolations();
      unmount();
    }
  });

  it('Automatische Positionierung (Nachtrag 8) mit dem echten Kern: gesperrt, Fokus auf „Signieren“, „Diesmal selbst platzieren“', async () => {
    const v = state('doc_auto_locked');
    expect(v.document?.placement_locked).toBe(true);
    const { mock, user } = renderApp({ view: v });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(sticker()).toHaveAttribute('aria-disabled', 'true');
    expect(sticker()).toHaveAttribute('aria-roledescription', 'automatisch platzierter Signaturblock');
    const inspector = screen.getByRole('complementary', { name: 'Signatur' });
    expect(within(inspector).getByText('Automatisch (laut Einstellung)')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', { name: /^Signieren/ })).toHaveFocus());
    expect(within(inspector).getByRole('switch', { name: 'Unsichtbar signieren' })).not.toBeDisabled();
    await expectNoAxeViolations();
    await user.click(within(inspector).getByRole('button', { name: 'Diesmal selbst platzieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'unlock_placement' });
    expect(sessionEvents(mock.dispatched)).toEqual([]);
  });

  it('Einstellungen „sofort wirksam“ (E3) mit dem echten Kern: Teilübernahme mit Feldfehler, Werte und Beispiel des Kerns', async () => {
    const nav = (dialog: HTMLElement, name: string) => within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name });
    const { user, mock } = renderApp({ view: state('settings_partial') });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(within(dialog).queryByText(/werden nicht gespeichert/)).not.toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Amtssignatur/ })).toBeChecked();
    await user.click(nav(dialog, 'Dokument'));
    expect(within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' })).toBeChecked();
    expect(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' })).toBeChecked();
    expect(within(dialog).getByRole('radiogroup', { name: 'Suchen nach' })).not.toHaveAttribute('aria-disabled');
    await user.click(nav(dialog, 'Erweitert'));
    expect(within(dialog).getByLabelText('Host')).toHaveValue('proxy.local');
    expect(within(dialog).getByText('Der Port muss zwischen 1 und 65535 liegen.')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Port')).toHaveAttribute('aria-invalid', 'true');
    await user.click(nav(dialog, 'Speichern'));
    expect(within(dialog).getByText(/Vertrag_signed\.pdf/)).toBeInTheDocument();
    await expectNoAxeViolations();
    // nichts davon stößt eine Sitzung an (F-54)
    expect(sessionEvents(mock.dispatched)).toEqual([]);
  });
});
