/**
 * Hinweis zum Teilen auf iOS (ADR 0004 Nachtrag 12, `ViewState::share_hint`): Teilt man mehrere PDFs aus der
 * Dateien-App, übergibt iOS nur das erste (keine Share-Extension, Entscheidung des Auftraggebers: „Hinweis reicht“).
 * Kurz und nicht blockierend über „Signieren“ (auch im eingeklappten Bottom-Sheet) mit „Weitere PDFs hinzufügen …“
 * (Dateiauswahl mit Mehrfachauswahl, `open_files { mode: 'add' }`) und „Hinweis ausblenden“ (`dismiss_share_hint`,
 * bis zum Beenden der App). Wann er erscheint – nur nach dem Empfang über „Teilen“ bzw. „Öffnen in“ und nur, solange
 * das empfangene Dokument allein offen ist –, entscheidet der Kern.
 */
import { useApp } from '../../app/context';
import { Button, IconButton } from '../../ui/controls';
import { IconClose, IconFilePlus, IconInfo } from '../../ui/Icons';

export function ShareHint() {
  const { t, view, dispatch, browse } = useApp();
  if (!view.share_hint) return null;
  return (
    <div className="callout callout-info share-hint" role="note" data-testid="share-hint">
      <IconInfo size={18} />
      <div>
        <p>{t.t('ng-ui-ws-share-hint')}</p>
        <Button variant="secondary" size="sm" onPress={() => void browse('add')} isDisabled={!view.actions.open_files}>
          <IconFilePlus size={15} />
          {t.t('ng-ui-shell-add-more')}
        </Button>
      </div>
      <IconButton className="share-hint-close" size="sm" label={t.t('ng-ui-ws-share-hint-dismiss')} tooltip={false} onPress={() => void dispatch({ type: 'dismiss_share_hint' })}>
        <IconClose size={16} />
      </IconButton>
    </div>
  );
}
