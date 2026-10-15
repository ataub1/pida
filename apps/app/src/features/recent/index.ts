/**
 * Stabile Exporte des Pakets UI-B für „Zuletzt signiert“ (ADR 0004 §6.1, in E0 festgelegt). Bis B3 ein
 * Platzhalter ohne Darstellung (der Slot auf der Startansicht bleibt leer); der Kern liefert
 * `ViewState::recent` erst ab K3.
 */
import type { RecentItemView } from '../../core/types';

/** Props der Liste „Zuletzt signiert“ (in E0 festgelegt). */
export interface RecentPanelProps {
  /** `ViewState::recent` (`null` = abgeschaltet) */
  items: RecentItemView[] | null;
}

/** „Zuletzt signiert“ auf der Startansicht (Platzhalter bis B3). */
export function RecentPanel(_props: RecentPanelProps): null {
  return null;
}

/** Liste darstellbar (B3 schaltet um); bis dahin bleibt die Seitenspalte der Startansicht ohne diesen Slot. */
export const RECENT_AVAILABLE = false;
