/**
 * Hauptleiste (F-100, docs/analyse/01 §4.0/§3.5): Einstellungen + vier Schritte Öffnen →
 * Positionieren → Signieren → Fertig mit „erreicht“ (A) und „klickbar“ (E) aus `MainBarView`.
 * „Fertig“ ist nie klickbar (reine Statusanzeige). „Öffnen“ im Öffnen-Schritt zeigt den Dateidialog,
 * sonst navigiert er (F-16). Tastenkürzel: Strg/⌘+, (Einstellungen), Strg/⌘+O (Öffnen) – in `App`.
 */
import { useEffect, useRef } from 'react';
import { Button } from 'react-aria-components';
import { useApp } from '../app/context';
import type { MainBarStep, Step } from '../core/types';
import { IconSettings } from './icons';

const LABEL: Record<Step, string> = {
  Open: 'common-open',
  Positioning: 'main-position',
  Signing: 'main-signature',
  Finish: 'main-done',
};

/** Hauptleiste. */
export function MainBar({ onOpenFiles }: { onOpenFiles: () => void }) {
  const { view, dispatch, t, isMac, openSettings } = useApp();
  const steps = view.main_bar.steps;
  // aktueller Schritt = letzter erreichter
  const currentIndex = steps.reduce((acc, s, i) => (s.active ? i : acc), -1);
  const settingsRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    settingsRef.current?.setAttribute('aria-keyshortcuts', isMac ? 'Meta+,' : 'Control+,');
  }, [isMac]);

  const press = (s: MainBarStep) => {
    if (!s.enabled || s.step === 'Finish') return;
    if (s.step === 'Open' && view.step.step === 'open') {
      onOpenFiles();
      return;
    }
    dispatch({ type: 'navigate', step: s.step });
  };

  return (
    <nav className="mainbar" aria-label={t.t('ng-ui-mainbar-label')}>
      <Button
        ref={settingsRef}
        className="mainbar-settings"
        isDisabled={!view.main_bar.settings_enabled}
        onPress={() => openSettings()}
        aria-label={t.t('main-configuration')}
      >
        <IconSettings size={22} />
      </Button>
      <ol className="mainbar-steps">
        {steps.map((s, i) => {
          const label = t.t(LABEL[s.step]);
          const isCurrent = i === currentIndex;
          const clickable = s.enabled && s.step !== 'Finish';
          return (
            <li
              key={s.step}
              className="mainbar-item"
              data-active={s.active || undefined}
              data-current={isCurrent || undefined}
            >
              <button
                type="button"
                className="mainbar-step"
                disabled={!clickable}
                onClick={() => press(s)}
                aria-current={isCurrent ? 'step' : undefined}
              >
                <span className="mainbar-index" aria-hidden="true">
                  {i + 1}
                </span>
                <span className="mainbar-label">{label}</span>
                {s.active && !isCurrent ? <span className="visually-hidden"> ({t.t('ng-ui-step-done')})</span> : null}
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
