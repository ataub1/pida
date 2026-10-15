/** Rendert die App mit dem Mock-Kern (ohne Verzögerung, ohne automatische App-Bestätigung). */
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from '../app/App';
import { MockCore, type MockOptions } from '../core/mock';
import type { ViewState } from '../core/types';

export function renderApp(options: MockOptions & { view?: ViewState; isMac?: boolean } = {}) {
  const { view, isMac, ...mockOptions } = options;
  const mock = new MockCore({ delayMs: 0, autoCompleteApp: false, ...mockOptions, initialView: view });
  const user = userEvent.setup();
  const utils = render(<App bridge={mock} isMac={isMac ?? false} />);
  return { mock, user, ...utils };
}

/** Letztes gesendetes Ereignis. */
export function lastEvent(mock: MockCore) {
  return mock.dispatched[mock.dispatched.length - 1];
}
