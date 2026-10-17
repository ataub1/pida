/**
 * Geparkte Arbeit auf der Startansicht (ADR 0004 Nachtrag 10): „Zur Startseite“ schließt nichts, der Kern parkt
 * das offene Dokument (mit Platzierung) bzw. den Stapel (`ViewState::parked`).
 *
 * - Karte „In Bearbeitung“ (Stil der Karten der Startansicht): Dokumentname bzw. „Stapel · n Dokumente“,
 *   „Weiter bearbeiten“ (`resume_parked`, zurück in den Arbeitsbereich bzw. die Stapelübersicht; die Platzierung
 *   bleibt bzw. wird Plan des Eintrags) und ein kleines „Schließen“ (`discard_parked`; signierte Dateien bleiben).
 *   Verwirft „Schließen“ Arbeit der Nutzerin – eigene Platzierung, Stapel mit mehreren Dokumenten, signiert und
 *   nicht gespeichert –, fragt es kurz nach (Standard „Abbrechen“).
 * - Rückfrage beim Wählen bzw. Ablegen neuer PDFs und bei „Erneut signieren“ aus „Zuletzt signiert“
 *   (`ParkedJoinDialog`, von der Shell geöffnet): Standard „Zum Stapel hinzufügen“ (`open_files { mode: 'add' }` bzw.
 *   `sign_recent { mode: 'add' }`), „Offene schließen und neu beginnen“ (`mode: 'replace'`), Abbrechen ändert nichts.
 *   „Öffnen mit“ bzw. eine zweite Instanz fragen nicht – der Kern fügt hinzu und meldet es.
 * - „Weiter bearbeiten“ eines Stapels führt in die Stapelübersicht (Nachtrag 13; der Kern nennt ihn dann
 *   „Stapel · n Dokumente“, `doc_name = null`), sonst zurück ins Dokument.
 *
 * Was „hinzufügen“, „fortsetzen“ und „verwerfen“ bedeutet, entscheidet der Kern (§6.4); die Rückfrage liest nur
 * seine Angaben (`placed_by_hand`, `documents`, `unsaved`).
 */
import { useId, useState } from 'react';
import { useApp } from '../../app/context';
import type { ParkedWorkView, UiEvent } from '../../core/types';
import type { Translator } from '../../i18n/catalog';
import { Button } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/Dialog';
import { IconFileText, IconLayers } from '../../ui/Icons';

/** Titel der Karte: Dokumentname bzw. „Stapel · n Dokumente“. */
function parkedTitle(t: Translator, parked: ParkedWorkView): string {
  return parked.doc_name ?? t.t('ng-ui-start-parked-batch', { n: parked.documents });
}

export function ParkedCard({ parked }: { parked: ParkedWorkView }) {
  const { t, dispatch } = useApp();
  const [asking, setAsking] = useState(false);
  const titleId = useId();
  const batch = parked.doc_name === null;
  // „Schließen“ verwirft Arbeit der Nutzerin → kurze Rückfrage; sonst geht nichts verloren
  const confirm = parked.placed_by_hand || parked.documents > 1 || parked.unsaved;
  const hint = parked.unsaved
    ? 'ng-ui-start-parked-hint-unsaved'
    : batch
      ? 'ng-ui-start-parked-hint-batch'
      : parked.placed_by_hand
        ? 'ng-ui-start-parked-hint-placed'
        : 'ng-ui-start-parked-hint';
  const name = parked.doc_name ?? '';
  const closeText = parked.unsaved ? 'ng-ui-start-parked-close-text-unsaved' : batch ? 'ng-ui-start-parked-close-text-batch' : 'ng-ui-start-parked-close-text';
  const discard = () => void dispatch({ type: 'discard_parked' });

  return (
    <section className="side-card parked-card" aria-label={t.t('ng-ui-start-parked-label')} data-testid="parked-card">
      <span className="parked-icon" aria-hidden="true">
        {batch ? <IconLayers size={22} /> : <IconFileText size={22} />}
      </span>
      <div className="parked-text">
        <p className="parked-label">{t.t('ng-ui-start-parked-label')}</p>
        <h2 className="side-title parked-title" id={titleId}>
          {parkedTitle(t, parked)}
        </h2>
        <p className="muted parked-hint">{t.t(hint)}</p>
      </div>
      <div className="parked-actions">
        <Button variant="primary" onPress={() => void dispatch({ type: 'resume_parked' })} aria-describedby={titleId}>
          {t.t('ng-ui-start-parked-resume')}
        </Button>
        <Button variant="ghost" size="sm" onPress={() => (confirm ? setAsking(true) : discard())} aria-describedby={titleId}>
          {t.t('ng-ui-start-parked-close')}
        </Button>
      </div>
      {asking ? (
        <ConfirmDialog
          title={t.t('ng-ui-start-parked-close-title')}
          tone="warning"
          onCancel={() => setAsking(false)}
          actions={
            <>
              <Button
                variant="secondary"
                onPress={() => {
                  setAsking(false);
                  discard();
                }}
              >
                {t.t('ng-ui-start-parked-close-confirm')}
              </Button>
              {/* Standardknopf: „Abbrechen“ (Fokus, Eingabe) – Escape bricht ebenfalls ab */}
              <Button variant="primary" onPress={() => setAsking(false)} autoFocus>
                {t.t('common-Cancel')}
              </Button>
            </>
          }
        >
          <p>{t.t(closeText, { name })}</p>
        </ConfirmDialog>
      ) : null}
    </section>
  );
}

/**
 * Was bei geparkter Arbeit dazukommen soll: gewählte bzw. abgelegte PDFs (`paths`) oder die Datei eines Eintrags aus
 * „Zuletzt signiert“ („Erneut signieren“, `recentId`; den Pfad kennt nur der Kern).
 */
export type JoinRequest = { paths: string[] } | { recentId: number };

/**
 * Rückfrage, wenn bei geparkter Arbeit neue PDFs gewählt bzw. abgelegt werden oder ein zuletzt signiertes Dokument
 * erneut signiert werden soll: Standard „Zum Stapel hinzufügen“, „Offene schließen und neu beginnen“, Abbrechen (auch
 * Escape) ändert nichts.
 */
export function ParkedJoinDialog({ parked, request, onClose }: { parked: ParkedWorkView; request: JoinRequest; onClose: () => void }) {
  const { t, dispatch } = useApp();
  const count = 'paths' in request ? request.paths.length : 1;
  const open = (mode: 'add' | 'replace') => {
    onClose();
    const event: UiEvent = 'paths' in request ? { type: 'open_files', paths: request.paths, mode } : { type: 'sign_recent', id: request.recentId, mode };
    void dispatch(event);
  };
  const text = parked.doc_name === null ? t.t('ng-ui-start-join-text-batch', { count }) : t.t('ng-ui-start-join-text', { name: parked.doc_name, count });
  return (
    <ConfirmDialog
      title={t.t('ng-ui-start-join-title')}
      tone="info"
      onCancel={onClose}
      actions={
        <>
          <Button variant="ghost" onPress={onClose}>
            {t.t('common-Cancel')}
          </Button>
          <Button variant="secondary" onPress={() => open('replace')}>
            {t.t('ng-ui-start-join-replace')}
          </Button>
          {/* Standardknopf (Fokus, Eingabe): die neuen Dokumente kommen zur offenen Arbeit */}
          <Button variant="primary" onPress={() => open('add')} autoFocus>
            {t.t('ng-ui-start-join-add')}
          </Button>
        </>
      }
    >
      <p>{text}</p>
    </ConfirmDialog>
  );
}
