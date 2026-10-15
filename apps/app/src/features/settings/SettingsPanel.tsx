/**
 * Einstellungen (stabiler Export für `App.tsx`, ADR 0004 §6.1). Bis B3 eine Hülle um den bisherigen
 * Einstellungsdialog (`settings/SettingsDialog.tsx`): Der Abschnitt aus `openSettings(section)` wählt den
 * passenden Tab des alten Dialogs. B3 ersetzt die Hülle durch die Seitenleiste mit Suche und sofortiger
 * Wirkung, ohne `app/**` anzufassen.
 */
import type { SettingsSectionId, SettingsTab } from '../../app/context';
import type { SettingsView } from '../../core/types';
import { SettingsDialog } from '../../settings/SettingsDialog';
import '../../styles/legacy/dialogs.css';

/** Props des Einstellungs-Overlays (in E0 festgelegt). */
export interface SettingsPanelProps {
  settingsView: SettingsView;
  /** Abschnitt beim Öffnen */
  section: SettingsSectionId;
}

const LEGACY_TAB: Record<SettingsSectionId, SettingsTab> = {
  signature: 'simple',
  signers: 'simple',
  save: 'simple',
  appearance: 'advanced',
  privacy: 'privacy',
  advanced: 'advanced',
  about: 'about',
};

export function SettingsPanel({ settingsView, section }: SettingsPanelProps) {
  return <SettingsDialog settingsView={settingsView} initialTab={LEGACY_TAB[section]} />;
}
