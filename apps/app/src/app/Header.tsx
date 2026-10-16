/**
 * Kopfzeile (Gestalt aus dem Prototyp, ADR 0004 §3 „Informationsarchitektur“): Wortmarke (→ Start, ohne
 * „Prototyp“-Plakette), Dokumentkontext (Name, Seiten, Größe; aus dem Stapel geöffnet mit „Zurück zum
 * Stapel“) bzw. Stapelkontext („Stapel · 4 Dokumente“), „Prüfen“ (Platzhalter bis zum Prüfmodus,
 * Freigabe `actions.verify`), „Befehle ⌘K“ (sobald die Palette da ist, B4), DE/EN (`set_ui_language`),
 * Farbschema (lokal je Gerät) und Einstellungen (`actions.settings`).
 */
import { Button as AriaButton, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components';
import { isMobile } from '../core/types';
import { PALETTE_AVAILABLE } from '../features/palette';
import { formatSize } from '../features/workspace/placement';
import { IconButton, Keys, Segmented, useModKey } from '../ui/controls';
import { IconArrowLeft, IconCheck, IconFileText, IconLayers, IconMonitor, IconMoon, IconSearch, IconSettings, IconShieldCheck, IconSun } from '../ui/Icons';
import { Wordmark } from '../ui/Logo';
import { useApp } from './context';
import type { Theme } from './theme';

export function Header({ theme, onTheme, onVerify }: { theme: Theme; onTheme: (theme: Theme) => void; onVerify: () => void }) {
  const { t, view, dispatch, openSettings, setOverlay } = useApp();
  const { mod } = useModKey();
  const doc = view.document;
  const mobile = isMobile(view);
  const themeIcon = theme === 'dark' ? <IconMoon size={18} /> : theme === 'light' ? <IconSun size={18} /> : <IconMonitor size={18} />;
  const home = () => {
    if (view.actions.home) void dispatch({ type: 'navigate', step: 'Open' });
  };
  const pages = doc ? doc.doc.page_count : 0;
  const batch = view.batch;
  // aus dem Stapel geöffnet (`return_to_batch`) bzw. weitere Dokumente in der Warteschlange (`show_batch`)
  const backToBatch = !!doc && !!batch && batch.open_item === doc.doc.id && (doc.phase === 'editing' || doc.phase === 'warning');
  const showBatch = !!doc && !backToBatch && view.actions.show_batch;

  return (
    <header className="app-header">
      <AriaButton className="brand" aria-label={t.t('ng-ui-shell-home')} onPress={home}>
        <Wordmark />
      </AriaButton>
      {doc && doc.phase !== 'done' ? (
        <div className="header-context">
          {backToBatch || showBatch ? (
            <IconButton label={t.t(backToBatch ? 'ng-ui-batch-back' : 'ng-ui-batch-show')} onPress={() => void dispatch({ type: backToBatch ? 'back_to_batch' : 'show_batch' })}>
              <IconArrowLeft size={18} />
            </IconButton>
          ) : null}
          <span className="header-doc">
            <IconFileText size={17} className="header-doc-icon" />
            <span className="header-doc-name">{doc.doc.name}</span>
            <span className="header-doc-meta">
              {t.t('ng-ui-shell-doc-info', { pages: t.t('ng-ui-shell-pages', { n: pages }), size: formatSize(doc.size_bytes, t.lang) })}
            </span>
          </span>
        </div>
      ) : !doc && batch ? (
        <div className="header-context">
          <span className="header-doc">
            <IconLayers size={17} className="header-doc-icon" />
            <span className="header-doc-name">{t.t('ng-ui-batch-title')}</span>
            <span className="header-doc-meta">{t.t('ng-ui-batch-docs', { n: batch.items.length })}</span>
          </span>
        </div>
      ) : null}
      <div className="header-spacer" />
      <div className="header-actions">
        <AriaButton className="btn btn-ghost btn-md header-verify" onPress={onVerify} isDisabled={!view.actions.verify} aria-label={t.t('ng-ui-shell-verify-long')}>
          <IconShieldCheck size={18} />
          <span className="hide-narrow">{t.t('ng-ui-shell-verify')}</span>
        </AriaButton>
        {PALETTE_AVAILABLE ? (
          <AriaButton className="command-button" onPress={() => setOverlay('palette')} aria-label={t.t('ng-ui-shell-commands-long')}>
            <IconSearch size={16} />
            <span className="command-button-text">{t.t('ng-ui-shell-commands')}</span>
            {mobile ? null : <Keys keys={[mod, 'K']} />}
          </AriaButton>
        ) : null}
        <div className="hide-narrow">
          <Segmented
            label={t.t('ng-ui-shell-lang')}
            hideLabel
            size="sm"
            value={view.language}
            onChange={(v) => void dispatch({ type: 'set_ui_language', language: v })}
            options={[
              { value: 'de', label: 'DE' },
              { value: 'en', label: 'EN' },
            ]}
          />
        </div>
        <MenuTrigger>
          <IconButton label={t.t('ng-ui-shell-theme')} className="header-theme">
            {themeIcon}
          </IconButton>
          <Popover className="popover" placement="bottom end" offset={6}>
            <Menu
              className="menu"
              aria-label={t.t('ng-ui-shell-theme')}
              selectionMode="single"
              selectedKeys={[theme]}
              disallowEmptySelection
              onSelectionChange={(keys) => {
                const k = [...(keys as Set<string>)][0];
                if (k === 'system' || k === 'light' || k === 'dark') onTheme(k);
              }}
            >
              {(
                [
                  ['system', <IconMonitor key="m" size={18} />],
                  ['light', <IconSun key="s" size={18} />],
                  ['dark', <IconMoon key="d" size={18} />],
                ] as const
              ).map(([id, icon]) => (
                <MenuItem key={id} id={id} className="menu-item" textValue={t.t(`ng-ui-shell-theme-${id}`)}>
                  {({ isSelected }) => (
                    <>
                      <span className="menu-item-icon">{icon}</span>
                      <span className="menu-item-label">{t.t(`ng-ui-shell-theme-${id}`)}</span>
                      {isSelected ? <IconCheck size={16} className="menu-item-check" /> : null}
                    </>
                  )}
                </MenuItem>
              ))}
            </Menu>
          </Popover>
        </MenuTrigger>
        <IconButton label={t.t('ng-ui-shell-settings')} onPress={() => openSettings()} isDisabled={!view.actions.settings || view.settings !== null} aria-keyshortcuts={mobile ? undefined : `${mod === '⌘' ? 'Meta' : 'Control'}+,`}>
          <IconSettings size={18} />
        </IconButton>
      </div>
    </header>
  );
}
