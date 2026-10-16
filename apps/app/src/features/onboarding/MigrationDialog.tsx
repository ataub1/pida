/**
 * Migrations-Assistent (Plan §7.2): einmaliges Angebot, die Einstellungen einer früheren
 * PDF-Over-Installation (`~/.pdf-over/PDF-Over.config`) zu übernehmen. Klartext-Passwörter der alten
 * Konfiguration gehen nur mit ausdrücklicher Zustimmung in den Schlüsselbund (Standard: nein).
 * „Nicht übernehmen“ beendet das Angebot dauerhaft (`decline_migration`).
 */
import { useState } from 'react';
import { useApp } from '../../app/context';
import type { MigrationOfferView } from '../../core/types';
import { AccessButton, CheckboxField } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/Dialog';

export function MigrationDialog({ offer }: { offer: MigrationOfferView }) {
  const { t, dispatch } = useApp();
  const [storeSecrets, setStoreSecrets] = useState(false);
  const [busy, setBusy] = useState(false);
  const answer = async (accept: boolean) => {
    if (busy) return;
    setBusy(true);
    const ok = await dispatch(
      accept ? { type: 'accept_migration', store_secrets: offer.plaintext_secrets && storeSecrets } : { type: 'decline_migration' },
    );
    if (!ok) setBusy(false);
  };
  return (
    <ConfirmDialog
      title={t.t('ng-migrate-offer-title')}
      actions={
        <>
          <AccessButton msg="ng-ui-migrate-decline" variant="quiet" isDisabled={busy} onPress={() => void answer(false)} />
          <AccessButton msg="ng-ui-migrate-accept" variant="primary" isDisabled={busy} onPress={() => void answer(true)} autoFocus />
        </>
      }
    >
      <p className="pre-line">{t.t('ng-migrate-offer-text', { path: offer.legacy_file })}</p>
      {offer.plaintext_secrets ? (
        <CheckboxField label={t.t('ng-migrate-secrets-consent')} isSelected={storeSecrets} onChange={setStoreSecrets} />
      ) : null}
    </ConfirmDialog>
  );
}
