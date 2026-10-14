//! `pdfover-placement` – Koordinatenmodell und Positionierung (Plan §6.7, §6.8;
//! docs/analyse/04 §4–§5).
//!
//! Alle Funktionen arbeiten auf **Datenstrukturen** (`PageInfo`, `SigField`, `ImageInvocation` aus
//! `pdfover-pdf-cos`; `Renderer` aus `pdfover-render`), damit Paket F unabhängig von Paket A testen
//! kann (ADR 0001 §5.3).
//!
//! Ownership: Paket **F** (zusammen mit `pdfover-render`).

pub mod coords;
pub mod placeholder;
pub mod position;

pub use coords::{CoordSpace, PageTransform, Point, ScreenGeometry};
pub use placeholder::{
    EmptyFieldCandidate, PlaceholderMode, PlaceholderQuery, QrPayload, QrPlaceholder, decode_qr, find_empty_sig_fields,
    find_qr_placeholders, parse_qr_payload, select_placeholder,
};
pub use position::{
    BlockGeometry, Coord, FreeSpaceScanner, PageSelector, Placement, PlacementNotice, PositionContext, RasterScanner,
    ResolvedPosition, SignaturePosition, TablePos, WidthDecision, appearance_rotation, auto_place,
    default_start_position, resolve_position, resolve_width, widget_rect,
};

/// Tastatur-Schrittweiten (Plan §6.7): Pfeile 15 pt (Original), Shift 1 pt, Strg/⌘ 50 pt.
pub const KEYBOARD_STEP_PT: f32 = 15.0;
/// Feinschritt.
pub const KEYBOARD_FINE_STEP_PT: f32 = 1.0;
/// Grobschritt.
pub const KEYBOARD_COARSE_STEP_PT: f32 = 50.0;
/// Seitenränder der PDF-AS-Positionierung (`MARGIN_H`, `MARGIN_V`).
pub const MARGIN_H: f32 = 50.0;
/// Vertikaler Abstand unter dem untersten Inhalt bzw. zum oberen Rand.
pub const MARGIN_V: f32 = 20.0;

/// Fehler.
#[derive(Debug, thiserror::Error)]
pub enum PlacementError {
    /// Positionsstring ungültig (`TablePos`-Syntax).
    #[error("ungültiger Positionsstring: {0}")]
    InvalidPosition(String),
    /// Seite existiert nicht.
    #[error("Seite {0} existiert nicht")]
    NoSuchPage(usize),
    /// Kein passender Platzhalter im Modus STRICT (`error.pdf.stamp.09`).
    #[error("kein passender Signaturplatzhalter")]
    PlaceholderNotFound,
    /// Kein Platz und neue Seite nicht erlaubt (`sigblock.placement.less.space.failing`).
    #[error("nicht genug Platz für den Signaturblock")]
    NotEnoughSpace,
    /// Renderer-Fehler (Rasterscan, Bildobjekte).
    #[error(transparent)]
    Render(#[from] pdfover_render::RenderError),
}

/// Ergebnis-Alias.
pub type Result<T, E = PlacementError> = std::result::Result<T, E>;
