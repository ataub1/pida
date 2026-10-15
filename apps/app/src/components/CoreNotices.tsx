/**
 * Hinweise ohne Rückfrage aus dem Kern (`ViewState::notices`, z. B. `error-PositioningNotPossible`,
 * `placement.notice.*`, `ng-config-reset-done`). Der Kern hält die Liste; die Oberfläche zeigt sie wie
 * Toasts an und schließt sie mit `dismiss_notice` (Knopf, Info-Hinweise zusätzlich nach
 * [`INFO_TOAST_MS`]). Fehlerhinweise (`error-…`, `exception-…`) bleiben bis zum Schließen (WCAG 2.2.1).
 */
import { useEffect } from 'react';
import { Button } from 'react-aria-components';
import { useApp } from '../app/context';
import type { NoticeView } from '../core/types';
import { IconAlert, IconClose, IconInfo } from './icons';
import { INFO_TOAST_MS } from './toast-timing';

/** Fehlerhinweis (bleibt stehen)? */
export function isErrorNotice(code: string): boolean {
  return /^(error|exception)[-.]/.test(code) || code.endsWith('-failed');
}

export function CoreNotice({ notice }: { notice: NoticeView }) {
  const { t, dispatch } = useApp();
  const error = isErrorNotice(notice.code);
  const dismiss = () => void dispatch({ type: 'dismiss_notice', id: notice.id });
  useEffect(() => {
    if (error) return undefined;
    const timer = setTimeout(() => void dispatch({ type: 'dismiss_notice', id: notice.id }), INFO_TOAST_MS);
    return () => clearTimeout(timer);
  }, [notice.id, error, dispatch]);
  return (
    <div className={`toast toast-${error ? 'error' : 'info'}`} role={error ? 'alert' : 'status'} data-notice-code={notice.code}>
      {error ? <IconAlert size={18} /> : <IconInfo size={18} />}
      <span className="toast-text">{t.code(notice.code, notice.params)}</span>
      <Button className="icon-button" aria-label={t.t('ng-ui-close')} onPress={dismiss}>
        <IconClose size={16} />
      </Button>
    </div>
  );
}

