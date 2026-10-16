/**
 * Mock-Kern, Bereich „Zuletzt signiert“ (Paket UI-B; ADR 0004 §3.10). Ohne `examples` wie der Kern bis K3:
 * `ViewState::recent = None`. Mit `examples` (bzw. einer Richtlinie, die die Liste erzwingt) wie K3: drei
 * Einträge, neueste zuerst; neue Einträge kommen über das Signal `signed` (nur Dateiname, Ordner, Zeitpunkt –
 * kein Signiermittel), höchstens zehn. `general.recent_documents = false` (Signal `settings-changed`) löscht
 * die Liste sofort (`None`), Wiedereinschalten beginnt leer; eine Richtlinie `recent_documents = false`
 * schaltet sie ganz ab. `verify_recent` auf eine fehlende Datei → Hinweis `ng-recent-missing`, der Eintrag
 * bleibt.
 */
import type { RecentItemView } from '../types';
import type { SettingsMockOptions } from './settings';
import { folderOf, isoWithOffset, type MockArea } from './shared';

export interface RecentState {
  items: RecentItemView[] | null;
  nextId: number;
  /** Funktion vorhanden (K3); sonst bleibt `recent` immer `None` */
  supported: boolean;
  /** Richtlinie `recent_documents` */
  policy: boolean | undefined;
}

export const RECENT_LIMIT = 10;

const EXAMPLES: RecentItemView[] = [
  { id: 3, file_name: 'Kaufvertrag_Fahrzeug_signiert.pdf', folder_display: '/Users/demo/Dokumente/Auto', signed_at: '2026-10-14T09:12:00+02:00', exists: true },
  { id: 2, file_name: 'Mietvertrag_signiert.pdf', folder_display: '/Users/demo/Dokumente/Wohnung', signed_at: '2026-10-13T08:47:00+02:00', exists: true },
  { id: 1, file_name: 'Angebot_2026-117_signed.pdf', folder_display: '/Users/demo/Dokumente/Angebote', signed_at: '2026-10-12T11:05:00+02:00', exists: false },
];

export const recentArea: MockArea<RecentState> = {
  init: (o) => {
    const policy = (o as SettingsMockOptions).policy?.recent_documents;
    const supported = !!o.examples || policy !== undefined;
    const enabled = policy ?? o.settings?.general.recent_documents ?? true;
    return {
      items: supported && enabled ? (o.examples ? EXAMPLES.map((i) => ({ ...i })) : []) : null,
      nextId: 4,
      supported,
      policy,
    };
  },
  handle(e, state, ctx) {
    switch (e.type) {
      case 'remove_recent':
        if (state.items) state.items = state.items.filter((i) => i.id !== e.id);
        return true;
      case 'clear_recent':
        if (state.items) state.items = [];
        return true;
      case 'verify_recent': {
        const item = state.items?.find((i) => i.id === e.id);
        if (item && !item.exists) ctx.notify('ng-recent-missing');
        return true;
      }
      default:
        return false;
    }
  },
  signal(s, state, ctx) {
    if (s.kind === 'settings-changed') {
      if (!state.supported || state.policy !== undefined) return;
      if (!s.settings.general.recent_documents) state.items = null;
      else if (state.items === null) state.items = [];
      return;
    }
    if (s.kind !== 'signed' || !state.items) return;
    const folder = ctx.platform().mobile ? null : folderOf(s.path);
    state.items = [{ id: state.nextId++, file_name: s.file_name, folder_display: folder, signed_at: isoWithOffset(new Date()), exists: true }, ...state.items].slice(0, RECENT_LIMIT);
  },
  project: (state) => ({ recent: state.items ? state.items.map((i) => ({ ...i })) : null }),
};
