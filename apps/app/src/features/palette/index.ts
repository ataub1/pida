/**
 * Stabile Exporte des Pakets UI-B für Befehlspalette und Kürzelübersicht (ADR 0004 §6.1, in E0
 * festgelegt). Bis B4 Platzhalter ohne Darstellung; die Befehle liefert `app/commands.ts` (UI-A).
 */
import type { CommandEntry, ShortcutEntry } from '../../app/commands';

/** Props der Befehlspalette (in E0 festgelegt). */
export interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  commands: CommandEntry[];
}

/** Props der Kürzelübersicht (in E0 festgelegt). */
export interface ShortcutsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  commands: CommandEntry[];
  shortcuts: ShortcutEntry[];
}

/** Befehlspalette ⌘/Strg+K (Platzhalter bis B4). */
export function CommandPalette(_props: CommandPaletteProps): null {
  return null;
}

/** Kürzelübersicht „?“ (Platzhalter bis B4). */
export function ShortcutsDialog(_props: ShortcutsDialogProps): null {
  return null;
}

/** Palette und Kürzelübersicht vorhanden (B4 schaltet um); bis dahin zeigt die Kopfzeile keinen Befehlsknopf. */
export const PALETTE_AVAILABLE = false;
