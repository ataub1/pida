/** Konto-Chip des Signiermittels mit Umschaltmenü (ID Austria, Keystore, Chipkarte). */
import { Button, Header, Menu, MenuItem, MenuSection, MenuTrigger, Popover, Separator } from 'react-aria-components';
import { useAppState, useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import { maskUser } from '../../model/format';
import type { SignerKind } from '../../model/types';
import { IconCard, IconCheck, IconChevronDown, IconFileKey, IconPhone, IconSettings } from '../../ui/Icons';

export function signerIcon(kind: SignerKind, size = 18) {
  if (kind === 'keystore') return <IconFileKey size={size} />;
  if (kind === 'card') return <IconCard size={size} />;
  return <IconPhone size={size} />;
}

export function useSignerLabel() {
  const { settings } = useAppState();
  const t = useT();
  return (kind: SignerKind) =>
    kind === 'idaustria'
      ? t('chip.idaustria', { user: maskUser(settings.idaUser) })
      : kind === 'keystore'
        ? t('chip.keystore', { file: settings.keystoreFile })
        : t('chip.card');
}

export function SignerChip({ compact = false }: { compact?: boolean }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const label = useSignerLabel();
  const current = state.signer;
  const kinds: SignerKind[] = ['idaustria', 'keystore', 'card'];
  return (
    <MenuTrigger>
      <Button className={`signer-chip${compact ? ' is-compact' : ''}`} aria-label={t('chip.label', { value: label(current) })}>
        <span className={`signer-chip-icon signer-${current}`}>{signerIcon(current, 16)}</span>
        <span className="signer-chip-text">{label(current)}</span>
        <IconChevronDown size={16} className="signer-chip-caret" />
      </Button>
      <Popover className="popover" placement="bottom end" offset={6}>
        <Menu
          className="menu"
          onAction={(key) => {
            if (key === 'manage') dispatch({ type: 'overlay', overlay: 'settings', section: 'signer' });
          }}
        >
          <MenuSection
            selectionMode="single"
            disallowEmptySelection
            selectedKeys={[current]}
            onSelectionChange={(keys) => {
              const k = [...(keys as Set<string>)][0];
              if (k) dispatch({ type: 'setSigner', signer: k as SignerKind });
            }}
          >
            <Header className="menu-header">{t('insp.switch')}</Header>
            {kinds.map((k) => (
              <MenuItem key={k} id={k} className="menu-item" textValue={label(k)}>
                {({ isSelected }) => (
                  <>
                    <span className={`menu-item-icon signer-${k}`}>{signerIcon(k, 18)}</span>
                    <span className="menu-item-text">
                      <span className="menu-item-label">{label(k)}</span>
                      <span className="menu-item-desc">{t(`signer.${k}.desc`)}</span>
                    </span>
                    {isSelected ? <IconCheck size={16} className="menu-item-check" /> : null}
                  </>
                )}
              </MenuItem>
            ))}
          </MenuSection>
          <Separator className="menu-sep" />
          <MenuItem id="manage" className="menu-item" textValue={t('insp.manage')}>
            <span className="menu-item-icon">
              <IconSettings size={18} />
            </span>
            <span className="menu-item-label">{t('insp.manage')}</span>
          </MenuItem>
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
