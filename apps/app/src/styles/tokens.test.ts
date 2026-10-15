// @vitest-environment node
/**
 * Design-Tokens (1:1 aus dem Prototyp, `src/test/catalog.test.ts` Teil „Tokens“, ADR 0004 §4.2): Textfarben
 * erfüllen WCAG 2.2 AA (4,5:1 für Text, 3:1 für Fokus/Bedienelemente) in Hell und Dunkel; System-Dunkelmodus
 * und Umschalter „Dunkel“ nutzen dieselben Farben; die Fokusfarbe ist klar von Fehlerrot verschieden.
 * `color-contrast` von axe ist in jsdom nicht messbar und wird hierdurch ersetzt.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const css = readFileSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), 'tokens.css'), 'utf8');

function parseBlock(source: string, selector: string): Record<string, string> {
  const start = source.indexOf(selector);
  if (start < 0) throw new Error(`Selektor fehlt: ${selector}`);
  const open = source.indexOf('{', start);
  const close = source.indexOf('}', open);
  const body = source.slice(open + 1, close);
  const out: Record<string, string> = {};
  for (const m of body.matchAll(/(--[\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) out[m[1] as string] = m[2] as string;
  return out;
}

function luminance(hex: string): number {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * (c[0] ?? 0) + 0.7152 * (c[1] ?? 0) + 0.0722 * (c[2] ?? 0);
}

function contrast(a: string, b: string): number {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return ((l1 ?? 0) + 0.05) / ((l2 ?? 0) + 0.05);
}

function hue(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as [number, number, number];
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  if (max === min) return 0;
  const d = max - min;
  const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return (h * 60 + 360) % 360;
}

const TEXT_PAIRS: [string, string][] = [
  ['--c-text', '--c-bg'],
  ['--c-text', '--c-surface'],
  ['--c-text-2', '--c-surface'],
  ['--c-text-3', '--c-surface'],
  ['--c-text-3', '--c-bg'],
  ['--c-text-3', '--c-surface-2'],
  ['--c-text-3', '--c-sunken'],
  ['--c-text-2', '--c-hover'],
  ['--c-on-primary', '--c-primary'],
  ['--c-link', '--c-surface'],
  ['--c-accent', '--c-surface'],
  ['--c-accent-on-soft', '--c-accent-soft'],
  ['--c-success', '--c-surface'],
  ['--c-success-on-soft', '--c-success-soft'],
  ['--c-warning-on-soft', '--c-warning-soft'],
  ['--c-danger', '--c-surface'],
  ['--c-danger-on-soft', '--c-danger-soft'],
  ['--c-info-on-soft', '--c-info-soft'],
];
const UI_PAIRS: [string, string][] = [
  ['--c-focus', '--c-surface'],
  ['--c-focus', '--c-bg'],
  ['--c-control-border', '--c-surface'],
  ['--c-control-border', '--c-surface-2'],
];

describe('Design-Tokens: Dunkelmodus und Fokus', () => {
  const light = parseBlock(css, ':root {');
  const darkMedia = parseBlock(css, ":root:not([data-theme='light'])");
  const darkForced = parseBlock(css, ":root[data-theme='dark']");

  it('System-Dunkelmodus und Umschalter „Dunkel“ nutzen dieselben Farben', () => {
    expect(darkMedia).toEqual(darkForced);
  });

  it('Fokusfarbe ist im Farbton klar von Fehlerrot verschieden', () => {
    for (const t of [light, { ...light, ...darkForced }]) {
      const diff = Math.abs(hue(t['--c-focus'] as string) - hue(t['--c-danger'] as string));
      expect(Math.min(diff, 360 - diff)).toBeGreaterThanOrEqual(90);
    }
  });
});

describe('Design-Tokens: Kontraste (WCAG 2.2 AA)', () => {
  const light = parseBlock(css, ':root {');
  const dark = { ...light, ...parseBlock(css, ":root[data-theme='dark']") };
  for (const [mode, t] of [
    ['hell', light],
    ['dunkel', dark],
  ] as const) {
    it(`Text ≥ 4,5:1 (${mode})`, () => {
      for (const [fg, bg] of TEXT_PAIRS) {
        const ratio = contrast(t[fg] as string, t[bg] as string);
        expect(ratio, `${fg} auf ${bg} = ${ratio.toFixed(2)}`).toBeGreaterThanOrEqual(4.5);
      }
    });
    it(`Fokus und Ränder von Bedienelementen ≥ 3:1 (${mode})`, () => {
      for (const [fg, bg] of UI_PAIRS) {
        const ratio = contrast(t[fg] as string, t[bg] as string);
        expect(ratio, `${fg} auf ${bg} = ${ratio.toFixed(2)}`).toBeGreaterThanOrEqual(3);
      }
    });
  }
});
