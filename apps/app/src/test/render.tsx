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

/**
 * Schmale Ansicht (< 760 px) bzw. Fingerbedienung für `useNarrow`/`useMediaQuery` (jsdom kennt kein
 * `matchMedia`); liefert die Rücknahme.
 */
export function emulateMedia(queries: { narrow?: boolean; coarse?: boolean } = { narrow: true }): () => void {
  const original = window.matchMedia;
  window.matchMedia = ((query: string) => {
    const matches = (queries.narrow === true && query.includes('max-width: 760px')) || (queries.coarse === true && query.includes('pointer: coarse'));
    return {
      matches,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    } as unknown as MediaQueryList;
  }) as typeof window.matchMedia;
  return () => {
    window.matchMedia = original;
  };
}
