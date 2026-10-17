/**
 * Automatische Positionierung im Stapel (ADR 0004 Nachtrag 8): Ist die Einstellung „Automatische
 * Positionierung“ an, nennt die Übersicht das nur („Automatische Positionierung (laut Einstellung)“). Sonst
 * schaltet „Für diesen Stapel automatisch positionieren“ sie **nur für diesen Stapel** ein
 * (`set_batch_auto_position`) – die Einstellung bleibt unverändert, nach dem Lauf gilt sie wieder (Kern).
 *
 * Von Hand platzierte Einträge (`BatchView::manual_plans`) behalten ihren Plan; vor dem Einschalten fragt die
 * Übersicht, ob sie ebenfalls automatisch platziert werden sollen – „Behalten“ ist die Vorauswahl, Escape
 * ändert nichts. Was als „Bereit“ gilt und welche Pläne bleiben, entscheidet der Kern; nichts davon startet
 * das Signieren (F-54: erst „Alle signieren“).
 */
import { useState } from 'react';
import { useApp } from '../../app/context';
import type { BatchView } from '../../core/types';
import { Button, SwitchField } from '../../ui/controls';
import { ConfirmDialog } from '../../ui/Dialog';
import { IconLock, IconTarget } from '../../ui/Icons';

export function BatchAutoPosition({ batch }: { batch: BatchView }) {
  const { t, dispatch } = useApp();
  const [asking, setAsking] = useState(false);
  if (batch.auto_position_setting) {
    return (
      <p className="batch-fact" data-testid="batch-auto-setting">
        <IconLock size={15} />
        <span>{t.t('ng-ui-batch-auto-setting')}</span>
      </p>
    );
  }
  const set = (enabled: boolean, replace = false) => {
    setAsking(false);
    void dispatch({ type: 'set_batch_auto_position', enabled, replace_manual: replace });
  };
  const onChange = (on: boolean) => {
    if (on && batch.manual_plans > 0) setAsking(true);
    else set(on);
  };
  return (
    <div className="batch-auto" data-testid="batch-auto">
      <SwitchField label={t.t('ng-ui-batch-auto')} hint={t.t('ng-ui-batch-auto-hint')} isSelected={batch.auto_position_once} isDisabled={batch.running} onChange={onChange} />
      {asking ? (
        <ConfirmDialog
          title={t.t('ng-ui-batch-auto-confirm-title', { n: batch.manual_plans })}
          icon={<IconTarget size={20} />}
          onCancel={() => setAsking(false)}
          actions={
            <>
              <Button variant="secondary" onPress={() => set(true, true)}>
                {t.t('ng-ui-batch-auto-replace', { n: batch.manual_plans })}
              </Button>
              <Button variant="primary" onPress={() => set(true)} autoFocus>
                {t.t('ng-ui-batch-auto-keep')}
              </Button>
            </>
          }
        >
          <p>{t.t('ng-ui-batch-auto-confirm-text', { n: batch.manual_plans })}</p>
        </ConfirmDialog>
      ) : null}
    </div>
  );
}
