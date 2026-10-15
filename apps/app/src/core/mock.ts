/**
 * TS-Mock-Kern für `VITE_MOCK=1`, die Browser-Vorschau und die Komponententests.
 *
 * Er spielt den Ablauf von PDF-Over nach (docs/analyse/01 §3–§4: Öffnen → Platzhalter →
 * Positionieren → Signiermittel → ID-Austria-Masken bzw. Keystore → Ergebnis), liefert `ViewState`s im
 * serde-Format von `pdfover-core-api` und simuliert die nativen Dienste (Dialoge, Drag & Drop). Er ist
 * **kein** Ersatz für die Fachlogik des Rust-Kerns: Regeln sind nur so weit nachgebaut, wie die
 * Oberfläche sie zum Bedienen und Testen braucht. Keine echten Zugangsdaten, kein Netz.
 */
import { MAIN_BAR_ORDER, baseName, defaultSettings, dirName, mainBar, mmToPt } from './defaults';
import type { Bridge, FileDragEvent } from './bridge';
import type {
  BackendId,
  BackendOptionView,
  Capabilities,
  DocId,
  DocInfo,
  DoneItemView,
  FormContext,
  InteractionView,
  KeystoreImportView,
  Lang,
  MainBarView,
  NativeForm,
  NoticeView,
  Placement,
  PlatformView,
  QueueItemView,
  ResumeOfferView,
  SaveDialogView,
  Settings,
  StepView,
  UiEvent,
  ViewState,
} from './types';

/** Ablauf der ID-Austria-Freigabe nach der Anmeldung. */
export type MockSigningFlow = 'app' | 'sms' | 'qr' | 'fido2';

/** Optionen des Mock-Kerns. */
export interface MockOptions {
  /** simulierte Latenz in ms (Standard 350; Tests: 0) */
  delayMs?: number;
  /** UI-Sprache (sonst Einstellung bzw. `de`) */
  language?: Lang;
  /** Platzhalter beim Öffnen */
  placeholder?: 'none' | 'fields' | 'qr';
  /** ID-Austria-Ablauf nach der Anmeldung */
  signingFlow?: MockSigningFlow;
  /** lokale BKU erreichbar */
  localBkuAvailable?: boolean;
  /** App-Bestätigung schließt sich nach einiger Zeit selbst (Long-Poll); Tests: `false` */
  autoCompleteApp?: boolean;
  /** Einstellungen beim Start */
  settings?: Settings;
  /** vorhandene Dateien (Überschreiben-Rückfrage) */
  existingFiles?: string[];
  /** Antwort des Dateidialogs (`null` = abgebrochen) */
  pickFilesResult?: string[] | null;
  /** Antwort des Speichern-Dialogs (`null` = abgebrochen; sonst Vorschlag) */
  pickSaveResult?: string | null;
  /** Antwort des Ordnerdialogs */
  pickDirectoryResult?: string | null;
  /** FIDO2 verfügbar */
  fido2Available?: boolean;
  /** Schlüsselbund verfügbar */
  keychainAvailable?: boolean;
  /** Start direkt in diesem Zustand (Tests) */
  initialView?: ViewState;
  /** Warnungen der Eingangsanalyse (Codes, z. B. `pdf-usage-rights`) → Schritt `warning` */
  warnings?: string[];
  /** Import-Angebot aus `~/.pdf-over` (Pfad der Altdatei) */
  migrationOffer?: string | null;
  /** die Altdatei enthält Klartext-Passwörter */
  migrationSecrets?: boolean;
  /** Einstellungen schreibgeschützt (`-c` mit Altdatei, unlesbare Datei) */
  readOnly?: boolean;
  /** Plattform (Standard: Desktop/macOS); Mobile: Teilen/Exportieren, PKCS#12-Import, kein FIDO2 */
  platform?: 'desktop' | 'ios' | 'android';
  /** Angebot „Signatur fortsetzen“ beim Start (Mobile) */
  resumeOffer?: ResumeOfferView | null;
  /** Antwort der Mobile-Dateiauswahl (`[]` = abgebrochen) */
  pickMobileFilesResult?: string[];
}

interface MockDoc {
  id: DocId;
  path: string;
  name: string;
  sizes: [number, number][];
  signed: boolean;
}

type Stage =
  | 'starting'
  | 'open'
  | 'loading'
  | 'warning'
  | 'placeholder'
  | 'position'
  | 'backend'
  | 'signing'
  | 'output'
  | 'error'
  | 'finished';

const A4: [number, number] = [595.28, 841.89];
const A4_LANDSCAPE: [number, number] = [841.89, 595.28];
const SIGNATURE_DATA_URL = 'https://www.a-trust.at/mobile/ShowSigobj.aspx?demo';

/** `<Basisname><Postfix>.<Erweiterung>` (F-81) – nur Simulation, die Regel liegt im Kern. */
function outputFileName(input: string, postfix: string): string {
  const name = baseName(input);
  const dot = name.lastIndexOf('.');
  if (dot <= 0) return `${name}${postfix}`;
  return `${name.slice(0, dot)}${postfix}${name.slice(dot)}`;
}

/** Ergebnis-Schritt: gespeichert, Speichern-Knopf, Überschreib-Rückfrage, Speichern-Dialog. */
interface OutputState {
  path: string | null;
  savePending: boolean;
  askOverwrite: string | null;
  saveDialog: SaveDialogView | null;
}

const EMPTY_OUTPUT: OutputState = { path: null, savePending: false, askOverwrite: null, saveDialog: null };

function joinPath(dir: string, name: string): string {
  if (!dir) return name;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function escapeXml(s: string): string {
  return s.replace(/[<>&"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

/** Beispiel-QR (SVG-Muster, kein echter Code). */
function demoQrBytes(): number[] {
  const cells: string[] = [];
  let seed = 7;
  for (let y = 0; y < 21; y++) {
    for (let x = 0; x < 21; x++) {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      const finder = (x < 7 && y < 7) || (x > 13 && y < 7) || (x < 7 && y > 13);
      const on = finder ? x % 6 === 0 || y % 6 === 0 || (x % 7 > 1 && x % 7 < 5 && y % 7 > 1 && y % 7 < 5) : seed % 3 === 0;
      if (on) cells.push(`<rect x="${x}" y="${y}" width="1" height="1"/>`);
    }
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 25 25" width="250" height="250"><rect x="-2" y="-2" width="25" height="25" fill="#fff"/><g fill="#000">${cells.join('')}</g></svg>`;
  return Array.from(new TextEncoder().encode(svg));
}

function capabilitiesFor(id: BackendId): Capabilities {
  switch (id) {
    case 'atrust_native':
      return {
        input: 'Document',
        output: 'Cms',
        certificate: 'OnlyForVisibleBlock',
        interaction: 'NativeForms',
        server: 'None',
        max_batch: 1,
        login_once_per_batch: true,
        document_leaves_device: true,
        qualification: 'Qes',
        factors: { app: true, sms: true, qr: true, fido2: true, card: false },
        cms_reserve: 16384,
      };
    case 'local_bku':
      return {
        input: 'Document',
        output: 'Cms',
        certificate: 'OnlyForVisibleBlock',
        interaction: 'External',
        server: 'None',
        max_batch: 1,
        login_once_per_batch: false,
        document_leaves_device: false,
        qualification: 'Qes',
        factors: { app: false, sms: false, qr: false, fido2: false, card: true },
        cms_reserve: 16384,
      };
    default:
      return {
        input: { Digest: { algs: ['Sha256', 'Sha384', 'Sha512'] } },
        output: 'RawSignature',
        certificate: 'Required',
        interaction: 'Secret',
        server: 'None',
        max_batch: 1,
        login_once_per_batch: false,
        document_leaves_device: false,
        qualification: 'Unknown',
        factors: { app: false, sms: false, qr: false, fido2: false, card: false },
        cms_reserve: null,
      };
  }
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

  private readonly opts: Required<
    Omit<MockOptions, 'initialView' | 'settings' | 'language' | 'migrationOffer' | 'resumeOffer' | 'pickMobileFilesResult'>
  > & {
    language: Lang | undefined;
  };
  private migrationOffer: string | null;
  private resumeOffer: ResumeOfferView | null;
  private keystoreImport: KeystoreImportView | null = null;
  private readonly pickMobileFilesResult: string[] | null;
  private readonly listeners = new Set<(v: ViewState) => void>();
  private readonly dragListeners = new Set<(e: FileDragEvent) => void>();
  private forced: ViewState | null = null;
  private timers = new Set<ReturnType<typeof setTimeout>>();

  private stage: Stage = 'starting';
  private queue: MockDoc[] = [];
  private nextDocId = 1;
  private page = 0;
  private placement: Placement | null = null;
  private newPage = false;
  private blockRevision = 1;
  private positionChoice: 'manual' | 'auto' | 'placeholder' | 'invisible' = 'manual';
  private backend: BackendId | null = null;
  private privacyFor: BackendId | null = null;
  private interaction: InteractionView = { kind: 'waiting', progress: 'Connecting' };
  private request = 0;
  private loggedIn = false;
  private rememberedPassword = false;
  private output: OutputState = EMPTY_OUTPUT;
  private error: { code: string; params: Record<string, string>; retry: boolean; onRetry: () => void; onCancel: () => void } | null =
    null;
  private settings: Settings;
  private settingsOpen = false;
  private problems: Record<string, string> = {};
  private notices: NoticeView[] = [];
  private noticeId = 0;
  private done: DoneItemView[] = [];
  private exitCode = 0;
  private readonly lang: Lang | undefined;

  constructor(options: MockOptions = {}) {
    this.opts = {
      delayMs: options.delayMs ?? 350,
      placeholder: options.placeholder ?? 'none',
      signingFlow: options.signingFlow ?? 'app',
      localBkuAvailable: options.localBkuAvailable ?? true,
      autoCompleteApp: options.autoCompleteApp ?? true,
      existingFiles: options.existingFiles ?? [],
      pickFilesResult:
        options.pickFilesResult === undefined
          ? ['/Users/demo/Dokumente/Vertrag.pdf', '/Users/demo/Dokumente/Angebot 2026.pdf']
          : options.pickFilesResult,
      pickSaveResult: options.pickSaveResult === undefined ? '' : options.pickSaveResult,
      pickDirectoryResult: options.pickDirectoryResult === undefined ? '/Users/demo/Signiert' : options.pickDirectoryResult,
      fido2Available: options.fido2Available ?? true,
      keychainAvailable: options.keychainAvailable ?? true,
      warnings: options.warnings ?? [],
      migrationSecrets: options.migrationSecrets ?? false,
      readOnly: options.readOnly ?? false,
      platform: options.platform ?? 'desktop',
      language: options.language,
    };
    this.migrationOffer = options.migrationOffer ?? null;
    this.resumeOffer = options.resumeOffer ?? null;
    this.pickMobileFilesResult = options.pickMobileFilesResult ?? null;
    this.lang = options.language;
    this.settings = options.settings ? structuredClone(options.settings) : defaultSettings();
    if (options.initialView) {
      this.forced = options.initialView;
    } else {
      this.later(() => {
        this.stage = 'open';
        this.emit();
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

  pageTileUrl(doc: DocId, page: number, scale: number): string {
    const d = this.queue.find((q) => q.id === doc);
    const size = d?.sizes[page] ?? A4;
    const w = Math.round(size[0] * scale);
    const h = Math.round(size[1] * scale);
    const lines: string[] = [];
    for (let i = 0; i < 18; i++) {
      const y = 0.12 + i * 0.035;
      const len = i % 5 === 4 ? 0.45 : 0.76;
      lines.push(`<rect x="${0.12 * w}" y="${y * h}" width="${len * w}" height="${0.011 * h}" rx="2" fill="#d5dbe3"/>`);
    }
    const title = escapeXml(`${d?.name ?? 'Dokument'} – ${page + 1}`);
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="#fff"/><text x="${0.12 * w}" y="${0.08 * h}" font-family="sans-serif" font-size="${Math.max(10, 0.025 * h)}" fill="#33415c">${title}</text>${lines.join('')}</svg>`;
    return svgDataUrl(svg);
  }

  blockPreviewUrl(scale: number, revision: number): string {
    const { width, height } = this.blockSize();
    const w = Math.round(width * scale);
    const h = Math.round(height * scale);
    const profile = this.settings.signature.profile;
    const content =
      profile === 'BASE_LOGO'
        ? `<rect x="${w * 0.1}" y="${h * 0.1}" width="${w * 0.8}" height="${h * 0.8}" rx="${w * 0.08}" fill="#2f7fd0"/><text x="${w / 2}" y="${h * 0.6}" text-anchor="middle" font-family="sans-serif" font-size="${h * 0.28}" fill="#fff">Logo</text>`
        : `<rect x="1" y="1" width="${w - 2}" height="${h - 2}" fill="#fff" stroke="#6b7a90"/><rect x="1" y="1" width="${h - 2}" height="${h - 2}" fill="#e8f0fb" stroke="#6b7a90"/><text x="${h / 2}" y="${h * 0.58}" text-anchor="middle" font-family="sans-serif" font-size="${h * 0.22}" fill="#174a8c">✓</text>${['Unterzeichner: Max Mustermann', 'Datum: 2026-10-15T10:00:00+02:00', 'Prüfinformation: Informationen zur Prüfung'].map((t, i) => `<text x="${h + 6}" y="${(i + 1) * h * 0.27}" font-family="sans-serif" font-size="${h * 0.12}" fill="#1d2633">${escapeXml(t)}</text>`).join('')}`;
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" data-rev="${revision}">${content}</svg>`;
    return svgDataUrl(svg);
  }

  async dispatch(event: UiEvent): Promise<void> {
    this.dispatched.push(event);
    if (this.forced) {
      // Feste Testzustände reagieren nicht; Tests prüfen `dispatched`.
      return;
    }
    this.handle(event);
    this.emit();
  }

  // ------------------------------------------------------------------------------------------
  // PlatformServices
  // ------------------------------------------------------------------------------------------

  async pickFiles(options: { title: string; multiple: boolean }): Promise<string[] | null> {
    this.nativeCalls.push(`pickFiles:${options.multiple ? 'multiple' : 'single'}`);
    const result = this.opts.pickFilesResult;
    if (result === null) return null;
    return options.multiple ? [...result] : result.slice(0, 1);
  }

  async pickSavePath(options: { defaultPath?: string | null }): Promise<string | null> {
    this.nativeCalls.push(`pickSavePath:${options.defaultPath ?? ''}`);
    const r = this.opts.pickSaveResult;
    if (r === null) return null;
    return r || options.defaultPath || '/Users/demo/Dokumente/signiert.pdf';
  }

  async pickDirectory(): Promise<string | null> {
    this.nativeCalls.push('pickDirectory');
    return this.opts.pickDirectoryResult;
  }

  async pickMobileFiles(options: { kind: 'pdf' | 'pkcs12'; multiple: boolean }): Promise<string[]> {
    this.nativeCalls.push(`pickMobileFiles:${options.kind}:${options.multiple ? 'multiple' : 'single'}`);
    if (!this.mobile) throw new Error('ng-mobile-unsupported');
    const sandbox = '/var/mobile/Containers/Data/Application/DEMO/Library/Caches/import/1';
    const result =
      this.pickMobileFilesResult ?? (options.kind === 'pdf' ? [`${sandbox}/Vertrag.pdf`, `${sandbox}/Angebot.pdf`] : [`${sandbox}/test.p12`]);
    return options.multiple ? [...result] : result.slice(0, 1);
  }

  /** Mobile-Plattform? */
  private get mobile(): boolean {
    return this.opts.platform !== 'desktop';
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

  async revealOutput(): Promise<void> {
    this.nativeCalls.push('revealOutput');
  }

  async openLogDir(): Promise<void> {
    this.nativeCalls.push('openLogDir');
  }

  async openNotices(): Promise<void> {
    this.nativeCalls.push('openNotices');
  }

  async appInfo() {
    return { version: '0.1.0', platform: this.mobile ? this.opts.platform : 'mock', core_available: true };
  }

  // ------------------------------------------------------------------------------------------
  // Steuerung für Tests
  // ------------------------------------------------------------------------------------------

  /** Setzt einen festen Zustand (Ereignisse werden nur protokolliert). */
  setView(view: ViewState): void {
    this.forced = view;
    this.emit();
  }

  /** Long-Poll meldet „bestätigt“ (App/QR). */
  completeConfirmation(): void {
    if (this.stage === 'signing' && this.interaction.kind === 'form') {
      this.finishSigning();
      this.emit();
    }
  }

  /** Gewählte Position und Signiermittel (Tests). */
  get choices(): { position: string; backend: BackendId | null } {
    return { position: this.positionChoice, backend: this.backend };
  }

  /** Alle Zeitgeber stoppen. */
  dispose(): void {
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
    const view: ViewState = {
      step: this.stepView(),
      main_bar: this.bar(),
      settings: this.settingsOpen
        ? {
            settings: structuredClone(this.settings),
            problems: { ...this.problems },
            keychain_available: this.opts.keychainAvailable,
            fido2_available: this.opts.fido2Available,
            read_only: this.opts.readOnly,
            preview_revision: this.blockRevision,
          }
        : null,
      language: this.settings.general.ui_language ?? this.lang ?? 'de',
      notices: this.notices.map((n) => ({ ...n, params: { ...n.params } })),
      migration: this.migrationOffer
        ? { legacy_file: this.migrationOffer, plaintext_secrets: this.opts.migrationSecrets }
        : null,
      done: this.done.map((d) => structuredClone(d)),
      platform: this.platformView(),
      resume: this.stage === 'open' && this.resumeOffer ? { ...this.resumeOffer } : null,
      keystore_import: this.keystoreImport ? { ...this.keystoreImport } : null,
    };
    return view;
  }

  private emit(): void {
    const view = this.current();
    for (const cb of this.listeners) cb(view);
  }

  private later(fn: () => void, factor = 1): void {
    const t = setTimeout(() => {
      this.timers.delete(t);
      fn();
    }, this.opts.delayMs * factor);
    this.timers.add(t);
  }

  private notify(code: string, params: Record<string, string> = {}): void {
    // wie der Kern: gleiche Meldung nicht doppelt stapeln
    if (this.notices.some((n) => n.code === code && JSON.stringify(n.params) === JSON.stringify(params))) return;
    this.noticeId += 1;
    this.notices.push({ id: this.noticeId, code, params });
  }

  private finishDoc(result: DoneItemView['result']): void {
    const d = this.doc();
    if (!d) return;
    this.done.push({ id: d.id, name: d.name, input: d.path, result });
  }

  private doc(): MockDoc | null {
    return this.queue[0] ?? null;
  }

  private docInfo(d: MockDoc): DocInfo {
    return { id: d.id, name: d.name, page_count: d.sizes.length, page_sizes: d.sizes.map((s) => [s[0], s[1]]), signed: d.signed };
  }

  private queueView(): QueueItemView[] {
    return this.queue.map((d, i) => ({ id: d.id, name: d.name, current: i === 0 }));
  }

  private blockSize(): { width: number; height: number } {
    switch (this.settings.signature.profile) {
      case 'AMTSSIGNATURBLOCK':
        return { width: 340, height: 108 };
      case 'BASE_LOGO': {
        const s = mmToPt(this.settings.signature.logo_only_size_mm);
        return { width: s, height: s };
      }
      default:
        return { width: 232, height: 82 };
    }
  }

  private pageSize(d: MockDoc, page: number): [number, number] {
    return d.sizes[Math.min(page, d.sizes.length - 1)] ?? A4;
  }

  private stepView(): StepView {
    const d = this.doc();
    switch (this.stage) {
      case 'starting':
        return { step: 'starting' };
      case 'open':
        return { step: 'open', queue: this.queueView() };
      case 'loading':
        return { step: 'loading', queue: this.queueView() };
      case 'warning':
        if (!d) return { step: 'open', queue: [] };
        return { step: 'warning', doc: this.docInfo(d), codes: [...this.opts.warnings] };
      case 'placeholder':
        if (!d) return { step: 'open', queue: [] };
        return this.opts.placeholder === 'qr'
          ? { step: 'placeholder', doc: this.docInfo(d), fields: [], qr: { page: d.sizes.length - 1, id: '1' } }
          : {
              step: 'placeholder',
              doc: this.docInfo(d),
              fields: [
                { name: 'Unterschrift Auftraggeber', page: d.sizes.length - 1 },
                { name: 'Unterschrift Auftragnehmer', page: d.sizes.length - 1 },
              ],
              qr: null,
            };
      case 'position': {
        if (!d || !this.placement) return { step: 'open', queue: this.queueView() };
        const p = this.placement;
        return {
          step: 'position',
          doc: this.docInfo(d),
          page: this.page,
          block: { width: p.width, height: p.height, preview_revision: this.blockRevision },
          placement: { ...p },
          new_page: this.newPage,
          announce: {
            page: String(this.page + 1),
            pages: String(d.sizes.length + (this.newPage ? 1 : 0)),
            left: ((p.x * 25.4) / 72).toFixed(0),
            top: ((p.y * 25.4) / 72).toFixed(0),
          },
        };
      }
      case 'backend':
        return {
          step: 'backend',
          options: this.backendOptions(),
          privacy_notice: this.privacyFor ? { backend: this.privacyFor, version: 1, custom_text: null } : null,
        };
      case 'signing':
        return { step: 'signing', interaction: this.interaction, queue: this.queueView() };
      case 'output': {
        const result = {
          path: this.output.path,
          save_pending: this.output.savePending,
          ask_overwrite: this.output.askOverwrite,
          save_dialog: this.output.saveDialog ? { ...this.output.saveDialog } : null,
        };
        const next = this.queue[1];
        return { step: 'output', result, next: next ? this.docInfo(next) : null };
      }
      case 'error':
        return {
          step: 'error',
          code: this.error?.code ?? 'error-Unexpected',
          params: { ...(this.error?.params ?? {}) },
          retry: this.error?.retry ?? false,
        };
      case 'finished':
        return { step: 'finished', exit_code: this.exitCode };
    }
  }

  private bar(): MainBarView {
    const s = !this.settingsOpen;
    switch (this.stage) {
      case 'starting':
      case 'finished':
        return mainBar(['', '', '', ''], false);
      case 'open':
      case 'loading':
      case 'warning':
      case 'placeholder':
        return mainBar(['AE', '', '', ''], s);
      case 'position':
        return mainBar(['AE', 'A', '', ''], s);
      case 'backend':
        return mainBar(['AE', 'AE', 'A', ''], s);
      case 'signing':
        return mainBar(['A', 'A', 'A', ''], false);
      case 'output':
        return mainBar(['AE', 'AE', 'AE', 'A'], s);
      case 'error':
        return mainBar(['AE', '', '', ''], s);
    }
  }

  private backendOptions(): BackendOptionView[] {
    const ks = this.settings.keystore;
    const ksAvailable = ks.enabled && !!ks.file;
    const list: { id: BackendId; available: boolean }[] = [{ id: 'atrust_native', available: true }];
    // Mobile: keine lokale BKU (Plan §5.2)
    if (!this.mobile) list.push({ id: 'local_bku', available: this.opts.localBkuAvailable });
    if (ks.enabled || this.mobile) list.push({ id: 'keystore', available: ksAvailable });
    return list.map(({ id, available }) => {
      const caps = available ? capabilitiesFor(id) : null;
      const notes: string[] = [];
      if (this.mobile && id === 'atrust_native') {
        // Mobile: ID Austria ohne FIDO2 (Plan §4.7) – Hinweis-ID wie im Kern
        if (caps) caps.factors.fido2 = false;
        notes.push('ng-fido2-unavailable-mobile');
      }
      return { id, available, capabilities: caps, notes };
    });
  }

  // ------------------------------------------------------------------------------------------
  // Ereignisse
  // ------------------------------------------------------------------------------------------

  private handle(e: UiEvent): void {
    switch (e.type) {
      case 'open_files':
        return this.openFiles(e.paths);
      case 'answer_warning':
        if (this.stage !== 'warning') return;
        if (e.proceed) return this.afterAnalysis();
        this.finishDoc({ kind: 'cancelled' });
        return this.dropCurrent();
      case 'dismiss_notice':
        this.notices = this.notices.filter((n) => n.id !== e.id);
        return;
      case 'accept_migration':
        if (!this.migrationOffer) return;
        this.migrationOffer = null;
        this.notify('ng-migrate-done', { imported: '12', skipped: e.store_secrets ? '0' : '1' });
        return;
      case 'decline_migration':
        this.migrationOffer = null;
        return;
      case 'placeholder_answer':
        if (e.answer === 'Yes') {
          this.positionChoice = 'placeholder';
          return this.toBackend();
        }
        if (e.answer === 'No') return this.toPosition(true);
        this.finishDoc({ kind: 'cancelled' });
        return this.dropCurrent();
      case 'move_block':
        return this.move(e.page, e.x, e.y);
      case 'nudge_block': {
        if (!this.placement) return;
        const step = e.fine ? 1 : e.coarse ? 50 : 15;
        return this.move(this.page, this.placement.x + e.dx * step, this.placement.y + e.dy * step);
      }
      case 'change_page':
        return this.changePage(e.page);
      case 'toggle_new_page':
        return this.toggleNewPage();
      case 'auto_place':
        return this.autoPlace();
      case 'choose_invisible':
        this.positionChoice = 'invisible';
        return this.toBackend();
      case 'confirm_position':
        if (this.stage === 'position') {
          this.positionChoice = 'manual';
          this.toBackend();
        }
        return;
      case 'choose_backend':
        return this.chooseBackend(e.backend);
      case 'acknowledge_privacy': {
        const b = this.privacyFor;
        this.privacyFor = null;
        if (e.accepted && b) {
          this.settings = {
            ...this.settings,
            privacy: { acknowledged: { ...this.settings.privacy.acknowledged, [b]: 1 } },
          };
          this.startSigning(b);
        }
        return;
      }
      case 'submit_form':
        return this.submitForm(e.request, e.answer);
      case 'submit_secret':
        if (this.interaction.kind !== 'secret' || this.interaction.request !== e.request) return;
        if (e.secret === 'falsch') {
          this.interaction = { kind: 'secret', request: ++this.request, prompt: { ...this.interaction.prompt, retry: true } };
          return;
        }
        return this.progressThenFinish(['ReadingCertificate', 'Signing']);
      case 'answer_notice':
        if (this.interaction.kind !== 'notice' || this.interaction.request !== e.request) return;
        if (e.answer === 'Cancel') return this.cancelSigning();
        return this.showLogin();
      case 'open_signature_data':
        if (this.interaction.kind === 'form' && this.interaction.request === e.request) this.openedUrls.push(SIGNATURE_DATA_URL);
        return;
      case 'cancel':
        if (this.stage === 'signing') return this.cancelSigning();
        if (this.stage === 'loading' || this.stage === 'warning' || this.stage === 'position') {
          this.finishDoc({ kind: 'cancelled' });
          return this.dropCurrent();
        }
        if (this.stage === 'error') {
          const cancel = this.error?.onCancel;
          this.error = null;
          cancel?.();
        }
        return;
      case 'retry':
        if (this.stage === 'error') {
          const retry = this.error?.onRetry;
          this.error = null;
          retry?.();
        }
        return;
      case 'save':
        return this.save(e.path);
      case 'confirm_overwrite':
        if (!this.output.askOverwrite) return;
        if (e.overwrite) return this.saved(this.output.askOverwrite);
        this.output = { ...EMPTY_OUTPUT, savePending: true };
        return;
      case 'open_result':
        if (this.output.path) this.nativeCalls.push(`openResult:${this.output.path}`);
        return;
      case 'navigate':
        return this.navigate(e.step);
      case 'open_settings':
        this.settingsOpen = true;
        this.problems = {};
        return;
      case 'close_settings':
        this.settingsOpen = false;
        this.problems = {};
        return;
      case 'save_settings':
        return this.saveSettings(e.settings);
      case 'reset_settings':
        this.settings = defaultSettings();
        this.settingsOpen = false;
        this.problems = {};
        return;
      case 'backgrounded':
      case 'foregrounded':
        return;
      case 'share_result':
        if (this.output.path) this.nativeCalls.push(`shareResult:${this.output.path}`);
        return;
      case 'export_result':
        if (this.output.path) {
          this.nativeCalls.push(`exportResult:${this.output.path}`);
          this.notify('ng-export-done', { name: baseName(this.output.path) });
        }
        return;
      case 'resume_signature': {
        const offer = this.resumeOffer;
        if (!offer || this.stage !== 'open') return;
        this.resumeOffer = null;
        return this.openFiles([`/var/mobile/Containers/Data/Application/DEMO/Library/tmp/resume/document/${offer.name}`]);
      }
      case 'discard_resume':
        this.resumeOffer = null;
        return;
      case 'import_keystore':
        return this.importKeystore(e.path, e.password, e.remember);
      case 'dismiss_keystore_import':
        this.keystoreImport = null;
        return;
      case 'quit':
        this.stage = 'finished';
        this.exitCode = 0;
        return;
    }
  }

  /** PKCS#12-Import (nur Simulation: Endung, Passwort „falsch“ = falsches Passwort). */
  private importKeystore(path: string, password: string | null, remember: boolean): void {
    const fileName = baseName(path);
    if (!/\.(p12|pfx|pkcs12)$/i.test(fileName)) {
      this.keystoreImport = { file_name: fileName, ok: false, code: 'ng-keystore-import-format', password_stored: false };
      return;
    }
    if (password === 'falsch') {
      this.keystoreImport = { file_name: fileName, ok: false, code: 'keystore-wrong-password', password_stored: false };
      return;
    }
    const stored = remember && !!password && this.opts.keychainAvailable;
    const file = `/var/mobile/Containers/Data/Application/DEMO/Library/Application Support/data/keystores/${fileName}`;
    this.settings = {
      ...this.settings,
      keystore: {
        ...this.settings.keystore,
        enabled: true,
        file,
        declared_type: 'PKCS12',
        password_storage: stored ? 'keychain' : this.settings.keystore.password_storage === 'keychain' ? 'memory' : this.settings.keystore.password_storage,
      },
    };
    this.keystoreImport = { file_name: fileName, ok: true, code: 'ng-keystore-imported', password_stored: stored };
  }

  private openFiles(paths: string[]): void {
    const before = this.queue.length;
    for (const path of paths) {
      if (this.queue.some((d) => d.path === path)) continue; // F-89 ohne Duplikate
      const id = this.nextDocId++;
      const sizes: [number, number][] = id % 3 === 0 ? [A4, A4_LANDSCAPE] : [A4, A4, A4];
      this.queue.push({ id, path, name: baseName(path), sizes, signed: false });
    }
    if (before === 0 && this.queue.length > 0 && (this.stage === 'open' || this.stage === 'error')) {
      this.analyze();
    } else if (this.stage === 'output' && this.output.path && before <= 1 && this.queue.length > 1) {
      // F-89: auf dem Ergebnis-Screen abgelegt und zuletzt gespeichert → sofort weiter
      this.finishDoc({ kind: 'signed', output: { path: this.output.path, len: 1024, sha256: '00' } });
      this.queue.shift();
      this.analyze();
    }
  }

  private analyze(): void {
    const d = this.doc();
    if (!d) {
      this.stage = 'open';
      return;
    }
    // Laden und Analyse (Platzhaltersuche) – Schritt `loading`
    this.stage = 'loading';
    this.later(() => {
      if (this.stage !== 'loading' || this.doc() !== d) return;
      if (!/\.pdf$/i.test(d.name)) {
        this.fail('error-MayNotBeAPDF', {}, true, () => this.analyze(), () => this.dropCurrent());
      } else if (this.opts.warnings.length > 0) {
        this.stage = 'warning';
      } else {
        this.afterAnalysis();
      }
      this.emit();
    });
  }

  /** Nach der Analyse (bzw. „Trotzdem signieren“): Platzhalter, Positionierung oder Signiermittel. */
  private afterAnalysis(): void {
    if (this.opts.placeholder !== 'none' && this.settings.placeholder.search) {
      this.stage = 'placeholder';
    } else if (this.settings.signature.auto_position || this.settings.signature.profile === 'INVISIBLE') {
      this.positionChoice = this.settings.signature.profile === 'INVISIBLE' ? 'invisible' : 'auto';
      this.toBackend();
    } else {
      this.toPosition(false);
    }
  }

  private toPosition(keep: boolean): void {
    const d = this.doc();
    if (!d) return;
    const size = this.blockSize();
    if (!keep || !this.placement) {
      // F-32/F-33: letzte Seite, linke obere Ecke bei 50 % Breite / 25 % Höhe
      this.page = d.sizes.length - 1;
      const [pw, ph] = this.pageSize(d, this.page);
      this.placement = { page_index: this.page, x: pw * 0.5, y: ph * 0.25, width: size.width, height: size.height };
      this.newPage = false;
    } else {
      this.placement = { ...this.placement, width: size.width, height: size.height };
    }
    this.clampPlacement();
    this.stage = 'position';
  }

  private clampPlacement(): void {
    const d = this.doc();
    if (!d || !this.placement) return;
    const [pw, ph] = this.pageSize(d, this.page);
    const p = this.placement;
    // F-35 mit KR-F11: Block größer als Seite → links/oben anliegend
    const x = Math.max(0, Math.min(p.x, pw - p.width));
    const y = Math.min(ph, Math.max(p.y, p.height));
    this.placement = { ...p, page_index: this.page, x, y };
  }

  private move(page: number, x: number, y: number): void {
    if (this.stage !== 'position' || !this.placement) return;
    if (page !== this.page) this.changePage(page);
    this.placement = { ...this.placement, x, y };
    this.clampPlacement();
  }

  private changePage(page: number): void {
    const d = this.doc();
    if (!d || this.stage !== 'position') return;
    const last = d.sizes.length - 1 + (this.newPage ? 1 : 0);
    this.page = Math.max(0, Math.min(last, Math.trunc(page)));
    this.clampPlacement();
  }

  private toggleNewPage(): void {
    const d = this.doc();
    if (!d || this.stage !== 'position') return;
    this.newPage = !this.newPage;
    this.page = this.newPage ? d.sizes.length : d.sizes.length - 1;
    this.clampPlacement();
  }

  private autoPlace(): void {
    const d = this.doc();
    if (!d || !this.placement) return;
    this.newPage = false;
    this.page = d.sizes.length - 1;
    const [pw] = this.pageSize(d, this.page);
    // Simulation: unter dem „untersten Inhalt“ mit Rand 50 pt
    this.placement = { ...this.placement, x: Math.max(0, (pw - this.placement.width) / 2), y: 230 };
    this.clampPlacement();
    this.notify('ng-ui-position-auto-done');
  }

  private toBackend(): void {
    const def = this.settings.backend.default;
    const options = this.backendOptions();
    const available = options.filter((o) => o.available).map((o) => o.id);
    this.backend = null;
    this.privacyFor = null;
    if (def) {
      const id: BackendId = def === 'id_austria' ? 'atrust_native' : def;
      if (available.includes(id)) return this.chooseBackend(id); // F-53
    }
    if (available.length === 1 && available[0] === 'atrust_native') return this.chooseBackend('atrust_native'); // F-54
    this.stage = 'backend';
  }

  private chooseBackend(id: BackendId): void {
    const option = this.backendOptions().find((o) => o.id === id);
    if (!option?.available) {
      this.stage = 'backend';
      return;
    }
    this.backend = id;
    if (option.capabilities?.document_leaves_device && !((this.settings.privacy.acknowledged[id] ?? 0) >= 1)) {
      this.stage = 'backend';
      this.privacyFor = id;
      return;
    }
    this.startSigning(id);
  }

  private ctx(extra: Partial<FormContext> = {}): FormContext {
    return {
      signature_data_url: SIGNATURE_DATA_URL,
      can_switch_to_sms: true,
      can_switch_to_fido2: this.opts.fido2Available,
      fido2_visible: this.opts.fido2Available,
      problem: null,
      ...extra,
    };
  }

  private form(form: NativeForm): void {
    this.interaction = { kind: 'form', request: ++this.request, form };
  }

  private startSigning(id: BackendId): void {
    this.stage = 'signing';
    this.interaction = { kind: 'waiting', progress: 'Connecting' };
    this.later(() => {
      if (this.stage !== 'signing') return;
      if (id === 'keystore') {
        this.interaction = {
          kind: 'secret',
          request: ++this.request,
          prompt: { kind: 'KeystorePassword', subject: baseName(this.settings.keystore.file ?? 'keystore.p12'), retry: false },
        };
      } else if (id === 'local_bku') {
        this.progressThenFinish(['ReadingCertificate', 'Signing']);
        return;
      } else if (this.loggedIn && this.rememberedPassword) {
        this.afterLogin();
      } else {
        this.showLogin();
      }
      this.emit();
    });
  }

  private showLogin(problem: FormContext['problem'] = null): void {
    this.form({
      Login: {
        prefill_username: this.settings.id_austria.username,
        remember: this.settings.id_austria.remember,
        ctx: this.ctx({ problem, can_switch_to_sms: false, can_switch_to_fido2: false, fido2_visible: false }),
      },
    });
  }

  private afterLogin(): void {
    const flow = this.settings.id_austria.fido2_default && this.opts.fido2Available ? 'fido2' : this.opts.signingFlow;
    switch (flow) {
      case 'sms':
        return this.form({ SmsTan: { reference_value: 'Xk7Pq', ctx: this.ctx() } });
      case 'qr':
        return this.form({
          QrCode: { reference_value: 'Xk7Pq', image: { bytes: demoQrBytes(), mime: 'image/svg+xml' }, timeout_secs: 300, ctx: this.ctx() },
        });
      case 'fido2':
        this.form({ Fido2: { in_progress: true, ctx: this.ctx({ can_switch_to_fido2: false }) } });
        this.later(() => {
          if (this.interaction.kind === 'form' && 'Fido2' in this.interaction.form) {
            this.form({ Fido2: { in_progress: false, ctx: this.ctx({ can_switch_to_fido2: false }) } });
            this.emit();
          }
        }, 4);
        return;
      default:
        this.form({ WaitForApp: { reference_value: 'Xk7Pq', timeout_secs: 300, ctx: this.ctx() } });
        if (this.opts.autoCompleteApp) {
          const req = this.request;
          this.later(() => {
            if (this.interaction.kind === 'form' && this.interaction.request === req) {
              this.finishSigning();
              this.emit();
            }
          }, 10);
        }
    }
  }

  private submitForm(request: number, answer: import('./types').FormAnswerView): void {
    if (this.interaction.kind !== 'form' || this.interaction.request !== request) return;
    const form = this.interaction.form;
    switch (answer.kind) {
      case 'login': {
        if (answer.password.length < 5) return this.showLogin('PasswordTooShort');
        if (answer.password.length > 200) return this.showLogin('PasswordTooLong');
        if (answer.password === 'falsch') return this.showLogin('LoginRejected');
        this.loggedIn = true;
        this.rememberedPassword = answer.remember;
        // BackendEvent::LoginSucceeded → Benutzername merken (Passwort nie)
        this.settings = {
          ...this.settings,
          id_austria: {
            ...this.settings.id_austria,
            remember: answer.remember,
            username: answer.remember ? answer.username : this.settings.id_austria.username,
          },
        };
        this.interaction = { kind: 'waiting', progress: 'WaitingForServer' };
        this.later(() => {
          this.afterLogin();
          this.emit();
        });
        return;
      }
      case 'tan':
        if (answer.tan === '000000') {
          return this.form({ SmsTan: { reference_value: 'Xk7Pq', ctx: this.ctx({ problem: { Atrust: 'TAN falsch' } }) } });
        }
        return this.progressThenFinish(['Signing']);
      case 'switch_to_sms':
        return this.form({ SmsTan: { reference_value: 'Xk7Pq', ctx: this.ctx() } });
      case 'switch_to_fido2':
        return this.form({ Fido2: { in_progress: false, ctx: this.ctx({ can_switch_to_fido2: false }) } });
      case 'start_fido2':
        this.form({ Fido2: { in_progress: true, ctx: this.ctx({ can_switch_to_fido2: false }) } });
        this.later(() => {
          this.progressThenFinish(['Signing']);
          this.emit();
        }, 3);
        return;
      case 'continue':
        if ('QrCode' in form || 'WaitForApp' in form) return this.progressThenFinish(['Signing']);
        return;
    }
  }

  private progressThenFinish(steps: import('./types').Progress[]): void {
    const [first, ...rest] = steps;
    if (!first) {
      this.finishSigning();
      return;
    }
    this.interaction = { kind: 'waiting', progress: first };
    this.later(() => {
      if (this.stage !== 'signing') return;
      this.progressThenFinish(rest);
      this.emit();
    });
  }

  private cancelSigning(): void {
    // F-72: Abbruch → zurück zur Auswahl; gemerktes Passwort vergessen, Benutzername bleibt
    this.rememberedPassword = false;
    this.loggedIn = false;
    this.stage = 'backend';
    this.privacyFor = null;
  }

  private finishSigning(): void {
    const d = this.doc();
    if (!d) return;
    d.signed = true;
    this.stage = 'output';
    const name = outputFileName(d.path, this.settings.output.postfix || '_signed');
    // Mobile: ohne Einstellung in die App-Dokumente (CoreOptions::default_output_dir), kein Dialog
    const folder = this.settings.output.folder ?? (this.mobile ? '/var/mobile/Containers/Data/Application/DEMO/Documents' : null);
    if (folder) {
      const target = joinPath(folder === '.' ? dirName(d.path) : folder, name);
      if (this.opts.existingFiles.includes(target)) {
        this.output = { ...EMPTY_OUTPUT, askOverwrite: target };
      } else {
        this.saved(target);
      }
    } else {
      this.output = { ...EMPTY_OUTPUT, saveDialog: { directory: dirName(d.path), file_name: name } };
    }
  }

  private save(path: string | null): void {
    if (this.stage !== 'output') return;
    if (path === null) {
      if (this.output.saveDialog) {
        // Dialog abgebrochen → Knopf „Speichern“ (F-86)
        this.output = { ...EMPTY_OUTPUT, savePending: true };
      } else {
        // Knopf „Speichern“ → Speichern-Dialog (F-86)
        const d = this.doc();
        const name = outputFileName(d?.path ?? 'signiert.pdf', this.settings.output.postfix || '_signed');
        this.output = { ...EMPTY_OUTPUT, saveDialog: { directory: dirName(d?.path ?? ''), file_name: name } };
      }
      return;
    }
    if (this.opts.existingFiles.includes(path)) {
      this.output = { ...EMPTY_OUTPUT, askOverwrite: path };
      return;
    }
    this.saved(path);
  }

  private saved(path: string): void {
    this.output = { ...EMPTY_OUTPUT, path };
    this.stage = 'output';
    if (this.queue.length > 1) {
      // F-88: Batch – nächstes Dokument ohne Ergebnis-Screen
      this.notify('ng-ui-output-saved-next', { file: baseName(path) });
      this.finishDoc({ kind: 'signed', output: { path, len: 1024, sha256: '00' } });
      this.queue.shift();
      this.analyze();
    }
  }

  private dropCurrent(): void {
    this.queue.shift();
    if (this.queue.length > 0) this.analyze();
    else this.stage = 'open';
  }

  private navigate(step: (typeof MAIN_BAR_ORDER)[number]): void {
    switch (step) {
      case 'Open':
        if (this.stage === 'output') {
          // F-16: aus dem Ergebnis das nächste Dokument
          this.finishDoc(
            this.output.path ? { kind: 'signed', output: { path: this.output.path, len: 1024, sha256: '00' } } : { kind: 'cancelled' },
          );
          this.queue.shift();
          this.output = EMPTY_OUTPUT;
          if (this.queue.length > 0) this.analyze();
          else this.stage = 'open';
        } else if (this.stage !== 'open') {
          this.queue = [];
          this.stage = 'open';
        }
        return;
      case 'Positioning':
        if (this.stage === 'backend' || this.stage === 'output') this.toPosition(true); // F-43, F-91
        return;
      case 'Signing':
        if (this.stage === 'output') this.toBackend(); // F-55
        return;
      case 'Finish':
        return;
    }
  }

  private saveSettings(next: Settings): void {
    const problems: Record<string, string> = {};
    if (next.proxy.mode === 'manual' && !next.proxy.host) problems['proxy.host'] = 'ng-ui-problem-required';
    if (next.proxy.port !== null && (next.proxy.port < 1 || next.proxy.port > 65535)) problems['proxy.port'] = 'ng-ui-problem-port';
    if (next.keystore.enabled && !next.keystore.file) problems['keystore.file'] = 'ng-ui-problem-required';
    if (Object.keys(problems).length > 0) {
      this.problems = problems;
      return;
    }
    this.settings = structuredClone(next);
    if (!(this.settings.signature.logo_only_size_mm > 0)) this.settings.signature.logo_only_size_mm = 23;
    this.settingsOpen = false;
    this.problems = {};
    this.blockRevision += 1;
    if (this.stage === 'position') this.toPosition(true);
  }

  private fail(code: string, params: Record<string, string>, retry: boolean, onRetry: () => void, onCancel: () => void): void {
    this.stage = 'error';
    this.error = { code, params, retry, onRetry, onCancel };
  }
}

/** Mock-Kern als Bridge. */
export function createMockBridge(options?: MockOptions): MockCore {
  return new MockCore(options);
}
