/**
 * Barrierefreiheit (ADR 0004 §4.7): axe-core ohne Verstöße. `color-contrast` ist in jsdom nicht messbar
 * (kein Layout/CSS) und wird stattdessen über die Tokens geprüft (`styles/tokens.test.ts`).
 */
import axe from 'axe-core';
import { expect } from 'vitest';

export async function expectNoAxeViolations(root: Element = document.body) {
  const results = await axe.run(root, {
    rules: { 'color-contrast': { enabled: false } },
    resultTypes: ['violations'],
  });
  const report = results.violations
    .map((v) => `${v.id} (${v.impact}): ${v.help}\n${v.nodes.map((n) => `   ${n.target.join(' ')} – ${n.failureSummary ?? ''}`).join('\n')}`)
    .join('\n');
  expect(results.violations, report).toEqual([]);
}
