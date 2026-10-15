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

  it('Datenschutzhinweis: Dialog mit Link, Zustimmen/Ablehnen', async () => {
    const { mock, user } = renderApp({ view: backendView(OPTIONS, true) });
    const dialog = await screen.findByRole('alertdialog', { name: 'Datenschutzinformation' });
    // Link als echter Inline-Link (Befund: Satzpunkt nicht mehr allein in der nächsten Zeile)
    const link = within(dialog).getByRole('link', { name: /a-trust\.at\/downloads/ });
    expect(link.tagName).not.toBe('BUTTON');
    // der Satzpunkt folgt direkt dem Link im selben Absatz (kein Block dazwischen)
    expect(link.nextSibling?.textContent).toBe('.');
    await user.click(link);
    expect(mock.openedUrls).toEqual(['https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf']);
    await user.click(within(dialog).getByRole('button', { name: 'Ablehnen' }));
    await user.click(within(dialog).getByRole('button', { name: 'Zustimmen und fortfahren' }));
    expect(mock.dispatched).toEqual([
      { type: 'acknowledge_privacy', accepted: false },
      { type: 'acknowledge_privacy', accepted: true },
    ]);
  });

  it('Escape im Datenschutzhinweis lehnt ab', async () => {
    const { mock, user } = renderApp({ view: backendView(OPTIONS, true) });
    await screen.findByRole('alertdialog');
    await user.keyboard('{Escape}');
    await waitFor(() => expect(mock.dispatched).toEqual([{ type: 'acknowledge_privacy', accepted: false }]));
  });

  it('eigener Datenschutztext per Richtlinie', async () => {
    renderApp({
      view: viewOf({ step: 'backend', options: OPTIONS, privacy_notice: { backend: 'atrust_native', version: 2, custom_text: 'Hinweis der Organisation' } }),
    });
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText('Hinweis der Organisation')).toBeInTheDocument();
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
