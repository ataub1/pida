import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { countdownStart, formatTime, useCountdown } from './CountdownRing';

function Remaining({ total, request }: { total: number; request?: string }) {
  const remaining = useCountdown(total, 250, () => undefined, request);
  return <span data-testid="left">{formatTime(remaining)}</span>;
}

describe('Countdown der App-Bestätigung', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-10-28T10:00:00Z'));
    localStorage.clear();
  });
  afterEach(() => vi.useRealTimers());

  it('läuft nach neuem Aufbau der Ansicht für dieselbe Anfrage weiter (Oberfläche neu geladen)', () => {
    const first = render(<Remaining total={300} request="request-7" />);
    expect(screen.getByTestId('left').textContent).toBe('5:00');
    first.unmount();
    vi.setSystemTime(new Date('2026-10-28T10:02:00Z'));
    render(<Remaining total={300} request="request-7" />);
    expect(screen.getByTestId('left').textContent).toBe('3:00');
  });

  it('übernimmt den Startzeitpunkt aus dem localStorage (Oberfläche neu geladen, Android-Activity neu angelegt)', () => {
    const start = countdownStart('request-9');
    // die Seite wurde neu geladen: nur noch der localStorage kennt den Start
    expect(JSON.parse(localStorage.getItem('pida.countdown') ?? '{}')).toEqual({ key: 'request-9', start });
    vi.setSystemTime(new Date('2026-10-28T10:04:30Z'));
    render(<Remaining total={300} request="request-9" />);
    expect(screen.getByTestId('left').textContent).toBe('0:30');
  });

  it('beginnt für eine neue Anfrage wieder bei der Gesamtzeit', () => {
    countdownStart('request-1');
    vi.setSystemTime(new Date('2026-10-28T10:03:00Z'));
    render(<Remaining total={300} request="request-2" />);
    expect(screen.getByTestId('left').textContent).toBe('5:00');
    act(() => vi.advanceTimersByTime(60_000));
    expect(screen.getByTestId('left').textContent).toBe('4:00');
  });
});
