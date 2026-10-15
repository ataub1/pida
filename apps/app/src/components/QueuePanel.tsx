/**
 * Warteschlangen-Panel (F-14, F-88): aktuelles Dokument und wartende Dokumente – nur Anzeige.
 */
import { useT } from '../app/context';
import type { QueueItemView } from '../core/types';
import { IconDocument } from './icons';

/** Warteschlange; leer → nichts. */
export function QueuePanel({ queue }: { queue: QueueItemView[] }) {
  const t = useT();
  if (queue.length === 0) return null;
  return (
    <aside className="queue" aria-labelledby="queue-title">
      <h2 id="queue-title" className="queue-title">
        {t.t('ng-ui-queue-title', { count: queue.length })}
      </h2>
      <ol className="queue-list">
        {queue.map((item) => (
          <li key={item.id} className="queue-item" data-current={item.current || undefined} aria-current={item.current ? 'true' : undefined}>
            <IconDocument size={18} />
            <span className="queue-name" title={item.name}>
              {item.name}
            </span>
            {item.current ? <span className="badge">{t.t('ng-ui-queue-current')}</span> : null}
          </li>
        ))}
      </ol>
    </aside>
  );
}
