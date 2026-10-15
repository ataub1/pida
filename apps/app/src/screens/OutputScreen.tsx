/**
 * Ergebnis (docs/analyse/01 §4.8, F-80…F-91): Erfolgsmeldung, Öffnen (F-87), im Ordner anzeigen,
 * Speichern bei Fehlschlag/Abbruch (F-86), Speichern-Dialog (F-83, `OutputView::save_dialog` – der
 * Dialog öffnet sich automatisch, die Antwort geht als `save{path}` bzw. `save{null}` zurück) und
 * Überschreiben-Rückfrage (F-84, `OutputView::ask_overwrite` → `confirm_overwrite`), nächstes Dokument
 * der Warteschlange; Dateien lassen sich weiter ablegen (F-89).
 *
 * Mobile (Plan §5.1 „Ausgabe“): Die Datei liegt in den App-Dokumenten; statt „Im Ordner anzeigen“ gibt es
 * „Teilen“ (Share-Sheet bzw. `ACTION_SEND`) und „Sichern unter …“ (Dokument-Picker bzw. SAF) – beides
 * führt der Kern über die Shell aus (`share_result`/`export_result`).
 */
import { useEffect, useRef } from 'react';
import { useApp } from '../app/context';
import { DonePanel } from '../components/DonePanel';
import { IconCheck, IconFolder, IconUpload } from '../components/icons';
import { AccessButton, ConfirmDialog, Markup, ScreenHeading } from '../components/ui';
import { baseName, dirName } from '../core/defaults';
import { isMobile, type DocInfo, type OutputView } from '../core/types';

function joinPath(dir: string, name: string): string {
  if (!dir) return name;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

export function OutputScreen({ result, next }: { result: OutputView; next: DocInfo | null }) {
  const { t, dispatch, bridge, toast, dragActive, view } = useApp();
  const mobile = isMobile(view);
  const filters = [
    { name: t.t('common-PDFExtension_Description'), extensions: ['pdf'] },
    { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
  ];

  const askSave = async (defaultPath: string | null) => {
    try {
      const path = await bridge.pickSavePath({ title: t.t('common-Save'), filters, defaultPath });
      void dispatch({ type: 'save', path });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
      // Dialog nicht verfügbar: wie abgebrochen (Knopf „Speichern“ bleibt, F-86)
      void dispatch({ type: 'save', path: null });
    }
  };

  // Speichern-Dialog automatisch, wenn der Kern ihn anfordert (einmal je Rückfrage). Der Vorschlag
  // (Startordner + Name) gilt auch für den Knopf „Speichern“ nach einem abgebrochenen Dialog (F-86).
  const asked = useRef<string | null>(null);
  const lastSuggestion = useRef<string | null>(null);
  const saveTarget = result.save_dialog ? joinPath(result.save_dialog.directory, result.save_dialog.file_name) : null;
  const askSaveRef = useRef(askSave);
  askSaveRef.current = askSave;
  useEffect(() => {
    if (saveTarget === null) {
      asked.current = null;
      return;
    }
    lastSuggestion.current = saveTarget;
    if (asked.current === saveTarget) return;
    asked.current = saveTarget;
    void askSaveRef.current(saveTarget);
  }, [saveTarget]);

  const reveal = () => bridge.revealOutput().catch(() => toast(t.t('ng-ui-open-failed'), 'error'));

  return (
    <div className="screen screen-output" data-drop-target={dragActive || undefined}>
      {result.path ? (
        <>
          <ScreenHeading>{t.t('output-success_message')}</ScreenHeading>
          <div className="card success-card">
            <span className="success-icon" aria-hidden="true">
              <IconCheck size={36} />
            </span>
            <p>
              <Markup text={t.t('output-link_open_message')} targets={[{ onPress: () => dispatch({ type: 'open_result' }) }]} />
            </p>
            <p className="path" title={result.path}>
              <strong>{baseName(result.path)}</strong>
              {/* Mobile: der Sandbox-Pfad sagt der Nutzerin nichts („liegt in den Dokumenten dieser App“) */}
              {mobile ? null : <span className="hint block">{dirName(result.path)}</span>}
            </p>
            {mobile ? (
              <>
                <p className="hint">{t.t('ng-ui-output-mobile-hint')}</p>
                <div className="actions actions-start">
                  <AccessButton msg="common-open" variant="primary" onPress={() => dispatch({ type: 'open_result' })} />
                  <AccessButton msg="ng-ui-output-share" icon={<IconUpload size={18} />} onPress={() => dispatch({ type: 'share_result' })} />
                  <AccessButton msg="ng-ui-output-export" icon={<IconFolder size={18} />} onPress={() => dispatch({ type: 'export_result' })} />
                </div>
              </>
            ) : (
              <div className="actions actions-start">
                <AccessButton msg="common-open" variant="primary" onPress={() => dispatch({ type: 'open_result' })} />
                <AccessButton msg="ng-ui-output-reveal" icon={<IconFolder size={18} />} onPress={reveal} />
              </div>
            )}
          </div>
        </>
      ) : (
        <>
          <ScreenHeading>{t.t('output-success_message')}</ScreenHeading>
          <div className="card">
            {result.save_pending ? <p>{t.t('output-link_save_message')}</p> : <p>{t.t('ng-ui-output-saving')}</p>}
            {result.save_pending ? (
              <div className="actions actions-start">
                <AccessButton msg="common-Save" variant="primary" onPress={() => void askSave(lastSuggestion.current)} />
              </div>
            ) : null}
          </div>
        </>
      )}

      <div className="card next-card">
        {next ? (
          <>
            <p>{t.t('ng-ui-output-next', { name: next.name })}</p>
            <AccessButton msg="ng-ui-output-next-button" variant="primary" onPress={() => dispatch({ type: 'navigate', step: 'Open' })} />
          </>
        ) : (
          <>
            {mobile ? null : <p className="hint">{t.t('ng-ui-output-drop-hint')}</p>}
            <AccessButton msg="ng-ui-output-more" onPress={() => dispatch({ type: 'navigate', step: 'Open' })} />
          </>
        )}
      </div>

      <DonePanel />

      {result.ask_overwrite ? (
        <ConfirmDialog
          title={t.t('common-warning')}
          tone="warning"
          onCancel={() => dispatch({ type: 'confirm_overwrite', overwrite: false })}
          actions={
            <>
              <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'confirm_overwrite', overwrite: false })} autoFocus />
              <AccessButton msg="common-Ok" variant="danger" onPress={() => dispatch({ type: 'confirm_overwrite', overwrite: true })} />
            </>
          }
        >
          {/* Mobile: nur der Dateiname – der Sandbox-Pfad ist unlesbar und läuft aus dem Dialog */}
          <p>{t.t('output-file_ask_overwrite', { arg1: mobile ? baseName(result.ask_overwrite) : result.ask_overwrite })}</p>
        </ConfirmDialog>
      ) : null}
    </div>
  );
}
