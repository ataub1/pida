/**
 * Stapel (Paket UI-A, ADR 0004 §3.8, Etappe E2): Stapelübersicht mit Voranalyse, Einzeldokument öffnen und
 * „Zurück zum Stapel“, Start mit Ordnerwahl und Umgang mit vorhandenen Dateien, Zusammenfassung. Die
 * Stapelkarte (Fortschritt k/n, Pausen, Überspringen/Abbrechen) ist Teil der Signierkarte
 * (`features/signing`). Der Kern liefert `StepView::Batch` mit `batch_overview` (K2).
 */
export const BATCH_AVAILABLE = true;
export { BatchView, needsStartDialog, signableDocs } from './BatchView';
export { StartBatchDialog } from './StartBatchDialog';
export { estimateText, isOpenable, isSignable, stageLabel, statusDetail, statusLabel, statusTone, summaryCounts } from './texts';
