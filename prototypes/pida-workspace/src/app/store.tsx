/**
 * Zustand des Prototyps als Reducer (explizite, ereignisgetriebene Zustandsmaschine – vgl. Empfehlung
 * in docs/analyse/01 §1). Kein Netz, keine echte Kryptografie: Signieren ist reine Simulation.
 */
import { createContext, useContext, useReducer, type Dispatch, type ReactNode } from 'react';
import {
  RECENT_SEED,
  batchDocs,
  blankDoc,
  contractDoc,
  fieldDoc,
  payslipDoc,
  signedDoc,
  verifyDoc,
} from '../model/documents';
import { autoPosition, MARGIN } from '../model/geometry';
import { blockSize, isoWithOffset, outputName } from '../model/format';
import type {
  DemoResponse,
  DocSpec,
  DocState,
  PageSpec,
  PlaceholderKind,
  Placement,
  RecentItem,
  ScenarioId,
  Settings,
  SigBlockSettings,
  SignerKind,
  Toast,
  View,
} from '../model/types';

export type Overlay = null | 'settings' | 'palette' | 'shortcuts' | 'onboarding';
export type SettingsSection = 'block' | 'document' | 'signer' | 'save' | 'appearance' | 'advanced';

export interface DemoState {
  scenario: ScenarioId;
  response: DemoResponse;
  /** A-Trust beim nächsten Verbindungsaufbau nicht erreichbar (einmalig) */
  atrustDown: boolean;
}

export interface BatchRun {
  phase: 'running' | 'summary';
  queue: string[];
  done: string[];
  skipped: string[];
}

export interface SigningSession {
  docIds: string[];
  batch: boolean;
}

export interface AppState {
  view: View;
  docs: DocState[];
  activeId: string | null;
  batchMode: boolean;
  batchRun: BatchRun | null;
  signer: SignerKind;
  signing: SigningSession | null;
  lastSigned: { docId: string; outputName: string; folder: string; time: string } | null;
  verify: DocSpec | null;
  overlay: Overlay;
  settingsSection: SettingsSection;
  privacyAcked: boolean;
  onboarded: boolean;
  settings: Settings;
  demo: DemoState;
  toasts: Toast[];
  recent: RecentItem[];
}

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  uiLang: 'de',
  sigBlock: { profile: 'standard', lang: 'de', logo: null, logoName: null, note: null },
  defaultSigner: 'idaustria',
  idaUser: '+43 660 1234123',
  rememberUser: true,
  fido2Default: false,
  keystoreFile: 'max.mustermann.p12',
  keystoreAlias: 'max mustermann',
  saveTarget: 'original',
  saveFolder: '~/Dokumente/Signiert',
  namePattern: '{name}_signiert',
  proxy: 'system',
  proxyHost: '',
  proxyPort: '',
  autoUpdate: true,
  debugLog: false,
  pdfaForce: false,
  placeholderSearch: true,
  placeholderKind: 'qr',
};

/** Name der signierenden Person in allen Mock-Signaturblöcken. */
export const SIGNER_NAME = 'Max Mustermann';

/* ------------------------------------------------------------------ Hilfen */

/** Seiten eines Dokuments inkl. angehängter leerer Seite („Auf neuer Seite“). */
export function displayPages(doc: DocState): PageSpec[] {
  const pages = doc.spec.pages;
  if (!doc.newPage) return pages;
  const last = pages[pages.length - 1] ?? { w: 595, h: 842, blocks: [] };
  return [...pages, { w: last.w, h: last.h, blocks: [] }];
}

/** Welche Platzhalter beim Öffnen automatisch genutzt werden (`null` = Suche aus). */
export function autoTarget(s: Settings): PlaceholderKind | null {
  return s.placeholderSearch ? s.placeholderKind : null;
}

/**
 * Erste Platzierung: gesuchter Platzhalter (QR-Code bzw. leeres Signaturfeld) automatisch, sonst freie
 * Fläche der letzten Seite. `auto = null` platziert immer frei.
 */
export function initialPlacement(doc: DocState, sig: SigBlockSettings, auto: PlaceholderKind | null = 'qr'): Placement {
  const target = auto ? doc.spec.fields.find((f) => f.kind === auto) : undefined;
  if (target) return { page: target.page, x: target.rect.x, y: target.rect.y, fieldId: target.id };
  const pages = displayPages(doc);
  const index = pages.length - 1;
  const page = pages[index];
  const size = blockSize(sig);
  if (!page) return { page: 0, x: MARGIN.left, y: MARGIN.top, fieldId: null };
  const pos = autoPosition(page, size) ?? { x: page.w - MARGIN.right - size.w, y: page.h - MARGIN.bottom - size.h };
  return { page: index, x: pos.x, y: pos.y, fieldId: null };
}

function docState(spec: DocSpec, sig: SigBlockSettings, auto: PlaceholderKind | null): DocState {
  const base: DocState = {
    spec,
    status: spec.locked ? 'error' : 'ready',
    placement: null,
    invisible: false,
    newPage: false,
    unlocked: !spec.locked,
    outputName: null,
    autoPlaced: true,
  };
  const placement = initialPlacement(base, sig, auto);
  return { ...base, placement, status: spec.locked ? 'error' : placement.fieldId ? 'positioned' : 'ready' };
}

export function activeDoc(state: AppState): DocState | null {
  return state.docs.find((d) => d.spec.id === state.activeId) ?? null;
}

export function saveFolder(state: AppState, doc: DocState): string {
  return state.settings.saveTarget === 'folder' ? state.settings.saveFolder : doc.spec.folder;
}

function signerFromSettings(s: Settings): SignerKind {
  return s.defaultSigner === 'ask' ? 'idaustria' : s.defaultSigner;
}

/* ------------------------------------------------------------------ Szenarien */

interface ScenarioResult {
  view: View;
  docs: DocSpec[];
  verify: DocSpec | null;
  atrustDown: boolean;
}

function scenario(id: ScenarioId): ScenarioResult {
  switch (id) {
    case 'start':
      return { view: 'start', docs: [], verify: null, atrustDown: false };
    case 'blank':
      return { view: 'workspace', docs: [blankDoc()], verify: null, atrustDown: false };
    case 'contract':
      return { view: 'workspace', docs: [contractDoc()], verify: null, atrustDown: false };
    case 'field':
      return { view: 'workspace', docs: [fieldDoc()], verify: null, atrustDown: false };
    case 'signed':
      return { view: 'workspace', docs: [signedDoc()], verify: null, atrustDown: false };
    case 'batch':
      return { view: 'batch', docs: batchDocs(), verify: null, atrustDown: false };
    case 'verify':
      return { view: 'verify', docs: [], verify: verifyDoc(), atrustDown: false };
    case 'error-password':
      return { view: 'workspace', docs: [payslipDoc()], verify: null, atrustDown: false };
    case 'error-atrust':
      return { view: 'workspace', docs: [contractDoc()], verify: null, atrustDown: true };
  }
}

export interface InitOptions {
  scenario?: ScenarioId;
  onboarded?: boolean;
  privacyAcked?: boolean;
  settings?: Partial<Settings>;
  response?: DemoResponse;
}

export function createState(opts: InitOptions = {}): AppState {
  const settings: Settings = { ...DEFAULT_SETTINGS, ...opts.settings, sigBlock: { ...DEFAULT_SETTINGS.sigBlock, ...opts.settings?.sigBlock } };
  const sc = scenario(opts.scenario ?? 'start');
  const docs = sc.docs.map((d) => docState(d, settings.sigBlock, autoTarget(settings)));
  return {
    view: sc.view,
    docs,
    activeId: sc.view === 'workspace' ? (docs[0]?.spec.id ?? null) : null,
    batchMode: sc.view === 'batch',
    batchRun: null,
    signer: signerFromSettings(settings),
    signing: null,
    lastSigned: null,
    verify: sc.verify,
    overlay: opts.onboarded === false ? 'onboarding' : null,
    settingsSection: 'block',
    privacyAcked: opts.privacyAcked ?? false,
    onboarded: opts.onboarded ?? true,
    settings,
    demo: { scenario: opts.scenario ?? 'start', response: opts.response ?? 'app', atrustDown: sc.atrustDown },
    toasts: [],
    recent: RECENT_SEED.map((r) => ({ ...r })),
  };
}

/* ------------------------------------------------------------------ Aktionen */

export type Action =
  | { type: 'scenario'; id: ScenarioId }
  | { type: 'openFiles'; names: string[] }
  | { type: 'openSpec'; spec: DocSpec }
  | { type: 'goHome' }
  | { type: 'openDoc'; id: string }
  | { type: 'backToBatch' }
  | { type: 'place'; id: string; placement: Placement }
  | { type: 'autoPlace'; id: string }
  | { type: 'setInvisible'; id: string; value: boolean }
  | { type: 'setNewPage'; id: string; value: boolean }
  | { type: 'unlock'; id: string }
  | { type: 'setSigner'; signer: SignerKind }
  | { type: 'startSigning' }
  | { type: 'cancelSigning' }
  | { type: 'ackPrivacy' }
  | { type: 'resetPrivacy' }
  | { type: 'atrustRecovered' }
  | { type: 'signed'; time?: string }
  | { type: 'batchStep' }
  | { type: 'batchDismiss' }
  | { type: 'overlay'; overlay: Overlay; section?: SettingsSection }
  | { type: 'settings'; patch: Partial<Settings> }
  | { type: 'sigBlock'; patch: Partial<SigBlockSettings> }
  | { type: 'demo'; patch: Partial<DemoState> }
  | { type: 'onboardingDone'; imported: boolean; signer: SignerKind | 'ask' }
  | { type: 'toast'; toast: Omit<Toast, 'id'> }
  | { type: 'dismissToast'; id: number }
  | { type: 'openVerify'; spec: DocSpec | null }
  | { type: 'signVerified' };

let toastSeq = 1;

function updateDoc(state: AppState, id: string, fn: (d: DocState) => DocState): AppState {
  return { ...state, docs: state.docs.map((d) => (d.spec.id === id ? fn(d) : d)) };
}

/** Bei geänderter Blockgröße frei platzierte Blöcke neu auf freie Fläche setzen (nur unberührte). */
function replaceUntouched(state: AppState, sig: SigBlockSettings): DocState[] {
  return state.docs.map((d) =>
    d.status === 'ready' && d.placement && !d.placement.fieldId
      ? { ...d, placement: initialPlacement(d, sig, d.autoPlaced ? autoTarget(state.settings) : null) }
      : d,
  );
}

/**
 * Geänderte Platzhaltersuche wirkt sofort auf automatisch platzierte Dokumente (von Hand gesetzte und
 * signierte bleiben). „Positioniert“ allein wegen eines Platzhalters fällt dabei auf „Bereit“ zurück.
 */
function reapplyAuto(state: AppState): DocState[] {
  const auto = autoTarget(state.settings);
  return state.docs.map((d) => {
    if (!d.autoPlaced || d.status === 'done' || d.status === 'signing') return d;
    const placement = initialPlacement(d, state.settings.sigBlock, auto);
    const reviewed = d.status === 'positioned' && !d.placement?.fieldId;
    const status = d.status === 'error' ? 'error' : placement.fieldId || reviewed ? 'positioned' : 'ready';
    return { ...d, placement, status };
  });
}

export function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case 'scenario': {
      const fresh = createState({
        scenario: action.id,
        onboarded: true,
        privacyAcked: state.privacyAcked,
        settings: state.settings,
        response: state.demo.response,
      });
      return { ...fresh, onboarded: state.onboarded, overlay: null, toasts: [] };
    }
    case 'openFiles': {
      const names = action.names.filter((n) => n.trim().length > 0);
      if (names.length === 0) return state;
      if (names.length === 1) {
        const name = names[0] as string;
        const spec = contractDoc(name, `doc-${Date.now()}`, false);
        const doc = docState(spec, state.settings.sigBlock, autoTarget(state.settings));
        return { ...state, view: 'workspace', docs: [doc], activeId: spec.id, batchMode: false, batchRun: null, verify: null };
      }
      const pool = batchDocs();
      const docs = names.map((name, i) => {
        const tpl = pool[i % pool.length] as DocSpec;
        return docState({ ...tpl, id: `doc-${i}-${tpl.id}`, fileName: name }, state.settings.sigBlock, autoTarget(state.settings));
      });
      return { ...state, view: 'batch', docs, activeId: null, batchMode: true, batchRun: null, verify: null };
    }
    case 'openSpec': {
      const doc = docState(action.spec, state.settings.sigBlock, autoTarget(state.settings));
      return { ...state, view: 'workspace', docs: [doc], activeId: action.spec.id, batchMode: false, batchRun: null, verify: null };
    }
    case 'goHome':
      return { ...state, view: 'start', docs: [], activeId: null, batchMode: false, batchRun: null, verify: null, signing: null, lastSigned: null };
    case 'openDoc':
      return { ...state, view: 'workspace', activeId: action.id };
    case 'backToBatch':
      return {
        ...state,
        view: 'batch',
        activeId: null,
        docs: state.docs.map((d) => (d.spec.id === state.activeId && d.status === 'ready' ? { ...d, status: 'positioned' } : d)),
      };
    case 'place':
      return updateDoc(state, action.id, (d) => ({ ...d, placement: action.placement, autoPlaced: false, status: d.status === 'ready' ? 'positioned' : d.status }));
    case 'autoPlace':
      return updateDoc(state, action.id, (d) => ({ ...d, placement: initialPlacement({ ...d }, state.settings.sigBlock, null), autoPlaced: false }));
    case 'setInvisible':
      return updateDoc(state, action.id, (d) => ({ ...d, invisible: action.value }));
    case 'setNewPage':
      return updateDoc(state, action.id, (d) => {
        const next = { ...d, newPage: action.value };
        return { ...next, placement: initialPlacement(next, state.settings.sigBlock, null), autoPlaced: false };
      });
    case 'unlock':
      return updateDoc(state, action.id, (d) => ({ ...d, unlocked: true, status: d.placement?.fieldId ? 'positioned' : 'ready' }));
    case 'setSigner':
      return { ...state, signer: action.signer };
    case 'startSigning': {
      if (state.signing) return state;
      if (state.view === 'batch') {
        const ids = state.docs.filter((d) => d.status === 'ready' || d.status === 'positioned').map((d) => d.spec.id);
        if (ids.length === 0) return state;
        return { ...state, signing: { docIds: ids, batch: true }, overlay: null };
      }
      const doc = activeDoc(state);
      if (!doc || !doc.unlocked || doc.status === 'done') return state;
      return { ...state, signing: { docIds: [doc.spec.id], batch: false }, overlay: null };
    }
    case 'cancelSigning':
      return { ...state, signing: null };
    case 'ackPrivacy':
      return { ...state, privacyAcked: true };
    case 'resetPrivacy':
      return { ...state, privacyAcked: false };
    case 'atrustRecovered':
      return { ...state, demo: { ...state.demo, atrustDown: false } };
    case 'signed': {
      const session = state.signing;
      if (!session) return state;
      const time = action.time ?? isoWithOffset(new Date());
      if (session.batch) {
        const queue = session.docIds;
        // Übersprungen sind nur Dokumente mit Fehler – bereits einzeln signierte zählen nicht dazu
        const skipped = state.docs.filter((d) => !queue.includes(d.spec.id) && d.status !== 'done').map((d) => d.spec.id);
        return {
          ...state,
          signing: null,
          batchRun: { phase: queue.length ? 'running' : 'summary', queue, done: [], skipped },
          docs: state.docs.map((d) => (d.spec.id === queue[0] ? { ...d, status: 'signing' } : d)),
        };
      }
      const id = session.docIds[0] as string;
      const doc = state.docs.find((d) => d.spec.id === id);
      if (!doc) return { ...state, signing: null };
      const out = outputName(doc.spec.fileName, state.settings.namePattern);
      const folder = saveFolder(state, doc);
      const recent: RecentItem = { id: `recent-${Date.now()}`, fileName: out, signedAt: time, signer: state.signer, pages: displayPages(doc).length };
      return {
        ...updateDoc(state, id, (d) => ({ ...d, status: 'done', outputName: out })),
        signing: null,
        view: state.batchMode ? 'batch' : 'done',
        activeId: state.batchMode ? null : id,
        lastSigned: { docId: id, outputName: out, folder, time },
        recent: [recent, ...state.recent].slice(0, 5),
      };
    }
    case 'batchStep': {
      const run = state.batchRun;
      if (!run || run.phase !== 'running') return state;
      const [current, ...rest] = run.queue;
      if (!current) return { ...state, batchRun: { ...run, phase: 'summary' } };
      const docs = state.docs.map((d) => {
        if (d.spec.id === current) return { ...d, status: 'done' as const, outputName: outputName(d.spec.fileName, state.settings.namePattern) };
        if (d.spec.id === rest[0]) return { ...d, status: 'signing' as const };
        return d;
      });
      const finished = state.docs.find((d) => d.spec.id === current);
      const recent: RecentItem[] = finished
        ? [{ id: `recent-${Date.now()}-${current}`, fileName: outputName(finished.spec.fileName, state.settings.namePattern), signedAt: isoWithOffset(new Date()), signer: state.signer, pages: finished.spec.pages.length }, ...state.recent].slice(0, 5)
        : state.recent;
      return { ...state, docs, recent, batchRun: { ...run, queue: rest, done: [...run.done, current], phase: rest.length ? 'running' : 'summary' } };
    }
    case 'batchDismiss':
      return { ...state, batchRun: null };
    case 'overlay':
      return { ...state, overlay: action.overlay, settingsSection: action.section ?? state.settingsSection };
    case 'settings': {
      const settings = { ...state.settings, ...action.patch };
      const next = { ...state, settings };
      if (action.patch.defaultSigner && action.patch.defaultSigner !== 'ask' && !state.signing) next.signer = action.patch.defaultSigner;
      if (autoTarget(settings) !== autoTarget(state.settings)) next.docs = reapplyAuto(next);
      return next;
    }
    case 'sigBlock': {
      const sigBlock = { ...state.settings.sigBlock, ...action.patch };
      const sizeChanged = JSON.stringify(blockSize(sigBlock)) !== JSON.stringify(blockSize(state.settings.sigBlock));
      const next = { ...state, settings: { ...state.settings, sigBlock } };
      return sizeChanged ? { ...next, docs: replaceUntouched(next, sigBlock) } : next;
    }
    case 'demo':
      return { ...state, demo: { ...state.demo, ...action.patch } };
    case 'onboardingDone': {
      const settings: Settings = action.imported
        ? { ...state.settings, saveTarget: 'folder', saveFolder: '~/Dokumente/Signiert', idaUser: '+43 660 1234123' }
        : state.settings;
      const withSigner = { ...settings, defaultSigner: action.signer };
      return { ...state, onboarded: true, overlay: null, settings: withSigner, signer: signerFromSettings(withSigner) };
    }
    case 'toast':
      return { ...state, toasts: [...state.toasts, { ...action.toast, id: toastSeq++ }].slice(-4) };
    case 'dismissToast':
      return { ...state, toasts: state.toasts.filter((t) => t.id !== action.id) };
    case 'openVerify':
      return { ...state, view: 'verify', verify: action.spec, docs: [], activeId: null, batchMode: false, batchRun: null };
    case 'signVerified': {
      if (!state.verify) return state;
      const spec = { ...state.verify, id: `${state.verify.id}-sign` };
      const doc = docState(spec, state.settings.sigBlock, autoTarget(state.settings));
      return { ...state, view: 'workspace', docs: [doc], activeId: spec.id, verify: null, batchMode: false };
    }
  }
}

/* ------------------------------------------------------------------ Kontext */

const StateContext = createContext<AppState | null>(null);
const DispatchContext = createContext<Dispatch<Action>>(() => undefined);

export function StoreProvider({ initial, children }: { initial: AppState; children: ReactNode }) {
  const [state, dispatch] = useReducer(reducer, initial);
  return (
    <StateContext.Provider value={state}>
      <DispatchContext.Provider value={dispatch}>{children}</DispatchContext.Provider>
    </StateContext.Provider>
  );
}

export function useAppState(): AppState {
  const s = useContext(StateContext);
  if (!s) throw new Error('StoreProvider fehlt');
  return s;
}

export function useDispatch(): Dispatch<Action> {
  return useContext(DispatchContext);
}
