/**
 * Hinweise ohne Rückfrage (Toasts). Info als `status`, Fehler als `alert`; schließen per Knopf,
 * Info verschwindet nach 8 s (Fehler bleiben, WCAG 2.2.1). Jeder Hinweis hat seine eigene Frist –
 * neue Hinweise verlängern ältere nicht.
 */
import { useEffect } from 'react';
import { Button } from 'react-aria-components';
import { useT, type Toast } from '../app/context';
import { IconAlert, IconClose, IconInfo } from './icons';

/** Anzeigedauer eines Info-Hinweises. */
export const INFO_TOAST_MS = 8000;

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: (id: number) => void }) {
  const t = useT();
  useEffect(() => {
    if (toast.kind !== 'info') return undefined;
    const timer = setTimeout(() => onDismiss(toast.id), INFO_TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast.id, toast.kind, onDismiss]);
  return (
    <div className={`toast toast-${toast.kind}`} role={toast.kind === 'error' ? 'alert' : 'status'}>
      {toast.kind === 'error' ? <IconAlert size={18} /> : <IconInfo size={18} />}
      <span className="toast-text">{toast.text}</span>
      <Button className="icon-button" aria-label={t.t('ng-ui-close')} onPress={() => onDismiss(toast.id)}>
        <IconClose size={16} />
      </Button>
    </div>
  );
}

export function Toasts({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((x) => (
        <ToastItem key={x.id} toast={x} onDismiss={onDismiss} />
      ))}
    </div>
  );
}
