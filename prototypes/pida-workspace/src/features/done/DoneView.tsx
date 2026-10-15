/** Fertig: Bestätigung mit Prüfsiegel, Dateiname, Aktionen und Link zur Signaturprüfung. */
import { useEffect, useRef } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { displayPages, SIGNER_NAME, useAppState, useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import { humanDateTime } from '../../model/format';
import { Button } from '../../ui/controls';
import { IconArrowLeft, IconCheck, IconExternal, IconFileText, IconFolder, IconShare, IconShieldCheck } from '../../ui/Icons';
import { SignatureBlock } from '../sigblock/SignatureBlock';
import { MockPage } from '../workspace/MockPage';
import { stickerRect } from '../workspace/placement';

const PREVIEW_W = 170;

export function DoneView({ onNext }: { onNext: () => void }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const last = state.lastSigned;
  const doc = state.docs.find((d) => d.spec.id === last?.docId);

  useEffect(() => {
    headingRef.current?.focus({ preventScroll: true });
  }, []);

  if (!last || !doc) return null;
  const pages = displayPages(doc);
  const pageIndex = doc.invisible || !doc.placement ? pages.length - 1 : doc.placement.page;
  const page = pages[pageIndex];
  const scale = page ? PREVIEW_W / page.w : 1;
  const rect = doc.placement && !doc.invisible ? stickerRect(doc, doc.placement, pages, state.settings.sigBlock) : null;
  const demo = (title: string) => dispatch({ type: 'toast', toast: { kind: 'info', title } });

  return (
    <div className="done-view">
      <section className="done-card" aria-labelledby="done-title">
        <div className="done-seal" aria-hidden="true">
          <span className="done-seal-ring" />
          <IconCheck size={30} />
        </div>
        <h1 className="done-title" id="done-title" ref={headingRef} tabIndex={-1}>
          {t('done.title')}
        </h1>
        <p className="seal-badge">
          <IconShieldCheck size={18} />
          {t('done.seal')}
        </p>
        <p className="done-text">{t('done.text')}</p>

        <div className="done-file">
          <div className="done-preview" role="img" aria-label={t('done.preview')}>
            {page ? (
              <div className="done-preview-page" style={{ width: PREVIEW_W, height: page.h * scale }}>
                {pageIndex < doc.spec.pages.length ? <MockPage page={page} scale={scale} signatures={doc.spec.signatures} /> : <div className="mock-page-box blank" style={{ width: PREVIEW_W, height: page.h * scale }} />}
                {rect ? (
                  <div className="done-preview-sticker" style={{ left: rect.x * scale, top: rect.y * scale }}>
                    <SignatureBlock
                      sig={state.settings.sigBlock}
                      signer={SIGNER_NAME}
                      time={last.time}
                      scale={scale}
                      fit={doc.placement?.fieldId ? { w: rect.w, h: rect.h } : undefined}
                    />
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
          <div className="done-file-info">
            <p className="done-file-name">
              <IconFileText size={18} />
              <span>{last.outputName}</span>
            </p>
            <p className="done-file-meta">{t('done.savedIn', { folder: last.folder })}</p>
            <p className="done-file-meta">{t('done.signedBy', { name: SIGNER_NAME, time: humanDateTime(last.time, t.lang) })}</p>
            <div className="done-actions">
              <Button variant="secondary" onPress={() => demo(t('done.demo.open', { file: last.outputName }))}>
                <IconFileText size={17} />
                {t('done.open')}
              </Button>
              <Button variant="secondary" onPress={() => demo(t('done.demo.reveal', { folder: last.folder }))}>
                <IconFolder size={17} />
                {t('done.reveal')}
              </Button>
              <Button variant="secondary" onPress={() => demo(t('done.demo.share'))}>
                <IconShare size={17} />
                {t('done.share')}
              </Button>
            </div>
          </div>
        </div>

        <div className="done-footer">
          <AriaButton className="link" onPress={() => demo(t('done.demo.check'))}>
            {t('done.check')}
            <IconExternal size={14} />
            <span className="sr-only"> {t('common.externalHint')}</span>
          </AriaButton>
          {state.batchMode ? (
            <Button variant="primary" size="lg" onPress={() => dispatch({ type: 'backToBatch' })}>
              <IconArrowLeft size={18} />
              {t('done.backToBatch')}
            </Button>
          ) : (
            <Button variant="primary" size="lg" onPress={onNext}>
              {t('done.next')}
            </Button>
          )}
        </div>
      </section>
    </div>
  );
}
