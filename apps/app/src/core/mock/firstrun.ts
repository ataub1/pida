/**
 * Mock-Kern, Bereich Ersteinrichtung und Migrationsangebot (Paket UI-B, in U0 aus dem bisherigen Mock
 * übernommen; ADR 0004 §3.12). `ViewState::migration` wie bisher; mit `firstRun` ein Beispiel für
 * `FirstRunView` (inkl. Übernahmeliste `items`). B4 baut darauf die Ersteinrichtung.
 */
import type { FirstRunView, MigrationItemView } from '../types';
import type { MockArea } from './shared';

export interface FirstRunState {
  offer: string | null;
  secrets: boolean;
  firstRun: FirstRunView | null;
  legacy: string | null;
}

const ITEMS: MigrationItemView[] = [
  { code: 'ng-migration-item-block', detail: null },
  { code: 'ng-migration-item-emblem', detail: 'firmenlogo.png' },
  { code: 'ng-migration-item-username', detail: '+43 660 ···123' },
  { code: 'ng-migration-item-output', detail: '~/Dokumente/Signiert' },
  { code: 'ng-migration-item-keystore', detail: 'max.p12' },
  { code: 'ng-migration-item-proxy', detail: null },
  { code: 'ng-migration-item-placeholder', detail: null },
  { code: 'ng-migration-item-language', detail: 'Deutsch' },
];

export const firstRunArea: MockArea<FirstRunState> = {
  init: (o) => ({
    offer: o.migrationOffer ?? null,
    secrets: o.migrationSecrets ?? false,
    legacy: o.migrationOffer ?? null,
    firstRun: o.firstRun
      ? {
          migration: o.migrationOffer ? { legacy_file: o.migrationOffer, plaintext_secrets: o.migrationSecrets ?? false, items: ITEMS } : null,
          default_backend: 'id_austria',
          profile: 'SIGNATURBLOCK_SMALL',
          block_language: null,
          preview_revision: 1,
        }
      : null,
  }),
  handle(e, state, ctx) {
    switch (e.type) {
      case 'accept_migration':
      case 'decline_migration':
        state.offer = null;
        if (state.firstRun) state.firstRun = { ...state.firstRun, migration: null };
        return true;
      case 'update_first_run':
        if (state.firstRun) state.firstRun = { ...state.firstRun, default_backend: e.default_backend, profile: e.profile, block_language: e.block_language, preview_revision: state.firstRun.preview_revision + 1 };
        return true;
      case 'finish_first_run':
        state.firstRun = null;
        ctx.emit({ kind: 'first-run-done' });
        return true;
      case 'offer_migration':
        state.offer = state.legacy;
        return true;
      default:
        return false;
    }
  },
  project: (state) => ({
    migration: state.offer ? { legacy_file: state.offer, plaintext_secrets: state.secrets, items: state.firstRun ? ITEMS : [] } : null,
    first_run: state.firstRun ? structuredClone(state.firstRun) : null,
  }),
};
