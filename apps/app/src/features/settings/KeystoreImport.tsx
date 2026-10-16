/**
 * Import-Assistent für PKCS#12 (Mobile, Plan §5.2 „Keystore über PKCS#12-Import in die App-Sandbox“), im Stil
 * der neuen Oberfläche (ADR 0004 E3): Dialog mit Kopf, Dateikarte, Passwortfeld und Schalter.
 *
 * 1. Datei wählen – System-Dateiauswahl über das Plugin (`pickMobileFiles`, Dokument-Picker bzw. SAF);
 *    die Shell legt eine Kopie in der Sandbox ab.
 * 2. Passwort (optional, nur im Speicher der Oberfläche bis zum Senden) und „sicher speichern“.
 * 3. „Importieren“ → `import_keystore`; der Kern kopiert die Datei in den Keystore-Ordner der App, prüft
 *    das Passwort, setzt Einstellungen › Keystore und legt das Passwort auf Wunsch im sicheren Speicher
 *    ab (iOS Keychain, Android Keystore). Das Ergebnis kommt als `ViewState.keystore_import`.
 *
 * Fachzustand hält der Kern; hier liegen nur Eingaben (gewählte Datei, Passwort, Schalter).
 */
import { useEffect, useState } from 'react';
import { Form } from 'react-aria-components';
import { useApp } from '../../app/context';
import { baseName } from '../../core/defaults';
import { AccessButton, SwitchField, TextInput } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconCheckCircle, IconFileKey, IconKey } from '../../ui/Icons';
import './keystore-import.css';

/** `onClose(imported)`: `imported` = der Kern hat den Keystore übernommen (Ergebnis „importiert“ wurde gezeigt). */
export function KeystoreImportDialog({ onClose }: { onClose: (imported: boolean) => void }) {
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
    onClose(result?.ok === true);
  };

  const title = t.t('ng-ui-keystore-import-title');

  if (result?.ok) {
    return (
      <ModalShell onClose={close} isDismissable={false} role="alertdialog" size="md" className="keystore-import">
        <DialogHeader title={title} icon={<IconCheckCircle size={20} />} onClose={close} />
        <div className="keystore-import-body">
          <p role="status" className="keystore-import-done">
            {t.t('ng-keystore-imported', { name: result.file_name })}
          </p>
          <p className="field-hint">{t.t(result.password_stored ? 'ng-ui-keystore-import-stored' : 'ng-ui-keystore-import-not-stored')}</p>
        </div>
        <div className="dialog-actions keystore-import-actions">
          <AccessButton msg="ng-ui-keystore-import-done" label={t.t('ng-ui-keystore-import-done')} variant="primary" onPress={close} autoFocus />
        </div>
      </ModalShell>
    );
  }

  return (
    <ModalShell onClose={close} isDismissable={false} role="alertdialog" size="md" className="keystore-import">
      <DialogHeader title={title} subtitle={t.t('ng-ui-keystore-import-lead')} icon={<IconKey size={20} />} onClose={close} />
      <Form
        className="keystore-import-body"
        onSubmit={(e) => {
          e.preventDefault();
          void start();
        }}
      >
        <div className={`keystore-import-file${file ? ' has-file' : ''}`}>
          <span className="keystore-import-icon" aria-hidden="true">
            <IconFileKey size={20} />
          </span>
          <span className="keystore-import-name">
            {file ? <span data-testid="keystore-import-file">{t.t('ng-ui-keystore-import-file', { name: baseName(file) })}</span> : <span className="muted">{t.t('ng-ui-set-keystore-none')}</span>}
          </span>
          <AccessButton msg="ng-ui-keystore-import-choose" size="sm" onPress={() => void choose()} autoFocus />
        </div>
        <TextInput label={t.t('ng-ui-keystore-import-password')} type="password" value={password} onChange={setPassword} isDisabled={!file} autoComplete="off" />
        <SwitchField label={t.t('ng-ui-keystore-import-remember')} isSelected={remember} onChange={setRemember} isDisabled={!file || password === ''} />
        {result && !result.ok ? (
          <p className="message message-error" role="alert">
            {t.code(result.code, { name: result.file_name }, 'ng-keystore-import-failed')}
          </p>
        ) : null}
      </Form>
      <div className="dialog-actions keystore-import-actions">
        <AccessButton msg="common-Cancel" variant="quiet" onPress={close} />
        <AccessButton msg="ng-ui-keystore-import-start" variant="primary" isDisabled={!file || busy} onPress={() => void start()} />
      </div>
    </ModalShell>
  );
}
