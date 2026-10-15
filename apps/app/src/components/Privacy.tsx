/**
 * Datenschutz ohne Zustimmungsdialog (ADR 0003): Statt eines blockierenden Hinweises vor dem ersten
 * Upload zeigt die Oberfläche beim Signiermittel mit `document_leaves_device` (ID Austria) einen
 * **Kurzhinweis** „Das Dokument wird zum Signieren an A-Trust übermittelt.“ mit „Details“. „Details“ öffnet
 * die Einstellungen beim Tab „Datenschutz“; während des Signierens (Einstellungen gesperrt wie in
 * PDF-Over) dieselben Angaben als schließbares Fenster.
 *
 * Der vollständige Text ([`PrivacyInfo`]) ist der Datenschutztext aus PDF-Over bzw. – per Richtlinie
 * `privacy_notice_text` – der eigene Text der Organisation, dazu der Hinweis auf das lokale Signieren mit
 * Keystore und Bürgerkarte und der Link zur Datenschutzerklärung von A-Trust.
 */
import { useState } from 'react';
import { Link } from 'react-aria-components';
import { useApp } from '../app/context';
import { IconShield } from './icons';
import { AccessButton, ConfirmDialog, Markup } from './ui';

/** Vollständige Datenschutzangaben (Einstellungen › Datenschutz und „Details“ im Signierbereich). */
export function PrivacyInfo() {
  const { t, view } = useApp();
  const url = t.t('config-DataProtectionURL');
  const custom = view.privacy_text;
  return (
    <>
      <p className="pre-line">
        {custom ? custom : <Markup text={t.t('config-DataProtectionStatement')} targets={[{ href: url }]} />}
      </p>
      <p>{t.t('ng-ui-privacy-local')}</p>
      {custom ? (
        // Der Text aus PDF-Over nennt die Datenschutzerklärung von A-Trust selbst; ein eigener Text evtl. nicht
        <p>
          <Markup text={t.t('ng-ui-privacy-atrust-link')} targets={[{ href: url }]} />
        </p>
      ) : null}
    </>
  );
}

/** Datenschutzangaben als schließbares Fenster (wenn die Einstellungen gerade gesperrt sind). */
function PrivacyDialog({ onClose }: { onClose: () => void }) {
  const { t } = useApp();
  return (
    <ConfirmDialog
      title={t.t('config-DataProtection')}
      onCancel={onClose}
      actions={<AccessButton msg="ng-ui-close" variant="primary" onPress={onClose} autoFocus />}
    >
      <PrivacyInfo />
    </ConfirmDialog>
  );
}

/**
 * Kurzhinweis beim Signiermittel (nicht blockierend). `id` kennzeichnet den Satz, damit die Karte des
 * Signiermittels ihn als Beschreibung referenzieren kann (`aria-describedby`, ohne „Details“).
 */
export function PrivacyNote({ id }: { id?: string }) {
  const { t, view, openSettings } = useApp();
  const [open, setOpen] = useState(false);
  const details = () => {
    // Einstellungen erreichbar (z. B. Signiermittel-Auswahl): dort beim Tab „Datenschutz“
    if (view.main_bar.settings_enabled && !view.settings) openSettings('privacy');
    else setOpen(true);
  };
  return (
    <>
      <p className="privacy-note" data-testid="privacy-note">
        <IconShield size={16} />
        <span>
          <span id={id}>{t.t('ng-ui-backend-leaves-device')}</span>{' '}
          <Link className="link" onPress={details} aria-label={t.t('ng-ui-privacy-details-label')}>
            {t.t('ng-ui-privacy-details')}
          </Link>
        </span>
      </p>
      {open ? <PrivacyDialog onClose={() => setOpen(false)} /> : null}
    </>
  );
}
