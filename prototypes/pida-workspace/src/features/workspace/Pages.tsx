/**
 * Durchgehend scrollbare Seitenansicht mit Signatur-„Sticker“: Ziehen (Maus/Touch/Stift), Einrasten an
 * Rändern/Mitte/unter dem Text, dezente Hervorhebung freier Flächen, Warnung bei Überdeckung,
 * „Hier signieren“-Markierungen für leere Signaturfelder und QR-Platzhalter, Tastaturbedienung.
 */
import { useId, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent, type RefObject } from 'react';
import { Button } from 'react-aria-components';
import { useT } from '../../i18n';
import { blockSize } from '../../model/format';
import { clampToPage, freeBands, snap, type Guide, type SnapKind } from '../../model/geometry';
import type { DocState, PageSpec, Placement, SigBlockSettings } from '../../model/types';
import { IconAlert, IconCheck, IconGrip, IconPen, IconQr } from '../../ui/Icons';
import { useModKey } from '../../ui/controls';
import { SignatureBlock } from '../sigblock/SignatureBlock';
import { MockPage } from './MockPage';
import { describePlacement, overlapCount, stickerRect } from './placement';

export const PAGE_PAD = 24;
export const PAGE_GAP = 28;

export interface PageLayout {
  tops: number[];
  lefts: number[];
  width: number;
  height: number;
}

export function layoutPages(pages: PageSpec[], scale: number): PageLayout {
  const maxW = Math.max(...pages.map((p) => p.w));
  const tops: number[] = [];
  const lefts: number[] = [];
  let y = PAGE_PAD;
  for (const p of pages) {
    tops.push(y);
    lefts.push(((maxW - p.w) * scale) / 2);
    y += p.h * scale + PAGE_GAP;
  }
  return { tops, lefts, width: maxW * scale, height: y - PAGE_GAP + PAGE_PAD };
}

interface Draft {
  placement: Placement;
  guides: Guide[];
  snapped: SnapKind[];
}

export function Pages({
  doc,
  pages,
  scale,
  sig,
  signer,
  signTime,
  onPlace,
  onAnnounce,
  onInteract,
  coachmark,
  hostRef,
  stickerRef,
  onRequestScroll,
}: {
  doc: DocState;
  pages: PageSpec[];
  scale: number;
  sig: SigBlockSettings;
  signer: string;
  signTime: string;
  onPlace: (p: Placement, snapped: SnapKind[]) => void;
  onAnnounce: (text: string) => void;
  onInteract: () => void;
  coachmark: boolean;
  hostRef: RefObject<HTMLDivElement | null>;
  stickerRef: RefObject<HTMLDivElement | null>;
  onRequestScroll: () => void;
}) {
  const t = useT();
  const { mod, enter } = useModKey();
  const helpId = useId();
  const layout = layoutPages(pages, scale);
  const [draft, setDraft] = useState<Draft | null>(null);
  const draftRef = useRef<Draft | null>(null);
  const grab = useRef<{ dx: number; dy: number; sx: number; sy: number; moved: boolean } | null>(null);

  const placement = draft?.placement ?? doc.placement;
  const visible = !!placement && !doc.invisible;
  const rect = placement ? stickerRect(doc, placement, pages, sig) : null;
  const page = placement ? pages[placement.page] : undefined;
  const overlap = rect && page && !placement?.fieldId ? overlapCount(rect, page) > 0 : false;
  const inField = !!placement?.fieldId;

  const pageIndexAt = (py: number) => {
    for (let i = 0; i < pages.length; i++) {
      const p = pages[i] as PageSpec;
      if (py < (layout.tops[i] ?? 0) + p.h * scale + PAGE_GAP / 2) return i;
    }
    return pages.length - 1;
  };

  /** Lage aus Zeigerposition (relativ zur Seitenspalte) inkl. Einrasten bzw. Feld-Magnet. */
  const compute = (px: number, py: number, dx: number, dy: number): Draft => {
    const i = pageIndexAt(py);
    const p = pages[i] as PageSpec;
    const size = blockSize(sig);
    const x = (px - (layout.lefts[i] ?? 0)) / scale - dx;
    const y = (py - (layout.tops[i] ?? 0)) / scale - dy;
    const cx = x + size.w / 2;
    const cy = y + size.h / 2;
    const field = doc.spec.fields.find((f) => f.page === i && cx >= f.rect.x && cx <= f.rect.x + f.rect.w && cy >= f.rect.y && cy <= f.rect.y + f.rect.h);
    if (field) return { placement: { page: i, x: field.rect.x, y: field.rect.y, fieldId: field.id }, guides: [], snapped: [] };
    const s = snap(x, y, size, p, 7 / scale);
    return { placement: { page: i, x: s.x, y: s.y, fieldId: null }, guides: s.guides, snapped: s.snapped };
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || !rect) return;
    onInteract();
    const el = e.currentTarget;
    try {
      el.setPointerCapture?.(e.pointerId);
    } catch {
      /* synthetischer Zeiger (Automatisierung) – ohne Capture weiter */
    }
    const r = el.getBoundingClientRect();
    // Greifversatz bezogen auf die Blockgröße (aus einem Feld heraus wird wieder die normale Größe gezogen)
    const size = blockSize(sig);
    const fx = r.width > 0 ? (e.clientX - r.left) / r.width : 0.5;
    const fy = r.height > 0 ? (e.clientY - r.top) / r.height : 0.5;
    grab.current = { dx: fx * size.w, dy: fy * size.h, sx: e.clientX, sy: e.clientY, moved: false };
    el.focus({ preventScroll: true });
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const g = grab.current;
    if (!g || !hostRef.current) return;
    if (!g.moved && Math.hypot(e.clientX - g.sx, e.clientY - g.sy) < 3) return;
    g.moved = true;
    const host = hostRef.current.getBoundingClientRect();
    const next = compute(e.clientX - host.left, e.clientY - host.top, g.dx, g.dy);
    draftRef.current = next;
    setDraft(next);
  };

  const finish = (e: PointerEvent<HTMLDivElement>) => {
    const g = grab.current;
    grab.current = null;
    try {
      e.currentTarget.releasePointerCapture?.(e.pointerId);
    } catch {
      /* ignorieren */
    }
    const d = draftRef.current;
    draftRef.current = null;
    setDraft(null);
    if (g?.moved && d) {
      onPlace(d.placement, d.snapped);
      onAnnounce(describePlacement(t, doc, d.placement, pages, sig, d.snapped));
    }
  };

  /**
   * Doppelklick auf die Seite setzt den Block dorthin (Mitte am Zeiger) – angelehnt an PDF-Over (F-34),
   * aber als Doppelklick, damit ein einfacher Klick beim Lesen/Scrollen nichts verschiebt.
   */
  const onPageClick = (e: MouseEvent<HTMLDivElement>) => {
    if (!hostRef.current || doc.invisible) return;
    onInteract();
    const host = hostRef.current.getBoundingClientRect();
    const size = blockSize(sig);
    const d = compute(e.clientX - host.left, e.clientY - host.top, size.w / 2, size.h / 2);
    onPlace(d.placement, d.snapped);
    onAnnounce(describePlacement(t, doc, d.placement, pages, sig, d.snapped));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!placement || !rect) return;
    const step = e.shiftKey ? 1 : e.altKey ? 50 : 10;
    let { x, y, page: pi } = { x: rect.x, y: rect.y, page: placement.page };
    switch (e.key) {
      case 'ArrowLeft':
        x -= step;
        break;
      case 'ArrowRight':
        x += step;
        break;
      case 'ArrowUp':
        y -= step;
        break;
      case 'ArrowDown':
        y += step;
        break;
      case 'PageUp':
        pi = Math.max(0, pi - 1);
        break;
      case 'PageDown':
        pi = Math.min(pages.length - 1, pi + 1);
        break;
      default:
        return;
    }
    e.preventDefault();
    onInteract();
    const p = pages[pi] as PageSpec;
    const size = blockSize(sig);
    const c = clampToPage(x, y, size, p);
    const next: Placement = { page: pi, x: c.x, y: c.y, fieldId: null };
    // Ansage „rastet ein“ nur bei exakter Ausrichtung, Tastatur verschiebt immer präzise.
    const aligned = snap(c.x, c.y, size, p, 0.5).snapped;
    onPlace(next, aligned);
    onAnnounce(describePlacement(t, doc, next, pages, sig, aligned));
    if (pi !== placement.page) onRequestScroll();
  };

  const top = (i: number) => layout.tops[i] ?? 0;
  const left = (i: number) => layout.lefts[i] ?? 0;
  const dragging = !!draft;

  return (
    <div className={`pages${dragging ? ' is-dragging' : ''}`} ref={hostRef} style={{ width: layout.width, height: layout.height }}>
      {pages.map((p, i) => {
        const isNew = i >= doc.spec.pages.length;
        return (
          <div
            key={i}
            className={`page-frame${isNew ? ' is-new' : ''}${placement?.page === i ? ' has-sticker' : ''}`}
            style={{ top: top(i), left: left(i), width: p.w * scale, height: p.h * scale }}
            role="img"
            aria-label={isNew ? t('ws.newPage') : t('ws.pageAlt', { n: i + 1, total: pages.length })}
            data-page={i}
            onDoubleClick={onPageClick}
          >
            {isNew ? (
              <div className="new-page-label" aria-hidden="true">
                {t('ws.newPage')}
              </div>
            ) : (
              <MockPage page={p} scale={scale} signatures={doc.spec.signatures} />
            )}
          </div>
        );
      })}

      <div className="pages-overlay">
        {dragging
          ? pages.map((p, i) =>
              freeBands(p).map((b, bi) => (
                <div
                  key={`band-${i}-${bi}`}
                  className="free-band"
                  style={{ top: top(i) + b.y * scale, left: left(i) + b.x * scale, width: b.w * scale, height: b.h * scale }}
                  aria-hidden="true"
                />
              )),
            )
          : null}

        {draft && page
          ? draft.guides.map((g, gi) =>
              g.axis === 'x' ? (
                <div key={`g${gi}`} className="snap-guide snap-guide-v" style={{ left: left(draft.placement.page) + g.at * scale, top: top(draft.placement.page), height: page.h * scale }} aria-hidden="true" />
              ) : (
                <div key={`g${gi}`} className="snap-guide snap-guide-h" style={{ top: top(draft.placement.page) + g.at * scale, left: left(draft.placement.page), width: page.w * scale }} aria-hidden="true" />
              ),
            )
          : null}

        {doc.spec.fields.map((f) => {
          if (placement?.fieldId === f.id && visible) return null;
          return (
            <Button
              key={f.id}
              className={`field-marker field-marker-${f.kind}${dragging ? ' is-target' : ''}`}
              style={{ top: top(f.page) + f.rect.y * scale, left: left(f.page) + f.rect.x * scale, width: f.rect.w * scale, height: f.rect.h * scale }}
              aria-label={f.kind === 'qr' ? t('ws.marker.qr') : t('ws.marker.field', { name: f.label })}
              onPress={() => {
                onInteract();
                const next: Placement = { page: f.page, x: f.rect.x, y: f.rect.y, fieldId: f.id };
                onPlace(next, []);
                onAnnounce(describePlacement(t, doc, next, pages, sig));
              }}
            >
              <span className="field-marker-pill">
                {f.kind === 'qr' ? <IconQr size={16} /> : <IconPen size={16} />}
                {t('ws.marker')}
              </span>
            </Button>
          );
        })}

        {visible && placement && rect ? (
          <div
            ref={stickerRef}
            className={`sticker${dragging ? ' is-dragging' : ''}${overlap ? ' is-overlap' : ''}${inField ? ' is-in-field' : ''}`}
            style={{ top: top(placement.page) + rect.y * scale, left: left(placement.page) + rect.x * scale, width: rect.w * scale, height: rect.h * scale }}
            role="application"
            aria-roledescription={t('ws.sticker.role')}
            aria-label={t('ws.sticker.label')}
            aria-describedby={helpId}
            tabIndex={0}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={finish}
            onPointerCancel={finish}
            onKeyDown={onKeyDown}
          >
            <span className="sticker-tab" aria-hidden="true">
              <IconGrip size={14} />
              {t('ws.sticker.label')}
            </span>
            <SignatureBlock sig={sig} signer={signer} time={signTime} scale={scale} fit={inField ? { w: rect.w, h: rect.h } : undefined} />
            {overlap ? (
              <span className="sticker-flag sticker-flag-warn" aria-hidden="true">
                <IconAlert size={13} />
                {t('ws.sticker.overlap')}
              </span>
            ) : inField ? (
              <span className="sticker-flag sticker-flag-ok" aria-hidden="true">
                <IconCheck size={13} />
                {t('ws.sticker.inField')}
              </span>
            ) : null}
            {coachmark && !dragging ? (
              <span className="sticker-coach" aria-hidden="true">
                {inField ? (doc.spec.fields.find((f) => f.id === placement.fieldId)?.kind === 'qr' ? t('ws.placed.qr') : t('ws.placed.field')) : t('ws.placed')}
              </span>
            ) : null}
            <span id={helpId} className="sr-only">
              {t('ws.sticker.help', { sign: `${mod}+${enter}` })}
            </span>
          </div>
        ) : null}
      </div>
    </div>
  );
}
