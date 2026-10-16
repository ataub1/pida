/**
 * Wurzel der Oberfläche „Arbeitsbereich“ (ADR 0004 §4.2): abonniert den `ViewState` und rendert je Zustand
 * eine Ansicht – Start, Arbeitsbereich (Warnung, Positionieren, Signieren als Overlay), Fertig, Stapel
 * (Übersicht, Lauf mit der Stapelkarte als Overlay), Zustandskarten – dazu Kopfzeile, Sprunglink,
 * Signierkarte, Overlays der Pakete UI-B (Einstellungen, Ersteinrichtung,
 * Prüfmodus, Palette), „Signatur fortsetzen“, Hinweise und die Live-Region.
 *
 * Shell-Dienste: Dateiauswahl (Desktop-Dialog bzw. Mobile-Picker), Drag & Drop über die Fenster-Ereignisse
 * (Pfade; auf „+ PDF hinzufügen“ der Miniaturleiste wie „Weitere PDFs hinzufügen“), Tastenkürzel ⌘/Strg+O
 * (Öffnen), ⌘/Strg+↵ (Signieren – eine ausdrückliche Aktion, A1), ⌘/Strg+B (Zum Stapel bzw. Zurück zum Stapel),
 * ⌘/Strg+, (Einstellungen), ⌘/Strg+K und „?“ (sobald die Palette da ist), Zugriffstasten (Alt+Taste bzw. ⌃⌥+Taste),
 * Farbschema, Fokus beim Ansichtswechsel. Der Kern bleibt maßgeblich: Freigaben kommen aus
 * `ViewState::actions`. Bei geparkter Arbeit („Zur Startseite“, ADR 0004 Nachtrag 10) fragen gewählte bzw.
 * abgelegte PDFs erst nach, ob sie zum Stapel kommen (`ParkedJoinDialog`).
 *
 * Der Kern läuft in der App immer im Arbeitsbereich-Modus (`workspace_ui`, ADR 0004 §3.14): `StepView::Position`
 * kommt dort nur mit `document`, `StepView::Placeholder` gar nicht. Die bisherigen Bildschirme (`screens/`) sind
 * mit dem Merge von A1 entfernt (§3.15); ein klassischer Zustand ohne Dokument zeigt die Startansicht.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { I18nProvider } from 'react-aria-components';
import type { Bridge, FileDragEvent } from '../core/bridge';
import { mainBar, workspaceDefaults } from '../core/defaults';
import { isMobile, type OpenFilesMode, type UiEvent, type ViewState } from '../core/types';
import { BatchView } from '../features/batch';
import { DoneView, OutputDialogs } from '../features/done';
import { Onboarding } from '../features/onboarding';
import { CommandPalette, PALETTE_AVAILABLE, ShortcutsDialog } from '../features/palette';
import { SettingsPanel } from '../features/settings';
import { cardMode, errorInCard, SigningCard } from '../features/signing';
import { ParkedJoinDialog, StartView } from '../features/start';
import { VERIFY_AVAILABLE, VerifyView } from '../features/verify';
import { ErrorCard, FinishedCard, LoadingCard, StartingCard, WarningCard, Workspace } from '../features/workspace';
import { translator } from '../i18n/catalog';
import { accessKeyFromEvent, trackAltKey, triggerAccessKey } from '../ui/AccessKeys';
import { Toasts } from '../ui/Toasts';
import { batchNav, buildCommands, buildShortcuts } from './commands';
import { AppContext, type AppContextValue, type SettingsSectionId, type ShellOverlay, type Toast } from './context';
import { EMBLEM_DROP_EVENT } from './events';
import { Header } from './Header';
import { ResumeDialog } from './ResumeDialog';
import { applyTheme, readTheme, writeTheme, type Theme } from './theme';
import './shell.css';

/** Zustand bis zum ersten `ViewState` des Kerns. */
const STARTING: ViewState = {
  step: { step: 'starting' },
  main_bar: mainBar(['', '', '', ''], false),
  settings: null,
  language: 'de',
  notices: [],
  migration: null,
  done: [],
  // bis zum ersten ViewState: Desktop-Bedienelemente (der Startschritt hat keine plattformabhängigen)
  platform: { os: 'linux', mobile: false },
  resume: null,
  keystore_import: null,
  privacy_text: null,
  ...workspaceDefaults(),
};

const IMAGE_EXT = /\.(png|jpe?g|gif)$/i;

/** Ansicht des Hauptbereichs. */
export type MainView = 'starting' | 'start' | 'loading' | 'workspace' | 'warning' | 'done' | 'error' | 'finished' | 'verify' | 'batch';

/**
 * Ansicht je Zustand (ADR 0004 §3.1: `document` gesetzt in Warnung, Positionieren, Signieren, Fertig; im
 * Stapellauf liegt die Karte über der Stapelübersicht, §3.8).
 */
export function mainView(view: ViewState): MainView {
  if (VERIFY_AVAILABLE && view.verify) return 'verify';
  const s = view.step;
  const doc = view.document;
  switch (s.step) {
    case 'starting':
      return 'starting';
    case 'finished':
      return 'finished';
    case 'open':
      return 'start';
    case 'batch':
      return view.batch ? 'batch' : 'start';
    case 'loading':
      return 'loading';
    case 'warning':
      return doc ? 'workspace' : 'warning';
    case 'signing':
      return doc ? 'workspace' : view.batch ? 'batch' : 'start';
    case 'placeholder':
    case 'position':
    case 'backend':
      return doc ? 'workspace' : 'start';
    case 'output':
      return s.result.path || s.result.save_pending ? 'done' : doc ? 'workspace' : 'done';
    case 'error':
      return errorInCard(view) ? (doc ? 'workspace' : 'start') : 'error';
  }
}

/** Titel des Fensters je Ansicht. */
const TITLE: Record<MainView, string> = {
  starting: 'ng-ui-shell-starting',
  start: 'ng-ui-start-title',
  loading: 'ng-ui-ws-loading-title',
  workspace: 'ng-ui-shell-title-workspace',
  warning: 'ng-ui-warning-title',
  done: 'ng-ui-fin-title',
  error: 'ng-ui-sc-error-title',
  finished: 'ng-ui-finished',
  verify: 'ng-ui-shell-verify-mode',
  batch: 'ng-ui-batch-title',
};

export { accessKeyFromEvent, triggerAccessKey };

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || !el.tagName) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

export function App({ bridge, isMac: isMacProp }: { bridge: Bridge; isMac?: boolean }) {
  const [view, setView] = useState<ViewState | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [liveText, setLiveText] = useState('');
  const [dragActive, setDragActive] = useState(false);
  const [section, setSection] = useState<SettingsSectionId>('signature');
  const [overlay, setOverlay] = useState<ShellOverlay>(null);
  const [theme, setThemeState] = useState<Theme>(() => readTheme());
  const toastId = useRef(0);
  const signHandler = useRef<(() => void) | null>(null);
  const isMac = isMacProp ?? (typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent));

  const current = view ?? STARTING;
  const t = translator(current.language);
  const mobile = isMobile(current);
  const main = mainView(current);

  // --- Zustand vom Kern ------------------------------------------------------------------------
  useEffect(() => {
    let alive = true;
    const off = bridge.onState((v) => alive && setView(v));
    bridge
      .view()
      .then((v) => alive && setView((cur) => cur ?? v))
      .catch(() => undefined);
    return () => {
      alive = false;
      off();
    };
  }, [bridge]);

  useEffect(() => applyTheme(theme), [theme]);
  useEffect(() => trackAltKey(), []);
  useEffect(() => {
    document.documentElement.lang = current.language;
  }, [current.language]);
  // Auch an der Dokumentwurzel: Dialoge (React Aria `Modal`) liegen als Portal außerhalb von `.app`
  useEffect(() => {
    document.documentElement.toggleAttribute('data-mobile', mobile);
  }, [mobile]);

  const setTheme = useCallback((next: Theme) => {
    writeTheme(next);
    setThemeState(next);
  }, []);

  const toast = useCallback((text: string, kind: Toast['kind'] = 'info', detail?: string) => {
    toastId.current += 1;
    const id = toastId.current;
    setToasts((list) => [...list.slice(-3), { id, kind, text, detail }]);
  }, []);
  const dismiss = useCallback((id: number) => setToasts((list) => list.filter((x) => x.id !== id)), []);

  const announce = useCallback((text: string) => {
    setLiveText('');
    setTimeout(() => setLiveText(text), 30);
  }, []);

  const language = current.language;
  const dispatch = useCallback(
    (event: UiEvent): Promise<boolean> =>
      bridge.dispatch(event).then(
        () => true,
        (e: unknown) => {
          const code = e instanceof Error ? e.message : 'ng-ui-bridge-error';
          toast(translator(language).code(code, {}, 'ng-ui-bridge-error'), 'error');
          return false;
        },
      ),
    [bridge, toast, language],
  );

  const openSettings = useCallback(
    (next: SettingsSectionId = 'signature') => {
      setSection(next);
      void dispatch({ type: 'open_settings' });
    },
    [dispatch],
  );

  // --- Dateien öffnen --------------------------------------------------------------------------
  // Geparkte Arbeit („Zur Startseite“, ADR 0004 Nachtrag 10): neue PDFs aus der Dateiauswahl bzw. abgelegte fragen
  // vorher, ob sie zum Stapel kommen (`ParkedJoinDialog`); „Öffnen mit“ und zweite Instanz entscheidet der Kern
  const [joinPaths, setJoinPaths] = useState<string[] | null>(null);
  const parkedRef = useRef(current.parked ?? null);
  parkedRef.current = current.parked ?? null;
  useEffect(() => {
    if (!current.parked) setJoinPaths(null);
  }, [current.parked]);

  // `mode` aus „+ PDF“ (ADR 0004 Nachtrag 9): „Anderes PDF öffnen“ wählt eine Datei, sonst Mehrfachauswahl;
  // was daraus folgt (Stapel, Schließen), entscheidet der Kern
  const browse = useCallback(
    async (mode?: OpenFilesMode) => {
      const multiple = mode !== 'replace';
      const open = (paths: string[]) => {
        if (!mode && parkedRef.current) setJoinPaths(paths);
        else void dispatch(mode ? { type: 'open_files', paths, mode } : { type: 'open_files', paths });
      };
      if (mobile) {
        // Mobile: Dokument-Picker bzw. SAF über das Plugin; Kopien in der Sandbox (keine Pfade der Quelle)
        try {
          const paths = await bridge.pickMobileFiles({ kind: 'pdf', multiple });
          if (paths.length > 0) open(paths);
        } catch (e) {
          const code = e instanceof Error ? e.message : 'ng-ui-dialog-failed';
          toast(t.code(code, {}, 'ng-ui-dialog-failed'), 'error');
        }
        return;
      }
      try {
        const paths = await bridge.pickFiles({
          title: t.t('dataSourceSelection-browse'),
          multiple,
          filters: [
            { name: t.t('common-PDFExtension_Description'), extensions: ['pdf'] },
            { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
          ],
        });
        if (paths && paths.length > 0) open(paths);
      } catch {
        toast(t.t('ng-ui-dialog-failed'), 'error');
      }
    },
    [bridge, dispatch, t, toast, mobile],
  );

  /** Prüfmodus: Datei wählen → `open_verify` (Freigabe `actions.verify`, ab K4). */
  const openVerify = useCallback(async () => {
    if (!current.actions.verify) return;
    try {
      const paths = mobile
        ? await bridge.pickMobileFiles({ kind: 'pdf', multiple: false })
        : await bridge.pickFiles({ title: t.t('ng-ui-shell-verify-long'), multiple: false, filters: [{ name: t.t('common-PDFExtension_Description'), extensions: ['pdf'] }] });
      const path = paths?.[0];
      if (path) void dispatch({ type: 'open_verify', path });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  }, [bridge, current.actions.verify, dispatch, mobile, t, toast]);

  const requestSign = useCallback(() => signHandler.current?.(), []);
  const registerSign = useCallback((handler: (() => void) | null) => {
    signHandler.current = handler;
  }, []);

  // --- Drag & Drop -----------------------------------------------------------------------------
  const dropState = useRef({ current, dispatch, toast });
  dropState.current = { current, dispatch, toast };
  useEffect(() => {
    return bridge.onFileDrag((e: FileDragEvent) => {
      if (e.type === 'enter' || e.type === 'over') {
        setDragActive(true);
        return;
      }
      setDragActive(false);
      if (e.type !== 'drop' || e.paths.length === 0) return;
      const { current: v, dispatch: d, toast: tst } = dropState.current;
      // Prüfmodus: die abgelegte Datei prüfen (wie im Prototyp), nicht verdeckt im Arbeitsbereich öffnen
      if (VERIFY_AVAILABLE && v.verify && !v.settings) {
        const path = e.paths[0];
        if (v.actions.verify && path) void d({ type: 'open_verify', path });
        else tst(translator(v.language).t('ng-ui-drop-not-here'));
        return;
      }
      if (v.settings) {
        const hit = typeof document.elementFromPoint === 'function' ? document.elementFromPoint(e.x, e.y) : null;
        const image = e.paths.find((p) => IMAGE_EXT.test(p));
        if (image && hit?.closest('[data-emblem-drop]')) window.dispatchEvent(new CustomEvent(EMBLEM_DROP_EVENT, { detail: image }));
        return;
      }
      // auf „+ PDF hinzufügen“ der Miniaturleiste: wie „Weitere PDFs hinzufügen“ (Nachtrag 9)
      const hit = typeof document.elementFromPoint === 'function' ? document.elementFromPoint(e.x, e.y) : null;
      const add = !!hit?.closest('[data-add-drop]');
      if (!v.actions.open_files) tst(translator(v.language).t('ng-ui-drop-not-here'));
      // auf die Startseite mit geparkter Arbeit: erst fragen (Nachtrag 10)
      else if (!add && v.parked) setJoinPaths(e.paths);
      else void d(add ? { type: 'open_files', paths: e.paths, mode: 'add' } : { type: 'open_files', paths: e.paths });
    });
  }, [bridge]);

  // --- Tastatur --------------------------------------------------------------------------------
  const keyState = useRef({ current, browse, openSettings, requestSign, overlay, dispatch });
  keyState.current = { current, browse, openSettings, requestSign, overlay, dispatch };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const { current: v, browse: b, openSettings: settings, requestSign: sign, overlay: o, dispatch: d } = keyState.current;
      const mod = isMac ? e.metaKey : e.ctrlKey;
      const k = e.key.toLowerCase();
      const blocked = o !== null || v.settings !== null || document.querySelector('[data-modal-layer]') !== null;
      if (mod && !e.altKey && !e.shiftKey && k === 'k' && PALETTE_AVAILABLE) {
        e.preventDefault();
        if (o === 'palette') {
          setOverlay(null);
          return;
        }
        // nicht beim Signieren und nicht über anderen Dialogen (Ersteinrichtung, Rückfragen, „Signatur
        // fortsetzen“ …); über den Einstellungen und der Kürzelübersicht wie im Prototyp
        const otherDialog = Array.from(document.querySelectorAll('[data-modal-layer]')).some((el) => !el.closest('.settings-modal, .shortcuts-modal, .palette-modal'));
        if (v.step.step !== 'signing' && !otherDialog) setOverlay('palette');
        return;
      }
      if (mod && !e.altKey && !e.shiftKey && e.key === ',') {
        if (v.actions.settings && !blocked) {
          e.preventDefault();
          settings();
        }
        return;
      }
      if (mod && !e.altKey && !e.shiftKey && k === 'o') {
        if (v.actions.open_files && !blocked) {
          e.preventDefault();
          void b();
        }
        return;
      }
      if (mod && !e.altKey && !e.shiftKey && k === 'b') {
        // ⌘/Strg+B = „Zum Stapel“ bzw. „Zurück zum Stapel“ (Freigabe `actions.show_batch` bzw. aus dem Stapel geöffnet)
        const nav = batchNav(v);
        if (nav && !blocked) {
          e.preventDefault();
          void d({ type: nav === 'back' ? 'back_to_batch' : 'show_batch' });
        }
        return;
      }
      if (mod && !e.altKey && !e.shiftKey && e.key === 'Enter') {
        // ⌘↵ = „Signieren“ bzw. im Stapel „Alle signieren“ (ausdrückliche Aktion, A1)
        if ((v.actions.sign || v.actions.start_batch) && !blocked) {
          e.preventDefault();
          sign();
        }
        return;
      }
      if (e.key === '?' && !mod && !isTyping(e.target) && PALETTE_AVAILABLE && !blocked) {
        e.preventDefault();
        setOverlay('shortcuts');
        return;
      }
      const key = accessKeyFromEvent(e, isMac);
      if (key && triggerAccessKey(key)) e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [isMac]);

  // --- Fokus und Titel beim Ansichtswechsel ------------------------------------------------------
  useEffect(() => {
    document.title = `PIDA – ${t.t(TITLE[main])}`;
  }, [main, t]);
  const firstView = useRef(true);
  useEffect(() => {
    if (firstView.current) {
      firstView.current = false;
      return undefined;
    }
    const id = setTimeout(() => {
      const active = document.activeElement;
      if (active && active !== document.body && document.body.contains(active)) return;
      (document.querySelector<HTMLElement>('[data-main-heading]') ?? document.getElementById('screen-heading') ?? document.getElementById('main'))?.focus({ preventScroll: true });
    }, 0);
    return () => clearTimeout(id);
  }, [main]);
  // Fokus-Rückfall: Schließt ein Overlay, dessen Auslöser nicht mehr existiert, stünde der Fokus auf <body>
  const overlayOpen = current.settings !== null || cardMode(current) !== null || overlay !== null;
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

  const ctx = useMemo<AppContextValue>(
    () => ({ bridge, view: current, dispatch, t, announce, toast, dragActive, isMac, theme, setTheme, openSettings, settingsSection: section, browse, overlay, setOverlay, requestSign, registerSign }),
    [bridge, current, dispatch, t, announce, toast, dragActive, isMac, theme, setTheme, openSettings, section, browse, overlay, requestSign, registerSign],
  );

  const commands = useMemo(
    () =>
      buildCommands({
        view: current,
        t,
        isMac,
        dispatch,
        browse,
        requestSign,
        openSettings,
        setTheme,
        setOverlay,
        openVerify: () => void openVerify(),
      }),
    [current, t, isMac, dispatch, browse, requestSign, openSettings, setTheme, openVerify],
  );

  const step = current.step;
  return (
    <I18nProvider locale={current.language === 'de' ? 'de-AT' : 'en-GB'}>
      <AppContext.Provider value={ctx}>
        <div
          className={`app view-${main === 'workspace' ? 'workspace' : main === 'done' ? 'done' : main === 'verify' ? 'verify' : main === 'batch' ? 'batch' : 'start'}`}
          data-step={step.step}
          data-view={main}
          data-drag={dragActive || undefined}
          data-mobile={mobile || undefined}
          data-os={current.platform.os}
        >
          <a className="skip-link" href="#main">
            {t.t('ng-ui-shell-skip')}
          </a>
          <Header theme={theme} onTheme={setTheme} onVerify={() => void openVerify()} />
          <main id="main" className="app-main" tabIndex={-1}>
            {main === 'starting' ? <StartingCard /> : null}
            {main === 'start' ? <StartView onVerify={() => void openVerify()} /> : null}
            {main === 'loading' && step.step === 'loading' ? <LoadingCard step={step} /> : null}
            {main === 'workspace' && current.document ? <Workspace doc={current.document} /> : null}
            {main === 'batch' && current.batch ? <BatchView batch={current.batch} /> : null}
            {main === 'warning' && step.step === 'warning' ? <WarningCard step={step} /> : null}
            {main === 'done' && step.step === 'output' ? <DoneView step={step} /> : null}
            {main === 'error' && step.step === 'error' ? <ErrorCard step={step} /> : null}
            {main === 'finished' && step.step === 'finished' ? <FinishedCard exitCode={step.exit_code} /> : null}
            {main === 'verify' && current.verify ? <VerifyView verify={current.verify} /> : null}
          </main>
          <SigningCard />
          {step.step === 'output' ? <OutputDialogs result={step.result} /> : null}
          {current.settings ? <SettingsPanel settingsView={current.settings} section={section} /> : null}
          {(current.migration || current.first_run) && !current.settings ? <Onboarding migration={current.migration} firstRun={current.first_run} /> : null}
          {current.resume && !current.settings && !current.migration ? <ResumeDialog offer={current.resume} /> : null}
          {joinPaths && current.parked ? <ParkedJoinDialog parked={current.parked} paths={joinPaths} onClose={() => setJoinPaths(null)} /> : null}
          <CommandPalette isOpen={overlay === 'palette'} onClose={() => setOverlay(null)} commands={commands} />
          <ShortcutsDialog isOpen={overlay === 'shortcuts'} onClose={() => setOverlay(null)} commands={commands} shortcuts={buildShortcuts(t, isMac)} />
          <Toasts toasts={toasts} onDismiss={dismiss} notices={current.notices} />
          <div className="sr-only" aria-live="polite" aria-atomic="true" data-testid="live-region">
            {liveText}
          </div>
        </div>
      </AppContext.Provider>
    </I18nProvider>
  );
}
