/**
 * Stabile Exporte des Pakets UI-B für „Zuletzt signiert“ (ADR 0004 §6.1, in E0 festgelegt). Seit B3 mit
 * Darstellung (`RecentPanel`); `ViewState::recent` liefert der Kern ab K3.
 */
export { RECENT_MAX, RecentPanel, type RecentPanelProps } from './RecentPanel';

/** Liste darstellbar (seit B3); die Startansicht zeigt den Slot, sobald `ViewState::recent` nicht `null` ist. */
export const RECENT_AVAILABLE = true;
