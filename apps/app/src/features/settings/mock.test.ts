/**
 * Mock-Kern der Pakets UI-B (`core/mock/{settings,recent,firstrun}.ts`) wie K3 (ADR 0004 §3.10, §3.11):
 * `apply_settings` mit Teilübernahme, bleibende Feldprobleme, unbekannte Pfade, Revision, nur bei geöffnetem
 * Overlay, Richtlinie; „Zuletzt signiert“ (höchstens zehn, Abschalten löscht sofort, fehlende Datei); die
 * Übernahmeliste der Ersteinrichtung erfindet keine Bildmarke.
 */
import { describe, expect, it } from 'vitest';
import { MockCore } from '../../core/mock';
import type { SettingsMockOptions } from '../../core/mock/settings';
import type { ViewState } from '../../core/types';
import type { SettingsViewDraft } from './contract-draft';

async function core(options: SettingsMockOptions = {}): Promise<MockCore> {
  const c = new MockCore({ delayMs: 0, autoCompleteApp: false, ...options });
  for (let i = 0; i < 20; i += 1) {
    if ((await c.view()).step.step !== 'starting') break;
    await new Promise((r) => setTimeout(r, 0));
  }
  return c;
}

async function settingsOf(c: MockCore): Promise<NonNullable<ViewState['settings']>> {
  const v = await c.view();
  if (!v.settings) throw new Error('Einstellungen nicht offen');
  return v.settings;
}

describe('apply_settings (Mock wie K3)', () => {
  it('Teilübernahme: ungültiger Port bleibt als Problem, gültiges Profil wird übernommen; Revision', async () => {
    const c = await core();
    await c.dispatch({ type: 'open_settings' });
    const rev0 = (await settingsOf(c)).preview_revision;
    await c.dispatch({ type: 'apply_settings', changes: { 'proxy.port': 99999, 'signature.profile': 'AMTSSIGNATURBLOCK' }, revision: 7 });
    let s = await settingsOf(c);
    expect(s.applied_revision).toBe(7);
    expect(s.settings.signature.profile).toBe('AMTSSIGNATURBLOCK');
    expect(s.settings.proxy.port).toBeNull();
    expect(s.problems).toEqual({ 'proxy.port': 'ng-ui-problem-port' });
    expect(s.preview_revision).toBeGreaterThan(rev0);
    // eine andere Änderung lässt das Feldproblem stehen; gültige Übernahme löscht es
    await c.dispatch({ type: 'apply_settings', changes: { 'signature.pdfa_force': true }, revision: 8 });
    s = await settingsOf(c);
    expect(s.problems).toEqual({ 'proxy.port': 'ng-ui-problem-port' });
    await c.dispatch({ type: 'apply_settings', changes: { 'proxy.port': 8080 }, revision: 9 });
    s = await settingsOf(c);
    expect(s.problems).toEqual({});
    expect(s.settings.proxy.port).toBe(8080);
    expect(s.settings.signature.pdfa_force).toBe(true);
  });

  it('unbekannter Pfad → Problem; manueller Proxy ohne Host → Hinweis am Host; nur bei geöffnetem Overlay', async () => {
    const c = await core();
    await c.dispatch({ type: 'apply_settings', changes: { 'signature.profile': 'AMTSSIGNATURBLOCK' }, revision: 1 });
    await c.dispatch({ type: 'open_settings' });
    expect((await settingsOf(c)).settings.signature.profile).toBe('SIGNATURBLOCK_SMALL');
    await c.dispatch({ type: 'apply_settings', changes: { 'signature.gibts_nicht': 1, 'proxy.mode': 'manual' }, revision: 2 });
    const s = await settingsOf(c);
    expect(s.problems).toEqual({ 'signature.gibts_nicht': 'ng-settings-field-unknown', 'proxy.host': 'ng-settings-proxy-host-missing' });
    expect(s.settings.proxy.mode).toBe('manual');
    await c.dispatch({ type: 'apply_settings', changes: { 'proxy.host': 'proxy.local' }, revision: 3 });
    expect((await settingsOf(c)).problems).toEqual({ 'signature.gibts_nicht': 'ng-settings-field-unknown' });
  });

  it('Richtlinie: gesperrte Pfade im (vorgeschlagenen) Feld `locked`, Änderung abgelehnt', async () => {
    const c = await core({ examples: true, policy: { recent_documents: false } });
    await c.dispatch({ type: 'open_settings' });
    const s = (await settingsOf(c)) as SettingsViewDraft;
    expect(s.locked).toEqual({ 'general.recent_documents': 'ng-recent-policy' });
    expect((await c.view()).recent).toBeNull();
    await c.dispatch({ type: 'apply_settings', changes: { 'general.recent_documents': true }, revision: 1 });
    expect((await settingsOf(c)).problems).toEqual({ 'general.recent_documents': 'ng-recent-policy' });
    expect((await c.view()).recent).toBeNull();
  });
});

describe('Zuletzt signiert (Mock wie K3)', () => {
  it('bis K3 (ohne Beispiele) `None`; mit Beispielen neueste zuerst, nur Name/Ordner/Zeit', async () => {
    expect((await (await core()).view()).recent).toBeNull();
    const v = await (await core({ examples: true })).view();
    expect(v.recent?.map((i) => i.id)).toEqual([3, 2, 1]);
    for (const item of v.recent ?? []) expect(Object.keys(item).sort()).toEqual(['exists', 'file_name', 'folder_display', 'id', 'signed_at']);
  });

  it('Abschalten löscht sofort, Wiedereinschalten beginnt leer; Entfernen, Leeren', async () => {
    const c = await core({ examples: true });
    await c.dispatch({ type: 'remove_recent', id: 2 });
    expect((await c.view()).recent?.map((i) => i.id)).toEqual([3, 1]);
    await c.dispatch({ type: 'open_settings' });
    await c.dispatch({ type: 'apply_settings', changes: { 'general.recent_documents': false }, revision: 1 });
    expect((await c.view()).recent).toBeNull();
    await c.dispatch({ type: 'apply_settings', changes: { 'general.recent_documents': true }, revision: 2 });
    expect((await c.view()).recent).toEqual([]);
    const d = await core({ examples: true });
    await d.dispatch({ type: 'clear_recent' });
    expect((await d.view()).recent).toEqual([]);
  });

  it('fehlende Datei → Hinweis `ng-recent-missing`, der Eintrag bleibt', async () => {
    const c = await core({ examples: true });
    await c.dispatch({ type: 'verify_recent', id: 1 });
    const v = await c.view();
    expect(v.notices.map((n) => n.code)).toContain('ng-recent-missing');
    expect(v.recent?.some((i) => i.id === 1)).toBe(true);
  });
});

describe('Ersteinrichtung (Beispiel)', () => {
  it('erfindet keine Bildmarke: das PIDA-Siegel bleibt Standard (ADR 0005)', async () => {
    const v = await (await core({ firstRun: true, migrationOffer: '/Users/demo/.pdf-over/PDF-Over.config' })).view();
    const codes = v.first_run?.migration?.items.map((i) => i.code) ?? [];
    expect(codes.length).toBeGreaterThan(0);
    expect(codes).not.toContain('ng-migration-item-emblem');
    expect(JSON.stringify(v.first_run)).not.toMatch(/logo/i);
  });
});
