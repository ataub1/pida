/**
 * Bildschirmtastatur auf Mobile (iOS-WebView): Beim Einblenden schrumpft nur der sichtbare Bereich
 * (`visualViewport`), das Fenster bleibt gleich hoch. Dialoge rücken per CSS über die Tastatur
 * (styles/components.css, `--visual-viewport-height` von React Aria) und rollen dann in sich – das
 * fokussierte Feld läge danach womöglich außerhalb des sichtbaren Teils. Dieses Modul holt es nach jeder
 * Größenänderung des sichtbaren Bereichs (kurz verzögert, damit React das neue Layout schon gesetzt hat)
 * wieder in den Blick. Nur Felder in einem Overlay: Seiteninhalte rollt WebKit selbst an das Feld heran.
 *
 * Außerdem steht, solange die Tastatur eingeblendet ist, `data-keyboard` am Wurzelelement: Mitlaufende
 * Teile (Vorschau des Signaturbilds in den Einstellungen, Knöpfe der Signierkarte) rollen dann mit, statt im
 * kleinen sichtbaren Rest das Feld zu verdecken (iPhone 17e, 17.10.2026: „Hinweistext“ lag unter der
 * Vorschau). Media Queries sehen das nicht – das Fenster bleibt ja gleich hoch.
 *
 * Und `--vv-top` am Wurzelelement: Beim Fokussieren eines Felds rollt iOS den sichtbaren Bereich im Fenster nach
 * unten (`visualViewport.offsetTop`); fest positionierte Overlays bleiben am Fenster und lagen dann um genau
 * diesen Betrag zu hoch – die Anmeldemaske von ID Austria verschwand oben aus dem Bild, sobald man ins
 * Passwortfeld tippte (iPhone, 29.10.2026). components.css schiebt die Overlays um `--vv-top` nach.
 */

/** Verzögerung nach dem letzten `resize` des sichtbaren Bereichs (Tastatur-Animation, React-Commit). */
export const REVEAL_DELAY_MS = 120;

/**
 * Ab so viel weniger sichtbarer Höhe als das Fenster gilt die Tastatur als eingeblendet (Punkte; die
 * Zubehörleiste allein mit Hardware-Tastatur ist rund 50 hoch, die Software-Tastatur über 250).
 */
export const KEYBOARD_MIN_PX = 120;

const FIELD = 'input:not([type=checkbox]):not([type=radio]):not([type=hidden]), textarea, [contenteditable="true"]';

/** Tastatur eingeblendet? Gezoomt (Pinch) ist der sichtbare Bereich kleiner, aber nicht verdeckt. */
export function keyboardShown(win: Window): boolean {
  const vv = win.visualViewport;
  if (!vv) return false;
  return win.innerHeight - vv.height * (vv.scale || 1) >= KEYBOARD_MIN_PX;
}

export function keepFocusedFieldVisible(win: Window = window): () => void {
  const vv = win.visualViewport;
  if (!vv) return () => {};
  const root = win.document.documentElement;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const reveal = () => {
    const el = win.document.activeElement as HTMLElement | null;
    if (el?.matches(FIELD) && el.closest('.overlay')) {
      el.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
    }
  };
  const follow = () => {
    const top = Math.max(0, Math.round(vv.offsetTop || 0));
    if (top > 0) root.style.setProperty('--vv-top', `${top}px`);
    else root.style.removeProperty('--vv-top');
  };
  const onResize = () => {
    // sofort (vor dem Rollen), damit das Layout ohne mitlaufende Teile schon steht
    root.toggleAttribute('data-keyboard', keyboardShown(win));
    follow();
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(reveal, REVEAL_DELAY_MS);
  };
  vv.addEventListener('resize', onResize);
  vv.addEventListener('scroll', follow);
  return () => {
    vv.removeEventListener('resize', onResize);
    vv.removeEventListener('scroll', follow);
    root.style.removeProperty('--vv-top');
    if (timer !== undefined) clearTimeout(timer);
    root.removeAttribute('data-keyboard');
  };
}
