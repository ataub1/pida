/**
 * Einstellungen › Signiermittel und › Speichern (ADR 0004 §3.11).
 *
 * - Standard-Signiermittel `backend.default` (`None` = „Jedes Mal fragen“); Keystore erst, wenn er eingerichtet
 *   ist; Chipkarte nur auf dem Desktop.
 * - Konten: ID Austria (Benutzername, „merken“; der Kern normalisiert), FIDO2 (nur mit Plattformunterstützung),
 *   Keystore (Desktop: Datei, Typ aus der Endung wie PDF-Over, Alias, Speicherart; Mobile nur über den
 *   PKCS#12-Import-Assistenten), Chipkarte (Status aus `ViewState::signers`).
 * - Speichern: neben dem Original = `output.folder = "."` (relativ zum Quellordner, F-82), fester Ordner =
 *   absoluter Pfad, jedes Mal fragen = `None`; Mobile ohne Ordnerwahl (App-Dokumente). Namenszusatz mit dem
 *   Beispiel des Kerns (`SettingsView::examples["output.postfix"]`).
 */
import { useState, type ReactNode } from 'react';
import { useApp } from '../../app/context';
import { baseName } from '../../core/defaults';
import type { BackendChoice, PasswordStorage } from '../../core/types';
import { Badge, Button, SwitchField } from '../../ui/controls';
import { IconCard, IconFileKey, IconFolder, IconPhone } from '../../ui/Icons';
import { AccountCard, ChoiceCards, ChoiceSegments, PathRow, SettingTextField } from './controls';
import { KeystoreImportDialog } from './KeystoreImport';
import { useSettings, useTextSetting } from './state';

/** Keystore-Typ aus der Dateiendung wie PDF-Over (`KeystoreConfigurationComposite`). */
export function keystoreTypeFromFile(file: string): 'PKCS12' | 'JCEKS' | null {
  const m = /\.([^./\\]+)$/.exec(file.trim());
  const ext = m?.[1]?.toLowerCase();
  if (ext === 'p12' || ext === 'pkcs12' || ext === 'pfx') return 'PKCS12';
  if (ext === 'ks' || ext === 'jks') return 'JCEKS';
  return null;
}

type DefaultChoice = BackendChoice | 'ask';

export function DefaultSignerRow() {
  const { t } = useApp();
  const s = useSettings();
  const current = s.get<BackendChoice | null>('backend.default');
  const keystore = s.get<boolean>('keystore.enabled');
  const locked = s.locked('backend.default');
  const options: { value: DefaultChoice; label: string; description: string; icon?: ReactNode; isDisabled?: boolean }[] = [
    { value: 'id_austria', label: t.t('ng-ui-set-signer-ida'), description: t.t(s.mobile ? 'ng-ui-set-signer-ida-desc-mobile' : 'ng-ui-set-signer-ida-desc'), icon: <IconPhone size={18} /> },
    {
      value: 'keystore',
      label: t.t('ng-ui-set-signer-keystore'),
      description: t.t(keystore || current === 'keystore' ? 'ng-ui-set-signer-keystore-desc' : 'ng-ui-set-signer-keystore-off'),
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

type KeystoreType = 'PKCS12' | 'JKS' | 'JCEKS';

function KeystoreDetails() {
  const { t } = useApp();
  const s = useSettings();
  const alias = useTextSetting(
    'keystore.alias',
    (v) => (typeof v === 'string' ? v : ''),
    (text) => ({ value: text.trim() === '' ? null : text }),
  );
  const storage = s.get<PasswordStorage>('keystore.password_storage');
  return (
    <>
      <SettingTextField label={t.t('ng-ui-set-keystore-alias')} description={t.t('ng-ui-keystore-alias-hint')} {...alias} />
      <ChoiceSegments<PasswordStorage>
        label={t.t('ng-ui-set-keystore-storage')}
        value={storage}
        onChange={(v) => s.apply({ 'keystore.password_storage': v })}
        description={s.view.keychain_available ? null : t.t('ng-secrets-not-persistent')}
        options={[
          { value: 'none', label: t.t('ng-ui-set-keystore-storage-none') },
          { value: 'memory', label: t.t('ng-ui-set-keystore-storage-memory') },
          { value: 'keychain', label: t.t('ng-ui-set-keystore-storage-keychain'), isDisabled: !s.view.keychain_available },
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
  const declared = (s.get<string | null>('keystore.declared_type') ?? 'PKCS12').toUpperCase();
  const type: KeystoreType = declared === 'JKS' || declared === 'JCEKS' ? declared : 'PKCS12';

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
          { name: t.t('common-KeystoreExtension_Description'), extensions: ['p12', 'pkcs12', 'pfx', 'ks', 'jks'] },
          { name: t.t('common-PKCS12Extension_Description'), extensions: ['p12', 'pkcs12', 'pfx'] },
          { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
        ],
        defaultPath: file,
      });
      const chosen = files?.[0];
      if (!chosen) return;
      // wie PDF-Over: der Typ folgt der Dateiendung (.p12/.pfx → PKCS12, .jks/.ks → Java-Keystore)
      const derived = keystoreTypeFromFile(chosen);
      s.apply(derived ? { 'keystore.file': chosen, 'keystore.declared_type': derived } : { 'keystore.file': chosen });
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
            ) : null}
          </div>
          {file ? (
            <ChoiceSegments<KeystoreType>
              label={t.t('ng-ui-set-keystore-type')}
              value={type}
              onChange={(v) => s.apply({ 'keystore.declared_type': v })}
              description={type === 'PKCS12' ? null : t.t('ng-ui-notice-keystore-conversion')}
              options={[
                { value: 'PKCS12', label: 'PKCS#12' },
                { value: 'JKS', label: 'JKS' },
                { value: 'JCEKS', label: 'JCEKS' },
              ]}
            />
          ) : null}
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
