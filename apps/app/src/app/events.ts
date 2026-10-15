/**
 * Ereignisse zwischen Shell und Einstellungen (ADR 0004 §4.2: aus `settings/SettingsDialog.tsx` verlegt,
 * damit `App.tsx` nichts aus `settings/**` importiert).
 */

/** Ereignis (`CustomEvent<string>` mit dem Pfad), mit dem die Shell eine auf die Bildmarke gezogene Bilddatei meldet. */
export const EMBLEM_DROP_EVENT = 'pida:emblem-drop';
