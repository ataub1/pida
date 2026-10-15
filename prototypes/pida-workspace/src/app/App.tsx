/**
 * App-Rahmen des Prototyps: Kopfzeile, Hauptbereich je Ansicht, Overlays (Signieren, Einstellungen,
 * Befehlspalette, Tastenkürzel, Ersteinrichtung), Benachrichtigungen, Demo-Steuerung und globale
 * Tastenkürzel. Keine Netzwerkzugriffe.
 */
import { useCallback, useEffect, useMemo, useRef, type ReactNode } from 'react';
import { Button as AriaButton, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components';
import { BatchView } from '../features/batch/BatchView';
import { DemoPanel } from '../features/demo/DemoPanel';
import { DoneView } from '../features/done/DoneView';
import { Onboarding } from '../features/onboarding/Onboarding';
import { CommandPalette, ShortcutsDialog, type Command } from '../features/palette/CommandPalette';
import { SettingsPanel } from '../features/settings/SettingsPanel';
import { SigningCard } from '../features/signing/SigningCard';
import { isPdfName } from '../features/start/PdfDropZone';
import { StartView } from '../features/start/StartView';
import { VerifyView } from '../features/verify/VerifyView';
import { Workspace } from '../features/workspace/Workspace';
import { LangProvider, useT } from '../i18n';
import { contractDoc, verifyDoc } from '../model/documents';
import type { ThemePref } from '../model/types';
import { IconButton, Keys, PlatformContext, Segmented, useModKey } from '../ui/controls';
import { Wordmark } from '../ui/Logo';
import {
  IconArrowLeft,
  IconCheck,
  IconFileText,
  IconFolder,
  IconGlobe,
  IconHome,
  IconKeyboard,
  IconLayers,
  IconMonitor,
  IconMoon,
  IconSearch,
  IconSettings,
  IconShieldCheck,
  IconSignature,
  IconSun,
} from '../ui/Icons';
import { Toasts } from '../ui/Toasts';
import { activeDoc, StoreProvider, useAppState, useDispatch, type AppState } from './store';
import { DEMO_TIMING, TimingContext, type Timing } from './timing';

export interface AppProps {
  initial: AppState;
  timing?: Timing;
  isMac?: boolean;
}

export function App({ initial, timing = DEMO_TIMING, isMac = false }: AppProps) {
  return (
    <StoreProvider initial={initial}>
      <TimingContext.Provider value={timing}>
        <PlatformContext.Provider value={{ isMac }}>
          <Localized>
            <Shell />
          </Localized>
        </PlatformContext.Provider>
      </TimingContext.Provider>
    </StoreProvider>
  );
}

function Localized({ children }: { children: ReactNode }) {
  const { settings } = useAppState();
  return <LangProvider lang={settings.uiLang}>{children}</LangProvider>;
}

/** Bevorzugungen pro Betrachter (localStorage, fehlertolerant). */
function persist(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* kein Speicher (privates Fenster, Vorschau) */
  }
}

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

function Shell() {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const { mod, enter } = useModKey();
  const fileInput = useRef<HTMLInputElement>(null);
  const stateRef = useRef(state);
  stateRef.current = state;

  // Farbschema und Sprache am Dokument
  useEffect(() => {
    const root = document.documentElement;
    if (state.settings.theme === 'system') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', state.settings.theme);
    persist('pida.theme', state.settings.theme);
  }, [state.settings.theme]);
  useEffect(() => {
    document.documentElement.lang = state.settings.uiLang;
    persist('pida.lang', state.settings.uiLang);
  }, [state.settings.uiLang]);
  useEffect(() => {
    if (state.onboarded) persist('pida.onboarded', '1');
  }, [state.onboarded]);

  // Fokus-Rückfall: Schließt ein Overlay, dessen Auslöser nicht mehr existiert (z. B. Einstellungen aus der
  // Befehlspalette), stünde der Fokus sonst auf <body> (WCAG 2.4.3). Dann auf den Hauptbereich setzen.
  const overlayOpen = !!state.overlay || !!state.signing;
  const wasOpen = useRef(overlayOpen);
  useEffect(() => {
    const was = wasOpen.current;
    wasOpen.current = overlayOpen;
    if (!was || overlayOpen) return undefined;
    const timer = setTimeout(() => {
      const active = document.activeElement;
      if (!active || active === document.body || !active.isConnected) document.getElementById('main')?.focus({ preventScroll: true });
    }, 60);
    return () => clearTimeout(timer);
  }, [overlayOpen]);

  const openPicker = useCallback(() => fileInput.current?.click(), []);

  const onFiles = useCallback(
    (names: string[]) => {
      if (stateRef.current.view === 'verify') dispatch({ type: 'openVerify', spec: verifyDoc(names[0]) });
      else dispatch({ type: 'openFiles', names });
    },
    [dispatch],
  );

  const canSign = (() => {
    if (state.view === 'batch') return state.docs.some((d) => d.status === 'ready' || d.status === 'positioned');
    if (state.view !== 'workspace') return false;
    const d = activeDoc(state);
    return !!d && d.unlocked && d.status !== 'done';
  })();

  // Globale Tastenkürzel
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = stateRef.current;
      const modKey = e.metaKey || e.ctrlKey;
      const k = e.key.toLowerCase();
      if (modKey && k === 'k') {
        e.preventDefault();
        if (!s.signing) dispatch({ type: 'overlay', overlay: s.overlay === 'palette' ? null : 'palette' });
        return;
      }
      if (s.overlay || s.signing) return;
      if (modKey && k === 'o') {
        e.preventDefault();
        openPicker();
      } else if (modKey && e.key === ',') {
        e.preventDefault();
        dispatch({ type: 'overlay', overlay: 'settings' });
      } else if (modKey && e.key === 'Enter') {
        e.preventDefault();
        dispatch({ type: 'startSigning' });
      } else if (e.key === '?' && !isTyping(e.target) && !modKey) {
        e.preventDefault();
        dispatch({ type: 'overlay', overlay: 'shortcuts' });
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [dispatch, openPicker]);

  const setTheme = (theme: ThemePref) => dispatch({ type: 'settings', patch: { theme } });

  const commands: Command[] = useMemo(
    () => [
      { id: 'open', label: t('pal.open'), group: 'actions', icon: <IconFolder size={18} />, keys: [mod, 'O'], run: openPicker },
      { id: 'sign', label: t('pal.sign'), group: 'actions', icon: <IconSignature size={18} />, keys: [mod, enter], isDisabled: !canSign, run: () => dispatch({ type: 'startSigning' }) },
      { id: 'verify', label: t('pal.verify'), group: 'actions', icon: <IconShieldCheck size={18} />, keywords: 'prüfen verify', run: () => dispatch({ type: 'openVerify', spec: null }) },
      { id: 'sample', label: t('pal.sample'), group: 'actions', icon: <IconFileText size={18} />, run: () => dispatch({ type: 'openSpec', spec: contractDoc('Vertrag.pdf', `doc-sample-${Date.now()}`) }) },
      { id: 'settings', label: t('pal.settings'), group: 'actions', icon: <IconSettings size={18} />, keys: [mod, ','], run: () => dispatch({ type: 'overlay', overlay: 'settings' }) },
      { id: 'block', label: t('pal.block'), group: 'actions', icon: <IconSignature size={18} />, keywords: 'logo profil', run: () => dispatch({ type: 'overlay', overlay: 'settings', section: 'block' }) },
      { id: 'home', label: t('pal.home'), group: 'actions', icon: <IconHome size={18} />, run: () => dispatch({ type: 'goHome' }) },
      { id: 'theme-light', label: t('pal.theme.light'), group: 'view', icon: <IconSun size={18} />, keywords: 'theme hell', run: () => setTheme('light') },
      { id: 'theme-dark', label: t('pal.theme.dark'), group: 'view', icon: <IconMoon size={18} />, keywords: 'theme dunkel', run: () => setTheme('dark') },
      { id: 'theme-system', label: t('pal.theme.system'), group: 'view', icon: <IconMonitor size={18} />, keywords: 'theme system', run: () => setTheme('system') },
      { id: 'lang-de', label: t('pal.lang.de'), group: 'view', icon: <IconGlobe size={18} />, keywords: 'language sprache', run: () => dispatch({ type: 'settings', patch: { uiLang: 'de' } }) },
      { id: 'lang-en', label: t('pal.lang.en'), group: 'view', icon: <IconGlobe size={18} />, keywords: 'language sprache', run: () => dispatch({ type: 'settings', patch: { uiLang: 'en' } }) },
      { id: 'shortcuts', label: t('pal.shortcuts'), group: 'help', icon: <IconKeyboard size={18} />, keys: ['?'], run: () => dispatch({ type: 'overlay', overlay: 'shortcuts' }) },
    ],
    [t, mod, enter, canSign, openPicker, dispatch],
  );

  return (
    <div className={`app view-${state.view}`}>
      <a className="skip-link" href="#main">
        {t('app.skip')}
      </a>
      <Header onCommands={() => dispatch({ type: 'overlay', overlay: 'palette' })} />
      <main id="main" className="app-main" tabIndex={-1}>
        {state.view === 'start' ? <StartView onPickFiles={openPicker} onFiles={onFiles} /> : null}
        {state.view === 'workspace' ? <Workspace onOpenFiles={openPicker} /> : null}
        {state.view === 'batch' ? <BatchView onAddFiles={openPicker} /> : null}
        {state.view === 'done' ? <DoneView onNext={() => dispatch({ type: 'goHome' })} /> : null}
        {state.view === 'verify' ? <VerifyView onPickFiles={openPicker} /> : null}
      </main>
      <input
        ref={fileInput}
        type="file"
        accept="application/pdf,.pdf"
        multiple
        hidden
        aria-label={t('file.input')}
        data-testid="file-input"
        onChange={(e) => {
          const files = Array.from(e.currentTarget.files ?? []);
          const names = files.map((f) => f.name);
          names.filter((n) => !isPdfName(n)).forEach((n) => dispatch({ type: 'toast', toast: { kind: 'error', title: t('file.notPdf', { name: n }) } }));
          const pdfs = names.filter(isPdfName);
          if (pdfs.length) onFiles(pdfs);
          e.currentTarget.value = '';
        }}
      />
      <SigningCard />
      <SettingsPanel />
      <CommandPalette commands={commands} />
      <ShortcutsDialog />
      <Onboarding />
      <Toasts />
      <DemoPanel />
    </div>
  );
}

function Header({ onCommands }: { onCommands: () => void }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const { mod } = useModKey();
  const doc = activeDoc(state);
  const themeIcon = state.settings.theme === 'dark' ? <IconMoon size={18} /> : state.settings.theme === 'light' ? <IconSun size={18} /> : <IconMonitor size={18} />;

  let context: ReactNode = null;
  if (state.view === 'workspace' && doc) {
    context = (
      <div className="header-context">
        {state.batchMode ? (
          <IconButton label={t('header.backToBatch')} onPress={() => dispatch({ type: 'backToBatch' })}>
            <IconArrowLeft size={18} />
          </IconButton>
        ) : null}
        <span className="header-doc">
          <IconFileText size={17} className="header-doc-icon" />
          <span className="header-doc-name">{doc.spec.fileName}</span>
          <span className="header-doc-meta">{t('header.docInfo', { pages: t.n('common.pages', doc.spec.pages.length), size: doc.spec.sizeKb })}</span>
        </span>
      </div>
    );
  } else if (state.view === 'batch') {
    context = (
      <div className="header-context">
        <span className="header-doc">
          <IconLayers size={17} className="header-doc-icon" />
          <span className="header-doc-name">{t('batch.title')}</span>
          <span className="header-doc-meta">{t.n('common.docs', state.docs.length)}</span>
        </span>
      </div>
    );
  } else if (state.view === 'verify') {
    context = (
      <div className="header-context">
        <span className="header-doc">
          <IconShieldCheck size={17} className="header-doc-icon" />
          <span className="header-doc-name">{t('vf.title')}</span>
        </span>
      </div>
    );
  }

  return (
    <header className="app-header">
      <AriaButton className="brand" aria-label={t('header.home')} onPress={() => dispatch({ type: 'goHome' })}>
        <Wordmark />
        <span className="proto-tag" aria-hidden="true">
          {t('app.prototype')}
        </span>
      </AriaButton>
      {context}
      <div className="header-spacer" />
      <div className="header-actions">
        {state.view !== 'verify' ? (
          <AriaButton className="btn btn-ghost btn-md header-verify" onPress={() => dispatch({ type: 'openVerify', spec: null })} aria-label={t('header.verifyLong')}>
            <IconShieldCheck size={18} />
            <span className="hide-narrow">{t('header.verify')}</span>
          </AriaButton>
        ) : null}
        <AriaButton className="command-button" onPress={onCommands} aria-label={t('header.commandsLong')}>
          <IconSearch size={16} />
          <span className="command-button-text">{t('header.commands')}</span>
          <Keys keys={[mod, 'K']} />
        </AriaButton>
        <div className="hide-narrow">
          <Segmented
            label={t('header.lang')}
            hideLabel
            size="sm"
            value={state.settings.uiLang}
            onChange={(v) => dispatch({ type: 'settings', patch: { uiLang: v } })}
            options={[
              { value: 'de', label: 'DE' },
              { value: 'en', label: 'EN' },
            ]}
          />
        </div>
        <MenuTrigger>
          <IconButton label={t('header.theme')} className="header-theme">
            {themeIcon}
          </IconButton>
          <Popover className="popover" placement="bottom end" offset={6}>
            <Menu
              className="menu"
              aria-label={t('header.theme')}
              selectionMode="single"
              selectedKeys={[state.settings.theme]}
              onSelectionChange={(keys) => {
                const k = [...(keys as Set<string>)][0];
                if (k) dispatch({ type: 'settings', patch: { theme: k as ThemePref } });
              }}
              disallowEmptySelection
            >
              {(
                [
                  ['system', <IconMonitor key="m" size={18} />],
                  ['light', <IconSun key="s" size={18} />],
                  ['dark', <IconMoon key="d" size={18} />],
                ] as const
              ).map(([id, icon]) => (
                <MenuItem key={id} id={id} className="menu-item" textValue={t(`theme.${id}`)}>
                  {({ isSelected }) => (
                    <>
                      <span className="menu-item-icon">{icon}</span>
                      <span className="menu-item-label">{t(`theme.${id}`)}</span>
                      {isSelected ? <IconCheck size={16} className="menu-item-check" /> : null}
                    </>
                  )}
                </MenuItem>
              ))}
            </Menu>
          </Popover>
        </MenuTrigger>
        <IconButton label={t('header.settings')} onPress={() => dispatch({ type: 'overlay', overlay: 'settings' })}>
          <IconSettings size={18} />
        </IconButton>
      </div>
    </header>
  );
}
