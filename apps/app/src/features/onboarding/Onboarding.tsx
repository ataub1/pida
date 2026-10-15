/**
 * Ersteinrichtung (stabiler Export für `App.tsx`, ADR 0004 §6.1). Bis B4 eine Hülle um das bisherige
 * Migrationsangebot (`MigrationDialog`); die Ersteinrichtung mit drei Schritten (`FirstRunView`) folgt in B4.
 */
import type { FirstRunView, MigrationOfferView } from '../../core/types';
import { MigrationDialog } from './MigrationDialog';

/** Props der Ersteinrichtung (in E0 festgelegt). */
export interface OnboardingProps {
  /** Migrationsangebot (`ViewState::migration`) */
  migration: MigrationOfferView | null;
  /** Ersteinrichtung (`ViewState::first_run`, ab K4) */
  firstRun: FirstRunView | null;
}

export function Onboarding({ migration }: OnboardingProps) {
  return migration ? <MigrationDialog offer={migration} /> : null;
}
