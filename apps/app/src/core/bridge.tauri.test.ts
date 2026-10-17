/**
 * Tauri-Bridge mit nachgebildeter Laufzeit (ADR 0004 Nachtrag 17, Mehrfachauswahl): `pickFiles` reicht
 * `multiple` an `@tauri-apps/plugin-dialog` (`open`, Rust: rfd `pick_files` → NSOpenPanel mit
 * `allowsMultipleSelection`) durch und gibt **alle** Pfade in der Reihenfolge des Dialogs zurück (eine einzelne
 * Zeichenkette wird zur Liste, Abbruch bleibt `null`); die Mobile-Auswahl liefert alle Kopien und die Zahl der
 * nicht übernommenen (Command `mobile_pick_files`), `pickMobileFiles` davon nur die Pfade.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls = vi.hoisted(() => ({ open: [] as unknown[], invoke: [] as Array<[string, unknown]>, openResult: null as unknown, invokeResult: null as unknown }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async (cmd: string, args: unknown) => {
    calls.invoke.push([cmd, args]);
    if (calls.invokeResult instanceof Error) throw calls.invokeResult.message;
    return calls.invokeResult;
  },
  convertFileSrc: (path: string, protocol: string) => `${protocol}://localhost/${encodeURIComponent(path)}`,
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ onDragDropEvent: async () => () => undefined }) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: async (options: unknown) => {
    calls.open.push(options);
    return calls.openResult;
  },
  save: async () => null,
}));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: async () => undefined }));

import { createTauriBridge } from './bridge';

const FILTERS = [
  { name: 'PDF-Dokumente', extensions: ['pdf'] },
  { name: 'Alle Dateien', extensions: ['*'] },
];

beforeEach(() => {
  calls.open.length = 0;
  calls.invoke.length = 0;
  calls.openResult = null;
  calls.invokeResult = null;
});

describe('Tauri-Bridge: Mehrfachauswahl', () => {
  it('pickFiles: `multiple` an den Dialog, alle Pfade in Dialogreihenfolge zurück', async () => {
    const bridge = await createTauriBridge();
    calls.openResult = ['/Users/x/c.pdf', '/Users/x/a.pdf', '/Users/x/b.pdf'];
    const paths = await bridge.pickFiles({ title: 'PDF-Dokumente wählen', filters: FILTERS, multiple: true });
    expect(paths).toEqual(['/Users/x/c.pdf', '/Users/x/a.pdf', '/Users/x/b.pdf']);
    expect(calls.open).toEqual([{ title: 'PDF-Dokumente wählen', filters: FILTERS, multiple: true, directory: false, defaultPath: undefined }]);
  });

  it('pickFiles: einzelne Zeichenkette (Einzelauswahl) wird zur Liste, Abbruch bleibt null', async () => {
    const bridge = await createTauriBridge();
    calls.openResult = '/Users/x/a.pdf';
    expect(await bridge.pickFiles({ title: 't', filters: FILTERS, multiple: false })).toEqual(['/Users/x/a.pdf']);
    calls.openResult = null;
    expect(await bridge.pickFiles({ title: 't', filters: FILTERS, multiple: true })).toBeNull();
    expect(calls.open.map((o) => (o as { multiple: boolean }).multiple)).toEqual([false, true]);
  });

  it('pickMobileDocuments: alle Kopien und die Zahl der nicht übernommenen; pickMobileFiles nur Pfade', async () => {
    const bridge = await createTauriBridge();
    calls.invokeResult = { paths: ['/c/import/1/A.pdf', '/c/import/2/B.pdf', '/c/import/3/C.pdf'], skipped: 1 };
    expect(await bridge.pickMobileDocuments({ multiple: true })).toEqual({ paths: ['/c/import/1/A.pdf', '/c/import/2/B.pdf', '/c/import/3/C.pdf'], skipped: 1 });
    // nur PDFs (seit ADR 0004 Nachtrag 11 kein PKCS#12-Import auf Mobilgeräten)
    expect(await bridge.pickMobileFiles({ kind: 'pdf', multiple: false })).toEqual(['/c/import/1/A.pdf', '/c/import/2/B.pdf', '/c/import/3/C.pdf']);
    expect(calls.invoke.filter(([cmd]) => cmd === 'mobile_pick_files')).toEqual([
      ['mobile_pick_files', { kind: 'pdf', multiple: true }],
      ['mobile_pick_files', { kind: 'pdf', multiple: false }],
    ]);
    calls.invokeResult = new Error('ng-mobile-not-implemented');
    await expect(bridge.pickMobileDocuments({ multiple: true })).rejects.toThrow('ng-mobile-not-implemented');
  });
});
