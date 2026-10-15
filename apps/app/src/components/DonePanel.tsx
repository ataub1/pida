/**
 * Abgeschlossene Dokumente dieser Sitzung (`ViewState::done`, F-88): signiert (mit Zieldatei),
 * abgebrochen oder fehlgeschlagen (mit übersetzter Meldung) – nur Anzeige.
 */
import { useApp } from '../app/context';
import { baseName } from '../core/defaults';
import type { DoneItemView, WorkflowError } from '../core/types';
import type { Translator } from '../i18n/catalog';
import { variantName } from '../core/types';
import { IconDocument } from './icons';

/** Meldungs-ID und Parameter eines Workflow-Fehlers (wie `WorkflowError::code`/`params`). */
export function workflowErrorText(t: Translator, error: WorkflowError): string {
  const name = variantName(error as Record<string, unknown>);
  switch (name) {
    case 'Rejected':
    case 'Backend':
    case 'Prepare': {
      const data = (error as Record<string, { code: string; detail?: string }>)[name];
      return t.code(data?.code ?? 'error-Unexpected', data?.detail ? { detail: data.detail } : {});
    }
    case 'Invariant': {
      const v = (error as { Invariant: { invariant: string; detail: string } }).Invariant;
      return t.code('ng-invariant-violated', { invariant: v.invariant, detail: v.detail });
    }
    case 'CmsTooLarge': {
      const v = (error as { CmsTooLarge: { needed: number; reserve: number } }).CmsTooLarge;
      return t.code('ng-cms-too-large', { needed: v.needed, reserve: v.reserve });
    }
    case 'Save': {
      const detail = (error as { Save: string }).Save;
      return t.code('output-save_failed', { arg1: '', arg2: detail, detail });
    }
    case 'Config':
      return t.code('error-Initialization', { detail: (error as { Config: string }).Config });
    default:
      return t.code('error-Unexpected', { detail: String((error as { Unexpected?: string }).Unexpected ?? '') });
  }
}

function resultText(t: Translator, item: DoneItemView): string {
  switch (item.result.kind) {
    case 'signed':
      return t.t('ng-ui-done-signed', { file: baseName(item.result.output.path) });
    case 'cancelled':
      return t.t('ng-ui-done-cancelled');
    case 'failed':
      return t.t('ng-ui-done-failed', { message: workflowErrorText(t, item.result.error) });
  }
}

/** Liste der abgeschlossenen Dokumente; leer → nichts. */
export function DonePanel() {
  const { t, view } = useApp();
  if (view.done.length === 0) return null;
  return (
    <aside className="queue done" aria-labelledby="done-title">
      <h2 id="done-title" className="queue-title">
        {t.t('ng-ui-done-title', { count: view.done.length })}
      </h2>
      <ol className="done-list">
        {view.done.map((item) => (
          <li key={item.id} className="done-item" data-result={item.result.kind}>
            <IconDocument size={18} />
            <span className="queue-name" title={item.input}>
              {item.name}
            </span>
            <span className="done-state hint">{resultText(t, item)}</span>
          </li>
        ))}
      </ol>
    </aside>
  );
}
