/**
 * Seitenminiaturen (Gestalt aus dem Prototyp). Kacheln des Kerns mit kleinem Maßstab, geladen nur in der
 * Nähe der sichtbaren Miniaturen; Klick springt zur Seite, die Seite mit dem Signaturblock ist markiert.
 */
import { useEffect, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { useApp } from '../../app/context';
import { tileScale } from '../../core/bridge';
import type { DocumentView } from '../../core/types';
import { IconSignature } from '../../ui/Icons';
import { rectToBox } from './placement';

const THUMB_W = 104;

/** Miniaturen in Sichtweite (IntersectionObserver; ohne ihn – jsdom – die ersten acht). */
function useNearThumbs(root: HTMLElement | null, count: number): Set<number> {
  const [near, setNear] = useState<Set<number>>(() => new Set(Array.from({ length: Math.min(count, 8) }, (_, i) => i)));
  useEffect(() => {
    if (!root || typeof IntersectionObserver === 'undefined') return undefined;
    const io = new IntersectionObserver(
      (entries) => {
        setNear((prev) => {
          const next = new Set(prev);
          for (const e of entries) {
            const i = Number((e.target as HTMLElement).dataset.thumb);
            if (e.isIntersecting) next.add(i);
          }
          return next;
        });
      },
      { root, rootMargin: '300px 0px' },
    );
    root.querySelectorAll('[data-thumb]').forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, [root, count]);
  return near;
}

export function Thumbnails({ doc, sizes, current, onSelect }: { doc: DocumentView; sizes: [number, number][]; current: number; onSelect: (i: number) => void }) {
  const { t, bridge } = useApp();
  const ref = useRef<HTMLElement>(null);
  const [root, setRoot] = useState<HTMLElement | null>(null);
  useEffect(() => setRoot(ref.current), []);
  const near = useNearThumbs(root, sizes.length);
  const stickerPage = doc.invisible ? -1 : doc.placement.page_index;
  return (
    <nav className="thumbs" aria-label={t.t('ng-ui-ws-pages')} ref={ref}>
      <ol className="thumbs-list">
        {sizes.map(([w, h], i) => {
          const scale = THUMB_W / w;
          const hasSticker = stickerPage === i && doc.sticker !== null;
          const isNew = i >= doc.doc.page_count;
          const box = hasSticker && doc.sticker ? rectToBox(doc.sticker, h, scale) : null;
          return (
            <li key={i} data-thumb={i}>
              <Button
                className={`thumb${current === i ? ' is-current' : ''}`}
                onPress={() => onSelect(i)}
                aria-current={current === i ? 'page' : undefined}
                aria-label={hasSticker ? t.t('ng-ui-ws-thumb-sticker', { n: i + 1 }) : t.t('ng-ui-ws-thumb', { n: i + 1 })}
              >
                <span className="thumb-page" style={{ width: THUMB_W, height: h * scale }}>
                  {isNew ? <span className="thumb-new" /> : near.has(i) ? <img className="thumb-img" src={bridge.pageTileUrl(doc.doc.id, i, tileScale(scale * 2))} alt="" draggable={false} /> : null}
                  {box ? <span className="thumb-sticker" style={{ left: box.left, top: box.top, width: box.width, height: box.height }} aria-hidden="true" /> : null}
                </span>
                <span className="thumb-label" aria-hidden="true">
                  {hasSticker ? <IconSignature size={13} /> : null}
                  {i + 1}
                </span>
              </Button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
