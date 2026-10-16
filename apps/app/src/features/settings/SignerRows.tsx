/**
 * Einstellungen › Signiermittel und › Speichern (ADR 0004 §3.11).
 *
 * - Standard-Signiermittel `backend.default` (`None` = „Jedes Mal fragen“); Keystore erst, wenn er eingerichtet
 *   ist; Chipkarte nur auf dem Desktop.
 * - Konten: ID Austria (Benutzername, „merken“; der Kern normalisiert), FIDO2 (nur mit Plattformunterstützung),
 *   Keystore (Desktop: Datei – das Format erkennt der Kern am Inhalt, keine Typauswahl –, Schlüssel als Auswahl
 *   aus den Einträgen der Datei, Speicherart; Mobile nur über den PKCS#12-Import-Assistenten), Chipkarte
 *   (Status aus `ViewState::signers`).
 * - Speichern: neben dem Original = `output.folder = "."` (relativ zum Quellordner, F-82), fester Ordner =
 *   absoluter Pfad, jedes Mal fragen = `None`; Mobile ohne Ordnerwahl (App-Dokumente). Namenszusatz mit dem
 *   Beispiel des Kerns (`SettingsView::examples["output.postfix"]`).
 */
import { useState, type FormEvent, type ReactNode } from 'react';
import { useApp } from '../../app/context';
import { baseName } from '../../core/defaults';
import type { BackendChoice, KeystoreEntryView, KeystoreFormatView, PasswordStorage, UiEvent } from '../../core/types';
import { Badge, Button, SwitchField, TextInput } from '../../ui/controls';
import { IconCard, IconFileKey, IconFolder, IconPhone } from '../../ui/Icons';
import { AccountCard, ChoiceCards, ChoiceSegments, PathRow, SettingSelect, SettingTextField } from './controls';
import { KeystoreImportDialog } from './KeystoreImport';
import { DEFAULT_STORAGE_CHOICE, PasswordStorageQuestion } from './StorageQuestion';
import { useSettings, useTextSetting } from './state';

type DefaultChoice = BackendChoice | 'ask';

export function DefaultSignerRow() {
  const { t } = useApp();
  const s = useSettings();
  const current = s.get<BackendChoice | null>('backend.default');
  // Keystore erst als Standard wählbar, wenn er eingerichtet ist (aktiviert, Datei vorhanden; Kern)
  const keystore = s.view.keystore_ready;
  const locked = s.locked('backend.default');
  const options: { value: DefaultChoice; label: string; description: string; icon?: ReactNode; isDisabled?: boolean }[] = [
    { value: 'id_austria', label: t.t('ng-ui-set-signer-ida'), description: t.t(s.mobile ? 'ng-ui-set-signer-ida-desc-mobile' : 'ng-ui-set-signer-ida-desc'), icon: <IconPhone size={18} /> },
    {
      value: 'keystore',
      label: t.t('ng-ui-set-signer-keystore'),
      description: t.t(keystore ? 'ng-ui-set-signer-keystore-desc' : 'ng-ui-set-signer-keystore-off'),
      icon: <IconFileKey size={18} />,
      isDisabled: !keystore && current !== 'keystore',
    },
  ];
  // Mobile: keine lokale Chipkarten-Software (Plan §5.2)
  if (!s.mobile) options.push({ value: 'local_bku', label: t.t('ng-ui-set-signer-card'), description: t.t('ng-ui-set-signer-card-desc'), icon: <IconCard size={18} /> });
  options.push({ value: 'ask', label: t.t('ng-ui-set-signer-ask'), description: t.t('ng-ui-set-signer-ask-desc') });
  return (
    <ChoiceCards<DefaultChoice>
      label={t.t('ng-ui-set-default')}
      value={current ?? 'ask'}
      columns={2}
      isDisabled={!!locked}
      description={locked}
      onChange={(v) => s.apply({ 'backend.default': v === 'ask' ? null : v })}
      options={options}
    />
  );
}

export function IdAustriaRow() {
  const { t } = useApp();
  const s = useSettings();
  const user = useTextSetting(
    'id_austria.username',
    (v) => (typeof v === 'string' ? v : ''),
    (text) => ({ value: text.trim() === '' ? null : text }),
  );
  return (
    <AccountCard icon={<IconPhone size={18} />} title={t.t('ng-ui-set-signer-ida')}>
      <SettingTextField label={t.t('ng-ui-set-ida-user')} inputMode="tel" autoComplete="username" {...user} />
      <SwitchField
        label={t.t('ng-ui-set-ida-remember')}
        hint={t.t('ng-ui-set-ida-remember-hint')}
        isSelected={s.get<boolean>('id_austria.remember')}
        onChange={(v) => s.apply({ 'id_austria.remember': v })}
      />
    </AccountCard>
  );
}

export function Fido2Row() {
  const { t } = useApp();
  const s = useSettings();
  if (!s.view.fido2_available) {
    return (
      <div className="stack-2">
        <span className="field-label">{t.t('ng-ui-set-fido2')}</span>
        <p className="message message-warning">{t.t(s.mobile ? 'ng-fido2-unavailable-mobile' : 'config-fido2-UnsupportedPlatform')}</p>
      </div>
    );
  }
  return (
    <SwitchField
      label={t.t('ng-ui-set-fido2')}
      hint={t.t('ng-ui-set-fido2-hint')}
      isSelected={s.get<boolean>('id_austria.fido2_default')}
      onChange={(v) => s.apply({ 'id_austria.fido2_default': v })}
    />
  );
}

/** Gültigkeitsende als Datum der Oberflächensprache. */
function validUntil(iso: string | null, lang: string): string | null {
  if (!iso) return null;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return null;
  return new Intl.DateTimeFormat(lang === 'de' ? 'de-AT' : 'en-GB', { dateStyle: 'medium' }).format(d);
}

/** „Name · gültig bis …“ eines Eintrags (ohne Namen im Zertifikat: der Alias). */
function entryLabel(t: ReturnType<typeof useApp>['t'], e: KeystoreEntryView): string {
  const name = e.subject_cn ?? e.alias;
  const date = validUntil(e.not_after, t.lang);
  return date ? t.t('ng-ui-set-keystore-key-entry', { name, date }) : name;
}

const sameAlias = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Format der gewählten Datei (am Inhalt erkannt, `SettingsView::keystore_format`). */
function KeystoreFormatNote({ format }: { format: KeystoreFormatView | null }) {
  const { t } = useApp();
  switch (format) {
    case 'pkcs12':
      return <p className="field-hint">{t.t('ng-ui-set-keystore-format', { format: 'PKCS#12' })}</p>;
    case 'jks':
    case 'jceks':
      return <p className="message message-warning">{t.t('ng-ui-notice-keystore-conversion')}</p>;
    case 'unknown':
      return <p className="message message-warning">{t.t('ng-ui-set-keystore-unsupported')}</p>;
    default:
      return null;
  }
}

/** Passwortfeld „Schlüssel anzeigen“ (inline; bei der ersten Verwendung mit „Passwort merken?“). */
function KeystorePasswordForm({ onClose }: { onClose: () => void }) {
  const { t, dispatch } = useApp();
  const s = useSettings();
  const ask = !s.view.settings.keystore.storage_chosen;
  const [password, setPassword] = useState('');
  const [storage, setStorage] = useState<PasswordStorage>(DEFAULT_STORAGE_CHOICE);
  const [empty, setEmpty] = useState(false);
  // Problem des Kerns erst nach dem Absenden zeigen (nicht ein älteres beim Öffnen bzw. beim Tippen)
  const [submitted, setSubmitted] = useState(false);
  const [sending, setSending] = useState(false);
  const problem = s.view.keystore_inspect_problem;
  const error = empty
    ? t.t('ng-ui-set-keystore-password-empty')
    : submitted && problem
      ? problem === 'keystore-wrong-password'
        ? t.t('ng-ui-set-keystore-wrong-password')
        : t.code(problem, {}, 'ng-ui-problem-invalid')
      : null;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (sending) return;
    if (!password) return setEmpty(true);
    setSending(true);
    setSubmitted(true);
    const event: UiEvent = ask ? { type: 'inspect_keystore', password, password_storage: storage } : { type: 'inspect_keystore', password };
    void dispatch(event).finally(() => setSending(false));
    setPassword('');
  };
  return (
    <form className="stack-2 keystore-password" onSubmit={submit} noValidate>
      <p className="field-hint">{t.t('ng-ui-set-keystore-key-lead')}</p>
      <TextInput
        label={t.t('ng-ui-set-keystore-password')}
        type="password"
        value={password}
        onChange={(v) => {
          setPassword(v);
          setEmpty(false);
          setSubmitted(false);
        }}
        error={error}
        autoComplete="off"
        autoFocus
      />
      {ask ? <PasswordStorageQuestion value={storage} onChange={setStorage} keychainAvailable={s.view.keychain_available} /> : null}
      <div className="button-row">
        <Button variant="primary" size="sm" type="submit" isDisabled={sending}>
          {t.t('ng-ui-set-keystore-key-submit')}
        </Button>
        <Button variant="ghost" size="sm" onPress={onClose}>
          {t.t('ng-ui-set-keystore-key-cancel')}
        </Button>
      </div>
    </form>
  );
}

/**
 * Schlüssel (Alias) als Auswahl statt Textfeld: Einträge aus `SettingsView::keystore_entries` (der Kern liest
 * sie mit einem gemerkten Passwort selbst, sonst nach „Schlüssel anzeigen …“). Nur Schlüsseleinträge sind
 * wählbar; genau ein Schlüssel → als Text, Alias bleibt leer (der Kern nimmt den einzigen Schlüssel); mehrere →
 * Auswahl setzt `keystore.alias`. Ein gesetzter Alias, der fehlt, erscheint als Warnung.
 */
function KeystoreKeyRow() {
  const { t } = useApp();
  const s = useSettings();
  const [asking, setAsking] = useState(false);
  const entries = s.view.keystore_entries;
  const alias = s.get<string | null>('keystore.alias');
  const label = t.t('ng-ui-set-keystore-key');

  if (!entries) {
    return (
      <div className="stack-2">
        <span className="field-label">{label}</span>
        {alias ? <p className="muted">{t.t('ng-ui-set-keystore-key-alias', { alias })}</p> : null}
        {asking ? (
          <KeystorePasswordForm onClose={() => setAsking(false)} />
        ) : (
          <div className="button-row">
            <Button variant="secondary" size="sm" onPress={() => setAsking(true)}>
              {t.t('ng-ui-set-keystore-key-show')}
            </Button>
          </div>
        )}
      </div>
    );
  }

  const keys = entries.filter((e) => e.is_key_entry);
  const chosen = alias ? (keys.find((k) => sameAlias(k.alias, alias)) ?? null) : null;
  const missing = alias && !chosen ? <p className="message message-warning">{t.t('ng-ui-set-keystore-alias-missing', { alias })}</p> : null;
  const only = keys.length === 1 ? keys[0] : undefined;
  if (keys.length === 0) {
    return (
      <div className="stack-2">
        <span className="field-label">{label}</span>
        <p className="message message-warning">{t.t('ng-ui-set-keystore-key-none')}</p>
      </div>
    );
  }
  if (only) {
    return (
      <div className="stack-2">
        <span className="field-label">{label}</span>
        <p className="keystore-key" data-testid="keystore-key">
          <span>{entryLabel(t, only)}</span>
          <span className="mono-small muted">{t.t('ng-ui-set-keystore-key-alias', { alias: only.alias })}</span>
        </p>
        {missing}
        {missing ? (
          <div className="button-row">
            <Button variant="secondary" size="sm" onPress={() => s.apply({ 'keystore.alias': null })}>
              {t.t('ng-ui-set-keystore-key-use')}
            </Button>
          </div>
        ) : null}
      </div>
    );
  }
  return (
    <div className="stack-2">
      <SettingSelect
        label={label}
        value={chosen?.alias ?? null}
        placeholder={t.t('ng-ui-set-keystore-key-choose')}
        description={chosen ? null : t.t('ng-ui-set-keystore-key-several')}
        onChange={(id) => s.apply({ 'keystore.alias': id })}
        options={keys.map((k) => ({ id: k.alias, label: entryLabel(t, k), detail: t.t('ng-ui-set-keystore-key-alias', { alias: k.alias }) }))}
      />
      {missing}
    </div>
  );
}

function KeystoreDetails() {
  const { t } = useApp();
  const s = useSettings();
  const storage = s.get<PasswordStorage>('keystore.password_storage');
  // die Schlüssel lassen sich nur aus einer PKCS#12-Datei lesen (Mobile: immer PKCS#12, per Import)
  const pkcs12 = s.view.keystore_format === 'pkcs12';
  return (
    <>
      {pkcs12 ? <KeystoreKeyRow /> : null}
      <ChoiceSegments<PasswordStorage>
        label={t.t('ng-ui-set-keystore-storage')}
        value={storage}
        onChange={(v) => s.apply({ 'keystore.password_storage': v })}
        description={s.view.keychain_available ? null : t.t('ng-secrets-not-persistent')}
        options={[
          { value: 'none', label: t.t('ng-ui-set-keystore-storage-none') },
          { value: 'memory', label: t.t('ng-ui-set-keystore-storage-memory') },
          // Mobile: Keychain bzw. Android Keystore – wie „Im sicheren Speicher“ bei „Passwort merken?“
          { value: 'keychain', label: t.t(s.mobile ? 'ng-ui-set-keystore-storage-keychain-mobile' : 'ng-ui-set-keystore-storage-keychain'), isDisabled: !s.view.keychain_available },
        ]}
      />
    </>
  );
}

export function KeystoreRow() {
  const { t, bridge, toast } = useApp();
  const s = useSettings();
  const [importing, setImporting] = useState(false);
  const enabled = s.get<boolean>('keystore.enabled');
  const file = s.get<string | null>('keystore.file');
  const fileProblem = s.problem('keystore.file');
  // Format am Inhalt (Kern); solange die neue Wahl noch nicht bestätigt ist, ohne Hinweis
  const format = s.isPending('keystore.file') ? null : s.view.keystore_format;

  if (s.mobile) {
    // Mobile: nur PKCS#12 und nur über den Import in die App-Sandbox (kein Dateipfad)
    return (
      <AccountCard icon={<IconFileKey size={18} />} title={t.t('ng-ui-set-keystore')}>
        <p>{file ? t.t('ng-ui-keystore-current', { name: baseName(file) }) : t.t('ng-ui-backend-keystore-import-hint')}</p>
        {fileProblem ? <p className="field-error">{fileProblem}</p> : null}
        <div className="button-row">
          <Button variant="secondary" size="sm" onPress={() => setImporting(true)}>
            {t.t('ng-ui-keystore-import')}
          </Button>
        </div>
        {file ? <KeystoreDetails /> : null}
        {importing ? <KeystoreImportDialog onClose={() => setImporting(false)} /> : null}
      </AccountCard>
    );
  }

  const pick = async () => {
    try {
      const files = await bridge.pickFiles({
        title: t.t('keystore_config-KeystoreFile-Dialog_Title'),
        multiple: false,
        filters: [
          { name: t.t('common-PKCS12Extension_Description'), extensions: ['p12', 'pkcs12', 'pfx'] },
          { name: t.t('common-KeystoreExtension_Description'), extensions: ['p12', 'pkcs12', 'pfx', 'ks', 'jks'] },
          { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
        ],
        defaultPath: file,
      });
      const chosen = files?.[0];
      // das Format erkennt der Kern am Inhalt; `keystore.declared_type` bleibt unberührt (PDF-Over)
      if (chosen) s.apply({ 'keystore.file': chosen });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };
  return (
    <AccountCard icon={<IconFileKey size={18} />} title={t.t('ng-ui-set-keystore')}>
      <SwitchField
        label={t.t('ng-ui-set-keystore-enabled')}
        isSelected={enabled}
        onChange={(v) => {
          // Keystore aus: eine Keystore-Voreinstellung fällt auf „Jedes Mal fragen“ zurück
          const changes: Record<string, unknown> = { 'keystore.enabled': v };
          if (!v && s.get<BackendChoice | null>('backend.default') === 'keystore') changes['backend.default'] = null;
          s.apply(changes);
        }}
      />
      {enabled ? (
        <>
          <div className="stack-2">
            <PathRow
              label={t.t('ng-ui-set-keystore-file')}
              value={file}
              empty={t.t('ng-ui-set-keystore-none')}
              action={
                <Button variant="secondary" size="sm" onPress={() => void pick()}>
                  {t.t('ng-ui-set-keystore-choose')}
                </Button>
              }
            />
            {fileProblem ? (
              <p className="field-error" role="alert">
                {fileProblem}
              </p>
            ) : file ? (
              <KeystoreFormatNote format={format} />
            ) : null}
          </div>
          <KeystoreDetails />
        </>
      ) : null}
    </AccountCard>
  );
}

/** Chipkarte (Desktop): Status der Chipkarten-Software aus `ViewState::signers`. */
export function CardRow() {
  const { t, view } = useApp();
  const local = view.signers.options.find((o) => o.backend === 'local_bku');
  const badge = local ? <Badge tone={local.available ? 'success' : 'neutral'}>{t.t(local.available ? 'ng-ui-set-card-found' : 'ng-ui-set-card-missing')}</Badge> : null;
  return (
    <AccountCard icon={<IconCard size={18} />} title={t.t('ng-ui-set-card')} badge={badge}>
      <p className="muted">{t.t('ng-ui-set-signer-card-desc')}</p>
      {local && !local.available && local.unavailable_reason ? <p className="field-hint">{t.code(local.unavailable_reason, {}, 'ng-signer-unavailable')}</p> : null}
    </AccountCard>
  );
}

/* ------------------------------------------------------------------ Speichern */

type Target = 'beside' | 'folder' | 'ask';

function targetOf(folder: string | null): Target {
  if (folder === null) return 'ask';
  return folder === '.' || folder === './' || folder === '.\\' ? 'beside' : 'folder';
}

export function SaveTargetRow() {
  const { t, bridge, toast } = useApp();
  const s = useSettings();
  const folder = s.get<string | null>('output.folder');
  const target = targetOf(folder);
  const problem = s.problem('output.folder');
  if (s.mobile) {
    return (
      <div className="stack-2">
        <span className="field-label">{t.t('ng-ui-set-target')}</span>
        <p className="muted">{t.t('ng-ui-output-mobile-hint')}</p>
      </div>
    );
  }
  const choose = async () => {
    try {
      const dir = await bridge.pickDirectory({ title: t.t('advanced_config-OutputFolder-Dialog_Title'), defaultPath: target === 'folder' ? folder : null });
      if (dir) s.apply({ 'output.folder': dir });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };
  return (
    <div className="stack-3">
      <ChoiceCards<Target>
        label={t.t('ng-ui-set-target')}
        value={target}
        onChange={(v) => {
          if (v === 'beside') s.apply({ 'output.folder': '.' });
          else if (v === 'ask') s.apply({ 'output.folder': null });
          else if (target !== 'folder') void choose();
        }}
        options={[
          { value: 'beside', label: t.t('ng-ui-set-target-beside'), description: t.t('ng-ui-set-target-beside-desc') },
          { value: 'folder', label: t.t('ng-ui-set-target-folder'), description: target === 'folder' && folder ? folder : t.t('ng-ui-set-folder-none') },
          { value: 'ask', label: t.t('ng-ui-set-target-ask'), description: t.t('ng-ui-set-target-ask-desc') },
        ]}
      />
      {target === 'folder' ? (
        <PathRow
          label={t.t('ng-ui-set-folder')}
          value={folder}
          empty={t.t('ng-ui-set-folder-none')}
          action={
            <Button variant="ghost" size="sm" onPress={() => void choose()}>
              <IconFolder size={15} />
              {t.t('ng-ui-set-folder-choose')}
            </Button>
          }
        />
      ) : null}
      {problem ? (
        <p className="field-error" role="alert">
          {problem}
        </p>
      ) : null}
    </div>
  );
}

export function PostfixRow() {
  const { t } = useApp();
  const s = useSettings();
  const field = useTextSetting(
    'output.postfix',
    (v) => (typeof v === 'string' ? v : ''),
    // leer bedeutet „_signed“ – gesendet erst beim Verlassen, damit das Feld beim Tippen leer bleiben darf
    (text, final) => (text.trim() === '' ? (final ? { value: '_signed' } : null) : { value: text }),
  );
  const example = s.view.examples['output.postfix'];
  return (
    <div className="stack-2">
      <SettingTextField label={t.t('ng-ui-set-postfix')} description={t.t('ng-ui-set-postfix-hint')} mono placeholder="_signed" {...field} />
      {example ? (
        <p className="pattern-preview" aria-live="polite">
          <span className="pattern-label">{t.t('ng-ui-set-example')}:</span> <span className="mono">Vertrag.pdf</span> → <strong className="mono">{example}</strong>
        </p>
      ) : null}
    </div>
  );
}
