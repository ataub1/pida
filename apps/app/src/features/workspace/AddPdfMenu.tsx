/**
 * „+ PDF“ in der Kopfzeile neben dem Dokumentnamen (ADR 0004 Nachtrag 9): Knopf mit Symbol, kurzer Beschriftung
 * und Tooltip, Menü im Stil des Konto-Chips (React Aria `Menu`, Tastatur: Eingabe/Leertaste/Pfeil ↓ öffnen,
 * Pfeile wählen, Esc schließt).
 *
 * - „Weitere PDFs hinzufügen …“: Dateiauswahl mit Mehrfachauswahl → `open_files { mode: 'add' }`. Das offene
 *   Dokument bleibt; alle zusammen werden zum Stapel, die Platzierung des offenen Dokuments bleibt dessen Plan.
 * - „Anderes PDF öffnen …“: eine Datei → `open_files { mode: 'replace' }`; das offene Dokument wird geschlossen.
 *   Hat die Nutzerin die Signatur selbst platziert, fragt eine kurze Rückfrage vorher nach (Standard
 *   „Abbrechen“) – ohne eigene Platzierung geht nichts verloren, dann ohne Rückfrage.
 *
 * Was „hinzufügen“ bzw. „anderes öffnen“ im jeweiligen Zustand bedeutet, entscheidet der Kern (§6.4); Mobile
 * öffnet dieselben Wege über die System-Dateiauswahl (`browse`).
 */
import { useState } from 'react';
import { Button as AriaButton, Menu, MenuItem, MenuTrigger, Popover, Text, Tooltip, TooltipTrigger, type Key } from 'react-aria-components';
import { useApp } from '../../app/context';
import type { DocumentView } from '../../core/types';
import { Button } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/Dialog';
import { IconFilePlus, IconFolder, IconPlus } from '../../ui/Icons';
import { placedByHand } from './placement';

export function AddPdfMenu({ doc }: { doc: DocumentView }) {
  const { t, view, browse } = useApp();
  const [asking, setAsking] = useState(false);
  const name = doc.doc.name;
  const label = t.t('ng-ui-shell-add-long');

  const onAction = (key: Key) => {
    if (key === 'add') void browse('add');
    else if (key === 'replace') {
      if (placedByHand(doc)) setAsking(true);
      else void browse('replace');
    }
  };

  return (
    <>
      <MenuTrigger>
        <TooltipTrigger delay={500} closeDelay={80}>
          <AriaButton className="btn btn-ghost btn-sm header-add" aria-label={label} isDisabled={!view.actions.open_files} data-testid="header-add">
            {/* schmal nur das Dokument-Symbol mit Plus (der Dokumentname braucht den Platz) */}
            <IconPlus size={16} className="header-add-plus" />
            <IconFilePlus size={18} className="header-add-file" />
            <span className="header-add-text">{t.t('ng-ui-shell-add')}</span>
          </AriaButton>
          <Tooltip className="tooltip" offset={6}>
            {label}
          </Tooltip>
        </TooltipTrigger>
        <Popover className="popover" placement="bottom start" offset={6}>
          <Menu className="menu" aria-label={label} onAction={onAction}>
            <MenuItem id="add" className="menu-item" textValue={t.t('ng-ui-shell-add-more')}>
              <span className="menu-item-icon" aria-hidden="true">
                <IconFilePlus size={18} />
              </span>
              <span className="menu-item-text">
                <Text slot="label" className="menu-item-label">
                  {t.t('ng-ui-shell-add-more')}
                </Text>
                <Text slot="description" className="menu-item-desc">
                  {t.t('ng-ui-shell-add-more-desc', { name })}
                </Text>
              </span>
            </MenuItem>
            <MenuItem id="replace" className="menu-item" textValue={t.t('ng-ui-shell-replace')}>
              <span className="menu-item-icon" aria-hidden="true">
                <IconFolder size={18} />
              </span>
              <span className="menu-item-text">
                <Text slot="label" className="menu-item-label">
                  {t.t('ng-ui-shell-replace')}
                </Text>
                <Text slot="description" className="menu-item-desc">
                  {t.t('ng-ui-shell-replace-desc', { name })}
                </Text>
              </span>
            </MenuItem>
          </Menu>
        </Popover>
      </MenuTrigger>
      {asking ? (
        <ConfirmDialog
          title={t.t('ng-ui-shell-replace-title')}
          tone="warning"
          onCancel={() => setAsking(false)}
          actions={
            <>
              <Button
                variant="secondary"
                onPress={() => {
                  setAsking(false);
                  void browse('replace');
                }}
              >
                {t.t('ng-ui-shell-replace-confirm')}
              </Button>
              {/* Standardknopf: „Abbrechen“ (Fokus, Eingabe) – Escape bricht ebenfalls ab */}
              <Button variant="primary" onPress={() => setAsking(false)} autoFocus>
                {t.t('common-Cancel')}
              </Button>
            </>
          }
        >
          <p>{t.t('ng-ui-shell-replace-text', { name })}</p>
        </ConfirmDialog>
      ) : null}
    </>
  );
}
