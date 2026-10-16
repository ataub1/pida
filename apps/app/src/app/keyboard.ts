/**
 * Bildschirmtastatur auf Mobile (iOS-WebView): Beim Einblenden schrumpft nur der sichtbare Bereich
 * (`visualViewport`), das Fenster bleibt gleich hoch. Dialoge rücken per CSS über die Tastatur
 * (styles/components.css, `--visual-viewport-height` von React Aria) und rollen dann in sich – das
 * fokussierte Feld läge danach womöglich außerhalb des sichtbaren Teils. Dieses Modul holt es nach jeder
 * Größenänderung des sichtbaren Bereichs (kurz verzögert, damit React das neue Layout schon gesetzt hat)
 * wieder in den Blick. Nur Felder in einem Overlay: Seiteninhalte rollt WebKit selbst an das Feld heran.
 */

/** Verzögerung nach dem letzten `resize` des sichtbaren Bereichs (Tastatur-Animation, React-Commit). */
export const REVEAL_DELAY_MS = 120;

const FIELD = 'input:not([type=checkbox]):not([type=radio]):not([type=hidden]), textarea, [contenteditable="true"]';

export function keepFocusedFieldVisible(win: Window = window): () => void {
  const vv = win.visualViewport;
  if (!vv) return () => {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  const reveal = () => {
    const el = win.document.activeElement as HTMLElement | null;
    if (el?.matches(FIELD) && el.closest('.overlay')) {
      el.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
    }
  };
  const onResize = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(reveal, REVEAL_DELAY_MS);
  };
  vv.addEventListener('resize', onResize);
  return () => {
    vv.removeEventListener('resize', onResize);
    if (timer !== undefined) clearTimeout(timer);
  };
}
