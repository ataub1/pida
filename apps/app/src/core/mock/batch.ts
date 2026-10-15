/**
 * Mock-Kern, Bereich Stapel (UI-A, ADR 0004 §3.8). In E1 ohne Wirkung: Der Kern liefert die
 * Stapelübersicht erst mit K2 (`batch_overview`); mehrere Dokumente laufen bis dahin nacheinander über
 * „Fertig“ → „Nächstes Dokument“. A2 füllt diesen Bereich (Übersicht, Lauf mit einer Sitzung, Pausen).
 * Mit `examples` liefert er ein festes Beispiel für `BatchView` (Abnahme E0 „je neuem Typ ein Beispiel“);
 * die Oberfläche zeigt es bis A2 nicht an (`actions.show_batch` bleibt aus).
 */
import type { BatchView } from '../types';
import type { MockArea } from './shared';

export interface BatchState {
  example: BatchView | null;
}

/** Beispiel wie im Prototyp (Szenario „Stapel“): Angebot, Auftragsbestätigung, gesperrter Scan, Vertrag. */
export function exampleBatchView(): BatchView {
  return {
    items: [
      { doc: 1, name: 'Angebot_2026-118.pdf', page_count: 2, size_bytes: 186_000, status: { state: 'positioned' }, position: { kind: 'auto' }, target_name: 'Angebot_2026-118_signed.pdf', target_exists: false, thumbnail: true },
      { doc: 2, name: 'Auftragsbestaetigung_4711.pdf', page_count: 1, size_bytes: 94_000, status: { state: 'ready' }, position: { kind: 'qr' }, target_name: 'Auftragsbestaetigung_4711_signed.pdf', target_exists: false, thumbnail: true },
      { doc: 3, name: 'Lieferschein_Scan.pdf', page_count: 1, size_bytes: 412_000, status: { state: 'rejected', code: 'error-PDFPwdProtected' }, position: { kind: 'auto' }, target_name: null, target_exists: false, thumbnail: false },
      { doc: 4, name: 'Vertrag.pdf', page_count: 3, size_bytes: 214_000, status: { state: 'ready' }, position: { kind: 'auto' }, target_name: 'Vertrag_signed.pdf', target_exists: true, thumbnail: true },
    ],
    open_item: null,
    running: false,
    summary: null,
    output: { mode: 'next_to_source', folder_display: null, needs_folder: false },
    estimate: { backend: 'atrust_native', documents: 3, logins: 1, confirmations: 3, certificate_confirmation_possible: true, large: false },
  };
}

export const batchArea: MockArea<BatchState> = {
  init: (o) => ({ example: o.examples ? exampleBatchView() : null }),
  handle: () => false,
  project: (state) => ({ batch: state.example }),
};
