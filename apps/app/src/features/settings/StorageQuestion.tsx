/**
 * „Passwort merken?“ bei der ersten Verwendung des Keystores (Wunsch des Auftraggebers, Nachtrag ADR 0004):
 * kein eigener Dialog, sondern eine Auswahl direkt unter der Passworteingabe – in der Signierkarte
 * (`InteractionView::Secret::ask_storage`) und bei „Schlüssel anzeigen …“ in den Einstellungen
 * (`keystore.storage_chosen = false`). Voreinstellung „Bis zum Beenden“; die Wahl geht mit dem Absenden an
 * den Kern (`password_storage`), der sie als `keystore.password_storage` speichert und danach nicht mehr
 * fragt. „Im Schlüsselbund“ (Mobile: „Im sicheren Speicher“) nur mit verfügbarem Schlüsselbund.
 */
import { useApp } from '../../app/context';
import type { PasswordStorage } from '../../core/types';
import { ChoiceSegments } from './controls';

/** Voreinstellung der Auswahl. */
export const DEFAULT_STORAGE_CHOICE: PasswordStorage = 'memory';

export function PasswordStorageQuestion({
  value,
  onChange,
  keychainAvailable,
  isDisabled,
}: {
  value: PasswordStorage;
  onChange: (v: PasswordStorage) => void;
  keychainAvailable: boolean;
  isDisabled?: boolean;
}) {
  const { t, view } = useApp();
  const mobile = view.platform.mobile;
  return (
    <ChoiceSegments<PasswordStorage>
      label={t.t('ng-ui-set-ks-remember')}
      value={value}
      onChange={onChange}
      isDisabled={isDisabled}
      description={keychainAvailable ? t.t('ng-ui-set-ks-remember-hint') : t.t('ng-secrets-not-persistent')}
      options={[
        { value: 'none', label: t.t('ng-ui-set-ks-remember-none') },
        { value: 'memory', label: t.t('ng-ui-set-ks-remember-memory') },
        { value: 'keychain', label: t.t(mobile ? 'ng-ui-set-ks-remember-keychain-mobile' : 'ng-ui-set-ks-remember-keychain'), isDisabled: !keychainAvailable },
      ]}
    />
  );
}
