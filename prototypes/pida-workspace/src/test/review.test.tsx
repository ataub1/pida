/**
 * Absicherung der Befunde aus dem UX- und Barrierefreiheits-Review: Fehlerzustand im Stepper nicht nur
 * über die Farbe, Einzahl/Mehrzahl im Stapel, Speicherort in der Zusammenfassung, Überdeckungswarnung
 * direkt bei „Signieren“, Fokus nach Overlays und Demo-Steuerung, Einstellungssuche, Tastenkürzel,
 * Vorwarnung vor Ablauf der App-Bestätigung, unterscheidbare Statusfarben im Stapel.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations, login, renderApp, signingDialog } from './render';

type User = ReturnType<typeof renderApp>['user'];

async function confirmInApp(user: User, dialog: HTMLElement) {
  await user.click(await within(dialog).findByRole('button', { name: 'In App bestätigt (Demo)' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
}

async function signSingleFromBatch(user: User, fileName: string) {
  await user.click(screen.getByRole('button', { name: `„${fileName}“ öffnen und positionieren` }));
  await user.click(await screen.findByRole('button', { name: /^Signieren/ }));
  await confirmInApp(user, await login(user));
  await screen.findByRole('list', { name: 'Dokumente im Stapel' });
}

describe('Review: Signierkarte', () => {
  it('zeigt einen Fehler im Stepper mit Symbol und Screenreader-Text, nicht nur farbig', async () => {
    const { user } = renderApp({ scenario: 'error-atrust', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    await within(dialog).findByRole('heading', { name: 'ID Austria ist gerade nicht erreichbar' });
    const steps = within(dialog).getByRole('list', { name: 'Fortschritt' });
    const current = steps.querySelector('[aria-current="step"]') as HTMLElement;
    expect(current).toHaveClass('is-error');
    expect(current).toHaveTextContent('Anmelden, fehlgeschlagen');
    expect(current.querySelector('.step-dot')).toHaveTextContent('!');
    await expectNoAxeViolations();
  });

  it('„Signaturdaten anzeigen“ gehört zum Vergleichswert, nicht zu den Alternativen', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    const data = await within(dialog).findByRole('button', { name: 'Signaturdaten anzeigen' });
    expect(data.closest('.refvalue')).not.toBeNull();
    const alternatives = within(dialog).getByText('Stattdessen:').parentElement as HTMLElement;
    expect(within(alternatives).queryByRole('button', { name: 'Signaturdaten anzeigen' })).not.toBeInTheDocument();
    expect(within(alternatives).getByRole('button', { name: 'FIDO2-Schlüssel' })).toBeInTheDocument();
  });

  it('kündigt eine Minute vor Ablauf der App-Bestätigung einmal an', async () => {
    const { user } = renderApp({ scenario: 'contract', privacyAcked: true, timing: { appTimeoutSecs: 60.3, tickMs: 20 } });
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await login(user);
    expect(await within(dialog).findByText('Noch eine Minute für die Bestätigung in der App.', {}, { timeout: 3000 })).toBeInTheDocument();
  });
});

describe('Review: Stapel', () => {
  it('verwendet die Einzahl, wenn nur ein Dokument signiert wird, und nennt den Speicherort richtig', async () => {
    const { user } = renderApp({ scenario: 'batch', privacyAcked: true });
    await signSingleFromBatch(user, 'Angebot_2026-118.pdf');
    await signSingleFromBatch(user, 'Auftragsbestaetigung_4711.pdf');

    const signAll = screen.getByRole('button', { name: /Alle signieren/ });
    expect(signAll).toHaveTextContent('1');
    await user.click(signAll);
    const dialog = await signingDialog();
    expect(within(dialog).getByText('1 Dokument – eine Anmeldung')).toBeInTheDocument();
    await login(user);
    await confirmInApp(user, dialog);

    expect(await screen.findByText('1 Dokument signiert')).toBeInTheDocument();
    expect(screen.getByText('Die Signatur wurde geprüft und ist gültig.')).toBeInTheDocument();
    // „Neben dem Original“: Dokumente aus verschiedenen Ordnern – kein einzelner Ordner behaupten
    expect(screen.getByText('3 von 4 Dokumenten signiert, jeweils neben dem Original gespeichert.')).toBeInTheDocument();
    // vorher einzeln signierte Dokumente gelten nicht als „übersprungen“
    expect(screen.getByText('1 Dokument wurde wegen eines Fehlers übersprungen.')).toBeInTheDocument();
  });

  it('nennt bei festem Ordner diesen Ordner', async () => {
    const { user } = renderApp({ scenario: 'batch', privacyAcked: true, settings: { saveTarget: 'folder' } });
    await user.click(screen.getByRole('button', { name: /Alle signieren/ }));
    await confirmInApp(user, await login(user));
    expect(await screen.findByText('3 von 4 Dokumenten signiert, gespeichert in ~/Dokumente/Signiert.')).toBeInTheDocument();
  });

  it('„Wird signiert“ ist blau (läuft), nicht im Rot von „Fehler“; Status hängt am Zeilenknopf', async () => {
    const { user } = renderApp({ scenario: 'batch', privacyAcked: true, timing: { batchItemMs: 60_000 } });
    const list = screen.getByRole('list', { name: 'Dokumente im Stapel' });
    expect(within(list).getByText('Positioniert').closest('.badge')).toHaveClass('badge-neutral');
    expect(screen.getByRole('button', { name: '„Angebot_2026-118.pdf“ öffnen und positionieren' })).toHaveAccessibleDescription(
      'Bereit, automatisch platziert',
    );
    await user.click(screen.getByRole('button', { name: /Alle signieren/ }));
    await confirmInApp(user, await login(user));
    const signing = (await within(list).findAllByText('Wird signiert')).find((el) => el.closest('.badge'));
    expect(signing?.closest('.badge')).toHaveClass('badge-info');
    expect(within(list).getByText('Fehler').closest('.badge')).toHaveClass('badge-danger');
  });
});

describe('Review: Arbeitsbereich', () => {
  it('zeigt die Überdeckungswarnung direkt über „Signieren“ (nicht unten in der Seitenleiste)', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    const sticker = screen.getByRole('application', { name: 'Signaturblock' });
    sticker.focus();
    for (let i = 0; i < 6; i++) await user.keyboard('{Alt>}{ArrowUp}{/Alt}');
    const inspector = screen.getByRole('complementary', { name: 'Signatur' });
    const warning = within(inspector).getByText('Der Signaturblock überdeckt Inhalt der Seite.').closest('.callout') as HTMLElement;
    expect(warning).toHaveClass('inspector-warn');
    expect(warning.closest('.inspector-scroll')).toBeNull();
    const sign = within(inspector).getByRole('button', { name: /^Signieren/ });
    expect(warning.compareDocumentPosition(sign) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    await expectNoAxeViolations();
  });
});

describe('Review: Werkzeugleiste der Seitenansicht', () => {
  it('ist eine Toolbar mit Pfeiltasten-Navigation', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    const toolbar = screen.getByRole('toolbar', { name: 'Ansicht' });
    const toggle = within(toolbar).getByRole('button', { name: 'Seitenleiste ein-/ausblenden' });
    toggle.focus();
    await user.keyboard('{ArrowRight}');
    expect(within(toolbar).getByRole('button', { name: 'Verkleinern' })).toHaveFocus();
    await user.keyboard('{ArrowRight}{ArrowRight}');
    expect(within(toolbar).getByRole('button', { name: 'An Breite anpassen' })).toHaveFocus();
    await user.keyboard('{ArrowLeft}');
    expect(within(toolbar).getByRole('button', { name: 'Vergrößern' })).toHaveFocus();
  });
});

describe('Review: Fokus', () => {
  it('setzt den Fokus auf den Hauptbereich, wenn der Auslöser eines Overlays nicht mehr existiert', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Befehle' });
    await user.type(within(palette).getByRole('searchbox'), 'einst');
    await user.keyboard('{ArrowDown}{Enter}');
    await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    await waitFor(() => expect(document.getElementById('main')).toHaveFocus());
  });

  it('nimmt den Fokus beim Auf- und Zuklappen der Demo-Steuerung mit', async () => {
    const { user } = renderApp({ scenario: 'start' });
    await user.click(screen.getByRole('button', { name: 'Demo' }));
    const panel = screen.getByRole('complementary', { name: 'Demo-Steuerung' });
    await waitFor(() => expect(within(panel).getByRole('button', { name: /Start \(kein Dokument\)/ })).toHaveFocus());
    await user.click(within(panel).getByRole('button', { name: 'Demo-Steuerung einklappen' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Demo' })).toHaveFocus());
  });
});

describe('Review: kennwortgeschütztes PDF', () => {
  it('unterscheidet leeres und falsches Kennwort und sagt den Fehler an', async () => {
    const { user } = renderApp({ scenario: 'error-password' });
    await user.click(screen.getByRole('button', { name: 'Entsperren' }));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Bitte geben Sie das Kennwort ein.');
    expect(screen.getByLabelText('Kennwort')).toHaveAccessibleDescription('Bitte geben Sie das Kennwort ein.');
    await user.type(screen.getByLabelText('Kennwort'), 'falsch');
    await user.click(screen.getByRole('button', { name: 'Entsperren' }));
    expect(await screen.findByText('Das Kennwort stimmt nicht. Bitte versuchen Sie es noch einmal.')).toBeInTheDocument();
  });
});

describe('Review: Mock-Daten „Zuletzt signiert“', () => {
  it('jedes Dokument der Liste enthält eine Signatur von Max Mustermann, Nummern passen zum Dateinamen', async () => {
    for (const name of ['Mietvertrag_signiert.pdf', 'Angebot_2026-117_signiert.pdf', 'Kaufvertrag_Fahrzeug_signiert.pdf']) {
      const { user, unmount } = renderApp({ scenario: 'start' });
      await user.click(screen.getByRole('button', { name: `„${name}“ im Prüfmodus öffnen` }));
      expect(await screen.findByRole('heading', { level: 1, name })).toBeInTheDocument();
      expect(screen.getByRole('heading', { level: 3, name: 'Max Mustermann' })).toBeInTheDocument();
      if (name.startsWith('Angebot')) {
        expect(document.body.textContent).toContain('Angebot Nr. 2026-117');
        expect(document.body.textContent).not.toContain('2026-118');
      }
      unmount();
    }
  });
});

describe('Review: Einstellungen und Kürzel', () => {
  it('Suche: Löschen-Knopf richtig benannt, Signaturbild-Treffer zeigen die Live-Vorschau', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.type(within(dialog).getByRole('searchbox', { name: 'Einstellungen durchsuchen' }), 'logo');
    expect(within(dialog).getByText('Bildmarke')).toBeInTheDocument();
    expect(within(dialog).getByRole('img', { name: /^Vorschau des Signaturblocks/ })).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Suche löschen' })).toBeInTheDocument();
  });

  it('Tastenkürzel-Übersicht nennt auch Alt (50 pt) und den Doppelklick', async () => {
    renderApp({ scenario: 'contract' });
    fireEvent.keyDown(window, { key: '?' });
    const dialog = await screen.findByRole('dialog', { name: 'Tastenkürzel' });
    expect(within(dialog).getByText('Grob verschieben (50 pt)')).toBeInTheDocument();
    expect(within(dialog).getByText('Alt ←')).toBeInTheDocument();
    expect(within(dialog).getByText('Block an diese Stelle setzen')).toBeInTheDocument();
  });
});
