/**
 * „QR-Platzhalter erstellen“ (ADR 0004 Nachtrag 24): Start über Startseite und Befehlsliste, Vorschau des Kerns mit
 * sprechendem Alternativtext, Prüfung der Nummer, Speichern als PNG/SVG – am Desktop mit Speichern-Dialog, auf
 * Mobilgeräten ohne Pfad (Teilen); ohne axe-Fehler.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../test/a11y';
import { lastEvent, renderApp } from '../../test/render';
import { onMobile, viewOf } from '../../test/views';

async function openFromStart(options: Parameters<typeof renderApp>[0] = {}) {
  const r = renderApp(options);
  const card = await screen.findByRole('region', { name: 'Dokument für andere vorbereiten' });
  await r.user.click(within(card).getByRole('button', { name: 'QR-Platzhalter erstellen' }));
  const dialog = await screen.findByRole('dialog', { name: 'QR-Platzhalter erstellen' });
  return { ...r, dialog };
}

describe('QR-Platzhalter erstellen', () => {
  it('Startseite öffnet den Dialog; Vorschau mit Nummer, Signaturbild und Sprache; ohne axe-Fehler', async () => {
    const { dialog, user } = await openFromStart();
    expect(within(dialog).getByRole('textbox', { name: 'Nummer' })).toHaveValue('0001');
    expect(within(dialog).getByRole('radio', { name: /^Standard/ })).toBeChecked();
    expect(within(dialog).getByRole('img', { name: 'QR-Code-Platzhalter Nummer 0001, Signaturbild Standard, Deutsch' })).toBeInTheDocument();
    await user.click(within(dialog).getByRole('radio', { name: /^Amtssignatur/ }));
    await user.click(within(dialog).getByRole('radio', { name: 'English' }));
    expect(within(dialog).getByTestId('qr-preview')).toHaveAccessibleName('QR-Code-Platzhalter Nummer 0001, Signaturbild Amtssignatur, English');
    // „Wie bei der unterzeichnenden Person“: die Sprache betrifft nur die Beschriftung
    await user.click(within(dialog).getByRole('radio', { name: /^Wie bei der unterzeichnenden Person/ }));
    expect(within(dialog).getByRole('radiogroup', { name: 'Sprache der Beschriftung' })).toBeInTheDocument();
    await expectNoAxeViolations(dialog);
  });

  it('Desktop: Speichern-Dialog mit Vorschlag, dann export_qr_placeholder mit Pfad; ungültige Nummer wird gemeldet', async () => {
    const { dialog, mock, user } = await openFromStart();
    const id = within(dialog).getByRole('textbox', { name: 'Nummer' });
    await user.clear(id);
    await user.type(id, '12a');
    await user.click(within(dialog).getByRole('button', { name: 'Als PNG speichern' }));
    expect(id).toHaveAccessibleDescription('Die Nummer darf nur Ziffern enthalten (1 bis 10 Stellen).');
    expect(mock.dispatched.some((e) => e.type === 'export_qr_placeholder')).toBe(false);
    // Vorschau bleibt bei der letzten gültigen Nummer
    expect(within(dialog).getByTestId('qr-preview')).toHaveAccessibleName(/Nummer 12,/);
    await user.clear(id);
    await user.type(id, '0002');
    await user.click(within(dialog).getByRole('button', { name: 'Als SVG speichern' }));
    await waitFor(() => expect(lastEvent(mock)?.type).toBe('export_qr_placeholder'));
    expect(mock.nativeCalls).toContain('pickSavePath:qr-platzhalter-0002.svg');
    expect(lastEvent(mock)).toEqual({
      type: 'export_qr_placeholder',
      spec: { id: '0002', profile: 'standard', language: 'de' },
      format: 'svg',
      path: 'qr-platzhalter-0002.svg',
    });
  });

  it('Mobile: „PNG teilen“ ohne Pfad (der Kern teilt)', async () => {
    const { dialog, mock, user } = await openFromStart({ view: onMobile(viewOf({ step: 'open', queue: [] })) });
    await user.click(within(dialog).getByRole('button', { name: 'PNG teilen' }));
    expect(lastEvent(mock)).toEqual({ type: 'export_qr_placeholder', spec: { id: '0001', profile: 'standard', language: 'de' }, format: 'png', path: null });
    expect(mock.nativeCalls.some((c) => c.startsWith('pickSavePath'))).toBe(false);
  });

  it('auch über die Befehlsliste erreichbar; Schließen beendet ohne Ereignis', async () => {
    const { mock, user } = renderApp();
    await screen.findByRole('heading', { name: 'Was möchten Sie signieren?' });
    await user.keyboard('{Control>}k{/Control}');
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    await user.type(within(palette).getByRole('searchbox', { name: 'Befehl suchen …' }), 'platzhalter');
    await user.click(await within(palette).findByRole('menuitem', { name: /QR-Platzhalter erstellen/ }));
    const dialog = await screen.findByRole('dialog', { name: 'QR-Platzhalter erstellen' });
    await user.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'QR-Platzhalter erstellen' })).not.toBeInTheDocument());
    expect(mock.dispatched.some((e) => e.type === 'export_qr_placeholder')).toBe(false);
  });
});
