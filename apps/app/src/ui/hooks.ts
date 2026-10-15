/** Kleine Hooks für Umgebungsabfragen (in jsdom ohne `matchMedia` gilt der Desktop-Fall). */
import { useEffect, useState } from 'react';
import type React from 'react';

export function useMediaQuery(query: string): boolean {
  const get = () => (typeof window !== 'undefined' && typeof window.matchMedia === 'function' ? window.matchMedia(query).matches : false);
  const [matches, setMatches] = useState(get);
  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return undefined;
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener?.('change', onChange);
    return () => mql.removeEventListener?.('change', onChange);
  }, [query]);
  return matches;
}

/** Schmale Ansicht: Seitenleiste wird zum Bottom-Sheet. Muss zu `--bp-narrow` in den Styles passen. */
export const NARROW_QUERY = '(max-width: 760px)';

export function useNarrow(): boolean {
  return useMediaQuery(NARROW_QUERY);
}

export function usePrefersReducedMotion(): boolean {
  return useMediaQuery('(prefers-reduced-motion: reduce)');
}

/** Maßstab, mit dem eine Seite der Breite `pageW` (pt) in das Element passt. */
export function useFitScale(ref: React.RefObject<HTMLElement | null>, pageW: number, opts: { min?: number; max?: number; pad?: number; initial?: number } = {}): number {
  const { min = 0.3, max = 1.4, pad = 48, initial = 1 } = opts;
  const [scale, setScale] = useState(initial);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth;
      if (w > 0) setScale(Math.min(max, Math.max(min, (w - pad) / pageW)));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref, pageW, min, max, pad]);
  return scale;
}
