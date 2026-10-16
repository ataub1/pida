/**
 * Inspektor des Arbeitsbereichs (Gestalt aus dem Prototyp, ADR 0004 §2.4), unter 760 px als Bottom-Sheet
 * (eingeklappt: Konto-Chip, „Signieren“, Überdeckungswarnung). Abschnitte: Signieren mit (Konto-Chip,
 * Kurzhinweis Datenschutz), Signaturbild (Vorschau des Kerns, „Bearbeiten“ → Einstellungen ›
 * Signaturbild), Optionen (Unsichtbar, Auf neuer Seite – gesperrt mit Begründung aus `constraints`),
 * Platzierung (sprechende Lage, „Neu platzieren“/„Frei platzieren“, **keine** Zahlenfelder, D1),
 * dezente Hinweise auf Erkanntes (Einstellungen › Dokument, `DocumentView::detection`: PDF/A erkannt bzw.
 * erzwungen; QR-Platzhalter bzw. Signaturfeld nur bei automatischer Platzierung),
 * vorhandene Signaturen, primärer Knopf „Signieren ⌘↵“, darunter „Zurück zum Stapel“ (aus der
 * Stapelübersicht geöffnet) bzw. „Zum Stapel“ (weitere Dokumente). In der Phase `warning` steht hier die Warnung
 * der Eingangsanalyse mit „Trotzdem signieren“/„Verwerfen“.
 *
 * Einstellung „Automatische Positionierung“ (`placement_locked`, ADR 0004 Nachtrag 8): Platzierung „Automatisch
 * (laut Einstellung)“ mit „Diesmal selbst platzieren“ (`unlock_placement`) statt „Neu platzieren“/„Frei
 * platzieren“; nach dem Öffnen liegt der Fokus auf „Signieren“ (das Dokument ist bereit). Optionen bleiben
 * bedienbar.
 */
import { useId, useRef, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { useApp } from '../../app/context';
import { tileScale } from '../../core/bridge';
import { defaultDocumentDetection } from '../../core/defaults';
import type { DocumentView, StepView } from '../../core/types';
import { AccessButton, Badge, Button, Keys, SwitchField, useModKey } from '../../ui/controls';
import { useFitScale, useNarrow } from '../../ui/hooks';
import { IconAlert, IconChevronDown, IconFileText, IconGrip, IconLock, IconPen, IconQr, IconShieldCheck, IconSignature, IconTarget, IconWand } from '../../ui/Icons';
import { PrivacyNote } from '../settings';
import { displayPages, overlapState, zoneText } from './placement';
import { SignerChip } from './SignerChip';

function formatTime(iso: string | null, lang: 'de' | 'en'): string | null {
  if (!iso) return null;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return null;
  return new Intl.DateTimeFormat(lang === 'de' ? 'de-AT' : 'en-GB', { dateStyle: 'medium', timeStyle: 'short' }).format(d);
}

export function Inspector({
  doc,
  chipOpen,
  onChipOpenChange,
  signMode,
  onSign,
}: {
  doc: DocumentView;
  chipOpen: boolean;
  onChipOpenChange: (open: boolean, signMode?: boolean) => void;
  signMode: boolean;
  onSign: () => void;
}) {
  const { t, view, dispatch, openSettings, bridge } = useApp();
  const { mod, enter } = useModKey();
  const [expanded, setExpanded] = useState(false);
  const narrow = useNarrow();
  const bodyId = useId();
  const previewRef = useRef<HTMLDivElement>(null);
  const previewScale = useFitScale(previewRef, doc.block.width, { min: 0.4, max: 1.2, pad: 28, initial: doc.block.width > 300 ? 0.6 : 1.05 });
  const editable = view.actions.edit_placement && doc.phase === 'editing';
  // „Automatische Positionierung“: automatisch platziert, bereit zum Signieren; verschieben erst nach „Diesmal
  // selbst platzieren“ (der Kern ignoriert Verschieben und Zielwahl bis dahin)
  const locked = editable && doc.placement_locked;
  const unlock = () => void dispatch({ type: 'unlock_placement' });
  const warning = doc.phase === 'warning' && view.step.step === 'warning' ? (view.step as Extract<StepView, { step: 'warning' }>) : null;
  const sizes = displayPages(doc);
  const page = doc.placement.page_index;
  const [pw, ph] = sizes[page] ?? sizes[sizes.length - 1] ?? [595.28, 841.89];
  const overlap = overlapState(doc);
  const c = doc.constraints;
  const selected = view.signers.options.find((o) => o.backend === view.signers.selected);
  const leaves = selected?.capabilities?.document_leaves_device === true;
  const field = doc.target.kind === 'field' ? doc.fields.find((f) => doc.target.kind === 'field' && f.index === doc.target.index) : undefined;
  const reason = (id: string) => (c.reasons.includes(id) ? t.code(id) : null);
  // Einstellungen › Dokument (E3): Erkanntes nur nennen, solange die Automatik platziert hat (nicht nach „Ins
  // Feld setzen“ bzw. „Frei platzieren“ von Hand); PDF/A wie beim Signieren entschieden (Kern)
  const detection = doc.detection ?? defaultDocumentDetection();
  const detected = detection.auto_placed && (doc.target.kind === 'qr' || doc.target.kind === 'field') ? doc.target.kind : null;
  const pdfaNote =
    detection.pdfa_decision === 'detected' ? t.t('ng-ui-insp-detect-pdfa') : detection.pdfa_decision === 'forced' ? t.t('ng-ui-insp-detect-pdfa-forced') : null;
  // aus der Stapelübersicht geöffnet: „Zurück zum Stapel“ (Platzierung wird als Plan gemerkt, §3.8)
  const backToBatch = view.batch?.open_item === doc.doc.id;
  const newPageReason = !c.new_page_allowed ? (reason('ng-constraint-signed') ?? reason('ng-constraint-certified') ?? t.t('ng-ui-insp-new-page-locked')) : null;
  const invisibleReason = !c.visible_allowed ? (reason('ng-constraint-invisible-profile') ?? null) : null;
  const chip = (compact: boolean) => (
    <SignerChip compact={compact} isOpen={chipOpen} onOpenChange={(open) => onChipOpenChange(open, false)} signMode={signMode} />
  );

  return (
    <aside className="inspector" data-expanded={expanded || undefined} data-narrow={narrow || undefined} aria-label={t.t('ng-ui-insp-label')}>
      {narrow ? (
        <AriaButton className="sheet-handle" aria-expanded={expanded} aria-controls={bodyId} onPress={() => setExpanded((v) => !v)}>
          <span className="sheet-grip" aria-hidden="true" />
          <span className="sheet-handle-text">{expanded ? t.t('ng-ui-insp-sheet-collapse') : t.t('ng-ui-insp-sheet-expand')}</span>
          <IconChevronDown size={16} className="sheet-caret" />
        </AriaButton>
      ) : null}

      <div className="inspector-scroll" id={bodyId} hidden={narrow && !expanded}>
        {warning ? (
          <section className="insp-section insp-warning" aria-labelledby={`${bodyId}-warn`}>
            <h2 className="insp-heading" id={`${bodyId}-warn`}>
              {t.t('ng-ui-insp-warning')}
            </h2>
            <div className="callout callout-warning" role="alert">
              <IconAlert size={18} />
              <div>
                <p className="callout-title">{t.t('ng-ui-warning-lead')}</p>
                <ul className="state-list">
                  {warning.codes.map((code) => (
                    <li key={code} data-code={code}>
                      {t.code(code)}
                    </li>
                  ))}
                </ul>
                <p>{t.t('ng-ui-warning-question')}</p>
              </div>
            </div>
            <div className="button-row">
              <Button variant="primary" onPress={() => void dispatch({ type: 'answer_warning', proceed: true })}>
                {t.t('ng-ui-insp-warning-proceed')}
              </Button>
              <Button variant="ghost" onPress={() => void dispatch({ type: 'answer_warning', proceed: false })}>
                {t.t('ng-ui-insp-warning-discard')}
              </Button>
            </div>
          </section>
        ) : null}

        {!narrow && !warning ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-signer`}>
            <h2 className="insp-heading" id={`${bodyId}-signer`}>
              {t.t('ng-ui-insp-sign-with')}
            </h2>
            {chip(false)}
            {leaves ? <PrivacyNote /> : null}
          </section>
        ) : null}

        <section className="insp-section" aria-labelledby={`${bodyId}-block`}>
          <div className="insp-heading-row">
            <h2 className="insp-heading" id={`${bodyId}-block`}>
              {t.t('ng-ui-insp-block')}
            </h2>
            <Button variant="ghost" size="sm" onPress={() => openSettings('signature')} isDisabled={!view.actions.settings} aria-label={t.t('ng-ui-insp-block-edit')}>
              <IconPen size={15} />
              {t.t('ng-ui-insp-edit')}
            </Button>
          </div>
          <div className={`insp-block-preview${doc.invisible ? ' is-muted' : ''}`} ref={previewRef}>
            <div className="insp-block-scroller" style={{ width: doc.block.width * previewScale, height: doc.block.height * previewScale }}>
              {/* Revision 0 = der Kern hat (noch) keine Vorschau gebaut, z. B. Startargument-Automatik ohne
                  Positionieren – kein Abruf, der nur mit „keine Blockvorschau“ scheitert */}
              {doc.block.preview_revision > 0 ? (
                <img
                className="insp-preview-img"
                src={bridge.blockPreviewUrl(tileScale(previewScale * 2), doc.block.preview_revision)}
                alt={t.t('ng-ui-insp-block-alt')}
                width={Math.round(doc.block.width * previewScale)}
                height={Math.round(doc.block.height * previewScale)}
                draggable={false}
                />
              ) : null}
            </div>
          </div>
        </section>

        {!warning ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-opts`}>
            <h2 className="insp-heading" id={`${bodyId}-opts`}>
              {t.t('ng-ui-insp-options')}
            </h2>
            <SwitchField
              label={t.t('ng-ui-insp-invisible')}
              hint={invisibleReason ?? t.t('ng-ui-insp-invisible-hint')}
              isSelected={doc.invisible}
              isDisabled={!editable || !c.visible_allowed}
              onChange={(v) => void dispatch({ type: 'set_invisible', invisible: v })}
            />
            <SwitchField
              label={t.t('ng-ui-insp-new-page')}
              hint={newPageReason ?? t.t('ng-ui-insp-new-page-hint')}
              isSelected={doc.new_page}
              isDisabled={!editable || doc.invisible || (!c.new_page_allowed && !doc.new_page)}
              onChange={() => void dispatch({ type: 'toggle_new_page' })}
            />
            {pdfaNote ? (
              <p className="insp-note" data-testid="pdfa-note">
                <IconFileText size={15} />
                {pdfaNote}
              </p>
            ) : null}
          </section>
        ) : null}

        {!warning && !doc.invisible && doc.sticker ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-place`}>
            <div className="insp-heading-row">
              <h2 className="insp-heading" id={`${bodyId}-place`}>
                {t.t('ng-ui-insp-placement')}
              </h2>
              {doc.target.kind === 'free' && !locked ? (
                <Button variant="ghost" size="sm" onPress={() => void dispatch({ type: 'auto_place' })} isDisabled={!editable}>
                  <IconWand size={15} />
                  {t.t('ng-ui-insp-auto-place')}
                </Button>
              ) : null}
            </div>
            {locked && detection.auto_placed ? (
              <p className="insp-placement" data-testid="placement-auto">
                <IconLock size={16} />
                <span>{t.t('ng-ui-insp-placement-auto')}</span>
              </p>
            ) : (
              <p className="insp-placement">
                {locked ? <IconLock size={16} /> : <IconTarget size={16} />}
                <span>
                  {field
                    ? t.t('ng-ui-insp-placement-field', { name: field.name })
                    : doc.target.kind === 'qr'
                      ? t.t('ng-ui-insp-placement-qr')
                      : t.t('ng-ui-insp-placement-text', { page: page + 1, zone: zoneText(t, doc.sticker, pw, ph) })}
                </span>
              </p>
            )}
            {detected ? (
              <p className="insp-note" data-testid="detect-note">
                {detected === 'qr' ? <IconQr size={15} /> : <IconPen size={15} />}
                {t.t(detected === 'qr' ? 'ng-ui-insp-detect-qr' : 'ng-ui-insp-detect-field')}
              </p>
            ) : null}
            {locked ? (
              <>
                <p className="insp-hint">{t.t('ng-ui-insp-placement-locked-hint')}</p>
                <Button variant="secondary" size="sm" onPress={unlock} data-testid="placement-unlock">
                  <IconGrip size={15} />
                  {t.t('ng-ui-insp-placement-unlock')}
                </Button>
              </>
            ) : doc.target.kind !== 'free' ? (
              <Button variant="secondary" size="sm" onPress={() => void dispatch({ type: 'use_target', target: { kind: 'free' } })} isDisabled={!editable}>
                {t.t('ng-ui-insp-placement-free')}
              </Button>
            ) : (
              <p className="insp-hint">{t.t('ng-ui-insp-placement-hint')}</p>
            )}
          </section>
        ) : null}

        {doc.existing.length > 0 ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-existing`}>
            <h2 className="insp-heading" id={`${bodyId}-existing`}>
              {t.t('ng-ui-insp-existing')}
            </h2>
            <ul className="existing-list">
              {doc.existing.map((s) => {
                const tone = s.status === 'valid' ? 'success' : s.status === 'invalid' ? 'danger' : 'warning';
                const time = formatTime(s.signing_time, t.lang);
                return (
                  <li key={s.field_name} className="existing-item">
                    <IconShieldCheck size={18} className={`existing-icon is-${s.status === 'valid' ? 'valid' : s.status === 'invalid' ? 'invalid' : 'unknown'}`} />
                    <div className="existing-text">
                      <span className="existing-signer">{s.signer ?? t.t('ng-ui-insp-existing-unknown')}</span>
                      <span className="existing-meta">{time ? t.t('ng-ui-insp-existing-meta', { time }) : s.field_name}</span>
                    </div>
                    <Badge tone={tone}>{t.t(`ng-ui-insp-status-${s.status === 'not_verifiable' ? 'unknown' : s.status}`)}</Badge>
                  </li>
                );
              })}
            </ul>
          </section>
        ) : null}
      </div>

      {/* Überdeckung direkt über „Signieren“ (auch im eingeklappten Bottom-Sheet), damit sie vor dem
          Signieren nicht unten in der Seitenleiste verschwindet; angesagt über die Live-Region. */}
      {overlap && !warning ? (
        <div className="callout callout-warning inspector-warn" data-testid="overlap-warning">
          <IconAlert size={18} />
          <div>
            <p>{t.t(overlap === 'widget' ? 'ng-ui-insp-overlap-widget' : 'ng-ui-insp-overlap')}</p>
            {locked ? (
              <Button variant="secondary" size="sm" onPress={unlock}>
                {t.t('ng-ui-insp-placement-unlock')}
              </Button>
            ) : (
              <Button variant="secondary" size="sm" onPress={() => void dispatch({ type: 'auto_place' })} isDisabled={!editable}>
                {t.t('ng-ui-insp-overlap-fix')}
              </Button>
            )}
          </div>
        </div>
      ) : null}

      {/* Schmal (Bottom-Sheet): der Konto-Chip steht unten – der Kurzhinweis „wird an A-Trust übermittelt“
          (ADR 0003, „bei der Signiermittel-Auswahl“) deshalb direkt darüber, auch eingeklappt */}
      {narrow && leaves && !warning ? (
        <div className="inspector-privacy">
          <PrivacyNote />
        </div>
      ) : null}

      {!warning ? (
        <div className="inspector-footer">
          {narrow ? <div className="inspector-footer-chip">{chip(true)}</div> : null}
          <AccessButton
            msg="ng-ui-insp-sign"
            variant="primary"
            size="lg"
            className="sign-button"
            onPress={onSign}
            isDisabled={!view.actions.sign}
            // beim Öffnen gesperrt = bereit: Fokus auf „Signieren“ (React Aria wertet `autoFocus` nur beim Einhängen
            // aus – nach der Analysewarnung bzw. beim Öffnen, nie bei späteren Änderungen)
            autoFocus={locked && view.actions.sign}
            icon={<IconSignature size={18} />}
          >
            <Keys keys={[mod, enter]} decorative />
          </AccessButton>
          {backToBatch || view.actions.show_batch ? (
            <Button variant="ghost" size="sm" className="insp-back" onPress={() => void dispatch({ type: backToBatch ? 'back_to_batch' : 'show_batch' })} isDisabled={!editable}>
              {t.t(backToBatch ? 'ng-ui-batch-back' : 'ng-ui-batch-show')}
            </Button>
          ) : null}
        </div>
      ) : null}
    </aside>
  );
}
