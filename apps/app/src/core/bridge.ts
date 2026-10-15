/**
 * Bridge zwischen Oberfläche und Kern (Plan §3.4 `CoreBridge`) plus die nativen Dienste der Shell.
 *
 * Die Oberfläche hält **keinen Fachzustand**: Sie projiziert den `ViewState` und sendet `UiEvent`s.
 * Zwei Implementierungen: `TauriBridge` (hier; Commands `core_view`/`core_dispatch`, Ereignis
 * `pdfover:view`, Custom Protocol `pdfover-render://`) und der TS-Mock-Kern (`mock.ts`, für
 * `VITE_MOCK=1`, Browser-Vorschau und Tests).
 */
import type { DocId, UiEvent, ViewState } from './types';

/** Name des Ereignisses mit dem aktuellen ViewState (`pdfover_app_lib::VIEW_EVENT`). */
export const VIEW_EVENT = 'pdfover:view';
/** Schema des Custom Protocols (`pdfover_app_lib::protocol::SCHEME`). */
export const RENDER_SCHEME = 'pdfover-render';

/** Dateifilter eines Dialogs. */
export interface FileFilter {
  name: string;
  extensions: string[];
}

/** Ziehen von Dateien über das Fenster (Tauri `onDragDropEvent`; Position in CSS-Pixeln). */
export type FileDragEvent =
  | { type: 'enter'; paths: string[]; x: number; y: number }
  | { type: 'over'; x: number; y: number }
  | { type: 'drop'; paths: string[]; x: number; y: number }
  | { type: 'leave' };

/** Angaben für den Über-Dialog. */
export interface AppInfo {
  version: string;
  platform: string;
  core_available: boolean;
}

/** Kern-Schnittstelle (Plan §3.4). */
export interface CoreBridge {
  /** `tauri` (Rust-Kern) oder `mock` (TS-Mock-Kern). */
  readonly kind: 'tauri' | 'mock';
  /** Aktueller Zustand (beim Start). */
  view(): Promise<ViewState>;
  /** Sendet ein Ereignis; Fehler tragen einen stabilen Code (i18n-ID). */
  dispatch(event: UiEvent): Promise<void>;
  /** Abo auf Zustandsänderungen; liefert die Abmeldung. */
  onState(cb: (view: ViewState) => void): () => void;
  /** URL einer Seitenkachel (`scale` relativ zu 72 dpi). */
  pageTileUrl(doc: DocId, page: number, scale: number): string;
  /** URL der Blockvorschau (Revision = Cache-Busting). */
  blockPreviewUrl(scale: number, revision: number): string;
}

/** Native Dienste (Dialoge, Drag & Drop, Öffnen im System). */
export interface PlatformServices {
  /** Dateidialog mit Mehrfachauswahl (F-11); `null` = abgebrochen. */
  pickFiles(options: { title: string; filters: FileFilter[]; defaultPath?: string | null; multiple: boolean }): Promise<string[] | null>;
  /** Speichern-Dialog (F-83); `null` = abgebrochen. */
  pickSavePath(options: { title: string; filters: FileFilter[]; defaultPath?: string | null }): Promise<string | null>;
  /** Ordnerdialog; `null` = abgebrochen. */
  pickDirectory(options: { title: string; defaultPath?: string | null }): Promise<string | null>;
  /**
   * Mobile: System-Dateiauswahl über das Plugin `pdfover-mobile` (Dokument-Picker bzw. SAF); die Dateien
   * liegen danach als Kopien in der App-Sandbox. `[]` = abgebrochen. Fehler tragen einen Code
   * (`ng-mobile-not-implemented`, `ng-mobile-unsupported` …).
   */
  pickMobileFiles(options: { kind: 'pdf' | 'pkcs12'; multiple: boolean }): Promise<string[]>;
  /** Drag & Drop von Dateien über das Fenster. */
  onFileDrag(cb: (event: FileDragEvent) => void): () => void;
  /** Öffnet eine `https:`/`mailto:`-URL im Systembrowser. */
  openUrl(url: string): Promise<void>;
  /** Zeigt die Ergebnisdatei des Ergebnis-Schritts im Dateimanager. */
  revealOutput(): Promise<void>;
  /** Öffnet den Log-Ordner. */
  openLogDir(): Promise<void>;
  /** Öffnet die Drittlizenz-Hinweise des Pakets (THIRD-PARTY-NOTICES). */
  openNotices(): Promise<void>;
  /** Version, Plattform, Kern verfügbar. */
  appInfo(): Promise<AppInfo>;
}

/** Bridge + Dienste. */
export type Bridge = CoreBridge & PlatformServices;

/** Läuft die Oberfläche in Tauri? */
export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Nur `https:` und `mailto:` gehen an den Systembrowser (Plan §7.7). */
export function isAllowedExternalUrl(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === 'https:' || u.protocol === 'mailto:';
  } catch {
    return false;
  }
}

/** Maßstab für Kacheln: auf 0,25 gerundet (weniger Neuberechnungen), begrenzt wie im Protokoll. */
export function tileScale(scale: number): number {
  const rounded = Math.round(scale * 4) / 4;
  return Math.min(8, Math.max(0.25, rounded));
}

/** Basis-URL des Custom Protocols je Plattform aus dem User-Agent (Rückfall, wie `convertFileSrc`). */
export function renderBase(userAgent: string): string {
  const windowsLike = /Windows|Android/i.test(userAgent);
  return windowsLike ? `http://${RENDER_SCHEME}.localhost/` : `${RENDER_SCHEME}://localhost/`;
}

/**
 * Basis-URL des Custom Protocols: maßgeblich ist `convertFileSrc` der Tauri-Laufzeit (kennt Plattform
 * und `useHttpsScheme`); nur wenn sie fehlt oder Unerwartetes liefert, der User-Agent.
 */
export function resolveRenderBase(convert: ((path: string, protocol: string) => string) | null, userAgent: string): string {
  try {
    const base = convert?.('', RENDER_SCHEME);
    if (typeof base === 'string' && base.endsWith('/') && base.includes(RENDER_SCHEME)) return base;
  } catch {
    /* Rückfall */
  }
  return renderBase(userAgent);
}

/** URL einer Seitenkachel relativ zur Basis. */
export function pageTilePath(doc: DocId, page: number, scale: number): string {
  return `page/${encodeURIComponent(String(doc))}/${encodeURIComponent(String(page))}?scale=${tileScale(scale)}`;
}

/** URL der Blockvorschau relativ zur Basis. */
export function blockPreviewPath(scale: number, revision: number): string {
  return `block?scale=${tileScale(scale)}&rev=${encodeURIComponent(String(revision))}`;
}

/** Fehlercode aus einer abgelehnten Tauri-Promise (Commands liefern stabile Codes als String). */
export function errorCode(error: unknown): string {
  if (typeof error === 'string' && /^[a-z0-9][a-z0-9_.-]*$/i.test(error)) return error;
  return 'ng-ui-bridge-error';
}

/** Bridge zum Rust-Kern über Tauri. */
export async function createTauriBridge(): Promise<Bridge> {
  const [{ invoke, convertFileSrc }, { listen }, { getCurrentWebview }, dialog, opener] = await Promise.all([
    import('@tauri-apps/api/core'),
    import('@tauri-apps/api/event'),
    import('@tauri-apps/api/webview'),
    import('@tauri-apps/plugin-dialog'),
    import('@tauri-apps/plugin-opener'),
  ]);
  const base = resolveRenderBase(convertFileSrc, navigator.userAgent);
  const listeners = new Set<(view: ViewState) => void>();
  await listen<ViewState>(VIEW_EVENT, (event) => {
    for (const cb of listeners) cb(event.payload);
  });

  const asPaths = (value: string | string[] | null): string[] | null => {
    if (value === null) return null;
    return Array.isArray(value) ? value : [value];
  };

  return {
    kind: 'tauri',
    view: () => invoke<ViewState>('core_view'),
    async dispatch(event) {
      try {
        await invoke('core_dispatch', { event });
      } catch (e) {
        throw new Error(errorCode(e));
      }
    },
    onState(cb) {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    pageTileUrl: (doc, page, scale) => base + pageTilePath(doc, page, scale),
    blockPreviewUrl: (scale, revision) => base + blockPreviewPath(scale, revision),

    async pickFiles({ title, filters, defaultPath, multiple }) {
      const result = await dialog.open({ title, filters, multiple, directory: false, defaultPath: defaultPath ?? undefined });
      return asPaths(result);
    },
    async pickSavePath({ title, filters, defaultPath }) {
      return dialog.save({ title, filters, defaultPath: defaultPath ?? undefined });
    },
    async pickDirectory({ title, defaultPath }) {
      const result = await dialog.open({ title, directory: true, multiple: false, defaultPath: defaultPath ?? undefined });
      return typeof result === 'string' ? result : null;
    },
    async pickMobileFiles({ kind, multiple }) {
      try {
        return await invoke<string[]>('mobile_pick_files', { kind, multiple });
      } catch (e) {
        throw new Error(errorCode(e));
      }
    },
    onFileDrag(cb) {
      let unlisten: (() => void) | null = null;
      let cancelled = false;
      const ratio = () => window.devicePixelRatio || 1;
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          const p = event.payload;
          switch (p.type) {
            case 'enter':
              cb({ type: 'enter', paths: p.paths, x: p.position.x / ratio(), y: p.position.y / ratio() });
              break;
            case 'over':
              cb({ type: 'over', x: p.position.x / ratio(), y: p.position.y / ratio() });
              break;
            case 'drop':
              cb({ type: 'drop', paths: p.paths, x: p.position.x / ratio(), y: p.position.y / ratio() });
              break;
            default:
              cb({ type: 'leave' });
          }
        })
        .then((fn) => {
          if (cancelled) fn();
          else unlisten = fn;
        });
      return () => {
        cancelled = true;
        unlisten?.();
      };
    },
    async openUrl(url) {
      if (!isAllowedExternalUrl(url)) throw new Error('ng-ui-open-failed');
      await opener.openUrl(url);
    },
    async revealOutput() {
      try {
        await invoke('reveal_output');
      } catch (e) {
        throw new Error(errorCode(e));
      }
    },
    async openLogDir() {
      try {
        await invoke('open_log_dir');
      } catch (e) {
        throw new Error(errorCode(e));
      }
    },
    async openNotices() {
      try {
        await invoke('open_notices');
      } catch (e) {
        throw new Error(errorCode(e));
      }
    },
    appInfo: () => invoke<AppInfo>('app_info'),
  };
}
