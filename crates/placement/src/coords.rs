//! Koordinatenräume (PDF-Over `SignaturePanel.U`, docs/analyse/04 §4.7).

use pdfover_pdf_cos::PageInfo;

use crate::{KEYBOARD_COARSE_STEP_PT, KEYBOARD_FINE_STEP_PT, KEYBOARD_STEP_PT};

/// Punkt.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Point {
    /// x
    pub x: f32,
    /// y
    pub y: f32,
}

/// Koordinatenraum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CoordSpace {
    /// Ursprung links **unten** der sichtbaren (gedrehten) Seite, y nach oben, pt – das `x`/`y`
    /// für PDF-AS.
    PageAbs,
    /// Ursprung links oben, y nach unten, pt.
    PageRel,
    /// gerendertes Seitenbild, px = pt · `page_to_image`.
    Image,
    /// Bildschirm relativ zum gezeichneten Bild, px = Image · `image_to_screen`.
    ScreenRel,
    /// Bildschirm relativ zum Panel (ScreenRel + Versatz).
    ScreenAbs,
}

/// Geometrie der Darstellung in der UI.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScreenGeometry {
    /// Skalierung Seite → Bild (px je pt).
    pub page_to_image: f32,
    /// Skalierung Bild → Bildschirm.
    pub image_to_screen: f32,
    /// Versatz des Bildes im Panel (zentriert).
    pub offset: Point,
}

impl ScreenGeometry {
    /// Identität (1 px = 1 pt, kein Versatz).
    pub fn identity() -> Self {
        Self { page_to_image: 1.0, image_to_screen: 1.0, offset: Point::default() }
    }

    /// Einpassen wie `SignaturePanel.paint`: Seitenbild `image_w × image_h` px (gerendert mit
    /// `page_to_image` px/pt) seitenverhältnistreu in ein Panel `panel_w × panel_h` px,
    /// `image_to_screen = min(panel_w/image_w, panel_h/image_h)`, zentriert mit ganzzahligem
    /// Versatz (`(panel − (int)(image · scale)) / 2`, Ganzzahldivision).
    pub fn fit(image_w: u32, image_h: u32, page_to_image: f32, panel_w: u32, panel_h: u32) -> Self {
        if image_w == 0 || image_h == 0 {
            return Self { page_to_image, image_to_screen: 1.0, offset: Point::default() };
        }
        let scale = (panel_w as f32 / image_w as f32).min(panel_h as f32 / image_h as f32);
        let render_w = (image_w as f32 * scale) as i64;
        let render_h = (image_h as f32 * scale) as i64;
        let off_x = (i64::from(panel_w) - render_w) / 2;
        let off_y = (i64::from(panel_h) - render_h) / 2;
        Self { page_to_image, image_to_screen: scale, offset: Point { x: off_x as f32, y: off_y as f32 } }
    }
}

/// Transformation zwischen den Räumen für eine Seite.
#[derive(Debug, Clone, PartialEq)]
pub struct PageTransform {
    /// Sichtbare Seitenbreite in pt (CropBox, gedreht).
    pub page_width: f32,
    /// Sichtbare Seitenhöhe in pt.
    pub page_height: f32,
    /// Darstellung.
    pub screen: ScreenGeometry,
}

/// Klemmt eine linke obere Ecke (PAGE_ABS) eines Blocks `w × h` auf eine Seite `pw × ph`:
/// `x ∈ [0, pw − w]`, `y ∈ [h, ph]`. Ist der Block breiter bzw. höher als die Seite, liegt er
/// links bzw. oben an (PDF-Over `clamp` lieferte hier `pw − w` < 0 bzw. `h` > `ph`).
pub(crate) fn clamp_top_left(p: Point, pw: f32, ph: f32, w: f32, h: f32) -> Point {
    let x = if pw - w < 0.0 { 0.0 } else { p.x.clamp(0.0, pw - w) };
    let y = if h > ph { ph } else { p.y.clamp(h, ph) };
    Point { x, y }
}

/// Tastaturschritt der Positionierung (Plan §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum KeyStep {
    /// Pfeiltaste: 15 pt (Original `SIGNATURE_KEYBOARD_POSITIONING_OFFSET`)
    #[default]
    Normal,
    /// Shift + Pfeil: 1 pt
    Fine,
    /// Strg/⌘ + Pfeil: 50 pt
    Coarse,
}

impl KeyStep {
    /// Schrittweite in pt.
    pub fn pt(self) -> f32 {
        match self {
            KeyStep::Normal => KEYBOARD_STEP_PT,
            KeyStep::Fine => KEYBOARD_FINE_STEP_PT,
            KeyStep::Coarse => KEYBOARD_COARSE_STEP_PT,
        }
    }
}

impl PageTransform {
    /// Für eine Seite und eine Darstellung.
    pub fn new(page: &PageInfo, screen: ScreenGeometry) -> Self {
        let (page_width, page_height) = page.visible_size();
        Self { page_width, page_height, screen }
    }

    fn total_scale(&self) -> f32 {
        self.screen.page_to_image * self.screen.image_to_screen
    }

    /// Wandelt `p` von Raum `from` nach `to`.
    ///
    /// Wie `SignaturePanel.scale`: PAGE_ABS ↔ PAGE_REL spiegelt y an der Seitenhöhe,
    /// PAGE_REL → IMAGE skaliert mit `page_to_image`, IMAGE → SCREEN_REL mit `image_to_screen`,
    /// SCREEN_REL → SCREEN_ABS addiert den Versatz. Die Skalierungen müssen > 0 sein.
    pub fn convert(&self, p: Point, from: CoordSpace, to: CoordSpace) -> Point {
        if from == to {
            return p;
        }
        let k1 = self.screen.page_to_image;
        let k = self.total_scale();
        let off = self.screen.offset;
        let rel = match from {
            CoordSpace::PageAbs => Point { x: p.x, y: self.page_height - p.y },
            CoordSpace::PageRel => p,
            CoordSpace::Image => Point { x: p.x / k1, y: p.y / k1 },
            CoordSpace::ScreenRel => Point { x: p.x / k, y: p.y / k },
            CoordSpace::ScreenAbs => Point { x: (p.x - off.x) / k, y: (p.y - off.y) / k },
        };
        match to {
            CoordSpace::PageAbs => Point { x: rel.x, y: self.page_height - rel.y },
            CoordSpace::PageRel => rel,
            CoordSpace::Image => Point { x: rel.x * k1, y: rel.y * k1 },
            CoordSpace::ScreenRel => Point { x: rel.x * k, y: rel.y * k },
            CoordSpace::ScreenAbs => Point { x: rel.x * k + off.x, y: rel.y * k + off.y },
        }
    }

    /// Wandelt eine Ausdehnung (Breite, Höhe) zwischen Räumen um – nur Skalierung, kein
    /// Spiegeln, kein Versatz (wie `scale(…, PAGE_REL, SCREEN_REL)` für die Blockgröße).
    pub fn convert_size(&self, size: Point, from: CoordSpace, to: CoordSpace) -> Point {
        let factor = |space: CoordSpace| match space {
            CoordSpace::PageAbs | CoordSpace::PageRel => 1.0,
            CoordSpace::Image => self.screen.page_to_image,
            CoordSpace::ScreenRel | CoordSpace::ScreenAbs => self.total_scale(),
        };
        let f = factor(to) / factor(from);
        Point { x: size.x * f, y: size.y * f }
    }

    /// Klemmt die linke obere Ecke eines Blocks `w × h` auf die Seite (F-35):
    /// `x ∈ [0, W − w]`, `y ∈ [h, H]` (PAGE_ABS).
    ///
    /// Ist der Block breiter (höher) als die Seite, liegt er links (oben) an.
    pub fn clamp_block(&self, top_left: Point, w: f32, h: f32) -> Point {
        clamp_top_left(top_left, self.page_width, self.page_height, w, h)
    }

    /// Verschiebt die linke obere Ecke um (`dx`, `dy`) pt (PAGE_ABS, y nach oben) und klemmt.
    pub fn nudge(&self, top_left: Point, dx: f32, dy: f32, w: f32, h: f32) -> Point {
        self.clamp_block(Point { x: top_left.x + dx, y: top_left.y + dy }, w, h)
    }
}
