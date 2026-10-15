import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import type { Settings, UiEvent } from '../core/types';
import { renderApp } from '../test/render';
import { settingsView } from '../test/views';
import { EMBLEM_DROP_EVENT, keystoreTypeFromFile, logoCm } from './SettingsDialog';

function savedSettings(dispatched: unknown[]): Settings {
  const ev = dispatched.find((e) => (e as UiEvent).type === 'save_settings') as Extract<UiEvent, { type: 'save_settings' }> | undefined;
  if (!ev) throw new Error('save_settings fehlt');
  return ev.settings;
}

describe('Einstellungen', () => {
  it('modaler Dialog mit Tabs Einfach/Erweitert/FIDO2/Über; Keystore-Tab nur wenn aktiviert', async () => {
    renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog', { name: 'Konfiguration' });
    const tabs = within(dialog).getAllByRole('tab').map((t) => t.textContent);
    expect(tabs).toEqual(['Einfach', 'Erweitert', 'FIDO2', 'Über PDF-Over NG']);
    expect(within(dialog).getByRole('tab', { name: 'Einfach' })).toHaveAttribute('aria-selected', 'true');
  });

  it('Speichern sendet die Arbeitskopie – unbekannte Einträge bleiben erhalten', async () => {
    const settings: Settings = { ...defaultSettings(), 'signature.future_key': 'bleibt', future: 42 };
    const { mock, user } = renderApp({ view: settingsView(settings) });
    const dialog = await screen.findByRole('dialog');
    await user.type(within(dialog).getByLabelText('Benutzername:'), '+43664123');
    await user.click(within(dialog).getByRole('radio', { name: 'Amtssignatur' }));
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const saved = savedSettings(mock.dispatched);
    expect(saved['signature.future_key']).toBe('bleibt');
    expect(saved.future).toBe(42);
    expect(saved.id_austria.username).toBe('+43664123');
    expect(saved.signature.profile).toBe('AMTSSIGNATURBLOCK');
    expect(saved.output.postfix).toBe('_signed');
  });

  it('Abbrechen und Escape verwerfen', async () => {
    const { mock, user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    await user.keyboard('{Escape}');
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'close_settings' }, { type: 'close_settings' }]));
  });

  it('Signaturhinweis: Standardtext, eigener Text, leer = kein Hinweis, Standard wiederherstellen', async () => {
    const { mock, user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    const note = within(dialog).getByLabelText('Text:') as HTMLTextAreaElement;
    // Systemsprache des Blocks = UI-Sprache DE
    expect(note.value).toContain('qualifizierten elektronischen Signatur');
    const restore = within(dialog).getByRole('button', { name: 'Standard wiederherstellen' });
    expect(restore).toBeDisabled();
    await user.clear(note);
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched).signature.note).toEqual({ mode: 'none' });
    await user.type(note, 'Eigener Hinweis');
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched.slice(1)).signature.note).toEqual({ mode: 'custom', text: 'Eigener Hinweis' });
    await user.click(restore);
    expect(note.value).toContain('qualifizierten elektronischen Signatur');
  });

  it('Nur Bildmarke: Größenregler mit Tastatur, Hinweis ausgeblendet', async () => {
    const { mock, user } = renderApp({ view: settingsView({ ...defaultSettings(), signature: { ...defaultSettings().signature, profile: 'BASE_LOGO' } }) });
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).queryByLabelText('Text:')).not.toBeInTheDocument();
    const slider = within(dialog).getByRole('slider', { name: /Größe der Bildmarke/ });
    // Anzeige und Screenreader-Wert in cm (gespeichert in mm), deutsche Dezimalschreibweise
    expect(within(dialog).getByText('2,3 cm')).toBeInTheDocument();
    expect(slider).toHaveAttribute('aria-valuetext', '2,3 cm');
    expect(slider).toHaveAttribute('min', '1');
    expect(slider).toHaveAttribute('max', '20');
    slider.focus();
    await user.keyboard('{ArrowRight}{ArrowRight}');
    expect(slider).toHaveAttribute('aria-valuetext', '2,5 cm');
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched).signature.logo_only_size_mm).toBe(25);
  });

  it('Logo-Größe: Umrechnung mm → cm mit Grenzen 1–20 cm', () => {
    expect(logoCm(23)).toBe(2.3);
    expect(logoCm(200)).toBe(20);
    expect(logoCm(500)).toBe(20);
    expect(logoCm(3)).toBe(1);
    expect(logoCm(Number.NaN)).toBe(2.3);
  });

  it('Bildmarke wählen, zurücksetzen und per Drop auf die Vorschau (→ Nur Bildmarke)', async () => {
    const { mock, user } = renderApp({ view: settingsView(), pickFilesResult: ['/Users/demo/logo.png'] });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: 'Logo auswählen' }));
    await waitFor(() => expect(within(dialog).getByText('/Users/demo/logo.png')).toBeInTheDocument());
    await user.click(within(dialog).getByRole('button', { name: 'Logo zurücksetzen' }));
    expect(within(dialog).getByText('Keine Bildmarke gewählt')).toBeInTheDocument();
    act(() => {
      window.dispatchEvent(new CustomEvent(EMBLEM_DROP_EVENT, { detail: '/Users/demo/drop.gif' }));
    });
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const saved = savedSettings(mock.dispatched);
    expect(saved.signature.emblem).toBe('/Users/demo/drop.gif');
    expect(saved.signature.profile).toBe('BASE_LOGO');
  });

  it('Erweitert: Sprache, Platzhalter, BKU, Keystore-Tab, Ausgabeordner, Proxy', async () => {
    const { mock, user } = renderApp({ view: settingsView(), pickDirectoryResult: '/Users/demo/Signiert' });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'Erweitert' }));
    // Interface-Sprache über Select
    await user.click(within(dialog).getByRole('button', { name: /Interface-Sprache/ }));
    await user.click(await screen.findByRole('option', { name: 'Englisch' }));
    // Platzhalter: Radios erst aktiv mit Suche
    const fields = within(dialog).getByRole('radio', { name: /Signaturfelder/ });
    expect(fields).toBeDisabled();
    await user.click(within(dialog).getByRole('checkbox', { name: /Platzhalter Suche/ }));
    await user.click(fields);
    // Keystore aktivieren → Tab erscheint, BKU-Auswahl bietet Keystore an
    await user.click(within(dialog).getByRole('checkbox', { name: /Keystore-Signatur aktivieren/ }));
    expect(within(dialog).getByRole('tab', { name: 'Keystore' })).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: /BKU-Auswahl/ }));
    await user.click(await screen.findByRole('option', { name: 'Keystore' }));
    // Ausgabeordner per Dialog
    await user.click(within(dialog).getByRole('button', { name: 'Durchsuchen' }));
    await waitFor(() => expect(within(dialog).getByLabelText('Standard-Ausgabeordner:')).toHaveValue('/Users/demo/Signiert'));
    // Proxy manuell, Port nur Ziffern
    await user.click(within(dialog).getByRole('radio', { name: 'Manuell' }));
    await user.type(within(dialog).getByLabelText('Host:'), 'proxy.local');
    await user.type(within(dialog).getByLabelText('Port:'), '80a80');
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const s = savedSettings(mock.dispatched);
    expect(s.general.ui_language).toBe('en');
    expect(s.placeholder).toEqual({ search: true, kind: 'fields' });
    expect(s.keystore.enabled).toBe(true);
    expect(s.backend.default).toBe('keystore');
    expect(s.output.folder).toBe('/Users/demo/Signiert');
    expect(s.proxy).toEqual({ mode: 'manual', host: 'proxy.local', port: 8080, user: null });
  });

  it('Keystore deaktivieren setzt eine Keystore-Voreinstellung zurück', async () => {
    const base = defaultSettings();
    const { mock, user } = renderApp({
      view: settingsView({ ...base, keystore: { ...base.keystore, enabled: true, file: '/k.p12' }, backend: { default: 'keystore' } }),
    });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'Erweitert' }));
    await user.click(within(dialog).getByRole('checkbox', { name: /Keystore-Signatur aktivieren/ }));
    expect(within(dialog).queryByRole('tab', { name: 'Keystore' })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const s = savedSettings(mock.dispatched);
    expect(s.keystore.enabled).toBe(false);
    expect(s.backend.default).toBeNull();
  });

  it('Keystore-Tab: Datei, Typ, Alias, Speicherart (Schlüsselbund nur wenn verfügbar)', async () => {
    const base = defaultSettings();
    const { mock, user } = renderApp({
      view: settingsView({ ...base, keystore: { ...base.keystore, enabled: true } }, { keychain_available: false }),
      pickFilesResult: ['/Users/demo/id.p12'],
    });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'Keystore' }));
    await user.click(within(dialog).getByRole('button', { name: 'Durchsuchen' }));
    await waitFor(() => expect(within(dialog).getByLabelText('Keystore-Datei')).toHaveValue('/Users/demo/id.p12'));
    await user.type(within(dialog).getByLabelText('Schlüssel-Alias'), 'signer');
    expect(within(dialog).getByRole('radio', { name: /Schlüsselbund/ })).toBeDisabled();
    await user.click(within(dialog).getByRole('radio', { name: /Nicht speichern/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched).keystore).toEqual({
      enabled: true,
      file: '/Users/demo/id.p12',
      // wie PDF-Over: Typ aus der Dateiendung
      declared_type: 'PKCS12',
      alias: 'signer',
      password_storage: 'none',
    });
  });

  it('Keystore-Typ folgt der Dateiendung (PDF-Over KeystoreConfigurationComposite)', () => {
    expect(keystoreTypeFromFile('/a/b/id.P12')).toBe('PKCS12');
    expect(keystoreTypeFromFile('C:\\Keys\\id.pfx')).toBe('PKCS12');
    expect(keystoreTypeFromFile('id.pkcs12')).toBe('PKCS12');
    expect(keystoreTypeFromFile('/a/keystore.jks')).toBe('JCEKS');
    expect(keystoreTypeFromFile('/a/keystore.ks ')).toBe('JCEKS');
    expect(keystoreTypeFromFile('/a.p12/keystore')).toBeNull();
    expect(keystoreTypeFromFile('/a/keystore.bin')).toBeNull();
  });

  it('Keystore-Typ beim Tippen des Pfads; andere Endungen behalten die Auswahl', async () => {
    const base = defaultSettings();
    const { mock, user } = renderApp({ view: settingsView({ ...base, keystore: { ...base.keystore, enabled: true, declared_type: 'JCEKS' } }) });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'Keystore' }));
    const file = within(dialog).getByLabelText('Keystore-Datei');
    await user.type(file, '/x/schluessel.dat');
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched).keystore.declared_type).toBe('JCEKS');
    await user.clear(file);
    await user.type(file, '/x/id.pfx');
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched.slice(1)).keystore).toMatchObject({ file: '/x/id.pfx', declared_type: 'PKCS12' });
  });

  it('FIDO2-Tab: Option nur mit Plattformunterstützung', async () => {
    const { mock, user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'FIDO2' }));
    await user.click(within(dialog).getByRole('checkbox', { name: 'FIDO2 standardmäßig auswählen' }));
    await user.click(within(dialog).getByRole('button', { name: /Informationsseite der ID-Austria/ }));
    expect(mock.openedUrls).toEqual(['https://www.oesterreich.gv.at/id-austria/haeufige-fragen/allgemeines-zu-id-austria#fido']);
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched).id_austria.fido2_default).toBe(true);
  });

  it('FIDO2 nicht verfügbar', async () => {
    const { user } = renderApp({ view: settingsView(defaultSettings(), { fido2_available: false }) });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'FIDO2' }));
    expect(within(dialog).queryByRole('checkbox')).not.toBeInTheDocument();
    expect(within(dialog).getByText('Ihr System unterstützt keine Authentifizierung mittels FIDO2-Token.')).toBeInTheDocument();
  });

  it('Über: Version, Lizenz- und Datenschutzlinks, Logs anzeigen', async () => {
    const { mock, user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'Über PDF-Over NG' }));
    expect(await within(dialog).findByText('Version 0.1.0')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: /software@egiz\.gv\.at/ }));
    expect(mock.openedUrls).toEqual(['mailto:software@egiz.gv.at']);
    await user.click(within(dialog).getByRole('button', { name: /Log/ }));
    await waitFor(() => expect(mock.nativeCalls).toContain('openLogDir'));
    // Update-Prüfung wie in PDF-Over im Tab „Über“
    await user.click(within(dialog).getByRole('checkbox', { name: /Automatisch auf Updates überprüfen/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const ev = mock.dispatched.find((e) => (e as UiEvent).type === 'save_settings') as Extract<UiEvent, { type: 'save_settings' }>;
    expect(ev.settings.general.update_check).toBe(false);
  });

  it('Validierungsprobleme vom Kern: Zusammenfassung und am Feld', async () => {
    const { user } = renderApp({ view: settingsView(defaultSettings(), { problems: { 'proxy.port': 'ng-ui-problem-port' } }) });
    const dialog = await screen.findByRole('dialog');
    const summary = within(dialog).getByRole('alert');
    expect(summary).toHaveTextContent('proxy.port');
    expect(summary).toHaveTextContent('Der Port muss zwischen 1 und 65535 liegen.');
    await waitFor(() => expect(summary).toHaveFocus());
    await user.click(within(dialog).getByRole('tab', { name: 'Erweitert' }));
    expect(within(dialog).getByLabelText('Port:')).toHaveAttribute('aria-invalid', 'true');
  });

  it('Zurücksetzen mit Rückfrage', async () => {
    const { mock, user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: /Einstellungen zurücksetzen/ }));
    const confirm = await screen.findByRole('alertdialog');
    await user.click(within(confirm).getByRole('button', { name: 'Zurücksetzen' }));
    expect(mock.dispatched).toEqual([{ type: 'reset_settings' }]);
  });

  it('Zurücksetzen bei offenem Dialog: die Arbeitskopie übernimmt den neuen Stand des Kerns', async () => {
    const base = defaultSettings();
    const custom: Settings = { ...base, id_austria: { ...base.id_austria, username: 'alt' }, output: { folder: '/alt', postfix: '_alt' } };
    const { mock, user } = renderApp({ view: settingsView(custom) });
    const dialog = await screen.findByRole('dialog');
    const username = within(dialog).getByLabelText('Benutzername:');
    expect(username).toHaveValue('alt');
    await user.type(username, '-ungespeichert');
    await user.click(within(dialog).getByRole('button', { name: /Einstellungen zurücksetzen/ }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Zurücksetzen' }));
    // Der Kern setzt zurück und lässt den Dialog offen (Werkseinstellungen)
    act(() => mock.setView(settingsView(defaultSettings())));
    await waitFor(() => expect(within(screen.getByRole('dialog')).getByLabelText('Benutzername:')).toHaveValue(''));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Speichern' }));
    const saved = savedSettings(mock.dispatched);
    expect(saved.id_austria.username).toBeNull();
    expect(saved.output).toEqual({ folder: null, postfix: '_signed' });
  });

  it('eigene Eingaben bleiben, solange der Kern denselben Stand oder das Echo des Speicherns meldet', async () => {
    const { mock, user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    const username = within(dialog).getByLabelText('Benutzername:');
    await user.type(username, 'neu');
    // neuer ViewState mit unverändertem Inhalt (z. B. Fortschritt im Hintergrund)
    act(() => mock.setView(settingsView()));
    expect(within(screen.getByRole('dialog')).getByLabelText('Benutzername:')).toHaveValue('neu');
    // Speichern scheitert an der Validierung: Kern meldet Problem mit den gesendeten Einstellungen
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Speichern' }));
    const sent = savedSettings(mock.dispatched);
    act(() => mock.setView(settingsView(sent, { problems: { 'proxy.port': 'ng-ui-problem-port' } })));
    expect(within(screen.getByRole('dialog')).getByLabelText('Benutzername:')).toHaveValue('neu');
    // Kern normalisiert einen Wert → Arbeitskopie folgt
    act(() => mock.setView(settingsView({ ...sent, id_austria: { ...sent.id_austria, username: '+43neu' } })));
    await waitFor(() => expect(within(screen.getByRole('dialog')).getByLabelText('Benutzername:')).toHaveValue('+43neu'));
  });

  it('Zugriffstasten der Tabs (Alt+Taste) im obersten Dialog', async () => {
    renderApp({ view: settingsView(defaultSettings(), {}, 'en') });
    const dialog = await screen.findByRole('dialog', { name: 'Configuration' });
    // EN: Advanced = &v
    fireEvent.keyDown(window, { key: 'v', code: 'KeyV', altKey: true });
    await waitFor(() => expect(within(dialog).getByRole('tab', { name: 'Advanced' })).toHaveAttribute('aria-selected', 'true'));
  });

  it('Hell/Dunkel wird sofort angewandt und lokal gemerkt', async () => {
    const { user } = renderApp({ view: settingsView() });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: 'Erweitert' }));
    await user.click(within(dialog).getByRole('button', { name: /Darstellung/ }));
    await user.click(await screen.findByRole('option', { name: 'Dunkel' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    expect(localStorage.getItem('pdfover-ng.theme')).toBe('dark');
  });
});
