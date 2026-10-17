// @vitest-environment node
/**
 * Stilregeln aus der Emulator-Abnahme (Android, 16.10.2026), die jsdom nicht auswertet (`css: false`):
 * - Prüfmodus schmal: das Ergebnis unter den Seiten scrollt – die Regel in `verify.css` muss die spätere
 *   `.view-verify .app-main { overflow: hidden }` aus `shell.css` überstimmen (höhere Spezifität);
 * - Fingerziele ≥ 44 px auch für „Befehle“/Wortmarke in der schmalen Kopfzeile, „Hinweis schließen“,
 *   „Suche löschen“ und den kompakten Konto-Chip im Bottom-Sheet;
 * - Segmente („Passwort merken?“, Speicherart) brechen um statt angeschnitten zu scrollen;
 * - Stapel schmal: der Papierkorb bleibt sichtbar (nur der Pfeil entfällt);
 * - Schlüssel des Keystores: Name und Alias untereinander;
 * - Fingerbedienung: Fokusrahmen von React-Aria-Elementen nur nach deren Modalität (Abnahme 17.10.2026).
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const src = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file: string) => readFileSync(path.join(src, file), 'utf8');

/** Rumpf der ersten Regel `selector { … }` außerhalb von `@media` (Regel am Zeilenanfang). */
function topRule(css: string, selector: string): string | null {
  const at = css.search(new RegExp(`^${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')} \\{`, 'm'));
  if (at < 0) return null;
  const start = css.indexOf('{', at) + 1;
  return css.slice(start, css.indexOf('}', start));
}

/** Rumpf der ersten Regel `selector { … }` innerhalb des ersten Blocks `@media <query> { … }`. */
function ruleInMedia(css: string, query: string, selector: string): string | null {
  let at = css.indexOf(`@media ${query}`);
  while (at >= 0) {
    const open = css.indexOf('{', at);
    let depth = 1;
    let i = open + 1;
    while (i < css.length && depth > 0) {
      if (css[i] === '{') depth += 1;
      else if (css[i] === '}') depth -= 1;
      i += 1;
    }
    const body = css.slice(open + 1, i - 1);
    const pos = body.indexOf(`${selector} {`);
    if (pos >= 0) {
      const start = body.indexOf('{', pos) + 1;
      return body.slice(start, body.indexOf('}', start));
    }
    at = css.indexOf(`@media ${query}`, i);
  }
  return null;
}

describe('Mobile-Stile', () => {
  it('Prüfmodus schmal: Hauptbereich scrollt (überstimmt shell.css)', () => {
    expect(read('app/shell.css')).toMatch(/\.view-verify \.app-main \{\s*overflow: hidden;/);
    expect(ruleInMedia(read('features/verify/verify.css'), '(max-width: 860px)', '.app.view-verify .app-main')).toMatch(/overflow: auto/);
  });

  it('Fingerziele ≥ 44 px', () => {
    // auf allen Finger-Geräten (auch Tablets): Mindestmaße überstimmen die 36 px der schmalen Kopfzeile
    const shell = ruleInMedia(read('app/shell.css'), '(pointer: coarse)', '.header-add');
    expect(shell).toMatch(/min-width: 44px/);
    expect(shell).toMatch(/min-height: 44px/);
    expect(read('app/shell.css')).toMatch(/\.brand,\s*\.command-button,\s*\.header-add \{\s*min-width: 44px;/);
    const components = read('styles/components.css');
    expect(ruleInMedia(components, '(pointer: coarse)', '.toast-close')).toMatch(/height: 44px/);
    expect(ruleInMedia(components, '(pointer: coarse)', '.search-clear')).toMatch(/height: 44px/);
    expect(ruleInMedia(components, '(pointer: coarse)', '.signer-chip.is-compact')).toMatch(/min-height: 44px/);
    // die Mindesthöhe der Segmente (components.css, nach base.css geladen) darf die 44 px nicht überstimmen
    expect(ruleInMedia(components, '(pointer: coarse)', '.segmented-sm .segment')).toMatch(/min-height: 44px/);
    expect(components.indexOf('\n.segmented-sm .segment {')).toBeLessThan(components.lastIndexOf('@media (pointer: coarse)'));
  });

  it('Segmente: Mindesthöhe statt fester Höhe, Beschriftung darf umbrechen', () => {
    const components = read('styles/components.css');
    const segment = topRule(components, '.segment');
    expect(segment).toMatch(/min-height: 30px/);
    expect(segment).not.toMatch(/white-space: nowrap/);
    expect(segment).not.toMatch(/(^|[^-])height: 30px/);
    expect(topRule(components, '.segmented-sm .segment')).toMatch(/min-height: 26px/);
    // Notbehelf bleibt: die Gruppe scrollt, falls selbst das längste Wort nicht passt
    expect(topRule(components, '.segmented')).toMatch(/overflow-x: auto/);
  });

  it('Stapel schmal: Papierkorb sichtbar, nur der Pfeil entfällt', () => {
    const css = read('features/batch/batch.css');
    expect(ruleInMedia(css, '(max-width: 520px)', '.batch-row-action.has-caret')).toMatch(/grid-column: 2;/);
    expect(css).toMatch(/\.batch-row-action\.has-caret \.batch-row-caret,\s*\.batch-row-action:empty \{\s*display: none;/);
    expect(css).not.toMatch(/\.batch-row-action\.has-caret,\s*\.batch-row-action:empty \{/);
  });

  it('Schlüssel des Keystores: Name und Alias untereinander', () => {
    expect(topRule(read('features/settings/settings.css'), '.keystore-key')).toMatch(/display: grid/);
  });

  it('Finger: Fokusrahmen der React-Aria-Bedienelemente nur mit `data-focus-visible` (nach Bildschirmtastatur und Zurück-Geste)', () => {
    const base = read('styles/base.css');
    // überall: natives `:focus-visible` und die Modalität von React Aria zeigen den Rahmen
    expect(base).toMatch(/^:focus-visible,\n\[data-focus-visible\] \{\n {2}outline: 2px solid var\(--c-focus\);/m);
    // bei Fingerbedienung entscheidet für React-Aria-Elemente (außer Textfeldern) nur `data-focus-visible`
    const rule = ruleInMedia(base, '(pointer: coarse)', '[data-rac]:not(input, textarea):focus-visible:not([data-focus-visible])');
    expect(rule).toMatch(/outline: none;/);
    // spätere Regel mit höherer Spezifität: gewinnt über `:focus-visible` (0,1,0), nicht über `[data-focus-visible]`
    expect(base.indexOf('[data-rac]:not(input, textarea):focus-visible')).toBeGreaterThan(base.indexOf(':focus-visible,\n[data-focus-visible] {'));
  });

  it('Telefon: Hinweise oben, solange ein Bottom-Sheet unten steht (Dialog, ausgeklappte Seitenleiste)', () => {
    const shell = read('app/shell.css');
    // Dialog als Bottom-Sheet (`overlay-center`): oben, über dem abgedunkelten Rest – auch im Arbeitsbereich
    const dialog = ruleInMedia(shell, '(max-width: 760px)', ':root:has(.overlay-center) .toasts,\n  :root:has(.overlay-center) .view-workspace .toasts');
    expect(dialog).toMatch(/top: max\(var\(--sp-2\), env\(safe-area-inset-top\)\);/);
    expect(dialog).toMatch(/bottom: auto;/);
    // ausgeklappte Seitenleiste: unter der Kopfzeile
    const sheet = ruleInMedia(shell, '(max-width: 760px)', ':root:has(.inspector[data-expanded]) .view-workspace .toasts');
    expect(sheet).toMatch(/top: calc\(var\(--header-h\) \+ env\(safe-area-inset-top\) \+ var\(--sp-2\)\);/);
    expect(sheet).toMatch(/bottom: auto;/);
    // nach den Regeln „unten“ derselben Breite (gleiche Eigenschaft, spätere Regel gewinnt ohnehin über `bottom: auto`)
    expect(shell.indexOf(':root:has(.overlay-center) .toasts')).toBeGreaterThan(shell.indexOf('.view-workspace .toasts {\n    right: var(--gutter);'));
  });
});
