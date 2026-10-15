/**
 * Benachrichtigungen unten rechts in einer dauerhaften Live-Region. Erfolg/Info verschwinden nach 6 s
 * (pausieren bei Fokus/Hover), Fehler sind `alert` und bleiben stehen (WCAG 2.2.1).
 */
import { useEffect, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { useAppState, useDispatch } from '../app/store';
import { useT } from '../i18n';
import type { Toast } from '../model/types';
import { IconAlertCircle, IconCheckCircle, IconClose, IconInfo } from './Icons';

export const TOAST_MS = 6000;

function ToastItem({ toast }: { toast: Toast }) {
  const dispatch = useDispatch();
  const t = useT();
  const [paused, setPaused] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (toast.kind === 'error' || paused) return undefined;
    const timer = setTimeout(() => dispatch({ type: 'dismissToast', id: toast.id }), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast.id, toast.kind, paused, dispatch]);
  const Icon = toast.kind === 'success' ? IconCheckCircle : toast.kind === 'error' ? IconAlertCircle : IconInfo;
  return (
    <div
      ref={ref}
      className={`toast toast-${toast.kind}`}
      role={toast.kind === 'error' ? 'alert' : undefined}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <Icon size={20} className="toast-icon" />
      <div className="toast-body">
        <p className="toast-title">{toast.title}</p>
        {toast.text ? <p className="toast-text">{toast.text}</p> : null}
        {toast.action ? (
          <Button
            className="toast-action"
            onPress={() => {
              toast.action?.run();
              dispatch({ type: 'dismissToast', id: toast.id });
            }}
          >
            {toast.action.label}
          </Button>
        ) : null}
      </div>
      <Button className="toast-close" aria-label={t('toast.close')} onPress={() => dispatch({ type: 'dismissToast', id: toast.id })}>
        <IconClose size={16} />
      </Button>
    </div>
  );
}

export function Toasts() {
  const { toasts } = useAppState();
  const t = useT();
  return (
    <section className="toasts" aria-label={t('toast.region')}>
      <div className="toasts-list" aria-live="polite" aria-relevant="additions text">
        {toasts.map((x) => (
          <ToastItem key={x.id} toast={x} />
        ))}
      </div>
    </section>
  );
}
