/**
 * Bisheriger Einstellungsdialog hinter `SettingsPanel` (Paket UI-B, bis B3; in U0 aus `app/App.test.tsx` und
 * `app/Mobile.test.tsx` verlegt, ADR 0004 §4.2): Sprache, Datenschutz-Tab aus dem Kurzhinweis, Keystore-Import
 * in der Arbeitskopie, Mobile ohne Desktop-Bereiche.
 */
import { act, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../../core/defaults';
import type { UiEvent, ViewState } from '../../core/types';
import { keystoreOnlyChanged } from '../../settings/SettingsDialog';
import { renderApp } from '../../test/render';
import { IOS, onMobile, settingsView, workspaceView } from '../../test/views';

describe('Einstellungen (bisheriger Dialog)', () => {
  it('Sprache wechseln wirkt nach dem Speichern', async () => {
    const { user } = renderApp();
    await user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Konfiguration' });
    await user.click(within(dialog).getByRole('tab', { name: 'Erweitert' }));
    await user.click(within(dialog).getByRole('button', { name: /Interface-Sprache/ }));
    await user.click(await screen.findByRole('option', { name: 'Englisch' }));
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'What would you like to sign?' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('„Details“ des Kurzhinweises am Konto-Chip öffnet die Einstellungen beim Tab Datenschutz', async () => {
    const { mock, user } = renderApp({ view: workspaceView() });
    const note = await screen.findByTestId('privacy-note');
    await user.click(within(note).getByRole('link', { name: 'Details zum Datenschutz' }));
    expect(mock.dispatched.at(-1)).toEqual({ type: 'open_settings' });
    act(() => mock.setView({ ...workspaceView(), settings: settingsView().settings }));
    const dialog = await screen.findByRole('dialog', { name: 'Konfiguration' });
    expect(within(dialog).getByRole('tab', { name: 'Datenschutz' })).toHaveAttribute('aria-selected', 'true');
  });

  it('Mock-Kern: Import setzt den Keystore in den Einstellungen (Arbeitskopie bleibt sonst erhalten)', async () => {
    const { mock, user } = renderApp({ platform: 'android', settings: { ...defaultSettings(), keystore: { ...defaultSettings().keystore, enabled: true } } });
    await screen.findByRole('heading', { level: 1 });
    await act(async () => {
      await mock.dispatch({ type: 'open_settings' });
    });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: /Erweitert/ }));
    await user.click(within(dialog).getByRole('checkbox', { name: /Automatische Positionierung/ }));
    await user.click(within(dialog).getByRole('tab', { name: 'Keystore' }));
    await user.click(within(dialog).getByRole('button', { name: 'PKCS#12-Datei importieren …' }));
    const wizard = await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' });
    await user.click(within(wizard).getByRole('button', { name: 'Datei wählen …' }));
    await user.type(within(wizard).getByLabelText('Passwort des Keystores'), 'test');
    await user.click(within(wizard).getByRole('button', { name: 'Importieren' }));
    await user.click(await within(await screen.findByRole('alertdialog', { name: 'PKCS#12-Keystore importieren' })).findByRole('button', { name: 'Fertig' }));
    expect(await within(dialog).findByText('Importierte Datei: test.p12')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const saved = mock.dispatched.find((e) => e.type === 'save_settings') as Extract<UiEvent, { type: 'save_settings' }>;
    expect(saved.settings.keystore.file).toMatch(/keystores\/test\.p12$/);
    expect(saved.settings.signature.auto_position).toBe(true);
  });

  it('keystoreOnlyChanged erkennt reine Keystore-Änderungen', () => {
    const a = defaultSettings();
    const b = { ...a, keystore: { ...a.keystore, file: '/x/k.p12', enabled: true } };
    expect(keystoreOnlyChanged(a, b)).toBe(true);
    expect(keystoreOnlyChanged(a, { ...b, signature: { ...b.signature, auto_position: true } })).toBe(false);
    expect(keystoreOnlyChanged(a, a)).toBe(false);
  });

  it('Mobile: ohne Ausgabeordner, Proxy, Update-Prüfung und Log-Ordner; FIDO2-Hinweis', async () => {
    const view: ViewState = onMobile(settingsView(defaultSettings(), { fido2_available: false }), IOS);
    const { user } = renderApp({ view });
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('tab', { name: /Erweitert/ }));
    expect(within(dialog).queryByRole('textbox', { name: /Ausgabeordner|Ordner/ })).not.toBeInTheDocument();
    expect(within(dialog).queryByText('Proxy')).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('tab', { name: 'FIDO2' }));
    expect(within(dialog).getByText(/FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('tab', { name: /Über/ }));
    expect(within(dialog).queryByRole('checkbox', { name: /Updates/ })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: /Log/ })).not.toBeInTheDocument();
  });
});
