/**
 * Mock-Kern der Pakets UI-B (`core/mock/{settings,recent,verify,firstrun}.ts`) wie K3/K4 (ADR 0004 §3.9–§3.12):
 * `apply_settings` mit Teilübernahme, bleibende Feldprobleme, unbekannte Pfade, Revision, nur bei geöffnetem
 * Overlay, Richtlinie; „Zuletzt signiert“ (höchstens zehn, Abschalten löscht sofort, fehlende Datei, öffnet den
 * Prüfmodus); Prüfmodus (Laden, Freigabe, Kacheln, „Dieses Dokument signieren“); Ersteinrichtung (erfindet keine
 * Bildmarke, `update_first_run`, Migration, Neuinstallation „neben dem Original“, `finish_first_run`).
 */
import { describe, expect, it } from 'vitest';
import { MockCore } from '../../core/mock';
import type { FirstRunMockOptions } from '../../core/mock/firstrun';
import type { SettingsMockOptions } from '../../core/mock/settings';
import { VERIFY_DOC_ID, verifyAllowed } from '../../core/mock/verify';
import type { ViewState } from '../../core/types';
import type { SettingsViewDraft } from './contract-draft';

async function core(options: SettingsMockOptions & FirstRunMockOptions = {}): Promise<MockCore> {
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

describe('Prüfmodus (Mock wie K4)', () => {
  const tick = () => new Promise((r) => setTimeout(r, 0));

  it('open_verify: zuerst „wird geprüft“, dann das Ergebnis mit eigener DocId und Kacheln; close_verify', async () => {
    const c = await core();
    expect((await c.view()).actions.verify).toBe(true);
    await c.dispatch({ type: 'open_verify', path: '/Users/demo/Vertrag_manipuliert.pdf' });
    expect((await c.view()).verify?.status).toEqual({ state: 'loading' });
    await tick();
    const v = (await c.view()).verify;
    expect(v?.status).toEqual({ state: 'ready' });
    expect(v?.summary).toBe('some_broken');
    expect(v?.doc?.id).toBe(VERIFY_DOC_ID);
    expect(c.pageTileUrl(VERIFY_DOC_ID, 1, 1)).toMatch(/^data:image\/svg\+xml/);
    // der Workflow-Zustand bleibt (Prüfmodus liegt darüber)
    expect((await c.view()).step.step).toBe('open');
    await c.dispatch({ type: 'close_verify' });
    expect((await c.view()).verify).toBeNull();
  });

  it('verify_recent öffnet den Prüfmodus; sign_verified öffnet die Datei im Arbeitsbereich', async () => {
    const c = await core({ examples: true });
    await c.dispatch({ type: 'verify_recent', id: 3 });
    await tick();
    expect((await c.view()).verify?.file_name).toBe('Kaufvertrag_Fahrzeug_signiert.pdf');
    await c.dispatch({ type: 'sign_verified' });
    expect((await c.view()).verify).toBeNull();
    for (let i = 0; i < 10 && (await c.view()).document === null; i += 1) await tick();
    expect((await c.view()).document?.doc.name).toBe('Kaufvertrag_Fahrzeug_signiert.pdf');
  });

  it('Freigabe wie K4: Leerlauf, Positionieren, Fertig, Stapelübersicht – nicht beim Signieren', () => {
    expect(verifyAllowed({ step: { step: 'open', queue: [] } })).toBe(true);
    expect(verifyAllowed({ step: { step: 'batch' } })).toBe(true);
    expect(verifyAllowed({ step: { step: 'starting' } })).toBe(false);
    expect(verifyAllowed({ step: { step: 'finished', exit_code: 0 } })).toBe(false);
  });
});

describe('Ersteinrichtung (Mock wie K4)', () => {
  const LEGACY = '/Users/demo/.pdf-over/PDF-Over.config';

  it('Neuinstallation ohne Altkonfiguration: „neben dem Original“, first_run_completed erst mit finish_first_run', async () => {
    const c = await core({ firstRun: true });
    let v = await c.view();
    expect(v.first_run).toMatchObject({ migration: null, default_backend: null, profile: 'SIGNATURBLOCK_SMALL', block_language: null });
    await c.dispatch({ type: 'open_settings' });
    expect((await settingsOf(c)).settings.output.folder).toBe('.');
    expect((await settingsOf(c)).settings.general.first_run_completed).toBe(false);
    await c.dispatch({ type: 'close_settings' });
    const rev = v.first_run?.preview_revision ?? 0;
    await c.dispatch({ type: 'update_first_run', default_backend: 'id_austria', profile: 'AMTSSIGNATURBLOCK', block_language: 'en' });
    v = await c.view();
    expect(v.first_run).toMatchObject({ default_backend: 'id_austria', profile: 'AMTSSIGNATURBLOCK', block_language: 'en' });
    expect(v.first_run?.preview_revision).toBeGreaterThan(rev);
    await c.dispatch({ type: 'finish_first_run' });
    v = await c.view();
    expect(v.first_run).toBeNull();
    expect(v.notices.map((n) => n.code)).toContain('ng-firstrun-done');
    await c.dispatch({ type: 'open_settings' });
    expect((await settingsOf(c)).settings.general.first_run_completed).toBe(true);
  });

  it('Migration: Werte der Altkonfiguration, Bildmarke nur bei eigener; offer_migration bietet erneut an', async () => {
    const c = await core({ firstRun: true, migrationOffer: LEGACY });
    await c.dispatch({ type: 'accept_migration', store_secrets: false });
    let v = await c.view();
    expect(v.first_run?.migration).toBeNull();
    expect(v.migration).toBeNull();
    await c.dispatch({ type: 'open_settings' });
    const s = (await settingsOf(c)).settings;
    expect(s.signature.emblem).toBeNull();
    expect(s.output.folder).toBe('/Users/demo/Dokumente/Signiert');
    await c.dispatch({ type: 'close_settings' });
    await c.dispatch({ type: 'offer_migration' });
    v = await c.view();
    expect(v.migration?.legacy_file).toBe(LEGACY);

    const own = await core({ firstRun: true, migrationOffer: LEGACY, legacyEmblem: '/Users/demo/.pdf-over/eigenes-logo.png' });
    expect((await own.view()).first_run?.migration?.items.map((i) => i.code)).toContain('ng-migration-item-emblem');
    await own.dispatch({ type: 'accept_migration', store_secrets: false });
    await own.dispatch({ type: 'open_settings' });
    expect((await settingsOf(own)).settings.signature.emblem).toBe('/Users/demo/.pdf-over/eigenes-logo.png');

    const none = await core();
    await none.dispatch({ type: 'offer_migration' });
    expect((await none.view()).migration).toBeNull();
  });
});
