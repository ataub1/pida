/** Befehlspalette (⌘K / Strg+K) und Übersicht der Tastenkürzel (?). */
import type { ReactNode } from 'react';
import { Autocomplete, Header, Input, Menu, MenuItem, MenuSection, SearchField, useFilter } from 'react-aria-components';
import { useAppState, useDispatch } from '../../app/store';
import { useT, type MsgKey } from '../../i18n';
import { Keys, useModKey } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconSearch } from '../../ui/Icons';

export interface Command {
  id: string;
  label: string;
  group: 'actions' | 'view' | 'help';
  icon: ReactNode;
  keys?: string[];
  keywords?: string;
  isDisabled?: boolean;
  run: () => void;
}

const GROUPS: { id: Command['group']; label: MsgKey }[] = [
  { id: 'actions', label: 'pal.group.actions' },
  { id: 'view', label: 'pal.group.view' },
  { id: 'help', label: 'pal.group.help' },
];

export function CommandPalette({ commands }: { commands: Command[] }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const { contains } = useFilter({ sensitivity: 'base' });
  const isOpen = state.overlay === 'palette';
  const close = () => dispatch({ type: 'overlay', overlay: null });
  const enabled = commands.filter((c) => !c.isDisabled);
  return (
    <ModalShell isOpen={isOpen} onClose={close} variant="palette" ariaLabel={t('pal.title')} className="palette-modal">
      <Autocomplete filter={contains}>
        <SearchField aria-label={t('pal.search')} autoFocus className="palette-search">
          <IconSearch size={18} className="search-icon" />
          <Input className="palette-input" placeholder={t('pal.search')} />
        </SearchField>
        <Menu
          className="palette-menu"
          aria-label={t('pal.title')}
          renderEmptyState={() => <p className="palette-empty">{t('pal.empty')}</p>}
          onAction={(key) => {
            const cmd = enabled.find((c) => c.id === key);
            close();
            if (cmd) setTimeout(cmd.run, 0);
          }}
        >
          {GROUPS.map((g) => {
            const items = enabled.filter((c) => c.group === g.id);
            if (!items.length) return null;
            return (
              <MenuSection key={g.id} id={g.id} className="palette-section">
                <Header className="palette-header">{t(g.label)}</Header>
                {items.map((c) => (
                  <MenuItem key={c.id} id={c.id} className="palette-item" textValue={`${c.label} ${c.keywords ?? ''}`}>
                    <span className="palette-icon" aria-hidden="true">
                      {c.icon}
                    </span>
                    <span className="palette-label">{c.label}</span>
                    {c.keys ? <Keys keys={c.keys} /> : null}
                  </MenuItem>
                ))}
              </MenuSection>
            );
          })}
        </Menu>
      </Autocomplete>
    </ModalShell>
  );
}

export function ShortcutsDialog() {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const { mod, enter, shift, isMac } = useModKey();
  const isOpen = state.overlay === 'shortcuts';
  const close = () => dispatch({ type: 'overlay', overlay: null });
  const groups: { title: MsgKey; rows: [string[], MsgKey][] }[] = [
    {
      title: 'sh.group.general',
      rows: [
        [[mod, 'K'], 'sh.palette'],
        [[mod, 'O'], 'sh.open'],
        [[mod, ','], 'sh.settings'],
        [['?'], 'sh.help'],
        [['Esc'], 'sh.close'],
      ],
    },
    {
      title: 'sh.group.workspace',
      rows: [
        [[mod, enter], 'sh.sign'],
        [['←', '↑', '→', '↓'], 'sh.move'],
        [[shift, '←'], 'sh.moveFine'],
        [[isMac ? '⌥' : 'Alt', '←'], 'sh.moveCoarse'],
        [[t.lang === 'de' ? 'Bild ↑' : 'PgUp', t.lang === 'de' ? 'Bild ↓' : 'PgDn'], 'sh.page'],
        [[mod, '+', '−'], 'sh.zoom'],
        [[mod, '0'], 'sh.fit'],
        [[t('sh.dblclick')], 'sh.place'],
      ],
    },
  ];
  return (
    <ModalShell isOpen={isOpen} onClose={close} size="md" className="shortcuts-modal">
      <DialogHeader title={t('sh.title')} onClose={close} />
      <div className="shortcuts-body">
        {groups.map((g) => (
          <section key={g.title} className="shortcuts-group" aria-labelledby={`sh-${g.title}`}>
            <h3 className="shortcuts-title" id={`sh-${g.title}`}>
              {t(g.title)}
            </h3>
            <dl className="shortcuts-list">
              {g.rows.map(([keys, label]) => (
                <div key={label} className="shortcuts-row">
                  <dt>{t(label)}</dt>
                  <dd>
                    <Keys keys={keys} label={keys.join(' ')} />
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
