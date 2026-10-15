/** Seitenminiaturen (links). Klick springt zur Seite; die Seite mit dem Signaturblock ist markiert. */
import { Button } from 'react-aria-components';
import { useT } from '../../i18n';
import type { DocState, PageSpec, Rect } from '../../model/types';
import { IconSignature } from '../../ui/Icons';
import { MockPage } from './MockPage';

const THUMB_W = 104;

export function Thumbnails({
  doc,
  pages,
  current,
  onSelect,
  sticker,
}: {
  doc: DocState;
  pages: PageSpec[];
  current: number;
  sticker: Rect | null;
  onSelect: (i: number) => void;
}) {
  const t = useT();
  return (
    <nav className="thumbs" aria-label={t('ws.pages')}>
      <ol className="thumbs-list">
        {pages.map((p, i) => {
          const scale = THUMB_W / p.w;
          const hasSticker = doc.placement?.page === i && !doc.invisible;
          const isNew = i >= doc.spec.pages.length;
          return (
            <li key={i}>
              <Button
                className={`thumb${current === i ? ' is-current' : ''}`}
                onPress={() => onSelect(i)}
                aria-current={current === i ? 'page' : undefined}
                aria-label={hasSticker ? t('ws.thumb.sticker', { n: i + 1 }) : t('ws.thumb', { n: i + 1 })}
              >
                <span className="thumb-page" style={{ width: THUMB_W, height: p.h * scale }}>
                  {isNew ? <span className="thumb-new" /> : <MockPage page={p} scale={scale} signatures={doc.spec.signatures} />}
                  {hasSticker && sticker ? (
                    <span
                      className="thumb-sticker"
                      style={{ left: sticker.x * scale, top: sticker.y * scale, width: sticker.w * scale, height: sticker.h * scale }}
                      aria-hidden="true"
                    />
                  ) : null}
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
