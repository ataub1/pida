/**
 * Sofort wirksame Einstellungen (ADR 0004 §3.11): Jede Änderung geht als `apply_settings` mit **nur den
 * geänderten Pfaden** (TOML-Punktpfad → serde-JSON des Felds) und einer steigenden `revision` an den Kern –
 * kein „Speichern“, kein „Abbrechen“. Maßgeblich bleibt `SettingsView::settings`; bis der Kern eine Änderung
 * bestätigt (`applied_revision ≥ revision`), zeigt die Oberfläche den gesendeten Wert (Antworten mit älterer
 * `applied_revision` überschreiben ihn nicht). Lehnt der Kern ab, steht der bisherige Wert wieder da und das
 * Feld zeigt `problems[pfad]`.
 *
 * Textfelder (`useTextSetting`) senden entprellt (400 ms nach der letzten Eingabe bzw. beim Verlassen) und
 * halten ungültige Eingaben als Eingabezustand, bis sie gültig sind; `Settings::unknown` fassen sie nie an.
 * Keine Fachlogik: Validierung, Normalisierung und Beispiele kommen vom Kern.
 */
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useApp } from '../../app/context';
import type { Settings, SettingsView } from '../../core/types';

/**
 * Grund (i18n-ID), falls eine Richtlinie den Pfad festlegt (`SettingsView::locked`, Nachtrag 5); sonst `null`.
 * Ein ganzer Abschnitt sperrt alle seine Pfade (`proxy` sperrt `proxy.mode`, `proxy.host` …).
 */
export function lockedReason(view: SettingsView, path: string): string | null {
  const locked = view.locked;
  if (!locked) return null;
  const direct = locked[path];
  if (direct) return direct;
  const section = path.split('.')[0] ?? '';
  return locked[section] ?? null;
}

/** Entprellung der Textfelder (ADR 0004 §3.11). */
export const TEXT_DELAY_MS = 400;

interface Pending {
  value: unknown;
  revision: number;
}

export interface SettingsState {
  view: SettingsView;
  /** aktueller Wert eines Pfads (gesendete, noch unbestätigte Änderung zuerst) */
  get: <T>(path: string) => T;
  /** Änderungen sofort übernehmen (eine Revision für alle Pfade) */
  apply: (changes: Record<string, unknown>) => void;
  /** noch nicht bestätigte Änderung für den Pfad */
  isPending: (path: string) => boolean;
  /** übersetztes Problem des Kerns für den Pfad */
  problem: (path: string) => string | null;
  /** übersetzter Grund, falls eine Richtlinie den Pfad festlegt */
  locked: (path: string) => string | null;
  /** Mobile-Oberfläche (ohne Proxy, Ordner, Updates, Protokolle; nur ID Austria – kein Keystore, ADR 0004 Nachtrag 11) */
  mobile: boolean;
  /** offene Texteingaben sofort senden (vor dem Schließen) */
  flush: () => void;
  /** Textfeld meldet seine Sofort-Senden-Funktion an; liefert die Abmeldung */
  onFlush: (fn: () => void) => () => void;
}

const Ctx = createContext<SettingsState | null>(null);

export function useSettings(): SettingsState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('SettingsProvider fehlt');
  return ctx;
}

/** Wert eines Punktpfads (`signature.profile`) in den Einstellungen. */
export function valueAt(settings: Settings, path: string): unknown {
  const [section, field] = path.split('.');
  const s = section ? (settings as Record<string, unknown>)[section] : undefined;
  if (!field) return s;
  return s && typeof s === 'object' ? (s as Record<string, unknown>)[field] : undefined;
}

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function SettingsProvider({ view, mobile, children }: { view: SettingsView; mobile: boolean; children: ReactNode }) {
  const { dispatch, t } = useApp();
  const [pending, setPending] = useState<Record<string, Pending>>({});
  const revision = useRef(view.applied_revision);
  const flushers = useRef(new Set<() => void>());
  const applied = view.applied_revision;

  // bestätigte Änderungen verwerfen (der Kern ist wieder maßgeblich)
  useEffect(() => {
    setPending((p) => {
      const left = Object.entries(p).filter(([, x]) => x.revision > applied);
      return left.length === Object.keys(p).length ? p : Object.fromEntries(left);
    });
  }, [applied]);

  const live = useMemo(() => Object.fromEntries(Object.entries(pending).filter(([, x]) => x.revision > applied)), [pending, applied]);

  const get = useCallback(<T,>(path: string): T => (path in live ? (live[path]?.value as T) : (valueAt(view.settings, path) as T)), [live, view.settings]);

  const apply = useCallback(
    (changes: Record<string, unknown>) => {
      const entries = Object.entries(changes).filter(([path, value]) => !same(value, path in live ? live[path]?.value : valueAt(view.settings, path)));
      if (entries.length === 0) return;
      revision.current = Math.max(revision.current, applied) + 1;
      const rev = revision.current;
      setPending((p) => ({ ...p, ...Object.fromEntries(entries.map(([path, value]) => [path, { value, revision: rev }])) }));
      void dispatch({ type: 'apply_settings', changes: Object.fromEntries(entries), revision: rev });
    },
    [applied, dispatch, live, view.settings],
  );

  const flush = useCallback(() => {
    for (const fn of flushers.current) fn();
  }, []);
  const onFlush = useCallback((fn: () => void) => {
    flushers.current.add(fn);
    return () => {
      flushers.current.delete(fn);
    };
  }, []);

  const value = useMemo<SettingsState>(
    () => ({
      view,
      get,
      apply,
      isPending: (path) => path in live,
      problem: (path) => {
        const id = view.problems[path];
        return id ? t.code(id, {}, 'ng-ui-problem-invalid') : null;
      },
      locked: (path) => {
        const id = lockedReason(view, path);
        return id ? t.code(id, {}, 'ng-ui-set-policy') : null;
      },
      mobile,
      flush,
      onFlush,
    }),
    [view, get, apply, live, t, mobile, flush, onFlush],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

/**
 * Textfeld einer Einstellung: lokaler Entwurf beim Tippen, entprellt gesendet; `parse` liefert `null`, solange
 * nichts gesendet werden soll (z. B. leerer Zusatz während der Eingabe), `final` = beim Verlassen.
 */
export function useTextSetting(
  path: string,
  format: (value: unknown) => string,
  parse: (text: string, final: boolean) => { value: unknown } | null,
  delay = TEXT_DELAY_MS,
) {
  const s = useSettings();
  const core = format(s.get(path));
  const [draft, setDraft] = useState<string | null>(null);
  const [focused, setFocused] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [waiting, setWaiting] = useState(false);
  const problem = s.problem(path);
  const pending = s.isPending(path);

  const commit = (text: string, final: boolean) => {
    const parsed = parse(text, final);
    if (parsed) s.apply({ [path]: parsed.value });
  };
  const latest = useRef({ commit, draft });
  latest.current = { commit, draft };

  const cancelTimer = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    setWaiting(false);
  };
  // Schließen der Einstellungen: wartende Eingabe sofort senden (sonst käme sie nach `close_settings`)
  const { onFlush } = s;
  useEffect(
    () =>
      onFlush(() => {
        if (!timer.current) return;
        clearTimeout(timer.current);
        timer.current = null;
        const text = latest.current.draft;
        if (text !== null) latest.current.commit(text, true);
      }),
    [onFlush],
  );
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  // Entwurf aufgeben, sobald der Kern den Wert übernommen hat (Normalisierung sichtbar) – nicht beim Tippen,
  // nicht solange der Kern ein Problem meldet (ungültige Eingabe bleibt stehen)
  useEffect(() => {
    if (draft !== null && !focused && !waiting && !pending && !problem) setDraft(null);
  }, [draft, focused, waiting, pending, problem, core]);

  return {
    value: draft ?? core,
    error: problem,
    onChange: (text: string) => {
      setDraft(text);
      cancelTimer();
      setWaiting(true);
      timer.current = setTimeout(() => {
        timer.current = null;
        setWaiting(false);
        latest.current.commit(text, false);
      }, delay);
    },
    onFocus: () => setFocused(true),
    onBlur: () => {
      setFocused(false);
      const text = draft;
      const wasWaiting = timer.current !== null;
      cancelTimer();
      if (text !== null && (wasWaiting || parse(text, false) === null)) commit(text, true);
    },
  };
}
