//! Positionsstring (`TablePos`) und PDF-AS-Positionierung (docs/analyse/04 §4.1–§4.5).

use std::fmt;
use std::str::FromStr;

use pdfover_pdf_cos::{PageInfo, Rect};
use pdfover_render::{ForcedColorOptions, Renderer};

use crate::coords::Point;
use crate::{PlacementError, Result};

/// Seitenmodus `p:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum PageSelector {
    /// `p:auto` (Default) – letzte Seite, Freiraumsuche.
    #[default]
    Auto,
    /// `p:new`
    New,
    /// `p:last`
    Last,
    /// `p:<n>` (1-basiert; `n > Seitenzahl` ⇒ neue Seite)
    Exact(u32),
}

/// Wert oder `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Coord {
    /// `auto`
    #[default]
    Auto,
    /// fester Wert in pt
    Value(f32),
}

/// PDF-AS-Positionsstring `p:..;x:..;y:..;w:..;f:..;r:..` (docs/analyse/04 §4.1).
/// `None` = Schlüssel nicht genannt (wichtig für [`TablePos::merge`]).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct TablePos {
    /// `p`
    pub page: Option<PageSelector>,
    /// `x` (linke Kante)
    pub x: Option<Coord>,
    /// `y` (Oberkante, von **unten** gemessen)
    pub y: Option<Coord>,
    /// `w` (≥ 1)
    pub w: Option<Coord>,
    /// `f` Fußzeilenhöhe (nur bei `y:auto` und nicht `p:new` wirksam)
    pub footer: Option<f32>,
    /// `r` Blockrotation (Vielfaches von 90, gegen den Uhrzeigersinn)
    pub rotation: Option<i32>,
}

/// Name des Positionsstrings in PDF-Over (`SignaturePosition`); identisch mit [`TablePos`].
pub type SignaturePosition = TablePos;

impl TablePos {
    /// Vollautomatisch (`p:auto;x:auto;y:auto`, was PDF-Over bei Auto-Positionierung übergibt).
    pub fn auto() -> Self {
        TablePos { page: Some(PageSelector::Auto), x: Some(Coord::Auto), y: Some(Coord::Auto), ..Default::default() }
    }

    /// Manuell auf Seite `page_1based` (PDF-Over: `p:%d;x:%f;y:%f`).
    pub fn manual(page_1based: u32, x: f32, y: f32) -> Self {
        TablePos {
            page: Some(PageSelector::Exact(page_1based)),
            x: Some(Coord::Value(x)),
            y: Some(Coord::Value(y)),
            ..Default::default()
        }
    }

    /// Parst die PDF-AS-Syntax; `k` genau ein Zeichen, sonst Fehler.
    pub fn parse(s: &str) -> Result<Self> {
        todo!("Paket F")
    }

    /// Request-Werte überschreiben **nur die genannten Schlüssel** des Profil-Defaults
    /// (`new TablePos(request, profileDefault)`).
    pub fn merge(request: &TablePos, profile_default: &TablePos) -> TablePos {
        TablePos {
            page: request.page.or(profile_default.page),
            x: request.x.or(profile_default.x),
            y: request.y.or(profile_default.y),
            w: request.w.or(profile_default.w),
            footer: request.footer.or(profile_default.footer),
            rotation: request.rotation.or(profile_default.rotation),
        }
    }
}

impl FromStr for TablePos {
    type Err = PlacementError;
    fn from_str(s: &str) -> Result<Self> {
        TablePos::parse(s)
    }
}

impl fmt::Display for TablePos {
    /// PDF-AS-Syntax (Werte mit Punkt als Dezimaltrenner).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("Paket F")
    }
}

/// Vom Nutzer gewählte Position (UI-Zustand, PAGE_ABS, linke obere Ecke).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Placement {
    /// 0-basierter Seitenindex; `== Seitenzahl` bedeutet „neue Seite“ (virtuelle Seite F-38).
    pub page_index: usize,
    /// linke Kante in pt
    pub x: f32,
    /// Oberkante in pt (von unten)
    pub y: f32,
    /// Blockbreite in pt
    pub width: f32,
    /// Blockhöhe in pt
    pub height: f32,
}

impl Placement {
    /// Als `TablePos` (`p:<page+1>;x;y`).
    pub fn to_table_pos(&self) -> TablePos {
        TablePos::manual(self.page_index as u32 + 1, self.x, self.y)
    }
}

/// Blockmaße.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockGeometry {
    /// Breite
    pub width: f32,
    /// Höhe
    pub height: f32,
}

/// Dokumentkontext der Positionierung.
#[derive(Debug, Clone, Copy)]
pub struct PositionContext<'a> {
    /// Seiten.
    pub pages: &'a [PageInfo],
    /// Dokument hat ≥ 1 signiertes Feld (neue Seite dann nicht erlaubt, docs/analyse/04 §4.5).
    pub already_signed: bool,
    /// Profil `SIGNED_NEWPAGE_FORCE`.
    pub signed_newpage_force: bool,
    /// `sigblock.placement.less.space.failing` (Fehler statt Ausweichen).
    pub less_space_failing: bool,
}

/// Ergebnis von [`resolve_width`] (vor dem Layout benötigt).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WidthDecision {
    /// 0-basierte Zielseite (vorläufig).
    pub page_index: usize,
    /// linke Kante
    pub x: f32,
    /// Blockbreite (AMTS ohne `w`: `pw − 2x`; Korrektur gegenüber PDF-AS: bei manuellem x gilt die
    /// Breite der Vorschau, Register)
    pub width: f32,
}

/// Hinweise an die UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlacementNotice {
    /// Neue Seite auf signiertem Dokument nicht erlaubt – letzte Seite verwendet (sichtbarer
    /// Hinweis statt stillem Ausweichen, Plan §6.1).
    NewPageNotAllowed,
    /// Block überdeckt Seiteninhalt (RTR 8852, Plan §6.1).
    OverlapsContent,
    /// Block überdeckt vorhandene Widgets.
    OverlapsWidget,
    /// Position wurde auf die Seite geklemmt.
    Clamped,
}

/// Endgültige Position.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedPosition {
    /// 0-basierter Seitenindex; bei `new_page` = Index der neu angehängten Seite.
    pub page_index: usize,
    /// Neue Seite anhängen (gleiche MediaBox und `/Rotate` wie die letzte Seite).
    pub new_page: bool,
    /// linke Kante (PAGE_ABS)
    pub x: f32,
    /// Oberkante (PAGE_ABS)
    pub y: f32,
    /// Breite
    pub width: f32,
    /// Höhe
    pub height: f32,
    /// Blockrotation `r`
    pub rotation: i32,
    /// Hinweise
    pub notices: Vec<PlacementNotice>,
}

/// Freiraumsuche: Abstand der untersten belegten Pixelzeile oberhalb der Fußzeile von der
/// Oberkante in pt (0 = leere Seite).
pub trait FreeSpaceScanner {
    /// Rasterscan einer Seite.
    fn lowest_content(&mut self, page_index: usize, footer: f32) -> Result<f32>;
}

/// Rasterscan über das Forced-Color-Rendering (docs/analyse/04 §4.4, Plan §6.7).
#[derive(Debug)]
pub struct RasterScanner<'a, R: Renderer + ?Sized> {
    /// Renderer des Dokuments.
    pub renderer: &'a R,
    /// Optionen (Signalfarbe, 72 dpi).
    pub options: ForcedColorOptions,
}

impl<R: Renderer + ?Sized> FreeSpaceScanner for RasterScanner<'_, R> {
    fn lowest_content(&mut self, page_index: usize, footer: f32) -> Result<f32> {
        todo!("Paket F")
    }
}

/// Schritt 1 von `adjustSignatureTableandCalculatePosition`: Seite, x und Breite.
pub fn resolve_width(pos: &TablePos, ctx: &PositionContext<'_>) -> Result<WidthDecision> {
    todo!("Paket F")
}

/// Schritt 2: endgültige Position mit der Blockhöhe aus dem Layout (Algorithmus docs/analyse/04 §4.3).
pub fn resolve_position(
    pos: &TablePos,
    ctx: &PositionContext<'_>,
    width: WidthDecision,
    block_height: f32,
    scanner: &mut dyn FreeSpaceScanner,
) -> Result<ResolvedPosition> {
    todo!("Paket F")
}

/// Bequemlichkeit: [`resolve_position`] mit [`RasterScanner`].
pub fn auto_place(
    renderer: &dyn Renderer,
    pos: &TablePos,
    ctx: &PositionContext<'_>,
    width: WidthDecision,
    block_height: f32,
) -> Result<ResolvedPosition> {
    let mut scanner = RasterScanner { renderer, options: ForcedColorOptions::default() };
    resolve_position(pos, ctx, width, block_height, &mut scanner)
}

/// Widget-`/Rect` aus der Position (unrotiert `[x, y−H, x+W, y]`; Rotation 90/180/270 nach
/// docs/analyse/04 §3.7; CropBox-Ursprung berücksichtigt – Korrektur, Register).
pub fn widget_rect(pos: &ResolvedPosition, page: &PageInfo) -> Rect {
    todo!("Paket F")
}

/// Rotation für die Holder-`/Matrix`: Blockrotation + Seitenrotation.
pub fn appearance_rotation(block_rotation: i32, page: &PageInfo) -> i32 {
    (block_rotation + i32::from(page.rotation.degrees())).rem_euclid(360)
}

/// Startposition der UI: x = 50 % Breite, y = 25 % Höhe, geklemmt (F-33).
pub fn default_start_position(page: &PageInfo, block: BlockGeometry) -> Point {
    todo!("Paket F")
}
