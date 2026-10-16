/**
 * Stabile Exporte des Pakets UI-B für Befehlspalette und Kürzelübersicht (ADR 0004 §6.1, in E0 festgelegt).
 * Die Befehle samt Freigaben liefert `app/commands.ts` (UI-A); Palette und Übersicht zeigen genau diese Liste.
 */
export { CommandPalette, type CommandPaletteProps } from './CommandPalette';
export { ShortcutsDialog, type ShortcutsDialogProps } from './ShortcutsDialog';

/** Palette und Kürzelübersicht vorhanden (seit B4): die Kopfzeile zeigt „Befehle ⌘K“, ⌘/Strg+K und „?“ wirken. */
export const PALETTE_AVAILABLE = true;
