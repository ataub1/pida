/**
 * Vertragsbedarf aus B3 (ADR 0004 §3.10 „Richtlinie … der Schalter erscheint gesperrt mit Hinweis“), vorläufig
 * nur im Mock abgebildet: Der Vertrag (`SettingsView`) sagt der Oberfläche nicht, welche Einstellungen eine
 * Richtlinie festlegt (`Policy::recent_documents`, `update_check`, `proxy`, `profile`, `forced_backend`).
 *
 * Vorschlag für K: `SettingsView::locked: BTreeMap<String, String>` (`#[serde(default)]`) – Schlüssel =
 * TOML-Punktpfad wie in `problems` (`general.recent_documents`, `general.update_check`, `proxy.mode`,
 * `signature.profile`, `backend.default` …), Wert = i18n-ID des Grunds (z. B. `ng-recent-policy`). Bis dahin
 * liest die Oberfläche das Feld, falls vorhanden, und behandelt sonst alles als frei.
 */
import type { SettingsView } from '../../core/types';

/** `SettingsView` mit dem vorgeschlagenen Feld `locked` (Pfad → i18n-ID des Grunds). */
export type SettingsViewDraft = SettingsView & { locked?: Record<string, string> };

/** Grund (i18n-ID), falls eine Richtlinie den Pfad festlegt; sonst `null`. */
export function lockedReason(view: SettingsView, path: string): string | null {
  const locked = (view as SettingsViewDraft).locked;
  if (!locked) return null;
  const direct = locked[path];
  if (direct) return direct;
  // ganze Abschnitte (`proxy` sperrt `proxy.mode`, `proxy.host` …)
  const section = path.split('.')[0] ?? '';
  return locked[section] ?? null;
}
