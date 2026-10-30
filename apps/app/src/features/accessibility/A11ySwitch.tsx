/**
 * Schalter „Barrierefreie Bedienung“ in der Kopfzeile (docs/analyse/06) – neben Befehlen, Sprache, Farbschema, Hilfe
 * und Einstellungen, ersetzt nichts. Schmal nur das Symbol (der zugängliche Name bleibt). Setzt die Werte mit
 * `set_accessibility` (`withMode`) und sagt das Ergebnis an.
 *
 * Einschalten von außen: Direktlink `?barrierefrei=1` (z. B. von PDF-AS-WAI); im Web merkt sich der Browser den
 * Schalter (dort werden Einstellungen sonst nicht gespeichert, docs/integration/web.md) – nur eine Bequemlichkeit,
 * ohne Speicher (privates Fenster) gilt er für die Sitzung.
 */
import { useEffect, useId } from 'react';
import { Switch } from 'react-aria-components';
import { useApp } from '../../app/context';
import { webProfile } from '../../core/web-profile';
import { IconAccessibility } from '../../ui/Icons';
import { accessibilityOf, modeRequestedByUrl, withMode } from './index';

const WEB_KEY = 'pida-barrierefrei';

function rememberedOnWeb(): boolean {
  if (!webProfile()) return false;
  try {
    return window.localStorage.getItem(WEB_KEY) === '1';
  } catch {
    return false;
  }
}

function rememberOnWeb(on: boolean): void {
  if (!webProfile()) return;
  try {
    window.localStorage.setItem(WEB_KEY, on ? '1' : '0');
  } catch {
    // ohne Speicher (privates Fenster): gilt nur für diese Sitzung
  }
}

/** Einmal je Programmstart: Direktlink bzw. im Web der gemerkte Schalter. */
let startHandled = false;

export function A11ySwitch() {
  const { t, view, dispatch, announce } = useApp();
  const descId = useId();
  const a = accessibilityOf(view);
  const ready = !!view.accessibility;

  useEffect(() => {
    if (!ready || startHandled) return;
    startHandled = true;
    const wanted = (typeof window !== 'undefined' && modeRequestedByUrl(window.location.search)) || rememberedOnWeb();
    if (wanted && !a.mode) void dispatch({ type: 'set_accessibility', accessibility: withMode(a, true) });
  }, [ready, a, dispatch]);

  if (!ready) return null;
  const toggle = (on: boolean) => {
    rememberOnWeb(on);
    void dispatch({ type: 'set_accessibility', accessibility: withMode(a, on) });
    announce(t.t(on ? 'ng-ui-shell-a11y-on' : 'ng-ui-shell-a11y-off'));
  };
  return (
    <>
      <Switch className="a11y-switch" isSelected={a.mode} onChange={toggle} aria-label={t.t('ng-ui-shell-a11y')} aria-describedby={descId} data-testid="a11y-switch">
        <IconAccessibility size={18} />
        <span className="a11y-switch-text" aria-hidden="true">
          {t.t('ng-ui-shell-a11y')}
        </span>
        <span className="switch-track" aria-hidden="true">
          <span className="switch-thumb" />
        </span>
      </Switch>
      <span id={descId} className="sr-only">
        {t.t('ng-ui-shell-a11y-desc')}
      </span>
    </>
  );
}

/** Nur für Tests: Start erneut behandeln. */
export function resetA11yStartForTests(): void {
  startHandled = false;
}
