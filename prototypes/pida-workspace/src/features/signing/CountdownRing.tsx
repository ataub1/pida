/** Countdown-Ring für die App-Bestätigung. `role="timer"` (nicht laufend angesagt). */
import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { formatTime } from '../../model/format';

export function useCountdown(totalSecs: number, tickMs: number, onExpire: () => void) {
  const [remaining, setRemaining] = useState(totalSecs);
  const start = useRef(Date.now());
  const expired = useRef(false);
  const cb = useRef(onExpire);
  cb.current = onExpire;
  useEffect(() => {
    start.current = Date.now();
    expired.current = false;
    setRemaining(totalSecs);
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
  }, [totalSecs, tickMs]);
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
    <div className={`countdown${low ? ' is-low' : ''}`} role="timer" aria-label={t('sc.app.remaining', { time: text })} style={{ width: size, height: size }}>
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
