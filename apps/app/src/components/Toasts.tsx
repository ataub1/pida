/**
 * Hinweise ohne Rückfrage (Toasts). Info als `status`, Fehler als `alert`; schließen per Knopf,
 * Info verschwindet nach 8 s (Fehler bleiben, WCAG 2.2.1).
 */
import { useEffect } from 'react';
import { Button } from 'react-aria-components';
import { useT, type Toast } from '../app/context';
import { IconAlert, IconClose, IconInfo } from './icons';

export function Toasts({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  const t = useT();
  useEffect(() => {
    const timers = toasts
      .filter((x) => x.kind === 'info')
      .map((x) => setTimeout(() => onDismiss(x.id), 8000));
    return () => timers.forEach(clearTimeout);
  }, [toasts, onDismiss]);
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((x) => (
        <div key={x.id} className={`toast toast-${x.kind}`} role={x.kind === 'error' ? 'alert' : 'status'}>
          {x.kind === 'error' ? <IconAlert size={18} /> : <IconInfo size={18} />}
          <span className="toast-text">{x.text}</span>
          <Button className="icon-button" aria-label={t.t('ng-ui-close')} onPress={() => onDismiss(x.id)}>
            <IconClose size={16} />
          </Button>
        </div>
      ))}
    </div>
  );
}
