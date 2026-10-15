/**
 * Start, Laden/Analyse, Warnung der Eingangsanalyse (Plan §6.1, mit „Trotzdem signieren“/„Verwerfen“),
 * Fehler mit „Wiederholen“ (docs/analyse/01 §4.10) und Programmende.
 */
import { useApp } from '../app/context';
import { IconAlert } from '../components/icons';
import { QueuePanel } from '../components/QueuePanel';
import { AccessButton, ScreenHeading, Spinner } from '../components/ui';
import type { DocInfo, QueueItemView } from '../core/types';

export function StartingScreen() {
  const { t } = useApp();
  return (
    <div className="screen screen-starting">
      <ScreenHeading>{t.t('ng-ui-starting')}</ScreenHeading>
      <p className="status-line" role="status">
        <Spinner size={22} /> {t.t('common-working')}
      </p>
    </div>
  );
}

/** Dokument wird geladen und analysiert (Platzhaltersuche); Abbrechen verwirft das Dokument. */
export function LoadingScreen({ queue }: { queue: QueueItemView[] }) {
  const { t, dispatch } = useApp();
  const current = queue.find((q) => q.current);
  return (
    <div className="screen screen-loading">
      <ScreenHeading subtitle={current?.name}>{t.t('ng-ui-open-title')}</ScreenHeading>
      <p className="status-line" role="status">
        <Spinner size={22} /> {t.t('ng-ui-open-analyzing')}
      </p>
      <div className="actions actions-start">
        <AccessButton msg="common-Cancel" onPress={() => dispatch({ type: 'cancel' })} />
      </div>
      <QueuePanel queue={queue} />
    </div>
  );
}

/**
 * Warnung der Eingangsanalyse mit Abbruchmöglichkeit (`/Perms /UR3`, Hybrid-XFA, unerfüllbare Seed
 * Values): „Trotzdem signieren“ oder Dokument verwerfen (`answer_warning`).
 */
export function WarningScreen({ doc, codes }: { doc: DocInfo; codes: string[] }) {
  const { t, dispatch } = useApp();
  return (
    <div className="screen screen-warning">
      <ScreenHeading subtitle={doc.name}>{t.t('ng-ui-warning-title')}</ScreenHeading>
      <div className="card warning-card" role="alert">
        <span className="error-icon" aria-hidden="true">
          <IconAlert size={30} />
        </span>
        <div>
          <p>{t.t('ng-ui-warning-lead')}</p>
          <ul className="warning-list">
            {codes.map((code) => (
              <li key={code} data-code={code}>
                {t.code(code)}
              </li>
            ))}
          </ul>
          <p>{t.t('ng-ui-warning-question')}</p>
        </div>
      </div>
      <div className="actions actions-start">
        <AccessButton msg="ng-ui-warning-proceed" variant="primary" onPress={() => dispatch({ type: 'answer_warning', proceed: true })} />
        <AccessButton msg="ng-ui-warning-discard" onPress={() => dispatch({ type: 'answer_warning', proceed: false })} />
      </div>
    </div>
  );
}

export function ErrorScreen({ code, params, retry }: { code: string; params: Record<string, string>; retry: boolean }) {
  const { t, dispatch } = useApp();
  const message = t.code(code, params);
  const detail = params.detail;
  return (
    <div className="screen screen-error">
      <ScreenHeading>{t.t('error-Title')}</ScreenHeading>
      <div className="card error-card" role="alert">
        <span className="error-icon" aria-hidden="true">
          <IconAlert size={30} />
        </span>
        <p className="pre-line">{message}</p>
      </div>
      <details className="details">
        <summary>{t.t('error-Details')}</summary>
        <p>
          <code>{code}</code>
        </p>
        {detail ? <p className="pre-line">{detail}</p> : null}
      </details>
      <div className="actions actions-start">
        {retry ? <AccessButton msg="error-Retry" variant="primary" onPress={() => dispatch({ type: 'retry' })} /> : null}
        <AccessButton msg="common-Cancel" onPress={() => dispatch({ type: 'cancel' })} />
      </div>
    </div>
  );
}

export function FinishedScreen({ exitCode }: { exitCode: number }) {
  const { t } = useApp();
  return (
    <div className="screen screen-finished">
      <ScreenHeading>{t.t('ng-ui-finished')}</ScreenHeading>
      {exitCode !== 0 ? <p className="hint">{t.t('ng-ui-finished-code', { code: exitCode })}</p> : null}
    </div>
  );
}
