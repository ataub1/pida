//! `pdfover-placement` – Koordinatenmodell und Positionierung (Plan §6.7, §6.8;
//! docs/analyse/04 §4–§5).
//!
//! Alle Funktionen arbeiten auf **Datenstrukturen** (`PageInfo`, `SigField`, `ImageInvocation` aus
//! `pdfover-pdf-cos`; `Renderer` aus `pdfover-render`), damit Paket F unabhängig von Paket A testen
//! kann (ADR 0001 §5.3).
//!
//! # Ablauf
//!
//! 1. [`TablePos::parse`] und [`TablePos::merge`] (Request-Position über Profil-`pos`),
//! 2. [`resolve_width`] (Seite, x, Breite – vor dem Layout),
//! 3. Layout in `pdfover-sigblock` (Blockhöhe),
//! 4. [`resolve_position`] bzw. [`auto_place`] (Rasterscan, neue Seite, Hinweise),
//! 5. [`widget_rect`] und [`appearance_rotation`] für das Widget.
//!
//! QR-Platzhalter: [`find_qr_placeholders`] (Bilder aus dem Renderer, Namen/CTM aus `pdf-cos`)
//! und [`select_placeholder`]; leere Signaturfelder: [`find_empty_sig_fields`]. Startzustand der
//! UI-Positionierung: [`default_start_placement`] (letzte Seite, 50 %/25 %).
//!
//! # Kompatibilitätsregister (Paket F, Plan §6.5)
//!
//! | ID | Abweichung von PDF-AS | Entscheidung | Test |
//! |---|---|---|---|
//! | KR-F01 | AMTS-Breite ohne `w` hängt bei manuellem `x` von `x` ab (`pw − 2x`) | korrigiert: `pw − 100` wie bei automatischer Position; [`CompatMode::PdfasQuirks`] rechnet wie PDF-AS | `amts_width_corrected_and_quirk` |
//! | KR-F02 | CropBox-Ursprung wird im Widget-Rect ignoriert, Rect mit MediaBox-Maßen | korrigiert: Rect relativ zur CropBox (Ursprung und Maße), neue Seiten mit MediaBox; QR-Position relativ zur CropBox | `cropbox_origin_is_honoured_when_corrected`, `quirk_rect_ignores_cropbox_origin`, `widget_rect_lands_where_pdfium_shows_it` |
//! | KR-F03 | Platzhalter-IDs > 127 passen nie (Integer-`==`) | korrigiert (numerischer Vergleich) | `id_matching_without_integer_cache_bug`, `ids_above_127` |
//! | KR-F04 | Rasterscan gedrehter Seiten im ungedrehten Bildformat (Inhalt teils außerhalb, transparente Ränder zählen als Inhalt) | korrigiert: Scan im gedrehten (sichtbaren) Raum | `auto_position_on_rotated_pages_scans_visible_space`, `forced_color_on_rotated_page_scans_visible_orientation` |
//! | KR-F05 | ungültige Platzhalterposition (x/y < 0, w < 1) bricht die ganze Suche ab | nur dieser Platzhalter wird übersprungen | `find_placeholders_in_generated_document` (Seite 5) |
//! | KR-F06 | Rasterscan färbt auch Widgets und Annotationen um | Annotationen (ohne Widgets) werden umgefärbt; Widgets zeichnet PDFium (`FPDF_FFLDraw`) in Originalfarben – sie zählen trotzdem als Inhalt | `forced_color_includes_widgets_and_annotations` (render) |
//! | KR-F07 | QR-Dekodierung mit ZXing auf `getImage()` (Masken angewandt) | `rxing` auf `FPDFImageObj_GetBitmap` (Masken `/SMask`, `/Mask` nicht angewandt, Transparenz eines BGRA-Bitmaps gilt als weiß); ohne Treffer zweiter Versuch 2× vergrößert, weil `rxing` Module um 2 px schlechter erkennt als ZXing | `next_unused_placeholder_like_pdfas`, `decode_generated_qr_codes` |
//! | KR-F08 | leere Signaturfelder nur auf oberster Ebene, ohne Sperrprüfung | auch hierarchische Felder; `ReadOnly`/`LockedContents` werden nicht angeboten | `empty_fields_filtering`, `empty_fields_like_pdfas` |
//! | KR-F09 | `x:NaN`, `w:Infinity` usw. werden akzeptiert | abgelehnt ([`PlacementError::InvalidPosition`]) | `parse_rejects_non_finite_numbers` |
//! | KR-F10 | Rasterscan mit Java2D (exakter Pixelvergleich) | PDFium mit Kantenglättungs-Toleranz [`SCAN_TOLERANCE`]; Glyphenrasterung weicht um höchstens eine Pixelzeile ab (Golden: 8 von 24 Fällen exakt, 16 Fälle mit eingebetteter LiberationSerif 1 pt tiefer) | `pdfas_positions_match_golden`, `tolerance_ignores_faint_antialiasing` |
//! | KR-F11 | UI-Klemmung bei Block größer als Seite liefert `x < 0` bzw. `y < h` | Block liegt links bzw. oben an | `clamp_block_like_f35` |
//! | KR-F12 | QR-Suche dekodiert jedes Bild (`getImage()`) ohne Obergrenze | der Renderer dekodiert je Seite höchstens [`pdfover_render::MAX_IMAGE_PIXELS_PER_PAGE`] Pixel (kleinste Bilder zuerst, jedes ≤ [`pdfover_render::MAX_PIXELS`]); größere Bilder werden nicht auf QR-Codes geprüft (Schutz gegen Dekompressionsbomben) | `image_objects_stay_within_the_pixel_budget` (render) |
//!
//! Ownership: Paket **F** (zusammen mit `pdfover-render`).

pub mod coords;
pub mod placeholder;
pub mod position;

pub use coords::{CoordSpace, KeyStep, PageTransform, Point, ScreenGeometry};
pub use placeholder::{
    EmptyFieldCandidate, MATRIX_TOLERANCE, MIN_QR_IMAGE_PX, PlaceholderMode, PlaceholderQuery,
    QR_PLACEHOLDER_IDENTIFIER, QR_UPSCALE_MAX_PIXELS, QrPayload, QrPlaceholder, decode_qr, find_empty_sig_fields,
    find_qr_placeholders, parse_qr_payload, placeholder_position, select_placeholder,
};
pub use position::{
    BlockGeometry, Coord, FreeSpaceScanner, PageSelector, Placement, PlacementNotice, PositionContext, RasterScanner,
    ResolvedPosition, SCAN_TOLERANCE, SignaturePosition, TablePos, WidthDecision, appearance_rotation, auto_place,
    default_start_placement, default_start_position, detect_background, lowest_content_row,
    lowest_content_row_with_tolerance, new_page_geometry, resolve_position, resolve_width, visible_block_rect,
    widget_overlaps, widget_rect, widget_rect_with,
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
/// Fußzeilenhöhe der PDF-Over-Profile (`f:80`).
pub const DEFAULT_FOOTER: f32 = 80.0;

/// Umgang mit bekannten PDF-AS-Eigenheiten (Kompatibilitätsregister oben, Plan §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum CompatMode {
    /// Korrigiertes Verhalten (Vorschau = Ergebnis), Standard.
    #[default]
    Corrected,
    /// Originalverhalten von PDF-AS 4.4 (`compat.pdfas_quirks`) – nur für Golden-Tests.
    PdfasQuirks,
}

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

impl PlacementError {
    /// Stabiler Code für UI/CLI (ADR 0001 Regel 11; Texte im Abschlussbericht). Renderfehler
    /// behalten den Code des Renderers.
    pub fn code(&self) -> &'static str {
        match self {
            PlacementError::InvalidPosition(_) => "placement.invalid_position",
            PlacementError::NoSuchPage(_) => "placement.no_such_page",
            PlacementError::PlaceholderNotFound => "placement.placeholder_not_found",
            PlacementError::NotEnoughSpace => "placement.not_enough_space",
            PlacementError::Render(e) => e.code(),
        }
    }
}

/// Ergebnis-Alias.
pub type Result<T, E = PlacementError> = std::result::Result<T, E>;
