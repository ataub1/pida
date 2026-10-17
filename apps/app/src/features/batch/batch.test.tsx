/**
 * Stapel gegen den Mock-Kern (ADR 0004 §3.8, Abnahme E2, alle [M]-Kriterien): Übersicht mit Miniatur, Seiten,
 * Größe, Status und Schätzung (1), „Alle signieren (3)“ mit **einer** Anmeldung und je Dokument eigener
 * Bestätigung mit eigenem Vergleichswert, Fortschritt k/n in Karte und Liste (2), Überspringen ohne neue
 * Anmeldung und Abbruch (3), Pausen Verbindung/Sitzung/Dokument mit den richtigen Aktionen und nicht
 * wiederholbarer Dokumentfehler (4), Zusammenfassung mit Speicherort und „Im Ordner zeigen“ (5), Einzeldokument
 * öffnen/zurück/einzeln signieren mit Hinweis (6), vorhandene Zieldateien und Ordnerwahl vor dem Start (7),
 * Mobile-Layout; axe-core ohne Verstöße (auch schmal). Invariante A1: Eine Sitzung beginnt nur nach
 * `start_batch` bzw. „Erneut versuchen“.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { defaultSettings } from '../../core/defaults';
import type { MockOptions } from '../../core/mock';
import type { Settings, UiEvent } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { emulateMedia, renderApp } from '../../test/render';
import { batchData, batchItem, batchOverview, IOS, onMobile, signersView } from '../../test/views';

/** ID Austria als Standard (gemerkter Benutzer), Platzhaltersuche an, Ausgabe neben dem Original. */
function settings(extra: Partial<Settings> = {}): Settings {
  const s = defaultSettings();
  return {
    ...s,
    backend: { default: 'id_austria' },
    id_austria: { ...s.id_austria, username: '+43 660 1234123', remember: true },
    output: { ...s.output, folder: '.' },
    placeholder: { search: true, kind: 'marker' },
    ...extra,
  };
}

type User = ReturnType<typeof renderApp>['user'];

const DIR = '/Users/demo/Dokumente';

/** Szenario „Stapel“ (Angebot, Auftragsbestätigung mit QR-Platzhalter, kennwortgeschützter Scan, Vertrag). */
async function openBatch(options: MockOptions & { isMac?: boolean } = {}) {
  const r = renderApp({ scenario: 'batch', settings: settings(), ...options });
  const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
  // Voranalyse nacheinander im Hintergrund
  await waitFor(() => expect(within(list).queryAllByText('Wird geprüft')).toHaveLength(0));
  return { ...r, list };
}

/** Zeilen der Liste (auch hinter der modalen Stapelkarte, die den Rest der Seite für AT ausblendet). */
function rows(list: HTMLElement) {
  return within(list).getAllByRole('listitem', { hidden: true });
}

function row(list: HTMLElement, name: string): HTMLElement {
  const r = rows(list).find((x) => within(x).queryByText(name));
  if (!r) throw new Error(`Zeile ${name} fehlt`);
  return r;
}

async function login(user: User, password = 'geheim123', remember?: boolean) {
  const dialog = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
  await user.type(await within(dialog).findByLabelText('Signatur-Passwort'), password);
  if (remember === false) await user.click(within(dialog).getByRole('checkbox', { name: /Daten merken/ }));
  await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));
  return dialog;
}

/** wartet auf die App-Bestätigung eines Dokuments (Kopf der Stapelkarte) und liefert den Vergleichswert. */
async function confirmation(dialog: HTMLElement, index: number, total: number, name: string): Promise<string> {
  await within(dialog).findByText(`Bestätigung ${index} von ${total} · ${name}`);
  await within(dialog).findByRole('heading', { name: 'Benachrichtigung an die ID-Austria-App gesendet' });
  const chars = dialog.querySelectorAll('.refvalue-char');
  return Array.from(chars)
    .map((c) => c.textContent)
    .join('');
}

const sessions = (events: UiEvent[]) => events.filter((e) => e.type === 'start_batch' || e.type === 'sign_with' || e.type === 'retry' || e.type === 'choose_backend' || e.type === 'resume_signature');

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Stapelübersicht (E2.1)', () => {
  it('vier Dateien: Miniatur, Seiten, Größe, Status (abgelehnt, QR → positioniert), Schätzung, Kopfzeile; axe', async () => {
    const { list, mock } = await openBatch();
    expect(screen.getByRole('heading', { level: 1, name: '4 Dokumente' })).toBeInTheDocument();
    expect(screen.getByRole('banner')).toHaveTextContent('Stapel4 Dokumente');
    expect(rows(list)).toHaveLength(4);
    const offer = row(list, 'Angebot_2026-118.pdf');
    expect(within(offer).getByText('2 Seiten · 96 KB')).toBeInTheDocument();
    expect(within(offer).getByText('Bereit')).toBeInTheDocument();
    expect(within(offer).getByText('automatisch platziert')).toBeInTheDocument();
    expect(offer.querySelector('.batch-thumb img')?.getAttribute('src')).toMatch(/^data:image\/svg\+xml/);
    const qr = row(list, 'Auftragsbestaetigung_4711.pdf');
    expect(within(qr).getByText('Positioniert')).toBeInTheDocument();
    expect(within(qr).getByText('im QR-Platzhalter')).toBeInTheDocument();
    const locked = row(list, 'Lieferschein_Scan.pdf');
    expect(within(locked).getByText('Abgelehnt')).toBeInTheDocument();
    expect(within(locked).getByText('Mit Kennwort geschützt')).toBeInTheDocument();
    expect(within(locked).getByText('1 Seite · 184 KB')).toBeInTheDocument();
    expect(locked.querySelector('.batch-thumb img')).toBeNull();
    // die abgelehnte Zeile lässt sich nicht öffnen
    expect(within(locked).queryByRole('button', { name: /öffnen und positionieren/ })).not.toBeInTheDocument();
    expect(screen.getByTestId('batch-estimate')).toHaveTextContent('Voraussichtlich: 3 Dokumente · 1 Anmeldung · 3 Bestätigungen (ggf. +1 für das Zertifikat)');
    expect(screen.getByTestId('batch-output')).toHaveTextContent('Speichern: jeweils neben dem Original');
    expect(screen.getByRole('button', { name: 'Alle signieren (3)' })).toBeEnabled();
    // Kurzhinweis Datenschutz beim Signiermittel ID Austria (nie blockierend), im Stapel in der Mehrzahl
    expect(screen.getByTestId('privacy-note')).toHaveTextContent('Die Dokumente werden zum Signieren an A-Trust übermittelt.');
    // nichts startet von selbst (A1)
    expect(sessions(mock.dispatched)).toEqual([]);
    await expectNoAxeViolations();
  });

  it('Schätzung: mit gültigem Zertifikats-Cache ohne „+1“, mit gemerktem Passwort „0 Anmeldungen“, Keystore', async () => {
    const { unmount } = renderApp({ view: batchOverview(batchData({ estimate: { backend: 'atrust_native', documents: 3, logins: 1, confirmations: 3, certificate_confirmation_possible: false, large: false } })) });
    expect(await screen.findByTestId('batch-estimate')).toHaveTextContent(/^Voraussichtlich: 3 Dokumente · 1 Anmeldung · 3 Bestätigungen$/);
    unmount();
    const second = renderApp({ view: batchOverview(batchData({ estimate: { backend: 'atrust_native', documents: 3, logins: 0, confirmations: 3, certificate_confirmation_possible: true, large: false } })) });
    expect(await screen.findByTestId('batch-estimate')).toHaveTextContent('3 Dokumente · 0 Anmeldungen · 3 Bestätigungen (ggf. +1 für das Zertifikat)');
    second.unmount();
    renderApp({ view: batchOverview(batchData({ estimate: { backend: 'keystore', documents: 3, logins: 1, confirmations: 3, certificate_confirmation_possible: false, large: false } }), { signers: signersView({ selected: 'keystore' }) }) });
    expect(await screen.findByTestId('batch-estimate')).toHaveTextContent('3 Dokumente · Keystore-Passwort einmal');
    expect(screen.queryByTestId('privacy-note')).not.toBeInTheDocument();
  });

  it('Mock: Schätzung nach Anmeldung mit „Daten merken“ – 0 Anmeldungen; Zertifikats-Cache ohne „+1“', async () => {
    const { list, user, mock } = await openBatch({ settings: settings({ certificate_cache: { enabled: true } }) });
    expect(screen.getByTestId('batch-estimate')).toHaveTextContent(/1 Anmeldung · 3 Bestätigungen$/);
    // ein Dokument einzeln signieren (Anmeldung mit „Daten merken“)
    await user.click(within(row(list, 'Vertrag.pdf')).getByRole('button', { name: '„Vertrag.pdf“ öffnen und positionieren' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    await login(user);
    await screen.findByRole('heading', { name: 'Benachrichtigung an die ID-Austria-App gesendet' });
    act(() => mock.completeConfirmation());
    await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(await screen.findByTestId('batch-estimate')).toHaveTextContent('2 Dokumente · 0 Anmeldungen · 2 Bestätigungen');
  });

  it('Englisch: Status, Schätzung und Speicherort', async () => {
    renderApp({ view: batchOverview(batchData(), { language: 'en' }) });
    expect(await screen.findByRole('heading', { level: 1, name: '4 documents' })).toBeInTheDocument();
    expect(screen.getByTestId('batch-estimate')).toHaveTextContent('Expected: 3 documents · 1 login · 3 confirmations (possibly +1 for the certificate)');
    expect(screen.getByTestId('batch-output')).toHaveTextContent('Save: next to each original');
    const list = screen.getByRole('list', { name: 'Documents in the batch' });
    expect(within(row(list, 'Scan.pdf')).getByText('Rejected')).toBeInTheDocument();
    expect(within(row(list, 'Scan.pdf')).getByText('Password-protected')).toBeInTheDocument();
    expect(within(row(list, 'Auftrag.pdf')).getByText('in the QR placeholder')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign all (3)' })).toBeEnabled();
  });

  it('ab 20 Dokumenten mit ID Austria: Hinweis, jedes Dokument einzeln zu bestätigen', async () => {
    renderApp({ view: batchOverview(batchData({ estimate: { backend: 'atrust_native', documents: 24, logins: 1, confirmations: 24, certificate_confirmation_possible: false, large: true } })) });
    expect(await screen.findByText('24 Dokumente: Jedes Dokument wird einzeln in der App bestätigt.')).toBeInTheDocument();
  });

  it('mehrere abgelegte Dateien werden zum Stapel; weitere kommen in die Übersicht', async () => {
    const { mock } = renderApp({ settings: settings() });
    await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' });
    act(() => mock.simulateFileDrag({ type: 'drop', paths: [`${DIR}/Rechnung_1.pdf`, `${DIR}/Rechnung_2.pdf`], x: 1, y: 1 }));
    const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(list).getByText('Rechnung_1.pdf')).toBeInTheDocument();
    expect(within(list).getByText('Rechnung_2.pdf')).toBeInTheDocument();
    act(() => mock.simulateFileDrag({ type: 'drop', paths: [`${DIR}/Rechnung_3.pdf`, `${DIR}/Rechnung_1.pdf`], x: 1, y: 1 }));
    await waitFor(() => expect(rows(list)).toHaveLength(3));
    expect(screen.getByRole('heading', { level: 1, name: '3 Dokumente' })).toBeInTheDocument();
  });

  it('abgelehnte Datei aus dem Stapel entfernen', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(within(row(list, 'Lieferschein_Scan.pdf')).getByRole('button', { name: '„Lieferschein_Scan.pdf“ aus dem Stapel entfernen' }));
    await waitFor(() => expect(rows(list)).toHaveLength(3));
    expect(mock.dispatched.filter((e) => e.type === 'remove_from_batch')).toEqual([{ type: 'remove_from_batch', doc: 3 }]);
    expect(screen.getByRole('heading', { level: 1, name: '3 Dokumente' })).toBeInTheDocument();
  });

  it('jedes noch nicht signierte Dokument lässt sich entfernen, nicht nur abgelehnte', async () => {
    const { list, user, mock } = await openBatch();
    const before = rows(list).length;
    const first = rows(list)[0]!;
    const name = within(first).getAllByText(/\.pdf$/)[0]?.textContent ?? '';
    await user.click(within(first).getByRole('button', { name: `„${name}“ aus dem Stapel entfernen` }));
    await waitFor(() => expect(rows(list)).toHaveLength(before - 1));
    expect(mock.dispatched.filter((e) => e.type === 'remove_from_batch')).toHaveLength(1);
    expect(within(list).queryByText(name)).toBeNull();
  });

  it('Analysewarnung im Stapel: „Bitte prüfen“, einzeln öffnen und bestätigen, zurück → bereit (wie der Kern: ohne eigenen Plan)', async () => {
    const { list, user } = await openBatch({ warnings: ['pdf-usage-rights'] });
    const offer = row(list, 'Angebot_2026-118.pdf');
    expect(within(offer).getByText('Bitte prüfen')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Alle signieren (0)' })).toBeDisabled();
    await user.click(within(offer).getByRole('button', { name: 'Prüfen' }));
    await user.click(await screen.findByRole('button', { name: 'Trotzdem signieren' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zurück zum Stapel' }));
    const back = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    // automatisch platziert, nichts verändert: kein Plan → „Bereit“ (Kern: `BatchItem::settle`)
    expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('Bereit')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Alle signieren (1)' })).toBeEnabled();
  });
});

describe('Stapellauf (E2.2–E2.5)', () => {
  it('„Alle signieren (3)“: eine Login-Maske, je Dokument eine Bestätigung mit Name und eigenem Vergleichswert, Fortschritt k/n in Karte und Liste', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    expect(mock.dispatched.filter((e) => e.type === 'start_batch')).toEqual([{ type: 'start_batch', backend: 'atrust_native', docs: [1, 2, 4], output_dir: null, on_existing: 'keep_both' }]);
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    expect(within(dialog).getByText('3 Dokumente')).toBeInTheDocument();
    const dialogAgain = await login(user);
    expect(dialogAgain).toBe(dialog);
    const ref1 = await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    expect(within(dialog).getByText('0 von 3')).toBeInTheDocument();
    // Liste hinter der Karte: aktuelles Dokument „Wird signiert“, die anderen warten
    expect(within(row(list, 'Angebot_2026-118.pdf')).getByText('Wird signiert')).toBeInTheDocument();
    expect(within(row(list, 'Vertrag.pdf')).getByText('Wartet')).toBeInTheDocument();
    expect(screen.getByText('0 von 3 signiert')).toBeInTheDocument();
    act(() => mock.completeConfirmation());
    const ref2 = await confirmation(dialog, 2, 3, 'Auftragsbestaetigung_4711.pdf');
    // keine zweite Anmeldung (die Sitzung meldet automatisch an)
    expect(within(dialog).queryByLabelText('Signatur-Passwort')).not.toBeInTheDocument();
    expect(within(dialog).getByText('1 von 3')).toBeInTheDocument();
    expect(within(row(list, 'Angebot_2026-118.pdf')).getByText('Signiert')).toBeInTheDocument();
    expect(within(row(list, 'Angebot_2026-118.pdf')).getByText('als Angebot_2026-118_signed.pdf')).toBeInTheDocument();
    expect(screen.getByText('1 von 3 signiert')).toBeInTheDocument();
    act(() => mock.completeConfirmation());
    const ref3 = await confirmation(dialog, 3, 3, 'Vertrag.pdf');
    expect(new Set([ref1, ref2, ref3]).size).toBe(3);
    act(() => mock.completeConfirmation());
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    // Zusammenfassung über alle Einträge, Hinweis des Kerns als Toast
    expect(await screen.findByText('Stapel abgeschlossen')).toBeInTheDocument();
    expect(screen.getByText('3 von 4 signiert, 1 abgelehnt')).toBeInTheDocument();
    expect(screen.getByText('Jeweils neben dem Original gespeichert.')).toBeInTheDocument();
    expect(await screen.findByText('3 von 4 Dokumenten signiert.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Alle signieren (0)' })).toBeDisabled();
    // genau eine Sitzung (A1): nur `start_batch`
    expect(sessions(mock.dispatched).map((e) => e.type)).toEqual(['start_batch']);
    await expectNoAxeViolations();
  });

  it('„Dokument überspringen“ beim 2. Dokument: weiter mit dem 3. ohne neue Anmeldung; Zusammenfassung „2 von 4 signiert, 1 übersprungen, 1 abgelehnt“; Im Ordner zeigen', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    act(() => mock.completeConfirmation());
    await confirmation(dialog, 2, 3, 'Auftragsbestaetigung_4711.pdf');
    await user.click(within(dialog).getByRole('button', { name: 'Dokument überspringen' }));
    await confirmation(dialog, 3, 3, 'Vertrag.pdf');
    expect(within(dialog).queryByLabelText('Signatur-Passwort')).not.toBeInTheDocument();
    // eine schon angezeigte Anfrage kann in der App sichtbar bleiben
    expect(within(dialog).getByText('Eine noch offene Anfrage in der App lehnen Sie bitte ab; vergleichen Sie immer den Vergleichswert.')).toBeInTheDocument();
    expect(within(row(list, 'Auftragsbestaetigung_4711.pdf')).getByText('Übersprungen')).toBeInTheDocument();
    act(() => mock.completeConfirmation());
    expect(await screen.findByText('2 von 4 signiert, 1 übersprungen, 1 abgelehnt')).toBeInTheDocument();
    expect(screen.getByText('Jeweils neben dem Original gespeichert.')).toBeInTheDocument();
    expect(within(row(list, 'Auftragsbestaetigung_4711.pdf')).getByText('von Ihnen übersprungen')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Im Ordner zeigen' }));
    expect(mock.nativeCalls).toContain('revealOutput:4');
    expect(mock.dispatched.filter((e) => e.type === 'skip_batch_document')).toHaveLength(1);
  });

  it('Esc fragt nach; „Stapel abbrechen“: Rest bleibt „Bereit“, Gespeichertes bleibt, Hinweis', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    act(() => mock.completeConfirmation());
    await confirmation(dialog, 2, 3, 'Auftragsbestaetigung_4711.pdf');
    await user.keyboard('{Escape}');
    expect(await within(dialog).findByRole('heading', { name: 'Stapel abbrechen?' })).toBeInTheDocument();
    expect(mock.dispatched.some((e) => e.type === 'abort_batch' || e.type === 'cancel')).toBe(false);
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Weiter signieren' }));
    await within(dialog).findByRole('heading', { name: 'Benachrichtigung an die ID-Austria-App gesendet' });
    await user.keyboard('{Escape}');
    await user.click(await within(dialog).findByRole('button', { name: 'Stapel abbrechen' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(mock.dispatched.filter((e) => e.type === 'abort_batch')).toHaveLength(1);
    expect(await screen.findByText('Stapel abgebrochen')).toBeInTheDocument();
    expect(screen.getByText('1 von 4 signiert, 1 abgelehnt, 1 abgebrochen, 1 nicht begonnen')).toBeInTheDocument();
    expect(within(row(list, 'Angebot_2026-118.pdf')).getByText('Signiert')).toBeInTheDocument();
    expect(within(row(list, 'Auftragsbestaetigung_4711.pdf')).getByText('Abgebrochen')).toBeInTheDocument();
    expect(within(row(list, 'Vertrag.pdf')).getByText('Bereit')).toBeInTheDocument();
    expect(await screen.findByText('Stapel abgebrochen – ein Dokument wurde bereits signiert und gespeichert.')).toBeInTheDocument();
  });

  it('Netzfehler nach der Anmeldung → Pause (Verbindung) → „Erneut versuchen“ beim aktuellen Dokument ohne neue Anmeldung', async () => {
    const { user, mock } = await openBatch({ batchFaults: { 'Auftragsbestaetigung_4711.pdf': 'network' } });
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    act(() => mock.completeConfirmation());
    const pause = await within(dialog).findByTestId('batch-pause');
    expect(pause).toHaveAttribute('data-scope', 'session');
    expect(within(pause).getByRole('heading', { name: 'Der Signaturdienst ist gerade nicht erreichbar' })).toBeInTheDocument();
    expect(within(pause).getByText('Der Stapel ist angehalten. Bitte entscheiden Sie, wie es weitergeht.')).toBeInTheDocument();
    expect(within(pause).queryByText(/neue Anmeldung ist möglich/)).not.toBeInTheDocument();
    expect(within(pause).getByRole('button', { name: 'Überspringen' })).toBeInTheDocument();
    expect(within(pause).getByRole('button', { name: 'Stapel abbrechen' })).toBeInTheDocument();
    // die Stapelkarte bietet in der Pause kein zweites „Überspringen“ an
    expect(within(dialog).queryByRole('button', { name: 'Dokument überspringen' })).not.toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(pause).getByRole('button', { name: 'Erneut versuchen' }));
    await confirmation(dialog, 2, 3, 'Auftragsbestaetigung_4711.pdf');
    expect(within(dialog).queryByLabelText('Signatur-Passwort')).not.toBeInTheDocument();
  });

  it('Sitzungsfehler → Pause mit „neue Anmeldung möglich“; „Erneut versuchen“ öffnet eine neue Sitzung (Anmeldung erneut)', async () => {
    const { user, mock } = await openBatch({ batchFaults: { 'Auftragsbestaetigung_4711.pdf': 'session' } });
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    // ohne „Daten merken“: eine neue Sitzung fragt erneut nach dem Passwort
    const dialog = await login(user, 'geheim123', false);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    act(() => mock.completeConfirmation());
    const pause = await within(dialog).findByTestId('batch-pause');
    expect(pause).toHaveAttribute('data-scope', 'session');
    expect(within(pause).getByText('Erneut versuchen öffnet eine neue Sitzung – eine neue Anmeldung ist möglich.')).toBeInTheDocument();
    expect(within(pause).getByText(/Unbekannte Seite des Signaturdienstes/)).toBeInTheDocument();
    // ohne eigene Überschrift: „Der Stapel ist angehalten“ steht nur im Titel, nicht doppelt
    expect(within(pause).getByRole('heading', { name: 'Der Stapel ist angehalten' })).toBeInTheDocument();
    expect(within(pause).getAllByText(/Der Stapel ist angehalten/)).toHaveLength(1);
    expect(within(pause).getByText('Bitte entscheiden Sie, wie es weitergeht.')).toBeInTheDocument();
    await user.click(within(pause).getByRole('button', { name: 'Erneut versuchen' }));
    // Stepper sichtbar zurück auf „Anmelden“
    expect(await within(dialog).findByLabelText('Signatur-Passwort')).toBeInTheDocument();
    expect(within(dialog).getByText('Anmelden', { selector: '.step-label' }).closest('.step')).toHaveClass('is-current');
    await user.type(within(dialog).getByLabelText('Signatur-Passwort'), 'geheim123');
    await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));
    await confirmation(dialog, 2, 3, 'Auftragsbestaetigung_4711.pdf');
  });

  it('Ablehnung in der App → Pause (Dokument), nie automatisch weiter; „Erneut versuchen“ fordert eine neue Bestätigung an', async () => {
    const { user, list } = await openBatch({ batchFaults: { 'Angebot_2026-118.pdf': 'decline' } });
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    const pause = await within(dialog).findByTestId('batch-pause', {}, { timeout: 3000 });
    expect(pause).toHaveAttribute('data-scope', 'document');
    expect(within(pause).getByRole('heading', { name: 'In der App abgelehnt oder abgelaufen' })).toBeInTheDocument();
    expect(within(row(list, 'Angebot_2026-118.pdf')).getByText('Wird signiert')).toBeInTheDocument();
    expect(within(row(list, 'Angebot_2026-118.pdf')).getByText('angehalten')).toBeInTheDocument();
    // bleibt stehen, bis entschieden ist
    await new Promise((r) => setTimeout(r, 30));
    expect(within(dialog).getByTestId('batch-pause')).toBeInTheDocument();
    await user.click(within(pause).getByRole('button', { name: 'Erneut versuchen' }));
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    expect(within(dialog).queryByLabelText('Signatur-Passwort')).not.toBeInTheDocument();
  });

  it('nicht wiederholbarer Dokumentfehler (zu groß): Zeile „Fehler“, der Lauf geht weiter', async () => {
    const { user, mock, list } = await openBatch({ batchFaults: { 'Auftragsbestaetigung_4711.pdf': 'too-large' } });
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    act(() => mock.completeConfirmation());
    await confirmation(dialog, 3, 3, 'Vertrag.pdf');
    const failed = row(list, 'Auftragsbestaetigung_4711.pdf');
    expect(within(failed).getByText('Fehler')).toBeInTheDocument();
    expect(within(failed).getByText('Das Dokument überschreitet die Größengrenze des Signaturdienstes.')).toBeInTheDocument();
    act(() => mock.completeConfirmation());
    expect(await screen.findByText('2 von 4 signiert, 1 abgelehnt, 1 fehlgeschlagen')).toBeInTheDocument();
  });

  it('⌘↵ startet „Alle signieren“; ohne Standard-Signiermittel öffnet sich das Menü, die Wahl startet (Keystore, Passwort einmal)', async () => {
    const ks = { enabled: true, file: '/Users/demo/max.p12', declared_type: 'PKCS12', alias: 'max', password_storage: 'memory' as const, storage_chosen: true };
    const { user, mock } = await openBatch({ settings: settings({ backend: { default: null }, keystore: ks }), isMac: true });
    fireEvent.keyDown(window, { key: 'Enter', metaKey: true });
    const menu = await screen.findByRole('menu');
    expect(menu).toHaveTextContent('Signieren mit');
    expect(sessions(mock.dispatched)).toEqual([]);
    await user.click(within(menu).getByRole('menuitem', { name: /Keystore/ }));
    expect(mock.dispatched.filter((e) => e.type === 'start_batch')).toEqual([{ type: 'start_batch', backend: 'keystore', docs: [1, 2, 4], output_dir: null, on_existing: 'keep_both' }]);
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    await user.type(await within(dialog).findByLabelText('Keystore-Passwort'), 'geheim');
    await user.click(within(dialog).getByRole('button', { name: 'Signieren' }));
    expect(await screen.findByText('3 von 4 signiert, 1 abgelehnt')).toBeInTheDocument();
    // das Passwort wurde nur einmal abgefragt
    expect(mock.dispatched.filter((e) => e.type === 'submit_secret')).toHaveLength(1);
  });
});

describe('Einzeldokument aus dem Stapel (E2.6)', () => {
  it('öffnen, verschieben, „Zurück zum Stapel“ → „Positioniert“ mit diesem Plan; erneut öffnen zeigt die Lage', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(within(row(list, 'Angebot_2026-118.pdf')).getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' }));
    const sticker = await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.getByRole('banner')).toHaveTextContent('Angebot_2026-118.pdf');
    // „Zurück zum Stapel“ als Pfeil in der Kopfzeile und unter „Signieren“
    expect(screen.getAllByRole('button', { name: 'Zurück zum Stapel' })).toHaveLength(2);
    const before = (await mock.view()).document?.placement;
    fireEvent.keyDown(sticker, { key: 'ArrowUp' });
    await waitFor(() => expect((async () => (await mock.view()).document?.origin)()).resolves.toBe('user'));
    const moved = (await mock.view()).document?.placement;
    expect(moved?.y).toBeCloseTo((before?.y ?? 0) + 15);
    // Zurück (Knopf im Inspektor; die Kopfzeile hat denselben als Pfeil)
    await user.click(within(screen.getByRole('complementary', { name: 'Signatur' })).getByRole('button', { name: 'Zurück zum Stapel' }));
    const back = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    const offer = row(back, 'Angebot_2026-118.pdf');
    expect(within(offer).getByText('Positioniert')).toBeInTheDocument();
    expect(within(offer).getByText('Seite 2, selbst platziert')).toBeInTheDocument();
    await user.click(within(offer).getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    const reopened = (await mock.view()).document;
    expect(reopened?.origin).toBe('user');
    expect(reopened?.placement.y).toBeCloseTo(moved?.y ?? 0);
  });

  it('einzeln signieren → Hinweis, zurück zur Übersicht; zählt in der Zusammenfassung als signiert', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(within(row(list, 'Angebot_2026-118.pdf')).getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    expect(mock.dispatched.filter((e) => e.type === 'sign_with')).toEqual([{ type: 'sign_with', backend: 'atrust_native' }]);
    await login(user);
    await screen.findByRole('heading', { name: 'Benachrichtigung an die ID-Austria-App gesendet' });
    act(() => mock.completeConfirmation());
    // kein „Fertig“, sondern zurück zur Übersicht mit Toast
    const back = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(screen.queryByRole('heading', { level: 1, name: 'Signiert' })).not.toBeInTheDocument();
    expect(await screen.findByText('„Angebot_2026-118.pdf“ wurde signiert.')).toBeInTheDocument();
    expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('Signiert')).toBeInTheDocument();
    // Lauf über den Rest: die Anmeldung ist gemerkt, die Zusammenfassung zählt das einzeln signierte mit
    await user.click(screen.getByRole('button', { name: 'Alle signieren (2)' }));
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' });
    await confirmation(dialog, 1, 2, 'Auftragsbestaetigung_4711.pdf');
    act(() => mock.completeConfirmation());
    await confirmation(dialog, 2, 2, 'Vertrag.pdf');
    act(() => mock.completeConfirmation());
    expect(await screen.findByText('3 von 4 signiert, 1 abgelehnt')).toBeInTheDocument();
  });

  it('weitere Dateien während eines Einzeldokuments: Hinweis und „Zum Stapel“ (aktuelle Platzierung als Plan)', async () => {
    const { mock, user } = renderApp({ settings: settings(), pickFilesResult: [`${DIR}/Vertrag.pdf`] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    act(() => mock.simulateFileDrag({ type: 'drop', paths: [`${DIR}/Angebot.pdf`], x: 1, y: 1 }));
    expect(await screen.findByText('Ein weiteres Dokument wurde hinzugefügt.')).toBeInTheDocument();
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Zum Stapel' }));
    const list = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(row(list, 'Vertrag.pdf')).getByText('Positioniert')).toBeInTheDocument();
    await waitFor(() => expect(within(row(list, 'Angebot.pdf')).getByText('Bereit')).toBeInTheDocument());
  });
});

describe('Start mit Ordnerwahl und vorhandenen Dateien (E2.7)', () => {
  const existing = [`${DIR}/Verträge/Vertrag_signed.pdf`];

  it('vorhandene Zieldatei: Hinweis, Auswahl vor dem Start, „Beide behalten“ (Standard) erzeugt „… (2).pdf“', async () => {
    const { list, user, mock } = await openBatch({ existingFiles: existing });
    expect(within(row(list, 'Vertrag.pdf')).getByText(/„Vertrag_signed\.pdf“ gibt es schon/, { selector: '.batch-row-detail' })).toBeInTheDocument();
    expect(screen.getByText('Eine Zieldatei gibt es schon – Sie entscheiden beim Start.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const start = await screen.findByRole('dialog', { name: '3 Dokumente signieren' });
    expect(within(start).getByText('Für ein Dokument gibt es die Zieldatei schon.')).toBeInTheDocument();
    // Einzahl wie der Satz darüber (Emulator-Abnahme 17.10.2026: „Diese Dokumente …“ bei nur einem)
    expect(within(start).getByRole('radio', { name: /Überspringen/ }).closest('label')).toHaveTextContent('Dieses Dokument wird nicht signiert – ohne Bestätigung in der App.');
    expect(within(start).getByTestId('privacy-note')).toHaveTextContent('Die Dokumente werden zum Signieren an A-Trust übermittelt.');
    expect(within(start).getByRole('radio', { name: /Beide behalten/ })).toBeChecked();
    expect(within(start).getByText(/„Vertrag_signed \(2\)\.pdf“/)).toBeInTheDocument();
    expect(mock.dispatched.some((e) => e.type === 'start_batch')).toBe(false);
    await expectNoAxeViolations();
    await user.click(within(start).getByRole('button', { name: 'Signieren starten' }));
    expect(mock.dispatched.filter((e) => e.type === 'start_batch')).toEqual([{ type: 'start_batch', backend: 'atrust_native', docs: [1, 2, 4], output_dir: null, on_existing: 'keep_both' }]);
    const dialog = await login(user);
    // Stapelkarte: der Kurzhinweis spricht von allen Dokumenten des Laufs
    expect(within(dialog).getByTestId('privacy-note')).toHaveTextContent('Die Dokumente werden zum Signieren an A-Trust übermittelt.');
    for (const [i, name] of [
      [1, 'Angebot_2026-118.pdf'],
      [2, 'Auftragsbestaetigung_4711.pdf'],
      [3, 'Vertrag.pdf'],
    ] as const) {
      await confirmation(dialog, i, 3, name);
      act(() => mock.completeConfirmation());
    }
    expect(await within(row(list, 'Vertrag.pdf')).findByText('als Vertrag_signed (2).pdf')).toBeInTheDocument();
  });

  it('Keystore: „Überspringen“ ohne „Bestätigung in der App“ (die gibt es dort nicht)', async () => {
    const ks = { enabled: true, file: '/Users/demo/max.p12', declared_type: 'PKCS12', alias: 'max', password_storage: 'memory' as const, storage_chosen: true };
    const { user } = await openBatch({ existingFiles: existing, settings: settings({ backend: { default: null }, keystore: ks }) });
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: /Keystore/ }));
    const start = await screen.findByRole('dialog', { name: '3 Dokumente signieren' });
    const skip = within(start).getByRole('radio', { name: /Überspringen/ });
    expect(skip.closest('label')).toHaveTextContent('Dieses Dokument wird nicht signiert.');
    expect(start).not.toHaveTextContent('ohne Bestätigung in der App');
  });

  it('„Überspringen“: für dieses Dokument keine Bestätigung', async () => {
    const { list, user, mock } = await openBatch({ existingFiles: existing });
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const start = await screen.findByRole('dialog', { name: '3 Dokumente signieren' });
    await user.click(within(start).getByRole('radio', { name: /Überspringen/ }));
    await user.click(within(start).getByRole('button', { name: 'Signieren starten' }));
    expect(mock.dispatched.filter((e) => e.type === 'start_batch').at(-1)).toMatchObject({ on_existing: 'skip' });
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    act(() => mock.completeConfirmation());
    await confirmation(dialog, 2, 3, 'Auftragsbestaetigung_4711.pdf');
    act(() => mock.completeConfirmation());
    // keine dritte Bestätigung: der Lauf endet
    expect(await screen.findByText('2 von 4 signiert, 1 übersprungen, 1 abgelehnt')).toBeInTheDocument();
    const contract = row(list, 'Vertrag.pdf');
    expect(within(contract).getByText('Übersprungen')).toBeInTheDocument();
    expect(within(contract).getByText('Zieldatei gab es schon')).toBeInTheDocument();
  });

  it('„jedes Mal fragen“: Ordnerwahl vor dem Start, „Signieren starten“ erst mit Ordner; Speicherort in der Zusammenfassung', async () => {
    const { user, mock } = await openBatch({ settings: settings({ output: { folder: null, postfix: '_signed' } }), pickDirectoryResult: '/Users/demo/Signiert' });
    expect(screen.getByTestId('batch-output')).toHaveTextContent('Den Ordner für die signierten Dokumente wählen Sie beim Start.');
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const start = await screen.findByRole('dialog', { name: '3 Dokumente signieren' });
    expect(within(start).getByTestId('batch-start-folder')).toHaveTextContent('Noch kein Ordner gewählt');
    expect(within(start).getByRole('button', { name: 'Signieren starten' })).toBeDisabled();
    await user.click(within(start).getByRole('button', { name: 'Ordner wählen …' }));
    expect(await within(start).findByText('/Users/demo/Signiert')).toBeInTheDocument();
    expect(mock.nativeCalls).toContain('pickDirectory');
    await user.click(within(start).getByRole('button', { name: 'Signieren starten' }));
    expect(mock.dispatched.filter((e) => e.type === 'start_batch')).toEqual([{ type: 'start_batch', backend: 'atrust_native', docs: [1, 2, 4], output_dir: '/Users/demo/Signiert', on_existing: 'keep_both' }]);
    const dialog = await login(user);
    for (const [i, name] of [
      [1, 'Angebot_2026-118.pdf'],
      [2, 'Auftragsbestaetigung_4711.pdf'],
      [3, 'Vertrag.pdf'],
    ] as const) {
      await confirmation(dialog, i, 3, name);
      act(() => mock.completeConfirmation());
    }
    expect(await screen.findByText('Gespeichert in /Users/demo/Signiert.')).toBeInTheDocument();
  });

  it('Dateipfad als Ausgabeordner verlangt ebenfalls die Ordnerwahl vor dem Start', async () => {
    const { user } = await openBatch({ settings: settings({ output: { folder: '/Users/demo/Signiert/alle.pdf', postfix: '_signed' } }) });
    expect(screen.getByTestId('batch-output')).toHaveTextContent('Den Ordner „/Users/demo/Signiert/alle.pdf“ gibt es nicht');
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const start = await screen.findByRole('dialog', { name: '3 Dokumente signieren' });
    expect(within(start).getByRole('button', { name: 'Signieren starten' })).toBeDisabled();
    await user.click(within(start).getByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});

describe('Mobile', () => {
  it('Übersicht und Lauf: App-Dokumente statt Ordner, kein „Im Ordner zeigen“, Hinweis zum App-Wechsel, schmal ohne axe-Verstöße', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { user, mock, list } = await openBatch({ platform: 'ios' });
    expect(screen.getByTestId('batch-output')).toHaveTextContent('Speichern in den Dokumenten dieser App');
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    // jede Bestätigung verlangt den Wechsel in die App (kein Abbruch)
    expect(within(dialog).getByText('Bestätigen Sie in der App „ID Austria“ und kehren Sie dann zu PIDA zurück.')).toBeInTheDocument();
    await expectNoAxeViolations();
    for (const [i, name] of [
      [2, 'Auftragsbestaetigung_4711.pdf'],
      [3, 'Vertrag.pdf'],
    ] as const) {
      act(() => mock.completeConfirmation());
      await confirmation(dialog, i, 3, name);
    }
    act(() => mock.completeConfirmation());
    expect(await screen.findByText('Gespeichert in den Dokumenten dieser App.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Im Ordner zeigen' })).not.toBeInTheDocument();
    expect(within(row(list, 'Vertrag.pdf')).getByText('Signiert')).toBeInTheDocument();
  });

  it('fester Zustand iOS: Zusammenfassung ohne Ordner, axe', async () => {
    restore = emulateMedia({ narrow: true });
    renderApp({
      view: onMobile(
        batchOverview(
          batchData({
            items: [batchItem({ doc: 1, status: { state: 'signed', path: '/var/mobile/X/Documents/Vertrag_signed.pdf' } }), batchItem({ doc: 2, name: 'Angebot.pdf', status: { state: 'skipped', existing: false } })],
            output: { mode: 'app_documents', folder_display: null, needs_folder: false },
            summary: { total: 2, signed: 1, rejected: 0, failed: 0, skipped: 1, cancelled: 0, not_started: 0, folder_display: null, aborted: false },
            estimate: null,
          }),
          { platform: IOS },
        ),
      ),
    });
    expect(await screen.findByText('1 von 2 signiert, 1 übersprungen')).toBeInTheDocument();
    expect(screen.getByText('Gespeichert in den Dokumenten dieser App.')).toBeInTheDocument();
    expect(screen.getByRole('main').textContent).not.toContain('/var/mobile');
    await expectNoAxeViolations();
  });
});

describe('Titel der Zusammenfassung (Nachtrag 4: BatchSummaryView::aborted)', () => {
  const summaryOf = (aborted: boolean, cancelled: number) => ({ total: 3, signed: 1, rejected: 0, failed: 0, skipped: 0, cancelled, not_started: 2 - cancelled, folder_display: null, aborted });

  it('regulär beendeter Lauf mit einem abgebrochenen Eintrag aus einem früheren Lauf: „Stapel abgeschlossen“', async () => {
    renderApp({ view: batchOverview(batchData({ summary: summaryOf(false, 1), estimate: null })) });
    expect(await screen.findByText('Stapel abgeschlossen')).toBeInTheDocument();
    expect(screen.queryByText('Stapel abgebrochen')).not.toBeInTheDocument();
  });

  it('Abbruch vor dem ersten Dokument (nichts „abgebrochen“): trotzdem „Stapel abgebrochen“', async () => {
    renderApp({ view: batchOverview(batchData({ summary: summaryOf(true, 0), estimate: null })) });
    expect(await screen.findByText('Stapel abgebrochen')).toBeInTheDocument();
  });
});

describe('Automatische Positionierung im Stapel (Nachtrag 8)', () => {
  const autoToggle = () => screen.getByRole('switch', { name: 'Für diesen Stapel automatisch positionieren' });
  const autoEvents = (events: UiEvent[]) => events.filter((e) => e.type === 'set_batch_auto_position');

  it('Einstellung aus: Schalter „Für diesen Stapel automatisch positionieren“ – QR-Eintrag gilt als „Bereit“, startet nichts; axe', async () => {
    const { list, mock, user } = await openBatch();
    const toggle = autoToggle();
    expect(toggle).not.toBeChecked();
    expect(toggle).toHaveAccessibleDescription('Alle Dokumente werden automatisch platziert und sind bereit zum Signieren – nur für diesen Stapel, die Einstellung bleibt unverändert.');
    expect(screen.queryByTestId('batch-auto-setting')).not.toBeInTheDocument();
    expect(within(row(list, 'Auftragsbestaetigung_4711.pdf')).getByText('Positioniert')).toBeInTheDocument();
    // ohne eigene Platzierungen keine Rückfrage
    await user.click(toggle);
    expect(autoEvents(mock.dispatched)).toEqual([{ type: 'set_batch_auto_position', enabled: true, replace_manual: false }]);
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    await waitFor(() => expect(autoToggle()).toBeChecked());
    const qr = row(list, 'Auftragsbestaetigung_4711.pdf');
    expect(within(qr).getByText('Bereit')).toBeInTheDocument();
    expect(within(qr).getByText('im QR-Platzhalter')).toBeInTheDocument();
    // die Einstellung bleibt unverändert
    expect((await mock.view()).batch?.auto_position_setting).toBe(false);
    expect(sessions(mock.dispatched)).toEqual([]);
    await expectNoAxeViolations();
    // ausschalten: wieder wie eingestellt
    await user.click(autoToggle());
    await waitFor(() => expect(autoToggle()).not.toBeChecked());
    expect(within(row(list, 'Auftragsbestaetigung_4711.pdf')).getByText('Positioniert')).toBeInTheDocument();
  });

  it('von Hand platziert: Rückfrage – „Behalten“ (Vorauswahl) behält den Plan, „Auch dieses automatisch platzieren“ setzt ihn neu; Escape ändert nichts', async () => {
    const { list, user, mock } = await openBatch();
    await user.click(within(row(list, 'Angebot_2026-118.pdf')).getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' }));
    const sticker = await screen.findByRole('application', { name: 'Signaturblock' });
    fireEvent.keyDown(sticker, { key: 'ArrowUp' });
    await waitFor(() => expect((async () => (await mock.view()).document?.origin)()).resolves.toBe('user'));
    await user.click(within(screen.getByRole('complementary', { name: 'Signatur' })).getByRole('button', { name: 'Zurück zum Stapel' }));
    const back = await screen.findByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('Seite 2, selbst platziert')).toBeInTheDocument();
    expect((await mock.view()).batch?.manual_plans).toBe(1);

    // Escape: nichts geändert
    await user.click(autoToggle());
    let ask = await screen.findByRole('alertdialog', { name: 'Eigene Platzierung behalten?' });
    expect(ask).toHaveTextContent('Ein Dokument haben Sie selbst platziert. Soll es ebenfalls automatisch platziert werden?');
    expect(within(ask).getByRole('button', { name: 'Behalten' })).toHaveFocus();
    await expectNoAxeViolations();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
    expect(autoEvents(mock.dispatched)).toEqual([]);
    expect(autoToggle()).not.toBeChecked();

    // „Behalten“: an, der eigene Plan bleibt
    await user.click(autoToggle());
    ask = await screen.findByRole('alertdialog', { name: 'Eigene Platzierung behalten?' });
    await user.click(within(ask).getByRole('button', { name: 'Behalten' }));
    expect(autoEvents(mock.dispatched).at(-1)).toEqual({ type: 'set_batch_auto_position', enabled: true, replace_manual: false });
    await waitFor(() => expect(autoToggle()).toBeChecked());
    expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('Positioniert')).toBeInTheDocument();
    expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('Seite 2, selbst platziert')).toBeInTheDocument();

    // aus und wieder an mit „Auch dieses automatisch platzieren“ (ein Dokument: Einzahl)
    await user.click(autoToggle());
    await waitFor(() => expect(autoToggle()).not.toBeChecked());
    await user.click(autoToggle());
    ask = await screen.findByRole('alertdialog', { name: 'Eigene Platzierung behalten?' });
    await user.click(within(ask).getByRole('button', { name: 'Auch dieses automatisch platzieren' }));
    expect(autoEvents(mock.dispatched).at(-1)).toEqual({ type: 'set_batch_auto_position', enabled: true, replace_manual: true });
    await waitFor(() => expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('Bereit')).toBeInTheDocument());
    expect(within(row(back, 'Angebot_2026-118.pdf')).getByText('automatisch platziert')).toBeInTheDocument();
    expect((await mock.view()).batch?.manual_plans).toBe(0);
    expect(sessions(mock.dispatched)).toEqual([]);
  });

  it('nach dem Lauf gilt wieder die Einstellung (Mock wie der Kern)', async () => {
    const { mock, user } = await openBatch();
    await user.click(autoToggle());
    await waitFor(() => expect(autoToggle()).toBeChecked());
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    expect((await mock.view()).batch?.auto_position_once).toBe(true);
    await user.keyboard('{Escape}');
    await user.click(await within(dialog).findByRole('button', { name: 'Stapel abbrechen' }));
    expect(await screen.findByText('Stapel abgebrochen')).toBeInTheDocument();
    expect((await mock.view()).batch?.auto_position_once).toBe(false);
    expect(autoToggle()).not.toBeChecked();
  });

  it('Einstellung an: Hinweis „Automatische Positionierung (laut Einstellung)“ statt Schalter; englisch', async () => {
    const { unmount } = renderApp({ view: batchOverview(batchData({ auto_position_setting: true })) });
    expect(await screen.findByTestId('batch-auto-setting')).toHaveTextContent('Automatische Positionierung (laut Einstellung)');
    expect(screen.queryByRole('switch', { name: /automatisch positionieren/ })).not.toBeInTheDocument();
    await expectNoAxeViolations();
    unmount();
    renderApp({ view: batchOverview(batchData({ auto_position_once: true }), { language: 'en' }) });
    expect(await screen.findByRole('switch', { name: 'Position automatically for this batch' })).toBeChecked();
  });
});

describe('Wortlaut je Signiermittel (Mobile-Check 16.10.2026, Nachtrag 12: SignerView::batch_flow)', () => {
  const lead = () => screen.getByRole('heading', { level: 1 }).closest('.batch-titles')?.querySelector('.view-lead')?.textContent;

  it('Einleitung der Übersicht: ID Austria, Keystore, Chipkarte und „Jedes Mal fragen“', async () => {
    const cases = [
      ['atrust_native', 'Alle Dokumente werden in einem Durchgang signiert: einmal anmelden, dann jedes Dokument einzeln bestätigen. Dokumente mit Fehler werden übersprungen.'],
      ['keystore', 'Alle Dokumente werden in einem Durchgang signiert – der Keystore wird dafür einmal entsperrt. Dokumente mit Fehler werden übersprungen.'],
      ['local_bku', 'Alle Dokumente werden in einem Durchgang signiert, jedes bestätigen Sie einzeln. Dokumente mit Fehler werden übersprungen.'],
      [null, 'Alle Dokumente werden in einem Durchgang signiert. Dokumente mit Fehler werden übersprungen.'],
    ] as const;
    for (const [selected, text] of cases) {
      const { unmount } = renderApp({ view: batchOverview(batchData(), { signers: signersView({ selected, default: selected }) }) });
      await screen.findByRole('list', { name: 'Dokumente im Stapel' });
      expect(lead()).toBe(text);
      unmount();
    }
  });

  it('Keystore-Stapel: „Dokument k von n“ und „Signiert“ statt „Bestätigung“, Abbruch ohne „Passwort wird vergessen“', async () => {
    const ks = { enabled: true, file: '/Users/demo/max.p12', declared_type: 'PKCS12', alias: 'max', password_storage: 'memory' as const, storage_chosen: true };
    const { user } = await openBatch({ settings: settings({ backend: { default: null }, keystore: ks }) });
    // „Jedes Mal fragen“: neutral
    expect(lead()).toBe('Alle Dokumente werden in einem Durchgang signiert. Dokumente mit Fehler werden übersprungen.');
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitem', { name: /Keystore/ }));
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    await within(dialog).findByLabelText('Keystore-Passwort');
    const strip = within(dialog).getByTestId('batch-strip');
    expect(strip).toHaveTextContent('Dokument 1 von 3 · Angebot_2026-118.pdf');
    expect(strip).not.toHaveTextContent(/Bestätigung/);
    expect(within(strip).getByRole('progressbar', { name: 'Signiert' })).toBeInTheDocument();
    // Esc fragt nach: ohne Anmeldung kein „Passwort wird vergessen“
    await user.keyboard('{Escape}');
    expect(await within(dialog).findByText('Stapel abbrechen?')).toBeInTheDocument();
    expect(dialog).toHaveTextContent('Bereits signierte Dokumente bleiben gespeichert, die übrigen werden nicht signiert.');
    expect(dialog).not.toHaveTextContent('Das Passwort wird vergessen.');
  });

  it('ID-Austria-Stapel: „Bestätigung k von n“, „Bestätigungen“ und beim Abbruch „Passwort wird vergessen“', async () => {
    const { user } = await openBatch();
    expect(lead()).toMatch(/einmal anmelden, dann jedes Dokument einzeln bestätigen/);
    await user.click(screen.getByRole('button', { name: 'Alle signieren (3)' }));
    const dialog = await login(user);
    await confirmation(dialog, 1, 3, 'Angebot_2026-118.pdf');
    expect(within(within(dialog).getByTestId('batch-strip')).getByRole('progressbar', { name: 'Bestätigungen' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(await within(dialog).findByText('Stapel abbrechen?')).toBeInTheDocument();
    expect(dialog).toHaveTextContent('Das Passwort wird vergessen.');
  });
});
