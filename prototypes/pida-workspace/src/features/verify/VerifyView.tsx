/** Prüfmodus: signiertes PDF ablegen → Signaturen mit Status und aufklappbaren Zertifikatsdetails. */
import { useRef, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { useDispatch, useAppState } from '../../app/store';
import { useT } from '../../i18n';
import { verifyDoc } from '../../model/documents';
import { humanDate, humanDateTime } from '../../model/format';
import type { DocSpec, ExistingSignature } from '../../model/types';
import { Badge, Button, Details } from '../../ui/controls';
import { useFitScale } from '../../ui/hooks';
import { IconAlertCircle, IconArrowLeft, IconCheck, IconFileText, IconShieldCheck, IconSignature, IconTarget, IconUpload } from '../../ui/Icons';
import { PdfDropZone } from '../start/PdfDropZone';
import { MockPage } from '../workspace/MockPage';

export function VerifyView({ onPickFiles }: { onPickFiles: () => void }) {
  const { verify } = useAppState();
  return verify ? <VerifyResult spec={verify} /> : <VerifyEmpty onPickFiles={onPickFiles} />;
}

function VerifyEmpty({ onPickFiles }: { onPickFiles: () => void }) {
  const t = useT();
  const dispatch = useDispatch();
  return (
    <div className="verify-view verify-empty">
      <p className="eyebrow">
        <IconShieldCheck size={16} />
        {t('vf.title')}
      </p>
      <h1 className="view-title">{t('vf.drop.title')}</h1>
      <p className="view-lead">{t('vf.lead')}</p>
      <PdfDropZone label={t('vf.drop.label')} onFiles={(names) => dispatch({ type: 'openVerify', spec: verifyDoc(names[0]) })} className="verify-drop">
        {({ isDropTarget }) => (
          <div className={`drop-inner drop-inner-compact${isDropTarget ? ' is-target' : ''}`}>
            <span className="drop-icon drop-icon-verify" aria-hidden="true">
              <IconUpload size={26} />
            </span>
            <p className="drop-title">{isDropTarget ? t('start.drop.release') : t('vf.drop.title')}</p>
            <div className="drop-buttons">
              <Button variant="primary" onPress={onPickFiles}>
                {t('vf.drop.button')}
              </Button>
              <Button variant="secondary" onPress={() => dispatch({ type: 'openVerify', spec: verifyDoc() })}>
                {t('vf.sample')}
              </Button>
            </div>
          </div>
        )}
      </PdfDropZone>
      <Button variant="ghost" onPress={() => dispatch({ type: 'goHome' })}>
        <IconArrowLeft size={17} />
        {t('vf.exit')}
      </Button>
    </div>
  );
}

function VerifyResult({ spec }: { spec: DocSpec }) {
  const t = useT();
  const dispatch = useDispatch();
  const previewRef = useRef<HTMLDivElement>(null);
  const [highlight, setHighlight] = useState<string | null>(null);
  const first = spec.pages[0];
  const scale = useFitScale(previewRef, first?.w ?? 595, { max: 1.1, pad: 40, initial: 0.9 });
  const sigs = spec.signatures;
  const anyInvalid = sigs.some((s) => s.status !== 'valid');
  const pageRefs = useRef<(HTMLDivElement | null)[]>([]);

  const showOnPage = (s: ExistingSignature) => {
    setHighlight(s.id);
    if (s.page !== null) pageRefs.current[s.page]?.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
  };

  return (
    <div className="verify-view verify-result">
      <section className="verify-preview" aria-label={t('vf.preview')} ref={previewRef}>
        <div className="verify-pages">
          {spec.pages.map((p, i) => (
            <div key={i} className="page-frame static" ref={(el) => { pageRefs.current[i] = el; }} role="img" aria-label={t('ws.pageAlt', { n: i + 1, total: spec.pages.length })} style={{ width: p.w * scale, height: p.h * scale }}>
              <MockPage page={p} scale={scale} signatures={spec.signatures} highlight={highlight} />
            </div>
          ))}
        </div>
      </section>

      <section className="verify-panel" aria-labelledby="vf-title">
        <p className="eyebrow">
          <IconShieldCheck size={16} />
          {t('vf.title')}
        </p>
        <h1 className="verify-file" id="vf-title">
          <IconFileText size={20} />
          <span>{spec.fileName}</span>
        </h1>
        {sigs.length === 0 ? (
          <div className="callout callout-info" role="status">
            <IconAlertCircle size={20} />
            <p className="callout-title">{t('vf.summary.none')}</p>
          </div>
        ) : anyInvalid ? (
          <div className="callout callout-danger" role="status">
            <IconAlertCircle size={20} />
            <div>
              <p className="callout-title">{t('vf.summary.invalid')}</p>
              <p>{t('vf.summary.invalidText')}</p>
            </div>
          </div>
        ) : (
          <div className="callout callout-success" role="status">
            <IconShieldCheck size={20} />
            <div>
              <p className="callout-title">{t.n('vf.summary.valid', sigs.length)}</p>
              <p>{t('vf.summary.validText')}</p>
            </div>
          </div>
        )}

        <h2 className="insp-heading">
          {t('vf.panel')} <span className="count-pill">{sigs.length}</span>
        </h2>
        <ol className="sig-list">
          {sigs.map((s) => (
            <li key={s.id} className={`sig-card is-${s.status}`} onMouseEnter={() => setHighlight(s.id)} onMouseLeave={() => setHighlight(null)}>
              <div className="sig-card-head">
                <span className={`sig-status-icon is-${s.status}`} aria-hidden="true">
                  {s.status === 'valid' ? <IconCheck size={16} /> : <IconAlertCircle size={16} />}
                </span>
                <div className="sig-card-titles">
                  <h3 className="sig-signer">{s.signer}</h3>
                  <p className="sig-role">{s.role}</p>
                </div>
                <Badge tone={s.status === 'valid' ? 'success' : s.status === 'invalid' ? 'danger' : 'warning'}>{t(`vf.status.${s.status}`)}</Badge>
              </div>
              <ul className="sig-facts">
                <li>
                  <IconSignature size={15} />
                  {t('vf.sig.time', { time: humanDateTime(s.time, t.lang) })}
                </li>
                <li>
                  <IconShieldCheck size={15} />
                  {t(`vf.sig.level.${s.level}`)} · {t('vf.sig.method', { method: s.method })}
                </li>
                <li className={s.coversWholeDocument ? 'is-ok' : 'is-bad'}>
                  {s.coversWholeDocument ? <IconCheck size={15} /> : <IconAlertCircle size={15} />}
                  {s.coversWholeDocument ? t('vf.sig.integrity.ok') : t('vf.sig.integrity.bad')}
                </li>
              </ul>
              <Details label={t('vf.cert')} level={4}>
                <dl className="cert-list">
                  <div>
                    <dt>{t('vf.cert.issuer')}</dt>
                    <dd>{s.issuer}</dd>
                  </div>
                  <div>
                    <dt>{t('vf.cert.serial')}</dt>
                    <dd className="mono">{s.serial}</dd>
                  </div>
                  <div>
                    <dt>{t('vf.cert.validity')}</dt>
                    <dd>{t('vf.cert.validityRange', { from: humanDate(s.validFrom, t.lang), to: humanDate(s.validTo, t.lang) })}</dd>
                  </div>
                  <div>
                    <dt>{t('vf.cert.algorithm')}</dt>
                    <dd>{s.algorithm}</dd>
                  </div>
                  <div>
                    <dt>{t('vf.cert.format')}</dt>
                    <dd>{s.format}</dd>
                  </div>
                  <div>
                    <dt>{t('vf.cert.appearance')}</dt>
                    <dd>{s.page !== null ? t('vf.cert.appearancePage', { n: s.page + 1 }) : t('vf.cert.invisible')}</dd>
                  </div>
                </dl>
              </Details>
              {s.page !== null ? (
                <AriaButton className="link sig-show" onPress={() => showOnPage(s)} onFocus={() => setHighlight(s.id)} onBlur={() => setHighlight(null)}>
                  <IconTarget size={15} />
                  {t('vf.showOnPage')}
                </AriaButton>
              ) : null}
            </li>
          ))}
        </ol>

        <div className="verify-actions">
          <Button variant="primary" onPress={() => dispatch({ type: 'signVerified' })}>
            <IconSignature size={17} />
            {t('vf.sign')}
          </Button>
          <Button variant="secondary" onPress={() => dispatch({ type: 'toast', toast: { kind: 'info', title: t('vf.report.demo') } })}>
            {t('vf.report')}
          </Button>
          <Button variant="ghost" onPress={() => dispatch({ type: 'openVerify', spec: null })}>
            {t('vf.other')}
          </Button>
        </div>
      </section>
    </div>
  );
}
