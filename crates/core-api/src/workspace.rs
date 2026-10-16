//! Oberfläche „Arbeitsbereich“ im Kern (ADR 0004 §3.2–§3.7, Paket K1): Projektion von
//! [`ViewState::document`](crate::ViewState::document) (Seiten, Sticker, Ziele, Layout, Überdeckung),
//! [`ViewState::signers`](crate::ViewState::signers), Fortschritt der Signierkarte, Fehlerkontext und
//! Zusammenfassung „Fertig“ sowie der Layout-Cache (Belegungsraster je Seite).
//!
//! **Leistung (§3.3):** Das Belegungsraster einer Seite entsteht einmal asynchron (`spawn_blocking`, eine
//! Seite je Schritt – danach dürfen wartende Kachelanfragen an den Renderer des Dokuments) und liegt im
//! [`LayoutCache`] (höchstens [`GRIDS_PER_DOC`] Seiten je Dokument, LRU). Projektion und Prüfung nach jeder
//! Positionsänderung lesen nur das Bitset – nie PDFium. Fehlt das Raster einer Seite, fehlt ihr Layout und
//! `check.overlaps_content` ist `None`, bis es da ist (dann neue Projektion mit derselben Revision).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use pida_backend_api::{BackendId, Capabilities, NativeForm, SecretKind};
use pida_pades::PriorStatus;
use pida_pdf_cos::{PageInfo, Rect};
use pida_placement::{
    BlockGeometry, GuideAxis as PGuideAxis, GuideKind as PGuideKind, LayoutOptions, OccupancyGrid, PageLayout,
    Placement, ResolvedPosition, matrix_unit_rect, new_page_geometry, page_layout, user_rect_to_visible,
    widget_overlaps,
};
use pida_workflow::{
    BackendOption, DocFacts, DocId, PlacementOrigin, PositionChoice, SignFlow, SigningPhase, State, Target,
    WorkspaceEdit,
};

use crate::docs::DocData;
use crate::view::{
    BatchFlowView, BlockView, CertificateSource, ConfirmMethod, DocConstraintsView, DocRefView, DocumentDetectionView,
    DocumentPhase, DocumentView, ErrorContextView, ErrorOrigin, ExistingSignatureView, FieldTargetView, GuideAxis,
    GuideKind, GuideView, InteractionView, PageLayoutView, PdfaDecision, PdfaIdView, PlacementCheckView,
    PlacementTarget, PriorStatusView, QrTargetView, RectPt, SignedSummaryView, SignerView, SignersView,
    SigningProgressView, SigningStage,
};

/// Höchstzahl gecachter Belegungsraster je Dokument (LRU).
pub(crate) const GRIDS_PER_DOC: usize = 24;
/// Höchstzahl der Seiten eines `InspectPages`.
pub(crate) const MAX_INSPECTED: usize = 8;

/// Meldungs-IDs der Einschränkungen (ADR 0004 §3.16).
const REASON_SIGNED: &str = pida_workflow::MSG_CONSTRAINT_SIGNED;
const REASON_CERTIFIED: &str = pida_workflow::MSG_CONSTRAINT_CERTIFIED;
const REASON_INVISIBLE: &str = pida_workflow::MSG_CONSTRAINT_INVISIBLE_PROFILE;

// ------------------------------------------------------------------------------------------------
// Layout-Cache
// ------------------------------------------------------------------------------------------------

/// Raster eines Dokuments.
#[derive(Debug, Default)]
struct DocGrids {
    /// fertige Raster, zuletzt benutzte hinten
    grids: VecDeque<(usize, Arc<OccupancyGrid>)>,
    /// nicht berechenbare Seiten (Renderfehler) – nicht erneut versuchen
    failed: Vec<usize>,
    /// noch nicht begonnene Seiten (nur der letzte Wunsch zählt)
    pending: VecDeque<usize>,
    /// gerade berechnete Seite
    running: Option<usize>,
}

/// Schlüssel eines gemerkten Layouts: Dokument, Seite, Blockbreite und -höhe (Bits).
type LayoutKey = (DocId, usize, u32, u32);

/// Belegungsraster aller offenen Dokumente und gemerkte Layouts (Schlüssel: Seite und Blockgröße).
#[derive(Debug, Default)]
pub(crate) struct LayoutCache {
    docs: HashMap<DocId, DocGrids>,
    memo: Mutex<HashMap<LayoutKey, Arc<PageLayout>>>,
}

impl LayoutCache {
    /// Raster der Seite, falls berechnet.
    pub(crate) fn grid(&self, doc: DocId, page: usize) -> Option<Arc<OccupancyGrid>> {
        self.docs.get(&doc)?.grids.iter().find(|(p, _)| *p == page).map(|(_, g)| g.clone())
    }

    /// Diese Seiten werden gebraucht: noch nicht begonnene Wünsche werden ersetzt; vorhandene Raster
    /// werden als zuletzt benutzt markiert. Liefert die als Nächstes zu berechnende Seite, falls gerade
    /// keine läuft.
    pub(crate) fn request(&mut self, doc: DocId, pages: &[usize]) -> Option<usize> {
        let d = self.docs.entry(doc).or_default();
        for p in pages {
            if let Some(i) = d.grids.iter().position(|(q, _)| q == p) {
                let entry = d.grids.remove(i).expect("Index geprüft");
                d.grids.push_back(entry);
            }
        }
        d.pending = pages
            .iter()
            .copied()
            .filter(|p| d.running != Some(*p) && !d.failed.contains(p) && !d.grids.iter().any(|(q, _)| q == p))
            .collect();
        if d.running.is_some() {
            return None;
        }
        let next = d.pending.pop_front()?;
        d.running = Some(next);
        Some(next)
    }

    /// Raster fertig (bzw. `None` = nicht berechenbar); liefert die nächste Seite.
    pub(crate) fn finished(&mut self, doc: DocId, page: usize, grid: Option<Arc<OccupancyGrid>>) -> Option<usize> {
        let d = self.docs.get_mut(&doc)?;
        if d.running == Some(page) {
            d.running = None;
        }
        match grid {
            Some(g) => {
                d.grids.retain(|(p, _)| *p != page);
                d.grids.push_back((page, g));
                while d.grids.len() > GRIDS_PER_DOC {
                    d.grids.pop_front();
                }
            }
            None => d.failed.push(page),
        }
        if d.running.is_some() {
            return None;
        }
        let next = d.pending.pop_front()?;
        d.running = Some(next);
        Some(next)
    }

    /// Dokument freigegeben.
    pub(crate) fn remove(&mut self, doc: DocId) {
        self.docs.remove(&doc);
        lock(&self.memo).retain(|k, _| k.0 != doc);
    }

    /// Layout der Seite für einen Block (gemerkt je Blockgröße).
    pub(crate) fn layout(
        &self,
        doc: DocId,
        page: usize,
        grid: &OccupancyGrid,
        block: BlockGeometry,
    ) -> Arc<PageLayout> {
        let key = (doc, page, block.width.to_bits(), block.height.to_bits());
        let mut memo = lock(&self.memo);
        if let Some(l) = memo.get(&key) {
            return l.clone();
        }
        let l = Arc::new(page_layout(grid, page, block, &LayoutOptions::default()));
        // Speicher begrenzen: alte Blockgrößen derselben Seite verwerfen
        memo.retain(|k, _| !(k.0 == doc && k.1 == page));
        memo.insert(key, l.clone());
        l
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Belegungsraster einer Seite aus dem Forced-Color-Rendering (läuft im Blocking-Pool).
pub(crate) fn compute_grid(
    data: &DocData,
    render: &dyn pida_render::RenderBackend,
    page: usize,
) -> Option<OccupancyGrid> {
    let opts = pida_render::ForcedColorOptions::default();
    let result = data.with_renderer(render, |r| -> Result<OccupancyGrid, pida_render::RenderError> {
        let size = r.page_size(page)?;
        let image = r.render_forced_color(page, &opts)?;
        Ok(OccupancyGrid::from_forced_color(
            &image,
            size.width,
            size.height,
            opts.dpi,
            opts.background,
            pida_placement::SCAN_TOLERANCE,
        ))
    });
    match result {
        Ok(Ok(grid)) => Some(grid),
        Ok(Err(e)) | Err(e) => {
            tracing::debug!(code = e.code(), "Belegungsraster nicht berechenbar");
            None
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Zustand des Arbeitsbereichs in core-api
// ------------------------------------------------------------------------------------------------

/// Konto-Chip-Auswahl: Signiermittel und (bei „Jedes Mal fragen“) das Dokument, für das sie gilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Selection {
    pub backend: Option<BackendId>,
    pub doc: Option<DocId>,
}

/// Schlüssel der Platzierung für die Revision (`DocumentView::revision`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlacementKey {
    doc: DocId,
    page: usize,
    placement: Option<[u32; 5]>,
    new_page: bool,
    target: Option<Target>,
    invisible: bool,
}

/// Arbeitsbereich-Daten des Kern-Tasks (nur Projektion; der Bearbeitungszustand liegt im Reducer).
#[derive(Debug, Default)]
pub(crate) struct WorkspaceRuntime {
    /// Arbeitsbereich-Modus (`CoreOptions::workspace_ui`, nie mit `headless`)
    pub enabled: bool,
    /// Startargument-Automatik (D-06)
    pub cli_auto: bool,
    pub layouts: LayoutCache,
    /// Seiten des letzten `InspectPages`
    pub inspected: Vec<usize>,
    /// Revision der Platzierung
    pub revision: u64,
    pub last_key: Option<PlacementKey>,
    /// letzte Bewegung vom Kern geklemmt
    pub clamped: bool,
    pub selection: Option<Selection>,
    /// core-api hat den laufenden Vorgang selbst abgebrochen (sonst ist `Cancelled` eine Ablehnung)
    pub own_cancel: bool,
    /// Anmeldung dieser Sitzung erfolgreich
    pub login_done: bool,
    /// Bestätigungsweg der zuletzt offenen Maske
    pub last_method: Option<ConfirmMethod>,
    /// Zeitpunkt des aktuellen Fehlers (RFC 3339)
    pub error_at: Option<String>,
    /// Optionen für den Konto-Chip
    pub signer_options: Vec<BackendOption>,
    /// iOS: über „Teilen“ bzw. „Öffnen in“ empfangenes Dokument, dem der Hinweis zum Teilen gilt (ADR 0004
    /// Nachtrag 12, `ViewState::share_hint`)
    pub share_hint: Option<DocId>,
    /// Hinweis zum Teilen in diesem Lauf ausgeblendet (`DismissShareHint`)
    pub share_hint_dismissed: bool,
}

/// Lokale Zeit als RFC 3339 mit Offset.
pub(crate) fn rfc3339_now() -> String {
    rfc3339(crate::plan::local_now())
}

/// RFC 3339 (Sekunden, mit Offset).
pub(crate) fn rfc3339(t: time::OffsetDateTime) -> String {
    let t = t.replace_nanosecond(0).unwrap_or(t);
    t.format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
}

/// Rechteck `PAGE_ABS` → Bridge.
pub(crate) fn rect_pt(r: Rect) -> RectPt {
    let r = r.normalized();
    RectPt { left: r.llx, bottom: r.lly, right: r.urx, top: r.ury }
}

/// Rechteck des Blocks `w × h` mit linker oberer Ecke `p` (`PAGE_ABS`).
fn block_rect(p: &Placement, w: f32, h: f32) -> Rect {
    Rect { llx: p.x, lly: p.y - h, urx: p.x + w, ury: p.y }
}

/// Seite `page` des Dokuments; `page == Seitenzahl` = virtuelle neue Seite.
pub(crate) fn page_info(data: &DocData, page: usize) -> Option<PageInfo> {
    if page < data.pages.len() { data.pages.get(page).cloned() } else { data.pages.last().map(new_page_geometry) }
}

/// Telefonnummer bzw. Benutzername für den Konto-Chip maskieren (ADR 0004 §3.4): Telefonnummern als
/// „+43 660 ···123“, Namen ab sechs Zeichen als „m···an“, kürzere nur „···“.
pub(crate) fn mask_account(value: &str) -> String {
    let v = value.trim();
    let digits: String = v.chars().filter(char::is_ascii_digit).collect();
    let phone = v.starts_with('+') && v[1..].chars().all(|c| c.is_ascii_digit() || c == ' ');
    if phone && digits.len() >= 8 {
        let (cc, rest) = digits.split_at(2);
        let (op, _) = rest.split_at(3);
        let tail = &digits[digits.len() - 3..];
        return format!("+{cc} {op} ···{tail}");
    }
    if phone {
        return "···".into();
    }
    let chars: Vec<char> = v.chars().collect();
    if chars.len() < 6 {
        return "···".into();
    }
    let head: String = chars[..1].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{head}···{tail}")
}

/// Signiermittel → Einstellung (`backend.default`).
pub(crate) fn choice_for(backend: BackendId) -> Option<pida_config::BackendChoice> {
    match backend {
        BackendId::AtrustNative => Some(pida_config::BackendChoice::IdAustria),
        BackendId::LocalBku => Some(pida_config::BackendChoice::LocalBku),
        BackendId::Keystore => Some(pida_config::BackendChoice::Keystore),
        _ => None,
    }
}

/// Endzertifikat eines CMS ohne bekannte Kette: das Zertifikat, das keines der anderen ausgestellt hat.
pub(crate) fn leaf_certificate(certs: &[pida_cades::Certificate]) -> Option<&pida_cades::Certificate> {
    match certs {
        [] => None,
        [one] => Some(one),
        many => many.iter().find(|c| {
            let subject = c.subject_string();
            !many.iter().any(|o| o.issuer_string() == subject && o.subject_string() != subject)
        }),
    }
}

// ------------------------------------------------------------------------------------------------
// Projektion
// ------------------------------------------------------------------------------------------------

/// Was der Arbeitsbereich zeigt (aus dem Workflow-Zustand abgeleitet).
pub(crate) struct Shown<'a> {
    pub phase: DocumentPhase,
    pub doc: &'a DocFacts,
    pub page: usize,
    pub placement: Option<Placement>,
    pub new_page: bool,
    pub edit: WorkspaceEdit,
}

/// Arbeitsbereich-Sicht des Zustands (Tabelle ADR 0004 §3.1); `None` = kein Dokument.
pub(crate) fn shown<'a>(state: &'a State, invisible_profile: bool, data: Option<&DocData>) -> Option<Shown<'a>> {
    let state = match state {
        State::Error { from: Some(f), .. } => f.as_ref(),
        s => s,
    };
    let from_position = |doc: &DocFacts, position: &PositionChoice, edit: &Option<WorkspaceEdit>| {
        let edit = edit.clone().unwrap_or_else(|| WorkspaceEdit::from_position(position, invisible_profile));
        let n = doc.page_count;
        let (page, placement, new_page) = match (&edit.parked, position.manual()) {
            (Some(p), _) => (p.page, p.placement, p.new_page),
            (None, Some(p)) => (p.page_index.min(n), Some(*p), n > 0 && p.page_index >= n),
            (None, None) => (doc.start.map_or(n.saturating_sub(1), |p| p.page_index.min(n)), doc.start, false),
        };
        (page, placement, new_page, edit)
    };
    let (phase, doc, page, placement, new_page, edit) = match state {
        State::AnalysisWarning { doc, fields, .. } => {
            let mut fields = fields.clone();
            if fields.is_empty()
                && let Some(d) = data
            {
                fields = pida_placement::find_empty_sig_fields(&d.report.fields);
            }
            let edit = WorkspaceEdit::new(fields, None, invisible_profile);
            let page = doc.start.map_or(doc.page_count.saturating_sub(1), |p| p.page_index);
            (DocumentPhase::Warning, doc, page, doc.start, false, edit)
        }
        State::Positioning { page, placement, new_page, doc, edit, .. } => {
            let edit = edit.clone().unwrap_or_else(|| {
                WorkspaceEdit::from_position(
                    &placement.map(PositionChoice::Manual).unwrap_or(PositionChoice::Auto),
                    invisible_profile,
                )
            });
            (DocumentPhase::Editing, doc, *page, *placement, *new_page, edit)
        }
        State::Signing { doc, position, edit, .. } | State::ChoosingBackend { doc, position, edit, .. } => {
            let (page, placement, new_page, edit) = from_position(doc, position, edit);
            (DocumentPhase::Signing, doc, page, placement, new_page, edit)
        }
        State::Output { doc, position, edit, .. } => {
            let (page, placement, new_page, edit) = from_position(doc, position, edit);
            (DocumentPhase::Done, doc, page, placement, new_page, edit)
        }
        _ => return None,
    };
    Some(Shown { phase, doc, page, placement, new_page, edit })
}

/// Rechteck und Seite des Ziels im sichtbaren Raum (`PAGE_ABS`): Feld-Widget bzw. QR-Bild.
fn target_rect(data: &DocData, edit: &WorkspaceEdit) -> Option<(usize, Rect)> {
    match edit.target {
        Target::Field(i) => {
            let f = edit.fields.get(i)?;
            let page = f.page_index.unwrap_or(0);
            let info = data.pages.get(page)?;
            Some((page, user_rect_to_visible(info, f.rect)))
        }
        Target::Qr => {
            let qr = edit.qr.as_ref()?;
            let info = data.pages.get(qr.page_index)?;
            Some((qr.page_index, user_rect_to_visible(info, matrix_unit_rect(&qr.ctm))))
        }
        Target::Free => None,
    }
}

impl crate::runtime::CoreTask {
    /// Revision der Platzierung fortschreiben (nach jeder Nachricht).
    pub(crate) fn track_revision(&mut self) {
        if !self.ws.enabled {
            return;
        }
        let key = shown(&self.state, self.ctx.invisible, None).map(|s| PlacementKey {
            doc: s.doc.id,
            page: s.page,
            placement: s
                .placement
                .map(|p| [p.page_index as u32, p.x.to_bits(), p.y.to_bits(), p.width.to_bits(), p.height.to_bits()]),
            new_page: s.new_page,
            target: Some(s.edit.target),
            invisible: s.edit.invisible,
        });
        if key != self.ws.last_key {
            if key.as_ref().map(|k| k.doc) != self.ws.last_key.as_ref().map(|k| k.doc) {
                self.ws.inspected.clear();
            }
            self.ws.revision += 1;
            self.ws.last_key = key;
        }
    }

    /// Seiten, deren Layout die Projektion zeigt: letzte `InspectPages` und die Seite des Stickers.
    fn layout_pages(&self, sticker_page: Option<usize>, page_count: usize) -> Vec<usize> {
        let mut pages: Vec<usize> = self.ws.inspected.iter().copied().filter(|p| *p <= page_count).collect();
        if let Some(p) = sticker_page.filter(|p| *p <= page_count) {
            pages.push(p);
        }
        pages.sort_unstable();
        pages.dedup();
        pages.truncate(MAX_INSPECTED + 1);
        pages
    }

    /// Raster der benötigten Seiten anfordern (asynchron, eine Seite je Schritt).
    pub(crate) fn request_layout(&mut self) {
        if !self.ws.enabled {
            return;
        }
        let Some(render) = self.shared.render.clone() else { return };
        let Some(s) = shown(&self.state, self.ctx.invisible, None) else { return };
        let doc = s.doc.id;
        let n = s.doc.page_count;
        let sticker_page = s.placement.map(|p| p.page_index).or(Some(s.page));
        let wanted: Vec<usize> = self.layout_pages(sticker_page, n).into_iter().filter(|p| *p < n).collect();
        if let Some(page) = self.ws.layouts.request(doc, &wanted) {
            self.spawn_grid(render, doc, page);
        }
    }

    fn spawn_grid(&self, render: Arc<dyn pida_render::RenderBackend>, doc: DocId, page: usize) {
        let Some(data) = self.shared.docs.get(doc) else {
            self.shared.send(crate::runtime::Msg::GridReady { doc, page, grid: None });
            return;
        };
        let shared = self.shared.clone();
        tokio::task::spawn_blocking(move || {
            let grid = compute_grid(&data, render.as_ref(), page).map(Arc::new);
            shared.send(crate::runtime::Msg::GridReady { doc, page, grid });
        });
    }

    /// Raster fertig: übernehmen und die nächste Seite beginnen.
    pub(crate) fn on_grid_ready(&mut self, doc: DocId, page: usize, grid: Option<Arc<OccupancyGrid>>) {
        if self.shared.docs.get(doc).is_none() {
            self.ws.layouts.remove(doc);
            return;
        }
        if let (Some(next), Some(render)) = (self.ws.layouts.finished(doc, page, grid), self.shared.render.clone()) {
            self.spawn_grid(render, doc, next);
        }
    }

    /// `ViewState::document` (nur im Arbeitsbereich-Modus).
    pub(crate) fn document_view(&self) -> Option<DocumentView> {
        if !self.ws.enabled {
            return None;
        }
        let data = self.state.doc().and_then(|d| self.shared.docs.get(d.id));
        let s = shown(&self.state, self.ctx.invisible, data.as_deref())?;
        let n = s.doc.page_count;
        let block = lock(&self.shared.block).clone();
        let edit = &s.edit;
        // Blockgröße: Vorschau des Dokuments, sonst Maße der Lage
        let (bw, bh) = match (block.doc == Some(s.doc.id), s.placement) {
            (true, _) => (block.width, block.height),
            (false, Some(p)) => (p.width, p.height),
            (false, None) => (block.width, block.height),
        };
        let target = data.as_deref().and_then(|d| target_rect(d, edit));
        // maßgebliche Lage: beim Feld/QR das Ziel-Rechteck
        let placement = match (&target, s.placement) {
            (Some((page, r)), _) => {
                Placement { page_index: *page, x: r.llx, y: r.ury, width: r.width(), height: r.height() }
            }
            (None, Some(p)) => Placement { width: bw, height: bh, ..p },
            (None, None) => {
                Placement { page_index: s.page.min(n.saturating_sub(1)), x: 0.0, y: 0.0, width: bw, height: bh }
            }
        };
        let sticker_rect = if edit.invisible {
            None
        } else {
            match &target {
                Some((_, r)) => Some(*r),
                None => s.placement.map(|p| block_rect(&p, bw, bh)),
            }
        };
        let sticker_page = sticker_rect.map(|_| placement.page_index);
        let doc_info = self.doc_info(s.doc.id);
        let fields = edit
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let rect = data.as_deref().and_then(|d| {
                    let info = d.pages.get(f.page_index?)?;
                    let r = f.rect.normalized();
                    (!r.is_empty()).then(|| rect_pt(user_rect_to_visible(info, r)))
                });
                FieldTargetView {
                    index: u32::try_from(i).unwrap_or(u32::MAX),
                    name: f.qualified_name.clone(),
                    page: f.page_index,
                    rect,
                    selectable: true,
                }
            })
            .collect();
        let qr = edit.qr.as_ref().and_then(|q| {
            let info = data.as_deref()?.pages.get(q.page_index)?;
            Some(QrTargetView {
                page: q.page_index,
                rect: rect_pt(user_rect_to_visible(info, matrix_unit_rect(&q.ctm))),
                id: q.payload.id.clone(),
            })
        });
        let existing = data.as_deref().map(existing_signatures).unwrap_or_default();
        let mut reasons = Vec::new();
        if s.doc.signed {
            reasons.push(REASON_SIGNED.to_owned());
        }
        if !s.doc.new_visible_field_allowed {
            reasons.push(REASON_CERTIFIED.to_owned());
        }
        if self.ctx.invisible {
            reasons.push(REASON_INVISIBLE.to_owned());
        }
        let constraints = DocConstraintsView {
            new_page_allowed: s.doc.new_page_allowed(),
            visible_allowed: !self.ctx.invisible,
            new_visible_field_allowed: s.doc.new_visible_field_allowed,
            reasons,
        };
        // Layout der angefragten Seiten und der Seite des Stickers (fertige Raster; virtuelle Seite leer)
        let block_geom = BlockGeometry { width: bw, height: bh };
        let layout = self
            .layout_pages(sticker_page.or(Some(s.page)), n)
            .into_iter()
            .filter_map(|page| {
                let grid = if page == n {
                    let info = page_info(data.as_deref()?, page)?;
                    let (w, h) = info.visible_size();
                    Arc::new(OccupancyGrid::empty(w, h))
                } else {
                    self.ws.layouts.grid(s.doc.id, page)?
                };
                Some(layout_view(&self.ws.layouts.layout(s.doc.id, page, &grid, block_geom)))
            })
            .collect();
        let check = self.placement_check(data.as_deref(), &s, &placement, sticker_rect);
        let mut announce = std::collections::BTreeMap::new();
        let mm = |pt: f32| format!("{:.0}", pt * 25.4 / 72.0);
        announce.insert("page".into(), (placement.page_index + 1).to_string());
        announce.insert("pages".into(), (n + usize::from(s.new_page)).to_string());
        announce.insert("left".into(), mm(placement.x));
        announce.insert("top".into(), mm(placement.y));
        announce.insert("bottom".into(), mm(placement.y - placement.height));
        Some(DocumentView {
            doc: doc_info,
            size_bytes: data.as_deref().map_or(0, |d| d.src.len()),
            phase: s.phase,
            revision: self.ws.revision,
            block: BlockView { width: bw, height: bh, preview_revision: block.revision },
            placement,
            origin: edit.origin,
            target: match edit.target {
                Target::Free => PlacementTarget::Free,
                Target::Field(i) => PlacementTarget::Field { index: u32::try_from(i).unwrap_or(u32::MAX) },
                Target::Qr => PlacementTarget::Qr,
            },
            invisible: edit.invisible,
            new_page: s.new_page,
            sticker: sticker_rect.map(rect_pt),
            fields,
            qr,
            existing,
            constraints,
            layout,
            check,
            announce,
            detection: Some(self.detection(data.as_deref(), edit)),
            placement_locked: edit.placement_locked(),
        })
    }

    /// Erkanntes und automatisch Entschiedenes (Einstellungen › Dokument; Nachtrag K3): PDF/A-Kennung und
    /// wirksame PDF/A-Entscheidung wie beim Signieren (`plan::pdfa_mode`: erkannte Kennung gilt immer,
    /// `signature.pdfa_force` nur mit einem Profil, das eine PDF/A-Variante hat), Lage aus der Automatik.
    fn detection(&self, data: Option<&DocData>, edit: &WorkspaceEdit) -> DocumentDetectionView {
        let id = data.and_then(|d| d.report.pdfa.as_ref());
        let pdfa = id.map(|p| PdfaIdView { part: p.part, conformance: p.conformance.clone() });
        DocumentDetectionView { pdfa, pdfa_decision: self.pdfa_decision(id), auto_placed: edit.automatic }
    }

    /// Wirksame PDF/A-Entscheidung nach derselben Regel wie beim Signieren: Profil-ID wie
    /// [`crate::plan`] (Kennung erkannt bzw. `signature.pdfa_force` → `_PDFA`-Variante, sofern das Profil eine
    /// hat), dann [`crate::plan::pdfa_mode`]. „Erkannt“ nur, wenn tatsächlich PDF/A-konform signiert wird (z. B.
    /// nicht bei `pdfaid:part = 4` mit einem Profil ohne PDF/A-Variante); „erzwungen“ nur ohne Kennung mit
    /// einem Profil, dessen Variante PDF/A ist (`SIG_PDFA1B_VALID`/`SIG_PDFA_VALID`). Ohne Profilsammlung
    /// (Start) gilt eine erkannte Kennung als „erkannt“.
    fn pdfa_decision(&self, id: Option<&pida_pades::PdfaId>) -> PdfaDecision {
        let eff = self.cfg.effective();
        let profile_id = eff.pdfas_profile_id(pida_i18n::Lang::system(), id.is_some());
        let Some(profile) = self.shared.profiles.as_deref().and_then(|store| store.get(&profile_id)) else {
            return if id.is_some() { PdfaDecision::Detected } else { PdfaDecision::None };
        };
        match (id, crate::plan::pdfa_mode(id, profile)) {
            (_, pida_sigblock::PdfaMode::None) => PdfaDecision::None,
            (Some(_), _) => PdfaDecision::Detected,
            (None, _) => PdfaDecision::Forced,
        }
    }

    /// Live-Prüfung (§3.3): liest nur das Belegungsraster und die Felder der Analyse.
    fn placement_check(
        &self,
        data: Option<&DocData>,
        s: &Shown<'_>,
        placement: &Placement,
        sticker: Option<Rect>,
    ) -> PlacementCheckView {
        let mut check = PlacementCheckView {
            for_revision: self.ws.revision,
            overlaps_content: None,
            overlaps_widget: false,
            clamped: self.ws.clamped && s.edit.origin == PlacementOrigin::User,
        };
        let (Some(data), Some(rect)) = (data, sticker) else {
            // unsichtbar: nichts kann überdecken
            if s.edit.invisible {
                check.overlaps_content = Some(false);
            }
            return check;
        };
        match s.edit.target {
            Target::Field(i) => {
                // das Zielfeld selbst ist kein Inhalt; Widgets anderer Felder schon
                check.overlaps_content = Some(false);
                if let Some(f) = s.edit.fields.get(i) {
                    check.overlaps_widget = other_widgets_overlap(data, f.page_index, f.rect, Some(f.field_id));
                }
            }
            Target::Qr => {
                check.overlaps_content = Some(false);
                if let Some(q) = &s.edit.qr {
                    check.overlaps_widget =
                        other_widgets_overlap(data, Some(q.page_index), matrix_unit_rect(&q.ctm), None);
                }
            }
            Target::Free => {
                let n = s.doc.page_count;
                if placement.page_index < n {
                    check.overlaps_content =
                        self.ws.layouts.grid(s.doc.id, placement.page_index).map(|g| g.occupied(rect));
                    let pos = ResolvedPosition {
                        page_index: placement.page_index,
                        new_page: false,
                        x: rect.llx,
                        y: rect.ury,
                        width: rect.width(),
                        height: rect.height(),
                        rotation: 0,
                        notices: Vec::new(),
                    };
                    if let Some(info) = data.pages.get(placement.page_index) {
                        check.overlaps_widget = widget_overlaps(&pos, info, &data.report.fields);
                    }
                } else {
                    // virtuelle neue Seite: leer (wie ihr Layout, `OccupancyGrid::empty`), ohne Felder
                    check.overlaps_content = Some(false);
                }
            }
        }
        check
    }

    /// `ViewState::signers` (Konto-Chip): Optionen, Auswahl, Standard.
    pub(crate) fn signers_view(&self) -> SignersView {
        // iOS/Android: ID Austria ist das einzige Signiermittel und damit der Standard (ADR 0004 Nachtrag 11)
        let default = self.cfg.default_backend().map(pida_workflow::backend_for_choice);
        let current = self.state.queue().and_then(|q| q.current_id());
        let options = self.signer_views();
        let available = |id: BackendId| options.iter().any(|o| o.backend == id && o.available);
        // eine Richtlinie, die ein auf Mobilgeräten fehlendes Signiermittel vorschreibt, wählt dort nichts vor
        let forced = self
            .cfg
            .policy
            .as_ref()
            .and_then(|p| p.forced_backend)
            .filter(|f| self.cfg.keystore_offered || *f == pida_config::BackendChoice::IdAustria)
            .map(pida_workflow::backend_for_choice);
        let selection = self.ws.selection.filter(|s| default.is_some() || s.doc.is_none() || s.doc == current);
        let selected = match (forced, selection) {
            (Some(f), _) => Some(f),
            (None, Some(s)) => s.backend,
            (None, None) => default.filter(|d| available(*d)),
        };
        SignersView { options, selected, default }
    }

    fn signer_views(&self) -> Vec<SignerView> {
        let mobile = !self.mobile.platform.is_desktop();
        let allowed = self.cfg.allowed_backends();
        let by_policy = |id: BackendId| allowed.as_ref().is_none_or(|a| a.contains(&id));
        let eff = self.cfg.effective();
        let mut out: Vec<SignerView> = self
            .ws
            .signer_options
            .iter()
            .filter(|o| matches!(o.id, BackendId::AtrustNative | BackendId::LocalBku | BackendId::Keystore))
            .map(|o| {
                let mut notes = Vec::new();
                if mobile && o.id == BackendId::AtrustNative && o.capabilities.as_ref().is_none_or(|c| !c.factors.fido2)
                {
                    notes.push(crate::runtime::MSG_FIDO2_UNAVAILABLE_MOBILE.to_owned());
                }
                let account = match o.id {
                    BackendId::AtrustNative => {
                        let s = &self.cfg.settings.id_austria;
                        let name = self
                            .cfg
                            .overlay
                            .username
                            .as_deref()
                            .or(s.remember.then_some(s.username.as_deref()).flatten());
                        name.map(mask_account)
                    }
                    BackendId::Keystore => eff.keystore_file().and_then(|f| f.file_name()).map(|n| {
                        let n = n.to_string_lossy().into_owned();
                        match eff.keystore_alias() {
                            Some(a) if !a.is_empty() => format!("{n} · {a}"),
                            _ => n,
                        }
                    }),
                    _ => None,
                };
                let unavailable_reason = (!o.available()).then(|| {
                    if !by_policy(o.id) {
                        "ng-signer-policy"
                    } else {
                        match o.id {
                            BackendId::LocalBku => "ng-signer-localbku-not-running",
                            BackendId::Keystore => "ng-signer-keystore-missing",
                            _ => pida_workflow::MSG_SIGNER_UNAVAILABLE,
                        }
                    }
                    .to_owned()
                });
                SignerView {
                    backend: o.id,
                    available: o.available(),
                    account,
                    capabilities: o.capabilities.clone(),
                    notes,
                    unavailable_reason,
                    batch_flow: o.capabilities.as_ref().map(BatchFlowView::from_capabilities),
                }
            })
            .collect();
        // Keystore nicht eingerichtet: trotzdem im Menü (Einrichten in den Einstellungen) – nur am Desktop; auf
        // iOS/Android gibt es nur ID Austria (ADR 0004 Nachtrag 11)
        if !self.registry.is_fixed()
            && self.cfg.keystore_offered
            && !out.iter().any(|o| o.backend == BackendId::Keystore)
        {
            let reason =
                if !by_policy(BackendId::Keystore) { "ng-signer-policy" } else { "ng-signer-keystore-missing" };
            out.push(SignerView {
                backend: BackendId::Keystore,
                available: false,
                account: None,
                capabilities: None,
                notes: Vec::new(),
                unavailable_reason: Some(reason.to_owned()),
                batch_flow: None,
            });
        }
        out
    }

    /// Optionen des Konto-Chips neu ermitteln (Start, Einstellungen, Polling, Vordergrund).
    pub(crate) fn refresh_signers(&mut self) {
        if !self.ws.enabled {
            return;
        }
        let env = self.registry.environment(self.shared.broker.interactive(), &self.cfg);
        self.ws.signer_options = self.registry.options(&env);
    }

    /// Fortschritt der Signierkarte (§3.6).
    pub(crate) fn progress_view(
        &self,
        backend: BackendId,
        phase: Option<SigningPhase>,
        doc: &DocFacts,
        flow: Option<&SignFlow>,
    ) -> SigningProgressView {
        if !self.ws.enabled {
            return SigningProgressView::default();
        }
        let interaction = self.shared.broker.current();
        let default_method = match backend {
            BackendId::Keystore => Some(ConfirmMethod::Password),
            BackendId::LocalBku => Some(ConfirmMethod::Card),
            _ => self.ws.last_method,
        };
        let by_phase = |phase: Option<SigningPhase>| match phase {
            None | Some(SigningPhase::OpeningSession) => (SigningStage::Connecting, None),
            Some(SigningPhase::ReadingCertificate) => (SigningStage::Certificate, None),
            Some(SigningPhase::Preparing) => (SigningStage::Preparing, None),
            Some(SigningPhase::Signing) => (SigningStage::Confirming, default_method),
            Some(SigningPhase::Finishing) => (SigningStage::Finishing, None),
            Some(SigningPhase::Writing) => (SigningStage::Saving, None),
        };
        let (stage, method) = match &interaction {
            InteractionView::Form { form: NativeForm::Login { .. }, .. } => (SigningStage::Login, None),
            InteractionView::Form { form, .. } => (SigningStage::Confirming, form_method(form)),
            InteractionView::Secret { prompt, .. } => match prompt.kind {
                SecretKind::Fido2Pin => (SigningStage::Confirming, Some(ConfirmMethod::Fido2)),
                SecretKind::KeystorePassword | SecretKind::KeyPassword => {
                    (SigningStage::Confirming, Some(ConfirmMethod::Password))
                }
                _ => by_phase(phase),
            },
            _ => by_phase(phase),
        };
        let certificate = match flow {
            Some(f) if !f.needs_certificate => CertificateSource::NotNeeded,
            None => CertificateSource::NotNeeded,
            Some(_) => lock(&self.shared.sign).certificate_source(),
        };
        let name = self
            .state
            .queue()
            .and_then(|q| q.current.as_ref())
            .filter(|d| d.id == doc.id)
            .map(|d| d.display_name.clone())
            .unwrap_or_default();
        let done = u32::from(matches!(phase, Some(SigningPhase::Finishing | SigningPhase::Writing)));
        SigningProgressView {
            backend: Some(backend),
            stage,
            method: method.or(if stage == SigningStage::Confirming { default_method } else { None }),
            doc: Some(DocRefView { id: doc.id, name }),
            index: 1,
            total: 1,
            login_done: self.ws.login_done,
            confirmations_done: done,
            confirmations_total: 1,
            certificate,
            batch: false,
            pause: None,
        }
    }

    /// Herkunft, Zeitpunkt, Signiermittel und Dokument eines Fehlers (§3.6).
    pub(crate) fn error_context(&self, error: &pida_workflow::WorkflowError, from: Option<&State>) -> ErrorContextView {
        if !self.ws.enabled {
            return ErrorContextView::default();
        }
        let origin = match from {
            Some(State::Analyzing { .. } | State::Resuming { doc: None, .. }) => match error.code().as_str() {
                "error-IOError" | "error-MayNotBeAPDF" | "error-PDFPwdProtected" | "error-PDFProtected" => {
                    ErrorOrigin::Load
                }
                _ => ErrorOrigin::Analysis,
            },
            Some(State::Signing { .. } | State::ChoosingBackend { .. } | State::Resuming { .. }) => {
                ErrorOrigin::Signing
            }
            Some(State::Output { .. }) => ErrorOrigin::Output,
            Some(State::Starting) => ErrorOrigin::Config,
            _ => ErrorOrigin::Other,
        };
        let backend = match from {
            Some(State::Signing { backend, .. } | State::Output { backend, .. }) => Some(*backend),
            Some(State::Resuming { plan, .. }) => Some(plan.backend),
            _ => None,
        };
        let doc = self
            .state
            .queue()
            .and_then(|q| q.current.as_ref())
            .map(|d| DocRefView { id: d.id, name: d.display_name.clone() });
        ErrorContextView { origin, occurred_at: self.ws.error_at.clone(), backend, doc }
    }

    /// Zusammenfassung „Fertig“ (§3.7), sobald gespeichert.
    pub(crate) fn signed_summary(
        &self,
        saved: &pida_pades::OutputInfo,
        backend: BackendId,
        caps: Option<&Capabilities>,
    ) -> Option<SignedSummaryView> {
        if !self.ws.enabled {
            return None;
        }
        let info = lock(&self.shared.sign).summary.clone()?;
        let mobile = !self.mobile.platform.is_desktop();
        let file_name = saved.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let folder_display =
            if mobile { None } else { saved.path.parent().map(|p| p.display().to_string()).filter(|p| !p.is_empty()) };
        // laut qcStatements des Signerzertifikats (K4); die Fähigkeiten des Backends nur, wenn unbekannt
        let qualification = match info.qualification {
            pida_backend_api::Qualification::Unknown => {
                caps.map_or(pida_backend_api::Qualification::AdES, |c| c.qualification)
            }
            q => q,
        };
        Some(SignedSummaryView {
            file_name,
            folder_display,
            signer: info.signer,
            signing_time: info.signing_time,
            backend,
            qualification,
            self_check: true,
            page: info.page,
        })
    }
}

/// Bestätigungsweg einer offenen Maske.
fn form_method(form: &NativeForm) -> Option<ConfirmMethod> {
    match form {
        NativeForm::WaitForApp { .. } => Some(ConfirmMethod::App),
        NativeForm::QrCode { .. } => Some(ConfirmMethod::Qr),
        NativeForm::SmsTan { .. } => Some(ConfirmMethod::Sms),
        NativeForm::Fido2 { .. } => Some(ConfirmMethod::Fido2),
        NativeForm::Login { .. } => None,
    }
}

/// Bestätigungsweg zum Merken (für den Stepper nach dem Schließen der Maske).
pub(crate) fn remembered_method(view: &InteractionView) -> Option<ConfirmMethod> {
    match view {
        InteractionView::Form { form, .. } => form_method(form),
        InteractionView::Secret { prompt, .. } if prompt.kind == SecretKind::Fido2Pin => Some(ConfirmMethod::Fido2),
        _ => None,
    }
}

/// Überdecken Widgets anderer Felder das Rechteck (Benutzerraum der Seite)?
fn other_widgets_overlap(
    data: &DocData,
    page: Option<usize>,
    rect: Rect,
    except: Option<pida_pdf_cos::ObjectId>,
) -> bool {
    let r = rect.normalized();
    data.report.fields.iter().filter(|f| Some(f.field_id) != except).flat_map(|f| f.widgets.iter()).any(|w| {
        let o = w.rect.normalized();
        w.page_index == page && !o.is_empty() && r.llx < o.urx && o.llx < r.urx && r.lly < o.ury && o.lly < r.ury
    })
}

/// Vorhandene Signaturen mit Lage des Signaturbilds (`PAGE_ABS`; leeres Rect = unsichtbar).
fn existing_signatures(data: &DocData) -> Vec<ExistingSignatureView> {
    data.report
        .signatures
        .iter()
        .map(|s| {
            let widget =
                data.report.fields.iter().find(|f| f.qualified_name == s.field_name).and_then(|f| f.widgets.first());
            let page = widget.and_then(|w| w.page_index);
            let rect = widget.and_then(|w| {
                let r = w.rect.normalized();
                let info = data.pages.get(w.page_index?)?;
                (!r.is_empty()).then(|| rect_pt(user_rect_to_visible(info, r)))
            });
            ExistingSignatureView {
                field_name: s.field_name.clone(),
                signer: s.signer_cn.clone(),
                signing_time: s.signing_time.map(rfc3339),
                status: match s.status {
                    PriorStatus::Valid => PriorStatusView::Valid,
                    PriorStatus::Invalid => PriorStatusView::Invalid,
                    PriorStatus::NotVerifiable => PriorStatusView::NotVerifiable,
                },
                page: rect.as_ref().and(page),
                rect,
            }
        })
        .collect()
}

/// Layout der Seite → Bridge.
fn layout_view(l: &PageLayout) -> PageLayoutView {
    PageLayoutView {
        page: l.page,
        content: l.content.map(rect_pt),
        free: l.free.iter().copied().map(rect_pt).collect(),
        guides: l
            .guides
            .iter()
            .map(|g| GuideView {
                axis: match g.axis {
                    PGuideAxis::X => GuideAxis::X,
                    PGuideAxis::Y => GuideAxis::Y,
                },
                at: g.at,
                kind: match g.kind {
                    PGuideKind::ContentLeft => GuideKind::ContentLeft,
                    PGuideKind::ContentRight => GuideKind::ContentRight,
                    PGuideKind::ContentTop => GuideKind::ContentTop,
                    PGuideKind::ContentBottom => GuideKind::ContentBottom,
                    PGuideKind::CenterX => GuideKind::CenterX,
                    PGuideKind::CenterY => GuideKind::CenterY,
                    PGuideKind::BelowContent => GuideKind::BelowContent,
                    PGuideKind::MarginLeft => GuideKind::MarginLeft,
                    PGuideKind::MarginRight => GuideKind::MarginRight,
                    PGuideKind::MarginTop => GuideKind::MarginTop,
                    PGuideKind::MarginBottom => GuideKind::MarginBottom,
                },
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_masking() {
        assert_eq!(mask_account("+436601234123"), "+43 660 ···123");
        assert_eq!(mask_account("+43 660 1234567"), "+43 660 ···567");
        assert_eq!(mask_account("+43660"), "···", "zu kurz");
        assert_eq!(mask_account("max.mustermann"), "m···nn");
        assert_eq!(mask_account("anna"), "···", "kurze Namen nie teilweise");
        assert_eq!(mask_account(""), "···");
        // keine vollständige Nummer im Ergebnis
        assert!(!mask_account("+436641234567").contains("1234567"));
    }

    #[test]
    fn layout_cache_requests_replace_pending_and_respect_lru() {
        let mut c = LayoutCache::default();
        let d = DocId(1);
        assert_eq!(c.request(d, &[3, 4, 5]), Some(3), "erste Seite beginnt");
        assert_eq!(c.request(d, &[7, 8]), None, "läuft noch; Wünsche ersetzt");
        let g = Arc::new(OccupancyGrid::empty(595.0, 842.0));
        assert_eq!(c.finished(d, 3, Some(g.clone())), Some(7), "nur der letzte Wunsch zählt");
        assert!(c.grid(d, 3).is_some());
        assert_eq!(c.finished(d, 7, None), Some(8));
        assert_eq!(c.request(d, &[7]), None, "nicht berechenbare Seiten nicht erneut");
        assert_eq!(c.finished(d, 8, Some(g.clone())), None);
        for p in 10..10 + GRIDS_PER_DOC {
            c.request(d, &[p]);
            c.finished(d, p, Some(g.clone()));
        }
        assert!(c.grid(d, 3).is_none(), "LRU: älteste Seite verdrängt");
        assert!(c.grid(d, 10 + GRIDS_PER_DOC - 1).is_some());
        c.remove(d);
        assert!(c.grid(d, 11).is_none());
    }

    #[test]
    fn rfc3339_has_offset() {
        let s = rfc3339_now();
        assert!(s.len() >= 20 && (s.ends_with('Z') || s.contains('+') || s[19..].contains('-')), "{s}");
    }
}
