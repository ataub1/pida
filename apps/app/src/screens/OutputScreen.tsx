/**
 * Ergebnis (docs/analyse/01 §4.8, F-80…F-91): Erfolgsmeldung, Öffnen (F-87), im Ordner anzeigen,
 * Speichern bei Fehlschlag/Abbruch (F-86), Speichern-Dialog (F-83) und Überschreiben-Rückfrage (F-84)
 * – sofern der Kern die Rückfrage meldet (vorgeschlagene Erweiterung `OutputView::prompt`) –,
 * nächstes Dokument der Warteschlange; Dateien lassen sich weiter ablegen (F-89).
 */
import { useEffect, useRef } from 'react';
import { useApp } from '../app/context';
import { IconCheck, IconFolder } from '../components/icons';
import { AccessButton, ConfirmDialog, Markup, ScreenHeading } from '../components/ui';
import { baseName, dirName } from '../core/defaults';
import { outputPrompt } from '../core/extensions';
import type { DocInfo, OutputView, StepView } from '../core/types';

function joinPath(dir: string, name: string): string {
  if (!dir) return name;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

export function OutputScreen({ step, result, next }: { step: StepView; result: OutputView; next: DocInfo | null }) {
  const { t, dispatch, bridge, toast, dragActive } = useApp();
  const prompt = outputPrompt(step);
  const filters = [
    { name: t.t('common-PDFExtension_Description'), extensions: ['pdf'] },
    { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
  ];

  const askSave = async (defaultPath: string | null) => {
    try {
      const path = await bridge.pickSavePath({ title: t.t('common-Save'), filters, defaultPath });
      dispatch({ type: 'save', path });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };

  // Speichern-Dialog automatisch, wenn der Kern ihn anfordert (einmal je Rückfrage).
  const asked = useRef<string | null>(null);
  const saveTarget = prompt?.kind === 'save_path' ? joinPath(prompt.directory, prompt.file_name) : null;
  const askSaveRef = useRef(askSave);
  askSaveRef.current = askSave;
  useEffect(() => {
    if (saveTarget === null) {
      asked.current = null;
      return;
    }
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
              <span className="hint block">{dirName(result.path)}</span>
            </p>
            <div className="actions actions-start">
              <AccessButton msg="common-open" variant="primary" onPress={() => dispatch({ type: 'open_result' })} />
              <AccessButton msg="ng-ui-output-reveal" icon={<IconFolder size={18} />} onPress={reveal} />
            </div>
          </div>
        </>
      ) : (
        <>
          <ScreenHeading>{t.t('output-success_message')}</ScreenHeading>
          <div className="card">
            {result.save_pending ? <p>{t.t('output-link_save_message')}</p> : <p>{t.t('ng-ui-output-saving')}</p>}
            {result.save_pending ? (
              <div className="actions actions-start">
                <AccessButton msg="common-Save" variant="primary" onPress={() => void askSave(null)} />
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
            <p className="hint">{t.t('ng-ui-output-drop-hint')}</p>
            <AccessButton msg="ng-ui-output-more" onPress={() => dispatch({ type: 'navigate', step: 'Open' })} />
          </>
        )}
      </div>

      {prompt?.kind === 'overwrite' ? (
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
          <p>{t.t('output-file_ask_overwrite', { arg1: prompt.path })}</p>
        </ConfirmDialog>
      ) : null}
    </div>
  );
}
