/**
 * Stabile Exporte des Pakets UI-B für den Prüfmodus (ADR 0004 §6.1, in E0 festgelegt). Seit B4 der Prüfmodus
 * (`VerifyView`); die Shell zeigt ihn, sobald der Kern `ViewState::verify` liefert (K4).
 */
export { VerifyView, type VerifyViewProps } from './VerifyView';

/** Prüfmodus darstellbar (seit B4). */
export const VERIFY_AVAILABLE = true;
