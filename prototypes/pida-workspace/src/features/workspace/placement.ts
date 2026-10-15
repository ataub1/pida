/** Abgeleitete Werte der Platzierung (Größe, Überdeckung, Ansagetext) – geteilt von Zeichenfläche und Inspektor. */
import { blockSize } from '../../model/format';
import { clampToPage, describeZone, overlaps, ptToMm, type SnapKind } from '../../model/geometry';
import type { DocState, PageSpec, Placement, Rect, SigBlockSettings } from '../../model/types';
import type { T } from '../../i18n';

export function stickerRect(doc: DocState, placement: Placement, pages: PageSpec[], sig: SigBlockSettings): Rect {
  const field = placement.fieldId ? doc.spec.fields.find((f) => f.id === placement.fieldId) : undefined;
  if (field) return { ...field.rect };
  const size = blockSize(sig);
  const page = pages[placement.page] ?? pages[pages.length - 1];
  const pos = page ? clampToPage(placement.x, placement.y, size, page) : { x: placement.x, y: placement.y };
  return { x: pos.x, y: pos.y, w: size.w, h: size.h };
}

export function overlapCount(rect: Rect, page: PageSpec | undefined): number {
  return page ? overlaps(rect, page).length : 0;
}

export function zoneLabel(t: T, rect: Rect, page: PageSpec): string {
  const z = describeZone(rect, page);
  return t(`zone.${z.v}.${z.h}` as Parameters<T>[0]);
}

/** Satz für die Live-Region, z. B. „Seite 3 von 3, 112 mm von links, 38 mm von unten. Rastet ein: rechts unten.“ */
export function describePlacement(
  t: T,
  doc: DocState,
  placement: Placement,
  pages: PageSpec[],
  sig: SigBlockSettings,
  snapped: SnapKind[] = [],
): string {
  const page = pages[placement.page];
  if (!page) return '';
  const rect = stickerRect(doc, placement, pages, sig);
  const parts = [
    t('ws.announce', {
      page: t('common.pageOf', { n: placement.page + 1, total: pages.length }),
      left: ptToMm(rect.x),
      bottom: ptToMm(page.h - rect.y - rect.h),
    }),
  ];
  const field = placement.fieldId ? doc.spec.fields.find((f) => f.id === placement.fieldId) : undefined;
  if (field) parts.push(t('ws.announce.field', { name: field.label }));
  if (snapped.length) {
    const corner = snapped.includes('right') && snapped.includes('bottom');
    const where = corner ? t('snap.corner') : snapped.map((s) => t(`snap.${s}` as Parameters<T>[0])).join(', ');
    parts.push(t('ws.announce.snap', { where }));
  }
  if (!field) parts.push(overlapCount(rect, page) > 0 ? t('ws.announce.overlap') : t('ws.announce.free'));
  return parts.join(' ');
}
