/**
 * Stapel (Paket UI-A, ADR 0004 §3.8): kommt mit A2 (Stapelübersicht, Lauf, Pausen, Zusammenfassung).
 * Bis dahin laufen mehrere Dokumente nacheinander über „Fertig“ → „Nächstes Dokument“; der Kern liefert
 * `StepView::Batch` erst mit K2 (`batch_overview`).
 */
export const BATCH_AVAILABLE = false;
