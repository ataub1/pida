/**
 * Befehlspalette und Kürzelübersicht (Paket UI-B, ADR 0004 §2.10, Abnahme E4 Punkte 4 und 6 [M]): alle Befehle
 * aus `app/commands.ts` mit den Freigaben aus `ViewState::actions` (gesperrte erscheinen nicht), Gruppen, Filter
 * mit Synonymen, Tastatur vollständig (⌘/Strg+K, Pfeile, Eingabe, Esc), Befehle wirken (Einstellungen,
 * Farbschema, Sprache, Signieren, Kürzel); Kürzelübersicht vollständig (Vergleich mit `commands.ts` inkl.
 * Zugriffstasten-Hinweis); Mobile ohne Tastenkürzel-Hinweise; axe.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { buildShortcuts, COMMANDS, isCommandEnabled } from '../../app/commands';
import { defaultSettings } from '../../core/defaults';
import { translator } from '../../i18n/catalog';
import { expectNoAxeViolations } from '../../test/a11y';
import { emulateMedia, lastEvent, renderApp } from '../../test/render';
import { shortcutRows } from './ShortcutsDialog';

const DEMO = { ...defaultSettings(), backend: { default: 'id_austria' as const }, id_austria: { username: '+43 660 1234123', remember: true, fido2_default: false } };

async function openPalette(mac = false) {
  fireEvent.keyDown(window, mac ? { key: 'k', metaKey: true } : { key: 'k', ctrlKey: true });
  return screen.findByRole('dialog', { name: 'Befehle' });
}

const names = (root: HTMLElement) => within(root).queryAllByRole('menuitem').map((el) => el.querySelector('[slot=label], .palette-label')?.textContent ?? '');

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe('Befehlspalette', () => {
  it('Strg+K öffnet mit Fokus im Suchfeld; genau die freigegebenen Befehle aus commands.ts, nach Gruppen', async () => {
    const { mock } = renderApp();
    await screen.findByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' });
    const palette = await openPalette();
    expect(within(palette).getByRole('searchbox', { name: 'Befehl suchen …' })).toHaveFocus();
    const view = await mock.view();
    const t = translator('de');
    const expected = COMMANDS.filter((c) => isCommandEnabled(c.id, view)).map((c) => t.t(c.label));
    expect(names(palette)).toEqual(expected);
    // ohne Dokument: „Signieren“ gesperrt und deshalb nicht in der Liste; kein „Signatur prüfen“ mehr (Nachtrag 15)
    expect(expected).not.toContain('Signieren');
    expect(names(palette).some((n) => /prüf/i.test(n))).toBe(false);
    const actions = within(palette).getByRole('group', { name: 'Aktionen' });
    expect(within(actions).getByRole('menuitem', { name: /^Dokument öffnen …/ })).toBeInTheDocument();
    expect(within(within(palette).getByRole('group', { name: 'Ansicht' })).getByRole('menuitem', { name: 'Farbschema: Dunkel' })).toBeInTheDocument();
    expect(within(within(palette).getByRole('group', { name: 'Hilfe' })).getByRole('menuitem', { name: /^Tastenkürzel anzeigen/ })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Filter mit Synonymen, Pfeile und Eingabe führen aus, „Keine Treffer“', async () => {
    const { mock, user } = renderApp();
    await screen.findByRole('heading', { level: 1 });
    let palette = await openPalette();
    const search = within(palette).getByRole('searchbox', { name: 'Befehl suchen …' });
    await user.type(search, 'xyz');
    expect(within(palette).getByText('Keine Treffer')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.clear(search);
    await user.type(search, 'logo');
    expect(names(palette)).toEqual(['Signaturbild bearbeiten']);
    await user.clear(search);
    await user.type(search, 'einst');
    expect(names(palette)).toEqual(['Einstellungen']);
    await user.keyboard('{ArrowDown}{Enter}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Befehle' })).not.toBeInTheDocument());
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'open_settings' }));
    expect(await screen.findByRole('dialog', { name: 'Einstellungen' })).toBeInTheDocument();
    // Einstellungen offen: der Befehl ist gesperrt
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Einstellungen' })).not.toBeInTheDocument());
    palette = await openPalette();
    expect(names(palette)).toContain('Einstellungen');
  });

  it('Esc schließt, ⌘/Strg+K schaltet um, Knopf in der Kopfzeile öffnet', async () => {
    const { user } = renderApp({ isMac: true });
    await screen.findByRole('heading', { level: 1 });
    const button = within(screen.getByRole('banner')).getByRole('button', { name: 'Befehle und Suche' });
    await user.click(button);
    await screen.findByRole('dialog', { name: 'Befehle' });
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Befehle' })).not.toBeInTheDocument());
    await waitFor(() => expect(button).toHaveFocus());
    await openPalette(true);
    fireEvent.keyDown(window, { key: 'k', metaKey: true });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Befehle' })).not.toBeInTheDocument());
  });

  it('Farbschema und Sprache über die Palette', async () => {
    const { mock, user } = renderApp();
    await screen.findByRole('heading', { level: 1 });
    await user.click(within(await openPalette()).getByRole('menuitem', { name: 'Farbschema: Dunkel' }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute('data-theme', 'dark'));
    await user.click(within(await openPalette()).getByRole('menuitem', { name: 'Language: English' }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'set_ui_language', language: 'en' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'What would you like to sign?' })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Commands' });
    expect(within(palette).getByRole('searchbox', { name: 'Search commands …' })).toBeInTheDocument();
    expect(within(palette).getByRole('group', { name: 'View' })).toBeInTheDocument();
  });

  it('Arbeitsbereich: „Signieren“ mit Tastenkappen; die Wahl signiert mit dem gewählten Signiermittel (A1: ausdrücklich)', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', settings: DEMO, isMac: true });
    await screen.findByRole('application', { name: 'Signaturblock' });
    const palette = await openPalette(true);
    const sign = within(palette).getByRole('menuitem', { name: /^Signieren/ });
    expect(sign).toHaveTextContent('⌘↵');
    expect(mock.dispatched.some((e) => e.type === 'sign_with')).toBe(false);
    await user.click(sign);
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'sign_with', backend: 'atrust_native' }));
  });

  it('„Tastenkürzel anzeigen“ öffnet die Übersicht', async () => {
    const { user } = renderApp();
    await screen.findByRole('heading', { level: 1 });
    await user.click(within(await openPalette()).getByRole('menuitem', { name: /^Tastenkürzel anzeigen/ }));
    expect(await screen.findByRole('dialog', { name: 'Tastenkürzel' })).toBeInTheDocument();
  });

  it('⌘/Strg+K nicht über anderen Dialogen (Ersteinrichtung), wohl über den Einstellungen und der Kürzelübersicht (Merge E4)', async () => {
    const first = renderApp({ firstRun: true });
    await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    await new Promise((r) => setTimeout(r, 30));
    expect(screen.queryByRole('dialog', { name: 'Befehle' })).not.toBeInTheDocument();
    first.unmount();

    const { user } = renderApp();
    await screen.findByRole('heading', { level: 1 });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Einstellungen' }));
    const settings = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const palette = await openPalette();
    // „Einstellungen“ ist gesperrt, solange sie offen sind
    expect(names(palette)).not.toContain('Einstellungen');
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Befehle' })).not.toBeInTheDocument());
    expect(settings).toBeInTheDocument();
    await user.click(within(settings).getAllByRole('button', { name: 'Schließen' })[0] as HTMLElement);
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Einstellungen' })).not.toBeInTheDocument());
    fireEvent.keyDown(window, { key: '?' });
    await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    await openPalette();
  });

  it('Mobile: ohne Tastenkappen; schmal als Sheet; axe', async () => {
    restore = emulateMedia({ narrow: true, coarse: true });
    const { user } = renderApp({ platform: 'ios' });
    await screen.findByRole('heading', { level: 1 });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Befehle und Suche' }));
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    expect(palette.querySelector('.keys')).toBeNull();
    await expectNoAxeViolations();
    // Kürzelübersicht schmal (über die Palette erreichbar)
    await user.click(within(palette).getByRole('menuitem', { name: /^Tastenkürzel anzeigen/ }));
    expect(await screen.findByRole('dialog', { name: 'Tastenkürzel' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });
});

describe('Kürzelübersicht', () => {
  it('„?“ öffnet; vollständig: jedes Kürzel aus commands.ts, Zugriffstasten-Hinweis, Palette; axe', async () => {
    renderApp({ scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    fireEvent.keyDown(window, { key: '?' });
    const dialog = await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    const t = translator('de');
    const expected = shortcutRows(buildShortcuts(t, false), { label: 'Befehlspalette', keys: ['Strg', 'K'] });
    const rows = [...dialog.querySelectorAll<HTMLElement>('.shortcuts-row')];
    expect(rows.map((r) => r.dataset.shortcut)).toEqual(expected.map((e) => e.id));
    for (const [i, row] of rows.entries()) {
      const e = expected[i];
      expect(row.querySelector('dt')).toHaveTextContent(e?.label ?? '');
      expect(row.querySelector('.sr-only')).toHaveTextContent(e?.keys.join('+') ?? '');
      expect([...row.querySelectorAll('.kbd')].map((k) => k.textContent)).toEqual(e?.keys);
    }
    // jeder Befehl mit Tastenkürzel steht in der Übersicht
    for (const c of COMMANDS.filter((x) => x.keys)) expect(within(dialog).getByText(t.t(c.label))).toBeInTheDocument();
    expect(within(dialog).getByText('Zugriffstasten (unterstrichene Buchstaben bei gedrückter Alt-Taste)')).toBeInTheDocument();
    expect(within(dialog).getByText('Signaturblock verschieben (15 pt)')).toBeInTheDocument();
    expect(within(dialog).getByText('Strg+K')).toBeInTheDocument();
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Allgemein' })).toBeInTheDocument();
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Arbeitsbereich' })).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('macOS-Tasten, Schließen mit Esc und ×; „?“ in Eingabefeldern bleibt Text', async () => {
    const { user } = renderApp({ isMac: true });
    await screen.findByRole('heading', { level: 1 });
    fireEvent.keyDown(window, { key: '?' });
    let dialog = await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    expect(within(dialog).getByText('⌘+K')).toBeInTheDocument();
    expect(within(dialog).getByText('⌃+⌥+A–Z')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Tastenkürzel' })).not.toBeInTheDocument());
    fireEvent.keyDown(window, { key: '?' });
    dialog = await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    await user.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Tastenkürzel' })).not.toBeInTheDocument());
    const palette = await openPalette(true);
    await user.type(within(palette).getByRole('searchbox'), '?');
    expect(screen.queryByRole('dialog', { name: 'Tastenkürzel' })).not.toBeInTheDocument();
  });
});
