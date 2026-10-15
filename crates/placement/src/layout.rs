//! Seitenlayout der Oberfläche „Arbeitsbereich“ (ADR 0004 §3.3): Belegungsraster, Inhaltsrahmen, freie
//! Flächen, Einrastlinien und Überdeckungsprüfung.
//!
//! Grundlage ist das Forced-Color-Raster der Seite (dasselbe Rendering wie der PDF-AS-Rasterscan,
//! [`crate::RasterScanner`]): Jeder Bildpunkt, der vom Hintergrund abweicht, ist **Inhalt** – auch
//! Bilder (`keep_images`) und Widgets (KR-F06). Daraus entsteht ein [`OccupancyGrid`] mit Zellen von
//! [`GRID_CELL_PT`] pt (A4 ≈ 199 × 281 Zellen ≈ 7 KB). Das Raster wird einmal je Seite berechnet und
//! danach nur noch gelesen: [`page_layout`] und [`OccupancyGrid::occupied`] sind reine Funktionen ohne
//! Renderer (die Prüfung nach jeder Positionsänderung liest nur das Bitset, nie PDFium).
//!
//! Die automatische Platzierung bleibt der pixelgenaue PDF-AS-Scan ([`crate::resolve_position`]); das
//! 3-pt-Raster ersetzt ihn nicht.
//!
//! Koordinaten: `PAGE_ABS` der sichtbaren (gedrehten) Seite in pt – Ursprung links unten, y nach oben
//! ([`Rect`]: `llx`/`lly` links unten, `urx`/`ury` rechts oben).

use pida_pdf_cos::Rect;
use pida_render::RgbaImage;

use crate::position::BlockGeometry;

/// Kantenlänge einer Rasterzelle in pt.
pub const GRID_CELL_PT: f32 = 3.0;
/// Abstand freier Flächen zum Inhalt in pt.
pub const FREE_GAP_PT: f32 = 6.0;
/// Einrastlinie „unter dem Text“: Oberkante des Blocks so weit unter dem Inhalt (pt).
pub const BELOW_CONTENT_PT: f32 = 18.0;
/// Ränder der Einrastlinien auf Seiten ohne Inhalt (pt).
pub const EMPTY_PAGE_MARGIN_PT: f32 = 36.0;
/// Höchstzahl freier Flächen je Seite.
pub const MAX_FREE_RECTS: usize = 16;

/// Belegungsraster einer Seite (Bitset, Zeile 0 = oberer Seitenrand).
#[derive(Clone, PartialEq)]
pub struct OccupancyGrid {
    page_width: f32,
    page_height: f32,
    cols: usize,
    rows: usize,
    bits: Vec<u64>,
}

impl std::fmt::Debug for OccupancyGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OccupancyGrid")
            .field("page_width", &self.page_width)
            .field("page_height", &self.page_height)
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .field("occupied", &self.count_occupied())
            .finish()
    }
}

/// Weicht der Bildpunkt um mindestens `tolerance` (Summe der Kanalbeträge) vom Hintergrund ab?
fn differs(p: &[u8; 4], background: &[u8; 4], tolerance: u32) -> bool {
    if tolerance == 0 {
        return p != background;
    }
    let d: u32 = p.iter().zip(background.iter()).map(|(a, b)| u32::from(a.abs_diff(*b))).sum();
    d >= tolerance
}

/// Zellenzahl für `extent` pt (mindestens 1, auch bei `NaN` oder negativen Werten).
fn cells(extent: f32) -> usize {
    let n = (extent / GRID_CELL_PT).ceil();
    if n.is_finite() && n >= 1.0 { (n as usize).min(1 << 20) } else { 1 }
}

impl OccupancyGrid {
    /// Leeres Raster für eine sichtbare Seite `width × height` pt (z. B. die virtuelle neue Seite).
    pub fn empty(width: f32, height: f32) -> OccupancyGrid {
        let (page_width, page_height) = (sane(width), sane(height));
        let (cols, rows) = (cells(page_width), cells(page_height));
        OccupancyGrid { page_width, page_height, cols, rows, bits: vec![0; (cols * rows).div_ceil(64)] }
    }

    /// Raster aus dem Forced-Color-Rendering der Seite (`dpi` wie in
    /// [`pida_render::ForcedColorOptions`]; die Seite liegt unskaliert an der linken oberen Ecke). Ein
    /// Bildpunkt zählt als Inhalt, wenn er um mindestens `tolerance` vom Hintergrund abweicht
    /// ([`crate::SCAN_TOLERANCE`] wie der Rasterscan); seine Zelle ist dann belegt.
    pub fn from_forced_color(
        image: &RgbaImage,
        page_width: f32,
        page_height: f32,
        dpi: f32,
        background: [u8; 4],
        tolerance: u32,
    ) -> OccupancyGrid {
        let mut grid = OccupancyGrid::empty(page_width, page_height);
        let scale = if dpi.is_finite() && dpi > 0.0 { dpi / 72.0 } else { 1.0 };
        // Spalte der Rasterzelle je Bildspalte (einmal berechnet)
        let col_of: Vec<usize> =
            (0..image.width).map(|x| (((x as f32 + 0.5) / scale / GRID_CELL_PT) as usize).min(grid.cols - 1)).collect();
        for y in 0..image.height {
            let row = (((y as f32 + 0.5) / scale / GRID_CELL_PT) as usize).min(grid.rows - 1);
            let pixels = image.row(y).as_chunks::<4>().0;
            let mut x = 0usize;
            while x < pixels.len() {
                let col = col_of[x];
                if grid.get(col, row) {
                    // Zelle schon belegt: Rest der Zelle überspringen
                    while x < pixels.len() && col_of[x] == col {
                        x += 1;
                    }
                    continue;
                }
                if differs(&pixels[x], &background, tolerance) {
                    grid.set(col, row);
                }
                x += 1;
            }
        }
        grid
    }

    /// Sichtbare Seitengröße in pt.
    pub fn page_size(&self) -> (f32, f32) {
        (self.page_width, self.page_height)
    }

    /// Spalten und Zeilen.
    pub fn dimensions(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// Speicherbedarf des Bitsets in Bytes.
    pub fn byte_len(&self) -> usize {
        self.bits.len() * 8
    }

    /// Zelle belegt? (`col`, `row` außerhalb = frei)
    pub fn get(&self, col: usize, row: usize) -> bool {
        if col >= self.cols || row >= self.rows {
            return false;
        }
        let i = row * self.cols + col;
        self.bits[i / 64] & (1u64 << (i % 64)) != 0
    }

    /// Zelle als belegt markieren (außerhalb ohne Wirkung).
    pub fn set(&mut self, col: usize, row: usize) {
        if col >= self.cols || row >= self.rows {
            return;
        }
        let i = row * self.cols + col;
        self.bits[i / 64] |= 1u64 << (i % 64);
    }

    /// Belegt alle Zellen, die `area` (`PAGE_ABS`) berühren (Tests, synthetische Raster).
    pub fn fill(&mut self, area: Rect) {
        if let Some((c0, c1, r0, r1)) = self.cell_range(area) {
            for r in r0..r1 {
                for c in c0..c1 {
                    self.set(c, r);
                }
            }
        }
    }

    /// Anzahl belegter Zellen.
    pub fn count_occupied(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Zellbereich `[c0, c1) × [r0, r1)`, den `area` (`PAGE_ABS`) berührt; `None` = außerhalb bzw. leer.
    /// Kanten werden um 0,01 pt nach innen genommen (ein Block, der eine Zellkante nur berührt, belegt
    /// die Nachbarzelle nicht).
    fn cell_range(&self, area: Rect) -> Option<(usize, usize, usize, usize)> {
        let a = area.normalized();
        let eps = 0.01;
        let (left, right) = (a.llx.max(0.0) + eps, a.urx.min(self.page_width) - eps);
        let (bottom, top) = (a.lly.max(0.0) + eps, a.ury.min(self.page_height) - eps);
        if !(left < right && bottom < top) {
            return None;
        }
        let c0 = (left / GRID_CELL_PT).floor() as usize;
        let c1 = ((right / GRID_CELL_PT).floor() as usize + 1).min(self.cols);
        // Zeile 0 = oben: y_top = h − y
        let r0 = ((self.page_height - top) / GRID_CELL_PT).floor().max(0.0) as usize;
        let r1 = (((self.page_height - bottom) / GRID_CELL_PT).floor() as usize + 1).min(self.rows);
        (c0 < c1 && r0 < r1).then_some((c0, c1, r0, r1))
    }

    /// Überdeckt `area` (`PAGE_ABS`, z. B. das Rechteck des Signaturblocks) Inhalt? Liest nur das Bitset.
    pub fn occupied(&self, area: Rect) -> bool {
        let Some((c0, c1, r0, r1)) = self.cell_range(area) else { return false };
        (r0..r1).any(|r| (c0..c1).any(|c| self.get(c, r)))
    }

    /// Zellrechteck → `PAGE_ABS` (an der Seite geklemmt).
    fn rect_of(&self, c0: usize, c1: usize, r0: usize, r1: usize) -> Rect {
        let left = c0 as f32 * GRID_CELL_PT;
        let right = (c1 as f32 * GRID_CELL_PT).min(self.page_width);
        let top = self.page_height - r0 as f32 * GRID_CELL_PT;
        let bottom = (self.page_height - r1 as f32 * GRID_CELL_PT).max(0.0);
        Rect { llx: left, lly: bottom, urx: right, ury: top }
    }

    /// Begrenzungsrahmen aller belegten Zellen (`None` = leere Seite).
    pub fn content_bounds(&self) -> Option<Rect> {
        let (mut c0, mut c1, mut r0, mut r1) = (usize::MAX, 0usize, usize::MAX, 0usize);
        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.get(c, r) {
                    c0 = c0.min(c);
                    c1 = c1.max(c + 1);
                    r0 = r0.min(r);
                    r1 = r1.max(r + 1);
                }
            }
        }
        (c0 < c1).then(|| self.rect_of(c0, c1, r0, r1))
    }

    /// Raster mit um `radius` Zellen vergrößerter Belegung (Abstand freier Flächen zum Inhalt).
    fn dilated(&self, radius: usize) -> Vec<bool> {
        let (cols, rows) = (self.cols, self.rows);
        let mut horizontal = vec![false; cols * rows];
        for r in 0..rows {
            // gleitendes Fenster über die Zeile
            let mut last_occupied: Option<usize> = None;
            let mut next_occupied = vec![usize::MAX; cols];
            let mut next = usize::MAX;
            for c in (0..cols).rev() {
                if self.get(c, r) {
                    next = c;
                }
                next_occupied[c] = next;
            }
            for c in 0..cols {
                if self.get(c, r) {
                    last_occupied = Some(c);
                }
                let near_left = last_occupied.is_some_and(|l| c - l <= radius);
                let near_right = next_occupied[c] != usize::MAX && next_occupied[c] - c <= radius;
                horizontal[r * cols + c] = near_left || near_right;
            }
        }
        let mut out = vec![false; cols * rows];
        for c in 0..cols {
            let mut last: Option<usize> = None;
            let mut next_in_col = vec![usize::MAX; rows];
            let mut next = usize::MAX;
            for r in (0..rows).rev() {
                if horizontal[r * cols + c] {
                    next = r;
                }
                next_in_col[r] = next;
            }
            for r in 0..rows {
                if horizontal[r * cols + c] {
                    last = Some(r);
                }
                let near_up = last.is_some_and(|l| r - l <= radius);
                let near_down = next_in_col[r] != usize::MAX && next_in_col[r] - r <= radius;
                out[r * cols + c] = near_up || near_down;
            }
        }
        out
    }
}

/// Endliche, nicht negative Größe (sonst 0).
fn sane(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v } else { 0.0 }
}

/// Achse einer Einrastlinie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuideAxis {
    /// senkrechte Linie bei `x = at`
    X,
    /// waagrechte Linie bei `y = at`
    Y,
}

/// Art einer Einrastlinie. `X`-Linien gelten für die linke (`ContentLeft`, `MarginLeft`) bzw. rechte
/// Blockkante (`ContentRight`, `MarginRight`) bzw. die Blockmitte (`CenterX`); `Y`-Linien für die Ober-
/// (`ContentTop`, `BelowContent`, `MarginTop`) bzw. Unterkante (`ContentBottom`, `MarginBottom`) bzw.
/// Mitte (`CenterY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuideKind {
    /// linke Kante des Inhaltsrahmens
    ContentLeft,
    /// rechte Kante des Inhaltsrahmens
    ContentRight,
    /// Oberkante des Inhaltsrahmens
    ContentTop,
    /// Unterkante des Inhaltsrahmens
    ContentBottom,
    /// waagrechte Seitenmitte
    CenterX,
    /// senkrechte Seitenmitte
    CenterY,
    /// Oberkante „unter dem Text“ ([`BELOW_CONTENT_PT`] unter dem Inhalt)
    BelowContent,
    /// linker Rand (nur ohne Inhalt)
    MarginLeft,
    /// rechter Rand (nur ohne Inhalt)
    MarginRight,
    /// oberer Rand (nur ohne Inhalt)
    MarginTop,
    /// unterer Rand (nur ohne Inhalt)
    MarginBottom,
}

/// Einrastlinie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    /// Achse
    pub axis: GuideAxis,
    /// Lage in pt (`PAGE_ABS`)
    pub at: f32,
    /// Art
    pub kind: GuideKind,
}

/// Optionen von [`page_layout`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutOptions {
    /// Abstand freier Flächen zum Inhalt (pt)
    pub gap: f32,
    /// Abstand der Linie „unter dem Text“ (pt)
    pub below_content: f32,
    /// Rand der Einrastlinien ohne Inhalt (pt)
    pub margin: f32,
    /// Höchstzahl freier Flächen
    pub max_free: usize,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        LayoutOptions {
            gap: FREE_GAP_PT,
            below_content: BELOW_CONTENT_PT,
            margin: EMPTY_PAGE_MARGIN_PT,
            max_free: MAX_FREE_RECTS,
        }
    }
}

/// Layout einer Seite (ADR 0004 §3.3 `PageLayoutView`).
#[derive(Debug, Clone, PartialEq)]
pub struct PageLayout {
    /// Seite (0-basiert; `== Seitenzahl` = virtuelle neue Seite)
    pub page: usize,
    /// Begrenzungsrahmen des Inhalts (`None` = leere Seite)
    pub content: Option<Rect>,
    /// freie Rechtecke, in die der Block in aktueller Größe passt (Abstand [`LayoutOptions::gap`] zum
    /// Inhalt), absteigend nach Eignung (zuerst unter dem Inhalt rechts unten), höchstens
    /// [`LayoutOptions::max_free`]
    pub free: Vec<Rect>,
    /// Einrastlinien
    pub guides: Vec<Guide>,
}

/// Layout der Seite `page` aus ihrem Belegungsraster: Inhaltsrahmen, freie Flächen für einen Block
/// `block` und Einrastlinien. Rein (kein Renderer).
///
/// Freie Flächen sind die **maximalen** leeren Rechtecke des um `gap` vergrößerten Rasters, in die der
/// Block passt; sortiert nach Eignung: zuerst solche unter dem Inhalt, dann nach dem Abstand ihrer
/// rechten unteren Ecke zur rechten unteren Seitenecke, dann nach Fläche.
pub fn page_layout(grid: &OccupancyGrid, page: usize, block: BlockGeometry, opts: &LayoutOptions) -> PageLayout {
    let (w, h) = grid.page_size();
    let content = grid.content_bounds();
    let free = free_rects(grid, content, block, opts);
    let mut guides = vec![
        Guide { axis: GuideAxis::X, at: w / 2.0, kind: GuideKind::CenterX },
        Guide { axis: GuideAxis::Y, at: h / 2.0, kind: GuideKind::CenterY },
    ];
    match content {
        Some(c) => {
            guides.push(Guide { axis: GuideAxis::X, at: c.llx, kind: GuideKind::ContentLeft });
            guides.push(Guide { axis: GuideAxis::X, at: c.urx, kind: GuideKind::ContentRight });
            guides.push(Guide { axis: GuideAxis::Y, at: c.ury, kind: GuideKind::ContentTop });
            guides.push(Guide { axis: GuideAxis::Y, at: c.lly, kind: GuideKind::ContentBottom });
            let below = c.lly - opts.below_content;
            // nur, wenn der Block darunter noch auf die Seite passt
            if below - block.height.max(0.0) >= 0.0 {
                guides.push(Guide { axis: GuideAxis::Y, at: below, kind: GuideKind::BelowContent });
            }
        }
        None => {
            let m = opts.margin;
            guides.push(Guide { axis: GuideAxis::X, at: m.min(w), kind: GuideKind::MarginLeft });
            guides.push(Guide { axis: GuideAxis::X, at: (w - m).max(0.0), kind: GuideKind::MarginRight });
            guides.push(Guide { axis: GuideAxis::Y, at: (h - m).max(0.0), kind: GuideKind::MarginTop });
            guides.push(Guide { axis: GuideAxis::Y, at: m.min(h), kind: GuideKind::MarginBottom });
        }
    }
    PageLayout { page, content, free, guides }
}

/// Maximale freie Rechtecke (siehe [`page_layout`]).
fn free_rects(grid: &OccupancyGrid, content: Option<Rect>, block: BlockGeometry, opts: &LayoutOptions) -> Vec<Rect> {
    let (cols, rows) = grid.dimensions();
    let (w, h) = grid.page_size();
    let fits = |r: &Rect| r.width() + 0.01 >= block.width.max(0.0) && r.height() + 0.01 >= block.height.max(0.0);
    if content.is_none() {
        let whole = Rect { llx: 0.0, lly: 0.0, urx: w, ury: h };
        return if fits(&whole) && opts.max_free > 0 { vec![whole] } else { Vec::new() };
    }
    let radius = (opts.gap.max(0.0) / GRID_CELL_PT).ceil() as usize;
    let blocked = grid.dilated(radius);
    let is_blocked = |c: usize, r: usize| blocked[r * cols + c];
    let mut heights = vec![0u32; cols];
    let mut found: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut stack: Vec<(usize, u32)> = Vec::with_capacity(cols);
    for r in 0..rows {
        for (c, height) in heights.iter_mut().enumerate() {
            *height = if is_blocked(c, r) { 0 } else { *height + 1 };
        }
        // nach unten maximal, wenn die nächste Zeile im Bereich belegt ist (bzw. Seitenende)
        let below_blocked = |c0: usize, c1: usize| r + 1 >= rows || (c0..c1).any(|c| is_blocked(c, r + 1));
        stack.clear();
        for c in 0..=cols {
            let cur = heights.get(c).copied().unwrap_or(0);
            let mut start = c;
            while let Some(&(s, ht)) = stack.last() {
                if ht < cur {
                    break;
                }
                stack.pop();
                if ht > cur && below_blocked(s, c) {
                    let r0 = r + 1 - ht as usize;
                    let rect = grid.rect_of(s, c, r0, r + 1);
                    if fits(&rect) {
                        found.push((s, c, r0, r + 1));
                    }
                }
                start = s;
            }
            if cur > 0 {
                stack.push((start, cur));
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    let below = content.map(|c| c.lly).unwrap_or(h);
    let mut rects: Vec<Rect> = found.into_iter().map(|(c0, c1, r0, r1)| grid.rect_of(c0, c1, r0, r1)).collect();
    rects.sort_by(|a, b| {
        let key = |r: &Rect| {
            let under = r.ury <= below + 0.01;
            let corner = (w - r.urx) + r.lly;
            (!under, corner, -(r.width() * r.height()))
        };
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1)).then(ka.2.total_cmp(&kb.2))
    });
    rects.truncate(opts.max_free);
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f32, f32) = (595.0, 842.0);
    const BLOCK: BlockGeometry = BlockGeometry { width: 230.0, height: 49.0 };

    fn rect(llx: f32, lly: f32, urx: f32, ury: f32) -> Rect {
        Rect { llx, lly, urx, ury }
    }

    fn inside(inner: &Rect, outer: &Rect) -> bool {
        inner.llx >= outer.llx - 0.01
            && inner.urx <= outer.urx + 0.01
            && inner.lly >= outer.lly - 0.01
            && inner.ury <= outer.ury + 0.01
    }

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.llx < b.urx && b.llx < a.urx && a.lly < b.ury && b.lly < a.ury
    }

    /// Leere Seite: kein Inhalt, die ganze Seite ist frei, Randlinien 36 pt und Mitte.
    #[test]
    fn empty_page_layout() {
        let grid = OccupancyGrid::empty(A4.0, A4.1);
        assert_eq!(grid.dimensions(), (199, 281));
        assert!(grid.byte_len() <= 7 * 1024 + 64, "A4 ≈ 7 KB: {}", grid.byte_len());
        let l = page_layout(&grid, 0, BLOCK, &LayoutOptions::default());
        assert_eq!(l.content, None);
        assert_eq!(l.free, vec![rect(0.0, 0.0, A4.0, A4.1)]);
        let kinds: Vec<GuideKind> = l.guides.iter().map(|g| g.kind).collect();
        for k in [
            GuideKind::CenterX,
            GuideKind::CenterY,
            GuideKind::MarginLeft,
            GuideKind::MarginRight,
            GuideKind::MarginTop,
            GuideKind::MarginBottom,
        ] {
            assert!(kinds.contains(&k), "{k:?} fehlt: {kinds:?}");
        }
        let at = |k: GuideKind| l.guides.iter().find(|g| g.kind == k).unwrap().at;
        assert_eq!(at(GuideKind::MarginLeft), 36.0);
        assert_eq!(at(GuideKind::MarginRight), A4.0 - 36.0);
        assert_eq!(at(GuideKind::MarginTop), A4.1 - 36.0);
        assert_eq!(at(GuideKind::MarginBottom), 36.0);
        assert_eq!(at(GuideKind::CenterX), A4.0 / 2.0);
        // zu großer Block: keine freie Fläche
        let huge = BlockGeometry { width: 600.0, height: 10.0 };
        assert!(page_layout(&grid, 0, huge, &LayoutOptions::default()).free.is_empty());
    }

    /// Text oben auf der Seite: Inhaltsrahmen, Linien an seinen Kanten und „unter dem Text“, die beste
    /// freie Fläche liegt unter dem Inhalt (mit 6 pt Abstand) und reicht bis zur rechten unteren Ecke.
    #[test]
    fn content_at_the_top() {
        let mut grid = OccupancyGrid::empty(A4.0, A4.1);
        let text = rect(72.0, 600.0, 523.0, 770.0);
        grid.fill(text);
        let l = page_layout(&grid, 2, BLOCK, &LayoutOptions::default());
        assert_eq!(l.page, 2);
        let c = l.content.unwrap();
        // Zellraster 3 pt: Rahmen umschließt den Inhalt mit höchstens einer Zelle Spiel
        assert!(inside(&text, &c), "{c:?}");
        assert!(c.llx >= text.llx - 3.0 && c.urx <= text.urx + 3.0 && c.lly >= text.lly - 3.0, "{c:?}");
        let at = |k: GuideKind| l.guides.iter().find(|g| g.kind == k).map(|g| g.at);
        assert_eq!(at(GuideKind::ContentLeft), Some(c.llx));
        assert_eq!(at(GuideKind::ContentBottom), Some(c.lly));
        assert_eq!(at(GuideKind::BelowContent), Some(c.lly - 18.0));
        assert_eq!(at(GuideKind::MarginLeft), None, "Ränder nur ohne Inhalt");
        assert!(!l.free.is_empty() && l.free.len() <= 16);
        let best = l.free[0];
        assert!(best.ury <= c.lly - 6.0 + 0.01, "Abstand zum Inhalt: {best:?}");
        assert!((best.urx - A4.0).abs() < 0.01 && best.lly.abs() < 0.01, "rechts unten: {best:?}");
        for f in &l.free {
            assert!(f.width() + 0.01 >= BLOCK.width && f.height() + 0.01 >= BLOCK.height, "{f:?}");
            assert!(!grid.occupied(*f), "frei heißt unbelegt: {f:?}");
            let grown = rect(f.llx + 5.9, f.lly + 5.9, f.urx - 5.9, f.ury - 5.9);
            assert!(!overlaps(&grown, &text), "{f:?}");
        }
    }

    /// Mehrere Inhaltsinseln: höchstens 16 Flächen, alle maximal (keine liegt in einer anderen).
    #[test]
    fn free_rects_are_maximal_and_capped() {
        let mut grid = OccupancyGrid::empty(A4.0, A4.1);
        // versetzte Inseln (deterministisch gestreut) ergeben viele maximale Rechtecke
        let mut seed = 7u32;
        for _ in 0..40 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let x = (seed >> 8) % 560;
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let y = (seed >> 8) % 800;
            grid.fill(rect(x as f32, y as f32, x as f32 + 10.0, y as f32 + 10.0));
        }
        let small = BlockGeometry { width: 40.0, height: 20.0 };
        let l = page_layout(&grid, 0, small, &LayoutOptions::default());
        assert_eq!(l.free.len(), 16);
        for (i, a) in l.free.iter().enumerate() {
            for (j, b) in l.free.iter().enumerate() {
                assert!(i == j || !inside(a, b) || a == b, "{a:?} in {b:?}");
            }
        }
    }

    /// Überdeckung an den Grenzen: Berühren ist keine Überdeckung, 1 pt hinein schon.
    #[test]
    fn overlap_at_boundaries() {
        let mut grid = OccupancyGrid::empty(A4.0, A4.1);
        // Zellkanten: x = 3k, y = 842 − 3k (Zeile 0 oben)
        grid.fill(rect(300.0, 302.0, 330.0, 332.0));
        assert!(grid.occupied(rect(310.0, 310.0, 320.0, 320.0)));
        assert!(!grid.occupied(rect(330.0, 302.0, 400.0, 332.0)), "berührt rechts");
        assert!(!grid.occupied(rect(300.0, 332.0, 330.0, 400.0)), "berührt oben");
        assert!(grid.occupied(rect(329.0, 302.0, 400.0, 332.0)));
        assert!(grid.occupied(rect(300.0, 331.0, 330.0, 400.0)));
        assert!(!grid.occupied(rect(0.0, 0.0, 100.0, 100.0)));
        // außerhalb der Seite, leer und NaN: nie belegt, keine Panik
        assert!(!grid.occupied(rect(-100.0, -100.0, -1.0, -1.0)));
        assert!(!grid.occupied(rect(1000.0, 1000.0, 2000.0, 2000.0)));
        assert!(!grid.occupied(rect(f32::NAN, 0.0, f32::NAN, 10.0)));
        assert!(!grid.occupied(rect(10.0, 10.0, 10.0, 10.0)));
        let _ = OccupancyGrid::empty(f32::NAN, f32::INFINITY);
    }

    /// Raster aus einem synthetischen Forced-Color-Bild (72 dpi, Zeile 0 = oben); Kantenglättung unter
    /// der Toleranz zählt nicht.
    #[test]
    fn grid_from_forced_color_image() {
        let bg = [152, 254, 52, 255];
        let mut img = RgbaImage::filled(100, 60, bg);
        // „Text“ oben links: x 10…19, Bildzeilen 5…9 → PAGE_ABS y 51…55
        for y in 5..10 {
            for x in 10..20 {
                img.set_pixel(x, y, [234, 14, 184, 211]);
            }
        }
        // fast Hintergrund (Kantenrest) unten rechts
        img.set_pixel(90, 50, [153, 254, 53, 255]);
        let grid = OccupancyGrid::from_forced_color(&img, 100.0, 60.0, 72.0, bg, crate::SCAN_TOLERANCE);
        let c = grid.content_bounds().unwrap();
        assert_eq!((c.llx, c.urx), (9.0, 21.0));
        assert_eq!((c.lly, c.ury), (48.0, 57.0));
        assert!(!grid.occupied(rect(80.0, 0.0, 100.0, 20.0)));
        // doppelte Auflösung: gleiche Belegung
        let mut img2 = RgbaImage::filled(200, 120, bg);
        for y in 10..20 {
            for x in 20..40 {
                img2.set_pixel(x, y, [0, 0, 0, 255]);
            }
        }
        let g2 = OccupancyGrid::from_forced_color(&img2, 100.0, 60.0, 144.0, bg, crate::SCAN_TOLERANCE);
        assert_eq!(g2.content_bounds(), grid.content_bounds());
    }
}
