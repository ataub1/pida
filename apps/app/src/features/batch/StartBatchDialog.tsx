/**
 * Start des Stapels (ADR 0004 §3.8 „Kein Dialog mitten im Lauf“): Speicherziel und Umgang mit vorhandenen
 * Dateien werden **vor** dem Start festgelegt. Erscheint nur, wenn es etwas zu entscheiden gibt – der Kern
 * verlangt einen Ordner (`output.needs_folder`: „jedes Mal fragen“ bzw. ein Dateipfad als Ausgabeordner) oder
 * Zieldateien gibt es schon (`target_exists`). Standard „Beide behalten“ (§8 F11); „Überspringen“ fordert für
 * diese Dokumente keine Bestätigung an. Dazu Schätzung und Kurzhinweis Datenschutz (V0). Erst „Signieren
 * starten“ sendet `start_batch` (ausdrückliche Aktion, A1).
 */
import { useState } from 'react';
import { useApp } from '../../app/context';
import type { BackendId, BatchView, ExistingPolicy } from '../../core/types';
import { Button, RadioCards } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconClock, IconFolder } from '../../ui/Icons';
import { PrivacyNote } from '../settings';
import { signerName, SignerGlyph } from '../workspace/signers';
import { batchFlow, skipDescMsg } from './flow';
import { estimateText, isSignable } from './texts';

/** „Vertrag_signed.pdf“ → „Vertrag_signed (2).pdf“ (nur Beispieltext; die Nummer vergibt der Kern). */
function numbered(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? `${name.slice(0, dot)} (2)${name.slice(dot)}` : `${name} (2)`;
}

export function StartBatchDialog({ batch, backend, onClose }: { batch: BatchView; backend: BackendId; onClose: () => void }) {
  const { t, view, dispatch, bridge, toast } = useApp();
  const signable = batch.items.filter(isSignable);
  const existing = signable.filter((i) => i.target_exists);
  const needsFolder = batch.output.needs_folder;
  const [folder, setFolder] = useState<string | null>(null);
  const [policy, setPolicy] = useState<ExistingPolicy>('keep_both');
  const showPolicy = existing.length > 0 || needsFolder;
  const example = numbered(existing[0]?.target_name ?? signable[0]?.target_name ?? 'Vertrag_signed.pdf');
  const option = view.signers.options.find((o) => o.backend === backend);
  const leaves = option?.capabilities?.document_leaves_device === true;
  const estimate = batch.estimate && batch.estimate.backend === backend ? batch.estimate : null;

  const pick = async () => {
    try {
      const dir = await bridge.pickDirectory({ title: t.t('ng-ui-batch-start-folder-dialog'), defaultPath: folder });
      if (dir) setFolder(dir);
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };
  const go = () => {
    void dispatch({ type: 'start_batch', backend, docs: signable.map((i) => i.doc), output_dir: needsFolder ? folder : null, on_existing: policy });
    onClose();
  };

  return (
    <ModalShell onClose={onClose} size="md" className="batch-start-modal">
      <DialogHeader title={t.t('ng-ui-batch-start-title', { n: signable.length })} subtitle={signerName(t, backend)} onClose={onClose} icon={<SignerGlyph backend={backend} />} />
      <div className="batch-start-body">
        {estimate ? (
          <p className="batch-fact">
            <IconClock size={15} />
            <span>
              <span className="batch-fact-label">{t.t('ng-ui-batch-estimate')}: </span>
              {estimateText(t, estimate)}
            </span>
          </p>
        ) : null}
        {needsFolder ? (
          <section className="batch-start-section" aria-labelledby="batch-start-folder">
            <h3 className="batch-start-heading" id="batch-start-folder">
              {t.t('ng-ui-batch-start-folder')}
            </h3>
            <p className={`batch-start-folder${folder ? '' : ' is-empty'}`} data-testid="batch-start-folder">
              {folder ?? t.t('ng-ui-batch-start-folder-none')}
            </p>
            <div>
              <Button variant="secondary" size="sm" onPress={() => void pick()}>
                <IconFolder size={15} />
                {t.t(folder ? 'ng-ui-batch-start-folder-change' : 'ng-ui-batch-start-folder-pick')}
              </Button>
            </div>
          </section>
        ) : null}
        {showPolicy ? (
          <section className="batch-start-section">
            {existing.length > 0 ? <p className="batch-start-text">{t.t('ng-ui-batch-start-existing-count', { n: existing.length })}</p> : null}
            <RadioCards
              label={t.t('ng-ui-batch-start-existing')}
              value={policy}
              onChange={setPolicy}
              options={[
                { value: 'keep_both', label: t.t('ng-ui-batch-start-keep'), description: t.t('ng-ui-batch-start-keep-desc', { example }) },
                { value: 'overwrite', label: t.t('ng-ui-batch-start-overwrite'), description: t.t('ng-ui-batch-start-overwrite-desc') },
                // „ohne Bestätigung in der App“ nur bei ID Austria (Kern: `batch_flow` des gewählten Signiermittels)
                { value: 'skip', label: t.t('ng-ui-batch-start-skip'), description: t.t(skipDescMsg(batchFlow(view, backend))) },
              ]}
            />
          </section>
        ) : null}
        {leaves ? <PrivacyNote /> : null}
      </div>
      <div className="dialog-actions batch-start-actions">
        <Button variant="ghost" onPress={onClose}>
          {t.t('ng-ui-sc-cancel')}
        </Button>
        <Button variant="primary" onPress={go} isDisabled={signable.length === 0 || (needsFolder && !folder)}>
          {t.t('ng-ui-batch-start-go')}
        </Button>
      </div>
    </ModalShell>
  );
}
