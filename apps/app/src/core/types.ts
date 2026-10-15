/**
 * Bridge-Typen der Oberfläche. **Einzige Quelle** ist `crates/core-api/src/view.rs`: Die Typen werden
 * mit ts-rs nach `generated.ts` erzeugt (`PIDA_BLESS=1 cargo test -p pida-core-api --test
 * ts_bindings`) und hier nur weitergereicht; der Rust-Test bricht ab, sobald `generated.ts` nicht
 * mehr zu den Rust-Typen passt. Hier stehen nur abgeleitete Hilfstypen und -funktionen.
 *
 * Abbildungsregeln (serde_json): `Option<T>` → `T | null` (beim Senden immer angeben), `PathBuf` →
 * `string`, Zahlen → `number`, Enums ohne serde-Attribut extern getaggt (Unit-Varianten als String,
 * Daten-Varianten als `{ "Variante": … }`), `#[serde(tag = …)]` intern getaggt.
 *
 * Die Konsistenz der serde-Ausgabe mit diesen Typen prüfen zusätzlich `src/core/contract.test.ts`
 * (Beispiel-JSON aus `apps/app/src-tauri/tests/serde_contract.rs`) und der Rust-Test selbst
 * (TS-Beispiele → `UiEvent`).
 */
export type * from './generated';

import type { NativeForm, StepView, UiEvent, ViewState } from './generated';

/** Name eines Schritts (`StepView['step']`). */
export type StepName = StepView['step'];

/** Name eines Ereignisses. */
export type UiEventType = UiEvent['type'];

/** Daten einer Maske (`NativeForm`) mit dem Variantennamen `K`. */
export type FormData<K extends string> = Extract<NativeForm, Record<K, unknown>> extends Record<K, infer D> ? D : never;

/** Mobile Oberfläche (iOS/Android: Teilen statt Ordner, PKCS#12-Import, kein FIDO2, keine lokale BKU). */
export function isMobile(view: Pick<ViewState, 'platform'>): boolean {
  return view.platform?.mobile === true;
}

/** Variante einer extern getaggten Enum mit Daten (`{ "Name": … }`) bzw. der Unit-String. */
export function variantName(value: string | Record<string, unknown>): string {
  if (typeof value === 'string') return value;
  const keys = Object.keys(value);
  return keys[0] ?? '';
}
