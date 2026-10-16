// @vitest-environment node
/**
 * Stilregeln aus der Emulator-Abnahme (Android, 16.10.2026), die jsdom nicht auswertet (`css: false`):
 * - Prüfmodus schmal: das Ergebnis unter den Seiten scrollt – die Regel in `verify.css` muss die spätere
 *   `.view-verify .app-main { overflow: hidden }` aus `shell.css` überstimmen (höhere Spezifität);
 * - Fingerziele ≥ 44 px auch für „Befehle“/Wortmarke in der schmalen Kopfzeile, „Hinweis schließen“,
 *   „Suche löschen“ und den kompakten Konto-Chip im Bottom-Sheet.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const src = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file: string) => readFileSync(path.join(src, file), 'utf8');

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
    const shell = ruleInMedia(read('app/shell.css'), '(max-width: 760px) and (pointer: coarse)', '.command-button');
    expect(shell).toMatch(/width: 44px/);
    expect(shell).toMatch(/height: 44px/);
    const components = read('styles/components.css');
    expect(ruleInMedia(components, '(pointer: coarse)', '.toast-close')).toMatch(/height: 44px/);
    expect(ruleInMedia(components, '(pointer: coarse)', '.search-clear')).toMatch(/height: 44px/);
    expect(ruleInMedia(components, '(pointer: coarse)', '.signer-chip.is-compact')).toMatch(/min-height: 44px/);
  });
});
