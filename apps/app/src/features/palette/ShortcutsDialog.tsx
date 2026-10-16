/**
 * Kürzelübersicht „?“ (ADR 0004 §2.10; Gestalt aus dem Prototyp `ShortcutsDialog`): alle Tastenkürzel der
 * Befehle aus `app/commands.ts` (`buildShortcuts`: Öffnen, Signieren, Einstellungen, diese Übersicht,
 * Schließen, Zugriffstasten) und des Arbeitsbereichs (Schrittweiten des Kerns 15/1/50 pt, Seite, Doppelklick,
 * Zoom), dazu die Befehlspalette selbst (⌘/Strg+K, kein Befehl der Liste).
 */
import type { CommandEntry, ShortcutEntry } from '../../app/commands';
import { useApp } from '../../app/context';
import { Keys, useModKey } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import './palette.css';

/** Props der Kürzelübersicht (in E0 festgelegt). */
export interface ShortcutsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  commands: CommandEntry[];
  shortcuts: ShortcutEntry[];
}

const GROUP_TITLE: Record<ShortcutEntry['group'], string> = {
  general: 'ng-ui-sh-group-general',
  workspace: 'ng-ui-sh-group-workspace',
};

/** Zeilen der Übersicht: die Palette zuerst, dann die Kürzel der Shell (`buildShortcuts`). */
export function shortcutRows(shortcuts: ShortcutEntry[], palette: { label: string; keys: string[] }): ShortcutEntry[] {
  return [{ id: 'palette', group: 'general', label: palette.label, keys: palette.keys }, ...shortcuts];
}

/** Textfassung eines Kürzels für Screenreader („Strg+K“, „Pfeiltasten“ bleiben als Zeichen). */
export function keysLabel(keys: string[]): string {
  return keys.join('+');
}

export function ShortcutsDialog({ isOpen, onClose, shortcuts }: ShortcutsDialogProps) {
  const { t } = useApp();
  const { mod } = useModKey();
  const rows = shortcutRows(shortcuts, { label: t.t('ng-ui-sh-palette'), keys: [mod, 'K'] });
  const groups = (['general', 'workspace'] as const).map((g) => ({ id: g, rows: rows.filter((r) => r.group === g) })).filter((g) => g.rows.length > 0);
  return (
    <ModalShell isOpen={isOpen} onClose={onClose} size="md" className="shortcuts-modal">
      <DialogHeader title={t.t('ng-ui-sh-title')} onClose={onClose} />
      <div className="shortcuts-body">
        {groups.map((g) => (
          <section key={g.id} className="shortcuts-group" aria-labelledby={`sh-${g.id}`}>
            <h3 className="shortcuts-title" id={`sh-${g.id}`}>
              {t.t(GROUP_TITLE[g.id])}
            </h3>
            <dl className="shortcuts-list">
              {g.rows.map((r) => (
                <div key={r.id} className="shortcuts-row" data-shortcut={r.id}>
                  <dt>{r.label}</dt>
                  <dd>
                    <Keys keys={r.keys} label={keysLabel(r.keys)} />
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    </ModalShell>
  );
}
