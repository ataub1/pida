/**
 * TS-Mock-Kern für `VITE_MOCK=1`, die Browser-Vorschau und die Komponententests (ADR 0004 §4.8).
 *
 * Er spielt den Arbeitsbereich-Modus des Kerns nach (K1: `CoreOptions::workspace_ui`): Öffnen →
 * automatische Platzierung → „Signieren mit X“ (`sign_with`) → Signierkarte → Fertig, liefert
 * `ViewState`s im serde-Format von `pida-core-api` und simuliert die nativen Dienste (Dialoge, Drag &
 * Drop). Er ist **kein** Ersatz für die Fachlogik des Rust-Kerns: Regeln nur so weit, wie Bedienung und
 * Tests sie brauchen. Keine echten Zugangsdaten, kein Netz. Invariante A1 gilt auch hier: Eine
 * Signiersitzung beginnt nur nach `sign_with`, `choose_backend` (Rückfall), `resume_signature` oder
 * „Erneut versuchen“.
 *
 * Aufbau: `index.ts` (UI-A) hält die Bereiche `{ flow, settings, verify, recent, firstrun }` mit je
 * eigenem Zustandsausschnitt (`shared.ts`), verteilt jedes Ereignis an alle, leitet Signale weiter und
 * setzt die Projektionen zusammen. Der Bereich `flow` (Kern, Arbeitsbereich, Signieren, Stapel) steht hier
 * und nutzt `workspace.ts` (Dokumente, Platzierung), `signing.ts` (Signiersitzung) und `batch.ts`
 * (Stapelübersicht und -lauf mit eigenem Zustand, wie `State::BatchOverview`/`BatchSigning` des Kerns neben
 * der Warteschlange; mit `batchOverview` bei ≥ 2 Dokumenten, ADR 0004 §3.8).
 */
import { defaultErrorContext, defaultSigners, defaultSigningProgress, mainBar, noActions, workspaceDefaults } from '../defaults';
import type { Bridge, FileDragEvent } from '../bridge';
import type {
  ActionsView,
  BackendId,
  DocId,
  DoneItemView,
  ErrorOrigin,
  Lang,
  MainBarView,
  NoticeView,
  PlatformView,
  ResumeOfferView,
  SaveDialogView,
  Settings,
  SignedSummaryView,
  SignerView,
  StepView,
  UiEvent,
  ViewState,
} from '../types';
import { MockBatch, type BatchHost, type BatchPlan } from './batch';
import { firstRunArea } from './firstrun';
import { recentArea } from './recent';
import { settingsArea } from './settings';
import {
  A4,
  capabilitiesFor,
  fileNameOf,
  folderOf,
  isoWithOffset,
  joinPath,
  maskAccount,
  outputFileName,
  SANDBOX,
  type MockArea,
  type MockCtx,
  type MockOptions,
  type MockSignal,
} from './shared';
import { SigningSession } from './signing';
import { verifyArea } from './verify';
import {
  autoPlace,
  blockPreviewSvg,
  blockSize,
  changePage,
  constraintsFor,
  docInfo,
  moveBlock,
  nudgeBlock,
  pageTileSvg,
  projectDocument,
  resizeBlock,
  specForName,
  startEdit,
  toggleNewPage,
  useTarget,
  contractSpec,
  blankSpec,
  fieldSpec,
  signedSpec,
  offerSpec,
  confirmationSpec,
  lockedSpec,
  type EditState,
  type MockDoc,
  type MockDocSpec,
} from './workspace';

export type { MockOptions, MockScenario, MockSigningFlow } from './shared';
export { MOCK_SCENARIOS } from './shared';

/* ================================================================ Bereich „flow“ */

type Stage = 'starting' | 'open' | 'loading' | 'warning' | 'editing' | 'choosing' | 'signing' | 'output' | 'error' | 'finished' | 'batch';

interface OutputState {
  path: string | null;
  savePending: boolean;
  askOverwrite: string | null;
  saveDialog: SaveDialogView | null;
}

const EMPTY_OUTPUT: OutputState = { path: null, savePending: false, askOverwrite: null, saveDialog: null };

interface ErrorState {
  code: string;
  params: Record<string, string>;
  retry: boolean;
  origin: ErrorOrigin;
  occurredAt: string;
  backend: BackendId | null;
  onRetry: () => void;
  onCancel: () => void;
}

interface FlowState {
  opts: MockOptions;
  stage: Stage;
  queue: MockDoc[];
  nextDocId: number;
  edit: EditState | null;
  /** Revision der Blockvorschau (steigt mit jeder Änderung der Einstellungen) */
  preview: number;
  signing: SigningSession | null;
  /** Auswahl des Konto-Chips; `explicit` = durch Nutzerin gewählt (gilt bis zum Ende des Dokuments) */
  selected: BackendId | null;
  explicit: boolean;
  /** Passwort im „Merken“-Speicher des Backends (gemerkt nach Anmeldung mit „merken“, F-72 löscht) */
  remembered: boolean;
  output: OutputState;
  summary: SignedSummaryView | null;
  error: ErrorState | null;
  done: DoneItemView[];
  resume: ResumeOfferView | null;
  exitCode: number;
  networkErrorOnce: boolean;
  /** letzte Position/Signiermittel (Tests) */
  last: { position: string; backend: BackendId | null };
  /** Stapel (Übersicht, Lauf; auch während ein Dokument daraus geöffnet ist) */
  batch: MockBatch | null;
  /** Plan des aus dem Stapel geöffneten Dokuments (wird nach der Analyse angewendet) */
  pendingPlan: BatchPlan | null;
}

function mobile(o: MockOptions): boolean {
  return (o.platform ?? 'desktop') !== 'desktop';
}

function backendFor(choice: Settings['backend']['default']): BackendId | null {
  switch (choice) {
    case 'id_austria':
      return 'atrust_native';
    case 'local_bku':
      return 'local_bku';
    case 'keystore':
      return 'keystore';
    default:
      return null;
  }
}

function signerOptions(s: FlowState, ctx: MockCtx): SignerView[] {
  const settings = ctx.settings();
  const isMobile = ctx.platform().mobile;
  const ida = settings.id_austria;
  const ks = settings.keystore;
  const out: SignerView[] = [
    {
      backend: 'atrust_native',
      available: true,
      account: ida.remember && ida.username ? maskAccount(ida.username) : null,
      capabilities: capabilitiesFor('atrust_native', isMobile),
      notes: isMobile ? ['ng-fido2-unavailable-mobile'] : [],
      unavailable_reason: null,
    },
  ];
  if (!isMobile) {
    const ok = s.opts.localBkuAvailable ?? true;
    out.push({ backend: 'local_bku', available: ok, account: null, capabilities: ok ? capabilitiesFor('local_bku') : null, notes: [], unavailable_reason: ok ? null : 'ng-signer-localbku-not-running' });
  }
  if (ks.enabled || isMobile) {
    const ok = ks.enabled && !!ks.file;
    out.push({
      backend: 'keystore',
      available: ok,
      account: ok && ks.file ? [fileNameOf(ks.file), ks.alias].filter(Boolean).join(' · ') : null,
      capabilities: ok ? capabilitiesFor('keystore') : null,
      notes: [],
      unavailable_reason: ok ? null : isMobile ? 'ng-signer-keystore-import' : 'ng-signer-keystore-missing',
    });
  }
  return out;
}

function available(s: FlowState, ctx: MockCtx, id: BackendId): boolean {
  return signerOptions(s, ctx).some((o) => o.backend === id && o.available);
}

function blockLang(ctx: MockCtx): Lang {
  const s = ctx.settings();
  return s.signature.block_language ?? s.general.ui_language ?? 'de';
}

function current(s: FlowState): MockDoc | null {
  return s.queue[0] ?? null;
}

function makeDoc(s: FlowState, path: string, specOverride?: MockDocSpec): MockDoc {
  const name = fileNameOf(path);
  const spec = specOverride ?? specForName(name, s.opts.placeholder ?? 'none');
  return { id: s.nextDocId++, path, name, spec, signed: spec.existing.length > 0 };
}

/** Auswahl des Konto-Chips zu Beginn eines Dokuments: Standard, sofern verfügbar (§3.4, §8 F12). */
function resetSelection(s: FlowState, ctx: MockCtx): void {
  if (s.explicit && s.selected && available(s, ctx, s.selected)) return;
  const def = backendFor(ctx.settings().backend.default);
  s.selected = def && available(s, ctx, def) ? def : null;
  s.explicit = false;
}

function analyze(s: FlowState, ctx: MockCtx): void {
  const d = current(s);
  if (!d) {
    s.stage = 'open';
    return;
  }
  s.stage = 'loading';
  s.edit = null;
  ctx.later(() => {
    if (s.stage !== 'loading' || current(s) !== d) return;
    if (!/\.pdf$/i.test(d.name)) {
      fail(s, ctx, 'error-MayNotBeAPDF', {}, true, 'load', () => analyze(s, ctx), () => dropCurrent(s, ctx));
    } else if (d.spec.locked) {
      fail(s, ctx, 'error-PDFPwdProtected', {}, false, 'load', () => undefined, () => dropCurrent(s, ctx));
    } else {
      s.edit = startEdit(d, ctx.settings(), blockLang(ctx));
      if (s.pendingPlan) s.edit = applyPlan(d, s.edit, s.pendingPlan, blockSize(ctx.settings(), blockLang(ctx)));
      s.pendingPlan = null;
      // Raster der Seite des Blocks (die übrigen nach `inspect_pages`)
      if (s.edit.placement) s.edit.ready = [s.edit.placement.page_index];
      resetSelection(s, ctx);
      s.stage = (s.opts.warnings ?? []).length > 0 ? 'warning' : 'editing';
    }
  });
}

function fail(s: FlowState, ctx: MockCtx, code: string, params: Record<string, string>, retry: boolean, origin: ErrorOrigin, onRetry: () => void, onCancel: () => void): void {
  void ctx;
  s.stage = 'error';
  s.error = { code, params, retry, origin, occurredAt: isoWithOffset(new Date()), backend: s.signing?.backend ?? null, onRetry, onCancel };
}

function dropCurrent(s: FlowState, ctx: MockCtx): void {
  s.queue.shift();
  s.edit = null;
  s.output = EMPTY_OUTPUT;
  s.summary = null;
  if (s.queue.length > 0) analyze(s, ctx);
  else s.stage = 'open';
}

function finishDoc(s: FlowState, result: DoneItemView['result']): void {
  const d = current(s);
  if (d) s.done.push({ id: d.id, name: d.name, input: d.path, result });
}

/** Stapelübersicht bei ≥ 2 Dokumenten (`batch_overview`, nur im Arbeitsbereich-Modus). */
function batchOn(s: FlowState): boolean {
  return s.opts.batchOverview !== false;
}

/** Dienste des Ablaufs für den Stapel (`batch.ts`). */
function batchHost(s: FlowState, ctx: MockCtx): BatchHost {
  return {
    ctx,
    opts: s.opts,
    available: (backend) => available(s, ctx, backend),
    selected: () => s.selected,
    remembered: () => s.remembered,
    setRemembered: (value) => {
      s.remembered = value;
    },
    openSingle: (doc, plan) => {
      s.queue = [doc];
      s.pendingPlan = plan;
      s.output = EMPTY_OUTPUT;
      s.summary = null;
      s.error = null;
      analyze(s, ctx);
    },
    runEnded: () => {
      s.explicit = false;
      resetSelection(s, ctx);
    },
  };
}

/** Stapelübersicht mit diesen Dokumenten (Voranalyse im Hintergrund). */
function enterBatch(s: FlowState, ctx: MockCtx, docs: MockDoc[]): void {
  const batch = new MockBatch(batchHost(s, ctx));
  batch.add(docs);
  s.batch = batch;
  s.queue = [];
  s.edit = null;
  s.error = null;
  s.output = EMPTY_OUTPUT;
  s.summary = null;
  s.stage = 'batch';
  resetSelection(s, ctx);
}

/** Zurück in die Übersicht (aus dem geöffneten Einzeldokument). */
function leaveSingle(s: FlowState): void {
  s.signing?.close();
  s.signing = null;
  s.queue = [];
  s.edit = null;
  s.error = null;
  s.output = EMPTY_OUTPUT;
  s.summary = null;
  s.stage = 'batch';
}

/** aus dem Stapel geöffnetes Dokument (`return_to_batch`) */
function fromBatch(s: FlowState): boolean {
  const d = current(s);
  return !!s.batch && !!d && s.batch.openItem === d.id;
}

function openFiles(s: FlowState, ctx: MockCtx, paths: string[]): void {
  const fresh = paths.filter((p, i) => paths.indexOf(p) === i && !s.queue.some((d) => d.path === p) && !s.batch?.has(p)); // F-89 ohne Duplikate
  if (fresh.length === 0) return;
  if (s.batch) {
    // Übersicht: hinzufügen und voranalysieren; während eines Laufs gehören sie nicht zum Lauf
    s.batch.add(fresh.map((p) => makeDoc(s, p)));
    if (s.stage !== 'batch') ctx.notify('ng-batch-added', { count: String(fresh.length) });
    return;
  }
  const saved = s.stage === 'output' && !!s.output.path;
  const idle = saved || (s.queue.length === 0 && (s.stage === 'open' || s.stage === 'error'));
  if (idle) {
    if (saved && s.output.path) {
      // gespeichert: Dokument verbuchen, dann wie im Leerlauf
      finishDoc(s, { kind: 'signed', output: { path: s.output.path, len: 1024, sha256: '00' } });
      s.queue.shift();
      s.output = EMPTY_OUTPUT;
      s.summary = null;
    }
    const docs = [...s.queue, ...fresh.map((p) => makeDoc(s, p))];
    s.error = null;
    if (batchOn(s) && docs.length >= 2) {
      enterBatch(s, ctx, docs);
      return;
    }
    s.queue = docs;
    analyze(s, ctx);
    return;
  }
  for (const path of fresh) s.queue.push(makeDoc(s, path));
  if (s.stage === 'editing' || s.stage === 'warning' || s.stage === 'loading') ctx.notify('ng-batch-added', { count: String(fresh.length) });
}

/** Plan aus dem Bearbeitungszustand (`BackToBatch`/`ShowBatch`; `origin == Auto` → automatisch). */
function planFromEdit(d: MockDoc, e: EditState): BatchPlan | null {
  if (e.invisible && !e.invisibleProfile) return { kind: 'invisible' };
  if (e.target.kind === 'qr') return { kind: 'qr' };
  if (e.target.kind === 'field') return { kind: 'field', index: e.target.index, name: d.spec.fields[e.target.index]?.name ?? '' };
  if (e.origin === 'user' && e.placement) return { kind: 'manual', placement: { ...e.placement }, newPage: e.newPage };
  return null;
}

/** Gemerkten Plan auf den Bearbeitungszustand anwenden (Dokument aus dem Stapel geöffnet). */
function applyPlan(d: MockDoc, e: EditState, plan: BatchPlan, block: { width: number; height: number }): EditState {
  switch (plan.kind) {
    case 'qr':
      return useTarget(d, e, { kind: 'qr' }, block) ?? e;
    case 'field':
      return useTarget(d, e, { kind: 'field', index: plan.index }, block) ?? e;
    case 'invisible':
      return { ...e, invisible: true, revision: e.revision + 1 };
    case 'manual':
      return { ...e, placement: { ...plan.placement, width: block.width, height: block.height }, newPage: plan.newPage, origin: 'user', target: { kind: 'free' }, revision: e.revision + 1 };
  }
}

function startSigning(s: FlowState, ctx: MockCtx, backend: BackendId): void {
  const d = current(s);
  const edit = s.edit;
  if (!d || !edit) return;
  if (!available(s, ctx, backend)) {
    ctx.notify('ng-signer-unavailable');
    return;
  }
  const settings = ctx.settings();
  if (!s.explicit || !s.selected) {
    s.selected = backend;
    s.explicit = true;
  }
  s.last = {
    position: edit.invisible ? 'invisible' : edit.target.kind === 'field' ? 'field' : edit.target.kind === 'qr' ? 'placeholder' : edit.origin === 'auto' ? 'auto' : 'manual',
    backend,
  };
  const session = new SigningSession(
    {
      later: (fn, factor) => ctx.later(fn, factor),
      finished: () => toOutput(s, ctx),
      failed: (code, params, retry) =>
        fail(
          s,
          ctx,
          code,
          params,
          retry,
          'signing',
          () => {
            s.stage = 'signing';
            s.error = null;
            session.retry();
          },
          () => {
            session.close();
            s.signing = null;
            s.error = null;
            s.stage = 'editing';
          },
        ),
      cancelled: () => cancelSigning(s, ctx),
      loggedIn: (remember) => {
        s.remembered = remember;
      },
      passwordCleared: () => {
        s.remembered = false;
      },
    },
    {
      backend,
      doc: { id: d.id, name: d.name },
      flow: s.opts.signingFlow ?? 'app',
      visible: !edit.invisible,
      fido2Available: s.opts.fido2Available ?? true,
      fido2Default: settings.id_austria.fido2_default,
      mobile: ctx.platform().mobile,
      autoComplete: s.opts.autoCompleteApp ?? true,
      remembered: s.remembered,
      prefillUsername: settings.id_austria.username,
      rememberChecked: settings.id_austria.remember,
      keystoreSubject: fileNameOf(settings.keystore.file ?? 'keystore.p12'),
      networkErrorOnce: s.networkErrorOnce,
    },
  );
  s.networkErrorOnce = false;
  s.signing = session;
  s.stage = 'signing';
  s.error = null;
  session.start();
}

function cancelSigning(s: FlowState, ctx: MockCtx): void {
  s.signing?.close();
  s.signing = null;
  // F-72: gemerktes Passwort vergessen, Benutzername bleibt; zurück zum Arbeitsbereich mit unveränderter Platzierung
  s.remembered = false;
  s.stage = s.edit ? 'editing' : 'open';
  ctx.notify('ng-signing-cancelled');
}

function toOutput(s: FlowState, ctx: MockCtx): void {
  const d = current(s);
  if (!d) return;
  const session = s.signing;
  s.signing = null;
  d.signed = true;
  s.stage = 'output';
  s.summary = null;
  const settings = ctx.settings();
  const name = outputFileName(d.path, settings.output.postfix || '_signed');
  const isMobile = ctx.platform().mobile;
  const folder = settings.output.folder ?? (isMobile ? `${SANDBOX}/Documents` : null);
  const backend = session?.backend ?? s.selected ?? 'atrust_native';
  const finish = (path: string) => saved(s, ctx, path, backend);
  if (folder) {
    const target = joinPath(folder === '.' ? folderOf(d.path) : folder, name);
    if ((s.opts.existingFiles ?? []).includes(target)) s.output = { ...EMPTY_OUTPUT, askOverwrite: target };
    else finish(target);
  } else {
    s.output = { ...EMPTY_OUTPUT, saveDialog: { directory: folderOf(d.path), file_name: name } };
  }
}

function saved(s: FlowState, ctx: MockCtx, path: string, backend: BackendId = s.last.backend ?? 'atrust_native'): void {
  const isMobile = ctx.platform().mobile;
  s.output = { ...EMPTY_OUTPUT, path };
  s.stage = 'output';
  const caps = capabilitiesFor(backend, isMobile);
  s.summary = {
    file_name: fileNameOf(path),
    folder_display: isMobile ? null : folderOf(path),
    signer: 'Max Mustermann',
    signing_time: isoWithOffset(new Date()),
    backend,
    qualification: caps.qualification,
    self_check: true,
    page: s.edit && !s.edit.invisible && s.edit.placement ? s.edit.placement.page_index : null,
  };
  ctx.emit({ kind: 'signed', file_name: fileNameOf(path), path });
  const d = current(s);
  if (s.batch && d && s.batch.openItem === d.id) {
    // aus dem Stapel geöffnet: zurück zur Übersicht statt „Fertig“ (§3.6), Hinweis `ng-batch-item-signed`
    d.signed = true;
    s.batch.singleSigned(d.id, path);
    leaveSingle(s);
    s.explicit = false;
    resetSelection(s, ctx);
  }
}

function save(s: FlowState, path: string | null, ctx: MockCtx): void {
  if (s.stage !== 'output') return;
  const d = current(s);
  if (path === null) {
    if (s.output.saveDialog) s.output = { ...EMPTY_OUTPUT, savePending: true };
    else {
      const name = outputFileName(d?.path ?? 'signiert.pdf', ctx.settings().output.postfix || '_signed');
      s.output = { ...EMPTY_OUTPUT, saveDialog: { directory: folderOf(d?.path ?? ''), file_name: name } };
    }
    return;
  }
  if ((s.opts.existingFiles ?? []).includes(path)) {
    s.output = { ...EMPTY_OUTPUT, askOverwrite: path };
    return;
  }
  saved(s, ctx, path);
}

function navigateOpen(s: FlowState, ctx: MockCtx): void {
  if (s.batch?.running) return;
  const d = current(s);
  if (s.batch && d && fromBatch(s) && s.stage !== 'signing') {
    // aus dem Stapel geöffnet (z. B. „Weiteres Dokument“ nach abgebrochenem Speichern-Dialog): wie der Kern
    // (`batch_run::intercept`) zurück zur Übersicht, Eintrag unverändert – der Stapel bleibt
    s.batch.back(d.id, undefined);
    leaveSingle(s);
    s.explicit = false;
    return;
  }
  if (s.batch) {
    // Startseite: Stapel verwerfen (gespeicherte Dokumente bleiben)
    s.batch.dispose();
    s.batch = null;
    s.signing?.close();
    s.signing = null;
    s.queue = [];
    s.edit = null;
    s.error = null;
    s.output = EMPTY_OUTPUT;
    s.summary = null;
    s.explicit = false;
    s.stage = 'open';
    return;
  }
  if (s.stage === 'output') {
    finishDoc(s, s.output.path ? { kind: 'signed', output: { path: s.output.path, len: 1024, sha256: '00' } } : { kind: 'cancelled' });
    s.queue.shift();
    s.output = EMPTY_OUTPUT;
    s.summary = null;
    s.edit = null;
    s.explicit = false;
    if (s.queue.length > 0) analyze(s, ctx);
    else s.stage = 'open';
  } else if (s.stage !== 'open' && s.stage !== 'signing' && s.stage !== 'starting' && s.stage !== 'finished') {
    s.signing?.close();
    s.signing = null;
    s.queue = [];
    s.edit = null;
    s.error = null;
    s.explicit = false;
    s.stage = 'open';
  }
}

/** Platzierung zulässig (z. B. kein neues sichtbares Feld auf DocMDP-zertifizierten Dokumenten). */
function placementAllowed(s: FlowState): boolean {
  const d = current(s);
  const e = s.edit;
  if (!d || !e) return false;
  const c = constraintsFor(d, e);
  return e.invisible || e.target.kind !== 'free' || c.new_visible_field_allowed;
}

function editEvent(s: FlowState, ctx: MockCtx, fn: (d: MockDoc, e: EditState) => EditState | null, rejected?: string): void {
  const d = current(s);
  if (s.stage !== 'editing' || !d || !s.edit) return;
  const next = fn(d, s.edit);
  if (next === null) {
    if (rejected) ctx.notify(rejected);
    return;
  }
  // Prüfung liest das Raster nur, wenn es für die Seite schon berechnet ist
  s.edit = next;
}

function scenarioDocs(o: MockOptions): { path: string; spec: MockDocSpec }[] {
  const dir = '/Users/demo/Dokumente';
  switch (o.scenario) {
    case 'blank':
      return [{ path: `${dir}/Leeres_Dokument.pdf`, spec: blankSpec() }];
    case 'contract':
    case 'error-atrust':
      return [{ path: `${dir}/Verträge/Vertrag.pdf`, spec: contractSpec() }];
    case 'field':
      return [{ path: `${dir}/Förderung/Antrag_Foerderung.pdf`, spec: fieldSpec() }];
    case 'signed':
      return [{ path: `${dir}/Wohnung/Mietvertrag_signiert.pdf`, spec: signedSpec() }];
    case 'batch':
      return [
        { path: `${dir}/Angebote/Angebot_2026-118.pdf`, spec: offerSpec() },
        { path: `${dir}/Aufträge/Auftragsbestaetigung_4711.pdf`, spec: confirmationSpec() },
        { path: `${dir}/Lieferschein_Scan.pdf`, spec: lockedSpec() },
        { path: `${dir}/Verträge/Vertrag.pdf`, spec: contractSpec() },
      ];
    case 'error-password':
      return [{ path: `${dir}/Personal/Gehaltszettel_2026-09.pdf`, spec: lockedSpec() }];
    default:
      return [];
  }
}

function bar(s: FlowState): MainBarView {
  switch (s.stage) {
    case 'starting':
    case 'finished':
      return mainBar(['', '', '', ''], false);
    case 'open':
    case 'loading':
    case 'warning':
    case 'error':
      return mainBar(['AE', '', '', ''], true);
    case 'editing':
      return mainBar(['AE', 'A', '', ''], true);
    case 'choosing':
      return mainBar(['AE', 'AE', 'A', ''], true);
    case 'signing':
      return mainBar(['A', 'A', 'A', ''], false);
    case 'output':
      return mainBar(['AE', 'AE', 'AE', 'A'], true);
    case 'batch':
      return s.batch?.running ? mainBar(['A', 'A', 'A', ''], false) : mainBar(['AE', '', '', ''], true);
  }
}

function stepView(s: FlowState, ctx: MockCtx): StepView {
  const d = current(s);
  const queue = s.queue.map((q, i) => ({ id: q.id, name: q.name, current: i === 0 }));
  switch (s.stage) {
    case 'starting':
      return { step: 'starting' };
    case 'open':
      return { step: 'open', queue };
    case 'loading':
      return { step: 'loading', queue };
    case 'warning':
      return d ? { step: 'warning', doc: docInfo(d), codes: [...(s.opts.warnings ?? [])] } : { step: 'open', queue };
    case 'editing': {
      if (!d || !s.edit?.placement) return { step: 'open', queue };
      const p = s.edit.placement;
      const size = blockSize(ctx.settings(), blockLang(ctx));
      return {
        step: 'position',
        doc: docInfo(d),
        page: p.page_index,
        block: { width: size.width, height: size.height, preview_revision: s.preview },
        placement: { ...p },
        new_page: s.edit.newPage,
        announce: {
          page: String(p.page_index + 1),
          pages: String(d.spec.pages.length + (s.edit.newPage ? 1 : 0)),
          left: String(Math.round((p.x * 25.4) / 72)),
          top: String(Math.round((p.y * 25.4) / 72)),
        },
      };
    }
    case 'choosing':
      return {
        step: 'backend',
        options: signerOptions(s, ctx).map((o) => ({ id: o.backend, available: o.available, capabilities: o.capabilities, notes: o.notes })),
      };
    case 'signing': {
      const session = s.signing;
      const leaves = session ? capabilitiesFor(session.backend).document_leaves_device : false;
      return {
        step: 'signing',
        interaction: session ? session.interaction : { kind: 'waiting', progress: 'Connecting' },
        queue,
        document_leaves_device: leaves,
        progress: session ? session.progress() : defaultSigningProgress(),
      };
    }
    case 'output': {
      const next = s.queue[1];
      return {
        step: 'output',
        result: { path: s.output.path, save_pending: s.output.savePending, ask_overwrite: s.output.askOverwrite, save_dialog: s.output.saveDialog ? { ...s.output.saveDialog } : null, summary: s.summary ? { ...s.summary } : null },
        next: next ? docInfo(next) : null,
        pending: Math.max(0, s.queue.length - 1),
      };
    }
    case 'error': {
      const e = s.error;
      return {
        step: 'error',
        code: e?.code ?? 'error-Unexpected',
        params: { ...(e?.params ?? {}) },
        retry: e?.retry ?? false,
        context: e ? { origin: e.origin, occurred_at: e.occurredAt, backend: e.backend, doc: d ? { id: d.id, name: d.name } : null } : defaultErrorContext(),
      };
    }
    case 'finished':
      return { step: 'finished', exit_code: s.exitCode };
    case 'batch': {
      const b = s.batch;
      if (!b?.running) return { step: 'batch' };
      const backend = b.backend();
      return {
        step: 'signing',
        interaction: b.interaction(),
        queue: b.docs().map((q, i) => ({ id: q.id, name: q.name, current: i === 0 })),
        document_leaves_device: backend ? capabilitiesFor(backend).document_leaves_device : false,
        progress: b.progress(),
      };
    }
  }
}

function actionsView(s: FlowState, b: MainBarView): ActionsView {
  const editing = s.stage === 'editing';
  if (s.stage === 'batch' && s.batch) {
    const running = s.batch.running;
    return { ...noActions(), open_files: !running, start_batch: !running && s.batch.signable().length > 0, settings: !running && b.settings_enabled, home: !running };
  }
  return {
    ...noActions(),
    open_files: !['starting', 'finished', 'signing'].includes(s.stage),
    sign: editing && placementAllowed(s),
    edit_placement: editing,
    // weitere Dokumente in der Warteschlange (Einzeldokument, nicht aus dem Stapel geöffnet)
    show_batch: batchOn(s) && !s.batch && (editing || s.stage === 'warning') && s.queue.length > 1,
    settings: b.settings_enabled,
    home: b.steps[0]?.enabled ?? false,
  };
}

const flowArea: MockArea<FlowState> = {
  init(o) {
    const s: FlowState = {
      opts: o,
      stage: 'starting',
      queue: [],
      nextDocId: 1,
      edit: null,
      preview: 1,
      signing: null,
      selected: null,
      explicit: false,
      remembered: false,
      output: EMPTY_OUTPUT,
      summary: null,
      error: null,
      done: [],
      resume: o.resumeOffer ?? null,
      exitCode: 0,
      networkErrorOnce: o.scenario === 'error-atrust',
      last: { position: 'manual', backend: null },
      batch: null,
      pendingPlan: null,
    };
    return s;
  },

  handle(e, s, ctx) {
    const d = current(s);
    const settings = ctx.settings();
    const lang = blockLang(ctx);
    const b = s.batch;
    // Stapellauf: Masken, Pausen, Überspringen, Abbruch (§3.8); `Cancel` im Lauf = `AbortBatch`
    if (s.stage === 'batch' && b?.running) {
      switch (e.type) {
        case 'submit_form':
          b.session()?.submitForm(e.request, e.answer);
          return true;
        case 'submit_secret':
          b.session()?.submitSecret(e.request, e.secret);
          return true;
        case 'answer_notice':
          b.session()?.answerNotice(e.request, e.answer);
          return true;
        case 'retry':
          b.retry();
          return true;
        case 'skip_batch_document':
          b.skip();
          return true;
        case 'abort_batch':
        case 'cancel':
          b.abort();
          return true;
        case 'quit':
          b.dispose();
          s.stage = 'finished';
          return true;
        case 'open_files':
          openFiles(s, ctx, e.paths);
          return true;
        case 'select_signer':
        case 'dismiss_notice':
          break;
        default:
          return e.type === 'open_signature_data';
      }
    }
    switch (e.type) {
      case 'open_files':
        openFiles(s, ctx, e.paths);
        return true;
      case 'show_batch':
        if ((s.stage === 'editing' || s.stage === 'warning') && !b && batchOn(s) && d && s.queue.length > 1) {
          const [cur, ...rest] = s.queue;
          const batch = new MockBatch(batchHost(s, ctx));
          if (cur && s.edit && s.stage === 'editing') batch.addEdited(cur, planFromEdit(cur, s.edit));
          batch.add(s.stage === 'editing' ? rest : s.queue);
          s.batch = batch;
          leaveSingle(s);
        }
        return true;
      case 'open_batch_item':
        if (s.stage === 'batch' && b) b.open(e.doc);
        return true;
      case 'back_to_batch':
        if (b && d && fromBatch(s) && (s.stage === 'editing' || s.stage === 'warning' || s.stage === 'loading')) {
          b.back(d.id, s.stage === 'editing' && s.edit ? planFromEdit(d, s.edit) : undefined);
          leaveSingle(s);
        }
        return true;
      case 'remove_from_batch':
        if (s.stage === 'batch' && b) {
          b.remove(e.doc);
          if (b.size === 0) {
            s.batch = null;
            s.stage = 'open';
          }
        }
        return true;
      case 'start_batch':
        if (s.stage === 'batch' && b) b.start(e);
        return true;
      case 'skip_batch_document':
      case 'abort_batch':
        return true;
      case 'answer_warning':
        if (s.stage !== 'warning') return true;
        if (e.proceed) s.stage = 'editing';
        else if (b && d && fromBatch(s)) {
          // „Dokument verwerfen“ aus dem Stapel: Eintrag entfernen, zurück zur Übersicht
          b.back(d.id, undefined);
          b.remove(d.id);
          leaveSingle(s);
          if (b.size === 0) {
            s.batch = null;
            s.stage = 'open';
          }
        } else {
          finishDoc(s, { kind: 'cancelled' });
          dropCurrent(s, ctx);
        }
        return true;
      case 'move_block':
        editEvent(s, ctx, (doc, ed) => moveBlock(doc, ed, e.page, e.x, e.y));
        return true;
      case 'nudge_block':
        editEvent(s, ctx, (doc, ed) => nudgeBlock(doc, ed, e.dx, e.dy, e.fine, e.coarse));
        return true;
      case 'change_page':
        editEvent(s, ctx, (doc, ed) => changePage(doc, ed, e.page));
        return true;
      case 'toggle_new_page':
        editEvent(s, ctx, (doc, ed) => toggleNewPage(doc, ed), 'placement.notice.new_page_not_allowed');
        return true;
      case 'auto_place':
        editEvent(s, ctx, (doc, ed) => {
          const next = autoPlace(doc, ed, blockSize(settings, lang));
          if (next.placement && !next.ready.includes(next.placement.page_index)) next.ready = [...next.ready, next.placement.page_index];
          return next;
        });
        return true;
      case 'set_invisible':
        editEvent(s, ctx, (_doc, ed) => (ed.invisibleProfile && !e.invisible ? null : { ...ed, invisible: e.invisible, revision: ed.revision + 1 }), 'ng-constraint-invisible-profile');
        return true;
      case 'use_target':
        editEvent(s, ctx, (doc, ed) => useTarget(doc, ed, e.target, blockSize(settings, lang)), 'ng-document-rejected');
        return true;
      case 'inspect_pages': {
        if (!s.edit || !d) return true;
        const pages = [...new Set(e.pages.filter((p) => Number.isInteger(p) && p >= 0 && p < d.spec.pages.length))].slice(0, 8);
        s.edit = { ...s.edit, inspected: pages };
        // Raster asynchron (niedrige Priorität), danach neue Projektion mit derselben Revision
        ctx.later(() => {
          if (s.edit && current(s) === d) s.edit = { ...s.edit, ready: [...new Set([...s.edit.ready, ...pages])] };
        }, 0.5);
        return true;
      }
      case 'select_signer':
        s.selected = e.backend && available(s, ctx, e.backend) ? e.backend : null;
        s.explicit = e.backend !== null;
        return true;
      case 'sign_with':
        if (s.stage === 'choosing') startSigning(s, ctx, e.backend);
        else if (s.stage === 'editing' && placementAllowed(s)) startSigning(s, ctx, e.backend);
        return true;
      case 'confirm_position':
        // Alias im Arbeitsbereich-Modus (§3.2): nur mit gewähltem Signiermittel
        if (s.stage === 'editing' && s.selected) startSigning(s, ctx, s.selected);
        return true;
      case 'choose_invisible':
        if (s.stage === 'editing' && s.edit && s.selected) {
          s.edit = { ...s.edit, invisible: true, revision: s.edit.revision + 1 };
          startSigning(s, ctx, s.selected);
        }
        return true;
      case 'choose_backend':
        if (s.stage === 'choosing') startSigning(s, ctx, e.backend);
        return true;
      case 'submit_form':
        s.signing?.submitForm(e.request, e.answer);
        return true;
      case 'submit_secret':
        s.signing?.submitSecret(e.request, e.secret);
        return true;
      case 'answer_notice':
        s.signing?.answerNotice(e.request, e.answer);
        return true;
      case 'open_signature_data':
        return true;
      case 'cancel':
        if (b && fromBatch(s) && (s.stage === 'loading' || s.stage === 'warning' || s.stage === 'editing')) {
          // aus dem Stapel geöffnet: zurück zur Übersicht, Status unverändert
          if (d) b.back(d.id, undefined);
          leaveSingle(s);
        } else if (s.stage === 'signing') cancelSigning(s, ctx);
        else if (s.stage === 'choosing') {
          s.stage = s.edit ? 'editing' : 'open';
          ctx.notify('ng-signing-cancelled');
        } else if (s.stage === 'error') {
          const cancel = s.error?.onCancel;
          s.error = null;
          cancel?.();
        } else if (s.stage === 'loading' || s.stage === 'warning' || s.stage === 'editing') {
          finishDoc(s, { kind: 'cancelled' });
          dropCurrent(s, ctx);
        }
        return true;
      case 'retry':
        if (s.stage === 'error') {
          const retry = s.error?.onRetry;
          if (s.error?.origin !== 'signing') s.error = null;
          retry?.();
        }
        return true;
      case 'save':
        save(s, e.path, ctx);
        return true;
      case 'confirm_overwrite':
        if (!s.output.askOverwrite) return true;
        if (e.overwrite) saved(s, ctx, s.output.askOverwrite);
        else s.output = { ...EMPTY_OUTPUT, savePending: true };
        return true;
      case 'navigate':
        if (e.step === 'Open') navigateOpen(s, ctx);
        return true;
      case 'resume_signature': {
        const offer = s.resume;
        if (!offer || s.stage !== 'open') return true;
        s.resume = null;
        const doc = makeDoc(s, `${SANDBOX}/Library/tmp/resume/document/${offer.name}`, contractSpec());
        s.queue.unshift(doc);
        s.edit = startEdit(doc, settings, lang);
        if (available(s, ctx, offer.backend)) startSigning(s, ctx, offer.backend);
        else s.stage = 'choosing';
        return true;
      }
      case 'discard_resume':
        s.resume = null;
        return true;
      case 'quit':
        s.signing?.close();
        s.signing = null;
        s.batch?.dispose();
        s.stage = 'finished';
        s.exitCode = 0;
        return true;
      default:
        return false;
    }
  },

  signal(sig, s, ctx) {
    if (sig.kind === 'settings-changed') {
      s.preview += 1;
      const d = current(s);
      if (d && s.edit && (s.stage === 'editing' || s.stage === 'warning')) {
        s.edit = resizeBlock(d, s.edit, sig.settings, sig.settings.signature.block_language ?? sig.settings.general.ui_language ?? 'de');
      }
      if (!s.explicit) resetSelection(s, ctx);
      s.batch?.settingsChanged();
    } else if (sig.kind === 'open-files') {
      openFiles(s, ctx, sig.paths);
    }
  },

  project(s, ctx) {
    const d = current(s);
    const b = bar(s);
    const lang = blockLang(ctx);
    const phase = s.stage === 'warning' ? 'warning' : s.stage === 'editing' ? 'editing' : s.stage === 'signing' || s.stage === 'choosing' ? 'signing' : s.stage === 'output' ? 'done' : s.stage === 'error' && s.error?.origin === 'signing' ? 'signing' : null;
    const options = signerOptions(s, ctx);
    return {
      step: stepView(s, ctx),
      main_bar: b,
      done: s.done.map((x) => structuredClone(x)),
      resume: s.stage === 'open' && s.resume ? { ...s.resume } : null,
      document: d && s.edit && phase ? projectDocument(d, s.edit, phase, ctx.settings(), lang, s.preview) : null,
      signers: { ...defaultSigners(), options, selected: s.selected, default: backendFor(ctx.settings().backend.default) },
      batch: s.batch ? s.batch.view() : null,
      actions: actionsView(s, b),
    };
  },
};

/* ================================================================ MockCore */

interface Area {
  area: MockArea<unknown>;
  state: unknown;
}

/** Mock-Kern samt nativer Dienste. */
export class MockCore implements Bridge {
  readonly kind = 'mock' as const;

  /** alle gesendeten Ereignisse (Tests) */
  readonly dispatched: UiEvent[] = [];
  /** im Systembrowser geöffnete URLs (Tests) */
  readonly openedUrls: string[] = [];
  /** Aufrufe nativer Dienste (Tests) */
  readonly nativeCalls: string[] = [];

  private readonly opts: MockOptions;
  private readonly listeners = new Set<(v: ViewState) => void>();
  private readonly dragListeners = new Set<(e: FileDragEvent) => void>();
  private forced: ViewState | null = null;
  private readonly timers = new Set<ReturnType<typeof setTimeout>>();
  private readonly flow: FlowState;
  private readonly areas: Area[];
  private notices: NoticeView[] = [];
  private noticeId = 0;
  private readonly ctx: MockCtx;

  constructor(options: MockOptions = {}) {
    this.opts = { ...options };
    this.flow = flowArea.init(this.opts);
    this.areas = [
      { area: flowArea as MockArea<unknown>, state: this.flow },
      { area: settingsArea as MockArea<unknown>, state: settingsArea.init(this.opts) },
      { area: verifyArea as MockArea<unknown>, state: verifyArea.init(this.opts) },
      { area: recentArea as MockArea<unknown>, state: recentArea.init(this.opts) },
      { area: firstRunArea as MockArea<unknown>, state: firstRunArea.init(this.opts) },
    ];
    const settingsState = this.areas[1]?.state as ReturnType<typeof settingsArea.init>;
    this.ctx = {
      emit: (signal: MockSignal) => {
        for (const a of this.areas) a.area.signal?.(signal, a.state, this.ctx);
      },
      settings: () => settingsState.settings,
      platform: () => this.platformView(),
      notify: (code, params = {}) => this.notify(code, params),
      later: (fn, factor = 1) => this.later(fn, factor),
    };
    if (options.initialView) {
      this.forced = options.initialView;
    } else {
      this.later(() => {
        this.flow.stage = 'open';
        const docs = scenarioDocs(this.opts).map(({ path, spec }) => makeDoc(this.flow, path, spec));
        if (docs.length >= 2 && batchOn(this.flow)) enterBatch(this.flow, this.ctx, docs);
        else if (docs.length > 0) {
          this.flow.queue.push(...docs);
          analyze(this.flow, this.ctx);
        }
      });
    }
  }

  // ------------------------------------------------------------------------------------------
  // CoreBridge
  // ------------------------------------------------------------------------------------------

  async view(): Promise<ViewState> {
    return this.current();
  }

  onState(cb: (v: ViewState) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  private doc(doc: DocId): MockDoc | undefined {
    return this.flow.queue.find((q) => q.id === doc) ?? this.flow.batch?.docs().find((q) => q.id === doc);
  }

  pageTileUrl(doc: DocId, page: number, scale: number): string {
    const d = this.doc(doc);
    const p = d?.spec.pages[page] ?? null;
    const last = d?.spec.pages[d.spec.pages.length - 1];
    const size: [number, number] = p ? [p.w, p.h] : last ? [last.w, last.h] : A4;
    return pageTileSvg(p, size, scale);
  }

  /** Miniatur der ersten Seite (Voranalyse des Stapels, `pida-render://thumb/<doc>`; Breite 96 px). */
  thumbUrl(doc: DocId): string {
    const d = this.doc(doc);
    const p = d?.spec.pages[0] ?? null;
    const size: [number, number] = p ? [p.w, p.h] : A4;
    return pageTileSvg(p, size, 96 / size[0]);
  }

  blockPreviewUrl(scale: number, revision: number): string {
    const settings = this.ctx.settings();
    return blockPreviewSvg(settings, settings.signature.block_language ?? settings.general.ui_language ?? this.opts.language ?? 'de', scale, revision);
  }

  async dispatch(event: UiEvent): Promise<void> {
    this.dispatched.push(event);
    if (this.forced) return; // feste Testzustände reagieren nicht; Tests prüfen `dispatched`
    if (event.type === 'dismiss_notice') this.notices = this.notices.filter((n) => n.id !== event.id);
    for (const a of this.areas) a.area.handle(event, a.state, this.ctx);
    const session = this.flow.signing ?? this.flow.batch?.session() ?? null;
    if (event.type === 'open_signature_data' && session) {
      const url = session.signatureDataUrl(event.request);
      if (url) this.openedUrls.push(url);
    }
    if (event.type === 'open_result' && this.flow.output.path) this.nativeCalls.push(`openResult:${this.flow.output.path}`);
    if (event.type === 'share_result' && this.flow.output.path) this.nativeCalls.push(`shareResult:${this.flow.output.path}`);
    if (event.type === 'export_result' && this.flow.output.path) {
      this.nativeCalls.push(`exportResult:${this.flow.output.path}`);
      this.notify('ng-export-done', { name: fileNameOf(this.flow.output.path) });
    }
    this.emit();
  }

  // ------------------------------------------------------------------------------------------
  // PlatformServices
  // ------------------------------------------------------------------------------------------

  async pickFiles(options: { title: string; multiple: boolean }): Promise<string[] | null> {
    this.nativeCalls.push(`pickFiles:${options.multiple ? 'multiple' : 'single'}`);
    const result = this.opts.pickFilesResult === undefined ? ['/Users/demo/Dokumente/Vertrag.pdf', '/Users/demo/Dokumente/Angebot 2026.pdf'] : this.opts.pickFilesResult;
    if (result === null) return null;
    return options.multiple ? [...result] : result.slice(0, 1);
  }

  async pickSavePath(options: { defaultPath?: string | null }): Promise<string | null> {
    this.nativeCalls.push(`pickSavePath:${options.defaultPath ?? ''}`);
    const r = this.opts.pickSaveResult === undefined ? '' : this.opts.pickSaveResult;
    if (r === null) return null;
    return r || options.defaultPath || '/Users/demo/Dokumente/signiert.pdf';
  }

  async pickDirectory(): Promise<string | null> {
    this.nativeCalls.push('pickDirectory');
    return this.opts.pickDirectoryResult === undefined ? '/Users/demo/Signiert' : this.opts.pickDirectoryResult;
  }

  async pickMobileFiles(options: { kind: 'pdf' | 'pkcs12'; multiple: boolean }): Promise<string[]> {
    this.nativeCalls.push(`pickMobileFiles:${options.kind}:${options.multiple ? 'multiple' : 'single'}`);
    if (!mobile(this.opts)) throw new Error('ng-mobile-unsupported');
    const sandbox = `${SANDBOX}/Library/Caches/import/1`;
    const result = this.opts.pickMobileFilesResult ?? (options.kind === 'pdf' ? [`${sandbox}/Vertrag.pdf`, `${sandbox}/Angebot.pdf`] : [`${sandbox}/test.p12`]);
    return options.multiple ? [...result] : result.slice(0, 1);
  }

  private platformView(): PlatformView {
    switch (this.opts.platform) {
      case 'ios':
        return { os: 'ios', mobile: true };
      case 'android':
        return { os: 'android', mobile: true };
      default:
        return { os: 'macos', mobile: false };
    }
  }

  onFileDrag(cb: (e: FileDragEvent) => void): () => void {
    this.dragListeners.add(cb);
    return () => this.dragListeners.delete(cb);
  }

  /** Simuliert Drag & Drop über das Fenster (Tests, Browser-Vorschau). */
  simulateFileDrag(event: FileDragEvent): void {
    for (const cb of this.dragListeners) cb(event);
  }

  async openUrl(url: string): Promise<void> {
    this.openedUrls.push(url);
  }

  async revealOutput(doc?: DocId | null): Promise<void> {
    this.nativeCalls.push(doc === undefined || doc === null ? 'revealOutput' : `revealOutput:${doc}`);
  }

  async openLogDir(): Promise<void> {
    this.nativeCalls.push('openLogDir');
  }

  async openNotices(): Promise<void> {
    this.nativeCalls.push('openNotices');
  }

  async appInfo() {
    return { version: '0.1.0', platform: mobile(this.opts) ? (this.opts.platform ?? 'mock') : 'mock', core_available: true };
  }

  // ------------------------------------------------------------------------------------------
  // Steuerung für Tests
  // ------------------------------------------------------------------------------------------

  /** Setzt einen festen Zustand (Ereignisse werden nur protokolliert). */
  setView(view: ViewState): void {
    this.forced = view;
    this.emit();
  }

  /** Long-Poll meldet „bestätigt“ (App/QR) bzw. die Chipkarten-Software ist fertig. */
  completeConfirmation(): void {
    if (this.flow.stage === 'signing' && this.flow.signing?.completeConfirmation()) this.emit();
    else if (this.flow.stage === 'batch' && this.flow.batch?.session()?.completeConfirmation()) this.emit();
  }

  /** Gewählte Position und Signiermittel des letzten Signiervorgangs (Tests). */
  get choices(): { position: string; backend: BackendId | null } {
    return { ...this.flow.last };
  }

  /** Alle Zeitgeber stoppen. */
  dispose(): void {
    this.flow.batch?.dispose();
    for (const t of this.timers) clearTimeout(t);
    this.timers.clear();
    this.listeners.clear();
    this.dragListeners.clear();
  }

  // ------------------------------------------------------------------------------------------
  // Zustand → ViewState
  // ------------------------------------------------------------------------------------------

  private current(): ViewState {
    if (this.forced) return this.forced;
    const settings = this.ctx.settings();
    const base: ViewState = {
      step: { step: 'starting' },
      main_bar: mainBar(['', '', '', ''], false),
      settings: null,
      language: settings.general.ui_language ?? this.opts.language ?? 'de',
      notices: this.notices.map((n) => ({ ...n, params: { ...n.params } })),
      migration: null,
      done: [],
      platform: this.platformView(),
      resume: null,
      keystore_import: null,
      privacy_text: this.opts.privacyText ?? null,
      ...workspaceDefaults(),
    };
    const view = this.areas.reduce<ViewState>((v, a) => ({ ...v, ...a.area.project(a.state, this.ctx) }), base);
    // Einstellungen offen: Einstellungen nicht erneut erreichbar (wie `main_bar.settings_enabled` im Kern)
    if (view.settings) {
      view.main_bar = { ...view.main_bar, settings_enabled: false };
      view.actions = { ...view.actions, settings: false };
    }
    return view;
  }

  private emit(): void {
    const view = this.current();
    for (const cb of this.listeners) cb(view);
  }

  private later(fn: () => void, factor = 1): void {
    const t = setTimeout(
      () => {
        this.timers.delete(t);
        fn();
        this.emit();
      },
      (this.opts.delayMs ?? 350) * factor,
    );
    this.timers.add(t);
  }

  private notify(code: string, params: Record<string, string> = {}): void {
    // wie der Kern: gleiche Meldung nicht doppelt stapeln
    if (this.notices.some((n) => n.code === code && JSON.stringify(n.params) === JSON.stringify(params))) return;
    this.noticeId += 1;
    this.notices.push({ id: this.noticeId, code, params });
  }
}

/** Mock-Kern als Bridge. */
export function createMockBridge(options?: MockOptions): MockCore {
  return new MockCore(options);
}
