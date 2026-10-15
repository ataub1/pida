/**
 * „Signatur fortsetzen“ (Mobile, Plan §3.5; neu gestaltet über der Startansicht, ADR 0004 §2.1): Beim
 * letzten Lauf wurde die App während eines Signiervorgangs vom System beendet (z. B. beim Wechsel in die
 * App „ID Austria“). Der Kern hat das vorbereitete Dokument gesichert und bietet es im Schritt „Öffnen“ an;
 * Fortsetzen signiert mit derselben Position und demselben Signiermittel weiter (Anmeldung und Bestätigung
 * erneut), Verwerfen löscht den gesicherten Zustand. Geht das Dokument mit dem Signiermittel an A-Trust
 * (`document_leaves_device`, ID Austria), steht der Kurzhinweis mit „Details“ auch hier (ADR 0003):
 * „Fortsetzen“ startet die Sitzung – eine ausdrückliche Nutzeraktion (A1).
 */
import { useState } from 'react';
import type { ResumeOfferView } from '../core/types';
import { PrivacyNote } from '../features/settings';
import { AccessButton } from '../ui/controls';
import { ConfirmDialog } from '../ui/Dialog';
import { IconPhone } from '../ui/Icons';
import { useApp } from './context';

export function ResumeDialog({ offer }: { offer: ResumeOfferView }) {
  const { t, dispatch } = useApp();
  const [busy, setBusy] = useState(false);
  const answer = async (resume: boolean) => {
    if (busy) return;
    setBusy(true);
    const ok = await dispatch(resume ? { type: 'resume_signature' } : { type: 'discard_resume' });
    if (!ok) setBusy(false);
  };
  return (
    // ohne Escape/Abbrechen: Verwerfen löscht den gesicherten Zustand und soll nur ausdrücklich passieren
    <ConfirmDialog
      title={t.t('ng-ui-resume-title')}
      icon={<IconPhone size={20} />}
      actions={
        <>
          <AccessButton msg="ng-ui-resume-discard" variant="ghost" isDisabled={busy} onPress={() => void answer(false)} />
          <AccessButton msg="ng-ui-resume-continue" variant="primary" isDisabled={busy} onPress={() => void answer(true)} autoFocus />
        </>
      }
    >
      <p>{t.t('ng-ui-resume-text', { name: offer.name, minutes: offer.age_minutes })}</p>
      {offer.pending > 0 ? (
        // Zahlen gehen als Text an Fluent (wie in Rust) – Einzahl deshalb als eigene Meldung
        <p className="hint">{offer.pending === 1 ? t.t('ng-ui-resume-pending-one') : t.t('ng-ui-resume-pending', { count: offer.pending })}</p>
      ) : null}
      {offer.document_leaves_device ? <PrivacyNote /> : null}
    </ConfirmDialog>
  );
}
