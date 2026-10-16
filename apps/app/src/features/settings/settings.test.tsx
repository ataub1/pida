/**
 * Einstellungen (Paket UI-B, ADR 0004 E3, Abnahmekriterien [M]): Seitenleiste mit Bereichen und Suche
 * (Synonyme), alles sofort wirksam (`apply_settings` nur mit den geänderten Pfaden, kein Speichern), Live-Editor
 * Signaturbild mit der Vorschau des Kerns, Bereich „Dokument“ (PDF/A, Platzhaltersuche), Signiermittel,
 * Speichern, Darstellung, Datenschutz, Erweitert, Über, schreibgeschützt, Mobile (Vollbild, ohne Proxy/Ordner/
 * Updates/Log, Keystore nur per Import).
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { EMBLEM_DROP_EVENT } from '../../app/events';
import { defaultSettings } from '../../core/defaults';
import type { SettingsMockOptions } from '../../core/mock/settings';
import type { Settings, SettingsView, UiEvent } from '../../core/types';
import { translator } from '../../i18n/catalog';
import { lastEvent, renderApp } from '../../test/render';
import { IOS, onMobile, settingsView, workspaceView } from '../../test/views';

type Apply = Extract<UiEvent, { type: 'apply_settings' }>;

function applies(dispatched: UiEvent[]): Apply[] {
  return dispatched.filter((e): e is Apply => e.type === 'apply_settings');
}

/** Öffnet die Einstellungen über die Kopfzeile (Mock-Kern). */
async function open(options: SettingsMockOptions & { isMac?: boolean } = {}, lang: 'de' | 'en' = 'de') {
  const r = renderApp(options);
  const name = lang === 'en' ? 'Settings' : 'Einstellungen';
  await r.user.click(await screen.findByRole('button', { name }));
  const dialog = await screen.findByRole('dialog', { name });
  return { ...r, dialog };
}

function nav(dialog: HTMLElement, name: string | RegExp) {
  return within(within(dialog).getByRole('navigation', { name: /Bereiche|Sections/ })).getByRole('button', { name });
}

function preview(dialog: HTMLElement) {
  return within(dialog).getByRole('img', { name: /^Vorschau des Signaturblocks/ });
}

describe('Rahmen und sofortige Wirkung', () => {
  it('Seitenleiste mit Bereichen; „Dokument“ direkt nach „Signaturbild“; kein Speichern/Abbrechen', async () => {
    const { dialog, mock, user } = await open();
    expect(within(dialog).getByText('Änderungen gelten sofort.')).toBeInTheDocument();
    const names = within(within(dialog).getByRole('navigation', { name: 'Bereiche' }))
      .getAllByRole('button')
      .map((b) => b.textContent);
    expect(names).toEqual(['Signaturbild', 'Dokument', 'Signiermittel', 'Speichern', 'Darstellung', 'Datenschutz', 'Erweitert', 'Über PIDA']);
    expect(nav(dialog, 'Signaturbild')).toHaveAttribute('aria-current', 'true');
    // „Speichern“ ist nur der Bereich, kein Knopf zum Übernehmen; kein „Abbrechen“
    expect(within(dialog).getAllByRole('button', { name: 'Speichern' })).toEqual([nav(dialog, 'Speichern')]);
    expect(within(dialog).queryByRole('button', { name: 'Abbrechen' })).toBeNull();
    // Schließen sendet nur `close_settings` – es gibt nichts zu verwerfen
    await user.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(mock.dispatched.map((e) => e.type)).toEqual(['open_settings', 'close_settings']);
  });

  it('Signaturbild: Profil, Blocksprache, Hinweistext wirken sofort (je Änderung ein apply_settings), Vorschau neu', async () => {
    const { dialog, mock, user } = await open();
    expect(preview(dialog)).toHaveAccessibleName('Vorschau des Signaturblocks: Standard (klein), Wie System');
    const src = preview(dialog).getAttribute('src');

    await user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.profile': 'AMTSSIGNATURBLOCK' }, revision: 1 });
    expect(preview(dialog)).toHaveAccessibleName(/Amtssignatur/);
    expect(preview(dialog).getAttribute('src')).not.toBe(src);

    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.block_language': 'en' }, revision: 2 });
    const note = within(dialog).getByLabelText('Hinweistext');
    expect(note).toHaveValue(translator('en').t('simple_config-Note_Default_OfficialSignature'));
    expect(within(dialog).queryByRole('button', { name: 'Standardtext wiederherstellen' })).toBeNull();

    await user.clear(note);
    await user.type(note, 'Intern – nur zur Ablage');
    await waitFor(() => expect(lastEvent(mock)).toMatchObject({ type: 'apply_settings', changes: { 'signature.note': { mode: 'custom', text: 'Intern – nur zur Ablage' } } }));
    // entprellt: kein Ereignis je Tastendruck
    expect(applies(mock.dispatched).filter((e) => 'signature.note' in e.changes).length).toBeLessThanOrEqual(2);
    await user.click(within(dialog).getByRole('button', { name: 'Standardtext wiederherstellen' }));
    expect(lastEvent(mock)).toMatchObject({ type: 'apply_settings', changes: { 'signature.note': { mode: 'default' } } });
    await waitFor(() => expect(note).toHaveValue(translator('en').t('simple_config-Note_Default_OfficialSignature')));
    // jedes Ereignis trägt nur die geänderten Pfade (keine Kernänderung wird überschrieben, ADR 0004 §3.11)
    for (const e of applies(mock.dispatched)) expect(Object.keys(e.changes)).toHaveLength(1);
    expect(mock.dispatched.some((e) => e.type === 'save_settings')).toBe(false);
  });

  it('der offene Arbeitsbereich übernimmt das Signaturbild sofort (Blockgröße, Vorschau)', async () => {
    const r = renderApp({ scenario: 'contract' });
    await r.user.click(await screen.findByRole('button', { name: 'Signaturbild bearbeiten' }));
    const before = (await r.mock.view()).document!.block;
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await r.user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    const after = (await r.mock.view()).document!.block;
    expect(after.width).toBeGreaterThan(before.width);
    expect(after.preview_revision).toBeGreaterThan(before.preview_revision);
    await r.user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('wartende Texteingabe wird beim Schließen vor `close_settings` gesendet', async () => {
    const { dialog, mock, user } = await open();
    await user.click(nav(dialog, 'Speichern'));
    const postfix = within(dialog).getByLabelText('Dateinamenzusatz');
    await user.clear(postfix);
    await user.type(postfix, '_final');
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    const types = mock.dispatched.map((e) => e.type);
    expect(types.slice(-2)).toEqual(['apply_settings', 'close_settings']);
    expect((await mock.view()).settings).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const again = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(again, 'Speichern'));
    expect(within(again).getByLabelText('Dateinamenzusatz')).toHaveValue('_final');
  });
});

describe('Teilübernahme und Feldprobleme (Abnahme E3 Punkt 2)', () => {
  it('ungültiger Proxy-Port: Feldfehler und Eingabe bleiben, andere Änderungen werden übernommen; Neuöffnen zeigt die Werte', async () => {
    const { dialog, mock, user } = await open();
    await user.click(nav(dialog, 'Erweitert'));
    await user.click(within(dialog).getByRole('radio', { name: 'Manuell' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'proxy.mode': 'manual' }, revision: 1 });
    // Hinweis des Kerns am Host (manueller Proxy ohne Host)
    expect(await within(dialog).findByText('Für einen manuellen Proxy ist ein Host anzugeben.')).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText('Host'), 'proxy.local');
    await user.type(within(dialog).getByLabelText('Port'), '9x9999');
    await user.tab();
    expect(await within(dialog).findByText('Der Port muss zwischen 1 und 65535 liegen.')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Port')).toHaveValue('99999');
    expect(within(dialog).getByLabelText('Port')).toHaveAttribute('aria-invalid', 'true');
    expect(within(dialog).queryByText('Für einen manuellen Proxy ist ein Host anzugeben.')).toBeNull();

    // andere Änderung wird trotzdem übernommen, das Feldproblem bleibt
    await user.click(nav(dialog, 'Signaturbild'));
    await user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    const v = (await mock.view()).settings!;
    expect(v.settings.signature.profile).toBe('AMTSSIGNATURBLOCK');
    expect(v.settings.proxy).toEqual({ mode: 'manual', host: 'proxy.local', port: null, user: null });
    expect(v.problems).toEqual({ 'proxy.port': 'ng-settings-invalid-port' });
    // nie ganze Einstellungen, nie Felder, die der Kern selbst ändert
    for (const e of applies(mock.dispatched)) expect(Object.keys(e.changes).every((k) => !k.startsWith('general.last_source'))).toBe(true);

    // schließen und neu öffnen: übernommene Werte
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const again = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(within(again).getByRole('radio', { name: /Amtssignatur/ })).toBeChecked();
    await user.click(nav(again, 'Erweitert'));
    expect(within(again).getByLabelText('Host')).toHaveValue('proxy.local');
    expect(within(again).getByLabelText('Port')).toHaveValue('');
  });

  it('Antworten mit älterer `applied_revision` überschreiben eine gesendete Änderung nicht', async () => {
    const view = settingsView();
    const { mock, user } = renderApp({ view });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(dialog, 'Dokument'));
    const pdfa = within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' });
    await user.click(pdfa);
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.pdfa_force': true }, revision: 1 });
    // der feste Zustand antwortet noch mit Revision 0: der gesendete Wert bleibt sichtbar
    act(() => mock.setView(settingsView()));
    expect(within(screen.getByRole('dialog')).getByRole('switch', { name: 'PDF/A-kompatibel signieren' })).toBeChecked();
    // Bestätigung (Revision 1) ohne Übernahme (z. B. Richtlinie): der Kern ist wieder maßgeblich
    act(() => mock.setView(settingsView(defaultSettings(), { applied_revision: 1 })));
    await waitFor(() => expect(within(screen.getByRole('dialog')).getByRole('switch', { name: 'PDF/A-kompatibel signieren' })).not.toBeChecked());
  });
});

describe('Suche (Abnahme E3 Punkt 3)', () => {
  it('„Logo“ findet die Bildmarke und zeigt die Vorschau mit; Löschen-Knopf; Trefferzahl', async () => {
    const { dialog, user } = await open();
    const search = within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' });
    await user.type(search, 'Logo');
    expect(within(dialog).queryByRole('navigation')).toBeNull();
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Signaturbild' })).toBeInTheDocument();
    expect(within(dialog).getByText('Bildmarke')).toBeInTheDocument();
    expect(preview(dialog)).toBeInTheDocument();
    expect(within(dialog).queryByRole('radiogroup', { name: 'Profil' })).toBeNull();
    expect(within(dialog).getByRole('status')).toHaveTextContent('Ein Treffer');
    await user.click(within(dialog).getByRole('button', { name: 'Suche löschen' }));
    expect(within(dialog).getByRole('navigation', { name: 'Bereiche' })).toBeInTheDocument();
  });

  it('„Handy“ findet das ID-Austria-Konto, „Proxy“ den Proxy; ohne Treffer ein Hinweis', async () => {
    const { dialog, user } = await open();
    const search = within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' });
    await user.type(search, 'Handy');
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Signiermittel' })).toBeInTheDocument();
    expect(within(dialog).getByRole('textbox', { name: 'ID Austria: Telefonnummer oder Benutzername' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('heading', { level: 3, name: 'Signaturbild' })).toBeNull();
    await user.clear(search);
    await user.type(search, 'proxy');
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Erweitert' })).toBeInTheDocument();
    expect(within(dialog).getByRole('radiogroup', { name: 'Proxy' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('heading', { level: 3, name: 'Signaturbild' })).toBeNull();
    await user.clear(search);
    await user.type(search, 'xyz');
    expect(within(dialog).getByText('Keine Einstellung gefunden für „xyz“.')).toBeInTheDocument();
    expect(within(dialog).getByRole('status')).toHaveTextContent('Keine Treffer');
  });

  it('Treffer sind bedienbar und wirken sofort', async () => {
    const { dialog, mock, user } = await open();
    await user.type(within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'dunkel');
    await user.click(within(dialog).getByRole('radio', { name: 'Dunkel' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    expect(mock.dispatched.filter((e) => e.type === 'apply_settings')).toEqual([]);
  });
});

describe('Einstellungen › Dokument (PDF/A, Platzhaltersuche)', () => {
  it('Standardwerte wie PDF-Over: PDF/A aus, Suche aus (Auswahl gesperrt, QR vorgewählt); nicht unter „Erweitert“', async () => {
    const { dialog, user } = await open();
    await user.click(nav(dialog, 'Dokument'));
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Dokument' })).toBeInTheDocument();
    const pdfa = within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' });
    expect(pdfa).not.toBeChecked();
    expect(pdfa).toHaveAccessibleDescription(/erkennt PIDA automatisch .* erzwingt das auch für Dokumente ohne PDF\/A-Kennung/);
    expect(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' })).not.toBeChecked();
    const kind = within(dialog).getByRole('radiogroup', { name: 'Suchen nach' });
    expect(kind).toHaveAttribute('aria-disabled', 'true');
    expect(within(kind).getByRole('radio', { name: /QR-Code-Platzhalter/ })).toBeChecked();
    expect(within(kind).getByRole('radio', { name: /Leere Signaturfelder/ })).not.toBeChecked();
    await user.click(nav(dialog, 'Erweitert'));
    expect(within(dialog).queryByRole('switch', { name: /Platzhalter|PDF\/A/ })).toBeNull();
    expect(within(dialog).queryByRole('radiogroup', { name: 'Suchen nach' })).toBeNull();
  });

  it('Schalter wirken sofort; die Auswahl ist bei ausgeschalteter Suche gesperrt', async () => {
    const { dialog, mock, user } = await open();
    await user.click(nav(dialog, 'Dokument'));
    await user.click(within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.pdfa_force': true }, revision: 1 });
    const search = within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' });
    const group = within(dialog).getByRole('radiogroup', { name: 'Suchen nach' });
    const field = within(group).getByRole('radio', { name: /Leere Signaturfelder/ });
    await user.click(field);
    expect(field).not.toBeChecked();
    await user.click(search);
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'placeholder.search': true }, revision: 2 });
    expect(group).not.toHaveAttribute('aria-disabled');
    await user.click(field);
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'placeholder.kind': 'fields' }, revision: 3 });
    expect(field).toBeChecked();
    await user.click(search);
    expect(group).toHaveAttribute('aria-disabled', 'true');
    expect(field).toBeChecked();
    expect((await mock.view()).settings!.settings.placeholder).toEqual({ search: false, kind: 'fields' });
  });

  it('auffindbar über „pdf/a“, „platzhalter“, „qr“, „signaturfeld“ (nur der passende Schalter)', async () => {
    const { dialog, user } = await open();
    const search = within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' });
    for (const [q, control] of [
      ['pdf/a', 'PDF/A-kompatibel signieren'],
      ['platzhalter', 'Signaturplatzhalter automatisch suchen'],
      ['qr', 'Signaturplatzhalter automatisch suchen'],
      ['signaturfeld', 'Signaturplatzhalter automatisch suchen'],
    ] as const) {
      await user.clear(search);
      await user.type(search, q);
      expect(within(dialog).getByRole('heading', { level: 3, name: 'Dokument' })).toBeInTheDocument();
      expect(within(dialog).getAllByRole('switch').map((s) => s.closest('.settings-row')?.getAttribute('data-row'))).toEqual([control === 'PDF/A-kompatibel signieren' ? 'document.pdfa' : 'document.placeholder']);
      expect(within(dialog).queryByRole('heading', { level: 3, name: 'Erweitert' })).toBeNull();
    }
  });

  it('englisch: „signature field“, „pdf/a“', async () => {
    const { dialog, user } = await open({ language: 'en' }, 'en');
    const search = within(dialog).getByRole('searchbox', { name: 'Search settings' });
    await user.type(search, 'signature field');
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Document' })).toBeInTheDocument();
    expect(within(dialog).getByRole('switch', { name: 'Search for signature placeholders automatically' })).toBeInTheDocument();
    await user.clear(search);
    await user.type(search, 'pdf/a');
    expect(within(dialog).getByRole('switch', { name: 'PDF/A-compatible signature' })).toBeInTheDocument();
  });
});

describe('Signaturbild: Bildmarke, Profile, Logo-Größe', () => {
  it('ohne eigene Bildmarke das PIDA-Siegel; Datei wählen, Standard verwenden, Ablegen (ohne Profilwechsel)', async () => {
    const { dialog, mock, user } = await open({ pickFilesResult: ['/Users/demo/logo.png'] });
    const zone = within(dialog).getByRole('group', { name: 'Bildmarke' });
    expect(zone).toHaveAttribute('data-emblem-drop');
    expect(within(zone).getByText('Standard-Bildmarke (PIDA-Siegel)')).toBeInTheDocument();
    await user.click(within(zone).getByRole('button', { name: 'Datei wählen …' }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.emblem': '/Users/demo/logo.png' }, revision: 1 }));
    expect(mock.nativeCalls).toContain('pickFiles:single');
    expect(within(zone).getByText('Aktuell: logo.png')).toBeInTheDocument();
    await user.click(within(zone).getByRole('button', { name: 'Standard verwenden' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.emblem': null }, revision: 2 });
    expect(within(zone).getByText('Standard-Bildmarke (PIDA-Siegel)')).toBeInTheDocument();
    act(() => {
      window.dispatchEvent(new CustomEvent(EMBLEM_DROP_EVENT, { detail: '/Users/demo/drop.gif' }));
    });
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.emblem': '/Users/demo/drop.gif' }, revision: 3 });
    expect((await mock.view()).settings!.settings.signature.profile).toBe('SIGNATURBLOCK_SMALL');
  });

  it('„Weitere Profile“: Nur Bildmarke mit Größenregler (cm, gesendet in mm), Unsichtbar ohne Vorschau', async () => {
    const { dialog, mock, user } = await open();
    expect(within(dialog).queryByRole('radio', { name: /Nur Bildmarke/ })).toBeNull();
    const more = within(dialog).getByRole('button', { name: 'Weitere Profile' });
    expect(more).toHaveAttribute('aria-expanded', 'false');
    await user.click(more);
    await user.click(within(dialog).getByRole('radio', { name: /Nur Bildmarke/ }));
    expect(within(dialog).queryByLabelText('Hinweistext')).toBeNull();
    expect(within(dialog).queryByRole('radiogroup', { name: 'Sprache des Signaturblocks' })).toBeNull();
    const slider = within(dialog).getByRole('slider', { name: /Größe der Bildmarke/ });
    expect(slider).toHaveAttribute('aria-valuetext', '2,3 cm');
    slider.focus();
    await user.keyboard('{ArrowRight}{ArrowRight}');
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'signature.logo_only_size_mm': 25 }, revision: expect.any(Number) }));
    expect(slider).toHaveAttribute('aria-valuetext', '2,5 cm');
    await user.click(within(dialog).getByRole('radio', { name: /Unsichtbar/ }));
    expect(within(dialog).queryByRole('img', { name: /Vorschau/ })).toBeNull();
    expect(within(dialog).getByText('Unsichtbare Signatur – auf dem Dokument erscheint kein Signaturbild.')).toBeInTheDocument();
    expect(within(dialog).queryByRole('group', { name: 'Bildmarke' })).toBeNull();
  });
});

describe('Signiermittel und Speichern', () => {
  it('Standard-Signiermittel, ID-Austria-Konto, FIDO2; Keystore erst nach dem Einrichten', async () => {
    const { dialog, mock, user } = await open({ pickFilesResult: ['/Users/demo/Zertifikate/max.p12'] });
    await user.click(nav(dialog, 'Signiermittel'));
    const group = within(dialog).getByRole('radiogroup', { name: 'Standard-Signiermittel' });
    expect(within(group).getByRole('radio', { name: /Jedes Mal fragen/ })).toBeChecked();
    expect(within(group).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
    await user.click(within(group).getByRole('radio', { name: /ID Austria/ }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'backend.default': 'id_austria' }, revision: 1 });

    await user.type(within(dialog).getByLabelText('ID Austria: Telefonnummer oder Benutzername'), '+43 660 1234123');
    await user.tab();
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'id_austria.username': '+43 660 1234123' }, revision: 2 }));
    await user.click(within(dialog).getByRole('switch', { name: 'Benutzername merken' }));
    await user.click(within(dialog).getByRole('switch', { name: 'FIDO2-Sicherheitsschlüssel bevorzugen' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'id_austria.fido2_default': true }, revision: 4 });

    // Keystore einrichten: Datei (Format erkennt der Kern am Inhalt), Speicherart
    const keystore = within(dialog).getByRole('region', { name: 'Keystore' });
    await user.click(within(keystore).getByRole('switch', { name: 'Keystore verwenden' }));
    // aktiviert, aber noch ohne Datei: kein Fehler, als Standard weiterhin nicht wählbar
    expect(within(keystore).getByText('Keine Datei gewählt')).toBeInTheDocument();
    expect(within(keystore).queryByText(/nicht gefunden|existiert nicht/)).toBeNull();
    expect(within(group).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
    expect(within(group).getByText('Zuerst unten den Keystore einrichten.')).toBeInTheDocument();
    await user.click(within(keystore).getByRole('button', { name: 'Datei wählen …' }));
    // nur die Datei – kein `keystore.declared_type` mehr (bleibt nur für PDF-Over gespeichert)
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'keystore.file': '/Users/demo/Zertifikate/max.p12' }, revision: 6 }));
    expect(within(keystore).getByText('/Users/demo/Zertifikate/max.p12')).toBeInTheDocument();
    expect(await within(keystore).findByText('Format: PKCS#12')).toBeInTheDocument();
    expect(within(keystore).queryByRole('radiogroup', { name: 'Typ' })).toBeNull();
    expect(within(keystore).getByRole('button', { name: 'Schlüssel anzeigen …' })).toBeInTheDocument();
    await user.click(within(keystore).getByRole('radio', { name: 'Nicht speichern' }));
    await waitFor(async () =>
      expect((await mock.view()).settings!.settings.keystore).toEqual({ enabled: true, file: '/Users/demo/Zertifikate/max.p12', declared_type: null, alias: null, password_storage: 'none', storage_chosen: true }),
    );
    expect(within(group).getByRole('radio', { name: /Keystore/ })).toBeEnabled();
    await user.click(within(group).getByRole('radio', { name: /Keystore/ }));
    // Keystore aus: die Voreinstellung fällt auf „Jedes Mal fragen“ zurück (ein Ereignis)
    await user.click(within(keystore).getByRole('switch', { name: 'Keystore verwenden' }));
    expect(lastEvent(mock)).toMatchObject({ type: 'apply_settings', changes: { 'keystore.enabled': false, 'backend.default': null } });
    expect(within(group).getByRole('radio', { name: /Jedes Mal fragen/ })).toBeChecked();
  });

  it('Schlüsselbund nicht verfügbar: gesperrt mit Hinweis; FIDO2 nicht verfügbar: Hinweis statt Schalter', async () => {
    const base = defaultSettings();
    const { user } = renderApp({ view: settingsView({ ...base, keystore: { ...base.keystore, enabled: true, file: '/k.p12' } }, { keychain_available: false, fido2_available: false }) });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(dialog, 'Signiermittel'));
    expect(within(dialog).getByRole('radio', { name: 'Schlüsselbund' })).toBeDisabled();
    expect(within(dialog).getByText('Kein Schlüsselbund verfügbar: Passwörter werden nur bis zum Programmende gespeichert.')).toBeInTheDocument();
    expect(within(dialog).queryByRole('switch', { name: /FIDO2/ })).toBeNull();
    expect(within(dialog).getByText('Ihr System unterstützt keine Authentifizierung mittels FIDO2-Token.')).toBeInTheDocument();
  });

  it('gewählte Keystore-Datei verschwunden: verständlicher Hinweis ohne Platzhalter', async () => {
    const base = defaultSettings();
    const { user } = renderApp({
      view: settingsView({ ...base, keystore: { ...base.keystore, enabled: true, file: '/weg/max.p12' } }, { keystore_ready: false, problems: { 'keystore.file': 'ng-keystore-file-not-found' } }),
    });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(dialog, 'Signiermittel'));
    const keystore = within(dialog).getByRole('region', { name: 'Keystore' });
    expect(within(keystore).getByText('Die gewählte Keystore-Datei wurde nicht gefunden. Bitte wählen Sie sie erneut aus.')).toHaveAttribute('role', 'alert');
    expect(within(keystore).queryByText(/\$arg1|existiert nicht/)).toBeNull();
    expect(within(within(dialog).getByRole('radiogroup', { name: 'Standard-Signiermittel' })).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
  });

  it('Speichern: neben dem Original, fester Ordner (Dialog), jedes Mal fragen; Namenszusatz mit Beispiel des Kerns', async () => {
    const { dialog, mock, user } = await open({ pickDirectoryResult: '/Users/demo/Signiert' });
    await user.click(nav(dialog, 'Speichern'));
    const target = within(dialog).getByRole('radiogroup', { name: 'Speicherort' });
    expect(within(target).getByRole('radio', { name: /Jedes Mal fragen/ })).toBeChecked();
    await user.click(within(target).getByRole('radio', { name: /Neben dem Original/ }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'output.folder': '.' }, revision: 1 });
    await user.click(within(target).getByRole('radio', { name: /In festem Ordner/ }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'output.folder': '/Users/demo/Signiert' }, revision: 2 }));
    expect(mock.nativeCalls).toContain('pickDirectory');
    expect(within(target).getByRole('radio', { name: /In festem Ordner/ })).toBeChecked();
    expect(within(dialog).getAllByText('/Users/demo/Signiert').length).toBeGreaterThan(0);

    expect(within(dialog).getByText('Vertrag_signed.pdf')).toBeInTheDocument();
    const postfix = within(dialog).getByLabelText('Dateinamenzusatz');
    await user.clear(postfix);
    await user.type(postfix, '_final');
    expect(await within(dialog).findByText('Vertrag_final.pdf')).toBeInTheDocument();
    // leer bedeutet „_signed“ (erst beim Verlassen gesendet)
    await user.clear(postfix);
    await user.tab();
    await waitFor(() => expect(lastEvent(mock)).toMatchObject({ type: 'apply_settings', changes: { 'output.postfix': '_signed' } }));
    expect(postfix).toHaveValue('_signed');
  });
});

describe('Darstellung', () => {
  it('Farbschema lokal und sofort; Oberflächensprache über general.ui_language', async () => {
    const { dialog, mock, user } = await open();
    await user.click(nav(dialog, 'Darstellung'));
    await user.click(within(dialog).getByRole('radio', { name: 'Dunkel' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    expect(localStorage.getItem('pida.theme')).toBe('dark');
    const lang = within(dialog).getByRole('radiogroup', { name: 'Sprache der Oberfläche' });
    await user.click(within(lang).getByRole('radio', { name: 'English' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'general.ui_language': 'en' }, revision: 1 });
    expect(await screen.findByRole('dialog', { name: 'Settings' })).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute('lang', 'en');
  });

  it('Kopfzeile und Einstellungen teilen das Farbschema (App-Kontext, Merge E3)', async () => {
    localStorage.removeItem('pida.theme');
    const { dialog, user } = await open();
    await user.click(nav(dialog, 'Darstellung'));
    await user.click(within(dialog).getByRole('radio', { name: 'Dunkel' }));
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    // Menü der Kopfzeile zeigt sofort „Dunkel“ (vorher erst nach einem Neustart)
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Farbschema' }));
    const menu = await screen.findByRole('menu', { name: 'Farbschema' });
    expect(within(menu).getByRole('menuitemradio', { name: 'Dunkel' })).toHaveAttribute('aria-checked', 'true');
    // und umgekehrt: Wahl in der Kopfzeile erscheint in den Einstellungen
    await user.click(within(menu).getByRole('menuitemradio', { name: 'Hell' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'light');
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const again = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(again, 'Darstellung'));
    expect(within(again).getByRole('radio', { name: 'Hell' })).toBeChecked();
    localStorage.removeItem('pida.theme');
  });
});

describe('Datenschutz (Abnahme E3 Punkt 4)', () => {
  it('Text, lokales Signieren, A-Trust-Link, Zertifikats-Cache, „Zuletzt signiert merken“, „Liste leeren“', async () => {
    const { dialog, mock, user } = await open({ examples: true });
    await user.click(nav(dialog, 'Datenschutz'));
    expect(within(dialog).getByRole('heading', { name: 'Datenschutzinformation' })).toBeInTheDocument();
    expect(within(dialog).getByText(/Die Software PIDA verarbeitet .*Beim Signaturvorgang mittels ID Austria wird das zu signierende PDF-Dokument an die Firma A-Trust gesendet/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Mit einem Keystore oder der Chipkarte/)).toBeInTheDocument();
    const links = within(dialog).getAllByRole('link');
    expect(links).toHaveLength(1);
    await user.click(links[0] as HTMLElement);
    expect(mock.openedUrls).toEqual(['https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf']);

    await user.click(within(dialog).getByRole('switch', { name: 'Signaturzertifikat zwischenspeichern' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'certificate_cache.enabled': true }, revision: 1 });

    const remember = within(dialog).getByRole('switch', { name: '„Zuletzt signiert“ merken' });
    expect(remember).toBeChecked();
    expect(within(dialog).getByText('3 Einträge in der Liste.')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Liste leeren' }));
    expect(lastEvent(mock)).toEqual({ type: 'clear_recent' });
    expect(await within(dialog).findByText('Die Liste ist leer.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Liste leeren' })).toBeDisabled();
    expect(await screen.findByText('Die Liste „Zuletzt signiert“ ist geleert.')).toBeInTheDocument();

    // Abschalten löscht sofort (Kern), die Startansicht zeigt die Karte nicht mehr
    await user.click(remember);
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'general.recent_documents': false }, revision: 2 });
    expect((await mock.view()).recent).toBeNull();
    expect(within(dialog).queryByRole('button', { name: 'Liste leeren' })).toBeNull();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.queryByTestId('recent-panel')).not.toBeInTheDocument();
  });

  it('eigener Datenschutztext per Richtlinie, dazu der Link zu A-Trust (EN)', async () => {
    const { user } = renderApp({ view: { ...settingsView(defaultSettings(), {}, 'en'), privacy_text: 'Notice of the organisation\nSecond line' } });
    const dialog = await screen.findByRole('dialog', { name: 'Settings' });
    await user.click(nav(dialog, 'Privacy'));
    expect(within(dialog).getByText(/Notice of the organisation/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/The software PIDA processes/)).toBeNull();
    expect(within(dialog).getByText(/signed on this device and is not uploaded/)).toBeInTheDocument();
    expect(within(dialog).getByRole('link', { name: /datenschutzerklaerung\.pdf/ })).toBeInTheDocument();
  });

  it('Richtlinie: „Zuletzt signiert“ gesperrt mit Hinweis', async () => {
    const draft: SettingsView = { ...settingsView().settings!, locked: { 'general.recent_documents': 'ng-recent-policy' } };
    const { user } = renderApp({ view: { ...settingsView(), settings: draft, recent: null } });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(dialog, 'Datenschutz'));
    const sw = within(dialog).getByRole('switch', { name: '„Zuletzt signiert“ merken' });
    expect(sw).toBeDisabled();
    expect(sw).not.toBeChecked();
    expect(sw).toHaveAccessibleDescription(/„Zuletzt signiert“ ist von Ihrer Organisation abgeschaltet\./);
  });

  it('„Details“ des Kurzhinweises öffnet die Einstellungen beim Abschnitt Datenschutz', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    const note = await screen.findByTestId('privacy-note');
    await user.click(within(note).getByRole('link', { name: 'Details zum Datenschutz' }));
    expect(lastEvent(mock)).toEqual({ type: 'open_settings' });
    act(() => mock.setView({ ...workspaceView(), settings: settingsView().settings }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(nav(dialog, 'Datenschutz')).toHaveAttribute('aria-current', 'true');
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Datenschutz' })).toBeInTheDocument();
  });
});

describe('Erweitert und Über', () => {
  it('Update-Prüfung, Zurücksetzen mit Rückfrage; kein Signaturformat (immer PDF-AS-Verfahren)', async () => {
    const { dialog, mock, user } = await open();
    await user.click(nav(dialog, 'Erweitert'));
    expect(within(dialog).queryByText(/Signaturformat|PAdES-BASELINE-B/)).toBeNull();
    await user.click(within(dialog).getByRole('switch', { name: 'Automatisch nach Updates suchen' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'general.update_check': false }, revision: 1 });
    await user.click(within(dialog).getByRole('button', { name: /Einstellungen zurücksetzen/ }));
    const confirm = await screen.findByRole('alertdialog', { name: /Einstellungen zurücksetzen/ });
    await user.click(within(confirm).getByRole('button', { name: 'Zurücksetzen' }));
    expect(lastEvent(mock)).toEqual({ type: 'reset_settings' });
  });

  it('Über: Version, Lizenz- und Supportlinks, Drittlizenzen, Protokolle, Verweis auf Datenschutz', async () => {
    const { dialog, mock, user } = await open();
    await user.click(nav(dialog, 'Über PIDA'));
    expect(await within(dialog).findByText('Version 0.1.0')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('link', { name: /software@egiz\.gv\.at/ }));
    expect(mock.openedUrls).toEqual(['mailto:software@egiz.gv.at']);
    await user.click(within(dialog).getByRole('button', { name: 'Drittlizenzen anzeigen' }));
    await waitFor(() => expect(mock.nativeCalls).toContain('openNotices'));
    await user.click(within(dialog).getByRole('button', { name: /Logs anzeigen/ }));
    await waitFor(() => expect(mock.nativeCalls).toContain('openLogDir'));
    await user.click(within(dialog).getByRole('link', { name: 'Datenschutz' }));
    expect(nav(dialog, 'Datenschutz')).toHaveAttribute('aria-current', 'true');
    await waitFor(() => expect(within(dialog).getByRole('heading', { name: 'Datenschutzinformation' })).toHaveFocus());
  });
});

describe('Schreibgeschützt (Abnahme E3 Punkt 6)', () => {
  it('Banner; Änderungen gelten trotzdem sofort (bis Programmende, Kern)', async () => {
    const { mock, user } = renderApp({ readOnly: true });
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(within(dialog).getByRole('note')).toHaveTextContent('Diese Einstellungen werden nicht gespeichert');
    expect(within(dialog).getByRole('note')).toHaveTextContent('Änderungen gelten bis zum Programmende.');
    await user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    expect((await mock.view()).settings!.settings.signature.profile).toBe('AMTSSIGNATURBLOCK');
  });
});

describe('Mobile (Abnahme E3 Punkt 7)', () => {
  it('Vollbild ohne Proxy, Ordner, Update-Prüfung, Protokolle, Chipkarte und Bildmarke', async () => {
    const { user } = renderApp({ view: onMobile(settingsView(defaultSettings(), { fido2_available: false }), IOS) });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(document.documentElement).toHaveAttribute('data-mobile');
    expect(dialog.closest('.settings-modal')).not.toBeNull();
    expect(within(dialog).queryByRole('group', { name: 'Bildmarke' })).toBeNull();
    await user.click(nav(dialog, 'Signiermittel'));
    expect(within(dialog).queryByRole('radio', { name: /Chipkarte/ })).toBeNull();
    expect(within(dialog).queryByRole('region', { name: 'Chipkarte' })).toBeNull();
    expect(within(dialog).getByText(/FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar/)).toBeInTheDocument();
    const keystore = within(dialog).getByRole('region', { name: 'Keystore' });
    expect(within(keystore).getByRole('button', { name: 'PKCS#12-Datei importieren …' })).toBeInTheDocument();
    expect(within(keystore).queryByRole('button', { name: 'Datei wählen …' })).toBeNull();
    expect(within(keystore).queryByRole('switch')).toBeNull();
    await user.click(nav(dialog, 'Speichern'));
    expect(within(dialog).queryByRole('radiogroup', { name: 'Speicherort' })).toBeNull();
    expect(within(dialog).getByText('Das signierte Dokument liegt in den Dokumenten dieser App.')).toBeInTheDocument();
    await user.click(nav(dialog, 'Erweitert'));
    expect(within(dialog).queryByRole('radiogroup', { name: 'Proxy' })).toBeNull();
    expect(within(dialog).queryByRole('switch', { name: /Updates/ })).toBeNull();
    await user.click(nav(dialog, 'Über PIDA'));
    expect(within(dialog).queryByRole('button', { name: /Logs/ })).toBeNull();
    expect(within(dialog).getByRole('button', { name: 'Drittlizenzen anzeigen' })).toBeInTheDocument();
  });

  it('Keystore nur per Import: Assistent im neuen Stil, danach Datei und Alias in den Einstellungen', async () => {
    const settings: Settings = defaultSettings();
    const { mock, user } = renderApp({ platform: 'android', settings });
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(nav(dialog, 'Signiermittel'));
    const keystore = within(dialog).getByRole('region', { name: 'Keystore' });
    expect(keystore).toHaveTextContent('Noch kein Keystore auf diesem Gerät.');
    await user.click(within(keystore).getByRole('button', { name: 'PKCS#12-Datei importieren …' }));
    const wizard = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    expect(within(wizard).getByLabelText('Passwort des Keystores')).toBeDisabled();
    await user.click(within(wizard).getByRole('button', { name: 'Datei wählen …' }));
    expect(await within(wizard).findByTestId('keystore-import-file')).toHaveTextContent('Gewählte Datei: test.p12');
    await user.type(within(wizard).getByLabelText('Passwort des Keystores'), 'test');
    expect(within(wizard).getByRole('switch', { name: 'Passwort sicher auf diesem Gerät speichern' })).toBeChecked();
    await user.click(within(wizard).getByRole('button', { name: 'Importieren' }));
    expect(mock.dispatched.find((e) => e.type === 'import_keystore')).toMatchObject({ password: 'test', remember: true });
    const done = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    expect(within(done).getByRole('status')).toHaveTextContent('test.p12');
    await user.click(within(done).getByRole('button', { name: 'Fertig' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
    expect(within(keystore).getByText('Importierte Datei: test.p12')).toBeInTheDocument();
    // mit Passwort importiert: die Schlüssel sind bekannt – Auswahl statt Textfeld (zwei Schlüssel)
    expect(within(keystore).getByRole('button', { name: /Schlüssel/ })).toHaveTextContent('Schlüssel wählen');
    expect(within(keystore).queryByRole('textbox', { name: 'Alias' })).toBeNull();
  });
});
