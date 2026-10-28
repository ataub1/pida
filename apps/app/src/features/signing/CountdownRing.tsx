/**
 * Countdown-Ring für die App-Bestätigung (aus dem Prototyp, ADR 0004 §4.2). `role="timer"` (nicht
 * laufend angesagt). Die Gesamtzeit kommt aus `timeout_secs` der Maske; die Oberfläche zählt selbst herunter.
 *
 * Mit `key` (Anfrage) zählt die Uhr ab der ersten Anzeige dieser Anfrage, auch wenn die Ansicht neu aufgebaut wird –
 * etwa nachdem iOS den Web-Prozess im Hintergrund beendet und PIDA die Oberfläche neu geladen hat, oder nachdem
 * Android die Activity samt WebView neu angelegt hat. Der Startzeitpunkt liegt deshalb zusätzlich im `localStorage`
 * (übersteht beides; `sessionStorage` gehört unter Android zur einzelnen WebView). Der Schlüssel enthält den
 * Vergleichswert, damit nach einem Neustart der App, der die Nummern der Anfragen neu beginnen lässt, kein alter
 * Startzeitpunkt gilt.
 */
import { useEffect, useRef, useState } from 'react';
import { useT } from '../../app/context';

/** Formatiert Sekunden als m:ss (aufgerundet). */
export function formatTime(totalSecs: number): string {
  const s = Math.max(0, Math.ceil(totalSecs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

const START_STORAGE = 'pida.countdown';
const starts = new Map<string, number>();

/** Startzeitpunkt der Anfrage `key` (erste Anzeige); ohne `key` jetzt. Es zählt immer nur eine Anfrage. */
export function countdownStart(key: string | undefined, now = Date.now()): number {
  if (key === undefined) return now;
  let known = starts.get(key);
  if (known === undefined) {
    try {
      const stored = JSON.parse(localStorage.getItem(START_STORAGE) ?? 'null') as { key?: unknown; start?: unknown } | null;
      if (stored?.key === key && typeof stored.start === 'number') known = stored.start;
    } catch {
      // kein Speicher (privater Modus, Tests): nur im Speicher der Seite
    }
  }
  // Uhr zurückgestellt: neu beginnen statt eine Restzeit über der Gesamtzeit anzuzeigen
  const start = known !== undefined && known <= now ? known : now;
  starts.set(key, start);
  try {
    localStorage.setItem(START_STORAGE, JSON.stringify({ key, start }));
  } catch {
    // siehe oben
  }
  return start;
}

export function useCountdown(totalSecs: number, tickMs: number, onExpire: () => void, key?: string) {
  const [remaining, setRemaining] = useState(() => Math.max(0, totalSecs - (Date.now() - countdownStart(key)) / 1000));
  const start = useRef(0);
  const expired = useRef(false);
  const cb = useRef(onExpire);
  cb.current = onExpire;
  useEffect(() => {
    start.current = countdownStart(key);
    expired.current = false;
    setRemaining(Math.max(0, totalSecs - (Date.now() - start.current) / 1000));
    const id = setInterval(() => {
      const left = totalSecs - (Date.now() - start.current) / 1000;
      setRemaining(Math.max(0, left));
      if (left <= 0 && !expired.current) {
        expired.current = true;
        clearInterval(id);
        cb.current();
      }
    }, tickMs);
    return () => clearInterval(id);
  }, [totalSecs, tickMs, key]);
  return remaining;
}

export function CountdownRing({ remaining, total, size = 76 }: { remaining: number; total: number; size?: number }) {
  const t = useT();
  const r = 30;
  const c = 2 * Math.PI * r;
  const frac = total > 0 ? Math.max(0, Math.min(1, remaining / total)) : 0;
  const text = formatTime(remaining);
  const low = remaining <= Math.min(30, total * 0.2);
  return (
    <div className={`countdown${low ? ' is-low' : ''}`} role="timer" aria-label={t.t('ng-ui-sc-app-remaining', { time: text })} style={{ width: size, height: size }}>
      <svg viewBox="0 0 72 72" width={size} height={size} aria-hidden="true" focusable="false">
        <circle cx="36" cy="36" r={r} className="countdown-track" />
        <circle cx="36" cy="36" r={r} className="countdown-arc" strokeDasharray={c} strokeDashoffset={c * (1 - frac)} transform="rotate(-90 36 36)" />
      </svg>
      <span className="countdown-text" aria-hidden="true">
        {text}
      </span>
    </div>
  );
}
