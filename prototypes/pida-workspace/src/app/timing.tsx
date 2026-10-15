/**
 * Zeitverhalten der Simulation. Im Prototyp realistisch verzögert, in Tests 0 bzw. aus (`null`),
 * damit Durchläufe deterministisch und schnell sind.
 */
import { createContext, useContext } from 'react';

export interface Timing {
  /** Verbindungsaufbau nach „Anmelden“ */
  connectMs: number;
  /** automatische App-Bestätigung (simulierter Long-Poll); `null` = nur per Demo-Knopf */
  autoConfirmMs: number | null;
  /** Gültigkeit der App-Bestätigung (Countdown-Ring) */
  appTimeoutSecs: number;
  /** verkürzter Countdown, wenn die Demo-Antwort „Zeitablauf“ gewählt ist */
  timeoutScenarioSecs: number;
  /** Ablehnung in der App (Demo-Antwort „Abbruch“) */
  declineMs: number;
  /** Signatur erstellen */
  signMs: number;
  /** Haltezeit des Schritts „Signiert“, bevor „Fertig“ erscheint */
  doneHoldMs: number;
  /** je Dokument im Stapel */
  batchItemMs: number;
  /** FIDO2: Berühren des Schlüssels automatisch nach … ; `null` = nur per Demo-Knopf */
  fido2AutoMs: number | null;
  /** Takt des Countdowns */
  tickMs: number;
}

export const DEMO_TIMING: Timing = {
  connectMs: 900,
  autoConfirmMs: 9000,
  appTimeoutSecs: 300,
  timeoutScenarioSecs: 12,
  declineMs: 3500,
  signMs: 1300,
  doneHoldMs: 900,
  batchItemMs: 900,
  fido2AutoMs: 6000,
  tickMs: 250,
};

export const TEST_TIMING: Timing = {
  connectMs: 0,
  autoConfirmMs: null,
  appTimeoutSecs: 300,
  timeoutScenarioSecs: 0.05,
  declineMs: 0,
  signMs: 0,
  doneHoldMs: 0,
  batchItemMs: 0,
  fido2AutoMs: null,
  tickMs: 20,
};

export const TimingContext = createContext<Timing>(DEMO_TIMING);

export function useTiming(): Timing {
  return useContext(TimingContext);
}
