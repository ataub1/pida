/**
 * Prüfmodus (Paket UI-B, ADR 0004 §3.9, Abnahme E4 Punkte 1–3 und 6 [M]): Status je Signatur (intakt,
 * verändert, nicht prüfbar, SHA-1-Hinweis), Unterzeichner, Zeit als Angabe des Signierenden, Zertifikatsdetails,
 * QES „laut Zertifikat“, spätere Revisionen benannt, Hinweis auf die vollständige Prüfung, nie „gültig“
 * (Katalogtest), „Auf der Seite zeigen“ mit Hervorhebung (Rechteck aus `PAGE_ABS`, auch auf einer Querseite),
 * unsichtbare Signaturen ohne Knopf, „Dieses Dokument signieren“ aus der Startansicht bzw. bei offenem
 * Dokument (Warteschlange + Hinweis), „Zuletzt signiert“ öffnet den Prüfmodus, Proben des Kerns, Mobile, axe.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { act, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ViewState } from '../../core/types';
import { SOURCES } from '../../i18n/catalog';
import { expectNoAxeViolations } from '../../test/a11y';
import { emulateMedia, lastEvent, renderApp } from '../../test/render';
import { IOS, onMobile } from '../../test/views';
import { boxOnPage, formatSerial } from './model';

const FIXTURE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../core/fixtures/view-states.json');
const PROBES = JSON.parse(readFileSync(FIXTURE, 'utf8')) as { name: string; view: ViewState }[];
function probe(name: string): ViewState {
  const v = PROBES.find((p) => p.name === name)?.view;
  if (!v) throw new Error(`Probe ${name} fehlt in view-states.json`);
  const view = structuredClone(v);
  // Freigaben wie K4 im Leerlauf (die Proben aus K0 tragen noch `verify: false`)
  view.actions = { ...view.actions, verify: true, open_files: true };
  return view;
}

/** Fester Zustand: erst nach dem ersten `ViewState` steht der Prüfmodus da. */
async function renderProbe(view: ViewState) {
  const r = renderApp({ view });
  await screen.findByTestId('verify-view');
  return r;
}

const DIR = '/Users/demo/Dokumente';

/** Startansicht → „Prüfen“ in der Kopfzeile → Dateidialog → Ergebnis. */
async function openVerify(file: string, opts: Parameters<typeof renderApp>[0] = {}) {
  const r = renderApp({ pickFilesResult: [`${DIR}/${file}`], ...opts });
  const header = await screen.findByRole('banner');
  await waitFor(() => expect(within(header).getByRole('button', { name: 'Signatur prüfen' })).toBeEnabled());
  await r.user.click(within(header).getByRole('button', { name: 'Signatur prüfen' }));
  await screen.findByRole('heading', { level: 1, name: file });
  return r;
}

const cards = () => screen.getAllByRole('listitem').filter((li) => li.classList.contains('sig-card'));

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Prüfmodus', () => {
  it('„Prüfen“ → Datei → „wird geprüft“ → Ergebnis: Status, Unterzeichner, Zeit, QES laut Zertifikat, Revisionen, Details', async () => {
    const { mock, user } = await openVerify('Mietvertrag_signiert.pdf');
    expect(mock.dispatched).toContainEqual({ type: 'open_verify', path: `${DIR}/Mietvertrag_signiert.pdf` });
    expect(mock.nativeCalls).toEqual(['pickFiles:single']);
    const summary = await screen.findByTestId('vf-summary');
    expect(summary).toHaveTextContent('Alle 2 Signaturen sind unverändert und kryptografisch intakt');
    expect(summary).toHaveTextContent('Das Dokument wurde seit der letzten Signatur nicht verändert.');
    expect(screen.getByText('2 Seiten · 2 Revisionen')).toBeInTheDocument();
    expect(screen.getAllByRole('img', { name: /^Seite \d von 2$/ })).toHaveLength(2);

    expect(cards()).toHaveLength(2);
    const first = cards()[0] as HTMLElement;
    expect(within(first).getByRole('heading', { level: 3, name: 'Dr. Anna Gruber' })).toBeInTheDocument();
    expect(within(first).getByText('Intakt')).toBeInTheDocument();
    expect(within(first).getByText('Signiert am 12.10.2026, 14:03 (Angabe des Signierenden)')).toBeInTheDocument();
    expect(within(first).getByText('Qualifizierte elektronische Signatur laut Zertifikat')).toBeInTheDocument();
    expect(within(first).getByText('Stand dieser Signatur unverändert')).toBeInTheDocument();
    expect(within(first).getByText('Danach eine weitere Revision, davon eine mit Signatur')).toBeInTheDocument();
    expect(within(cards()[1] as HTMLElement).getByText('Dokument seit dieser Signatur unverändert')).toBeInTheDocument();

    const trigger = within(first).getByRole('button', { name: 'Zertifikatsdetails' });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    await user.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    const details = within(first);
    expect(details.getByText('a-sign-premium-mobile-07 (Demo-CA)')).toBeVisible();
    expect(details.getByText('1 288 004 912')).toBeVisible();
    expect(details.getByText('20.11.2024 bis 20.11.2029')).toBeVisible();
    expect(details.getByText('ECDSA P-256 · SHA-256')).toBeVisible();
    expect(details.getByText('PAdES (ETSI.CAdES.detached)')).toBeVisible();
    expect(details.getByText('Signature1')).toBeVisible();
    expect(details.getByText('Seite 2')).toBeVisible();
    expect(details.getByText('Nicht geprüft')).toBeVisible();
    expect(details.getByText('Angabe aus dem Zertifikat, nicht gegen die EU-Vertrauensliste geprüft.')).toBeVisible();
    await expectNoAxeViolations();
  });

  it('Hinweis auf die vollständige Prüfung (signaturpruefung.gv.at); nie „gültig“', async () => {
    const { mock, user } = await openVerify('Mietvertrag_signiert.pdf');
    await screen.findByTestId('vf-summary');
    expect(screen.getByText(/Vertrauenskette und Sperrstatus prüft PIDA nicht/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Vollständige Prüfung bei signaturpruefung\.gv\.at/ }));
    expect(mock.openedUrls).toEqual(['https://www.signaturpruefung.gv.at']);
    expect(screen.getByTestId('verify-view')).not.toHaveTextContent(/(^|[^\p{L}])(un|rechts)?gültig(e|er|en|es)?(?![\p{L}])/iu);
  });

  it('verändert: Gesamtergebnis mit Warnung, Status „Verändert“, Erklärung und technischer Code', async () => {
    const { user } = await openVerify('Vertrag_manipuliert.pdf');
    expect(await screen.findByTestId('vf-summary')).toHaveTextContent('Achtung: Eine Signatur ist nicht intakt');
    const broken = cards()[1] as HTMLElement;
    expect(broken).toHaveClass('is-broken');
    expect(within(broken).getByText('Verändert')).toBeInTheDocument();
    expect(within(broken).getByText('Dokument nach dieser Signatur verändert oder Signatur beschädigt')).toBeInTheDocument();
    expect(within(broken).getByText('Die Prüfsumme der Signatur passt nicht zum Dokument.')).toBeInTheDocument();
    await user.click(within(broken).getByRole('button', { name: 'Zertifikatsdetails' }));
    expect(within(broken).getByText('cms-digest-mismatch')).toBeVisible();
    expect(within(cards()[0] as HTMLElement).getByText('Intakt')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('nicht prüfbar und SHA-1-Hinweis', async () => {
    await openVerify('Rechnung_sha1.pdf');
    expect(await screen.findByTestId('vf-summary')).toHaveTextContent('Nicht alle Signaturen konnten geprüft werden');
    const [sha1, pkcs7] = cards() as [HTMLElement, HTMLElement];
    expect(within(sha1).getByText('Intakt')).toBeInTheDocument();
    expect(within(sha1).getByText(/veraltete Hashverfahren SHA-1/)).toBeInTheDocument();
    expect(within(sha1).getByText('Qualifikation nicht feststellbar')).toBeInTheDocument();
    expect(within(pkcs7).getByText('Nicht prüfbar')).toBeInTheDocument();
    expect(within(pkcs7).getByText('Diese Signatur kann PIDA nicht prüfen')).toBeInTheDocument();
    expect(within(pkcs7).getByText('Nicht qualifiziert (laut Zertifikat)')).toBeInTheDocument();
    expect(pkcs7.querySelector('.sig-role')).toHaveTextContent('Muster IT-Dienstleistungen GmbH');
    await expectNoAxeViolations();
  });

  it('nach der letzten Signatur ergänzt: benannt, ohne „gültig“', async () => {
    await openVerify('Vertrag_ergaenzt.pdf');
    expect(await screen.findByTestId('vf-summary')).toHaveTextContent('Nach der letzten Signatur ergänzt oder geändert');
    expect(within(cards()[1] as HTMLElement).getByText('Danach eine weitere Revision, ohne weitere Signatur')).toBeInTheDocument();
    expect(screen.getByText('2 Seiten · 3 Revisionen')).toBeInTheDocument();
  });

  it('keine Signatur; „Prüfmodus verlassen“', async () => {
    const { mock, user } = await openVerify('Angebot_unsigniert.pdf');
    expect(await screen.findByTestId('vf-summary')).toHaveTextContent('Dieses Dokument enthält keine Signatur');
    expect(screen.queryByRole('heading', { level: 2, name: /Signaturen/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Dieses Dokument signieren' })).toBeEnabled();
    await expectNoAxeViolations();
    await user.click(screen.getByRole('button', { name: 'Prüfmodus verlassen' }));
    expect(lastEvent(mock)).toEqual({ type: 'close_verify' });
    expect(await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
  });

  it('defekte Datei: Fehler des Kerns, kein „Signieren“, andere Datei prüfen', async () => {
    const { mock, user } = await openVerify('Scan_defekt.pdf');
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Die Datei konnte nicht geprüft werden');
    expect(alert).toHaveTextContent('Die Datei konnte nicht zum Prüfen geöffnet werden.');
    expect(screen.queryByRole('button', { name: 'Dieses Dokument signieren' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Anderes Dokument prüfen' }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'open_verify', path: `${DIR}/Scan_defekt.pdf` }));
    await expectNoAxeViolations();
  });
});

describe('Prüfmodus: Auf der Seite zeigen', () => {
  it('hebt das Signaturbild hervor, blättert dorthin und sagt es an; Überfahren hebt hervor', async () => {
    const scroll = vi.fn();
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = scroll;
    restore = () => {
      Element.prototype.scrollIntoView = original;
    };
    const { user } = await openVerify('Mietvertrag_signiert.pdf');
    await screen.findByTestId('vf-summary');
    const [first, second] = cards() as [HTMLElement, HTMLElement];
    expect(screen.getByTestId('vf-mark-2')).not.toHaveClass('is-active');
    await user.click(within(second).getByRole('button', { name: 'Auf der Seite zeigen' }));
    expect(screen.getByTestId('vf-mark-2')).toHaveClass('is-active');
    expect(screen.getByTestId('vf-mark-1')).not.toHaveClass('is-active');
    expect(scroll).toHaveBeenCalledWith(expect.objectContaining({ block: 'center' }));
    expect(scroll.mock.contexts[0]).toBe(screen.getByRole('img', { name: 'Seite 2 von 2' }));
    await waitFor(() => expect(screen.getByTestId('live-region')).toHaveTextContent('Signaturbild von Max Mustermann auf Seite 2 hervorgehoben.'));
    // Name des Knopfs bleibt kurz, die Beschreibung nennt die Signatur
    expect(within(second).getByRole('button', { name: 'Auf der Seite zeigen' })).toHaveAccessibleDescription('Max Mustermann');
    await user.hover(first);
    expect(screen.getByTestId('vf-mark-1')).toHaveClass('is-active');
    await user.unhover(first);
    expect(screen.getByTestId('vf-mark-2')).toHaveClass('is-active');
  });

  it('Rechteck aus `AppearanceView` (PAGE_ABS, Ursprung unten) auf der Querseite der Kernprobe', async () => {
    await renderProbe(probe('ws_verify_intact'));
    const mark = screen.getByTestId('vf-mark-1');
    // Probe: Seite 2 quer (842 × 595,5 pt), Rechteck links 297,5 · unten 128,5 · rechts 530 · oben 210,5; Maßstab 0,9
    expect(parseFloat(mark.style.left)).toBeCloseTo(297.5 * 0.9);
    expect(parseFloat(mark.style.top)).toBeCloseTo((595.5 - 210.5) * 0.9);
    expect(parseFloat(mark.style.width)).toBeCloseTo(232.5 * 0.9);
    expect(parseFloat(mark.style.height)).toBeCloseTo(82 * 0.9);
    expect(mark.closest('[role=img]')).toHaveAccessibleName('Seite 2 von 2');
    expect(boxOnPage({ left: 0, bottom: 0, right: 10, top: 20 }, 100, 2)).toEqual({ left: 0, top: 160, width: 20, height: 40 });
    expect(formatSerial('1288004912')).toBe('1 288 004 912');
    expect(formatSerial('0x1f')).toBe('0x1f');
  });

  it('unsichtbare Signatur: kein Knopf, „unsichtbar“ in den Details', async () => {
    const { user } = await openVerify('Vertrag_unsichtbar.pdf');
    await screen.findByTestId('vf-summary');
    const [visible, invisible] = cards() as [HTMLElement, HTMLElement];
    expect(within(visible).getByRole('button', { name: 'Auf der Seite zeigen' })).toBeInTheDocument();
    expect(within(invisible).queryByRole('button', { name: 'Auf der Seite zeigen' })).not.toBeInTheDocument();
    expect(screen.queryByTestId('vf-mark-2')).not.toBeInTheDocument();
    await user.click(within(invisible).getByRole('button', { name: 'Zertifikatsdetails' }));
    expect(within(invisible).getByText('unsichtbar')).toBeVisible();
    expect(within(invisible).getByText('Qualifiziertes Zertifikat ohne qualifizierte Signaturerstellungseinheit (laut Zertifikat)')).toBeInTheDocument();
  });
});

describe('Prüfmodus: Dieses Dokument signieren', () => {
  it('aus der Startansicht: Arbeitsbereich mit der Datei', async () => {
    const { mock, user } = await openVerify('Mietvertrag_signiert.pdf');
    await screen.findByTestId('vf-summary');
    await user.click(screen.getByRole('button', { name: 'Dieses Dokument signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_verified' });
    expect(await screen.findByRole('application', { name: 'Signaturblock' })).toBeInTheDocument();
    expect(screen.getByRole('banner')).toHaveTextContent('Mietvertrag_signiert.pdf');
    expect(screen.queryByTestId('verify-view')).not.toBeInTheDocument();
  });

  it('bei offenem Dokument: in die Warteschlange, Hinweis, das offene Dokument bleibt', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', pickFilesResult: [`${DIR}/Mietvertrag_signiert.pdf`] });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Signatur prüfen' }));
    await screen.findByTestId('vf-summary');
    await user.click(screen.getByRole('button', { name: 'Dieses Dokument signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_verified' });
    expect(await screen.findByText('Ein weiteres Dokument wurde hinzugefügt.')).toBeInTheDocument();
    expect(screen.getByRole('banner')).toHaveTextContent('Vertrag.pdf');
  });

  it('ohne Freigabe `open_files` gesperrt; „Anderes Dokument prüfen“ nur mit `actions.verify`', async () => {
    const v = probe('ws_verify_intact');
    await renderProbe({ ...v, actions: { ...v.actions, open_files: false, verify: false } });
    expect(screen.getByRole('button', { name: 'Dieses Dokument signieren' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Anderes Dokument prüfen' })).toBeDisabled();
    // im Prüfmodus kein Kopfzeilenknopf „Prüfen“ (wie im Prototyp), stattdessen der Kontext „Prüfmodus“
    expect(within(screen.getByRole('banner')).queryByRole('button', { name: 'Signatur prüfen' })).not.toBeInTheDocument();
  });
});

describe('Prüfmodus in der Shell (Merge E4)', () => {
  it('über einem offenen Dokument: Kopfzeile „Prüfmodus“, kein Signieren des verdeckten Dokuments, Wortmarke verlässt ihn', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', pickFilesResult: [`${DIR}/Mietvertrag_signiert.pdf`], isMac: true });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const banner = screen.getByRole('banner');
    expect(banner).toHaveTextContent('Vertrag.pdf');
    await user.click(within(banner).getByRole('button', { name: 'Signatur prüfen' }));
    await screen.findByTestId('vf-summary');
    expect(banner).toHaveTextContent('Prüfmodus');
    expect(banner).not.toHaveTextContent('Vertrag.pdf');
    expect(within(banner).queryByRole('button', { name: 'Signatur prüfen' })).not.toBeInTheDocument();
    expect(document.title).toBe('PIDA – Prüfmodus');
    // wie K4: „Signieren“ gesperrt (Palette ohne den Befehl), ⌘↵ wirkt nicht
    const view = await mock.view();
    expect(view.actions.sign || view.actions.edit_placement || view.actions.start_batch).toBe(false);
    const before = mock.dispatched.length;
    await user.keyboard('{Meta>}{Enter}{/Meta}');
    expect(mock.dispatched.slice(before).some((e) => e.type === 'sign_with')).toBe(false);
    await user.keyboard('{Meta>}k{/Meta}');
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    expect(within(palette).queryByRole('menuitem', { name: /^Signieren/ })).not.toBeInTheDocument();
    await user.keyboard('{Escape}');
    // „Prüfmodus verlassen“: zurück im Arbeitsbereich mit dem Dokument, Signieren wieder frei
    await user.click(screen.getByRole('button', { name: 'Prüfmodus verlassen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(banner).toHaveTextContent('Vertrag.pdf');
    expect((await mock.view()).actions.sign).toBe(true);
    // „Zur Startseite“ (Palette bzw. Wortmarke) wie im Prototyp: Prüfmodus schließen, dann Startansicht
    await user.click(within(banner).getByRole('button', { name: 'Signatur prüfen' }));
    await screen.findByTestId('vf-summary');
    await user.keyboard('{Meta>}k{/Meta}');
    await user.click(within(await screen.findByRole('dialog', { name: 'Befehle' })).getByRole('menuitem', { name: /^Zur Startseite/ }));
    await waitFor(() => expect(mock.dispatched.slice(-2)).toEqual([{ type: 'close_verify' }, { type: 'navigate', step: 'Open' }]));
    await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' });
    expect(screen.queryByTestId('verify-view')).not.toBeInTheDocument();
  });

  it('Ablegen im Prüfmodus prüft die Datei; Öffnen (⌘O) zeigt sie im Arbeitsbereich', async () => {
    const { mock, user } = await openVerify('Mietvertrag_signiert.pdf', { isMac: true });
    act(() => mock.simulateFileDrag({ type: 'drop', paths: [`${DIR}/Kaufvertrag_manipuliert.pdf`, `${DIR}/b.pdf`], x: 1, y: 1 }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'open_verify', path: `${DIR}/Kaufvertrag_manipuliert.pdf` }));
    await screen.findByRole('heading', { level: 1, name: 'Kaufvertrag_manipuliert.pdf' });
    await user.keyboard('{Meta>}o{/Meta}');
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'open_files', paths: [`${DIR}/Mietvertrag_signiert.pdf`] }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('verify-view')).not.toBeInTheDocument();
  });
});

describe('Prüfmodus: Proben des Kerns, Mobile, Sprache', () => {
  it.each([
    ['ws_verify_loading', 'Signaturen werden geprüft …'],
    ['ws_verify_failed', 'Die Datei konnte nicht geprüft werden'],
    ['ws_verify_intact', 'Die Signatur ist unverändert und kryptografisch intakt'],
    ['ws_verify_broken', 'Achtung: Eine Signatur ist nicht intakt'],
    ['ws_verify_not_verifiable', 'Nicht alle Signaturen konnten geprüft werden'],
    ['ws_verify_modified', 'Nach der letzten Signatur ergänzt oder geändert'],
  ])('%s', async (name, text) => {
    const v = probe(name);
    // die Probe trägt vier Signaturen, das Gesamtergebnis zählt nur die verletzten bzw. alle
    if (name === 'ws_verify_intact') v.verify = { ...(v.verify as NonNullable<ViewState['verify']>), signatures: v.verify?.signatures.slice(0, 1) ?? [] };
    await renderProbe(v);
    expect(await screen.findByRole('heading', { level: 1, name: 'Vertrag_signed.pdf' })).toBeInTheDocument();
    expect(screen.getByTestId('verify-view')).toHaveTextContent(text);
    await expectNoAxeViolations();
  });

  it('Proben: unbekannter Unterzeichner, Hinweise, Format, kein Knopf bei unsichtbaren', async () => {
    await renderProbe(probe('ws_verify_not_verifiable'));
    const list = cards();
    expect(list).toHaveLength(4);
    expect(within(list[2] as HTMLElement).getByRole('heading', { name: 'Signatur 3 (Unterzeichner unbekannt)' })).toBeInTheDocument();
    expect(within(list[2] as HTMLElement).getByText('Ohne Angabe des Signaturzeitpunkts')).toBeInTheDocument();
    expect(within(list[2] as HTMLElement).getByText(/SHA-1/)).toBeInTheDocument();
    expect(within(list[3] as HTMLElement).getByText('Nicht prüfbar')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Auf der Seite zeigen' })).toHaveLength(1);
  });

  it('Mobile (schmal): Seiten über dem Ergebnis, Dokumentauswahl über das Plugin', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { mock, user } = await renderProbe(onMobile(probe('ws_verify_broken'), IOS));
    expect(await screen.findByRole('heading', { level: 1, name: 'Vertrag_signed.pdf' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Anderes Dokument prüfen' }));
    await waitFor(() => expect(mock.nativeCalls).toContain('pickMobileFiles:pdf:single'));
    await expectNoAxeViolations();
  });

  it('englisch', async () => {
    const v = probe('ws_verify_modified');
    await renderProbe({ ...v, language: 'en' });
    expect(await screen.findByText('Added to or changed after the last signature')).toBeInTheDocument();
    expect(screen.getAllByText(/\(as stated by the signatory\)$/).length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Sign this document' })).toBeInTheDocument();
    expect(screen.getByTestId('verify-view')).not.toHaveTextContent(/\b(in)?valid\b/i);
    await expectNoAxeViolations();
  });
});

describe('Prüfmodus: Wortlaut (ADR 0004 §8 F3)', () => {
  /** Texte des Prüfmodus (`ng-ui-vf-*`) je Sprache. */
  function vfTexts(lang: 'de' | 'en'): string[] {
    const text = SOURCES[lang].find((s) => s.name === `${lang}/ui-tools.ftl`)?.text ?? '';
    const out: string[] = [];
    let current = false;
    for (const line of text.split('\n')) {
      if (/^ng-ui-[a-z]/.test(line)) current = line.startsWith('ng-ui-vf-');
      if (current && !line.trimStart().startsWith('.')) out.push(line);
    }
    return out;
  }

  it('nie „gültig“, „rechtsgültig“, „valid“; Qualifikation nur „laut Zertifikat“; Hinweis auf die vollständige Prüfung', () => {
    const de = vfTexts('de');
    const en = vfTexts('en');
    expect(de.length).toBeGreaterThan(40);
    expect(de.filter((l) => /(^|[^\p{L}])(un|rechts)?gültig(e|er|en|es)?(?![\p{L}])/iu.test(l))).toEqual([]);
    expect(en.filter((l) => /\b(in)?valid\b|legally/i.test(l))).toEqual([]);
    const all = de.join('\n');
    expect(all).toMatch(/Qualifizierte elektronische Signatur laut Zertifikat/);
    expect(all).toMatch(/Vollständige Prüfung bei signaturpruefung\.gv\.at/);
    expect(all).toMatch(/Angabe des Signierenden/);
  });
});
