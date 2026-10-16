/**
 * Einstellungen › Signiermittel › Keystore (Wünsche des Auftraggebers, Nachtrag ADR 0004): keine Typauswahl
 * mehr – das Format erkennt der Kern am Inhalt (`keystore_format`; Hinweise je Format, als Standard nur mit
 * PKCS#12 wählbar) –, der Schlüssel (Alias) als Auswahl aus `keystore_entries` (genau ein Schlüssel als Text,
 * mehrere als Liste, nur Schlüsseleinträge, fehlender Alias als Warnung), „Schlüssel anzeigen …“ mit
 * Passwortfeld (falsches Passwort als Feldfehler) und „Passwort merken?“ bei der ersten Verwendung – in den
 * Einstellungen und in der Signierkarte. Jeweils mit axe.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../../core/defaults';
import { MOCK_KEYSTORE_ENTRIES } from '../../core/mock/settings';
import type { InteractionView, KeystoreEntryView, Settings, SettingsView } from '../../core/types';
import { expectNoAxeViolations } from '../../test/a11y';
import { lastEvent, renderApp } from '../../test/render';
import { cardView, IOS, onMobile, settingsView } from '../../test/views';

const FILE = '/Users/demo/Zertifikate/max.p12';

function withKeystore(file: string | null = FILE, extra: Partial<Settings['keystore']> = {}): Settings {
  const base = defaultSettings();
  return { ...base, keystore: { ...base.keystore, enabled: true, file, ...extra } };
}

/** Einstellungen (statische Ansicht) im Bereich Signiermittel. */
async function signers(view: ReturnType<typeof settingsView>) {
  const r = renderApp({ view });
  const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
  await r.user.click(within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Signiermittel' }));
  return { ...r, dialog, keystore: within(dialog).getByRole('region', { name: 'Keystore' }) };
}

/** Einstellungen mit dem Mock-Kern im Bereich Signiermittel. */
async function signersLive(options: Parameters<typeof renderApp>[0]) {
  const r = renderApp(options);
  await r.user.click(await screen.findByRole('button', { name: 'Einstellungen' }));
  const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
  await r.user.click(within(within(dialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Signiermittel' }));
  return { ...r, dialog, keystore: within(dialog).getByRole('region', { name: 'Keystore' }) };
}

const defaultGroup = (dialog: HTMLElement) => within(dialog).getByRole('radiogroup', { name: 'Standard-Signiermittel' });

describe('Keystore: Format am Inhalt statt Typauswahl', () => {
  it('PKCS#12: dezent „Format: PKCS#12“, keine Typsegmente, als Standard wählbar', async () => {
    const { dialog, keystore } = await signers(settingsView(withKeystore(), { keystore_format: 'pkcs12', keystore_ready: true }));
    expect(within(keystore).getByText('Format: PKCS#12')).toHaveClass('field-hint');
    expect(within(keystore).queryByRole('radiogroup', { name: 'Typ' })).toBeNull();
    for (const name of ['PKCS#12', 'JKS', 'JCEKS']) expect(within(keystore).queryByRole('radio', { name })).toBeNull();
    expect(within(defaultGroup(dialog)).getByRole('radio', { name: /Keystore/ })).toBeEnabled();
    await expectNoAxeViolations();
  });

  it.each([
    ['jks', 'JKS- und JCEKS-Keystores werden nicht unterstützt.'],
    ['jceks', 'JKS- und JCEKS-Keystores werden nicht unterstützt.'],
    ['unknown', 'Diese Datei ist kein unterstützter Keystore. Bitte wählen Sie eine PKCS#12-Datei (.p12/.pfx).'],
  ] as const)('%s: verständlicher Hinweis als Warnung (kein Fehler), als Standard nicht wählbar', async (format, text) => {
    const { dialog, keystore } = await signers(settingsView(withKeystore('/Users/demo/alt.jks'), { keystore_format: format, keystore_ready: false }));
    const note = within(keystore).getByText(new RegExp(text.replace(/[.()/#]/g, '\\$&')));
    expect(note).toHaveClass('message-warning');
    expect(note).not.toHaveClass('field-error');
    expect(note).not.toHaveAttribute('role', 'alert');
    expect(within(defaultGroup(dialog)).getByRole('radio', { name: /Keystore/ })).toBeDisabled();
    // ohne PKCS#12 keine Schlüsselauswahl
    expect(within(keystore).queryByRole('button', { name: 'Schlüssel anzeigen …' })).toBeNull();
    await expectNoAxeViolations();
  });

  it('Datei wählen setzt nur `keystore.file` (kein `declared_type` aus der Endung)', async () => {
    const { mock, user, keystore } = await signersLive({ settings: withKeystore(null), pickFilesResult: ['/Users/demo/alt.jks'] });
    await user.click(within(keystore).getByRole('button', { name: 'Datei wählen …' }));
    await waitFor(() => expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'keystore.file': '/Users/demo/alt.jks' }, revision: 1 }));
    expect(await within(keystore).findByText(/JKS- und JCEKS-Keystores werden nicht unterstützt/)).toBeInTheDocument();
    expect((await mock.view()).settings?.settings.keystore.declared_type).toBeNull();
  });
});

const one: KeystoreEntryView[] = [
  { alias: 'max', subject_cn: 'Max Mustermann (NUR TEST)', not_after: '2027-03-31T12:00:00Z', is_key_entry: true },
  { alias: 'test-root-ca', subject_cn: 'Test Root CA (NUR TEST)', not_after: '2035-12-31T23:59:59Z', is_key_entry: false },
];

function entriesView(entries: KeystoreEntryView[], alias: string | null = null, extra: Partial<SettingsView> = {}) {
  return settingsView(withKeystore(FILE, { alias }), { keystore_format: 'pkcs12', keystore_ready: true, keystore_entries: entries, ...extra });
}

describe('Keystore: Schlüssel als Auswahl', () => {
  it('genau ein Schlüssel: als Text „Name · gültig bis …“, Alias klein darunter; nichts zu wählen', async () => {
    const { mock, keystore } = await signers(entriesView(one));
    const key = within(keystore).getByTestId('keystore-key');
    expect(key).toHaveTextContent('Max Mustermann (NUR TEST) · gültig bis 31.03.2027');
    expect(key).toHaveTextContent('Alias: max');
    // das Zertifikat ohne Schlüssel ist nicht wählbar und erscheint nicht
    expect(keystore).not.toHaveTextContent('Test Root CA');
    expect(within(keystore).queryByRole('button', { name: /Schlüssel wählen/ })).toBeNull();
    expect(within(keystore).queryByRole('textbox', { name: 'Alias' })).toBeNull();
    expect(mock.dispatched.filter((e) => e.type === 'apply_settings')).toEqual([]);
    await expectNoAxeViolations();
  });

  it('ein Schlüssel, gespeicherter Alias fehlt: Warnung und „Diesen Schlüssel verwenden“ (Alias leer)', async () => {
    const { mock, user, keystore } = await signers(entriesView(one, 'alt'));
    expect(within(keystore).getByText('Der gespeicherte Schlüssel „alt“ ist in diesem Keystore nicht enthalten. Bitte wählen Sie einen Schlüssel.')).toHaveClass('message-warning');
    await user.click(within(keystore).getByRole('button', { name: 'Diesen Schlüssel verwenden' }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'keystore.alias': null }, revision: 1 });
  });

  it('mehrere Schlüssel: Auswahl nur mit Schlüsseleinträgen, die Wahl setzt `keystore.alias`', async () => {
    const { mock, user, keystore } = await signers(entriesView(MOCK_KEYSTORE_ENTRIES));
    const trigger = within(keystore).getByRole('button', { name: /Schlüssel/ });
    expect(trigger).toHaveTextContent('Schlüssel wählen');
    expect(within(keystore).getByText('Der Keystore enthält mehrere Schlüssel. Bitte wählen Sie den Schlüssel zum Signieren.')).toBeInTheDocument();
    await user.click(trigger);
    const list = await screen.findByRole('listbox');
    const options = within(list).getAllByRole('option');
    expect(options.map((o) => o.textContent)).toEqual([
      'Max Mustermann (NUR TEST) · gültig bis 31.03.2027Alias: max',
      'Muster GmbH Siegel (NUR TEST) · gültig bis 15.01.2028Alias: muster-gmbh',
    ]);
    await expectNoAxeViolations();
    await user.click(within(list).getByRole('option', { name: /Muster GmbH/ }));
    expect(lastEvent(mock)).toEqual({ type: 'apply_settings', changes: { 'keystore.alias': 'muster-gmbh' }, revision: 1 });
  });

  it('mehrere Schlüssel, gesetzter Alias: gewählt angezeigt (ohne Groß-/Kleinschreibung); fehlender Alias als Warnung', async () => {
    const first = await signers(entriesView(MOCK_KEYSTORE_ENTRIES, 'MAX'));
    expect(within(first.keystore).getByRole('button', { name: /Schlüssel/ })).toHaveTextContent('Max Mustermann (NUR TEST) · gültig bis 31.03.2027');
    expect(within(first.keystore).queryByText(/nicht enthalten/)).toBeNull();
    first.unmount();
    const { keystore } = await signers(entriesView(MOCK_KEYSTORE_ENTRIES, 'weg'));
    expect(within(keystore).getByRole('button', { name: /Schlüssel/ })).toHaveTextContent('Schlüssel wählen');
    expect(within(keystore).getByText('Der gespeicherte Schlüssel „weg“ ist in diesem Keystore nicht enthalten. Bitte wählen Sie einen Schlüssel.')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  it('keine Schlüssel in der Datei: Hinweis', async () => {
    const { keystore } = await signers(entriesView([one[1] as KeystoreEntryView]));
    expect(within(keystore).getByText('Diese Datei enthält keinen Schlüssel zum Signieren.')).toBeInTheDocument();
  });

  it('englisch: „valid until“ im Datum der Sprache', async () => {
    const { user } = renderApp({ view: settingsView(withKeystore(), { keystore_format: 'pkcs12', keystore_ready: true, keystore_entries: one }, 'en') });
    const dialog = await screen.findByRole('dialog', { name: 'Settings' });
    await user.click(within(within(dialog).getByRole('navigation', { name: 'Sections' })).getByRole('button', { name: 'Signing methods' }));
    expect(within(dialog).getByTestId('keystore-key')).toHaveTextContent('Max Mustermann (NUR TEST) · valid until 31 Mar 2027');
  });
});

describe('Keystore: „Schlüssel anzeigen …“ mit Passwort', () => {
  it('falsches Passwort → Feldfehler; richtiges → Auswahl mit zwei Schlüsseln (Mock: „test“)', async () => {
    const { mock, user, keystore } = await signersLive({ settings: withKeystore() });
    expect(within(keystore).getByText('Format: PKCS#12')).toBeInTheDocument();
    await user.click(within(keystore).getByRole('button', { name: 'Schlüssel anzeigen …' }));
    const field = within(keystore).getByLabelText('Passwort des Keystores');
    await waitFor(() => expect(field).toHaveFocus());
    // bereits gewählte Speicherart: keine Rückfrage
    expect(within(keystore).queryByRole('radiogroup', { name: 'Passwort merken?' })).toBeNull();
    await user.click(within(keystore).getByRole('button', { name: 'Anzeigen' }));
    expect(within(keystore).getByText('Bitte geben Sie das Passwort ein.')).toBeInTheDocument();
    await user.type(field, 'falsch-1{Enter}');
    expect(lastEvent(mock)).toEqual({ type: 'inspect_keystore', password: 'falsch-1' });
    const error = await within(keystore).findByText('Das Passwort ist falsch. Bitte versuchen Sie es erneut.');
    expect(error).toHaveClass('field-error');
    expect(field).toHaveAttribute('aria-invalid', 'true');
    await expectNoAxeViolations();
    // beim Tippen verschwindet der Fehler; richtiges Passwort
    await user.type(field, 'te');
    expect(within(keystore).queryByText('Das Passwort ist falsch. Bitte versuchen Sie es erneut.')).toBeNull();
    await user.type(field, 'st{Enter}');
    const trigger = await within(keystore).findByRole('button', { name: /Schlüssel/ });
    expect(trigger).toHaveTextContent('Schlüssel wählen');
    expect(within(keystore).queryByLabelText('Passwort des Keystores')).toBeNull();
    await user.click(trigger);
    await user.click(within(await screen.findByRole('listbox')).getByRole('option', { name: /Max Mustermann/ }));
    await waitFor(async () => expect((await mock.view()).settings?.settings.keystore.alias).toBe('max'));
    // Passwort nur im Ereignis, nie in den Einstellungen
    expect(JSON.stringify((await mock.view()).settings)).not.toContain('falsch-1');
  });

  it('Abbrechen schließt das Passwortfeld ohne Ereignis', async () => {
    const { mock, user, keystore } = await signersLive({ settings: withKeystore() });
    await user.click(within(keystore).getByRole('button', { name: 'Schlüssel anzeigen …' }));
    await user.click(within(keystore).getByRole('button', { name: 'Abbrechen' }));
    expect(within(keystore).queryByLabelText('Passwort des Keystores')).toBeNull();
    expect(mock.dispatched.some((e) => e.type === 'inspect_keystore')).toBe(false);
  });

  it('gemerktes Passwort: der Kern liefert die Schlüssel sofort – kein Knopf', async () => {
    const { keystore } = await signersLive({ settings: withKeystore(), keystorePasswordKnown: true });
    expect(within(keystore).queryByRole('button', { name: 'Schlüssel anzeigen …' })).toBeNull();
    expect(within(keystore).getByRole('button', { name: /Schlüssel/ })).toHaveTextContent('Schlüssel wählen');
  });

  it('Mobile: kein Keystore – auch mit Keystore-Einstellungen eines früheren Builds (ADR 0004 Nachtrag 11)', async () => {
    const { user } = renderApp({ view: onMobile(settingsView(withKeystore('/var/mobile/k/test.p12')), IOS) });
    const dialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    const nav = within(dialog).getByRole('navigation', { name: 'Bereiche' });
    await user.click(within(nav).getByRole('button', { name: 'Signiermittel' }));
    expect(dialog.querySelector('[data-row="signers.keystore"]')).toBeNull();
    expect(dialog.querySelector('[data-row="signers.default"]')).toBeNull();
    expect(within(dialog).queryByRole('button', { name: 'Schlüssel anzeigen …' })).toBeNull();
    expect(dialog.textContent).not.toMatch(/Keystore|PKCS|Schlüssel anzeigen/);
    await expectNoAxeViolations();
  });
});

describe('„Passwort merken?“ bei der ersten Verwendung', () => {
  it('Einstellungen: Auswahl unter dem Passwortfeld, Voreinstellung „Bis zum Beenden“, Wahl geht mit dem Absenden', async () => {
    const { mock, user, keystore } = await signersLive({ settings: withKeystore(FILE, { storage_chosen: false }) });
    await user.click(within(keystore).getByRole('button', { name: 'Schlüssel anzeigen …' }));
    const question = within(keystore).getByRole('radiogroup', { name: 'Passwort merken?' });
    expect(within(question).getByRole('radio', { name: 'Bis zum Beenden' })).toBeChecked();
    expect(within(question).getByRole('radio', { name: 'Im Schlüsselbund' })).toBeEnabled();
    expect(within(question).getByText('Sie können das später unter Einstellungen › Signiermittel ändern.')).toBeInTheDocument();
    await expectNoAxeViolations();
    await user.click(within(question).getByRole('radio', { name: 'Im Schlüsselbund' }));
    await user.type(within(keystore).getByLabelText('Passwort des Keystores'), 'test{Enter}');
    expect(mock.dispatched.find((e) => e.type === 'inspect_keystore')).toEqual({ type: 'inspect_keystore', password: 'test', password_storage: 'keychain' });
    await waitFor(async () => expect((await mock.view()).settings?.settings.keystore).toMatchObject({ password_storage: 'keychain', storage_chosen: true }));
    // danach nicht mehr gefragt (die Speicherart bleibt in den Einstellungen änderbar)
    expect(within(keystore).queryByRole('radiogroup', { name: 'Passwort merken?' })).toBeNull();
    expect(within(keystore).getByRole('radio', { name: 'Schlüsselbund' })).toBeChecked();
  });

  it('ohne Schlüsselbund: „Im Schlüsselbund“ gesperrt mit Hinweis', async () => {
    const { user, keystore } = await signers(settingsView(withKeystore(FILE, { storage_chosen: false }), { keystore_format: 'pkcs12', keystore_ready: true, keychain_available: false }));
    await user.click(within(keystore).getByRole('button', { name: 'Schlüssel anzeigen …' }));
    const question = within(keystore).getByRole('radiogroup', { name: 'Passwort merken?' });
    expect(within(question).getByRole('radio', { name: 'Im Schlüsselbund' })).toBeDisabled();
    expect(within(question).getByText('Kein Schlüsselbund verfügbar: Passwörter werden nur bis zum Programmende gespeichert.')).toBeInTheDocument();
    await expectNoAxeViolations();
  });

  const secret = (ask: boolean, keychain = true): InteractionView => ({
    kind: 'secret',
    request: 4,
    prompt: { kind: 'KeystorePassword', subject: 'max.p12', retry: false },
    ask_storage: ask,
    keychain_available: keychain,
  });
  const ksCard = (i: InteractionView) => cardView(i, { backend: 'keystore', stage: 'confirming', method: 'password', certificate: 'not_needed' });

  it('Signierkarte: beim ersten Mal unter dem Passwortfeld, kein eigener Dialog; Wahl mit dem Absenden', async () => {
    const { mock, user } = renderApp({ view: ksCard(secret(true)) });
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
    const question = within(dialog).getByRole('radiogroup', { name: 'Passwort merken?' });
    expect(within(question).getByRole('radio', { name: 'Bis zum Beenden' })).toBeChecked();
    await expectNoAxeViolations();
    await user.click(within(question).getByRole('radio', { name: 'Nicht speichern' }));
    await user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'geheim{Enter}');
    expect(mock.dispatched).toEqual([{ type: 'submit_secret', request: 4, secret: 'geheim', password_storage: 'none' }]);
  });

  it('Signierkarte: Voreinstellung ohne Änderung = „Bis zum Beenden“; „Im Schlüsselbund“ gesperrt ohne Speicher', async () => {
    const first = renderApp({ view: ksCard(secret(true)) });
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    await first.user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'geheim{Enter}');
    expect(first.mock.dispatched).toEqual([{ type: 'submit_secret', request: 4, secret: 'geheim', password_storage: 'memory' }]);
    first.unmount();
    renderApp({ view: ksCard(secret(true, false)) });
    const locked = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    expect(within(locked).getByRole('radio', { name: 'Im Schlüsselbund' })).toBeDisabled();
    await expectNoAxeViolations();
  });

  it('Signierkarte: danach nie mehr gefragt (ohne `password_storage`)', async () => {
    const { mock, user } = renderApp({ view: ksCard(secret(false)) });
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    expect(within(dialog).queryByRole('radiogroup', { name: 'Passwort merken?' })).toBeNull();
    await user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'geheim{Enter}');
    expect(mock.dispatched).toEqual([{ type: 'submit_secret', request: 4, secret: 'geheim' }]);
  });

  it('Mock: erste Verwendung fragt genau einmal; die Wahl wird gespeichert (auch nach falschem Passwort)', async () => {
    const base = withKeystore(FILE, { storage_chosen: false });
    const settings: Settings = { ...base, backend: { default: 'keystore' }, output: { folder: '/Users/demo/out', postfix: '_signed' } };
    const { mock, user } = renderApp({ settings, localBkuAvailable: false, pickFilesResult: ['/Users/demo/Vertrag.pdf'] });
    await user.click(await screen.findByRole('button', { name: 'PDF auswählen' }));
    await screen.findByRole('application', { name: 'Signaturblock' });
    await user.click(screen.getByRole('switch', { name: 'Unsichtbar signieren' }));
    await user.click(screen.getByRole('button', { name: /^Signieren/ }));
    const dialog = await screen.findByRole('dialog', { name: 'Signieren mit Keystore' });
    const question = await within(dialog).findByRole('radiogroup', { name: 'Passwort merken?' });
    await user.click(within(question).getByRole('radio', { name: 'Nicht speichern' }));
    await user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'falsch{Enter}');
    expect(await within(dialog).findByText(/Keystore-Passwort nicht akzeptiert/)).toBeInTheDocument();
    // erneute Abfrage: nicht mehr gefragt, die Wahl ist gespeichert
    expect(within(dialog).queryByRole('radiogroup', { name: 'Passwort merken?' })).toBeNull();
    expect(mock.dispatched.filter((e) => e.type === 'submit_secret')).toEqual([{ type: 'submit_secret', request: expect.any(Number), secret: 'falsch', password_storage: 'none' }]);
    await user.type(within(dialog).getByLabelText('Keystore-Passwort'), 'richtig{Enter}');
    expect(await screen.findByRole('heading', { level: 1, name: 'Signiert' }, { timeout: 3000 })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Einstellungen' }));
    const settingsDialog = await screen.findByRole('dialog', { name: 'Einstellungen' });
    await user.click(within(within(settingsDialog).getByRole('navigation', { name: 'Bereiche' })).getByRole('button', { name: 'Signiermittel' }));
    expect(within(settingsDialog).getByRole('radio', { name: 'Nicht speichern' })).toBeChecked();
    expect((await mock.view()).settings?.settings.keystore.storage_chosen).toBe(true);
  });
});
