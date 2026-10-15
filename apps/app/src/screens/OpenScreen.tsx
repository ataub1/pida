/**
 * Dokumentauswahl (docs/analyse/01 §4.1, F-10/F-11/F-14): Drop-Zone mit Hervorhebung beim Ziehen,
 * Dateidialog mit Mehrfachauswahl, Warteschlange. Mobile: kein Ziehen und keine Tastenkürzel –
 * Dateiauswahl des Systems und der Hinweis auf „Teilen“/„Öffnen in“ (Plan §5.1).
 */
import { useApp } from '../app/context';
import { DonePanel } from '../components/DonePanel';
import { IconUpload } from '../components/icons';
import { QueuePanel } from '../components/QueuePanel';
import { AccessButton, ScreenHeading, Spinner } from '../components/ui';
import { isMobile, type QueueItemView } from '../core/types';

export function OpenScreen({ queue, onBrowse }: { queue: QueueItemView[]; onBrowse: () => void }) {
  const { t, dragActive, view } = useApp();
  const analyzing = queue.some((q) => q.current);
  const mobile = isMobile(view);
  return (
    <div className="screen screen-open">
      <ScreenHeading>{t.t('ng-ui-open-title')}</ScreenHeading>
      <section
        className="dropzone"
        data-drop-target={dragActive || undefined}
        aria-label={t.t('ng-ui-dropzone-label')}
        aria-describedby="dropzone-hint"
      >
        <IconUpload size={44} className="dropzone-icon" />
        {mobile ? null : (
          <>
            <p className="dropzone-title">{t.t('dataSourceSelection-DropLabel')}</p>
            <p className="dropzone-or">{t.t('dataSourceSelection-DropLabel2')}</p>
          </>
        )}
        {/* Mobile: „… auf Ihrem Computer“ passt nicht – eigener Text für die Dateiauswahl des Systems */}
        <AccessButton msg={mobile ? 'ng-ui-browse-mobile' : 'dataSourceSelection-browse'} variant="primary" onPress={onBrowse} />
        <p id="dropzone-hint" className="hint">
          {t.t(mobile ? 'ng-ui-dropzone-hint-mobile' : 'ng-ui-dropzone-hint')}
        </p>
        {dragActive ? (
          <p className="dropzone-active" aria-live="polite">
            {t.t('ng-ui-dropzone-release')}
          </p>
        ) : null}
      </section>
      {analyzing ? (
        <p className="status-line" role="status">
          <Spinner size={18} /> {t.t('ng-ui-open-analyzing')}
        </p>
      ) : null}
      <QueuePanel queue={queue} />
      <DonePanel />
    </div>
  );
}
