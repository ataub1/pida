/**
 * Konto-Chip des Signiermittels mit Umschaltmenü (Gestalt aus dem Prototyp, ADR 0004 §3.4). Optionen,
 * Verfügbarkeit, maskiertes Konto und Standard kommen aus `ViewState::signers`.
 *
 * - Wahl im Menü → `select_signer` (startet nichts, A1); „Als Standard verwenden“ → `make_default`.
 * - Öffnet „Signieren“ das Menü (kein Signiermittel gewählt, „Jedes Mal fragen“, §8 F12), sendet die Wahl
 *   direkt `sign_with` – eine ausdrückliche Aktion. In der Stapelübersicht startet die Wahl stattdessen den
 *   Stapel (`onSignWith`, „Alle signieren mit X“).
 * - Nur ein Signiermittel (iOS/Android: nur ID Austria, ADR 0004 Nachtrag 11): nichts zu wechseln – das Menü
 *   zeigt dann keine Signiermittel (kein leeres „Signiermittel wechseln“), nur „Einstellungen …“; öffnet
 *   „Signieren“ das Menü, steht das eine Signiermittel als ausdrückliche Wahl darin.
 */
import { Button, Header, Menu, MenuItem, MenuSection, MenuTrigger, Popover, Separator, type Key } from 'react-aria-components';
import { useApp } from '../../app/context';
import { isMobile, type BackendId } from '../../core/types';
import { IconCheck, IconChevronDown, IconSettings, IconStar } from '../../ui/Icons';
import { chipLabel, SHOWN_BACKENDS, SignerIcon, signerClass, signerDescription } from './signers';

export function SignerChip({
  compact = false,
  isOpen,
  onOpenChange,
  signMode,
  onSignWith,
}: {
  compact?: boolean;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  /** Menü wurde von „Signieren“ geöffnet: die Wahl signiert sofort */
  signMode: boolean;
  /** statt `sign_with`: was die Wahl im Signier-Modus auslöst (Stapel: „Alle signieren mit X“) */
  onSignWith?: (backend: BackendId) => void;
}) {
  const { t, view, dispatch, openSettings } = useApp();
  const mobile = isMobile(view);
  const options = view.signers.options.filter((o) => SHOWN_BACKENDS.includes(o.backend));
  const selected = view.signers.selected;
  const current = options.find((o) => o.backend === selected);
  const label = current ? chipLabel(t, current) : t.t('ng-ui-insp-signer-none');
  // ein einziges, schon gewähltes Signiermittel: nichts zu wechseln (Signier-Modus: die Wahl bleibt die Aktion)
  const switchable = signMode || options.length > 1 || (options.length === 1 && options[0]?.backend !== selected);
  const canMakeDefault = selected !== null && selected !== view.signers.default;
  const disabled = options.filter((o) => !o.available).map((o) => `signer:${o.backend}`);

  const onAction = (key: Key) => {
    const k = String(key);
    if (k.startsWith('signer:')) {
      const backend = k.slice('signer:'.length) as BackendId;
      if (signMode && onSignWith) onSignWith(backend);
      else if (signMode) void dispatch({ type: 'sign_with', backend });
      else void dispatch({ type: 'select_signer', backend, make_default: false });
    } else if (k === 'default' && selected) {
      void dispatch({ type: 'select_signer', backend: selected, make_default: true });
    } else if (k === 'manage') {
      openSettings('signers');
    }
  };

  return (
    <>
      <MenuTrigger isOpen={isOpen} onOpenChange={onOpenChange}>
        <Button className={`signer-chip${compact ? ' is-compact' : ''}`} aria-label={t.t(switchable ? 'ng-ui-insp-chip-label' : 'ng-ui-insp-chip-label-single', { value: label })} data-testid="signer-chip">
          <span className={`signer-chip-icon ${signerClass(selected)}`}>
            <SignerIcon backend={selected} size={16} />
          </span>
          <span className="signer-chip-text" title={label}>{label}</span>
          <IconChevronDown size={16} className="signer-chip-caret" />
        </Button>
        <Popover className="popover" placement="bottom end" offset={6}>
          <Menu
            className="menu"
            onAction={onAction}
            disabledKeys={disabled}
            aria-label={t.t(signMode ? 'ng-ui-insp-sign-with' : switchable ? 'ng-ui-insp-switch' : 'ng-ui-insp-signer-menu')}
          >
            {switchable ? (
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
            ) : null}
            {switchable ? <Separator className="menu-sep" /> : null}
            <MenuSection>
              {canMakeDefault && !signMode ? (
                <MenuItem id="default" className="menu-item" textValue={t.t('ng-ui-insp-make-default')}>
                  <span className="menu-item-icon">
                    <IconStar size={18} />
                  </span>
                  <span className="menu-item-label">{t.t('ng-ui-insp-make-default')}</span>
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
    </>
  );
}
