/**
 * Stapelübersicht (Gestalt aus dem Prototyp `features/batch/BatchView.tsx`, ADR 0004 §2.7/§3.8): Liste mit
 * Miniatur, Seitenzahl, Größe und Status, Konto-Chip und „Alle signieren (n)“, darunter die Schätzung
 * („3 Dokumente · 1 Anmeldung · 3 Bestätigungen“), der Speicherort und der Kurzhinweis Datenschutz;
 * während eines Laufs Fortschritt k/n (Details in der Stapelkarte), danach die Zusammenfassung über alle
 * Einträge mit Speicherort und „Im Ordner zeigen“.
 *
 * Alles kommt aus `ViewState::batch` (Status, Schätzung, Ausgabenamen, Ordner). „Alle signieren“ ist eine
 * ausdrückliche Aktion (A1): mit gewähltem Signiermittel `start_batch`, sonst öffnet sich das Menü des
 * Konto-Chips und die Wahl dort startet. Ordnerwahl und der Umgang mit vorhandenen Dateien werden **vor**
 * dem Start festgelegt (`StartBatchDialog`, nur wenn es etwas zu entscheiden gibt). „Für diesen Stapel
 * automatisch positionieren“ bzw. der Hinweis „laut Einstellung“: `BatchAutoPosition` (Nachtrag 8).
 */
import { useCallback, useEffect, useId, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { useApp } from '../../app/context';
import { isMobile, type BackendId, type BatchFlowView, type BatchItemView, type BatchView as BatchViewData } from '../../core/types';
import { AccessButton, Badge, Button, IconButton, Progress, Spinner } from '../../ui/controls';
import { IconAlert, IconCheckCircle, IconChevronRight, IconClock, IconFolder, IconInfo, IconLayers, IconLock, IconPlus, IconSignature, IconTarget, IconTrash } from '../../ui/Icons';
import { PrivacyNote } from '../settings';
import { formatSize } from '../workspace/placement';
import { SignerChip } from '../workspace/SignerChip';
import { BatchAutoPosition } from './BatchAutoPosition';
import { batchFlow, leadMsg } from './flow';
import { StartBatchDialog } from './StartBatchDialog';
import { estimateText, isOpenable, isSignable, outputText, statusDetail, statusLabel, statusTone, summaryCounts, summaryPlace } from './texts';
import './batch.css';

/** Einträge, die „Alle signieren“ an den Kern übergibt (genau diese, §3.8). */
export function signableDocs(batch: BatchViewData): number[] {
  return batch.items.filter(isSignable).map((i) => i.doc);
}

/** Startdialog nötig: Ordnerwahl oder vorhandene Zieldateien (sonst startet „Alle signieren“ direkt). */
export function needsStartDialog(batch: BatchViewData): boolean {
  return batch.output.needs_folder || batch.items.some((i) => isSignable(i) && i.target_exists);
}

export function BatchView({ batch }: { batch: BatchViewData }) {
  const { t, view, dispatch, browse, bridge, toast, registerSign } = useApp();
  const mobile = isMobile(view);
  const [chip, setChip] = useState<{ open: boolean; sign: boolean }>({ open: false, sign: false });
  const [starting, setStarting] = useState<BackendId | null>(null);
  const running = batch.running;
  const signable = batch.items.filter(isSignable);
  const existing = signable.filter((i) => i.target_exists).length;
  const progress = view.step.step === 'signing' && view.step.progress.batch ? view.step.progress : null;
  const selected = view.signers.options.find((o) => o.backend === view.signers.selected);
  const leaves = selected?.capabilities?.document_leaves_device === true;
  // Wortlaut je Signiermittel (Kern: `batch_flow`); im Lauf das des Laufs, ohne Wahl („Jedes Mal fragen“) neutral
  const flow = batchFlow(view, progress?.backend ?? view.signers.selected);
  const summary = running ? null : batch.summary;

  const start = useCallback(
    (backend: BackendId) => {
      setChip({ open: false, sign: false });
      if (needsStartDialog(batch)) setStarting(backend);
      else void dispatch({ type: 'start_batch', backend, docs: signableDocs(batch), output_dir: null, on_existing: 'keep_both' });
    },
    [batch, dispatch],
  );

  // „Alle signieren“ (Knopf, ⌘↵): mit gewähltem Signiermittel sofort, sonst das Menü des Konto-Chips
  const signAll = useCallback(() => {
    if (!view.actions.start_batch) return;
    if (view.signers.selected) start(view.signers.selected);
    else setChip({ open: true, sign: true });
  }, [view.actions.start_batch, view.signers.selected, start]);
  useEffect(() => {
    registerSign(signAll);
    return () => registerSign(null);
  }, [signAll, registerSign]);

  const lastSigned = [...batch.items].reverse().find((i) => i.status.state === 'signed');
  const reveal = () => bridge.revealOutput(lastSigned?.doc ?? null).catch(() => toast(t.t('ng-ui-open-failed'), 'error'));
  const done = progress?.confirmations_done ?? 0;
  const total = progress?.total ?? 0;
  const current = progress?.doc ?? null;
  const place = summary ? summaryPlace(t, summary, batch.output) : null;
  const aborted = !!summary && summary.aborted;

  return (
    <div className="batch-view" data-running={running || undefined}>
      <div className="batch-head">
        <div className="batch-titles">
          <p className="eyebrow">
            <IconLayers size={16} />
            {t.t('ng-ui-batch-title')}
          </p>
          <h1 className="view-title" id="main-heading" tabIndex={-1} data-main-heading="">
            {t.t('ng-ui-batch-docs', { n: batch.items.length })}
          </h1>
          <p className="view-lead">{t.t(leadMsg(flow))}</p>
        </div>
        <div className="batch-actions">
          <AccessButton msg="ng-ui-batch-add" icon={<IconPlus size={17} />} onPress={() => void browse()} isDisabled={!view.actions.open_files} />
          <SignerChip isOpen={chip.open} onOpenChange={(open) => setChip({ open, sign: open ? chip.sign : false })} signMode={chip.sign} onSignWith={start} />
          <AccessButton
            msg="ng-ui-batch-sign-all"
            variant="primary"
            size="lg"
            className="batch-sign"
            icon={<IconSignature size={18} />}
            aria-label={t.t('ng-ui-batch-sign-all-long', { n: signable.length })}
            onPress={signAll}
            isDisabled={!view.actions.start_batch}
          >
            <span className="btn-count" aria-hidden="true">
              {signable.length}
            </span>
          </AccessButton>
        </div>
      </div>

      {!running ? (
        <div className="batch-facts">
          {batch.estimate && batch.estimate.documents === 0 ? null : batch.estimate ? (
            <p className="batch-fact" data-testid="batch-estimate">
              <IconClock size={15} />
              <span>
                <span className="batch-fact-label">{t.t('ng-ui-batch-estimate')}: </span>
                {estimateText(t, batch.estimate)}
              </span>
            </p>
          ) : (
            <p className="batch-fact">
              <IconInfo size={15} />
              <span>{t.t('ng-ui-batch-estimate-none')}</span>
            </p>
          )}
          <p className="batch-fact" data-testid="batch-output">
            <IconFolder size={15} />
            <span>{outputText(t, batch.output)}</span>
          </p>
          {existing > 0 ? (
            <p className="batch-fact">
              <IconAlert size={15} />
              <span>{t.t('ng-ui-batch-existing', { n: existing })}</span>
            </p>
          ) : null}
          {batch.estimate?.large ? (
            <p className="batch-fact">
              <IconInfo size={15} />
              <span>{t.code('ng-batch-large', { count: String(batch.estimate.documents) })}</span>
            </p>
          ) : null}
          {leaves ? <PrivacyNote /> : null}
          <BatchAutoPosition batch={batch} />
        </div>
      ) : null}

      {running ? (
        <div className="batch-progress">
          <Progress label={t.t('ng-ui-batch-progress', { done, total })} value={total ? (done / total) * 100 : 0} valueLabel={`${done} / ${total}`} />
          {current ? (
            <p className="batch-progress-current">
              <Spinner size={14} />
              {current.name}
            </p>
          ) : null}
        </div>
      ) : null}

      {summary ? (
        <div className={`callout ${aborted || summary.signed === 0 ? 'callout-info' : 'callout-success'} batch-summary`} role="status">
          {aborted ? <IconInfo size={22} /> : <IconCheckCircle size={22} />}
          <div>
            <p className="callout-title">{t.t(aborted ? 'ng-ui-batch-sum-title-aborted' : 'ng-ui-batch-sum-title')}</p>
            <p>{summaryCounts(t, summary)}</p>
            {place ? <p className="callout-muted">{place}</p> : null}
            <div className="callout-actions">
              {!mobile && lastSigned ? (
                <Button variant="secondary" size="sm" onPress={() => void reveal()}>
                  <IconFolder size={15} />
                  {t.t('ng-ui-batch-reveal')}
                </Button>
              ) : null}
              <Button variant="ghost" size="sm" onPress={() => void dispatch({ type: 'navigate', step: 'Open' })} isDisabled={!view.actions.home}>
                {t.t('ng-ui-batch-new')}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      <ul className="batch-list" aria-label={t.t('ng-ui-batch-list')}>
        {batch.items.map((item) => (
          <BatchRow key={item.doc} item={item} running={running} flow={flow} />
        ))}
      </ul>

      {starting ? <StartBatchDialog batch={batch} backend={starting} onClose={() => setStarting(null)} /> : null}
    </div>
  );
}

function BatchRow({ item, running, flow }: { item: BatchItemView; running: boolean; flow: BatchFlowView | null }) {
  const { t, dispatch, bridge } = useApp();
  const statusId = useId();
  const s = item.status;
  const openable = isOpenable(item) && !running;
  const label = statusLabel(t, s);
  const detail = statusDetail(t, item, flow);
  const pages = item.page_count !== null ? t.t('ng-ui-shell-pages', { n: item.page_count }) : null;
  const size = item.size_bytes !== null ? formatSize(item.size_bytes, t.lang) : null;
  const meta = pages && size ? t.t('ng-ui-shell-doc-info', { pages, size }) : (pages ?? size ?? '');
  const open = () => void dispatch({ type: 'open_batch_item', doc: item.doc });
  const icon =
    s.state === 'active' || s.state === 'analyzing' ? (
      <Spinner size={12} />
    ) : s.state === 'signed' ? (
      <IconCheckCircle size={13} />
    ) : s.state === 'rejected' || s.state === 'failed' || s.state === 'needs_review' ? (
      <IconAlert size={13} />
    ) : s.state === 'positioned' ? (
      <IconTarget size={13} />
    ) : undefined;
  const content = (
    <>
      <span className="batch-thumb" aria-hidden="true">
        {item.thumbnail ? <img src={bridge.thumbUrl(item.doc)} alt="" draggable={false} /> : null}
        {s.state === 'rejected' ? (
          <span className="batch-thumb-lock">
            <IconLock size={16} />
          </span>
        ) : null}
      </span>
      <span className="batch-row-text">
        <span className="batch-row-name">{item.name}</span>
        {meta ? <span className="batch-row-meta">{meta}</span> : null}
      </span>
    </>
  );
  return (
    <li className={`batch-row is-${s.state}`} data-doc={item.doc}>
      {openable ? (
        <AriaButton className="batch-row-main" onPress={open} aria-label={t.t('ng-ui-batch-open', { name: item.name })} aria-describedby={statusId}>
          {content}
        </AriaButton>
      ) : (
        <div className="batch-row-main">{content}</div>
      )}
      {/* Beschreibung des Zeilenknopfs (Status + Detail), damit er beim Tabben nicht nur den Dateinamen nennt */}
      {openable ? (
        <span id={statusId} hidden>
          {detail ? `${label}, ${detail}` : label}
        </span>
      ) : null}
      <span className="batch-row-status">
        <Badge tone={statusTone(s)} icon={icon}>
          {label}
        </Badge>
        {detail ? <span className="batch-row-detail">{detail}</span> : null}
      </span>
      <span className={`batch-row-action${openable && s.state !== 'needs_review' ? ' has-caret' : ''}`}>
        {/* Entfernen für jedes noch nicht signierte Dokument, solange kein Lauf aktiv ist (abgelehnte zeigen es als Hauptaktion) */}
        {!running && s.state !== 'signed' && s.state !== 'rejected' && s.state !== 'active' && s.state !== 'queued' ? (
          <IconButton size="sm" className="batch-row-remove" label={t.t('ng-ui-batch-remove', { name: item.name })} onPress={() => void dispatch({ type: 'remove_from_batch', doc: item.doc })}>
            <IconTrash size={15} />
          </IconButton>
        ) : null}
        {s.state === 'needs_review' && !running ? (
          <Button variant="secondary" size="sm" onPress={open}>
            <IconAlert size={15} />
            {t.t('ng-ui-batch-review')}
          </Button>
        ) : s.state === 'rejected' && !running ? (
          <Button variant="secondary" size="sm" onPress={() => void dispatch({ type: 'remove_from_batch', doc: item.doc })} aria-label={t.t('ng-ui-batch-remove', { name: item.name })}>
            {t.t('ng-ui-batch-remove-short')}
          </Button>
        ) : openable ? (
          <IconChevronRight size={18} className="batch-row-caret" />
        ) : null}
      </span>
    </li>
  );
}
