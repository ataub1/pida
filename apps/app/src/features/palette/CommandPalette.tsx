/**
 * Befehlspalette ⌘/Strg+K (ADR 0004 §2.10; Gestalt aus dem Prototyp `features/palette/CommandPalette.tsx`):
 * Suchfeld mit Filter (Beschriftung und Synonyme `.keywords`, ohne Groß-/Kleinschreibung und Akzente), Befehle
 * nach Gruppen. Die Befehle und ihre Freigaben kommen aus `app/commands.ts` (`ViewState::actions`); gesperrte
 * Befehle erscheinen nicht. Tastatur: Pfeiltasten wählen, Eingabe führt aus, Esc schließt (React Aria
 * `Autocomplete` mit virtuellem Fokus). Ein Befehl läuft erst, nachdem die Palette geschlossen ist (Fokus
 * zurück an den Auslöser). Bei Fingerbedienung keine Tastenkürzel-Hinweise (ADR 0004 §4.6).
 */
import { Autocomplete, Header, Input, Menu, MenuItem, MenuSection, SearchField, Text, useFilter } from 'react-aria-components';
import type { CommandEntry, CommandGroup } from '../../app/commands';
import { useApp } from '../../app/context';
import { isMobile } from '../../core/types';
import { Keys } from '../../ui/controls';
import { ModalShell } from '../../ui/Dialog';
import { IconSearch } from '../../ui/Icons';
import './palette.css';

/** Props der Befehlspalette (in E0 festgelegt). */
export interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  commands: CommandEntry[];
}

/** Gruppen in der Reihenfolge der Palette. */
export const GROUPS: readonly { id: CommandGroup; label: string }[] = [
  { id: 'actions', label: 'ng-ui-pal-group-actions' },
  { id: 'view', label: 'ng-ui-pal-group-view' },
  { id: 'help', label: 'ng-ui-pal-group-help' },
];

export function CommandPalette({ isOpen, onClose, commands }: CommandPaletteProps) {
  const { t, view } = useApp();
  const { contains } = useFilter({ sensitivity: 'base' });
  const showKeys = !isMobile(view);
  const enabled = commands.filter((c) => !c.isDisabled);
  return (
    <ModalShell isOpen={isOpen} onClose={onClose} variant="palette" ariaLabel={t.t('ng-ui-pal-title')} className="palette-modal">
      <Autocomplete filter={contains}>
        <SearchField aria-label={t.t('ng-ui-pal-search')} autoFocus className="palette-search">
          <IconSearch size={18} className="search-icon" />
          <Input className="palette-input" placeholder={t.t('ng-ui-pal-search')} />
        </SearchField>
        <Menu
          className="palette-menu"
          aria-label={t.t('ng-ui-pal-title')}
          renderEmptyState={() => <p className="palette-empty">{t.t('ng-ui-pal-empty')}</p>}
          onAction={(key) => {
            const cmd = enabled.find((c) => c.id === key);
            onClose();
            if (cmd) setTimeout(cmd.run, 0);
          }}
        >
          {GROUPS.map((g) => {
            const items = enabled.filter((c) => c.group === g.id);
            if (items.length === 0) return null;
            return (
              <MenuSection key={g.id} id={g.id} className="palette-section">
                <Header className="palette-header">{t.t(g.label)}</Header>
                {items.map((c) => {
                  const Icon = c.icon;
                  return (
                    <MenuItem key={c.id} id={c.id} className="palette-item" textValue={`${c.label} ${c.keywords}`}>
                      <span className="palette-icon" aria-hidden="true">
                        <Icon size={16} />
                      </span>
                      <Text slot="label" className="palette-label">
                        {c.label}
                      </Text>
                      {showKeys && c.keys.length > 0 ? <Keys keys={c.keys} /> : null}
                    </MenuItem>
                  );
                })}
              </MenuSection>
            );
          })}
        </Menu>
      </Autocomplete>
    </ModalShell>
  );
}
