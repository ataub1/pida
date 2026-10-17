/**
 * Keine Signiermittel-Auswahl bei nur einem Signiermittel (ADR 0004 Nachtrag 20; Meldung des Auftraggebers: „In der
 * mobilen App brauchen wir die Signiermittelauswahl nicht, es gibt nur ID Austria“). Der Kern entscheidet
 * (`SignersView::choice_offered`), die Oberfläche rendert: ohne Auswahl kein Konto-Chip mit Menü, kein Abschnitt
 * „Signieren mit“, keine Wahl in Stapel, Ersteinrichtung und Signierkarte; „Signieren“ signiert mit dem vom Kern
 * gewählten Signiermittel, der Kurzhinweis Datenschutz bleibt. Mobile (iOS/Android) prüfen `Mobile.test.tsx` und
 * `MobileIdAustriaOnly.test.tsx`; hier der Desktop mit einer Richtlinie, die nur ein Signiermittel freigibt (Mock-Kern
 * `allowedBackends` wie `allowed_backends`), und die Gegenprobe: Desktop mit mehreren Signiermitteln **unverändert**.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/defaults';
import type { Settings } from '../core/types';
import { expectNoAxeViolations } from '../test/a11y';
import { emulateMedia, lastEvent, renderApp } from '../test/render';

/** „Jedes Mal fragen“ (kein Standard), ID Austria mit gemerktem Benutzer – wie eine frische Desktop-Installation. */
const ASK: Settings = { ...defaultSettings(), backend: { default: null }, id_austria: { ...defaultSettings().id_austria, username: '+43 660 1234123', remember: true } };
const ONLY_IDA = { allowedBackends: ['atrust_native' as const], settings: ASK };

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

const inspector = () => screen.getByRole('complementary', { name: 'Signatur' });
const footer = () => inspector().querySelector('.inspector-footer') as HTMLElement;

describe('Desktop mit nur einem Signiermittel (Richtlinie): keine Auswahl', () => {
  it('Arbeitsbereich breit: kein „Signieren mit“, kein Konto-Chip, Kurzhinweis über „Signieren“; signiert direkt mit ID Austria', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', ...ONLY_IDA });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(within(inspector()).queryByRole('heading', { name: 'Signieren mit' })).not.toBeInTheDocument();
    expect(screen.queryByTestId('signer-chip')).not.toBeInTheDocument();
    expect(footer()).toHaveClass('is-single');
    expect(within(footer()).getByTestId('privacy-note')).toHaveTextContent('Das Dokument wird zum Signieren an A-Trust übermittelt.');
    const view = await mock.view();
    expect(view.signers).toMatchObject({ choice_offered: false, selected: 'atrust_native', default: null });
    await expectNoAxeViolations();
    // „Jedes Mal fragen“ – trotzdem nichts zu fragen: „Signieren“ (die ausdrückliche Aktion) signiert, kein Menü
    await user.click(within(footer()).getByRole('button', { name: /^Signieren/ }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_with', backend: 'atrust_native' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(await screen.findByRole('dialog', { name: 'Signieren mit ID Austria' })).toBeInTheDocument();
  });

  it('Arbeitsbereich schmal: „Signieren“ über die volle Breite, kein Konto-Chip; axe', async () => {
    restore = emulateMedia({ narrow: true });
    renderApp({ scenario: 'contract', ...ONLY_IDA });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(footer()).toHaveClass('is-single');
    expect(inspector().querySelector('.inspector-footer-chip')).toBeNull();
    expect(within(inspector()).getByTestId('privacy-note')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('Stapel: kein Konto-Chip, Schätzung für ID Austria, „Alle signieren“ ohne Menü', async () => {
    const { mock, user } = renderApp({ scenario: 'batch', ...ONLY_IDA });
    const signAll = await screen.findByRole('button', { name: /Alle signieren/ });
    expect(screen.queryByTestId('signer-chip')).not.toBeInTheDocument();
    expect(screen.getByTestId('batch-estimate')).toHaveTextContent('1 Anmeldung');
    expect(screen.queryByText('Wählen Sie beim Signieren das Signiermittel.')).not.toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(signAll);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    await waitFor(() => expect(mock.dispatched.some((e) => e.type === 'start_batch' && e.backend === 'atrust_native') || screen.queryByRole('dialog') !== null).toBe(true));
  });

  it('Ersteinrichtung ohne Schritt „Standard-Signiermittel“', async () => {
    renderApp({ firstRun: true, allowedBackends: ['atrust_native'] });
    const dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expect(within(dialog).getByText('Schritt 1 von 1')).toBeInTheDocument();
    expect(within(dialog).queryByText('Jedes Mal fragen')).not.toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('nur die Chipkarte, Chipkarten-Software läuft nicht: „Signieren“ nennt den Grund statt „Bitte wählen Sie ein anderes“', async () => {
    const { user } = renderApp({ scenario: 'contract', allowedBackends: ['local_bku'], localBkuAvailable: false, settings: ASK });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(screen.queryByTestId('signer-chip')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    expect(await screen.findByText(/Die Chipkarten-Software \(lokale BKU\) läuft nicht/)).toBeInTheDocument();
    expect(screen.queryByText(/Bitte wählen Sie ein anderes/)).not.toBeInTheDocument();
    expect(screen.queryByRole('dialog', { name: /Signieren mit/ })).not.toBeInTheDocument();
  });
});

describe('Desktop mit mehreren Signiermitteln: Auswahl unverändert (Gegenprobe)', () => {
  it('breit: Abschnitt „Signieren mit“ mit Konto-Chip und Menü „Signiermittel wechseln“', async () => {
    const { mock, user } = renderApp({ scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(within(inspector()).getByRole('heading', { name: 'Signieren mit' })).toBeInTheDocument();
    expect(footer()).not.toHaveClass('is-single');
    expect((await mock.view()).signers.choice_offered).toBe(true);
    await user.click(screen.getByTestId('signer-chip'));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByText('Signiermittel wechseln')).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: /Chipkarte/ })).toBeInTheDocument();
  });

  it('schmal: Konto-Chip neben „Signieren“', async () => {
    restore = emulateMedia({ narrow: true });
    renderApp({ scenario: 'contract' });
    await screen.findByRole('application', { name: 'Signaturblock' });
    expect(within(footer()).getByTestId('signer-chip')).toBeInTheDocument();
    expect(footer()).not.toHaveClass('is-single');
  });

  it('„Jedes Mal fragen“: „Signieren“ öffnet das Menü, erst die Wahl signiert', async () => {
    const { mock, user } = renderApp({ scenario: 'contract', settings: ASK });
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const menu = await screen.findByRole('menu');
    expect(menu).toHaveTextContent('Signieren mit');
    expect(mock.dispatched.some((e) => e.type === 'sign_with')).toBe(false);
    await user.click(within(menu).getByRole('menuitem', { name: /ID Austria/ }));
    expect(lastEvent(mock)).toEqual({ type: 'sign_with', backend: 'atrust_native' });
  });

  it('Stapel mit Konto-Chip; Ersteinrichtung mit Schritt „Standard-Signiermittel“', async () => {
    const batch = renderApp({ scenario: 'batch' });
    await screen.findByRole('button', { name: /Alle signieren/ });
    expect(screen.getByTestId('signer-chip')).toBeInTheDocument();
    batch.unmount();
    renderApp({ firstRun: true });
    const dialog = await screen.findByRole('dialog', { name: 'Willkommen bei PIDA' });
    expect(within(dialog).getByText('Schritt 1 von 2')).toBeInTheDocument();
    expect(within(dialog).getByRole('radio', { name: /Jedes Mal fragen/ })).toBeInTheDocument();
  });
});
