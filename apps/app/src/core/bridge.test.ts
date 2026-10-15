import { describe, expect, it } from 'vitest';
import { blockPreviewPath, errorCode, inTauri, isAllowedExternalUrl, pageTilePath, renderBase, resolveRenderBase, tileScale } from './bridge';
import { mainBar, mmToPt, ptToMm, withSection, defaultSettings, baseName, dirName } from './defaults';
import { MockCore } from './mock';

describe('Bridge-Hilfen', () => {
  it('Custom Protocol je Plattform wie convertFileSrc', () => {
    expect(renderBase('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit')).toBe('pida-render://localhost/');
    expect(renderBase('Mozilla/5.0 (X11; Linux x86_64)')).toBe('pida-render://localhost/');
    expect(renderBase('Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/140')).toBe('http://pida-render.localhost/');
    expect(renderBase('Mozilla/5.0 (Linux; Android 15)')).toBe('http://pida-render.localhost/');
  });

  it('Basis-URL kommt aus convertFileSrc der Tauri-Laufzeit, sonst aus dem User-Agent', () => {
    const tauriWindows = (path: string, protocol: string) => `https://${protocol}.localhost/${encodeURIComponent(path)}`;
    const tauriMac = (path: string, protocol: string) => `${protocol}://localhost/${encodeURIComponent(path)}`;
    expect(resolveRenderBase(tauriWindows, 'Macintosh')).toBe('https://pida-render.localhost/');
    expect(resolveRenderBase(tauriMac, 'Windows NT 10.0')).toBe('pida-render://localhost/');
    expect(resolveRenderBase(null, 'Windows NT 10.0')).toBe('http://pida-render.localhost/');
    expect(
      resolveRenderBase(() => {
        throw new Error('ohne Tauri');
      }, 'X11; Linux'),
    ).toBe('pida-render://localhost/');
    expect(resolveRenderBase(() => 'asset://localhost/', 'X11; Linux')).toBe('pida-render://localhost/');
  });

  it('Pfade für Kacheln und Blockvorschau (Maßstab gerundet und begrenzt)', () => {
    expect(pageTilePath(7, 2, 1.37)).toBe('page/7/2?scale=1.25');
    expect(pageTilePath(1, 0, 100)).toBe('page/1/0?scale=8');
    expect(blockPreviewPath(0.01, 3)).toBe('block?scale=0.25&rev=3');
    expect(tileScale(2.126)).toBe(2.25);
  });

  it('nur https: und mailto: gehen nach außen', () => {
    expect(isAllowedExternalUrl('https://www.a-trust.at/x')).toBe(true);
    expect(isAllowedExternalUrl('mailto:software@egiz.gv.at')).toBe(true);
    expect(isAllowedExternalUrl('http://example.invalid')).toBe(false);
    expect(isAllowedExternalUrl('file:///etc/passwd')).toBe(false);
    expect(isAllowedExternalUrl('javascript:alert(1)')).toBe(false);
    expect(isAllowedExternalUrl('kein url')).toBe(false);
  });

  it('Fehlercodes aus Commands, sonst allgemeiner Code (keine Fremdtexte)', () => {
    expect(errorCode('ng-ui-core-unavailable')).toBe('ng-ui-core-unavailable');
    expect(errorCode('render.failed')).toBe('render.failed');
    expect(errorCode('Fehler mit Leerzeichen <script>')).toBe('ng-ui-bridge-error');
    expect(errorCode({ message: 'x' })).toBe('ng-ui-bridge-error');
  });

  it('außerhalb von Tauri', () => {
    expect(inTauri()).toBe(false);
  });
});

describe('Standardwerte und Hilfen', () => {
  it('Hauptleiste aus der Tabelle docs/analyse/01 §3.5', () => {
    expect(mainBar(['AE', 'A', '', ''], true)).toEqual({
      steps: [
        { step: 'Open', active: true, enabled: true },
        { step: 'Positioning', active: true, enabled: false },
        { step: 'Signing', active: false, enabled: false },
        { step: 'Finish', active: false, enabled: false },
      ],
      settings_enabled: true,
    });
  });

  it('mm ↔ pt', () => {
    expect(ptToMm(72)).toBeCloseTo(25.4);
    expect(mmToPt(25.4)).toBeCloseTo(72);
  });

  it('withSection erhält unbekannte Einträge', () => {
    const s = { ...defaultSettings(), 'x.y': 1 };
    const next = withSection(s, 'proxy', { mode: 'none' });
    expect(next['x.y']).toBe(1);
    expect(next.proxy.mode).toBe('none');
    expect(s.proxy.mode).toBe('system');
  });

  it('Pfadteile (/ und \\)', () => {
    expect(baseName('/a/b/c.pdf')).toBe('c.pdf');
    expect(baseName('C:\\a\\c.pdf')).toBe('c.pdf');
    expect(dirName('/a/b/c.pdf')).toBe('/a/b');
    expect(dirName('c.pdf')).toBe('');
  });
});

describe('Mock-Kern', () => {
  it('startet im Öffnen-Schritt und liefert serde-nahe Zustände', async () => {
    const mock = new MockCore({ delayMs: 0 });
    const views: string[] = [];
    mock.onState((v) => views.push(v.step.step));
    await new Promise((r) => setTimeout(r, 5));
    expect(views).toEqual(['open']);
    await mock.dispatch({ type: 'open_files', paths: ['/a.pdf', '/a.pdf', '/b.pdf'] });
    const v = await mock.view();
    expect(v.step).toEqual({
      step: 'loading',
      queue: [
        { id: 1, name: 'a.pdf', current: true },
        { id: 2, name: 'b.pdf', current: false },
      ],
    });
    mock.dispose();
  });

  it('Kacheln und Blockvorschau als data:-SVG', () => {
    const mock = new MockCore({ delayMs: 0 });
    expect(mock.pageTileUrl(1, 0, 1)).toMatch(/^data:image\/svg\+xml/);
    expect(decodeURIComponent(mock.blockPreviewUrl(2, 5))).toContain('data-rev="5"');
    mock.dispose();
  });

  it('Einstellungen: Validierung und Sprache', async () => {
    const mock = new MockCore({ delayMs: 0 });
    await mock.dispatch({ type: 'open_settings' });
    const s = defaultSettings();
    await mock.dispatch({ type: 'save_settings', settings: { ...s, proxy: { ...s.proxy, mode: 'manual', port: 70000 } } });
    expect((await mock.view()).settings?.problems).toEqual({ 'proxy.host': 'ng-ui-problem-required', 'proxy.port': 'ng-ui-problem-port' });
    await mock.dispatch({ type: 'save_settings', settings: { ...s, general: { ...s.general, ui_language: 'en' } } });
    const v = await mock.view();
    expect(v.settings).toBeNull();
    expect(v.language).toBe('en');
    mock.dispose();
  });
});
