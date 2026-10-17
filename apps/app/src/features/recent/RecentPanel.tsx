/**
 * „Zuletzt signiert“ auf der Startansicht (ADR 0004 §2.1, §3.10; Gestalt aus dem Prototyp): höchstens zehn
 * Einträge aus `ViewState::recent` (neueste zuerst, vom Kern), je Eintrag Name und Zeitpunkt – kein
 * Signiermittel, kein Unterzeichner (Datenschutz). `recent = null` (abgeschaltet bzw. per Richtlinie aus) blendet
 * die Karte aus.
 *
 * Ein Eintrag bietet, was „Fertig“ kann (ADR 0004 Nachtrag 15; einen Prüfmodus gibt es nicht mehr): der Eintrag
 * selbst und „Öffnen“ öffnen die Datei mit der Standardanwendung bzw. auf Mobilgeräten als Vorschau (`open_recent`),
 * dazu im Menü „Im Ordner zeigen“ (Desktop, `reveal_recent`) bzw. „Teilen“ (Mobile, `share_recent`) und „Erneut
 * signieren“ – wie „Öffnen“ in den Arbeitsbereich (`sign_recent`, bei geparkter Arbeit mit der Rückfrage der Shell).
 * Den Pfad kennt nur der Kern; fehlt die Datei, meldet er das (`ng-recent-missing`), der Eintrag bleibt – Entfernen
 * ist eine ausdrückliche Aktion (`remove_recent`).
 */
import { Button as AriaButton, Menu, MenuItem, MenuTrigger, Popover, type Key } from 'react-aria-components';
import { useApp } from '../../app/context';
import { isMobile, type RecentItemView } from '../../core/types';
import { IconButton } from '../../ui/controls';
import { IconClose, IconFileText, IconFolder, IconMore, IconShare, IconSignature } from '../../ui/Icons';
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
  const { t, view, dispatch, signRecent } = useApp();
  if (items === null) return null;
  const mobile = isMobile(view);
  // eine fehlende Datei meldet der Kern selbst (`ng-recent-missing`)
  const open = (item: RecentItemView) => void dispatch({ type: 'open_recent', id: item.id });
  const onAction = (item: RecentItemView, key: Key) => {
    if (key === 'open') open(item);
    else if (key === 'reveal') void dispatch({ type: 'reveal_recent', id: item.id });
    else if (key === 'share') void dispatch({ type: 'share_recent', id: item.id });
    else if (key === 'sign') signRecent(item.id);
  };
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
            const more = t.t('ng-ui-recent-more', { name: item.file_name });
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
                </AriaButton>
                <MenuTrigger>
                  <IconButton label={more} size="sm" className="recent-more">
                    <IconMore size={16} />
                  </IconButton>
                  <Popover className="popover" placement="bottom end" offset={4}>
                    <Menu className="menu recent-menu" aria-label={more} onAction={(key) => onAction(item, key)} disabledKeys={view.actions.open_files ? [] : ['sign']}>
                      <MenuItem id="open" className="menu-item" textValue={t.t('ng-ui-fin-open')}>
                        <span className="menu-item-icon" aria-hidden="true">
                          <IconFileText size={18} />
                        </span>
                        <span className="menu-item-label">{t.t('ng-ui-fin-open')}</span>
                      </MenuItem>
                      {mobile ? (
                        <MenuItem id="share" className="menu-item" textValue={t.t('ng-ui-fin-share')}>
                          <span className="menu-item-icon" aria-hidden="true">
                            <IconShare size={18} />
                          </span>
                          <span className="menu-item-label">{t.t('ng-ui-fin-share')}</span>
                        </MenuItem>
                      ) : (
                        <MenuItem id="reveal" className="menu-item" textValue={t.t('ng-ui-fin-reveal')}>
                          <span className="menu-item-icon" aria-hidden="true">
                            <IconFolder size={18} />
                          </span>
                          <span className="menu-item-label">{t.t('ng-ui-fin-reveal')}</span>
                        </MenuItem>
                      )}
                      <MenuItem id="sign" className="menu-item" textValue={t.t('ng-ui-recent-sign')}>
                        <span className="menu-item-icon" aria-hidden="true">
                          <IconSignature size={18} />
                        </span>
                        <span className="menu-item-label">{t.t('ng-ui-recent-sign')}</span>
                      </MenuItem>
                    </Menu>
                  </Popover>
                </MenuTrigger>
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
