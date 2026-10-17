/**
 * Start/Leerzustand (Gestalt aus dem Prototyp, ADR 0004 §2.1): große Ablagefläche, „PDF auswählen“,
 * daneben der Slot „Zuletzt signiert“ (Paket UI-B, `RecentPanel`) und der Hinweis auf den Prüfmodus (erst,
 * wenn der Kern ihn freigibt). Die Ablagefläche ist nur optisch: Dateien kommen über die
 * Fenster-Ereignisse der Shell (Pfade, `bridge.onFileDrag`); hervorgehoben wird sie beim Ziehen
 * (`dragActive`). Die Oberfläche filtert nicht nach Endung – der Kern meldet Nicht-PDFs.
 *
 * Mobile: keine Ablagefläche und keine Tastenkürzel, Knopf „Dokument auswählen“ (Dokument-Picker bzw.
 * SAF) und der Hinweis auf „Teilen“/„Öffnen in“.
 *
 * Unter dem Knopf steht je Plattform, wie die Dateiauswahl mehrere PDFs auf einmal übernimmt (ADR 0004 Nachtrag 17).
 *
 * Geparkte Arbeit („Zur Startseite“, ADR 0004 Nachtrag 10): oben die Karte „In Bearbeitung“ (`ParkedCard`); neue
 * PDFs fragen dann vorher, ob sie zum Stapel kommen (`ParkedJoinDialog`, Shell).
 */
import { useApp } from '../../app/context';
import { isMobile } from '../../core/types';
import { AccessButton, Button, Keys, useModKey } from '../../ui/controls';
import { IconLock, IconShieldCheck, IconUpload } from '../../ui/Icons';
import { RECENT_AVAILABLE, RecentPanel } from '../recent';
import { ParkedCard } from './ParkedCard';
import './start.css';

export function StartView({ onVerify }: { onVerify: () => void }) {
  const { t, view, dragActive, browse } = useApp();
  const { mod } = useModKey();
  const mobile = isMobile(view);
  const showRecent = RECENT_AVAILABLE && view.recent !== null;
  const showVerify = view.actions.verify;
  const single = !showRecent && !showVerify;
  return (
    <div className={`start-view${single ? ' is-single' : ''}`}>
      <div className="start-main">
        <h1 className="view-title start-title" id="main-heading" tabIndex={-1} data-main-heading="">
          {t.t('ng-ui-start-title')}
        </h1>
        <p className="view-lead">{t.t('ng-ui-start-lead')}</p>
        {view.parked ? <ParkedCard parked={view.parked} /> : null}
        <section className="dropzone start-drop" aria-label={t.t('ng-ui-start-drop-label')} data-drop-target={dragActive || undefined}>
          <div className={`drop-inner${dragActive ? ' is-target' : ''}`}>
            <span className="drop-icon" aria-hidden="true">
              <IconUpload size={30} />
            </span>
            {mobile ? (
              <p className="drop-title">{t.t('ng-ui-start-drop-title-mobile')}</p>
            ) : (
              <p className="drop-title" aria-live="polite">
                {dragActive ? t.t('ng-ui-start-drop-release') : t.t('ng-ui-start-drop-title')}
              </p>
            )}
            <p className="drop-hint">{t.t(mobile ? 'ng-ui-start-drop-hint-mobile' : 'ng-ui-start-drop-hint')}</p>
            <AccessButton msg={mobile ? 'ng-ui-start-pick-mobile' : 'ng-ui-start-pick'} variant="primary" size="lg" onPress={() => void browse()} isDisabled={!view.actions.open_files} />
            {/* wie die System-Dateiauswahl mehrere PDFs übernimmt (macOS: Doppelklick nimmt nur eines; Android: Antippen
                nimmt sofort eines; ADR 0004 Nachtrag 17) */}
            <p className="drop-multi">{t.t('ng-ui-start-multi-hint', { os: view.platform.os })}</p>
            <p className="drop-foot">
              <span className="drop-foot-item">
                <IconLock size={14} />
                {t.t('ng-ui-start-local')}
              </span>
              {mobile ? null : (
                <span className="drop-foot-item">
                  <Keys keys={[mod, 'O']} label={`${mod}+O`} />
                  {t.t('ng-ui-start-shortcut')}
                </span>
              )}
            </p>
          </div>
        </section>
      </div>

      {single ? null : (
        <aside className="start-side" aria-label={t.t('ng-ui-start-side')}>
          {showRecent ? <RecentPanel items={view.recent} /> : null}
          {showVerify ? (
            <section className="side-card side-card-verify">
              <span className="side-icon" aria-hidden="true">
                <IconShieldCheck size={22} />
              </span>
              <h2 className="side-title">{t.t('ng-ui-start-verify-title')}</h2>
              <p className="muted">{t.t('ng-ui-start-verify-text')}</p>
              <Button variant="secondary" onPress={onVerify}>
                {t.t('ng-ui-start-verify-button')}
              </Button>
            </section>
          ) : null}
        </aside>
      )}
    </div>
  );
}
