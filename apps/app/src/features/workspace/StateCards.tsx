/**
 * Zustandskarten des Hauptbereichs (Gestalt aus dem Prototyp, „Fehler in Alltagssprache mit einer Aktion“,
 * ADR 0004 §2.11): Start des Kerns, Laden/Analyse, Fehler beim Öffnen (u. a. kennwortgeschützt – ohne
 * Kennwortfeld, §8 F1 – mit „Anderes Dokument öffnen“ = `cancel` + Dateiauswahl), Warnung der
 * Eingangsanalyse ohne Dokumentansicht und Programmende. Technik steht in den aufklappbaren
 * „Technischen Details“ (Code, Zeitpunkt).
 */
import type { ReactNode } from 'react';
import { useApp } from '../../app/context';
import type { StepView } from '../../core/types';
import { Button, Details, Spinner } from '../../ui/controls';
import { IconAlert, IconAlertCircle, IconCheckCircle, IconFileText, IconLock } from '../../ui/Icons';
import { friendlyError } from '../signing/texts';
import './workspace.css';

function StateCard({ icon, tone, title, children, labelId }: { icon: ReactNode; tone: 'warning' | 'danger' | 'info'; title: string; children: ReactNode; labelId: string }) {
  return (
    <div className="state-card-wrap">
      <section className="state-card" aria-labelledby={labelId}>
        <span className={`state-icon state-icon-${tone}`} aria-hidden="true">
          {icon}
        </span>
        <h1 className="state-title" id={labelId} tabIndex={-1} data-main-heading="">
          {title}
        </h1>
        {children}
      </section>
    </div>
  );
}

export function StartingCard() {
  const { t } = useApp();
  return (
    <div className="state-card-wrap">
      <p className="step-hint" role="status">
        <Spinner size={20} /> {t.t('ng-ui-shell-starting')}
      </p>
    </div>
  );
}

export function LoadingCard({ step }: { step: Extract<StepView, { step: 'loading' }> }) {
  const { t, dispatch } = useApp();
  const current = step.queue.find((q) => q.current);
  return (
    <StateCard icon={<IconFileText size={26} />} tone="info" title={current?.name ?? t.t('ng-ui-ws-loading-title')} labelId="loading-title">
      <p className="state-text step-hint" role="status">
        <Spinner size={16} /> {t.t('ng-ui-ws-loading')}
      </p>
      <div className="state-actions">
        <Button variant="ghost" onPress={() => void dispatch({ type: 'cancel' })}>
          {t.t('ng-ui-sc-cancel')}
        </Button>
      </div>
    </StateCard>
  );
}

function when(iso: string | null, lang: 'de' | 'en'): string | null {
  if (!iso) return null;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : new Intl.DateTimeFormat(lang === 'de' ? 'de-AT' : 'en-GB', { dateStyle: 'medium', timeStyle: 'medium' }).format(d);
}

/** Fehler außerhalb der Signierkarte (Öffnen, Analyse, Ausgabe, Einstellungen). */
export function ErrorCard({ step }: { step: Extract<StepView, { step: 'error' }> }) {
  const { t, dispatch, browse } = useApp();
  const locked = step.code === 'error-PDFPwdProtected' || step.code === 'pdf-user-password';
  const friendly = friendlyError(t, step.code);
  const at = when(step.context.occurred_at, t.lang);
  const docName = step.context.doc?.name ?? null;
  const other = async () => {
    const ok = await dispatch({ type: 'cancel' });
    if (ok) void browse();
  };
  return (
    <StateCard icon={locked ? <IconLock size={26} /> : <IconAlertCircle size={26} />} tone={locked ? 'warning' : 'danger'} title={friendly.title} labelId="error-title">
      <p className="state-text pre-line" role="alert">
        {locked ? t.t('ng-ui-ws-locked-text') : t.code(step.code, step.params)}
      </p>
      <div className="state-actions">
        {step.retry ? (
          <Button variant="primary" onPress={() => void dispatch({ type: 'retry' })}>
            {friendly.action ?? t.t('ng-ui-ws-retry')}
          </Button>
        ) : null}
        {locked ? (
          <Button variant="primary" onPress={() => void other()}>
            {t.t('ng-ui-ws-locked-other')}
          </Button>
        ) : null}
        <Button variant="ghost" onPress={() => void dispatch({ type: 'cancel' })}>
          {t.t('ng-ui-close')}
        </Button>
      </div>
      <Details label={t.t('ng-ui-sc-details')} level={2}>
        <p className="mono-small">{[step.code, docName, at].filter(Boolean).join(' · ')}</p>
        {step.params.detail ? <p className="mono-small pre-line">{step.params.detail}</p> : null}
      </Details>
    </StateCard>
  );
}

/** Warnung der Eingangsanalyse, falls der Kern kein Dokument zur Ansicht liefert. */
export function WarningCard({ step }: { step: Extract<StepView, { step: 'warning' }> }) {
  const { t, dispatch } = useApp();
  return (
    <StateCard icon={<IconAlert size={26} />} tone="warning" title={t.t('ng-ui-warning-title')} labelId="warning-title">
      <p className="state-text">{step.doc.name}</p>
      <p className="state-text">{t.t('ng-ui-warning-lead')}</p>
      <ul className="state-list">
        {step.codes.map((code) => (
          <li key={code} data-code={code}>
            {t.code(code)}
          </li>
        ))}
      </ul>
      <p className="state-text">{t.t('ng-ui-warning-question')}</p>
      <div className="state-actions">
        <Button variant="primary" onPress={() => void dispatch({ type: 'answer_warning', proceed: true })}>
          {t.t('ng-ui-insp-warning-proceed')}
        </Button>
        <Button variant="ghost" onPress={() => void dispatch({ type: 'answer_warning', proceed: false })}>
          {t.t('ng-ui-insp-warning-discard')}
        </Button>
      </div>
    </StateCard>
  );
}

export function FinishedCard({ exitCode }: { exitCode: number }) {
  const { t } = useApp();
  return (
    <StateCard icon={<IconCheckCircle size={26} />} tone="info" title={t.t('ng-ui-finished')} labelId="finished-title">
      {exitCode !== 0 ? <p className="state-text">{t.t('ng-ui-finished-code', { code: exitCode })}</p> : null}
    </StateCard>
  );
}
