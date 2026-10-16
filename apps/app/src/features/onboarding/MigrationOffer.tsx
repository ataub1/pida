/**
 * Angebot „PDF-Over-Einstellungen übernehmen“ (Plan §7.2, ADR 0004 §3.12): Pfad der Altdatei und die Liste
 * dessen, was übernommen würde (`MigrationOfferView::items`, Probelauf des Kerns). Klartext-Passwörter der
 * alten Konfiguration gehen nur mit ausdrücklicher Zustimmung in den Schlüsselbund (Standard: nein).
 * „Übernehmen“ = `accept_migration`, „Nicht übernehmen“ = `decline_migration` (über Einstellungen › Erweitert
 * erneut erreichbar).
 *
 * Bildmarke: Eine eigene Bildmarke der Altkonfiguration steht als eigener Eintrag in der Liste (der Kern
 * übernimmt nur eine tatsächlich gesetzte eigene, nicht den PDF-AS-Standardlogo-Pfad). Fehlt sie, sagt die
 * Ersteinrichtung ausdrücklich, dass das PIDA-Siegel bleibt (ADR 0005) – die Oberfläche ersetzt es nie.
 */
import { useState } from 'react';
import { useApp } from '../../app/context';
import type { MigrationItemView, MigrationOfferView } from '../../core/types';
import type { Translator } from '../../i18n/catalog';
import { Button, CheckboxField } from '../../ui/controls';
import { IconCheck, IconFolder, IconInfo } from '../../ui/Icons';
import './onboarding.css';

const EMBLEM_ITEM = 'ng-migration-item-emblem';

export interface MigrationOfferPaneProps {
  offer: MigrationOfferView;
  /** nach angenommener Antwort (`true` = übernommen) */
  onDone?: (accepted: boolean) => void;
  /** Überschriftenebene (Dialog: 3, in den Einstellungen: 4) */
  level?: 3 | 4;
  /** Hinweis „Es bleibt das PIDA-Siegel“, wenn keine eigene Bildmarke übernommen wird (Ersteinrichtung) */
  showSeal?: boolean;
  /** Kennung der Überschrift (Fokus nach dem Schrittwechsel) */
  titleId?: string;
}

/** Text eines Eintrags der Übernahmeliste („Benutzername für ID Austria: +43 660 ···123“). */
export function migrationItemText(item: MigrationItemView, t: Translator): string {
  const label = t.has(item.code) ? t.t(item.code) : item.code;
  return item.detail ? t.t('ng-ui-ob-import-item', { item: label, detail: item.detail }) : label;
}

export function MigrationOfferPane({ offer, onDone, level = 3, showSeal = false, titleId }: MigrationOfferPaneProps) {
  const { t, dispatch } = useApp();
  const [storeSecrets, setStoreSecrets] = useState(false);
  const [busy, setBusy] = useState(false);
  const Title = level === 3 ? 'h3' : 'h4';
  const ownEmblem = offer.items.some((i) => i.code === EMBLEM_ITEM);
  const listId = `${titleId ?? 'ob-import'}-list`;

  const answer = async (accept: boolean) => {
    if (busy) return;
    setBusy(true);
    const ok = await dispatch(accept ? { type: 'accept_migration', store_secrets: offer.plaintext_secrets && storeSecrets } : { type: 'decline_migration' });
    setBusy(false);
    if (ok) onDone?.(accept);
  };

  return (
    <div className="step-pane ob-import" data-testid="migration-offer">
      <span className="step-hero step-hero-info" aria-hidden="true">
        <IconFolder size={26} />
      </span>
      <Title className="step-title" id={titleId} tabIndex={-1}>
        {t.t('ng-ui-ob-import-title')}
      </Title>
      <p className="step-text">{t.t('ng-ui-ob-import-text')}</p>
      <p className="mono-small ob-path">{offer.legacy_file}</p>
      {offer.items.length > 0 ? (
        <>
          <p className="ob-list-label" id={listId}>
            {t.t('ng-ui-ob-import-list')}
          </p>
          <ul className="check-list" aria-labelledby={listId}>
            {offer.items.map((item) => (
              <li key={item.code}>
                <IconCheck size={16} />
                <span>{migrationItemText(item, t)}</span>
              </li>
            ))}
            {showSeal && !ownEmblem ? (
              <li className="is-note" data-testid="seal-stays">
                <IconInfo size={16} />
                <span>{t.t('ng-ui-ob-import-seal')}</span>
              </li>
            ) : null}
          </ul>
        </>
      ) : null}
      {offer.plaintext_secrets ? <CheckboxField label={t.t('ng-migrate-secrets-consent')} isSelected={storeSecrets} onChange={setStoreSecrets} /> : null}
      <div className="step-actions">
        <Button variant="secondary" isDisabled={busy} onPress={() => void answer(false)}>
          {t.t('ng-ui-ob-import-decline')}
        </Button>
        <Button variant="primary" isDisabled={busy} onPress={() => void answer(true)}>
          {t.t('ng-ui-ob-import-accept')}
        </Button>
      </div>
    </div>
  );
}
