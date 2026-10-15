/**
 * Import-Assistent für PKCS#12 (Mobile, Plan §5.2 „Keystore über PKCS#12-Import in die App-Sandbox“):
 *
 * 1. Datei wählen – System-Dateiauswahl über das Plugin (`pickMobileFiles`, Dokument-Picker bzw. SAF);
 *    die Shell legt eine Kopie in der Sandbox ab.
 * 2. Passwort (optional, nur im Speicher der Oberfläche bis zum Senden) und „sicher speichern“.
 * 3. „Importieren“ → `import_keystore`; der Kern kopiert die Datei in den Keystore-Ordner der App, prüft
 *    das Passwort, setzt Einstellungen › Keystore und legt das Passwort auf Wunsch im sicheren Speicher
 *    ab (iOS Keychain, Android Keystore). Das Ergebnis kommt als `ViewState.keystore_import`.
 *
 * Fachzustand hält der Kern; hier liegen nur Eingaben (gewählte Datei, Passwort, Häkchen).
 */
import { useEffect, useState } from 'react';
import { Form, Input, Label, TextField } from 'react-aria-components';
import { useApp } from '../../app/context';
import { baseName } from '../../core/defaults';
import { CheckField } from '../../settings/fields';
import { AccessButton } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/Dialog';
import { IconKey } from '../../ui/Icons';

export function KeystoreImportDialog({ onClose }: { onClose: () => void }) {
  const { t, view, bridge, dispatch, toast } = useApp();
  const [file, setFile] = useState<string | null>(null);
  const [password, setPassword] = useState('');
  const [remember, setRemember] = useState(true);
  const [busy, setBusy] = useState(false);
  const result = view.keystore_import;

  // eine neue Antwort des Kerns beendet das Warten
  useEffect(() => {
    if (result) setBusy(false);
  }, [result]);

  const choose = async () => {
    try {
      const files = await bridge.pickMobileFiles({ kind: 'pkcs12', multiple: false });
      const first = files[0];
      if (first) {
        setFile(first);
        if (result) void dispatch({ type: 'dismiss_keystore_import' });
      }
    } catch (e) {
      const code = e instanceof Error ? e.message : 'ng-ui-dialog-failed';
      toast(t.code(code, {}, 'ng-ui-dialog-failed'), 'error');
    }
  };

  const start = async () => {
    if (!file || busy) return;
    setBusy(true);
    const ok = await dispatch({ type: 'import_keystore', path: file, password: password === '' ? null : password, remember });
    setPassword('');
    if (!ok) setBusy(false);
  };

  const close = () => {
    if (result) void dispatch({ type: 'dismiss_keystore_import' });
    onClose();
  };

  if (result?.ok) {
    return (
      <ConfirmDialog
        title={t.t('ng-ui-keystore-import-title')}
        onCancel={close}
        actions={<AccessButton msg="ng-ui-keystore-import-done" label={t.t('ng-ui-keystore-import-done')} variant="primary" onPress={close} autoFocus />}
      >
        <p role="status">{t.t('ng-keystore-imported', { name: result.file_name })}</p>
        <p className="hint">{t.t(result.password_stored ? 'ng-ui-keystore-import-stored' : 'ng-ui-keystore-import-not-stored')}</p>
      </ConfirmDialog>
    );
  }

  return (
    <ConfirmDialog
      title={t.t('ng-ui-keystore-import-title')}
      onCancel={close}
      actions={
        <>
          <AccessButton msg="common-Cancel" variant="quiet" onPress={close} />
          <AccessButton msg="ng-ui-keystore-import-start" variant="primary" isDisabled={!file || busy} onPress={() => void start()} />
        </>
      }
    >
      <p className="privacy-lead">
        <IconKey size={20} /> {t.t('ng-ui-keystore-import-lead')}
      </p>
      <div className="actions actions-start">
        <AccessButton msg="ng-ui-keystore-import-choose" onPress={() => void choose()} autoFocus />
      </div>
      {file ? <p data-testid="keystore-import-file">{t.t('ng-ui-keystore-import-file', { name: baseName(file) })}</p> : null}
      <Form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          void start();
        }}
      >
        <TextField className="field" type="password" value={password} onChange={setPassword} isDisabled={!file} autoComplete="off">
          <Label className="field-label">{t.t('ng-ui-keystore-import-password')}</Label>
          <Input className="input" />
        </TextField>
        <CheckField label={t.t('ng-ui-keystore-import-remember')} isSelected={remember} onChange={setRemember} isDisabled={!file || password === ''} />
      </Form>
      {result && !result.ok ? (
        <p className="message message-error" role="alert">
          {t.code(result.code, { name: result.file_name }, 'ng-keystore-import-failed')}
        </p>
      ) : null}
    </ConfirmDialog>
  );
}
