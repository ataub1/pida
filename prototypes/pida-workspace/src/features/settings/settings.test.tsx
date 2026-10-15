/** Einstellungen: Live-Editor des Signaturbilds, Suche, sofort wirksame Änderungen. */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../test/render';

function preview(dialog: HTMLElement) {
  return within(dialog).getByRole('img', { name: /Vorschau des Signaturblocks/ });
}

describe('Einstellungen', () => {
  it('ändert die Signaturbild-Vorschau sofort (Profil, Sprache, Hinweistext)', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('button', { name: 'Signaturbild bearbeiten' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    expect(preview(dialog)).toHaveAccessibleName(/Standard \(klein\), Deutsch/);
    expect(preview(dialog)).toHaveTextContent('Unterzeichner');
    expect(preview(dialog)).not.toHaveTextContent('Aussteller-Zertifikat');

    await user.click(within(dialog).getByRole('radio', { name: /Amtssignatur/ }));
    expect(preview(dialog)).toHaveAccessibleName(/Amtssignatur/);
    expect(preview(dialog)).toHaveTextContent('Aussteller-Zertifikat');
    expect(preview(dialog)).toHaveTextContent('amtssigniert');

    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(preview(dialog)).toHaveTextContent('Signatory');
    expect(preview(dialog)).toHaveTextContent('official signature');

    const note = within(dialog).getByLabelText('Hinweistext');
    await user.clear(note);
    await user.type(note, 'Intern – nur zur Ablage');
    expect(preview(dialog)).toHaveTextContent('Intern – nur zur Ablage');
    await user.click(within(dialog).getByRole('button', { name: 'Standardtext wiederherstellen' }));
    expect(preview(dialog)).toHaveTextContent('official signature');

    // Sofort wirksam: Inspektor im Arbeitsbereich zeigt das geänderte Signaturbild
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(document.querySelector('.insp-block-preview')).toHaveTextContent('Signatory');
  });

  it('übernimmt ein Logo per Dateiauswahl', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const input = dialog.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, new File(['<svg xmlns="http://www.w3.org/2000/svg"/>'], 'logo.svg', { type: 'image/svg+xml' }));
    expect(await within(dialog).findByText('Aktuell: logo.svg')).toBeInTheDocument();
    await waitFor(() => expect(preview(dialog).querySelector('img')).not.toBeNull());
  });

  it('durchsucht alle Bereiche', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.type(within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'proxy');
    expect(within(dialog).getByRole('heading', { name: 'Erweitert' })).toBeInTheDocument();
    expect(within(dialog).getByRole('radiogroup', { name: 'Proxy' })).toBeInTheDocument();
    expect(within(dialog).queryByRole('heading', { name: 'Signaturbild' })).not.toBeInTheDocument();
    await user.clear(within(dialog).getByRole('searchbox'));
    await user.type(within(dialog).getByRole('searchbox'), 'xyz');
    expect(within(dialog).getByText('Keine Einstellung gefunden für „xyz“.')).toBeInTheDocument();
  });

  it('Speichern: Namensmuster mit Vorschau wirkt auf den Ausgabenamen', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(dialog).getByRole('button', { name: 'Speichern' }));
    const pattern = within(dialog).getByLabelText('Dateiname');
    expect(pattern).toHaveValue('{name}_signiert');
    expect(within(dialog).getByText('Vertrag_signiert.pdf')).toBeInTheDocument();
    await user.clear(pattern);
    await user.type(pattern, '{{name}-final');
    expect(within(dialog).getByText('Vertrag-final.pdf')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('radio', { name: /In festem Ordner/ }));
    expect(within(dialog).getAllByText('~/Dokumente/Signiert').length).toBeGreaterThan(0);
  });

  it('Darstellung und Sprache gelten sofort', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(dialog).getByRole('button', { name: 'Darstellung' }));
    await user.click(within(dialog).getByRole('radio', { name: 'Dunkel' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(await screen.findByRole('dialog', { name: 'Settings' })).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute('lang', 'en');
  });
});

describe('Einstellungen › Dokument (PDF/A, Platzhaltersuche)', () => {
  async function openDocumentSection(opts: Parameters<typeof renderApp>[0] = { scenario: 'start' }) {
    const r = renderApp(opts);
    await r.user.click(screen.getByRole('button', { name: opts.settings?.uiLang === 'en' ? 'Settings' : 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: opts.settings?.uiLang === 'en' ? 'Settings' : 'Einstellungen' });
    return { ...r, dialog };
  }

  it('steht direkt nach „Signaturbild“ und enthält PDF/A-Schalter und Platzhaltersuche (nicht mehr unter „Erweitert“)', async () => {
    const { user, dialog } = await openDocumentSection();
    const nav = within(dialog).getByRole('navigation', { name: 'Bereiche' });
    const names = within(nav)
      .getAllByRole('button')
      .map((b) => b.textContent);
    expect(names.indexOf('Dokument')).toBe(names.indexOf('Signaturbild') + 1);

    await user.click(within(nav).getByRole('button', { name: 'Dokument' }));
    expect(within(dialog).getByRole('heading', { level: 3, name: 'Dokument' })).toBeInTheDocument();
    const pdfa = within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' });
    expect(pdfa).not.toBeChecked();
    expect(pdfa).toHaveAccessibleDescription(/erkennt PIDA automatisch .* erzwingt das auch für Dokumente ohne PDF\/A-Kennung/);
    expect(within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' })).toBeChecked();
    const kind = within(dialog).getByRole('radiogroup', { name: 'Suchen nach' });
    expect(within(kind).getByRole('radio', { name: /QR-Code-Platzhalter/ })).toBeChecked();
    expect(within(kind).getByRole('radio', { name: /Leere Signaturfelder/ })).not.toBeChecked();

    await user.click(within(nav).getByRole('button', { name: 'Erweitert' }));
    expect(within(dialog).queryByRole('switch', { name: /Platzhalter/ })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('radiogroup', { name: 'Suchen nach' })).not.toBeInTheDocument();
  });

  it('ist über „pdf/a“, „platzhalter“, „qr“ und „signaturfeld“ auffindbar', async () => {
    const { user, dialog } = await openDocumentSection();
    const search = within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' });
    const cases: [string, RegExp][] = [
      ['pdf/a', /PDF\/A-kompatibel signieren/],
      ['PDF/A', /PDF\/A-kompatibel signieren/],
      ['platzhalter', /Signaturplatzhalter automatisch suchen/],
      ['qr', /Signaturplatzhalter automatisch suchen/],
      ['signaturfeld', /Signaturplatzhalter automatisch suchen/],
    ];
    for (const [q, control] of cases) {
      await user.clear(search);
      await user.type(search, q);
      expect(within(dialog).getByRole('heading', { level: 3, name: 'Dokument' })).toBeInTheDocument();
      expect(within(dialog).getByRole('switch', { name: control })).toBeInTheDocument();
      expect(within(dialog).queryByRole('heading', { name: 'Erweitert' })).not.toBeInTheDocument();
    }
    // Platzhalter-Treffer zeigen nur die Platzhaltersuche, PDF/A-Treffer nur den PDF/A-Schalter
    expect(within(dialog).queryByRole('switch', { name: 'PDF/A-kompatibel signieren' })).not.toBeInTheDocument();
    await user.clear(search);
    await user.type(search, 'pdf/a');
    expect(within(dialog).queryByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' })).not.toBeInTheDocument();
  });

  it('ist auch englisch auffindbar („signature field“, „pdf/a“)', async () => {
    const en = await openDocumentSection({ scenario: 'start', settings: { uiLang: 'en' } });
    const enSearch = within(en.dialog).getByRole('searchbox', { name: 'Search settings' });
    await en.user.type(enSearch, 'signature field');
    expect(within(en.dialog).getByRole('heading', { level: 3, name: 'Document' })).toBeInTheDocument();
    expect(within(en.dialog).getByRole('switch', { name: 'Search for signature placeholders automatically' })).toBeInTheDocument();
    await en.user.clear(enSearch);
    await en.user.type(enSearch, 'pdf/a');
    expect(within(en.dialog).getByRole('switch', { name: 'PDF/A-compatible signature' })).toBeInTheDocument();
  });

  it('Schalter wirken sofort; die Auswahl ist bei ausgeschalteter Suche gesperrt', async () => {
    const { user, dialog } = await openDocumentSection();
    await user.click(within(dialog).getByRole('button', { name: 'Dokument' }));
    const pdfa = within(dialog).getByRole('switch', { name: 'PDF/A-kompatibel signieren' });
    await user.click(pdfa);
    expect(pdfa).toBeChecked();

    const search = within(dialog).getByRole('switch', { name: 'Signaturplatzhalter automatisch suchen' });
    const group = within(dialog).getByRole('radiogroup', { name: 'Suchen nach' });
    const qr = within(group).getByRole('radio', { name: /QR-Code-Platzhalter/ });
    const field = within(group).getByRole('radio', { name: /Leere Signaturfelder/ });
    await user.click(search);
    expect(search).not.toBeChecked();
    expect(group).toHaveAttribute('aria-disabled', 'true');
    expect(qr).toBeDisabled();
    expect(field).toBeDisabled();
    await user.click(field);
    expect(field).not.toBeChecked();
    expect(qr).toBeChecked();

    await user.click(search);
    expect(search).toBeChecked();
    expect(group).not.toHaveAttribute('aria-disabled');
    expect(field).toBeEnabled();
    await user.click(field);
    expect(field).toBeChecked();
    expect(qr).not.toBeChecked();
  });
});
