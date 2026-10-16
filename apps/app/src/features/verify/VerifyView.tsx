/**
 * Prüfmodus (ADR 0004 §2.9, §3.9; Gestalt aus dem Prototyp `features/verify/VerifyView.tsx`): links die Seiten
 * des geprüften Dokuments (Kacheln des Kerns über `pida-render://page/<verify.doc.id>/…`), rechts das
 * Gesamtergebnis und je Signatur eine Karte mit Status (intakt, verändert, nicht prüfbar), Unterzeichner,
 * Zeitpunkt (als Angabe des Signierenden gekennzeichnet), Qualifikation „laut Zertifikat“, Abdeckung samt
 * späteren Revisionen, Hinweisen (z. B. SHA-1) und aufklappbaren Zertifikatsdetails.
 *
 * „Auf der Seite zeigen“ hebt das Signaturbild hervor (Rechteck aus `AppearanceView` in `PAGE_ABS` der
 * sichtbaren Seite) und blättert dorthin; Überfahren bzw. Fokus einer Karte hebt ebenfalls hervor. Unsichtbare
 * Signaturen haben keinen Knopf. Wortlaut nach §8 F3 (nie „gültig“), immer mit dem Hinweis auf die vollständige
 * Prüfung bei signaturpruefung.gv.at. „Dieses Dokument signieren“ (`sign_verified`, Freigabe
 * `actions.open_files`), „Anderes Dokument prüfen“ (`open_verify`), „Prüfmodus verlassen“ (`close_verify`).
 * „Prüfbericht speichern“ gibt es in v1 nicht (§8 F8).
 */
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { useApp } from '../../app/context';
import { tileScale } from '../../core/bridge';
import { isMobile, type VerifiedSignatureView, type VerifyView as VerifyViewData } from '../../core/types';
import { Badge, Button, Details, Spinner } from '../../ui/controls';
import { useFitScale, usePrefersReducedMotion } from '../../ui/hooks';
import {
  IconAlert,
  IconAlertCircle,
  IconArrowLeft,
  IconCheck,
  IconExternal,
  IconFileText,
  IconInfo,
  IconShieldCheck,
  IconSignature,
  IconTarget,
} from '../../ui/Icons';
import {
  FORMAT_LABEL,
  FULL_CHECK_URL,
  STATUS_LABEL,
  STATUS_TONE,
  boxOnPage,
  codeText,
  formatDate,
  formatDateTime,
  formatSerial,
  integrityCode,
  integrityLines,
  methodText,
  qualificationText,
  signerName,
  statusOf,
  summaryOf,
  type SigStatus,
} from './model';
import './verify.css';

/** Props des Prüfmodus (in E0 festgelegt). */
export interface VerifyViewProps {
  verify: VerifyViewData;
}

const SUMMARY_ICON = {
  info: <IconInfo size={20} />,
  success: <IconShieldCheck size={20} />,
  warning: <IconAlert size={20} />,
  danger: <IconAlertCircle size={20} />,
} as const;

const STATUS_ICON: Record<SigStatus, ReactNode> = {
  intact: <IconCheck size={16} />,
  broken: <IconAlertCircle size={16} />,
  unverifiable: <IconAlert size={16} />,
};

export function VerifyView({ verify }: VerifyViewProps) {
  const { t, view, dispatch, bridge, toast, announce } = useApp();
  const mobile = isMobile(view);
  const reduced = usePrefersReducedMotion();
  const previewRef = useRef<HTMLElement>(null);
  const pageRefs = useRef<(HTMLDivElement | null)[]>([]);
  const [pinned, setPinned] = useState<number | null>(null);
  const [hover, setHover] = useState<number | null>(null);
  const doc = verify.doc;
  const firstW = doc?.page_sizes[0]?.[0] ?? 595;
  const scale = useFitScale(previewRef, firstW, { max: 1.1, pad: 40, initial: 0.9 });
  const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1;
  const tile = tileScale(scale * dpr);
  const status = verify.status;
  const ready = status.state === 'ready';
  const sigs = ready ? verify.signatures : [];
  const highlight = hover ?? pinned;

  // anderes Dokument: Hervorhebung zurücksetzen
  useEffect(() => {
    setPinned(null);
    setHover(null);
  }, [verify.file_name, doc?.id]);

  const pickOther = async () => {
    try {
      const paths = mobile
        ? await bridge.pickMobileFiles({ kind: 'pdf', multiple: false })
        : await bridge.pickFiles({ title: t.t('ng-ui-shell-verify-long'), multiple: false, filters: [{ name: t.t('common-PDFExtension_Description'), extensions: ['pdf'] }] });
      const path = paths?.[0];
      if (path) void dispatch({ type: 'open_verify', path });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };

  const showOnPage = (sig: VerifiedSignatureView) => {
    const a = sig.appearance;
    if (!a) return;
    setPinned(sig.index);
    pageRefs.current[a.page]?.scrollIntoView?.({ block: 'center', behavior: reduced ? 'auto' : 'smooth' });
    announce(t.t('ng-ui-vf-shown', { name: signerName(sig, t), page: a.page + 1 }));
  };

  const fullCheck = () => void bridge.openUrl(FULL_CHECK_URL).catch(() => toast(t.t('ng-ui-open-failed'), 'error'));

  return (
    <div className={`verify-view verify-result${doc ? '' : ' is-without-pages'}`} data-testid="verify-view">
      <section className="verify-preview" aria-label={t.t('ng-ui-vf-preview')} ref={previewRef}>
        {doc ? (
          <div className="verify-pages">
            {Array.from({ length: doc.page_count }, (_, i) => {
              const [w, h] = doc.page_sizes[i] ?? doc.page_sizes[0] ?? [595.28, 841.89];
              const marks = sigs.filter((s) => s.appearance?.page === i);
              return (
                <div
                  key={i}
                  className="vf-page"
                  ref={(el) => {
                    pageRefs.current[i] = el;
                  }}
                  role="img"
                  aria-label={t.t('ng-ui-vf-page', { n: i + 1, total: doc.page_count })}
                  style={{ width: w * scale, height: h * scale }}
                >
                  <img className="vf-page-tile" src={bridge.pageTileUrl(doc.id, i, tile)} alt="" loading="lazy" draggable={false} />
                  {marks.map((s) => {
                    const box = boxOnPage((s.appearance as NonNullable<typeof s.appearance>).rect, h, scale);
                    return <span key={s.index} className={`vf-mark${highlight === s.index ? ' is-active' : ''}`} style={box} data-testid={`vf-mark-${s.index}`} />;
                  })}
                </div>
              );
            })}
          </div>
        ) : (
          <div className="verify-preview-empty" aria-hidden="true">
            {status.state === 'loading' ? <Spinner size={28} /> : <IconFileText size={34} />}
          </div>
        )}
      </section>

      <section className="verify-panel" aria-labelledby="vf-title">
        <p className="eyebrow">
          <IconShieldCheck size={16} />
          {t.t('ng-ui-vf-title')}
        </p>
        <div className="verify-file-head">
          <h1 className="verify-file" id="vf-title" tabIndex={-1} data-main-heading="">
            <IconFileText size={20} />
            <span>{verify.file_name}</span>
          </h1>
          {doc && ready ? (
            <p className="verify-meta">
              {t.t('ng-ui-vf-meta', { pages: t.t('ng-ui-vf-pages', { n: doc.page_count }), revisions: t.t('ng-ui-vf-revisions', { n: verify.revisions }) })}
            </p>
          ) : null}
        </div>

        {status.state === 'loading' ? (
          <div className="callout callout-info" role="status">
            <Spinner size={20} />
            <p className="callout-title">{t.t('ng-ui-vf-loading')}</p>
          </div>
        ) : null}
        {status.state === 'failed' ? (
          <div className="callout callout-danger" role="alert">
            <IconAlertCircle size={20} />
            <div>
              <p className="callout-title">{t.t('ng-ui-vf-failed')}</p>
              <p>{t.code(status.code)}</p>
            </div>
          </div>
        ) : null}
        {ready ? <Summary verify={verify} /> : null}

        {sigs.length > 0 ? (
          <>
            <h2 className="vf-heading">
              {t.t('ng-ui-vf-signatures')} <span className="count-pill">{sigs.length}</span>
            </h2>
            <ol className="sig-list">
              {sigs.map((s) => (
                <SignatureCard
                  key={s.index}
                  sig={s}
                  onShow={() => showOnPage(s)}
                  onHover={(on) => setHover(on ? s.index : null)}
                />
              ))}
            </ol>
          </>
        ) : null}

        <div className="vf-scope">
          <IconInfo size={16} />
          <div>
            <p>{t.t('ng-ui-vf-scope')}</p>
            <AriaButton className="link" onPress={fullCheck}>
              {t.t('ng-ui-vf-full-check')}
              <IconExternal size={14} />
            </AriaButton>
          </div>
        </div>

        <div className="verify-actions">
          {ready ? (
            <Button variant="primary" onPress={() => void dispatch({ type: 'sign_verified' })} isDisabled={!view.actions.open_files}>
              <IconSignature size={17} />
              {t.t('ng-ui-vf-sign')}
            </Button>
          ) : null}
          <Button variant="secondary" onPress={() => void pickOther()} isDisabled={!view.actions.verify}>
            {t.t('ng-ui-vf-other')}
          </Button>
          <Button variant="ghost" onPress={() => void dispatch({ type: 'close_verify' })}>
            <IconArrowLeft size={17} />
            {t.t('ng-ui-vf-exit')}
          </Button>
        </div>
      </section>
    </div>
  );
}

function Summary({ verify }: { verify: VerifyViewData }) {
  const { t } = useApp();
  const s = summaryOf(verify, t);
  return (
    <div className={`callout callout-${s.tone}`} role="status" data-testid="vf-summary">
      {SUMMARY_ICON[s.tone]}
      <div>
        <p className="callout-title">{s.title}</p>
        <p>{s.text}</p>
      </div>
    </div>
  );
}

function Fact({ icon, tone, children }: { icon: ReactNode; tone?: 'ok' | 'info' | 'bad' | 'warn'; children: ReactNode }) {
  return (
    <li className={tone ? `is-${tone}` : undefined}>
      {icon}
      <span>{children}</span>
    </li>
  );
}

function Row({ label, children, mono }: { label: string; children: ReactNode; mono?: boolean }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={mono ? 'mono' : undefined}>{children}</dd>
    </div>
  );
}

function SignatureCard({ sig, onShow, onHover }: { sig: VerifiedSignatureView; onShow: () => void; onHover: (on: boolean) => void }) {
  const { t } = useApp();
  const status = statusOf(sig);
  const name = signerName(sig, t);
  const headingId = `vf-sig-${sig.index}`;
  const code = integrityCode(sig);
  const explained = code ? codeText(code, t) : null;
  const unknown = t.t('ng-ui-vf-cert-unknown');
  const lang = t.lang;
  const method = methodText(sig);
  return (
    <li className={`sig-card is-${status}`} onMouseEnter={() => onHover(true)} onMouseLeave={() => onHover(false)} data-testid={`vf-card-${sig.index}`}>
      <div className="sig-card-head">
        <span className={`sig-status-icon is-${status}`} aria-hidden="true">
          {STATUS_ICON[status]}
        </span>
        <div className="sig-card-titles">
          <h3 className="sig-signer" id={headingId}>
            {name}
          </h3>
          {sig.organization ? <p className="sig-role">{sig.organization}</p> : null}
        </div>
        <Badge tone={STATUS_TONE[status]}>{t.t(STATUS_LABEL[status])}</Badge>
      </div>
      <ul className="sig-facts">
        <Fact icon={<IconSignature size={15} />}>
          {sig.signing_time ? t.t('ng-ui-vf-time', { time: formatDateTime(sig.signing_time, lang) }) : t.t('ng-ui-vf-time-none')}
        </Fact>
        <Fact icon={<IconShieldCheck size={15} />}>{qualificationText(sig.qualification, t)}</Fact>
        {integrityLines(sig, t).map((l, i) => (
          <Fact key={i} tone={l.tone} icon={l.tone === 'ok' ? <IconCheck size={15} /> : l.tone === 'info' ? <IconInfo size={15} /> : <IconAlertCircle size={15} />}>
            {l.text}
          </Fact>
        ))}
        {explained ? (
          <Fact tone="bad" icon={<IconInfo size={15} />}>
            {explained}
          </Fact>
        ) : null}
        {sig.cert_valid_at_signing === false ? (
          <Fact tone="warn" icon={<IconAlert size={15} />}>
            {t.t('ng-ui-vf-time-outside')}
          </Fact>
        ) : null}
        {sig.warnings.map((w) => (
          <Fact key={w} tone="warn" icon={<IconAlert size={15} />}>
            {t.code(w)}
          </Fact>
        ))}
      </ul>
      <Details label={t.t('ng-ui-vf-details')} level={4}>
        <dl className="cert-list">
          {sig.organization ? <Row label={t.t('ng-ui-vf-cert-org')}>{sig.organization}</Row> : null}
          <Row label={t.t('ng-ui-vf-cert-issuer')}>{sig.issuer ?? unknown}</Row>
          <Row label={t.t('ng-ui-vf-cert-serial')} mono={!!sig.serial}>
            {sig.serial ? formatSerial(sig.serial) : unknown}
          </Row>
          <Row label={t.t('ng-ui-vf-cert-validity')}>
            {sig.cert_valid_from && sig.cert_valid_to ? t.t('ng-ui-vf-cert-range', { from: formatDate(sig.cert_valid_from, lang), to: formatDate(sig.cert_valid_to, lang) }) : unknown}
          </Row>
          <Row label={t.t('ng-ui-vf-cert-algorithm')}>{method ?? unknown}</Row>
          <Row label={t.t('ng-ui-vf-cert-format')}>{t.t(FORMAT_LABEL[sig.format])}</Row>
          <Row label={t.t('ng-ui-vf-cert-qualification')}>{t.t('ng-ui-vf-qual-hint')}</Row>
          <Row label={t.t('ng-ui-vf-cert-field')}>{sig.field_name}</Row>
          <Row label={t.t('ng-ui-vf-cert-appearance')}>{sig.appearance ? t.t('ng-ui-vf-cert-page', { n: sig.appearance.page + 1 }) : t.t('ng-ui-vf-cert-invisible')}</Row>
          <Row label={t.t('ng-ui-vf-cert-trust')}>{t.t('ng-ui-vf-cert-trust-not-checked')}</Row>
          {code ? (
            <Row label={t.t('ng-ui-vf-cert-code')} mono>
              {code}
            </Row>
          ) : null}
        </dl>
      </Details>
      {sig.appearance ? (
        <AriaButton className="link sig-show" onPress={onShow} onFocus={() => onHover(true)} onBlur={() => onHover(false)} aria-describedby={headingId}>
          <IconTarget size={15} />
          {t.t('ng-ui-vf-show')}
        </AriaButton>
      ) : null}
    </li>
  );
}
