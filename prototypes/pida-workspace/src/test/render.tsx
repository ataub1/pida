/** Rendert die App mit Testzeiten (keine Verzögerung, keine automatische App-Bestätigung). */
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { expect } from 'vitest';
import { App } from '../app/App';
import { createState, type InitOptions } from '../app/store';
import { TEST_TIMING, type Timing } from '../app/timing';

export function renderApp(opts: InitOptions & { timing?: Partial<Timing> } = {}) {
  const { timing, ...init } = opts;
  const initial = createState({ onboarded: true, ...init });
  const user = userEvent.setup();
  const utils = render(<App initial={initial} timing={{ ...TEST_TIMING, ...timing }} isMac={false} />);
  return { user, ...utils };
}

/** Signierkarte (Dialog) holen. */
export async function signingDialog(name = /Signieren mit ID Austria/) {
  return screen.findByRole('dialog', { name });
}

/** ID-Austria-Anmeldung im offenen Signier-Dialog ausfüllen. */
export async function login(user: ReturnType<typeof userEvent.setup>, password = 'geheim123') {
  const dialog = await signingDialog();
  const pw = await within(dialog).findByLabelText('Signatur-Passwort');
  await user.type(pw, password);
  await user.click(within(dialog).getByRole('button', { name: 'Anmelden' }));
  return dialog;
}

/**
 * axe-core-Prüfung ohne Verstöße. `color-contrast` ist in jsdom nicht messbar (kein Layout/CSS) und
 * wird stattdessen über die Tokens geprüft (tokens.test.ts).
 */
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
