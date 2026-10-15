/**
 * Wurzel der Oberfläche: abonniert den `ViewState`, rendert Hauptleiste + Schritt + Einstellungen,
 * verteilt Drag & Drop, Zugriffstasten (`&`-Mnemonics: Alt+Taste, macOS ⌃⌥+Taste), Tastenkürzel
 * (Strg/⌘+O, Strg/⌘+,), Fokuswechsel beim Schrittwechsel, Live-Region und Hinweise.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { I18nProvider } from 'react-aria-components';
import { MainBar } from '../components/MainBar';
import { Toasts } from '../components/Toasts';
import type { Bridge, FileDragEvent } from '../core/bridge';
import { mainBar } from '../core/defaults';
import { isMobile, type StepView, type UiEvent, type ViewState } from '../core/types';
import { translator } from '../i18n/catalog';
import { BackendScreen } from '../screens/BackendScreen';
import { OpenScreen } from '../screens/OpenScreen';
import { OutputScreen } from '../screens/OutputScreen';
import { PlaceholderScreen } from '../screens/PlaceholderScreen';
import { PositionScreen } from '../screens/PositionScreen';
import { SigningScreen } from '../screens/SigningScreen';
import { ErrorScreen, FinishedScreen, LoadingScreen, StartingScreen, WarningScreen } from '../screens/StatusScreens';
import { EMBLEM_DROP_EVENT, SettingsDialog } from '../settings/SettingsDialog';
import { applyTheme, readTheme } from '../settings/theme';
import { AppContext, type AppContextValue, type Toast } from './context';
import { MigrationDialog } from './MigrationDialog';
import { ResumeDialog } from './ResumeDialog';

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
};

const STEP_TITLE: Record<StepView['step'], string> = {
  starting: 'ng-ui-starting',
  open: 'ng-ui-open-title',
  loading: 'ng-ui-open-analyzing',
  warning: 'ng-ui-warning-title',
  placeholder: 'ng-ui-placeholder-title',
  position: 'ng-ui-position-title',
  backend: 'ng-ui-backend-title',
  signing: 'main-signature',
  output: 'main-done',
  error: 'error-Title',
  finished: 'ng-ui-finished',
};

const IMAGE_EXT = /\.(png|jpe?g|gif)$/i;
const INTERACTIVE = 'button, a[href], input, select, textarea, [role="tab"], [role="radio"], [role="checkbox"], [role="button"], [tabindex]:not([tabindex="-1"])';

/** Zugriffstaste aus einem Tastaturereignis (`KeyB` → `b`), plattformgerecht. */
export function accessKeyFromEvent(e: KeyboardEvent, isMac: boolean): string | null {
  const modifier = isMac ? e.ctrlKey && e.altKey && !e.metaKey : e.altKey && !e.ctrlKey && !e.metaKey;
  if (!modifier) return null;
  const m = /^(?:Key([A-Z])|Digit([0-9]))$/.exec(e.code);
  if (!m) return null;
  return (m[1] ?? m[2] ?? '').toLowerCase() || null;
}

/**
 * Löst eine Zugriffstaste in der obersten Ebene (Dialog bzw. Seite) aus. Teilen sich mehrere
 * Bedienelemente dieselbe Taste (Kataloge aus PDF-Over, z. B. „Sprache des Signaturblocks“ und „Logo
 * zurücksetzen“ mit R), wandert der Fokus wie unter Windows reihum zum nächsten Element, ohne etwas
 * auszulösen – so bleibt jedes Element per Tastatur erreichbar.
 */
export function triggerAccessKey(key: string, root: Document = document): boolean {
  const layers = root.querySelectorAll<HTMLElement>('[data-modal-layer]');
  const scope: ParentNode = layers.length > 0 ? (layers[layers.length - 1] as HTMLElement) : root;
  const candidates = Array.from(scope.querySelectorAll<HTMLElement>(`[data-accesskey="${key}"]`));
  const targets: HTMLElement[] = [];
  for (const el of candidates) {
    if (el.closest('[inert], [hidden], [aria-hidden="true"]')) continue;
    const target = el.closest<HTMLElement>(INTERACTIVE) ?? el.closest<HTMLElement>('.field, .radio-group, fieldset')?.querySelector<HTMLElement>(INTERACTIVE) ?? null;
    if (!target) continue;
    if (target.matches(':disabled') || target.getAttribute('aria-disabled') === 'true' || target.hasAttribute('data-disabled')) continue;
    if (!targets.includes(target)) targets.push(target);
  }
  const first = targets[0];
  if (!first) return false;
  if (targets.length > 1) {
    const active = root.activeElement;
    const index = targets.findIndex((el) => el === active || (active !== null && el.contains(active)));
    (targets[(index + 1) % targets.length] ?? first).focus();
    return true;
  }
  first.focus();
  if (!first.matches('input, select, textarea') && !first.getAttribute('aria-haspopup')) first.click();
  return true;
}

export function App({ bridge, isMac: isMacProp }: { bridge: Bridge; isMac?: boolean }) {
  const [view, setView] = useState<ViewState | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [liveText, setLiveText] = useState('');
  const [dragActive, setDragActive] = useState(false);
  const toastId = useRef(0);
  const isMac = isMacProp ?? (typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent));

  const current = view ?? STARTING;
  const t = translator(current.language);

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

  useEffect(() => {
    applyTheme(readTheme());
  }, []);

  useEffect(() => {
    document.documentElement.lang = current.language;
  }, [current.language]);

  const toast = useCallback((text: string, kind: Toast['kind'] = 'info') => {
    toastId.current += 1;
    const id = toastId.current;
    setToasts((list) => [...list.slice(-3), { id, kind, text }]);
  }, []);
  const dismiss = useCallback((id: number) => setToasts((list) => list.filter((x) => x.id !== id)), []);

  const announce = useCallback((text: string) => {
    setLiveText('');
    setTimeout(() => setLiveText(text), 30);
  }, []);

  const dispatch = useCallback(
    (event: UiEvent): Promise<boolean> =>
      bridge.dispatch(event).then(
        () => true,
        (e: unknown) => {
          const code = e instanceof Error ? e.message : 'ng-ui-bridge-error';
          toast(translator(current.language).code(code, {}, 'ng-ui-bridge-error'), 'error');
          return false;
        },
      ),
    [bridge, toast, current.language],
  );

  // --- Dateien öffnen --------------------------------------------------------------------------
  const mobile = isMobile(current);
  // Auch an der Dokumentwurzel: Dialoge (React Aria `Modal`) liegen als Portal außerhalb von `.app`;
  // Stilregeln für Mobile (z. B. keine Zugriffstasten-Unterstreichung) gelten so auch dort.
  useEffect(() => {
    document.documentElement.toggleAttribute('data-mobile', mobile);
  }, [mobile]);
  const browse = useCallback(async () => {
    if (mobile) {
      // Mobile: Dokument-Picker bzw. SAF über das Plugin; Kopien in der Sandbox (keine Pfade der Quelle)
      try {
        const paths = await bridge.pickMobileFiles({ kind: 'pdf', multiple: true });
        if (paths.length > 0) void dispatch({ type: 'open_files', paths });
      } catch (e) {
        const code = e instanceof Error ? e.message : 'ng-ui-dialog-failed';
        toast(t.code(code, {}, 'ng-ui-dialog-failed'), 'error');
      }
      return;
    }
    try {
      const paths = await bridge.pickFiles({
        title: t.t('dataSourceSelection-browse'),
        multiple: true,
        filters: [
          { name: t.t('common-PDFExtension_Description'), extensions: ['pdf'] },
          { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
        ],
      });
      if (paths && paths.length > 0) void dispatch({ type: 'open_files', paths });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  }, [bridge, dispatch, t, toast, mobile]);

  // --- Drag & Drop -----------------------------------------------------------------------------
  const stepName = current.step.step;
  const settingsOpen = current.settings !== null;
  const dropState = useRef({ stepName, settingsOpen });
  dropState.current = { stepName, settingsOpen };
  useEffect(() => {
    return bridge.onFileDrag((e: FileDragEvent) => {
      if (e.type === 'enter' || e.type === 'over') {
        setDragActive(true);
        return;
      }
      setDragActive(false);
      if (e.type !== 'drop' || e.paths.length === 0) return;
      const { stepName: s, settingsOpen: so } = dropState.current;
      if (so) {
        const hit = typeof document.elementFromPoint === 'function' ? document.elementFromPoint(e.x, e.y) : null;
        const image = e.paths.find((p) => IMAGE_EXT.test(p));
        if (image && hit?.closest('[data-emblem-drop]')) {
          window.dispatchEvent(new CustomEvent(EMBLEM_DROP_EVENT, { detail: image }));
        }
        return;
      }
      if (s === 'open' || s === 'loading' || s === 'output') {
        void dispatch({ type: 'open_files', paths: e.paths });
      } else {
        toast(translator(current.language).t('ng-ui-drop-not-here'));
      }
    });
  }, [bridge, dispatch, toast, current.language]);

  // --- Tastatur --------------------------------------------------------------------------------
  const keyState = useRef({ current, browse, dispatch });
  keyState.current = { current, browse, dispatch };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const { current: v, browse: b, dispatch: d } = keyState.current;
      const mod = isMac ? e.metaKey : e.ctrlKey;
      if (mod && !e.altKey && !e.shiftKey && e.key === ',') {
        if (v.main_bar.settings_enabled && !v.settings) {
          e.preventDefault();
          void d({ type: 'open_settings' });
        }
        return;
      }
      if (mod && !e.altKey && !e.shiftKey && (e.key === 'o' || e.key === 'O')) {
        if (!v.settings && v.step.step === 'open') {
          e.preventDefault();
          void b();
        } else if (!v.settings && v.main_bar.steps.find((s) => s.step === 'Open')?.enabled) {
          e.preventDefault();
          void d({ type: 'navigate', step: 'Open' });
        }
        return;
      }
      const key = accessKeyFromEvent(e, isMac);
      if (key && triggerAccessKey(key)) e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [isMac]);

  // --- Fokus und Titel beim Schrittwechsel -----------------------------------------------------
  useEffect(() => {
    document.title = `PIDA – ${t.t(STEP_TITLE[stepName])}`;
  }, [stepName, t]);
  const firstStep = useRef(true);
  useEffect(() => {
    if (firstStep.current) {
      firstStep.current = false;
      return;
    }
    const id = setTimeout(() => {
      const active = document.activeElement;
      if (active && active !== document.body && document.body.contains(active)) return;
      document.getElementById('screen-heading')?.focus();
    }, 0);
    return () => clearTimeout(id);
  }, [stepName]);

  const ctx = useMemo<AppContextValue>(
    () => ({ bridge, view: current, dispatch, t, announce, toast, dragActive, isMac }),
    [bridge, current, dispatch, t, announce, toast, dragActive, isMac],
  );

  return (
    <I18nProvider locale={current.language === 'de' ? 'de-AT' : 'en-GB'}>
      <AppContext.Provider value={ctx}>
        <div className="app" data-step={stepName} data-drag={dragActive || undefined} data-mobile={mobile || undefined} data-os={current.platform.os}>
          <a className="skip-link" href="#main">
            {t.t('ng-ui-skip-to-content')}
          </a>
          <header className="topbar">
            <div className="brand" aria-hidden="true">
              <img src="/app-icon.svg" alt="" width={28} height={28} />
              <span>PIDA</span>
            </div>
            <MainBar onOpenFiles={() => void browse()} />
          </header>
          <main id="main" className="content" tabIndex={-1}>
            <Screen step={current.step} onBrowse={() => void browse()} />
          </main>
          {current.settings ? <SettingsDialog settingsView={current.settings} /> : null}
          {current.migration && !current.settings ? <MigrationDialog offer={current.migration} /> : null}
          {current.resume && !current.settings && !current.migration ? <ResumeDialog offer={current.resume} /> : null}
          <Toasts toasts={toasts} onDismiss={dismiss} notices={current.notices} />
          <div className="visually-hidden" aria-live="polite" aria-atomic="true" data-testid="live-region">
            {liveText}
          </div>
        </div>
      </AppContext.Provider>
    </I18nProvider>
  );
}

function Screen({ step, onBrowse }: { step: StepView; onBrowse: () => void }) {
  switch (step.step) {
    case 'starting':
      return <StartingScreen />;
    case 'open':
      return <OpenScreen queue={step.queue} onBrowse={onBrowse} />;
    case 'loading':
      return <LoadingScreen queue={step.queue} />;
    case 'warning':
      return <WarningScreen doc={step.doc} codes={step.codes} />;
    case 'placeholder':
      return <PlaceholderScreen doc={step.doc} fields={step.fields} qr={step.qr} />;
    case 'position':
      return <PositionScreen view={step} />;
    case 'backend':
      return <BackendScreen options={step.options} privacyNotice={step.privacy_notice} />;
    case 'signing':
      return <SigningScreen interaction={step.interaction} queue={step.queue} />;
    case 'output':
      return <OutputScreen result={step.result} next={step.next} />;
    case 'error':
      return <ErrorScreen code={step.code} params={step.params} retry={step.retry} />;
    case 'finished':
      return <FinishedScreen exitCode={step.exit_code} />;
  }
}
