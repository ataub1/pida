/**
 * Kopfzeile (Gestalt aus dem Prototyp, ADR 0004 §3 „Informationsarchitektur“): Wortmarke (→ Start, ohne
 * „Prototyp“-Plakette), Dokumentkontext (Name, Seiten, Größe; aus dem Stapel geöffnet mit „Zurück zum
 * Stapel“; daneben „+ PDF“ mit „Weitere PDFs hinzufügen“ und „Anderes PDF öffnen“, Nachtrag 9) bzw.
 * Stapelkontext („Stapel · 4 Dokumente“), Schalter „Barrierefreie Bedienung“ (docs/analyse/06), „Befehle ⌘K“, DE/EN (`set_ui_language`), Farbschema (lokal je Gerät)
 * und Einstellungen (`actions.settings`). Kein Knopf „Prüfen“: den Prüfmodus gibt es nicht mehr (ADR 0004
 * Nachtrag 15).
 *
 * „Zur Startseite“ (Wortmarke, Haus-Symbol) schließt nichts: Der Kern parkt das offene Dokument bzw. den Stapel
 * (`go_home`, ADR 0004 Nachtrag 10), die Startansicht bietet „Weiter bearbeiten“ – deshalb ohne Rückfrage.
 */
import { Button as AriaButton, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components';
import { isMobile } from '../core/types';
import { A11ySwitch } from '../features/accessibility/A11ySwitch';
import { HELP_AVAILABLE } from '../features/help';
import { PALETTE_AVAILABLE } from '../features/palette';
import { AddPdfMenu } from '../features/workspace';
import { formatSize } from '../features/workspace/placement';
import { IconButton, Keys, Segmented, useModKey } from '../ui/controls';
import { IconArrowLeft, IconCheck, IconFileText, IconHelp, IconHome, IconLayers, IconMonitor, IconMoon, IconSearch, IconSettings, IconSun } from '../ui/Icons';
import { Wordmark } from '../ui/Logo';
import { batchNav, goHome } from './commands';
import { useApp } from './context';
import type { Theme } from './theme';

export function Header({ theme, onTheme }: { theme: Theme; onTheme: (theme: Theme) => void }) {
  const { t, view, dispatch, openSettings, openHelp, setOverlay } = useApp();
  const { mod } = useModKey();
  const doc = view.document;
  const mobile = isMobile(view);
  const themeIcon = theme === 'dark' ? <IconMoon size={18} /> : theme === 'light' ? <IconSun size={18} /> : <IconMonitor size={18} />;
  const home = () => void goHome(view, dispatch);
  // Haus-Symbol (Wunsch des Auftraggebers): sichtbar, sobald man nicht auf der Startansicht ist; beim Signieren
  // und im Stapellauf gesperrt (`actions.home`). Offene Arbeit parkt der Kern, nichts geht verloren.
  const awayFromStart = !!view.document || !!view.batch;
  const pages = doc ? doc.doc.page_count : 0;
  const batch = view.batch;
  // aus dem Stapel geöffnet (`return_to_batch`) bzw. weitere Dokumente in der Warteschlange (`show_batch`)
  const nav = doc ? batchNav(view) : null;

  return (
    <header className="app-header">
      <AriaButton className="brand" aria-label={t.t('ng-ui-shell-home')} onPress={home}>
        <Wordmark tagline={t.t('ng-ui-brand-tagline')} />
      </AriaButton>
      {awayFromStart && view.actions.home ? (
        <IconButton label={t.t('ng-ui-shell-cmd-home')} className="header-home" onPress={home}>
          <IconHome size={18} />
        </IconButton>
      ) : null}
      {doc && doc.phase !== 'done' ? (
        <div className="header-context">
          {nav ? (
            <IconButton label={t.t(nav === 'back' ? 'ng-ui-batch-back' : 'ng-ui-batch-show')} className="header-back" onPress={() => void dispatch({ type: nav === 'back' ? 'back_to_batch' : 'show_batch' })}>
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
          {/* „+ PDF“ nur, solange das Dokument bearbeitet wird (beim Signieren liegt die Karte darüber) */}
          {doc.phase === 'editing' || doc.phase === 'warning' ? <AddPdfMenu doc={doc} /> : null}
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
        {/* Barrierefreie Bedienung (docs/analyse/06): zusätzlich zu den übrigen Symbolen, ersetzt keines */}
        <A11ySwitch />
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
        {HELP_AVAILABLE ? (
          <IconButton label={t.t('ng-ui-shell-help')} onPress={() => openHelp()} aria-keyshortcuts={mobile ? undefined : 'F1'}>
            <IconHelp size={18} />
          </IconButton>
        ) : null}
        <IconButton label={t.t('ng-ui-shell-settings')} onPress={() => openSettings()} isDisabled={!view.actions.settings || view.settings !== null} aria-keyshortcuts={mobile ? undefined : `${mod === '⌘' ? 'Meta' : 'Control'}+,`}>
          <IconSettings size={18} />
        </IconButton>
      </div>
    </header>
  );
}
