// @vitest-environment node
/**
 * Stilregeln aus der Emulator-Abnahme (Android, 16.10.2026), die jsdom nicht auswertet (`css: false`):
 * - Arbeitsbereich: der Hauptbereich scrollt nicht selbst (`.view-workspace .app-main { overflow: hidden }`), die
 *   übrigen Ansichten schon; einen Prüfmodus mit eigener Regel gibt es nicht mehr (ADR 0004 Nachtrag 15);
 * - Fingerziele ≥ 44 px auch für „Befehle“/Wortmarke in der schmalen Kopfzeile, „Hinweis schließen“,
 *   „Suche löschen“ und den kompakten Konto-Chip im Bottom-Sheet;
 * - Segmente („Passwort merken?“, Speicherart) brechen um statt angeschnitten zu scrollen;
 * - Stapel schmal: der Papierkorb bleibt sichtbar (nur der Pfeil entfällt);
 * - Schlüssel des Keystores: Name und Alias untereinander.
 * Dazu aus dem Gerätenachweis am iPhone 17e (17.10.2026): Knöpfe der Signierkarte bleiben am unteren Rand stehen,
 * Stapelkarte und Zoomwert bei großer Schrift, mitlaufende Teile rollen bei eingeblendeter Bildschirmtastatur mit.
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
  it('nur der Arbeitsbereich sperrt den Bildlauf des Hauptbereichs; kein Prüfmodus mehr', () => {
    const shell = read('app/shell.css');
    expect(topRule(shell, '.view-workspace .app-main')).toMatch(/overflow: hidden/);
    expect(topRule(shell, '.app-main')).toMatch(/overflow: auto/);
    expect(shell).not.toMatch(/view-verify/);
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

  it('Signierkarte auf Mobile: Knöpfe bleiben am unteren Rand stehen – nicht bei Kurzansichten und nicht mit Tastatur', () => {
    const css = read('features/signing/signing.css');
    const sel = ':root[data-mobile] .signing-card .step-pane:not(.step-pane-center) > .step-actions';
    const sticky = topRule(css, sel);
    expect(sticky).toMatch(/position: sticky;/);
    expect(sticky).toMatch(/bottom: 0;/);
    expect(sticky).toMatch(/background: var\(--c-surface\);/);
    // gleiche Einrückung wie der Inhalt der Karte (schmal: Bundsteg)
    expect(sticky).toMatch(/margin-inline: calc\(-1 \* var\(--sp-5\)\);/);
    expect(ruleInMedia(css, '(max-width: 420px)', sel)).toMatch(/margin-inline: calc\(-1 \* var\(--gutter\)\);/);
    expect(topRule(css, sel.replace(':root[data-mobile]', ':root[data-mobile][data-keyboard]'))).toMatch(/position: static;/);
    // nach der Grundregel `.step-actions` (gleiche Eigenschaften, spätere Regel)
    expect(css.indexOf(sel)).toBeGreaterThan(css.indexOf('\n.step-actions {'));
  });

  it('große Schrift: Stapelkarte nicht breiter als die Karte, Zoomwert ohne Umbruch', () => {
    expect(read('features/signing/signing.css')).toMatch(/\.sc-batch-foot \.details \{[^}]*min-width: min\(12rem, 100%\);/);
    expect(read('features/signing/signing.css')).not.toMatch(/min-width: 12rem;/);
    expect(topRule(read('features/workspace/workspace.css'), '.zoom-value')).toMatch(/white-space: nowrap;/);
    // umbrochener Auslöser „Alle Dokumente des Stapels“ bzw. „Technische Details“ linksbündig
    expect(topRule(read('styles/components.css'), '.details-trigger')).toMatch(/text-align: start;/);
  });

  it('Kopfzeile hochkant mit „Zurück zum Stapel“: nur das Logo, damit „+ PDF“ nicht über „Prüfen“ liegt', () => {
    expect(ruleInMedia(read('app/shell.css'), '(max-width: 480px)', '.app-header:has(.header-back) .wordmark-text')).toMatch(/display: none;/);
    expect(read('app/Header.tsx')).toMatch(/className="header-back"/);
  });

  it('Bildschirmtastatur (iOS): mitlaufende Vorschau des Signaturbilds rollt mit', () => {
    const css = read('features/settings/settings.css');
    expect(topRule(css, ":root[data-keyboard] .settings-row[data-row='signature.preview']")).toMatch(/position: static;/);
    expect(css.indexOf(':root[data-keyboard]')).toBeGreaterThan(css.indexOf("\n.settings-row[data-row='signature.preview'] {"));
  });
});
