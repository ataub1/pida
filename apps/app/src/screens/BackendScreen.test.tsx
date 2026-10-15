import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { BackendOptionView, Capabilities } from '../core/types';
import { renderApp } from '../test/render';
import { backendView, viewOf } from '../test/views';

const native: Capabilities = {
  input: 'Document',
  output: 'Cms',
  certificate: 'OnlyForVisibleBlock',
  interaction: 'NativeForms',
  server: 'None',
  max_batch: 1,
  login_once_per_batch: true,
  document_leaves_device: true,
  qualification: 'Qes',
  factors: { app: true, sms: true, qr: true, fido2: true, card: false },
  cms_reserve: 16384,
};

const OPTIONS: BackendOptionView[] = [
  { id: 'atrust_native', available: true, capabilities: native, notes: [] },
  { id: 'local_bku', available: false, capabilities: null, notes: [] },
  {
    id: 'keystore',
    available: true,
    capabilities: { ...native, document_leaves_device: false, qualification: 'Unknown', factors: { app: false, sms: false, qr: false, fido2: false, card: false } },
    notes: [],
  },
  { id: 'csc', available: false, capabilities: null, notes: [] },
];

describe('Signiermittel', () => {
  it('zeigt die Optionen mit Hinweisen; nicht verfügbare sind gesperrt; reservierte fehlen', async () => {
    renderApp({ view: backendView(OPTIONS) });
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiermittel wählen' })).toBeInTheDocument();
    const list = document.querySelector('.backend-list') as HTMLElement;
    const buttons = within(list).getAllByRole('button');
    expect(buttons).toHaveLength(3);
    const idA = screen.getByRole('button', { name: /^ID Austria/ });
    expect(idA).toHaveAccessibleDescription(/an A-Trust übermittelt/);
    expect(within(idA).getByText('Qualifizierte Signatur')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Lokale Bürgerkarte/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Lokale Bürgerkarte/ })).toHaveAccessibleDescription('Keine lokale Bürgerkartenumgebung gefunden.');
  });

  it('Auswahl per Klick und Zugriffstaste', async () => {
    const { mock, user } = renderApp({ view: backendView(OPTIONS) });
    await user.click(await screen.findByRole('button', { name: /^ID Austria/ }));
    // DE: KeyStore → Zugriffstaste aus bku_selection-ks
    const ks = screen.getByRole('button', { name: /^Keystore/ });
    const key = ks.getAttribute('data-accesskey');
    expect(key).toMatch(/^[a-z]$/);
    fireEvent.keyDown(window, { key: key ?? '', code: `Key${(key ?? '').toUpperCase()}`, altKey: true });
    expect(mock.dispatched).toEqual([
      { type: 'choose_backend', backend: 'atrust_native' },
      { type: 'choose_backend', backend: 'keystore' },
    ]);
  });

  it('Kurzhinweis bei ID Austria, nicht beim Keystore; kein Zustimmungsdialog (ADR 0003)', async () => {
    renderApp({ view: backendView(OPTIONS) });
    const idA = await screen.findByRole('button', { name: /^ID Austria/ });
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    const notes = screen.getAllByTestId('privacy-note');
    expect(notes).toHaveLength(1);
    const note = notes[0] as HTMLElement;
    expect(note).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt. Details');
    // beim Signiermittel ID Austria (dieselbe Listenzeile), nicht beim Keystore
    expect(idA.closest('li')).toContainElement(note);
    expect(screen.getByRole('button', { name: /^Keystore/ }).closest('li')?.querySelector('[data-testid="privacy-note"]')).toBeNull();
    expect(screen.getByRole('button', { name: /^Keystore/ })).not.toHaveAccessibleDescription(/A-Trust/);
    // „Details“ ist ein eigener Link außerhalb der Karte (kein Bedienelement im Knopf)
    const details = within(note).getByRole('link', { name: 'Details zum Datenschutz' });
    expect(idA).not.toContainElement(details);
    expect(details).toHaveTextContent('Details');
  });

  it('Kurzhinweis „Details“ öffnet die Einstellungen (ohne Ereignis an den Signiervorgang)', async () => {
    const { mock, user } = renderApp({ view: backendView(OPTIONS) });
    await user.click(await screen.findByRole('link', { name: 'Details zum Datenschutz' }));
    expect(mock.dispatched).toEqual([{ type: 'open_settings' }]);
  });

  it('ohne Signiermittel mit Upload kein Kurzhinweis', async () => {
    const localOnly = OPTIONS.filter((o) => o.id !== 'atrust_native');
    renderApp({ view: backendView(localOnly) });
    await screen.findByRole('button', { name: /^Keystore/ });
    expect(screen.queryByTestId('privacy-note')).not.toBeInTheDocument();
  });
});

describe('Datenschutz: Kurzhinweis → Einstellungen › Datenschutz (Mock-Kern)', () => {
  it('„Details“ in der Signiermittel-Auswahl öffnet die Einstellungen beim Tab Datenschutz', async () => {
    const { mock, user } = renderApp({ pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: /Durchsuchen/ }));
    await screen.findByRole('application', { name: 'Signatur' });
    await user.click(screen.getByRole('button', { name: 'Signieren' }));
    await screen.findByRole('heading', { level: 1, name: 'Signiermittel wählen' });
    await user.click(screen.getByRole('link', { name: 'Details zum Datenschutz' }));
    const dialog = await screen.findByRole('dialog', { name: 'Konfiguration' });
    expect(within(dialog).getByRole('tab', { name: 'Datenschutz' })).toHaveAttribute('aria-selected', 'true');
    expect(within(dialog).getByRole('heading', { name: 'Datenschutzinformation' })).toBeInTheDocument();
    // vollständiger Text aus PDF-Over mit Link zur Datenschutzerklärung von A-Trust, Hinweis auf lokales Signieren
    expect(within(dialog).getByText(/zu signierende PDF Dokument an die Firma A-Trust gesendet/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Keystore oder der Bürgerkarte .* nicht hochgeladen/)).toBeInTheDocument();
    const link = within(dialog).getByRole('link', { name: /a-trust\.at\/downloads\/documents\/datenschutzerklaerung\.pdf/ });
    // Link als echter Inline-Link; der Satzpunkt folgt direkt im selben Absatz
    expect(link.tagName).not.toBe('BUTTON');
    expect(link.nextSibling?.textContent).toBe('.');
    await user.click(link);
    expect(mock.openedUrls).toEqual(['https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf']);
    // Schließen: zurück zur Auswahl, nichts wurde gestartet
    await user.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByRole('heading', { level: 1, name: 'Signiermittel wählen' })).toBeInTheDocument();
    expect(mock.dispatched.some((e) => e.type === 'choose_backend')).toBe(false);
    // erneutes Öffnen über die Hauptleiste beginnt wieder bei „Einfach“
    await user.click(screen.getByRole('button', { name: 'Konfiguration' }));
    const again = await screen.findByRole('dialog', { name: 'Konfiguration' });
    expect(within(again).getByRole('tab', { name: 'Einfach' })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('Platzhalter', () => {
  it('mehrere Felder: Auswahl per Tastatur, Ja sendet den Index', async () => {
    const { mock, user } = renderApp({
      view: viewOf({
        step: 'placeholder',
        doc: { id: 1, name: 'Formular.pdf', page_count: 1, page_sizes: [[595, 842]], signed: false },
        fields: [
          { name: 'Auftraggeber', page: 0 },
          { name: 'Auftragnehmer', page: null },
        ],
        qr: null,
      }),
    });
    const group = await screen.findByRole('radiogroup', { name: /Platzhalter aus der Liste/ });
    const radios = within(group).getAllByRole('radio');
    expect(radios[0]).toBeChecked();
    await user.click(radios[0] as HTMLElement);
    await user.keyboard('{ArrowDown}');
    expect(radios[1]).toBeChecked();
    await user.click(screen.getByRole('button', { name: 'Ja' }));
    await user.click(screen.getByRole('button', { name: 'Nein' }));
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(mock.dispatched).toEqual([
      { type: 'placeholder_answer', answer: 'Yes', field_index: 1 },
      { type: 'placeholder_answer', answer: 'No', field_index: null },
      { type: 'placeholder_answer', answer: 'Cancel', field_index: null },
    ]);
  });

  it('QR-Platzhalter (EN)', async () => {
    const { mock, user } = renderApp({
      view: viewOf(
        {
          step: 'placeholder',
          doc: { id: 1, name: 'Form.pdf', page_count: 2, page_sizes: [[595, 842], [595, 842]], signed: false },
          fields: [],
          qr: { page: 1, id: '1' },
        },
        { language: 'en' },
      ),
    });
    expect(await screen.findByText('QR code placeholder on page 2')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Yes' }));
    expect(mock.dispatched).toEqual([{ type: 'placeholder_answer', answer: 'Yes', field_index: null }]);
  });
});
