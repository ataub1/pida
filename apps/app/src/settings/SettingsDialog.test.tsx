import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import type { Settings, UiEvent } from '../core/types';
import { renderApp } from '../test/render';
import { settingsView } from '../test/views';
import { EMBLEM_DROP_EVENT } from './SettingsDialog';

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
    expect(within(dialog).getByText('2.3 cm')).toBeInTheDocument();
    slider.focus();
    await user.keyboard('{ArrowRight}{ArrowRight}');
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(savedSettings(mock.dispatched).signature.logo_only_size_mm).toBe(25);
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
      declared_type: null,
      alias: 'signer',
      password_storage: 'none',
    });
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
