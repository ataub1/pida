/**
 * „Zuletzt signiert“ auf der Startansicht (ADR 0004 §2.1, §3.10; Gestalt aus dem Prototyp): höchstens zehn
 * Einträge aus `ViewState::recent` (neueste zuerst, vom Kern), je Eintrag Name und Zeitpunkt – kein
 * Signiermittel, kein Unterzeichner (Datenschutz). Ein Eintrag öffnet den Prüfmodus (`verify_recent`, der
 * Kern liefert danach `ViewState::verify`); fehlt die Datei, meldet das der Kern (`ng-recent-missing`), der
 * Eintrag bleibt – Entfernen ist eine ausdrückliche Aktion (`remove_recent`). `recent = null` (abgeschaltet
 * bzw. per Richtlinie aus) blendet die Karte aus.
 */
import { Button as AriaButton } from 'react-aria-components';
import { useApp } from '../../app/context';
import type { RecentItemView } from '../../core/types';
import { IconButton } from '../../ui/controls';
import { IconChevronRight, IconClose, IconFileText } from '../../ui/Icons';
import './recent.css';

/** Props der Liste „Zuletzt signiert“ (in E0 festgelegt). */
export interface RecentPanelProps {
  /** `ViewState::recent` (`null` = abgeschaltet) */
  items: RecentItemView[] | null;
}

/** Höchstzahl der Einträge (der Kern speichert höchstens zehn, ADR 0004 §3.10). */
export const RECENT_MAX = 10;

function formatTime(iso: string, lang: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return new Intl.DateTimeFormat(lang === 'de' ? 'de-AT' : 'en-GB', { dateStyle: 'medium', timeStyle: 'short' }).format(d);
}

export function RecentPanel({ items }: RecentPanelProps) {
  const { t, dispatch } = useApp();
  if (items === null) return null;
  // Prüfmodus öffnen; eine fehlende Datei meldet der Kern selbst (`ng-recent-missing`)
  const open = (item: RecentItemView) => void dispatch({ type: 'verify_recent', id: item.id });
  return (
    <section className="side-card recent-card" aria-labelledby="recent-title" data-testid="recent-panel">
      <h2 className="side-title" id="recent-title">
        {t.t('ng-ui-recent-title')}
      </h2>
      {items.length === 0 ? (
        <p className="muted">{t.t('ng-ui-recent-empty')}</p>
      ) : (
        <ul className="recent-list">
          {items.slice(0, RECENT_MAX).map((item) => {
            const metaId = `recent-meta-${item.id}`;
            return (
              <li key={item.id} className={`recent-entry${item.exists ? '' : ' is-missing'}`}>
                <AriaButton
                  className="recent-item"
                  aria-label={t.t('ng-ui-recent-open', { name: item.file_name })}
                  aria-describedby={metaId}
                  onPress={() => open(item)}
                >
                  <span className="recent-icon" aria-hidden="true">
                    <IconFileText size={18} />
                  </span>
                  <span className="recent-text" title={item.folder_display ?? undefined}>
                    <span className="recent-name">{item.file_name}</span>
                    <span className="recent-meta" id={metaId}>
                      {formatTime(item.signed_at, t.lang)}
                      {item.exists ? null : <span className="recent-missing"> · {t.t('ng-ui-recent-missing')}</span>}
                    </span>
                  </span>
                  <IconChevronRight size={16} className="recent-caret" />
                </AriaButton>
                <IconButton label={t.t('ng-ui-recent-remove', { name: item.file_name })} size="sm" className="recent-remove" onPress={() => void dispatch({ type: 'remove_recent', id: item.id })}>
                  <IconClose size={14} />
                </IconButton>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
