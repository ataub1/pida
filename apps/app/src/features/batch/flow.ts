/**
 * Wortlaut des Stapels je Signiermittel (Mobile-Check 16.10.2026, ADR 0004 Nachtrag 12): Welcher Ablauf gilt,
 * liefert der Kern je Signiermittel (`SignerView::batch_flow` aus den Fähigkeiten) – „einmal anmelden, dann
 * jedes Dokument einzeln bestätigen“ und „Bestätigung k von n“ nur bei ID Austria, beim Keystore „einmal
 * entsperren“ und „Dokument k von n“, bei der Chipkarte „jedes bestätigen Sie einzeln“. Ohne gewähltes
 * Signiermittel („Jedes Mal fragen“) ein neutraler Text. Hier nur die Zuordnung zu den Texten.
 */
import type { BackendId, BatchFlowView, ViewState } from '../../core/types';

/** Ablauf für `backend` laut Kern (`null` ohne Signiermittel bzw. ohne Fähigkeiten). */
export function batchFlow(view: Pick<ViewState, 'signers'>, backend: BackendId | null | undefined): BatchFlowView | null {
  if (!backend) return null;
  return view.signers.options.find((o) => o.backend === backend)?.batch_flow ?? null;
}

/** Text je Ablauf; `ask` = kein Signiermittel gewählt bzw. Ablauf unbekannt. */
type FlowTexts = Record<BatchFlowView | 'ask', string>;

function pick(texts: FlowTexts, flow: BatchFlowView | null): string {
  return texts[flow ?? 'ask'];
}

/** Einleitung der Stapelübersicht. */
export function leadMsg(flow: BatchFlowView | null): string {
  return pick(
    { login_then_confirm_each: 'ng-ui-batch-lead', confirm_each: 'ng-ui-batch-lead-each', unlock_once: 'ng-ui-batch-lead-unlock', ask: 'ng-ui-batch-lead-ask' },
    flow,
  );
}

/** Hinweis der Startfläche zu mehreren Dateien (Desktop). */
export function dropHintMsg(flow: BatchFlowView | null): string {
  return pick(
    {
      login_then_confirm_each: 'ng-ui-start-drop-hint',
      confirm_each: 'ng-ui-start-drop-hint-each',
      unlock_once: 'ng-ui-start-drop-hint-unlock',
      ask: 'ng-ui-start-drop-hint-ask',
    },
    flow,
  );
}

/** Je Dokument eine eigene Bestätigung (Kopf „Bestätigung k von n“, Zähler „Bestätigungen“)? */
export function confirmsEach(flow: BatchFlowView | null): boolean {
  return flow !== 'unlock_once';
}

/** „Überspringen“ im Startdialog: „ohne Bestätigung in der App“ nur bei ID Austria. */
export function skipDescMsg(flow: BatchFlowView | null): string {
  return flow === 'login_then_confirm_each' ? 'ng-ui-batch-start-skip-desc' : 'ng-ui-batch-start-skip-desc-local';
}

/** Rückfrage vor dem Abbruch: „Das Passwort wird vergessen“ nur mit Anmeldung (ID Austria). */
export function abortTextMsg(flow: BatchFlowView | null): string {
  return flow === 'login_then_confirm_each' ? 'ng-ui-sc-batch-abort-text' : 'ng-ui-sc-batch-abort-text-local';
}

/** Fußnote einer Sitzungspause mit neuer Sitzung. */
export function newSessionMsg(flow: BatchFlowView | null): string {
  return pick(
    {
      login_then_confirm_each: 'ng-ui-sc-batch-pause-new-session',
      confirm_each: 'ng-ui-sc-batch-pause-new-session-each',
      unlock_once: 'ng-ui-sc-batch-pause-new-session-unlock',
      ask: 'ng-ui-sc-batch-pause-new-session',
    },
    flow,
  );
}
