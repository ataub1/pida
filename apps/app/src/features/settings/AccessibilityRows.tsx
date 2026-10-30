/**
 * Einstellungen › Barrierefreiheit (docs/analyse/06): „Barrierefreie Bedienung“ stellt die übrigen Werte gemeinsam
 * ein (`withMode`), jeder bleibt einzeln änderbar; sofort wirksam wie alle Einstellungen (`apply_settings`).
 * Kontrast, Bewegung und Schriftgröße folgen dem System – dafür gibt es hier bewusst keine eigenen Schalter.
 */
import { useApp } from '../../app/context';
import type { AccessibilitySettings, AnnounceDetail, KeyboardStep, PlacementStart } from '../../core/types';
import { Button, SwitchField } from '../../ui/controls';
import { IconKeyboard } from '../../ui/Icons';
import { accessibilityChanges, withMode } from '../accessibility';
import { ChoiceCards, SettingSelect } from './controls';
import { useSettings } from './state';

function useAccessibilitySetting(): AccessibilitySettings {
  const s = useSettings();
  return s.get<AccessibilitySettings>('accessibility');
}

export function A11yModeRow() {
  const { t } = useApp();
  const s = useSettings();
  const a = useAccessibilitySetting();
  const mode = s.get<boolean>('accessibility.mode');
  return (
    <SwitchField
      label={t.t('ng-ui-set-a11y-mode')}
      hint={s.locked('accessibility.mode') ?? t.t('ng-ui-set-a11y-mode-hint')}
      isSelected={mode}
      isDisabled={!!s.locked('accessibility.mode')}
      onChange={(on) => s.apply(accessibilityChanges(withMode({ ...a, mode }, on)))}
    />
  );
}

export function A11yStartRow() {
  const { t } = useApp();
  const s = useSettings();
  return (
    <ChoiceCards<PlacementStart>
      label={t.t('ng-ui-set-a11y-start')}
      value={s.get<PlacementStart>('accessibility.start')}
      onChange={(v) => s.apply({ 'accessibility.start': v })}
      isDisabled={!!s.locked('accessibility.start')}
      description={s.locked('accessibility.start')}
      options={[
        { value: 'preview', label: t.t('ng-ui-set-a11y-start-preview'), description: t.t('ng-ui-set-a11y-start-preview-desc') },
        { value: 'methods', label: t.t('ng-ui-set-a11y-start-methods'), description: t.t('ng-ui-set-a11y-start-methods-desc') },
      ]}
    />
  );
}

export function A11yCheckRow() {
  const { t } = useApp();
  const s = useSettings();
  return (
    <SwitchField
      label={t.t('ng-ui-set-a11y-check')}
      hint={s.locked('accessibility.check_before_sign') ?? t.t('ng-ui-set-a11y-check-hint')}
      isSelected={s.get<boolean>('accessibility.check_before_sign')}
      isDisabled={!!s.locked('accessibility.check_before_sign')}
      onChange={(v) => s.apply({ 'accessibility.check_before_sign': v })}
    />
  );
}

export function A11yAnnounceRow() {
  const { t } = useApp();
  const s = useSettings();
  return (
    <div className="stack-3">
      <ChoiceCards<AnnounceDetail>
        label={t.t('ng-ui-set-a11y-announce')}
        value={s.get<AnnounceDetail>('accessibility.announce')}
        onChange={(v) => s.apply({ 'accessibility.announce': v })}
        isDisabled={!!s.locked('accessibility.announce')}
        description={s.locked('accessibility.announce')}
        columns={2}
        options={[
          { value: 'short', label: t.t('ng-ui-set-a11y-announce-short'), description: t.t('ng-ui-set-a11y-announce-short-desc') },
          { value: 'detailed', label: t.t('ng-ui-set-a11y-announce-detailed'), description: t.t('ng-ui-set-a11y-announce-detailed-desc') },
        ]}
      />
      <SwitchField
        label={t.t('ng-ui-set-a11y-after-move')}
        hint={t.t('ng-ui-set-a11y-after-move-hint')}
        isSelected={s.get<boolean>('accessibility.announce_after_move')}
        isDisabled={!!s.locked('accessibility.announce_after_move')}
        onChange={(v) => s.apply({ 'accessibility.announce_after_move': v })}
      />
      <p className="hint">{t.t('ng-ui-set-a11y-system')}</p>
    </div>
  );
}

/** Auswahl der Schrittweite (auch im Inspektor). */
export function keyboardStepOptions(t: ReturnType<typeof useApp>['t']): { id: KeyboardStep; label: string }[] {
  return [
    { id: 'standard', label: t.t('ng-ui-set-a11y-step-standard') },
    { id: 'mm1', label: t.t('ng-ui-set-a11y-step-mm', { mm: 1 }) },
    { id: 'mm5', label: t.t('ng-ui-set-a11y-step-mm', { mm: 5 }) },
    { id: 'mm10', label: t.t('ng-ui-set-a11y-step-mm', { mm: 10 }) },
  ];
}

export function A11yStepRow() {
  const { t } = useApp();
  const s = useSettings();
  return (
    <SettingSelect
      label={t.t('ng-ui-set-a11y-step')}
      value={s.get<KeyboardStep>('accessibility.keyboard_step')}
      onChange={(v) => s.apply({ 'accessibility.keyboard_step': v })}
      options={keyboardStepOptions(t)}
      description={s.locked('accessibility.keyboard_step') ?? t.t('ng-ui-set-a11y-step-hint')}
      isDisabled={!!s.locked('accessibility.keyboard_step')}
    />
  );
}

/** Kürzelübersicht („?“) aus den Einstellungen öffnen. */
export function A11yKeysRow() {
  const { t, setOverlay } = useApp();
  return (
    <div>
      <Button variant="secondary" size="sm" onPress={() => setOverlay('shortcuts')}>
        <IconKeyboard size={16} />
        {t.t('ng-ui-set-a11y-keys')}
      </Button>
    </div>
  );
}
