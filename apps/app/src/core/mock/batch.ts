/**
 * Mock-Kern, Bereich Stapel (UI-A, ADR 0004 §3.8, Abnahme E2): Stapelübersicht mit Voranalyse (Seiten,
 * Größe, Miniatur, Status), Einzeldokument öffnen und „Zurück zum Stapel“ (die Platzierung wird als Plan
 * gemerkt), Lauf mit **einer** Sitzung – eine Anmeldung, je Dokument eine eigene Bestätigung mit eigenem
 * Vergleichswert –, „Dokument überspringen“ ≠ „Stapel abbrechen“, Pausen (Dokument, Verbindung, Sitzung),
 * vorhandene Zieldateien (Überschreiben, Beide behalten `… (2).pdf`, Überspringen ohne Bestätigung),
 * Ordnerwahl vor dem Start und die Zusammenfassung über **alle** Einträge.
 *
 * Der Zustand gehört diesem Modul (`MockBatch`). Der Bereich `flow` (`index.ts`, Kern und Arbeitsbereich)
 * hält ihn wie der Kern `State::BatchOverview`/`BatchSigning` neben der Warteschlange, leitet die
 * Stapelereignisse weiter und stellt über `BatchHost` das Einzeldokument und die Signiermittel bereit; die
 * Sitzung je Dokument ist die Signiersitzung aus `signing.ts` mit den Daten der Stapelsitzung (Anmeldung,
 * Zertifikat). Wie überall im Mock: Regeln nur so weit, wie Bedienung und Tests sie brauchen; keine echten
 * Zugangsdaten, kein Netz. Invariante A1: Eine Sitzung beginnt nur nach `start_batch` bzw. „Erneut
 * versuchen“ einer Pause.
 */
import type {
  BackendId,
  BatchItemStatusView,
  BatchItemView,
  BatchOutputView,
  BatchPauseView,
  BatchSummaryView,
  BatchView,
  BatchEstimateView,
  DocId,
  ExistingPolicy,
  InteractionView,
  PauseScope,
  Placement,
  PositionSummaryView,
  SigningProgressView,
  UiEvent,
} from '../types';
import { fileNameOf, folderOf, joinPath, outputFileName, SANDBOX, type MockCtx, type MockOptions } from './shared';
import { SigningSession, type SigningHost } from './signing';
import type { MockDoc } from './workspace';

/** Gemerkte Platzierung eines Stapeleintrags (`BatchItem::plan`; `null` = automatisch). */
export type BatchPlan =
  | { kind: 'qr' }
  | { kind: 'field'; index: number; name: string }
  | { kind: 'manual'; placement: Placement; newPage: boolean }
  | { kind: 'invisible' };

/** Was der Stapel vom umgebenden Ablauf braucht. */
export interface BatchHost {
  readonly ctx: MockCtx;
  readonly opts: MockOptions;
  /** Signiermittel verfügbar (`signers`) */
  available(backend: BackendId): boolean;
  /** Auswahl des Konto-Chips */
  selected(): BackendId | null;
  /** Passwort im „Merken“-Speicher des Backends (ID Austria) */
  remembered(): boolean;
  setRemembered(value: boolean): void;
  /** Dokument aus der Übersicht im Arbeitsbereich öffnen (`return_to_batch`) */
  openSingle(doc: MockDoc, plan: BatchPlan | null): void;
  /** Lauf beendet (Auswahl „Jedes Mal fragen“ gilt nur bis zum Ende des Laufs, §8 F12) */
  runEnded(): void;
}

interface Entry {
  doc: MockDoc;
  status: BatchItemStatusView;
  /** Status vor dem Lauf (Abbruch: wartende bleiben „Bereit“ bzw. „Positioniert“) */
  before: BatchItemStatusView | null;
  plan: BatchPlan | null;
  /** Plan stammt aus der Voranalyse (QR-Platzhalter) – geänderte Einstellungen bestimmen ihn neu */
  autoPlan: boolean;
  analyzed: boolean;
}

interface Run {
  backend: BackendId;
  order: DocId[];
  /** Position im Lauf (0-basiert) */
  index: number;
  outputDir: string | null;
  onExisting: ExistingPolicy;
  /** Zugangsdaten in der Sitzung (ID Austria) bzw. Keystore entsperrt */
  loggedIn: boolean;
  certificateRead: boolean;
  /** Sitzung offen (weitere Dokumente ohne Verbindungsaufbau) */
  sessionOpen: boolean;
  session: SigningSession | null;
  pause: BatchPauseView | null;
  confirmations: number;
  signedInRun: number;
  /** Störung je Dokument schon ausgelöst (nur der erste Versuch scheitert) */
  faulted: Set<DocId>;
  firstAttempt: boolean;
}

/** Vergleichswerte je Dokument (frei erfunden, je Dokument ein eigener). */
const REFERENCE_VALUES = ['Xk7Pq', 'R4mZt', 'b9QwL', 'T2nHs', 'pE6Vd', 'Kz3Ya', 'w8GcN', 'Hq5Uj'];

export function referenceValueFor(doc: DocId): string {
  return REFERENCE_VALUES[(doc - 1 + REFERENCE_VALUES.length) % REFERENCE_VALUES.length] ?? 'Xk7Pq';
}

/** `Vertrag_signed.pdf` → `Vertrag_signed (2).pdf` (erste freie Nummer ab 2, §3.8). */
export function numberedName(name: string, n: number): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? `${name.slice(0, dot)} (${n})${name.slice(dot)}` : `${name} (${n})`;
}

function isAbsolute(path: string): boolean {
  return path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path) || path.startsWith('\\\\');
}

const FAULTS = {
  network: { code: 'backend-network', params: { detail: 'service.a-trust.at:443 – Zeitüberschreitung nach 30 s' }, retry: true },
  session: { code: 'backend-unknown-page', params: { diagnostic: 'Seite „/mobile/Unbekannt.aspx“' }, retry: true },
  'too-large': { code: 'backend-document-too-large', params: {}, retry: false },
} as const;

/** Wirkungsbereich eines Fehlers im Lauf (wie `BackendError::scope`, §3.8). */
function scopeOf(code: string, retry: boolean): { pause: PauseScope | null; newSession: boolean } {
  switch (code) {
    case 'ng-signing-declined':
      return { pause: 'document', newSession: false };
    case 'backend-network':
    case 'backend-timeout':
    case 'backend-http':
      return { pause: 'session', newSession: false };
    case 'backend-document-too-large':
      return { pause: null, newSession: false };
    case 'backend-security-layer':
    case 'ng-cms-too-large':
    case 'output-save_failed':
      return { pause: 'document', newSession: false };
    default:
      return retry ? { pause: 'session', newSession: true } : { pause: null, newSession: false };
  }
}

/** Stapel des Mock-Kerns (Übersicht und Lauf). */
export class MockBatch {
  private entries: Entry[] = [];
  /** gerade im Arbeitsbereich geöffnetes Dokument (`OpenBatchItem`) */
  openItem: DocId | null = null;
  private run: Run | null = null;
  /** nach einem Lauf: Zusammenfassung zeigen */
  private lastRun = false;
  private lastRunFolder: string | null = null;
  /** in dieser Programmsitzung geschriebene Dateien (Vorabprüfung „existiert“) */
  private readonly written: string[] = [];
  private analyzing = false;

  constructor(private readonly host: BatchHost) {}

  private get ctx(): MockCtx {
    return this.host.ctx;
  }

  get running(): boolean {
    return this.run !== null;
  }

  get size(): number {
    return this.entries.length;
  }

  has(path: string): boolean {
    return this.entries.some((e) => e.doc.path === path);
  }

  docs(): MockDoc[] {
    return this.entries.map((e) => e.doc);
  }

  private find(id: DocId): Entry | undefined {
    return this.entries.find((e) => e.doc.id === id);
  }

  /** signierbare Einträge (`Ready`/`Positioned`) in der Reihenfolge der Übersicht */
  signable(): Entry[] {
    return this.entries.filter((e) => e.status.state === 'ready' || e.status.state === 'positioned');
  }

  /* ---------------------------------------------------------------- Übersicht */

  /** Dokumente hinzufügen; die Voranalyse läuft nacheinander im Hintergrund. */
  add(docs: MockDoc[]): void {
    for (const doc of docs) {
      if (this.has(doc.path)) continue;
      this.entries.push({ doc, status: { state: 'analyzing' }, before: null, plan: null, autoPlan: false, analyzed: false });
    }
    this.preanalyzeNext();
  }

  /** Aktuelles Dokument aus dem Arbeitsbereich übernehmen (`ShowBatch`: Platzierung als Plan). */
  addEdited(doc: MockDoc, plan: BatchPlan | null): void {
    this.entries.push({ doc, status: { state: 'positioned' }, before: null, plan, autoPlan: false, analyzed: true });
  }

  private preanalyzeNext(): void {
    if (this.analyzing) return;
    const next = this.entries.find((e) => e.status.state === 'analyzing');
    if (!next) return;
    this.analyzing = true;
    this.ctx.later(() => {
      this.analyzing = false;
      if (this.entries.includes(next) && next.status.state === 'analyzing') this.classify(next);
      this.preanalyzeNext();
    }, 0.6);
  }

  private qrAuto(doc: MockDoc): boolean {
    const p = this.ctx.settings().placeholder;
    return !!doc.spec.qr && p.search && p.kind === 'marker';
  }

  /** Ergebnis der Voranalyse (`Preanalyzed`): abgelehnt, Analysewarnung, QR → positioniert, sonst bereit. */
  private classify(e: Entry): void {
    e.analyzed = true;
    const warnings = this.host.opts.warnings ?? [];
    if (!/\.pdf$/i.test(e.doc.name)) e.status = { state: 'rejected', code: 'error-MayNotBeAPDF' };
    else if (e.doc.spec.locked) e.status = { state: 'rejected', code: 'error-PDFPwdProtected' };
    else if (warnings.length > 0) e.status = { state: 'needs_review', codes: [...warnings] };
    else if (this.qrAuto(e.doc)) {
      e.status = { state: 'positioned' };
      e.plan = { kind: 'qr' };
      e.autoPlan = true;
    } else e.status = { state: 'ready' };
  }

  /** Einstellungen geändert: automatisch bestimmte Pläne (QR) neu bestimmen, gemerkte bleiben. */
  settingsChanged(): void {
    if (this.run) return;
    for (const e of this.entries) {
      if (!e.analyzed || (e.status.state !== 'ready' && e.status.state !== 'positioned')) continue;
      if (e.plan !== null && !e.autoPlan) continue;
      if (this.qrAuto(e.doc)) {
        e.status = { state: 'positioned' };
        e.plan = { kind: 'qr' };
        e.autoPlan = true;
      } else if (e.autoPlan) {
        e.status = { state: 'ready' };
        e.plan = null;
        e.autoPlan = false;
      }
    }
  }

  /** `OpenBatchItem`: Dokument im Arbeitsbereich öffnen (nur Bereit, Positioniert, Analysewarnung). */
  open(id: DocId): boolean {
    const e = this.find(id);
    if (this.run || !e || !['ready', 'positioned', 'needs_review'].includes(e.status.state)) return false;
    this.openItem = id;
    this.host.openSingle(e.doc, e.plan);
    return true;
  }

  /**
   * `BackToBatch`: Platzierung als Plan merken (`origin == Auto` → automatisch) und „Positioniert“. Eine
   * noch nicht bestätigte Analysewarnung bleibt „Prüfen“.
   */
  back(id: DocId, plan: BatchPlan | null | undefined): void {
    const e = this.find(id);
    this.openItem = null;
    if (!e) return;
    if (plan === undefined) return; // Warnung nicht bestätigt bzw. Abbruch: Status unverändert
    e.plan = plan;
    e.autoPlan = false;
    e.status = { state: 'positioned' };
  }

  /** Einzeln signiert (aus der Übersicht geöffnet): zurück zur Übersicht, Hinweis (§3.6). */
  singleSigned(id: DocId, path: string): void {
    const e = this.find(id);
    this.openItem = null;
    if (!e) return;
    e.status = { state: 'signed', path };
    this.written.push(path);
    this.ctx.notify('ng-batch-item-signed', { file: e.doc.name });
  }

  /** `RemoveFromBatch` (nicht während eines Laufs). */
  remove(id: DocId): void {
    if (this.run) return;
    this.entries = this.entries.filter((e) => e.doc.id !== id);
  }

  /* ---------------------------------------------------------------- Ausgabe */

  private mobile(): boolean {
    return this.ctx.platform().mobile;
  }

  outputView(): BatchOutputView {
    if (this.mobile()) return { mode: 'app_documents', folder_display: null, needs_folder: false };
    const folder = this.ctx.settings().output.folder;
    if (folder === null) return { mode: 'ask', folder_display: null, needs_folder: true };
    if (folder === '.' || !isAbsolute(folder)) return { mode: 'next_to_source', folder_display: folder === '.' ? null : folder, needs_folder: false };
    // F-82: ein Dateipfad bzw. ein fehlender Ordner wäre sonst das Ziel aller Dokumente
    const missing = /\.pdf$/i.test(folder) || (this.host.opts.missingFolders ?? []).includes(folder);
    return { mode: 'folder', folder_display: folder, needs_folder: missing };
  }

  /** Zielordner eines Dokuments (`null` = erst nach der Ordnerwahl bekannt). */
  private folderFor(doc: MockDoc, outputDir: string | null): string | null {
    if (this.mobile()) return `${SANDBOX}/Documents`;
    if (outputDir) return outputDir;
    const out = this.outputView();
    if (out.needs_folder) return null;
    const folder = this.ctx.settings().output.folder;
    if (folder === null) return null;
    if (folder === '.') return folderOf(doc.path);
    if (!isAbsolute(folder)) return joinPath(folderOf(doc.path), folder);
    return folder;
  }

  private targetName(e: Entry): string | null {
    if (!e.analyzed || e.status.state === 'rejected') return null;
    return outputFileName(e.doc.path, this.ctx.settings().output.postfix || '_signed');
  }

  private exists(path: string): boolean {
    return (this.host.opts.existingFiles ?? []).includes(path) || this.written.includes(path);
  }

  private targetPath(e: Entry, outputDir: string | null): string | null {
    const name = this.targetName(e);
    const folder = this.folderFor(e.doc, outputDir);
    return name && folder ? joinPath(folder, name) : null;
  }

  /** Pfad beim Schreiben nach `on_existing` (Überspringen wurde vorher geprüft). */
  private writePath(e: Entry, run: Run): string {
    const name = this.targetName(e) ?? outputFileName(e.doc.path, '_signed');
    const folder = this.folderFor(e.doc, run.outputDir) ?? folderOf(e.doc.path);
    const target = joinPath(folder, name);
    if (run.onExisting === 'overwrite' || !this.exists(target)) return target;
    for (let n = 2; n < 1000; n++) {
      const candidate = joinPath(folder, numberedName(name, n));
      if (!this.exists(candidate)) return candidate;
    }
    return target;
  }

  /* ---------------------------------------------------------------- Lauf */

  /** `StartBatch`: genau die genannten Dokumente, eine Sitzung (Prüfungen wie im Kern). */
  start(e: Extract<UiEvent, { type: 'start_batch' }>): void {
    if (this.run) return;
    const entries = e.docs.map((id) => this.find(id)).filter((x): x is Entry => !!x && (x.status.state === 'ready' || x.status.state === 'positioned'));
    if (entries.length === 0) {
      this.ctx.notify('ng-batch-nothing-to-sign');
      return;
    }
    if (!this.host.available(e.backend)) {
      this.ctx.notify('ng-signer-unavailable');
      return;
    }
    const out = this.outputView();
    if (out.needs_folder && !e.output_dir) {
      this.ctx.notify('ng-batch-output-folder-required');
      return;
    }
    for (const x of entries) {
      x.before = x.status;
      x.status = { state: 'queued' };
    }
    this.lastRun = false;
    this.lastRunFolder = this.mobile() ? null : (e.output_dir ?? (out.mode === 'folder' || out.mode === 'next_to_source' ? out.folder_display : null));
    this.run = {
      backend: e.backend,
      order: entries.map((x) => x.doc.id),
      index: 0,
      outputDir: out.needs_folder || out.mode === 'ask' ? e.output_dir : null,
      onExisting: e.on_existing,
      loggedIn: e.backend === 'atrust_native' ? this.host.remembered() : false,
      certificateRead: false,
      sessionOpen: false,
      session: null,
      pause: null,
      confirmations: 0,
      signedInRun: 0,
      faulted: new Set(),
      firstAttempt: true,
    };
    this.nextDocument();
  }

  private current(): Entry | undefined {
    const run = this.run;
    if (!run) return undefined;
    const id = run.order[run.index];
    return id === undefined ? undefined : this.find(id);
  }

  private nextDocument(): void {
    const run = this.run;
    if (!run) return;
    for (;;) {
      if (run.index >= run.order.length) {
        this.finishRun();
        return;
      }
      const e = this.current();
      if (!e) {
        run.index += 1;
        continue;
      }
      // `on_existing = Skip`: vor dem Signieren prüfen – keine Bestätigung für ein Ergebnis, das nicht gespeichert würde
      const target = this.targetPath(e, run.outputDir);
      if (run.onExisting === 'skip' && target && this.exists(target)) {
        e.status = { state: 'skipped', existing: true };
        run.index += 1;
        continue;
      }
      e.status = { state: 'active', stage: run.sessionOpen ? 'preparing' : 'connecting' };
      this.openSession(e);
      return;
    }
  }

  private openSession(e: Entry): void {
    const run = this.run;
    if (!run) return;
    const settings = this.ctx.settings();
    const o = this.host.opts;
    const fault = run.faulted.has(e.doc.id) ? undefined : o.batchFaults?.[e.doc.name];
    if (fault) run.faulted.add(e.doc.id);
    const base = o.signingFlow ?? 'app';
    let flow = base === 'decline' || base === 'timeout' ? (run.firstAttempt ? base : 'app') : base;
    if (fault === 'decline') flow = 'decline';
    run.firstAttempt = false;
    const visible = settings.signature.profile !== 'INVISIBLE' && e.plan?.kind !== 'invisible';
    const failOnce = fault && fault !== 'decline' ? { ...FAULTS[fault], params: { ...FAULTS[fault].params } } : null;
    const continuing = run.sessionOpen ? { index: run.index + 1, total: run.order.length } : null;
    let session: SigningSession | null = null;
    const host: SigningHost = {
      later: (fn, factor) => this.ctx.later(fn, factor),
      finished: () => session && this.documentDone(e, session),
      failed: (code, params, retry) => session && this.documentFailed(e, session, code, params, retry),
      // „Abbrechen“ in einer A-Trust-Meldung: wie eine Ablehnung – nie automatisch weiter (§3.8)
      cancelled: () => session && this.documentFailed(e, session, 'ng-signing-declined', {}, true),
      loggedIn: (remember) => {
        run.loggedIn = true;
        this.host.setRemembered(remember);
      },
      passwordCleared: () => {
        run.loggedIn = false;
        this.host.setRemembered(false);
      },
      unlocked: () => {
        run.loggedIn = true;
      },
    };
    session = new SigningSession(host, {
      backend: run.backend,
      doc: { id: e.doc.id, name: e.doc.name },
      flow,
      visible,
      fido2Available: o.fido2Available ?? true,
      fido2Default: settings.id_austria.fido2_default,
      mobile: this.mobile(),
      autoComplete: o.autoCompleteApp ?? true,
      remembered: run.backend === 'atrust_native' && run.loggedIn,
      prefillUsername: settings.id_austria.username,
      rememberChecked: settings.id_austria.remember,
      keystoreSubject: fileNameOf(settings.keystore.file ?? 'keystore.p12'),
      networkErrorOnce: false,
      referenceValue: referenceValueFor(e.doc.id),
      certificateRead: run.certificateRead,
      sessionUnlocked: run.backend === 'keystore' && run.loggedIn,
      failOnce,
      continuing,
    });
    run.session = session;
    run.sessionOpen = true;
    session.start();
  }

  private documentDone(e: Entry, session: SigningSession): void {
    const run = this.run;
    if (!run || run.session !== session) return;
    const p = session.progress();
    if (p.certificate === 'session') run.certificateRead = true;
    if (run.backend === 'atrust_native' && p.login_done) run.loggedIn = true;
    // Schreiben (`BatchPhase::Writing`): Speicherziel steht seit dem Start fest
    this.ctx.later(() => {
      if (this.run !== run || run.session !== session) return;
      const path = this.writePath(e, run);
      e.status = { state: 'signed', path };
      this.written.push(path);
      run.confirmations += 1;
      run.signedInRun += 1;
      this.ctx.emit({ kind: 'signed', file_name: fileNameOf(path), path });
      run.session = null;
      run.index += 1;
      this.nextDocument();
    });
  }

  private documentFailed(e: Entry, session: SigningSession, code: string, params: Record<string, string>, retry: boolean): void {
    const run = this.run;
    if (!run || run.session !== session) return;
    const scope = scopeOf(code, retry);
    session.close();
    run.session = null;
    if (scope.pause === null) {
      // nicht wiederholbarer Dokumentfehler: Zeile „Fehler“, weiter mit dem nächsten (`ReleaseDocument`)
      e.status = { state: 'failed', code, params };
      run.index += 1;
      this.nextDocument();
      return;
    }
    if (scope.newSession) {
      // Sitzungsfehler: Sitzung geschlossen; eine neue übernimmt Zugangsdaten nur aus dem „Merken“-Speicher
      run.sessionOpen = false;
      run.loggedIn = run.backend === 'atrust_native' && this.host.remembered();
      run.certificateRead = false;
    }
    e.status = { state: 'active', stage: 'paused' };
    run.pause = { doc: { id: e.doc.id, name: e.doc.name }, code, params: { ...params }, scope: scope.pause, can_retry: true, new_session: scope.newSession };
  }

  /** „Erneut versuchen“ einer Pause: dasselbe Dokument (Verbindung: dieselbe Sitzung ohne neue Anmeldung). */
  retry(): void {
    const run = this.run;
    const e = this.current();
    if (!run || !run.pause || !e) return;
    run.pause = null;
    e.status = { state: 'active', stage: run.sessionOpen ? 'preparing' : 'connecting' };
    this.openSession(e);
  }

  /** „Dokument überspringen“: nur dieser Job; Zugangsdaten bleiben (§8 F10). */
  skip(): void {
    const run = this.run;
    const e = this.current();
    if (!run || !e || e.status.state !== 'active') return;
    // schon bestätigt (Signatur wird erstellt bzw. gespeichert): nicht mehr überspringbar
    const stage = run.session?.progress().stage;
    if (!run.pause && (stage === 'finishing' || stage === 'saving')) return;
    run.session?.close();
    run.session = null;
    run.pause = null;
    e.status = { state: 'skipped', existing: false };
    run.index += 1;
    this.nextDocument();
  }

  /** „Stapel abbrechen“ (auch `Cancel` im Lauf): Sitzung abbrechen, Passwort vergessen (F-72), Gespeichertes bleibt. */
  abort(): void {
    const run = this.run;
    if (!run) return;
    run.session?.close();
    for (const id of run.order) {
      const e = this.find(id);
      if (!e) continue;
      if (e.status.state === 'active') e.status = { state: 'cancelled' };
      else if (e.status.state === 'queued') e.status = e.before ?? { state: 'ready' };
    }
    this.run = null;
    this.lastRun = true;
    this.host.setRemembered(false);
    this.ctx.notify('ng-batch-aborted', { signed: String(run.signedInRun) });
    this.host.runEnded();
  }

  private finishRun(): void {
    this.run = null;
    this.lastRun = true;
    const sum = this.summary();
    this.ctx.notify('ng-batch-done', { signed: String(sum.signed), total: String(sum.total) });
    this.host.runEnded();
  }

  /* ---------------------------------------------------------------- Masken der laufenden Sitzung */

  session(): SigningSession | null {
    return this.run?.pause ? null : (this.run?.session ?? null);
  }

  /* ---------------------------------------------------------------- Projektion */

  private positionOf(e: Entry): PositionSummaryView {
    const p = e.plan;
    if (!p) return { kind: 'auto' };
    switch (p.kind) {
      case 'qr':
        return { kind: 'qr' };
      case 'field':
        return { kind: 'field', name: p.name };
      case 'manual':
        return { kind: 'manual', page: p.placement.page_index };
      case 'invisible':
        return { kind: 'invisible' };
    }
  }

  private itemView(e: Entry, progress: SigningProgressView | null): BatchItemView {
    const pdf = e.analyzed && e.status.state !== 'rejected';
    // kennwortgeschützt: Seitenzahl lesbar (/Count ist nicht verschlüsselt), aber keine Miniatur
    const pages = e.analyzed && /\.pdf$/i.test(e.doc.name) ? e.doc.spec.pages.length : null;
    const target = this.targetPath(e, this.run?.outputDir ?? null);
    let status: BatchItemStatusView = structuredClone(e.status);
    if (status.state === 'active' && progress && progress.doc?.id === e.doc.id) status = { state: 'active', stage: progress.stage };
    return {
      doc: e.doc.id,
      name: e.doc.name,
      page_count: pages,
      size_bytes: e.analyzed ? e.doc.spec.sizeBytes : null,
      status,
      position: this.positionOf(e),
      target_name: this.targetName(e),
      target_exists: target !== null && this.exists(target) && e.status.state !== 'signed',
      thumbnail: pdf,
    };
  }

  summary(): BatchSummaryView {
    const count = (state: BatchItemStatusView['state']) => this.entries.filter((e) => e.status.state === state).length;
    const signed = count('signed');
    const rejected = count('rejected');
    const failed = count('failed');
    const skipped = count('skipped');
    const cancelled = count('cancelled');
    return {
      total: this.entries.length,
      signed,
      rejected,
      failed,
      skipped,
      cancelled,
      not_started: this.entries.length - signed - rejected - failed - skipped - cancelled,
      folder_display: this.lastRunFolder,
    };
  }

  private estimate(): BatchEstimateView | null {
    const backend = this.host.selected();
    if (!backend) return null;
    const settings = this.ctx.settings();
    const docs = this.signable();
    const atrust = backend === 'atrust_native';
    const visible = settings.signature.profile !== 'INVISIBLE' && docs.some((e) => e.plan?.kind !== 'invisible');
    const logins = atrust ? (this.host.remembered() ? 0 : 1) : backend === 'keystore' ? (settings.keystore.password_storage === 'keychain' ? 0 : 1) : 0;
    return {
      backend,
      documents: docs.length,
      logins,
      confirmations: docs.length,
      certificate_confirmation_possible: atrust && visible && !settings.certificate_cache.enabled,
      large: atrust && docs.length >= 20,
    };
  }

  view(): BatchView {
    const progress = this.run ? this.progress() : null;
    return {
      items: this.entries.map((e) => this.itemView(e, progress)),
      open_item: this.openItem,
      running: this.run !== null,
      summary: this.lastRun && !this.run ? this.summary() : null,
      output: this.outputView(),
      estimate: this.estimate(),
    };
  }

  /** Fortschritt der Stapelkarte (`SigningProgressView` mit `batch`, k/n, Pause). */
  progress(): SigningProgressView {
    const run = this.run;
    const e = this.current();
    const p = run?.session?.progress() ?? null;
    return {
      backend: run?.backend ?? null,
      stage: run?.pause ? 'paused' : (p?.stage ?? (run?.sessionOpen ? 'preparing' : 'connecting')),
      method: run?.pause ? null : (p?.method ?? null),
      doc: e ? { id: e.doc.id, name: e.doc.name } : null,
      index: run ? Math.min(run.index + 1, run.order.length) : 0,
      total: run?.order.length ?? 0,
      login_done: run ? (p && run.backend === 'atrust_native' ? p.login_done : run.loggedIn) : false,
      confirmations_done: run?.confirmations ?? 0,
      confirmations_total: run?.order.length ?? 0,
      certificate: p?.certificate ?? (run?.certificateRead ? 'session' : 'read'),
      batch: true,
      pause: run?.pause ? structuredClone(run.pause) : null,
    };
  }

  interaction(): InteractionView {
    const s = this.run?.session;
    return s && !this.run?.pause ? structuredClone(s.interaction) : { kind: 'waiting', progress: 'Connecting' };
  }

  /** Datenschutz-Kurzhinweis der Karte (`document_leaves_device`). */
  backend(): BackendId | null {
    return this.run?.backend ?? null;
  }

  dispose(): void {
    this.run?.session?.close();
  }
}
