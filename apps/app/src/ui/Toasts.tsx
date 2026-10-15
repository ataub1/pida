/**
 * Benachrichtigungen unten rechts in einer dauerhaften Live-Region (aus dem Prototyp, ADR 0004 §4.2).
 * Quellen: die Hinweise des Kerns (`ViewState::notices`, Schließen = `dismiss_notice`) und eigene
 * Hinweise der Oberfläche. Erfolg/Info verschwinden nach 6 s (pausieren bei Fokus/Hover), Fehler sind
 * `alert` und bleiben stehen (WCAG 2.2.1). Jeder Hinweis hat seine eigene Frist.
 */
import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { useApp, type Toast } from '../app/context';
import type { NoticeView } from '../core/types';
import { IconAlertCircle, IconCheckCircle, IconClose, IconInfo } from './Icons';

/** Anzeigedauer eines Info-/Erfolgshinweises (Fehler bleiben bis zum Schließen). */
export const TOAST_MS = 6000;

/** Fehlerhinweis des Kerns (bleibt stehen)? */
export function isErrorNotice(code: string): boolean {
  return /^(error|exception)[-.]/.test(code) || code.endsWith('-failed');
}

/** Erfolgsmeldungen des Kerns (grünes Symbol). */
const SUCCESS_CODES = new Set(['ng-batch-item-signed', 'ng-batch-done', 'ng-export-done', 'ng-keystore-imported', 'ng-firstrun-done', 'ng-migrate-done']);

function ToastCard({ kind, title, text, onDismiss }: { kind: Toast['kind']; title: string; text?: string; onDismiss: () => void }) {
  const { t } = useApp();
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    if (kind === 'error' || paused) return undefined;
    const timer = setTimeout(onDismiss, TOAST_MS);
    return () => clearTimeout(timer);
  }, [kind, paused, onDismiss]);
  const Icon = kind === 'success' ? IconCheckCircle : kind === 'error' ? IconAlertCircle : IconInfo;
  return (
    <div
      className={`toast toast-${kind}`}
      role={kind === 'error' ? 'alert' : undefined}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <Icon size={20} className="toast-icon" />
      <div className="toast-body">
        <p className="toast-title">{title}</p>
        {text ? <p className="toast-text">{text}</p> : null}
      </div>
      <Button className="toast-close" aria-label={t.t('ng-ui-shell-toast-close')} onPress={onDismiss}>
        <IconClose size={16} />
      </Button>
    </div>
  );
}

function CoreToast({ notice }: { notice: NoticeView }) {
  const { t, dispatch } = useApp();
  const kind: Toast['kind'] = isErrorNotice(notice.code) ? 'error' : SUCCESS_CODES.has(notice.code) ? 'success' : 'info';
  const id = notice.id;
  const [dismiss] = useState(() => () => void dispatch({ type: 'dismiss_notice', id }));
  return (
    <div data-notice-code={notice.code}>
      <ToastCard kind={kind} title={t.code(notice.code, notice.params)} onDismiss={dismiss} />
    </div>
  );
}

function UiToast({ toast, onDismiss }: { toast: Toast; onDismiss: (id: number) => void }) {
  const [dismiss] = useState(() => () => onDismiss(toast.id));
  return <ToastCard kind={toast.kind} title={toast.text} text={toast.detail} onDismiss={dismiss} />;
}

export function Toasts({ toasts, onDismiss, notices = [] }: { toasts: Toast[]; onDismiss: (id: number) => void; notices?: NoticeView[] }) {
  const { t } = useApp();
  return (
    <section className="toasts" aria-label={t.t('ng-ui-shell-toasts')}>
      <div className="toasts-list" aria-live="polite" aria-relevant="additions text">
        {notices.map((n) => (
          <CoreToast key={`core-${n.id}`} notice={n} />
        ))}
        {toasts.map((x) => (
          <UiToast key={x.id} toast={x} onDismiss={onDismiss} />
        ))}
      </div>
    </section>
  );
}
