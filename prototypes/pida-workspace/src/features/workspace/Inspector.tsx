/**
 * Schmale rechte Seitenleiste des Arbeitsbereichs (unter 760 px als Bottom-Sheet): Signiermittel,
 * Signaturbild, Optionen, Platzierung, vorhandene Signaturen, primärer Knopf „Signieren“.
 * Bewusst ohne Zahlenfelder für die Position (vom Auftraggeber verworfen).
 */
import { useId, useRef, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { SIGNER_NAME, useAppState, useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import { blockSize, humanDateTime } from '../../model/format';
import type { DocState, PageSpec, Rect } from '../../model/types';
import { Badge, Button, Keys, SwitchField, useModKey } from '../../ui/controls';
import { IconAlert, IconChevronDown, IconInfo, IconPen, IconShieldCheck, IconSignature, IconTarget, IconWand } from '../../ui/Icons';
import { useFitScale, useNarrow } from '../../ui/hooks';
import { SignatureBlock } from '../sigblock/SignatureBlock';
import { SignerChip } from './SignerChip';
import { overlapCount, zoneLabel } from './placement';

export function Inspector({
  doc,
  pages,
  sticker,
  signTime,
  onSign,
  onAutoPlace,
}: {
  doc: DocState;
  pages: PageSpec[];
  sticker: Rect | null;
  signTime: string;
  onSign: () => void;
  onAutoPlace: () => void;
}) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const { mod, enter } = useModKey();
  const [expanded, setExpanded] = useState(false);
  const narrow = useNarrow();
  const bodyId = useId();
  const sig = state.settings.sigBlock;
  const placement = doc.placement;
  const page = placement ? pages[placement.page] : undefined;
  const field = placement?.fieldId ? doc.spec.fields.find((f) => f.id === placement.fieldId) : undefined;
  const overlap = sticker && page && !field ? overlapCount(sticker, page) > 0 : false;
  const alreadySigned = doc.spec.signatures.length > 0;
  const previewRef = useRef<HTMLDivElement>(null);
  const previewScale = useFitScale(previewRef, blockSize(sig).w, { min: 0.4, max: 1.2, pad: 28, initial: sig.profile === 'amt' ? 0.6 : 1.05 });

  return (
    <aside className="inspector" data-expanded={expanded || undefined} data-narrow={narrow || undefined} aria-label={t('insp.label')}>
      {narrow ? (
        <AriaButton className="sheet-handle" aria-expanded={expanded} aria-controls={bodyId} onPress={() => setExpanded((v) => !v)}>
          <span className="sheet-grip" aria-hidden="true" />
          <span className="sheet-handle-text">{expanded ? t('insp.sheet.collapse') : t('insp.sheet.expand')}</span>
          <IconChevronDown size={16} className="sheet-caret" />
        </AriaButton>
      ) : null}

      <div className="inspector-scroll" id={bodyId} hidden={narrow && !expanded}>
        {!narrow ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-signer`}>
            <h2 className="insp-heading" id={`${bodyId}-signer`}>
              {t('insp.signWith')}
            </h2>
            <SignerChip />
            {state.signer === 'idaustria' ? (
              <p className="insp-note">
                <IconInfo size={15} />
                {t('insp.leaves')}
              </p>
            ) : null}
          </section>
        ) : null}

        <section className="insp-section" aria-labelledby={`${bodyId}-block`}>
          <div className="insp-heading-row">
            <h2 className="insp-heading" id={`${bodyId}-block`}>
              {t('insp.block')}
            </h2>
            <Button variant="ghost" size="sm" onPress={() => dispatch({ type: 'overlay', overlay: 'settings', section: 'block' })} aria-label={t('insp.block.edit')}>
              <IconPen size={15} />
              {t('common.edit')}
            </Button>
          </div>
          <div className={`insp-block-preview${doc.invisible ? ' is-muted' : ''}`} ref={previewRef}>
            <div className="insp-block-scroller">
              <SignatureBlock sig={sig} signer={SIGNER_NAME} time={signTime} scale={previewScale} />
            </div>
          </div>
        </section>

        <section className="insp-section" aria-labelledby={`${bodyId}-opts`}>
          <h2 className="insp-heading" id={`${bodyId}-opts`}>
            {t('insp.options')}
          </h2>
          <SwitchField
            label={t('insp.invisible')}
            hint={t('insp.invisible.hint')}
            isSelected={doc.invisible}
            onChange={(v) => dispatch({ type: 'setInvisible', id: doc.spec.id, value: v })}
          />
          <SwitchField
            label={t('insp.newPage')}
            hint={alreadySigned ? t('insp.newPage.signed') : t('insp.newPage.hint')}
            isSelected={doc.newPage}
            isDisabled={alreadySigned || doc.invisible}
            onChange={(v) => dispatch({ type: 'setNewPage', id: doc.spec.id, value: v })}
          />
        </section>

        {!doc.invisible && placement && page && sticker ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-place`}>
            <div className="insp-heading-row">
              <h2 className="insp-heading" id={`${bodyId}-place`}>
                {t('insp.placement')}
              </h2>
              {!field ? (
                <Button variant="ghost" size="sm" onPress={onAutoPlace}>
                  <IconWand size={15} />
                  {t('ws.autoPlace')}
                </Button>
              ) : null}
            </div>
            <p className="insp-placement">
              <IconTarget size={16} />
              <span>
                {field
                  ? field.kind === 'qr'
                    ? t('insp.placement.qr')
                    : t('insp.placement.field', { name: field.label })
                  : t('insp.placement.text', { page: placement.page + 1, zone: zoneLabel(t, sticker, page) })}
              </span>
            </p>
            {field ? (
              <Button variant="secondary" size="sm" onPress={onAutoPlace}>
                {t('insp.placement.free')}
              </Button>
            ) : (
              <p className="insp-hint">{t('insp.placement.hint')}</p>
            )}
          </section>
        ) : null}

        {alreadySigned ? (
          <section className="insp-section" aria-labelledby={`${bodyId}-existing`}>
            <h2 className="insp-heading" id={`${bodyId}-existing`}>
              {t('insp.existing')}
            </h2>
            <ul className="existing-list">
              {doc.spec.signatures.map((s) => (
                <li key={s.id} className="existing-item">
                  <IconShieldCheck size={18} className={`existing-icon is-${s.status}`} />
                  <div className="existing-text">
                    <span className="existing-signer">{s.signer}</span>
                    <span className="existing-meta">
                      {s.role} · {humanDateTime(s.time, t.lang)}
                    </span>
                  </div>
                  <Badge tone={s.status === 'valid' ? 'success' : s.status === 'invalid' ? 'danger' : 'warning'}>{t(`status.${s.status}`)}</Badge>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </div>

      {/* Überdeckung direkt über „Signieren“ zeigen (auch im eingeklappten Bottom-Sheet), damit sie vor
          dem Signieren nicht unten in der Seitenleiste verschwindet. Angesagt wird sie über die
          Live-Region des Stickers. */}
      {overlap && !doc.invisible ? (
        <div className="callout callout-warning inspector-warn">
          <IconAlert size={18} />
          <div>
            <p>{t('ws.overlap')}</p>
            <Button variant="secondary" size="sm" onPress={onAutoPlace}>
              {t('ws.overlap.fix')}
            </Button>
          </div>
        </div>
      ) : null}

      <div className="inspector-footer">
        {narrow ? (
          <div className="inspector-footer-chip">
            <SignerChip compact />
          </div>
        ) : null}
        <Button variant="primary" size="lg" className="sign-button" onPress={onSign}>
          <IconSignature size={18} />
          <span>{t('insp.sign')}</span>
          <Keys keys={[mod, enter]} decorative />
        </Button>
        {state.batchMode ? (
          <Button variant="ghost" size="sm" className="insp-back" onPress={() => dispatch({ type: 'backToBatch' })}>
            {t('insp.signBatchBack')}
          </Button>
        ) : null}
      </div>
    </aside>
  );
}
