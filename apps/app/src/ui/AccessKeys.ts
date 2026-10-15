/**
 * Zugriffstasten (`&`-Mnemonics aus den PDF-Over-Katalogen, `.accesskey`; ADR 0004 §8 F7): Alt+Taste
 * (Windows/Linux) bzw. ⌃⌥+Taste (macOS) löst das Bedienelement mit `data-accesskey` in der obersten Ebene
 * aus. Unterstrichen werden die Buchstaben nur, solange die Alt-Taste gedrückt ist (`data-accesskeys` am
 * Wurzelelement, [`trackAltKey`]).
 */

const INTERACTIVE =
  'button, a[href], input, select, textarea, [role="tab"], [role="radio"], [role="checkbox"], [role="switch"], [role="button"], [tabindex]:not([tabindex="-1"])';

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
    const target =
      el.closest<HTMLElement>(INTERACTIVE) ?? el.closest<HTMLElement>('.field, .radio-group, fieldset')?.querySelector<HTMLElement>(INTERACTIVE) ?? null;
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

/**
 * Unterstreichung der Zugriffstasten nur bei gedrückter Alt-Taste: setzt bzw. entfernt `data-accesskeys`
 * am Wurzelelement; liefert die Abmeldung.
 */
export function trackAltKey(target: Window = window, root: HTMLElement = document.documentElement): () => void {
  const set = (on: boolean) => root.toggleAttribute('data-accesskeys', on);
  const down = (e: KeyboardEvent) => {
    if (e.key === 'Alt' || e.altKey) set(true);
  };
  const up = (e: KeyboardEvent) => {
    if (e.key === 'Alt' || !e.altKey) set(false);
  };
  const reset = () => set(false);
  target.addEventListener('keydown', down);
  target.addEventListener('keyup', up);
  target.addEventListener('blur', reset);
  return () => {
    target.removeEventListener('keydown', down);
    target.removeEventListener('keyup', up);
    target.removeEventListener('blur', reset);
    reset();
  };
}
