/**
 * Hinweise ohne Rückfrage (Toasts): die des Kerns (`ViewState::notices`) und eigene der Oberfläche.
 * Info als `status`, Fehler als `alert`; schließen per Knopf,
 * Info verschwindet nach 8 s (Fehler bleiben, WCAG 2.2.1). Jeder Hinweis hat seine eigene Frist –
 * neue Hinweise verlängern ältere nicht.
 */
import { useEffect } from 'react';
import { Button } from 'react-aria-components';
import { useT, type Toast } from '../app/context';
import type { NoticeView } from '../core/types';
import { CoreNotice } from './CoreNotices';
import { IconAlert, IconClose, IconInfo } from './icons';
import { INFO_TOAST_MS } from './toast-timing';

export { INFO_TOAST_MS };

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

/** Hinweise des Kerns (`ViewState::notices`, älteste zuerst) und eigene Hinweise der Oberfläche. */
export function Toasts({ toasts, onDismiss, notices = [] }: { toasts: Toast[]; onDismiss: (id: number) => void; notices?: NoticeView[] }) {
  return (
    <div className="toasts" aria-live="polite">
      {notices.map((n) => (
        <CoreNotice key={`core-${n.id}`} notice={n} />
      ))}
      {toasts.map((x) => (
        <ToastItem key={x.id} toast={x} onDismiss={onDismiss} />
      ))}
    </div>
  );
}
