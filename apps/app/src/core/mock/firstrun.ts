/**
 * Mock-Kern, Bereich Ersteinrichtung und Migrationsangebot (Paket UI-B; ADR 0004 §3.12) – wie K4:
 *
 * - `firstRun` = fehlende Einstellungsdatei beim Start mit Oberfläche → `FirstRunView`; mit `migrationOffer`
 *   zusätzlich das Angebot (Schritt 1) samt Übernahmeliste `items`. Ohne `firstRun` (Bestandsnutzer) gibt es
 *   keine Ersteinrichtung; ein `migrationOffer` erscheint dann wie bisher allein (`ViewState::migration`).
 * - `accept_migration`/`decline_migration` wirken sofort (die übernommenen Werte setzt der Bereich
 *   Einstellungen, `migratedSettings`); danach zeigen Schritt 2/3 die Werte aus den Einstellungen.
 * - `update_first_run` übernimmt der Bereich Einstellungen (Vorschau neu), `finish_first_run` beendet die
 *   Ersteinrichtung (Hinweis `ng-firstrun-done`), `offer_migration` bietet die Altkonfiguration erneut an
 *   (Einstellungen › Erweitert, auch nach `decline_migration`).
 *
 * Die Übernahmeliste enthält eine Bildmarke **nur**, wenn die Altkonfiguration eine eigene gesetzt hat
 * (Option `legacyEmblem`, `FirstRunMockOptions`): Der PDF-AS-Standardlogo-Pfad zählt nicht als eigene
 * Bildmarke, und ein erfundenes Beispiel-Logo würde das PIDA-Siegel (ADR 0005) still ersetzen.
 */
import type { MigrationItemView, MigrationOfferView, Settings } from '../types';
import { fileNameOf, type MockArea, type MockCtx, type MockOptions } from './shared';

/** Optionen des Mock-Kerns für die Ersteinrichtung (Erweiterung des Bereichs). */
export type FirstRunMockOptions = MockOptions & {
  /** eigene Bildmarke der Altkonfiguration (`EMBLEM` in `PDF-Over.config`, Pfad); ohne = keine */
  legacyEmblem?: string | null;
};

export interface FirstRunState {
  /** offenes Angebot (Pfad der Altdatei) */
  offer: string | null;
  secrets: boolean;
  /** Ersteinrichtung läuft (`first_run_completed = false`) */
  active: boolean;
  /** gefundene Altkonfiguration (für `offer_migration`) */
  legacy: string | null;
  emblem: string | null;
  previewRevision: number;
}

/** Was übernommen würde (Probelauf von `pida_config::migrate` im Kern). */
export function migrationItems(emblem: string | null, mobile: boolean): MigrationItemView[] {
  const items: MigrationItemView[] = [{ code: 'ng-migration-item-block', detail: null }];
  if (emblem) items.push({ code: 'ng-migration-item-emblem', detail: fileNameOf(emblem) });
  items.push({ code: 'ng-migration-item-username', detail: '+43 660 ···123' });
  if (!mobile) {
    items.push({ code: 'ng-migration-item-output', detail: '~/Dokumente/Signiert' });
    items.push({ code: 'ng-migration-item-keystore', detail: 'max.p12' });
    items.push({ code: 'ng-migration-item-proxy', detail: null });
  }
  items.push({ code: 'ng-migration-item-placeholder', detail: null });
  items.push({ code: 'ng-migration-item-language', detail: 'Deutsch' });
  return items;
}

/**
 * Einstellungen nach `accept_migration` (Beispielwerte zur Übernahmeliste, frei erfunden). Die Bildmarke
 * ändert sich nur mit einer eigenen Bildmarke der Altkonfiguration – sonst bleibt das PIDA-Siegel.
 */
export function migratedSettings(settings: Settings, emblem: string | null, mobile: boolean): Settings {
  const next: Settings = {
    ...settings,
    general: { ...settings.general, ui_language: 'de' },
    signature: { ...settings.signature, profile: 'SIGNATURBLOCK_SMALL', block_language: 'de', note: { mode: 'default' }, emblem: emblem ?? settings.signature.emblem },
    placeholder: { search: true, kind: 'marker' },
    id_austria: { ...settings.id_austria, username: '+43 660 1234123', remember: true },
  };
  if (!mobile) {
    next.output = { ...settings.output, folder: '/Users/demo/Dokumente/Signiert' };
    next.keystore = { ...settings.keystore, enabled: true, file: '/Users/demo/Zertifikate/max.p12', alias: 'max', declared_type: 'PKCS12' };
  }
  return next;
}

function offerView(state: FirstRunState, ctx: MockCtx): MigrationOfferView | null {
  return state.offer ? { legacy_file: state.offer, plaintext_secrets: state.secrets, items: migrationItems(state.emblem, ctx.platform().mobile) } : null;
}

export const firstRunArea: MockArea<FirstRunState> = {
  init: (o) => ({
    offer: o.migrationOffer ?? null,
    secrets: o.migrationSecrets ?? false,
    active: o.firstRun ?? false,
    legacy: o.migrationOffer ?? null,
    emblem: (o as FirstRunMockOptions).legacyEmblem ?? null,
    previewRevision: 1,
  }),
  handle(e, state, ctx) {
    switch (e.type) {
      case 'accept_migration': {
        if (!state.offer) return true;
        const imported = migrationItems(state.emblem, ctx.platform().mobile).length;
        state.offer = null;
        ctx.notify('ng-migrate-done', { imported: String(imported), skipped: '0' });
        return true;
      }
      case 'decline_migration':
        state.offer = null;
        return true;
      case 'finish_first_run':
        if (!state.active) return true;
        state.active = false;
        ctx.notify('ng-firstrun-done');
        ctx.emit({ kind: 'first-run-done' });
        return true;
      case 'offer_migration':
        state.offer = state.legacy;
        return true;
      default:
        return false;
    }
  },
  signal(s, state) {
    if (s.kind === 'settings-changed') state.previewRevision += 1;
  },
  project: (state, ctx) => {
    const migration = offerView(state, ctx);
    const settings = ctx.settings();
    return {
      migration,
      first_run: state.active
        ? {
            migration: migration ? structuredClone(migration) : null,
            default_backend: settings.backend.default,
            profile: settings.signature.profile,
            block_language: settings.signature.block_language,
            preview_revision: state.previewRevision,
          }
        : null,
    };
  },
};
