/**
 * Stabile Exporte des Pakets UI-B für den Prüfmodus (ADR 0004 §6.1, in E0 festgelegt). Bis B4 ein
 * Platzhalter ohne Darstellung; der Kern liefert `ViewState::verify` erst ab K4.
 */
import type { VerifyView as VerifyViewData } from '../../core/types';

/** Props des Prüfmodus (in E0 festgelegt). */
export interface VerifyViewProps {
  verify: VerifyViewData;
}

/** Prüfmodus (Platzhalter bis B4). */
export function VerifyView(_props: VerifyViewProps): null {
  return null;
}

/** Prüfmodus darstellbar (B4 schaltet um); bis dahin zeigt die Shell den Workflow-Zustand. */
export const VERIFY_AVAILABLE = false;
