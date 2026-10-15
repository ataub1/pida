/**
 * Arbeitsbereich (Kern des Prototyps): Miniaturen | durchgehende Seitenansicht mit Zoom | Inspektor.
 * Ein Bildschirm statt vier Schritten: Öffnen → (automatisch platziert) → Signieren → Fertig.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Toolbar } from 'react-aria-components';
import { activeDoc, displayPages, initialPlacement, SIGNER_NAME, useAppState, useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import { isoWithOffset } from '../../model/format';
import { clamp, type SnapKind } from '../../model/geometry';
import type { DocState, Placement } from '../../model/types';
import { Button, IconButton } from '../../ui/controls';
import { useNarrow } from '../../ui/hooks';
import { IconEyeOff, IconFitWidth, IconSidebar, IconSignature, IconZoomIn, IconZoomOut } from '../../ui/Icons';
import { Inspector } from './Inspector';
import { LockedDocument } from './LockedDocument';
import { layoutPages, Pages, PAGE_PAD } from './Pages';
import { describePlacement, stickerRect } from './placement';
import { Thumbnails } from './Thumbnails';

const PT_TO_PX = 96 / 72;
const ZOOM_STEPS = [50, 67, 75, 90, 100, 110, 125, 150, 175, 200];

export function Workspace({ onOpenFiles }: { onOpenFiles: () => void }) {
  const state = useAppState();
  const doc = activeDoc(state);
  if (!doc) return null;
  if (!doc.unlocked) {
    return (
      <div className="workspace workspace-locked">
        <LockedDocument doc={doc} onOther={onOpenFiles} />
      </div>
    );
  }
  return <WorkspaceView key={doc.spec.id} doc={doc} />;
}

function WorkspaceView({ doc }: { doc: DocState }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const narrow = useNarrow();
  const sig = state.settings.sigBlock;
  const pages = displayPages(doc);
  const [zoom, setZoom] = useState<number | null>(null);
  const [fitScale, setFitScale] = useState(1.15);
  const [fitReady, setFitReady] = useState(typeof ResizeObserver === 'undefined');
  const didInitialScroll = useRef(false);
  const [current, setCurrent] = useState(doc.placement?.page ?? 0);
  const [announcement, setAnnouncement] = useState('');
  const [pagesOpen, setPagesOpen] = useState(!narrow);
  const [coach, setCoach] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const hostRef = useRef<HTMLDivElement>(null);
  const stickerRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const signTime = useMemo(() => isoWithOffset(new Date()), []);

  const maxW = Math.max(...pages.map((p) => p.w));
  const scale = zoom ? (zoom / 100) * PT_TO_PX : fitScale;
  const layout = layoutPages(pages, scale);
  const zoomPct = Math.round((scale / PT_TO_PX) * 100);

  // Seitenbreite einpassen
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth;
      if (w > 0) {
        setFitScale(clamp((w - 2 * PAGE_PAD - 8) / maxW, 0.35, 1.45));
        setFitReady(true);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [maxW]);

  // Scrollen/Fokus nach dem nächsten Layout (ohne requestAnimationFrame – robust auch in gedrosselten Tabs)
  const pending = useRef<{ scroll: ScrollBehavior | null; focus: boolean }>({ scroll: null, focus: false });
  const requestStickerScroll = useCallback((behavior: ScrollBehavior = 'smooth', focus = false) => {
    pending.current = { scroll: behavior, focus };
  }, []);
  useLayoutEffect(() => {
    const { scroll, focus } = pending.current;
    if (!scroll && !focus) return;
    pending.current = { scroll: null, focus: false };
    if (scroll) stickerRef.current?.scrollIntoView?.({ block: 'center', behavior: scroll });
    if (focus) stickerRef.current?.focus({ preventScroll: true });
  });

  // Beim Öffnen zum Signaturblock scrollen (er steht auf der letzten Seite) – so lange sich die
  // eingepasste Breite in der ersten Sekunde noch ändert, erneut ausrichten.
  const mountedAt = useRef(Date.now());
  useLayoutEffect(() => {
    if (!fitReady) return;
    if (didInitialScroll.current && Date.now() - mountedAt.current > 1000) return;
    didInitialScroll.current = true;
    stickerRef.current?.scrollIntoView?.({ block: 'center' });
  }, [fitReady, fitScale]);

  // Von der Automatik auf eine andere Seite bzw. in einen Platzhalter versetzt (z. B. geänderte
  // Platzhaltersuche in den Einstellungen): Block in den Blick holen und den Hinweis erneut zeigen
  const autoKey = doc.autoPlaced && doc.placement ? `${doc.placement.page}:${doc.placement.fieldId ?? ''}` : null;
  const lastAutoKey = useRef(autoKey);
  useLayoutEffect(() => {
    if (autoKey === lastAutoKey.current) return;
    lastAutoKey.current = autoKey;
    if (!autoKey) return;
    stickerRef.current?.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
    setCoach(true);
  }, [autoKey]);

  // Hinweis „automatisch platziert“ nach einigen Sekunden ausblenden – er verdeckt sonst (v. a. schmal) Text
  useEffect(() => {
    if (!coach) return undefined;
    const timer = setTimeout(() => setCoach(false), 8000);
    return () => clearTimeout(timer);
  }, [coach]);

  // Miniaturen folgen der Layoutbreite (schmal: eingeklappt, breit: sichtbar)
  useEffect(() => {
    setPagesOpen(!narrow);
  }, [narrow]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const probe = el.scrollTop + el.clientHeight / 3;
    let idx = 0;
    layout.tops.forEach((top, i) => {
      if (top <= probe) idx = i;
    });
    if (idx !== current) setCurrent(idx);
  };

  const scrollToPage = (i: number) => {
    const el = scrollRef.current;
    const top = layout.tops[i];
    if (el && top !== undefined) el.scrollTo?.({ top: top - 12, behavior: 'smooth' });
    setCurrent(i);
    // Schmal liegen die Miniaturen über der Seite – nach der Auswahl wieder schließen und den Fokus
    // auf den Umschalter zurückgeben (der gewählte Miniatur-Knopf verschwindet)
    if (narrow) {
      setPagesOpen(false);
      toolbarRef.current?.querySelector<HTMLElement>('button')?.focus();
    }
  };

  const place = useCallback(
    (p: Placement, _snapped: SnapKind[]) => {
      dispatch({ type: 'place', id: doc.spec.id, placement: p });
    },
    [dispatch, doc.spec.id],
  );

  const announce = useCallback((text: string) => setAnnouncement(text), []);

  const autoPlace = () => {
    const p = initialPlacement({ ...doc, placement: null }, sig, null);
    dispatch({ type: 'place', id: doc.spec.id, placement: p });
    announce(describePlacement(t, doc, p, pages, sig));
    requestStickerScroll('smooth');
  };

  const focusField = (fieldId: string) => {
    const f = doc.spec.fields.find((x) => x.id === fieldId);
    if (!f) return;
    const p: Placement = { page: f.page, x: f.rect.x, y: f.rect.y, fieldId: f.id };
    dispatch({ type: 'place', id: doc.spec.id, placement: p });
    announce(describePlacement(t, doc, p, pages, sig));
    setCoach(false);
    requestStickerScroll('smooth', true);
  };

  const zoomBy = (dir: 1 | -1) => {
    const cur = zoomPct;
    const next = dir > 0 ? ZOOM_STEPS.find((z) => z > cur + 1) : [...ZOOM_STEPS].reverse().find((z) => z < cur - 1);
    setZoom(next ?? (dir > 0 ? ZOOM_STEPS[ZOOM_STEPS.length - 1]! : ZOOM_STEPS[0]!));
  };

  // Zoom-Kürzel (Strg/⌘ + Plus/Minus/0)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || state.overlay || state.signing) return;
      if (e.key === '+' || e.key === '=') {
        e.preventDefault();
        zoomBy(1);
      } else if (e.key === '-') {
        e.preventDefault();
        zoomBy(-1);
      } else if (e.key === '0') {
        e.preventDefault();
        setZoom(null);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const sticker = doc.placement ? stickerRect(doc, doc.placement, pages, sig) : null;
  const unusedField = doc.spec.fields.find((f) => f.kind === 'field' && f.id !== doc.placement?.fieldId);

  return (
    <div className={`workspace${pagesOpen ? ' pages-open' : ''}`}>
      {pagesOpen ? <Thumbnails doc={doc} pages={pages} current={current} onSelect={scrollToPage} sticker={sticker} /> : null}

      <h1 className="sr-only">{doc.spec.fileName}</h1>
      <section className="canvas" aria-label={t('ws.canvas', { name: doc.spec.fileName })}>
        {/* React-Aria-Toolbar: Pfeiltasten zwischen den Knöpfen, Tab verlässt die Leiste (ARIA-Muster „toolbar“) */}
        <Toolbar className="canvas-toolbar" aria-label={t('ws.toolbar')} ref={toolbarRef}>
          <IconButton label={t('ws.togglePages')} onPress={() => setPagesOpen((v) => !v)} aria-pressed={pagesOpen}>
            <IconSidebar size={18} />
          </IconButton>
          <span className="page-indicator" aria-live="off">
            {t('ws.pageIndicator', { n: current + 1, total: pages.length })}
          </span>
          <span className="toolbar-spacer" />
          <IconButton label={t('ws.zoomOut')} onPress={() => zoomBy(-1)} isDisabled={zoomPct <= ZOOM_STEPS[0]!}>
            <IconZoomOut size={18} />
          </IconButton>
          <span className="zoom-value" aria-label={t('ws.zoomValue', { value: zoomPct })}>
            {zoomPct} %
          </span>
          <IconButton label={t('ws.zoomIn')} onPress={() => zoomBy(1)} isDisabled={zoomPct >= ZOOM_STEPS[ZOOM_STEPS.length - 1]!}>
            <IconZoomIn size={18} />
          </IconButton>
          <IconButton label={t('ws.fit')} onPress={() => setZoom(null)} aria-pressed={zoom === null}>
            <IconFitWidth size={18} />
          </IconButton>
        </Toolbar>

        {doc.invisible ? (
          <p className="canvas-banner">
            <IconEyeOff size={16} />
            {t('ws.invisible')}
          </p>
        ) : unusedField ? (
          <div className="canvas-banner canvas-banner-field">
            <IconSignature size={16} />
            <span>
              <strong>{t('insp.field.found')}:</strong> {t('ws.field.banner', { name: unusedField.label, page: unusedField.page + 1 })}
            </span>
            <Button variant="secondary" size="sm" onPress={() => focusField(unusedField.id)}>
              {t('insp.field.use')}
            </Button>
          </div>
        ) : null}

        <div className="canvas-scroll" ref={scrollRef} onScroll={onScroll}>
          <Pages
            doc={doc}
            pages={pages}
            scale={scale}
            sig={sig}
            signer={SIGNER_NAME}
            signTime={signTime}
            onPlace={place}
            onAnnounce={announce}
            onInteract={() => setCoach(false)}
            coachmark={coach}
            hostRef={hostRef}
            stickerRef={stickerRef}
            onRequestScroll={() => requestStickerScroll('smooth')}
          />
        </div>
        <div className="sr-only" role="status">
          {announcement}
        </div>
      </section>

      <Inspector
        doc={doc}
        pages={pages}
        sticker={sticker}
        signTime={signTime}
        onSign={() => dispatch({ type: 'startSigning' })}
        onAutoPlace={autoPlace}
      />
    </div>
  );
}
