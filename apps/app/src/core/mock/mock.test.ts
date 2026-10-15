/**
 * Mock-Kern auf Ebene der Projektion (ADR 0004 §4.8, Abnahme E0 „je neuem Typ ein Beispiel“, Invariante A1):
 * Szenarien wie im Prototyp, PDF-AS-Regel der automatischen Platzierung, Raster/Prüfung je Revision,
 * Verschieben in 15/1/50 pt, keine Signiersitzung ohne ausdrückliche Aktion, Bereiche über Signale.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { defaultSettings } from '../defaults';
import type { Settings, UiEvent, ViewState } from '../types';
import { MOCK_SCENARIOS, MockCore, type MockOptions } from './index';

let cores: MockCore[] = [];
afterEach(() => {
  for (const c of cores) c.dispose();
  cores = [];
});

function idAustria(): Settings {
  const s = defaultSettings();
  return { ...s, backend: { default: 'id_austria' }, id_austria: { ...s.id_austria, username: '+43 660 1234123', remember: true }, output: { ...s.output, folder: '.' } };
}

function core(options: MockOptions = {}): MockCore {
  const c = new MockCore({ delayMs: 0, autoCompleteApp: false, ...options });
  cores.push(c);
  return c;
}

/** wartet, bis die Projektion `pred` erfüllt (simulierte Latenz 0, aber über Zeitgeber). */
async function until(c: MockCore, pred: (v: ViewState) => boolean): Promise<ViewState> {
  for (let i = 0; i < 100; i++) {
    const v = await c.view();
    if (pred(v)) return v;
    await new Promise((r) => setTimeout(r, 2));
  }
  throw new Error(`Zustand nicht erreicht: ${JSON.stringify((await c.view()).step).slice(0, 300)}`);
}

const hasDoc = (v: ViewState) => v.document !== null && v.document.phase === 'editing';

describe('Szenarien', () => {
  it('jedes Szenario liefert einen Ausgangszustand', async () => {
    for (const scenario of MOCK_SCENARIOS) {
      const c = core({ scenario });
      const v = await until(c, (x) => x.actions.open_files || x.step.step === 'error' || x.document !== null);
      switch (scenario) {
        case 'start':
          expect(v.document).toBeNull();
          expect(v.actions.open_files).toBe(true);
          expect(v.actions.sign).toBe(false);
          break;
        case 'verify':
          expect(v.verify).not.toBeNull();
          break;
        case 'error-password': {
          const e = await until(c, (x) => x.step.step === 'error');
          expect(e.step.step === 'error' && e.step.code).toBe('error-PDFPwdProtected');
          break;
        }
        default: {
          const d = await until(c, hasDoc);
          expect(d.document?.doc.page_count).toBeGreaterThan(0);
          expect(d.signers?.options.length).toBeGreaterThan(0);
        }
      }
    }
  });

  it('Vertrag: PDF-AS-Regel (waagrecht mittig, unter dem Inhalt der letzten Seite), Raster und Prüfung je Revision', async () => {
    const v = await until(core({ scenario: 'contract' }), hasDoc);
    const d = v.document!;
    const last = d.doc.page_count - 1;
    const [pw] = d.doc.page_sizes[last] as [number, number];
    expect(d.origin).toBe('auto');
    expect(d.target).toEqual({ kind: 'free' });
    expect(d.placement.page_index).toBe(last);
    expect(Math.abs(d.placement.x + d.placement.width / 2 - pw / 2)).toBeLessThan(0.5);
    const layout = d.layout.find((l) => l.page === last);
    expect(layout?.content).not.toBeNull();
    expect(d.placement.y).toBeLessThanOrEqual((layout?.content?.bottom ?? 0) - 20 + 0.5);
    expect(layout?.free.length).toBeGreaterThan(0);
    expect(layout?.guides.some((g) => g.axis === 'x')).toBe(true);
    expect(d.check.for_revision).toBe(d.revision);
    expect(d.check.overlaps_content).toBe(false);
    expect(d.sticker).toEqual({ left: d.placement.x, top: d.placement.y, right: d.placement.x + d.placement.width, bottom: d.placement.y - d.placement.height });
  });

  it('Signaturfeld: Felder als Ziele, Übernahme setzt Herkunft `field`', async () => {
    const c = core({ scenario: 'field' });
    const v = await until(c, hasDoc);
    const field = v.document!.fields.find((f) => f.selectable);
    expect(field).toBeDefined();
    await c.dispatch({ type: 'use_target', target: { kind: 'field', index: field!.index } });
    const after = await until(c, (x) => x.document?.origin === 'field');
    expect(after.document?.target).toEqual({ kind: 'field', index: field!.index });
    expect(after.document?.check.for_revision).toBe(after.document?.revision);
  });

  it('bereits signiert: vorhandene Signaturen mit Status, keine neue Seite', async () => {
    const v = await until(core({ scenario: 'signed' }), hasDoc);
    expect(v.document?.existing.length).toBeGreaterThan(0);
    expect(v.document?.existing.every((e) => ['valid', 'invalid', 'not_verifiable'].includes(e.status))).toBe(true);
    expect(v.document?.constraints.new_page_allowed).toBe(false);
  });

  it('Stapel: bis E2 das erste von vier Dokumenten im Arbeitsbereich, keine Stapelansicht', async () => {
    const v = await until(core({ scenario: 'batch' }), hasDoc);
    expect(v.document?.doc.name).toBe('Angebot_2026-118.pdf');
    expect(v.batch).toBeNull();
    expect(v.actions.show_batch).toBe(false);
  });

  it('Beispiele der späteren Etappen (`examples`): Stapel, zuletzt signiert, Prüfbericht, Ersteinrichtung', async () => {
    const v = await until(core({ examples: true, firstRun: true }), (x) => x.step.step === 'open');
    expect(v.recent?.length).toBeGreaterThan(0);
    expect(v.verify).not.toBeNull();
    expect(v.first_run).not.toBeNull();
    expect(v.batch?.items.map((i) => i.status.state)).toEqual(['positioned', 'ready', 'rejected', 'ready']);
    // bis A2 ohne Stapelansicht
    expect(v.actions.show_batch).toBe(false);
    const plain = await until(core(), (x) => x.step.step === 'open');
    expect(plain.recent).toBeNull();
    expect(plain.verify).toBeNull();
    expect(plain.first_run).toBeNull();
    expect(plain.batch).toBeNull();
  });
});

describe('Platzierung', () => {
  it('Verschieben in 15 pt, fein 1 pt, grob 50 pt; Herkunft `user`', async () => {
    const c = core({ scenario: 'blank' });
    const v = await until(c, hasDoc);
    const x0 = v.document!.placement.x;
    await c.dispatch({ type: 'nudge_block', dx: -1, dy: 0, fine: false, coarse: false });
    let d = (await until(c, (x) => x.document!.placement.x !== x0)).document!;
    expect(d.placement.x).toBeCloseTo(x0 - 15);
    expect(d.origin).toBe('user');
    await c.dispatch({ type: 'nudge_block', dx: -1, dy: 0, fine: true, coarse: false });
    d = (await c.view()).document!;
    expect(d.placement.x).toBeCloseTo(x0 - 16);
    await c.dispatch({ type: 'nudge_block', dx: -1, dy: 0, fine: false, coarse: true });
    d = (await c.view()).document!;
    expect(d.placement.x).toBeCloseTo(x0 - 66);
    expect(d.check.for_revision).toBe(d.revision);
  });

  it('Verschieben bleibt auf der Seite (`clamped`)', async () => {
    const c = core({ scenario: 'blank' });
    const v = await until(c, hasDoc);
    await c.dispatch({ type: 'move_block', page: v.document!.placement.page_index, x: -500, y: 99999 });
    const d = (await c.view()).document!;
    expect(d.placement.x).toBe(0);
    expect(d.check.clamped).toBe(true);
  });

  it('unsichtbar und neue Seite folgen den Einschränkungen', async () => {
    const c = core({ scenario: 'blank' });
    await until(c, hasDoc);
    await c.dispatch({ type: 'set_invisible', invisible: true });
    expect((await c.view()).document?.invisible).toBe(true);
    await c.dispatch({ type: 'set_invisible', invisible: false });
    await c.dispatch({ type: 'toggle_new_page' });
    const d = (await c.view()).document!;
    expect(d.new_page).toBe(true);
    expect(d.placement.page_index).toBe(d.doc.page_count);
  });
});

describe('Invariante A1: Sitzung nur nach ausdrücklicher Aktion', () => {
  it('Öffnen, Platzieren und Signer wählen starten keine Sitzung; „Signieren“ (`sign_with`) schon', async () => {
    const c = core({ scenario: 'contract', settings: idAustria() });
    const seen: string[] = [];
    c.onState((v) => seen.push(v.step.step));
    const v = await until(c, hasDoc);
    const events: UiEvent[] = [
      { type: 'select_signer', backend: 'keystore', make_default: false },
      { type: 'select_signer', backend: 'atrust_native', make_default: false },
      { type: 'nudge_block', dx: 1, dy: 0, fine: false, coarse: false },
      { type: 'inspect_pages', pages: [0, 1] },
      { type: 'auto_place' },
    ];
    for (const e of events) await c.dispatch(e);
    await new Promise((r) => setTimeout(r, 10));
    expect(seen).not.toContain('signing');
    expect(v.signers?.selected).toBe('atrust_native');
    await c.dispatch({ type: 'sign_with', backend: 'atrust_native' });
    const s = await until(c, (x) => x.step.step === 'signing');
    expect(s.step.step === 'signing' && s.step.progress.backend).toBe('atrust_native');
    expect(s.step.step === 'signing' && s.step.document_leaves_device).toBe(true);
    // Abbrechen: zurück in den Arbeitsbereich, Platzierung unverändert, Hinweis
    const before = s.document?.placement;
    await c.dispatch({ type: 'cancel' });
    const back = await until(c, (x) => x.step.step === 'position');
    expect(back.document?.placement).toEqual(before);
    expect(back.notices.some((n) => n.code === 'ng-signing-cancelled')).toBe(true);
  });

  it('nicht verfügbarer Signer → Hinweis, keine Sitzung', async () => {
    const c = core({ scenario: 'contract', localBkuAvailable: false });
    await until(c, hasDoc);
    await c.dispatch({ type: 'sign_with', backend: 'local_bku' });
    const v = await c.view();
    expect(v.step.step).toBe('position');
    expect(v.notices.some((n) => n.code === 'ng-signer-unavailable')).toBe(true);
  });
});

describe('Bereiche über Signale', () => {
  it('Einstellungen speichern → neue Blockvorschau (Revision) im Arbeitsbereich', async () => {
    const c = core({ scenario: 'contract' });
    const v = await until(c, hasDoc);
    const rev = v.document!.block.preview_revision;
    await c.dispatch({ type: 'open_settings' });
    expect((await c.view()).actions.settings).toBe(false);
    const s = (await c.view()).settings!.settings;
    await c.dispatch({ type: 'save_settings', settings: { ...s, signature: { ...s.signature, profile: 'AMTSSIGNATURBLOCK' } } });
    const after = await until(c, (x) => x.settings === null);
    expect(after.document!.block.preview_revision).toBeGreaterThan(rev);
    expect(after.document!.block.width).toBeGreaterThan(v.document!.block.width);
  });
});
