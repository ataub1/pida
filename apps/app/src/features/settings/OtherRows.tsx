/**
 * Einstellungen › Darstellung, › Datenschutz, › Erweitert, › Über (ADR 0004 §3.11).
 *
 * - Farbschema bleibt UI-lokal je Gerät (`pida.theme`, `app/theme.ts`); die Oberflächensprache ist
 *   `general.ui_language` (sofort wirksam).
 * - Datenschutz (ADR 0003): Datenschutztext (bzw. Text der Organisation), lokales Signieren, A-Trust-Link,
 *   Zertifikats-Cache, „Zuletzt signiert merken“ (`general.recent_documents`; Abschalten löscht die Liste im
 *   Kern sofort) und „Liste leeren“ (`clear_recent`). Kein Dialog, kein Zustimmungszwang.
 * - Erweitert: Proxy, Update-Prüfung (Desktop), „PDF-Over-Einstellungen übernehmen“ (`offer_migration`, das
 *   Angebot erscheint direkt im Bereich; Desktop), Zurücksetzen (mit Rückfrage – das ist
 *   unumkehrbar). „Ausführliche Protokolle“ entfällt (kein Settings-Feld, `--log-level`).
 * - Über PIDA: eigenes Modul `about.tsx` (Symbol, Version mit Datum, Update-Prüfung, Links, Urheberrecht; ADR 0004
 *   Nachtrag 21).
 */
import { useState } from 'react';
import { useApp } from '../../app/context';
import type { Theme } from '../../app/theme';
import type { Lang, ProxyMode } from '../../core/types';
import { AccessButton, Button, SwitchField } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/Dialog';
import { IconMonitor, IconMoon, IconSun, IconTrash } from '../../ui/Icons';
import { MigrationOfferPane } from '../onboarding';
import { ChoiceSegments, SettingTextField } from './controls';
import { PrivacyInfo } from './privacy';
import { useSettings, useTextSetting } from './state';

/* ------------------------------------------------------------------ Darstellung */

export function ThemeRow() {
  // dasselbe Farbschema wie die Kopfzeile (App-Kontext): eine Änderung hier zeigt sie dort sofort
  const { t, theme, setTheme } = useApp();
  return (
    <ChoiceSegments<Theme>
      label={t.t('ng-ui-set-theme')}
      value={theme}
      onChange={setTheme}
      options={[
        { value: 'system', label: t.t('ng-ui-set-theme-system'), icon: <IconMonitor size={16} /> },
        { value: 'light', label: t.t('ng-ui-set-theme-light'), icon: <IconSun size={16} /> },
        { value: 'dark', label: t.t('ng-ui-set-theme-dark'), icon: <IconMoon size={16} /> },
      ]}
    />
  );
}

export function UiLangRow() {
  const { t } = useApp();
  const s = useSettings();
  const v = s.get<Lang | null>('general.ui_language');
  return (
    <ChoiceSegments<'system' | Lang>
      label={t.t('ng-ui-set-ui-lang')}
      value={v ?? 'system'}
      onChange={(x) => s.apply({ 'general.ui_language': x === 'system' ? null : x })}
      options={[
        { value: 'system', label: t.t('ng-ui-set-lang-system') },
        { value: 'de', label: t.t('ng-ui-set-lang-de') },
        { value: 'en', label: t.t('ng-ui-set-lang-en') },
      ]}
    />
  );
}

/* ------------------------------------------------------------------ Datenschutz */

/** Überschrift des Datenschutztexts (Fokusziel des Verweises aus „Über“). */
export const PRIVACY_HEADING_ID = 'settings-privacy-heading';

export function PrivacyInfoRow() {
  const { t } = useApp();
  return (
    <div className="privacy-info">
      <h4 className="settings-row-title" id={PRIVACY_HEADING_ID} tabIndex={-1}>
        {t.t('ng-ui-set-privacy-info')}
      </h4>
      <PrivacyInfo />
    </div>
  );
}

export function CertCacheRow() {
  const { t } = useApp();
  const s = useSettings();
  return (
    <SwitchField
      label={t.t('ng-ui-set-certcache')}
      hint={t.t('ng-ui-certcache-hint')}
      isSelected={s.get<boolean>('certificate_cache.enabled')}
      onChange={(v) => s.apply({ 'certificate_cache.enabled': v })}
    />
  );
}

export function RecentRow() {
  const { t, view, dispatch, toast, announce } = useApp();
  const s = useSettings();
  const locked = s.locked('general.recent_documents');
  const on = s.get<boolean>('general.recent_documents');
  const count = view.recent?.length ?? 0;
  const hint = locked ? `${t.t('ng-ui-set-recent-hint')} ${locked}` : t.t('ng-ui-set-recent-hint');
  return (
    <div className="stack-3">
      <SwitchField label={t.t('ng-ui-set-recent')} hint={hint} isSelected={locked ? view.recent !== null : on} isDisabled={!!locked} onChange={(v) => s.apply({ 'general.recent_documents': v })} />
      {view.recent !== null ? (
        <div className="recent-clear">
          <p className="muted">{t.t('ng-ui-set-recent-count', { n: count })}</p>
          <Button
            variant="secondary"
            size="sm"
            isDisabled={count === 0}
            onPress={() => {
              void dispatch({ type: 'clear_recent' }).then((ok) => {
                if (!ok) return;
                toast(t.t('ng-ui-set-recent-cleared'), 'success');
                announce(t.t('ng-ui-set-recent-cleared'));
              });
            }}
          >
            <IconTrash size={15} />
            {t.t('ng-ui-set-recent-clear')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

/* ------------------------------------------------------------------ Erweitert */

export function ProxyRow() {
  const { t } = useApp();
  const s = useSettings();
  const mode = s.get<ProxyMode>('proxy.mode');
  const locked = s.locked('proxy.mode');
  const host = useTextSetting(
    'proxy.host',
    (v) => (typeof v === 'string' ? v : ''),
    (text) => ({ value: text.trim() === '' ? null : text.trim() }),
  );
  const port = useTextSetting(
    'proxy.port',
    (v) => (typeof v === 'number' ? String(v) : ''),
    (text) => ({ value: text === '' ? null : Number(text) }),
  );
  const user = useTextSetting(
    'proxy.user',
    (v) => (typeof v === 'string' ? v : ''),
    (text) => ({ value: text.trim() === '' ? null : text }),
  );
  const manual = mode === 'manual';
  return (
    <div className="stack-3">
      <ChoiceSegments<ProxyMode>
        label={t.t('ng-ui-set-proxy')}
        value={mode}
        isDisabled={!!locked}
        description={locked}
        onChange={(v) => s.apply({ 'proxy.mode': v })}
        options={[
          { value: 'system', label: t.t('ng-ui-proxy-system') },
          { value: 'none', label: t.t('ng-ui-proxy-none') },
          { value: 'manual', label: t.t('ng-ui-proxy-manual') },
        ]}
      />
      {manual ? (
        <>
          <div className="grid-2">
            <SettingTextField label={t.t('ng-ui-set-proxy-host')} placeholder="proxy.example.at" isDisabled={!!locked} {...host} />
            {/* nur Ziffern; den Bereich prüft der Kern (`problems["proxy.port"]`) */}
            <SettingTextField label={t.t('ng-ui-set-proxy-port')} inputMode="numeric" placeholder="8080" isDisabled={!!locked} {...port} onChange={(v) => port.onChange(v.replace(/[^0-9]/g, '').slice(0, 5))} />
          </div>
          <SettingTextField label={t.t('ng-ui-set-proxy-user')} description={t.t('ng-ui-proxy-password-hint')} autoComplete="off" isDisabled={!!locked} {...user} />
        </>
      ) : null}
    </div>
  );
}

export function UpdatesRow() {
  const { t } = useApp();
  const s = useSettings();
  const locked = s.locked('general.update_check');
  return (
    <SwitchField
      label={t.t('ng-ui-set-updates')}
      hint={locked ? `${t.t('advanced_config-UpdateCheck_ToolTip')}. ${locked}` : t.t('advanced_config-UpdateCheck_ToolTip')}
      isSelected={s.get<boolean>('general.update_check')}
      isDisabled={!!locked}
      onChange={(v) => s.apply({ 'general.update_check': v })}
    />
  );
}

/**
 * Altkonfiguration erneut suchen und anbieten (`offer_migration`, auch nach „Nicht übernehmen“; ADR 0004
 * §3.12). Das Angebot (`ViewState::migration`) erscheint hier im Bereich statt als eigener Dialog.
 */
export function MigrationRow() {
  const { t, view, dispatch } = useApp();
  const offer = view.migration;
  return (
    <div className="stack-2">
      <span className="field-label">{t.t('ng-ui-set-migration')}</span>
      <p className="field-hint">{t.t('ng-ui-set-migration-hint')}</p>
      {offer ? (
        <div className="settings-migration-offer">
          <MigrationOfferPane offer={offer} level={4} titleId="set-migration-offer" />
        </div>
      ) : (
        // ohne Fund meldet der Kern `ng-migration-not-found` (Hinweis über dem Dialog, eine Quelle)
        <div className="button-row">
          <Button size="sm" onPress={() => void dispatch({ type: 'offer_migration' })}>
            {t.t('ng-ui-set-migration-search')}
          </Button>
        </div>
      )}
    </div>
  );
}

export function ResetRow() {
  const { t, dispatch } = useApp();
  const [confirm, setConfirm] = useState(false);
  return (
    <div className="stack-2">
      <span className="field-label">{t.t('ng-ui-set-reset')}</span>
      <p className="field-hint">{t.t('ng-ui-settings-reset-confirm')}</p>
      <div className="button-row">
        <AccessButton msg="ng-ui-settings-reset" variant="danger" size="sm" onPress={() => setConfirm(true)} />
      </div>
      {confirm ? (
        <ConfirmDialog
          title={t.t('ng-ui-settings-reset')}
          tone="warning"
          onCancel={() => setConfirm(false)}
          actions={
            <>
              <AccessButton msg="common-Cancel" variant="quiet" onPress={() => setConfirm(false)} autoFocus />
              <AccessButton
                msg="ng-ui-settings-reset-confirm-button"
                variant="danger"
                onPress={() => {
                  setConfirm(false);
                  void dispatch({ type: 'reset_settings' });
                }}
              />
            </>
          }
        >
          <p>{t.t('ng-ui-settings-reset-confirm')}</p>
        </ConfirmDialog>
      ) : null}
    </div>
  );
}
