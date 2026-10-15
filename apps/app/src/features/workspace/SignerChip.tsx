/**
 * Konto-Chip des Signiermittels mit Umschaltmenü (Gestalt aus dem Prototyp, ADR 0004 §3.4). Optionen,
 * Verfügbarkeit, maskiertes Konto und Standard kommen aus `ViewState::signers`.
 *
 * - Wahl im Menü → `select_signer` (startet nichts, A1); „Als Standard verwenden“ → `make_default`.
 * - Öffnet „Signieren“ das Menü (kein Signiermittel gewählt, „Jedes Mal fragen“, §8 F12), sendet die Wahl
 *   direkt `sign_with` – eine ausdrückliche Aktion.
 * - Mobile: „PKCS#12 importieren …“ öffnet den Import-Assistenten (UI-B).
 */
import { useState } from 'react';
import { Button, Header, Menu, MenuItem, MenuSection, MenuTrigger, Popover, Separator, type Key } from 'react-aria-components';
import { useApp } from '../../app/context';
import { isMobile, type BackendId } from '../../core/types';
import { IconCheck, IconChevronDown, IconKey, IconSettings, IconStar } from '../../ui/Icons';
import { KeystoreImportDialog } from '../settings';
import { chipLabel, SHOWN_BACKENDS, SignerIcon, signerClass, signerDescription } from './signers';

export function SignerChip({
  compact = false,
  isOpen,
  onOpenChange,
  signMode,
}: {
  compact?: boolean;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  /** Menü wurde von „Signieren“ geöffnet: die Wahl signiert sofort */
  signMode: boolean;
}) {
  const { t, view, dispatch, openSettings } = useApp();
  const [importing, setImporting] = useState(false);
  const mobile = isMobile(view);
  const options = view.signers.options.filter((o) => SHOWN_BACKENDS.includes(o.backend));
  const selected = view.signers.selected;
  const current = options.find((o) => o.backend === selected);
  const label = current ? chipLabel(t, current) : t.t('ng-ui-insp-signer-none');
  const offerImport = mobile && !options.some((o) => o.backend === 'keystore' && o.available);
  const canMakeDefault = selected !== null && selected !== view.signers.default;
  const disabled = options.filter((o) => !o.available).map((o) => `signer:${o.backend}`);

  const onAction = (key: Key) => {
    const k = String(key);
    if (k.startsWith('signer:')) {
      const backend = k.slice('signer:'.length) as BackendId;
      if (signMode) void dispatch({ type: 'sign_with', backend });
      else void dispatch({ type: 'select_signer', backend, make_default: false });
    } else if (k === 'default' && selected) {
      void dispatch({ type: 'select_signer', backend: selected, make_default: true });
    } else if (k === 'manage') {
      openSettings('signers');
    } else if (k === 'import') {
      setImporting(true);
    }
  };

  return (
    <>
      <MenuTrigger isOpen={isOpen} onOpenChange={onOpenChange}>
        <Button className={`signer-chip${compact ? ' is-compact' : ''}`} aria-label={t.t('ng-ui-insp-chip-label', { value: label })} data-testid="signer-chip">
          <span className={`signer-chip-icon ${signerClass(selected)}`}>
            <SignerIcon backend={selected} size={16} />
          </span>
          <span className="signer-chip-text">{label}</span>
          <IconChevronDown size={16} className="signer-chip-caret" />
        </Button>
        <Popover className="popover" placement="bottom end" offset={6}>
          <Menu className="menu" onAction={onAction} disabledKeys={disabled} aria-label={t.t(signMode ? 'ng-ui-insp-sign-with' : 'ng-ui-insp-switch')}>
            <MenuSection>
              <Header className="menu-header">{t.t(signMode ? 'ng-ui-insp-sign-with' : 'ng-ui-insp-switch')}</Header>
              {options.map((o) => (
                <MenuItem key={o.backend} id={`signer:${o.backend}`} className="menu-item" textValue={chipLabel(t, o)}>
                  <span className={`menu-item-icon ${signerClass(o.backend)}`}>
                    <SignerIcon backend={o.backend} size={18} />
                  </span>
                  <span className="menu-item-text">
                    <span className="menu-item-label">{chipLabel(t, o)}</span>
                    <span className="menu-item-desc">{signerDescription(t, o, mobile)}</span>
                  </span>
                  {o.backend === selected && !signMode ? <IconCheck size={16} className="menu-item-check" /> : null}
                </MenuItem>
              ))}
            </MenuSection>
            <Separator className="menu-sep" />
            <MenuSection>
              {canMakeDefault && !signMode ? (
                <MenuItem id="default" className="menu-item" textValue={t.t('ng-ui-insp-make-default')}>
                  <span className="menu-item-icon">
                    <IconStar size={18} />
                  </span>
                  <span className="menu-item-label">{t.t('ng-ui-insp-make-default')}</span>
                </MenuItem>
              ) : null}
              {offerImport ? (
                <MenuItem id="import" className="menu-item" textValue={t.t('ng-ui-insp-signer-import')}>
                  <span className="menu-item-icon">
                    <IconKey size={18} />
                  </span>
                  <span className="menu-item-label">{t.t('ng-ui-insp-signer-import')}</span>
                </MenuItem>
              ) : null}
              <MenuItem id="manage" className="menu-item" textValue={t.t('ng-ui-insp-manage')} isDisabled={!view.actions.settings}>
                <span className="menu-item-icon">
                  <IconSettings size={18} />
                </span>
                <span className="menu-item-label">{t.t('ng-ui-insp-manage')}</span>
              </MenuItem>
            </MenuSection>
          </Menu>
        </Popover>
      </MenuTrigger>
      {importing ? <KeystoreImportDialog onClose={() => setImporting(false)} /> : null}
    </>
  );
}
