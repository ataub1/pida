/**
 * Geometrie der Platzierung: Seitenränder, Einrasten, freie Flächen, Überdeckung, automatische Position.
 * Reine Funktionen (ohne DOM), damit Ziehen und Tastatur dieselben Regeln nutzen und testbar sind.
 */
import type { Block, PageSpec, Rect } from './types';

/** Satzspiegel der Mock-Seiten (pt). */
export const MARGIN = { top: 56, right: 56, bottom: 48, left: 56 } as const;

/** Abstand, den der Block zu Inhalt halten soll (pt). */
export const CLEARANCE = 6;

/** 1 mm in pt. */
export const PT_PER_MM = 72 / 25.4;

export function ptToMm(pt: number): number {
  return Math.round(pt / PT_PER_MM);
}

export function intersects(a: Rect, b: Rect, pad = 0): boolean {
  return a.x < b.x + b.w + pad && a.x + a.w + pad > b.x && a.y < b.y + b.h + pad && a.y + a.h + pad > b.y;
}

export function clamp(v: number, min: number, max: number): number {
  return Math.min(Math.max(v, min), max);
}

/** Block vollständig auf der Seite halten (F-35). */
export function clampToPage(x: number, y: number, size: { w: number; h: number }, page: Pick<PageSpec, 'w' | 'h'>) {
  return { x: clamp(x, 0, Math.max(0, page.w - size.w)), y: clamp(y, 0, Math.max(0, page.h - size.h)) };
}

/** Flächen, die als Inhalt gelten (alles außer leeren Signaturfeldern – die liefert `fields`). */
export function occupied(page: PageSpec): Rect[] {
  return page.blocks.map((b: Block) => b.rect);
}

/** Inhaltsblöcke, die der Block überdeckt. */
export function overlaps(rect: Rect, page: PageSpec): Block[] {
  return page.blocks.filter((b) => intersects(rect, b.rect));
}

/**
 * Freie waagrechte Bänder innerhalb des Satzspiegels (für die dezente Hervorhebung beim Ziehen).
 * Ein Band ist frei, wenn kein Inhaltsblock seine y-Spanne berührt; Bänder unter `minH` entfallen.
 */
export function freeBands(page: PageSpec, minH = 40): Rect[] {
  const top = MARGIN.top - 16;
  const bottom = page.h - MARGIN.bottom + 8;
  const spans = page.blocks
    .map((b) => [b.rect.y - CLEARANCE, b.rect.y + b.rect.h + CLEARANCE] as const)
    .sort((a, b) => a[0] - b[0]);
  const bands: Rect[] = [];
  let cursor = top;
  for (const [y0, y1] of spans) {
    if (y0 > cursor && y0 - cursor >= minH) bands.push({ x: MARGIN.left - 16, y: cursor, w: page.w - MARGIN.left - MARGIN.right + 32, h: y0 - cursor });
    cursor = Math.max(cursor, y1);
  }
  if (bottom - cursor >= minH) bands.push({ x: MARGIN.left - 16, y: cursor, w: page.w - MARGIN.left - MARGIN.right + 32, h: bottom - cursor });
  return bands;
}

/** Ist die Fläche frei (kein Inhalt, innerhalb der Seite)? */
export function isFree(rect: Rect, page: PageSpec): boolean {
  if (rect.x < 0 || rect.y < 0 || rect.x + rect.w > page.w || rect.y + rect.h > page.h) return false;
  return !page.blocks.some((b) => intersects(rect, b.rect, CLEARANCE));
}

/**
 * Automatische Position auf einer Seite: bevorzugt rechtsbündig direkt unter dem letzten Inhalt
 * (dort, wo man unterschreiben würde), sonst die erste freie Stelle von unten rechts nach oben links.
 * `null`, wenn die Seite keinen Platz bietet.
 */
export function autoPosition(page: PageSpec, size: { w: number; h: number }): { x: number; y: number } | null {
  const right = page.w - MARGIN.right - size.w;
  const body = page.blocks.filter((b) => b.kind !== 'footer');
  // Leere Seite: rechts unten, wo man üblicherweise unterschreibt.
  if (body.length === 0) return { x: right, y: page.h - MARGIN.bottom - 24 - size.h };
  const contentBottom = body.reduce<number>((m, b) => Math.max(m, b.rect.y + b.rect.h), MARGIN.top);
  const below = { x: right, y: contentBottom + 18, w: size.w, h: size.h };
  if (below.y + size.h <= page.h - MARGIN.bottom && isFree(below, page)) return { x: below.x, y: below.y };
  const step = 6;
  for (let y = page.h - MARGIN.bottom - size.h; y >= MARGIN.top; y -= step) {
    for (let x = right; x >= MARGIN.left; x -= step) {
      if (isFree({ x, y, w: size.w, h: size.h }, page)) return { x, y };
    }
  }
  return null;
}

export type SnapKind = 'left' | 'right' | 'hcenter' | 'top' | 'bottom' | 'vcenter' | 'below-content';

export interface Guide {
  axis: 'x' | 'y';
  /** Lage der Linie in pt */
  at: number;
  kind: SnapKind;
}

export interface SnapResult {
  x: number;
  y: number;
  guides: Guide[];
  snapped: SnapKind[];
}

/**
 * Einrasten an Satzspiegelrändern, Seitenmitte und „unter dem Text“. `threshold` in pt.
 * Rechts + unten ergibt die Ecke „rechts unten“.
 */
export function snap(x: number, y: number, size: { w: number; h: number }, page: PageSpec, threshold: number): SnapResult {
  const body = page.blocks.filter((b) => b.kind !== 'footer');
  const contentBottom = body.reduce((m, b) => Math.max(m, b.rect.y + b.rect.h), 0);
  const xTargets: { pos: number; line: number; kind: SnapKind }[] = [
    { pos: MARGIN.left, line: MARGIN.left, kind: 'left' },
    { pos: page.w - MARGIN.right - size.w, line: page.w - MARGIN.right, kind: 'right' },
    { pos: (page.w - size.w) / 2, line: page.w / 2, kind: 'hcenter' },
  ];
  const yTargets: { pos: number; line: number; kind: SnapKind }[] = [
    { pos: MARGIN.top, line: MARGIN.top, kind: 'top' },
    { pos: page.h - MARGIN.bottom - size.h, line: page.h - MARGIN.bottom, kind: 'bottom' },
    { pos: (page.h - size.h) / 2, line: page.h / 2, kind: 'vcenter' },
  ];
  if (contentBottom > 0 && contentBottom + 18 + size.h <= page.h - MARGIN.bottom) {
    yTargets.push({ pos: contentBottom + 18, line: contentBottom + 18, kind: 'below-content' });
  }
  const guides: Guide[] = [];
  const snapped: SnapKind[] = [];
  let nx = x;
  let ny = y;
  const bestX = nearest(xTargets, x, threshold);
  if (bestX) {
    nx = bestX.pos;
    guides.push({ axis: 'x', at: bestX.line, kind: bestX.kind });
    snapped.push(bestX.kind);
  }
  const bestY = nearest(yTargets, y, threshold);
  if (bestY) {
    ny = bestY.pos;
    guides.push({ axis: 'y', at: bestY.line, kind: bestY.kind });
    snapped.push(bestY.kind);
  }
  const clamped = clampToPage(nx, ny, size, page);
  return { x: clamped.x, y: clamped.y, guides, snapped };
}

function nearest<T extends { pos: number }>(targets: T[], v: number, threshold: number): T | null {
  let best: T | null = null;
  let dist = threshold;
  for (const t of targets) {
    const d = Math.abs(t.pos - v);
    if (d <= dist) {
      best = t;
      dist = d;
    }
  }
  return best;
}

/** Beschreibende Lage auf der Seite (für Ansage und Inspektor), z. B. „rechts unten“. */
export function describeZone(rect: Rect, page: Pick<PageSpec, 'w' | 'h'>): { h: 'left' | 'center' | 'right'; v: 'top' | 'middle' | 'bottom' } {
  const cx = rect.x + rect.w / 2;
  const cy = rect.y + rect.h / 2;
  const h = cx < page.w / 3 ? 'left' : cx > (page.w * 2) / 3 ? 'right' : 'center';
  const v = cy < page.h * 0.33 ? 'top' : cy > page.h * 0.55 ? 'bottom' : 'middle';
  return { h, v };
}
