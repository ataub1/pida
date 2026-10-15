/**
 * Start, Fehler mit „Wiederholen“ (docs/analyse/01 §4.10) und Programmende.
 */
import { useApp } from '../app/context';
import { IconAlert } from '../components/icons';
import { AccessButton, ScreenHeading, Spinner } from '../components/ui';

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
