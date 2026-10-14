//! Koordinatenräume (PDF-Over `SignaturePanel.U`, docs/analyse/04 §4.7).

use pdfover_pdf_cos::PageInfo;

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

impl PageTransform {
    /// Für eine Seite und eine Darstellung.
    pub fn new(page: &PageInfo, screen: ScreenGeometry) -> Self {
        let (page_width, page_height) = page.visible_size();
        Self { page_width, page_height, screen }
    }

    /// Wandelt `p` von Raum `from` nach `to`.
    pub fn convert(&self, p: Point, from: CoordSpace, to: CoordSpace) -> Point {
        todo!("Paket F")
    }

    /// Klemmt die linke obere Ecke eines Blocks `w × h` auf die Seite (F-35):
    /// `x ∈ [0, W − w]`, `y ∈ [h, H]` (PAGE_ABS).
    pub fn clamp_block(&self, top_left: Point, w: f32, h: f32) -> Point {
        todo!("Paket F")
    }
}
