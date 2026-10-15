/**
 * **Vorgeschlagene additive Erweiterungen** der Bridge-Typen (noch nicht in
 * crates/core-api/src/view.rs). Die Oberfläche nutzt sie, wenn der Kern sie liefert, und bleibt
 * ohne sie bedienbar. Grund: `ViewState` bildet offene Rückfragen der Ausgabe und Hinweise ohne
 * Rückfrage (`workflow::Effect::{AskSavePath, AskOverwrite, Notify}`) sowie die unsichtbare Signatur
 * je Dokument (`PositionChoice::Invisible`) noch nicht ab. Im Abschlussbericht (Paket I3) als
 * Schnittstellenwunsch an `core-api` gemeldet; alle Felder sind optional und mit
 * `#[serde(default, skip_serializing_if = "Option::is_none")]` abwärtskompatibel.
 *
 * Vorschlag Rust (view.rs):
 * ```rust
 * #[serde(tag = "kind", rename_all = "snake_case")]
 * pub enum OutputPromptView {
 *     SavePath { directory: PathBuf, file_name: String },   // F-83 (Effect::AskSavePath)
 *     Overwrite { path: PathBuf },                          // F-84 (Effect::AskOverwrite)
 * }
 * pub struct OutputView { …, #[serde(default)] pub prompt: Option<OutputPromptView> }
 * pub struct NotificationView { pub id: u64, pub code: String, pub params: BTreeMap<String, String> }
 * pub struct ViewState { …, #[serde(default)] pub notification: Option<NotificationView> }  // Effect::Notify
 * pub enum UiEvent { …, ChooseInvisible }  // „Unsichtbar“ im Positionierungsschritt
 * ```
 */
import type { OutputView, StepView, UiEvent, ViewState } from './types';

/** Rückfrage der Ausgabe (Vorschlag). */
export type OutputPromptView =
  | { kind: 'save_path'; directory: string; file_name: string }
  | { kind: 'overwrite'; path: string };

/** `OutputView` mit Rückfrage (Vorschlag). */
export interface OutputViewExt extends OutputView {
  prompt?: OutputPromptView | null;
}

/** Hinweis ohne Rückfrage (`Effect::Notify`, Vorschlag); `id` wechselt je Hinweis. */
export interface NotificationView {
  id: number;
  code: string;
  params: Record<string, string>;
}

/** `ViewState` mit Hinweis (Vorschlag). */
export interface ViewStateExt extends ViewState {
  notification?: NotificationView | null;
}

/** Zusätzliche Ereignisse (Vorschlag). */
export type UiEventExt = UiEvent | { type: 'choose_invisible' };

/** Rückfrage der Ausgabe, falls der Kern sie liefert. */
export function outputPrompt(step: StepView): OutputPromptView | null {
  if (step.step !== 'output') return null;
  return (step.result as OutputViewExt).prompt ?? null;
}

/** Hinweis, falls der Kern ihn liefert. */
export function notification(view: ViewState): NotificationView | null {
  return (view as ViewStateExt).notification ?? null;
}
