/** Befehlspalette (Strg/⌘+K), Tastenkürzel-Übersicht (?), Ersteinrichtung und Sprache. */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../test/render';

describe('Befehlspalette und Kürzel', () => {
  it('öffnet mit Strg+K, filtert und führt Befehle aus', async () => {
    const { user } = renderApp({ scenario: 'start' });
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    const search = within(palette).getByRole('searchbox', { name: 'Befehl suchen …' });
    expect(search).toHaveFocus();
    for (const name of ['Dokument öffnen …', 'Signatur prüfen (Prüfmodus)', 'Einstellungen', 'Farbschema: Dunkel', 'Language: English', 'Tastenkürzel anzeigen']) {
      expect(within(palette).getByRole('menuitem', { name: new RegExp(`^${name.replace(/[.()…]/g, '\\$&')}`) })).toBeInTheDocument();
    }
    // „Signieren“ ist ohne Dokument nicht verfügbar
    expect(within(palette).queryByRole('menuitem', { name: /^Signieren/ })).not.toBeInTheDocument();

    await user.type(search, 'einst');
    expect(within(palette).getAllByRole('menuitem')).toHaveLength(1);
    await user.keyboard('{ArrowDown}{Enter}');
    expect(await screen.findByRole('dialog', { name: 'Einstellungen' })).toBeInTheDocument();
  });

  it('wechselt Farbschema und Sprache über die Palette', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Befehle und Suche' }));
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    await user.click(within(palette).getByRole('menuitem', { name: 'Farbschema: Dunkel' }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute('data-theme', 'dark'));
    await user.click(screen.getByRole('button', { name: 'Befehle und Suche' }));
    await user.click(within(await screen.findByRole('dialog', { name: 'Befehle' })).getByRole('menuitem', { name: 'Language: English' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'What would you like to sign?' })).toBeInTheDocument();
  });

  it('Farbschema über das Menü in der Kopfzeile', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Farbschema' }));
    await user.click(await screen.findByRole('menuitemradio', { name: 'Dunkel' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    await user.click(screen.getByRole('button', { name: 'Farbschema' }));
    await user.click(await screen.findByRole('menuitemradio', { name: 'Wie System' }));
    expect(document.documentElement).not.toHaveAttribute('data-theme');
  });

  it('zeigt die Tastenkürzel mit ?', async () => {
    renderApp({ scenario: 'contract' });
    fireEvent.keyDown(window, { key: '?' });
    const dialog = await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    expect(within(dialog).getByText('Signaturblock verschieben (10 pt)')).toBeInTheDocument();
    expect(within(dialog).getByText('Fein verschieben (1 pt)')).toBeInTheDocument();
    expect(within(dialog).getByText('Strg K')).toBeInTheDocument();
  });
});

describe('Ersteinrichtung', () => {
  it('übernimmt PDF-Over-Einstellungen, wählt das Standard-Signiermittel und das Signaturbild', async () => {
    const { user } = renderApp({ scenario: 'start', onboarded: false });
    const dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expect(within(dialog).getByText('PDF-Over-Einstellungen gefunden')).toBeInTheDocument();
    expect(within(dialog).getByText('Schritt 1 von 3')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Übernehmen' }));
    expect(await screen.findByText('3 Einstellungen übernommen')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('radio', { name: /Keystore/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Weiter' }));
    // Die Übernahme ersetzt die Standard-Bildmarke nicht: im Signaturbild steht weiter das PIDA-Siegel.
    expect(within(dialog).getByRole('img', { name: 'Vorschau des Signaturblocks' }).querySelector('img')).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: 'Los geht’s' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Kein PDF zur Hand? Beispielvertrag öffnen' }));
    expect(await screen.findByRole('button', { name: /Signiermittel: Keystore · max\.mustermann\.p12/ })).toBeInTheDocument();
  });

  it('ist überspringbar', async () => {
    const { user } = renderApp({ scenario: 'start', onboarded: false });
    const dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    await user.click(within(dialog).getByRole('button', { name: 'Einrichtung überspringen' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByRole('heading', { level: 1, name: 'Was möchten Sie signieren?' })).toBeInTheDocument();
  });
});

describe('Demo-Steuerung', () => {
  it('lädt Szenarien', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Demo' }));
    const panel = screen.getByRole('complementary', { name: 'Demo-Steuerung' });
    await user.click(within(panel).getByRole('button', { name: /Start \(kein Dokument\)/ }));
    await user.click(await screen.findByRole('option', { name: 'Stapel mit 4 Dateien' }));
    expect(await screen.findByRole('heading', { level: 1, name: '4 Dokumente' })).toBeInTheDocument();
  });
});
