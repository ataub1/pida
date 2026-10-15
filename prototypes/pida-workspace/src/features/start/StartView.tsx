/** Start/Leerzustand: große Ablagefläche, zuletzt signierte Dokumente, Hinweis auf den Prüfmodus. */
import { Button as AriaButton } from 'react-aria-components';
import { useAppState, useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import { contractDoc, verifySpecForRecent } from '../../model/documents';
import { humanDateTime } from '../../model/format';
import { Button, Keys, useModKey } from '../../ui/controls';
import { IconChevronRight, IconFileText, IconLock, IconShieldCheck, IconUpload } from '../../ui/Icons';
import { PdfDropZone } from './PdfDropZone';

export function StartView({ onPickFiles, onFiles }: { onPickFiles: () => void; onFiles: (names: string[]) => void }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const { mod } = useModKey();
  return (
    <div className="start-view">
      <div className="start-main">
        <h1 className="view-title start-title">{t('start.title')}</h1>
        <p className="view-lead">{t('start.lead')}</p>
        <PdfDropZone label={t('start.drop.label')} onFiles={onFiles} className="start-drop">
          {({ isDropTarget }) => (
            <div className={`drop-inner${isDropTarget ? ' is-target' : ''}`}>
              <span className="drop-icon" aria-hidden="true">
                <IconUpload size={30} />
              </span>
              <p className="drop-title">{isDropTarget ? t('start.drop.release') : t('start.drop.title')}</p>
              <p className="drop-hint">{t('start.drop.hint')}</p>
              <Button variant="primary" size="lg" onPress={onPickFiles}>
                {t('start.drop.button')}
              </Button>
              <p className="drop-foot">
                <span className="drop-foot-item">
                  <IconLock size={14} />
                  {t('start.drop.local')}
                </span>
                <span className="drop-foot-item">
                  <Keys keys={[mod, 'O']} label={`${mod}+O`} />
                  {t('start.drop.shortcut', { key: '' }).trim()}
                </span>
              </p>
            </div>
          )}
        </PdfDropZone>
        <AriaButton className="link start-sample" onPress={() => dispatch({ type: 'openSpec', spec: contractDoc('Vertrag.pdf', `doc-sample-${Date.now()}`) })}>
          {t('start.sample')}
          <IconChevronRight size={15} />
        </AriaButton>
      </div>

      <aside className="start-side" aria-label={t('start.recent.title')}>
        <section className="side-card">
          <h2 className="side-title">{t('start.recent.title')}</h2>
          {state.recent.length === 0 ? (
            <p className="muted">{t('start.recent.empty')}</p>
          ) : (
            <ul className="recent-list">
              {[...state.recent].sort((a, b) => b.signedAt.localeCompare(a.signedAt)).map((r) => (
                <li key={r.id}>
                  <AriaButton
                    className="recent-item"
                    aria-label={t('start.recent.verify', { name: r.fileName })}
                    onPress={() => {
                      const spec = verifySpecForRecent(r.id);
                      dispatch({ type: 'openVerify', spec: { ...spec, fileName: r.fileName } });
                    }}
                  >
                    <span className="recent-icon" aria-hidden="true">
                      <IconFileText size={18} />
                    </span>
                    <span className="recent-text">
                      <span className="recent-name">{r.fileName}</span>
                      <span className="recent-meta">
                        {t('start.recent.meta', { time: humanDateTime(r.signedAt, t.lang), signer: t(`signer.${r.signer}`) })}
                      </span>
                    </span>
                    <IconChevronRight size={16} className="recent-caret" />
                  </AriaButton>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section className="side-card side-card-verify">
          <span className="side-icon" aria-hidden="true">
            <IconShieldCheck size={22} />
          </span>
          <h2 className="side-title">{t('start.verify.title')}</h2>
          <p className="muted">{t('start.verify.text')}</p>
          <Button variant="secondary" onPress={() => dispatch({ type: 'openVerify', spec: null })}>
            {t('start.verify.button')}
          </Button>
        </section>
      </aside>
    </div>
  );
}
