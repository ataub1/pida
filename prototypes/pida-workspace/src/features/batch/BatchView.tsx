/**
 * Stapelbetrieb: Liste mit Miniatur, Seitenzahl und Status; „Alle signieren“ mit einer Anmeldung,
 * Fortschrittsbalken und Zusammenfassung. Einzelne Dokumente öffnen den Arbeitsbereich.
 */
import { useEffect, useId, useRef } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { useAppState, useDispatch } from '../../app/store';
import { useTiming } from '../../app/timing';
import { useT } from '../../i18n';
import type { DocState, DocStatus } from '../../model/types';
import { Badge, Button, Progress, Spinner } from '../../ui/controls';
import { IconAlert, IconCheckCircle, IconChevronRight, IconFolder, IconLayers, IconLock, IconPlus, IconSignature, IconTarget } from '../../ui/Icons';
import { MockPage } from '../workspace/MockPage';
import { SignerChip } from '../workspace/SignerChip';

/**
 * Statusfarben: grau = wartet, blau = läuft gerade, grün = fertig, rot = Fehler. „Wird signiert“ bewusst
 * nicht im roten Markenakzent – das wäre neben „Fehler“ nicht zu unterscheiden.
 */
const TONE: Record<DocStatus, 'neutral' | 'info' | 'accent' | 'success' | 'danger'> = {
  ready: 'neutral',
  positioned: 'neutral',
  signing: 'info',
  done: 'success',
  error: 'danger',
};

export function BatchView({ onAddFiles }: { onAddFiles: () => void }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const timing = useTiming();
  const t = useT();
  const run = state.batchRun;
  const docs = state.docs;
  const signable = docs.filter((d) => d.status === 'ready' || d.status === 'positioned');
  const fixedFolder = state.settings.saveTarget === 'folder';
  const folder = fixedFolder ? state.settings.saveFolder : (docs[0]?.spec.folder ?? '');
  const announcedSummary = useRef(false);

  // Stapel abarbeiten
  useEffect(() => {
    if (!run || run.phase !== 'running') return undefined;
    const timer = setTimeout(() => dispatch({ type: 'batchStep' }), timing.batchItemMs);
    return () => clearTimeout(timer);
  }, [run, dispatch, timing.batchItemMs]);

  // Abschluss als Toast (Variante „Fertig“ im Stapelbetrieb)
  useEffect(() => {
    if (run?.phase === 'summary' && !announcedSummary.current) {
      announcedSummary.current = true;
      dispatch({
        type: 'toast',
        toast: {
          kind: 'success',
          title: t.n('batch.toast.title', run.done.length),
          text: t.n('batch.toast.text', run.done.length),
          action: { label: t('batch.reveal'), run: () => dispatch({ type: 'toast', toast: { kind: 'info', title: t('done.demo.reveal', { folder }) } }) },
        },
      });
    }
    if (!run) announcedSummary.current = false;
  }, [run, dispatch, t, folder]);

  const total = run ? run.done.length + run.queue.length : 0;
  // Zusammenfassung über den ganzen Stapel (auch vorher einzeln signierte Dokumente)
  const doneCount = docs.filter((d) => d.status === 'done').length;
  const current = run?.queue[0] ? docs.find((d) => d.spec.id === run.queue[0]) : undefined;

  return (
    <div className="batch-view">
      <header className="batch-head">
        <div className="batch-titles">
          <p className="eyebrow">
            <IconLayers size={16} />
            {t('batch.title')}
          </p>
          <h1 className="view-title">{t.n('common.docs', docs.length)}</h1>
          <p className="view-lead">{t('batch.lead')}</p>
        </div>
        <div className="batch-actions">
          <Button variant="secondary" onPress={onAddFiles} isDisabled={!!run && run.phase === 'running'}>
            <IconPlus size={17} />
            {t('start.drop.button')}
          </Button>
          <SignerChip />
          <Button variant="primary" size="lg" onPress={() => dispatch({ type: 'startSigning' })} isDisabled={signable.length === 0 || run?.phase === 'running'}>
            <IconSignature size={18} />
            {t('batch.signAll')}
            <span className="btn-count">{signable.length}</span>
          </Button>
        </div>
      </header>

      {run?.phase === 'running' ? (
        <div className="batch-progress">
          <Progress label={t('batch.progress', { done: run.done.length, total })} value={total ? (run.done.length / total) * 100 : 0} valueLabel={`${run.done.length} / ${total}`} />
          {current ? (
            <p className="batch-progress-current">
              <Spinner size={14} />
              {current.spec.fileName}
            </p>
          ) : null}
        </div>
      ) : null}

      {run?.phase === 'summary' ? (
        <div className="callout callout-success batch-summary" role="status">
          <IconCheckCircle size={22} />
          <div>
            <p className="callout-title">{t('batch.summary.title')}</p>
            <p>
              {fixedFolder
                ? t('batch.summary.text', { ok: doneCount, total: docs.length, folder })
                : t('batch.summary.textOriginal', { ok: doneCount, total: docs.length })}
            </p>
            {run.skipped.length ? <p className="callout-muted">{t.n('batch.summary.skipped', run.skipped.length)}</p> : null}
            <div className="callout-actions">
              <Button variant="secondary" size="sm" onPress={() => dispatch({ type: 'toast', toast: { kind: 'info', title: t('done.demo.reveal', { folder }) } })}>
                <IconFolder size={15} />
                {t('batch.reveal')}
              </Button>
              <Button variant="ghost" size="sm" onPress={() => dispatch({ type: 'goHome' })}>
                {t('batch.new')}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      <ul className="batch-list" aria-label={t('batch.list')}>
        {docs.map((d) => (
          <BatchRow key={d.spec.id} doc={d} disabled={run?.phase === 'running'} skipped={!!run && run.skipped.includes(d.spec.id)} />
        ))}
      </ul>
    </div>
  );
}

function BatchRow({ doc, disabled, skipped }: { doc: DocState; disabled: boolean; skipped: boolean }) {
  const t = useT();
  const dispatch = useDispatch();
  const first = doc.spec.pages[0];
  const scale = first ? 46 / first.w : 1;
  const usesPlaceholder = !!doc.placement?.fieldId;
  const detail =
    doc.status === 'error'
      ? skipped
        ? `${t('batch.detail.locked')} · ${t('batch.detail.skipped')}`
        : t('batch.detail.locked')
      : doc.status === 'done'
        ? t('batch.detail.done', { file: doc.outputName ?? '' })
        : doc.status === 'positioned'
          ? usesPlaceholder
            ? t('batch.detail.placeholder')
            : t('batch.detail.positioned')
          : doc.status === 'ready'
            ? t('batch.detail.ready')
            : '';
  const open = () => dispatch({ type: 'openDoc', id: doc.spec.id });
  const statusId = useId();
  const hasCaret = doc.status === 'ready' || doc.status === 'positioned';
  return (
    <li className={`batch-row is-${doc.status}`}>
      <AriaButton
        className="batch-row-main"
        onPress={open}
        isDisabled={disabled || doc.status === 'done' || doc.status === 'signing'}
        aria-label={t('batch.open', { name: doc.spec.fileName })}
        aria-describedby={statusId}
      >
        <span className="batch-thumb" aria-hidden="true">
          {first ? <MockPage page={first} scale={scale} /> : null}
          {doc.status === 'error' ? (
            <span className="batch-thumb-lock">
              <IconLock size={16} />
            </span>
          ) : null}
        </span>
        <span className="batch-row-text">
          <span className="batch-row-name">{doc.spec.fileName}</span>
          <span className="batch-row-meta">
            {t.n('common.pages', doc.spec.pages.length)} · {doc.spec.sizeKb} KB
          </span>
        </span>
      </AriaButton>
      {/* Beschreibung des Zeilenknopfs (Status + Detail), damit er beim Tabben nicht nur den Dateinamen nennt */}
      <span id={statusId} hidden>
        {detail ? `${t(`batch.status.${doc.status}`)}, ${detail}` : t(`batch.status.${doc.status}`)}
      </span>
      <span className="batch-row-status">
        <Badge
          tone={TONE[doc.status]}
          icon={
            doc.status === 'signing' ? (
              <Spinner size={12} />
            ) : doc.status === 'done' ? (
              <IconCheckCircle size={13} />
            ) : doc.status === 'error' ? (
              <IconAlert size={13} />
            ) : doc.status === 'positioned' ? (
              <IconTarget size={13} />
            ) : undefined
          }
        >
          {t(`batch.status.${doc.status}`)}
        </Badge>
        <span className="batch-row-detail">{detail}</span>
      </span>
      <span className={`batch-row-action${hasCaret ? ' has-caret' : ''}`}>
        {doc.status === 'error' ? (
          <Button variant="secondary" size="sm" onPress={open} isDisabled={disabled}>
            <IconLock size={15} />
            {t('batch.unlock')}
          </Button>
        ) : doc.status === 'ready' || doc.status === 'positioned' ? (
          <IconChevronRight size={18} className="batch-row-caret" />
        ) : null}
      </span>
    </li>
  );
}
