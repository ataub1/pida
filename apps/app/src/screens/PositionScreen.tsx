/**
 * Positionierung (docs/analyse/01 §4.3, F-30…F-43; Plan §6.7):
 * - Seitenvorschau aus Kacheln (`pida-render://`), Blockvorschau aus dem echten Appearance-Objekt,
 * - Ziehen per Maus/Touch/Stift (Block springt an den Zeiger, beim Greifen bleibt der Versatz – F-34);
 *   Touch (Plan §5.1 „Ziehen mit Fingerversatz“): außerhalb des Blocks angetippt, steht der Block mittig
 *   **über** dem Finger (der Finger verdeckt ihn nicht), kurze Haptik beim Greifen, größere Trefferfläche,
 * - Tastatur am Block: Pfeile 15 pt, Shift 1 pt, Strg/⌘ 50 pt (der Kern rechnet die Schritte),
 *   Bild↑/↓ Seite, Pos1/Ende erste/letzte Seite, Enter bestätigt; Mausrad blättert (Shift: 5 Seiten),
 * - Zahlenfelder x/y/Seite in mm oder pt als gleichwertige Alternative, Live-Region (nennt dieselben Kanten
 *   wie die Felder: linke Kante von links, Oberkante von unten),
 * - „Neue Seite“, „Automatisch“, „Unsichtbar“, „Signieren“.
 * Die Position im Kern ist maßgeblich; die Oberfläche zeigt beim Ziehen nur eine vorläufige Lage.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent, type WheelEvent } from 'react';
import { Button, Group, Input, Label, NumberField, Radio, RadioGroup } from 'react-aria-components';
import { useApp } from '../app/context';
import { IconChevronLeft, IconChevronRight, IconEyeOff, IconPagePlus, IconWand } from '../components/icons';
import { AccessButton, ScreenHeading, Spinner } from '../components/ui';
import { tileScale } from '../core/bridge';
import { mmToPt, ptToMm } from '../core/defaults';
import { isMobile, type StepView } from '../core/types';

type PositionView = Extract<StepView, { step: 'position' }>;
type Unit = 'mm' | 'pt';

const UNIT_KEY = 'pida.position-unit';

function readUnit(): Unit {
  try {
    return localStorage.getItem(UNIT_KEY) === 'pt' ? 'pt' : 'mm';
  } catch {
    return 'mm';
  }
}

/**
 * Mausrad-Weg in Pixeln: Zeilen/Seiten-Modus (`deltaMode` 1/2) umrechnen; mit Shift liefern
 * WebKit/Chromium unter macOS die Bewegung als `deltaX` (horizontales Scrollen).
 */
export function wheelDelta(e: { deltaX: number; deltaY: number; deltaMode: number; shiftKey: boolean }): number {
  const raw = e.deltaY !== 0 ? e.deltaY : e.shiftKey ? e.deltaX : 0;
  const factor = e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? 800 : 1;
  return raw * factor;
}

/** Klemmung wie F-35 mit KR-F11 (nur für die vorläufige Anzeige beim Ziehen). */
export function clampToPage(x: number, y: number, bw: number, bh: number, pw: number, ph: number): { x: number; y: number } {
  return { x: Math.max(0, Math.min(x, pw - bw)), y: Math.min(ph, Math.max(y, bh)) };
}

/** Touch: Abstand zwischen Blockunterkante und Finger in pt (Fingerversatz). */
export const TOUCH_OFFSET_PT = 12;

/**
 * Greifpunkt im Block in pt (von der linken oberen Ecke; `grabTop` nach unten). Auf dem Block bleibt der
 * Versatz zum Zeiger (F-34). Außerhalb springt der Block bei Maus/Stift mit der linken oberen Ecke an den
 * Zeiger, bei Touch mittig über den Finger (Unterkante [`TOUCH_OFFSET_PT`] über dem Berührpunkt).
 */
export function grabPoint(
  onBlock: boolean,
  pointerType: string,
  pointer: { x: number; top: number },
  block: { x: number; y: number; width: number; height: number },
  pageHeight: number,
): { grabX: number; grabTop: number } {
  if (onBlock) return { grabX: pointer.x - block.x, grabTop: pointer.top - (pageHeight - block.y) };
  if (pointerType === 'touch') return { grabX: block.width / 2, grabTop: block.height + TOUCH_OFFSET_PT };
  return { grabX: 0, grabTop: 0 };
}

/** Kurze Haptik beim Greifen per Touch (Android-WebView; iOS-WebKit kennt `vibrate` nicht). */
function haptic(): void {
  try {
    if (typeof navigator !== 'undefined' && typeof navigator.vibrate === 'function') navigator.vibrate(8);
  } catch {
    /* nur Komfort */
  }
}

export function PositionScreen({ view }: { view: PositionView }) {
  const { t, dispatch, bridge, announce, view: appView } = useApp();
  const mobile = isMobile(appView);
  const { doc, page, block, placement, new_page: newPage } = view;
  const totalPages = doc.page_count + (newPage ? 1 : 0);
  const virtual = page >= doc.page_count;
  const size = doc.page_sizes[Math.min(page, doc.page_count - 1)] ?? [595.28, 841.89];
  const [pw, ph] = size;

  // --- Einpassen -----------------------------------------------------------------------------
  const stageRef = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = stageRef.current;
    if (!el) return undefined;
    const measure = () => setBox({ w: el.clientWidth, h: el.clientHeight });
    measure();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const scale = useMemo(() => {
    const w = box.w > 0 ? box.w - 16 : 560;
    const h = box.h > 0 ? box.h - 16 : 760;
    return Math.max(0.1, Math.min(w / pw, h / ph));
  }, [box, pw, ph]);
  const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1;

  // --- Ziehen --------------------------------------------------------------------------------
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null);
  const dragging = useRef<{ grabX: number; grabTop: number; pointer: number } | null>(null);
  const pendingMove = useRef<{ x: number; y: number } | null>(null);
  const frame = useRef<number | null>(null);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const blockRef = useRef<HTMLDivElement>(null);

  const settle = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Vorläufige Lage verwerfen, sobald der Kern einen neuen Zustand meldet (nicht während des Ziehens):
  // maßgeblich ist immer die Position des Kerns – auch wenn sie sich nicht geändert hat (Klick auf den
  // Block ohne Bewegung) oder der Kern anders klemmt/rundet als die Vorschau.
  useEffect(() => {
    if (!dragging.current) setDrag(null);
  }, [view]);
  useEffect(
    () => () => {
      if (settle.current) clearTimeout(settle.current);
    },
    [],
  );

  const flushMove = useCallback(() => {
    frame.current = null;
    const m = pendingMove.current;
    if (!m) return;
    pendingMove.current = null;
    dispatch({ type: 'move_block', page, x: m.x, y: m.y });
  }, [dispatch, page]);

  const queueMove = (pos: { x: number; y: number }) => {
    pendingMove.current = pos;
    if (frame.current == null) {
      frame.current = typeof requestAnimationFrame === 'function' ? requestAnimationFrame(flushMove) : (setTimeout(flushMove, 16) as unknown as number);
    }
  };

  const toPagePoint = (e: PointerEvent) => {
    const rect = surfaceRef.current?.getBoundingClientRect();
    if (!rect) return null;
    return { x: (e.clientX - rect.left) / scale, top: (e.clientY - rect.top) / scale };
  };

  const current = drag ?? { x: placement.x, y: placement.y };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    // ein zweiter Finger (Pinch, Scrollen) verschiebt den Block nicht
    if (dragging.current && dragging.current.pointer !== e.pointerId) return;
    const p = toPagePoint(e);
    if (!p) return;
    const onBlock = blockRef.current?.contains(e.target as Node) ?? false;
    const pointerType = e.pointerType || 'mouse';
    const { grabX, grabTop } = grabPoint(onBlock, pointerType, p, { ...current, width: block.width, height: block.height }, ph);
    if (pointerType === 'touch') haptic();
    dragging.current = { grabX, grabTop, pointer: e.pointerId };
    e.currentTarget.setPointerCapture?.(e.pointerId);
    const next = clampToPage(p.x - grabX, ph - (p.top - grabTop), block.width, block.height, pw, ph);
    setDrag(next);
    queueMove(next);
    blockRef.current?.focus({ preventScroll: true });
    e.preventDefault();
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = dragging.current;
    if (!d || d.pointer !== e.pointerId) return;
    const p = toPagePoint(e);
    if (!p) return;
    const next = clampToPage(p.x - d.grabX, ph - (p.top - d.grabTop), block.width, block.height, pw, ph);
    setDrag(next);
    queueMove(next);
  };

  const endDrag = (e: PointerEvent<HTMLDivElement>) => {
    const d = dragging.current;
    if (!d || d.pointer !== e.pointerId) return;
    dragging.current = null;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
    if (settle.current) clearTimeout(settle.current);
    if (frame.current != null) {
      if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(frame.current);
      flushMove();
      // Die Antwort des Kerns ersetzt die Vorschau; meldet er keinen neuen Zustand (gleiche Position),
      // spätestens nach kurzer Zeit auf seine Position zurückfallen.
      settle.current = setTimeout(() => {
        settle.current = null;
        if (!dragging.current) setDrag(null);
      }, 400);
    } else {
      // Alles ist beim Kern angekommen – seine Position gilt.
      setDrag(null);
    }
  };

  // --- Seiten --------------------------------------------------------------------------------
  const goTo = useCallback(
    (p: number) => {
      const target = Math.max(0, Math.min(totalPages - 1, p));
      if (target !== page) dispatch({ type: 'change_page', page: target });
    },
    [dispatch, page, totalPages],
  );

  const wheel = useRef({ acc: 0, last: 0 });
  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    const w = wheel.current;
    const now = Date.now();
    w.acc += wheelDelta(e);
    if (Math.abs(w.acc) < 60 || now - w.last < 120) return;
    const dir = w.acc > 0 ? 1 : -1;
    w.acc = 0;
    w.last = now;
    goTo(page + dir * (e.shiftKey ? 5 : 1));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const fine = e.shiftKey;
    const coarse = e.ctrlKey || e.metaKey;
    const nudge = (dx: number, dy: number) => {
      e.preventDefault();
      dispatch({ type: 'nudge_block', dx, dy, fine, coarse });
    };
    switch (e.key) {
      case 'ArrowLeft':
        return nudge(-1, 0);
      case 'ArrowRight':
        return nudge(1, 0);
      case 'ArrowUp':
        return nudge(0, 1);
      case 'ArrowDown':
        return nudge(0, -1);
      case 'PageUp':
        e.preventDefault();
        return goTo(page - 1);
      case 'PageDown':
        e.preventDefault();
        return goTo(page + 1);
      case 'Home':
        e.preventDefault();
        return goTo(0);
      case 'End':
        e.preventDefault();
        return goTo(totalPages - 1);
      case 'Enter':
        e.preventDefault();
        dispatch({ type: 'confirm_position' });
        return;
      default:
    }
  };

  // --- Live-Region ---------------------------------------------------------------------------
  const announceArgs = useMemo(() => {
    // dieselben Kanten wie die Zahlenfelder: linke Kante von links, Oberkante (Feld Y) von unten
    const computed: Record<string, string> = {
      page: String(page + 1),
      pages: String(totalPages),
      left: ptToMm(placement.x).toFixed(0),
      top: ptToMm(placement.y).toFixed(0),
    };
    return { ...computed, ...view.announce };
  }, [page, totalPages, placement, view.announce]);
  const positionText = t.t('ng-ui-position-announce', announceArgs);
  const firstRender = useRef(true);
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return undefined;
    }
    const timer = setTimeout(() => announce(positionText), 350);
    return () => clearTimeout(timer);
  }, [positionText, announce]);

  // --- Zahlenfelder --------------------------------------------------------------------------
  const [unit, setUnit] = useState<Unit>(readUnit);
  const toUnit = (pt: number) => (unit === 'mm' ? ptToMm(pt) : pt);
  const fromUnit = (v: number) => (unit === 'mm' ? mmToPt(v) : v);
  const changeUnit = (u: string) => {
    const next: Unit = u === 'pt' ? 'pt' : 'mm';
    setUnit(next);
    try {
      localStorage.setItem(UNIT_KEY, next);
    } catch {
      /* nur Komfort */
    }
  };
  const fmt = { maximumFractionDigits: 1 } as const;

  // --- Kacheln -------------------------------------------------------------------------------
  const tile = tileScale(scale * dpr);
  const [loaded, setLoaded] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const pageUrl = virtual ? null : bridge.pageTileUrl(doc.id, page, tile);
  const blockUrl = bridge.blockPreviewUrl(tile, block.preview_revision);

  const left = current.x * scale;
  const top = (ph - current.y) * scale;

  return (
    <div className="screen screen-position">
      <ScreenHeading subtitle={doc.name}>{t.t('ng-ui-position-title')}</ScreenHeading>
      <p id="position-help" className="hint">
        {t.t(mobile ? 'ng-ui-position-help-mobile' : 'ng-ui-position-help')}
      </p>
      <div className="position-layout">
        <div className="page-stage" ref={stageRef} onWheel={onWheel}>
          <div
            className="page-surface"
            ref={surfaceRef}
            style={{ width: pw * scale, height: ph * scale }}
            data-virtual={virtual || undefined}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={endDrag}
            onPointerCancel={endDrag}
          >
            {pageUrl ? (
              <img
                key={pageUrl}
                className="page-image"
                src={pageUrl}
                alt={t.t('ng-ui-position-page-alt', { page: page + 1, pages: totalPages, name: doc.name })}
                draggable={false}
                onLoad={() => setLoaded(pageUrl)}
                onError={() => setFailed(pageUrl)}
              />
            ) : (
              <div className="page-blank" role="img" aria-label={t.t('ng-ui-position-new-page-alt')} />
            )}
            {pageUrl && loaded !== pageUrl && failed !== pageUrl ? (
              <div className="page-loading" aria-hidden="true">
                <Spinner size={22} /> {t.t('common-working')}
              </div>
            ) : null}
            {pageUrl && failed === pageUrl ? (
              <div className="page-loading" role="status">
                {t.t('ng-ui-position-preview-failed')}
              </div>
            ) : null}
            <div
              ref={blockRef}
              className="block-overlay"
              data-dragging={drag ? true : undefined}
              style={{ left, top, width: block.width * scale, height: block.height * scale }}
              role="application"
              aria-roledescription={t.t('ng-ui-position-block-role')}
              aria-label={t.t('positioning-signature')}
              aria-describedby="position-help position-status"
              tabIndex={0}
              onKeyDown={onKeyDown}
            >
              <img className="block-image" src={blockUrl} alt="" draggable={false} />
            </div>
          </div>
        </div>

        <div className="position-controls">
          <p id="position-status" className="position-status">
            {positionText}
          </p>
          <Group className="pager" aria-label={t.t('ng-ui-position-pages')}>
            <Button className="icon-button" aria-label={t.t('ng-ui-page-previous')} isDisabled={page <= 0} onPress={() => goTo(page - 1)}>
              <IconChevronLeft />
            </Button>
            <span className="pager-label">{t.t('positioning-page', { arg1: page + 1, arg2: totalPages })}</span>
            <Button className="icon-button" aria-label={t.t('ng-ui-page-next')} isDisabled={page >= totalPages - 1} onPress={() => goTo(page + 1)}>
              <IconChevronRight />
            </Button>
          </Group>

          <fieldset className="coords">
            <legend>{t.t('ng-ui-position-coordinates')}</legend>
            <RadioGroup className="unit-switch" orientation="horizontal" value={unit} onChange={changeUnit} aria-label={t.t('ng-ui-unit')}>
              <Radio value="mm" className="segment">
                mm
              </Radio>
              <Radio value="pt" className="segment">
                pt
              </Radio>
            </RadioGroup>
            <NumberField
              className="field number-field"
              value={Number(toUnit(current.x).toFixed(1))}
              minValue={0}
              formatOptions={fmt}
              onChange={(v) => {
                if (Number.isFinite(v)) dispatch({ type: 'move_block', page, x: fromUnit(v), y: placement.y });
              }}
            >
              <Label className="field-label">{t.t('ng-ui-position-x', { unit })}</Label>
              <Input className="input" />
            </NumberField>
            <NumberField
              className="field number-field"
              value={Number(toUnit(current.y).toFixed(1))}
              minValue={0}
              formatOptions={fmt}
              onChange={(v) => {
                if (Number.isFinite(v)) dispatch({ type: 'move_block', page, x: placement.x, y: fromUnit(v) });
              }}
            >
              <Label className="field-label">{t.t('ng-ui-position-y', { unit })}</Label>
              <Input className="input" />
            </NumberField>
            <NumberField
              className="field number-field"
              value={page + 1}
              minValue={1}
              maxValue={totalPages}
              step={1}
              onChange={(v) => {
                if (Number.isFinite(v)) goTo(Math.round(v) - 1);
              }}
            >
              <Label className="field-label">{t.t('ng-ui-position-page-field')}</Label>
              <Input className="input" />
            </NumberField>
          </fieldset>

          <div className="position-actions">
            <AccessButton
              msg={newPage ? 'positioning-removeNewPage' : 'positioning-newPage'}
              icon={<IconPagePlus size={18} />}
              onPress={() => dispatch({ type: 'toggle_new_page' })}
              isDisabled={doc.signed && !newPage}
            />
            <AccessButton msg="ng-ui-position-auto" icon={<IconWand size={18} />} onPress={() => dispatch({ type: 'auto_place' })} />
            <AccessButton msg="ng-ui-position-invisible" icon={<IconEyeOff size={18} />} onPress={() => dispatch({ type: 'choose_invisible' })} />
          </div>
          {doc.signed ? <p className="hint">{t.t('ng-ui-position-signed-hint')}</p> : null}
          <AccessButton msg="positioning-sign" variant="primary" className="btn-wide" onPress={() => dispatch({ type: 'confirm_position' })} />
        </div>
      </div>
    </div>
  );
}
