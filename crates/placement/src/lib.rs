//! `pida-placement` – Koordinatenmodell und Positionierung (Plan §6.7, §6.8;
//! docs/analyse/04 §4–§5).
//!
//! Alle Funktionen arbeiten auf **Datenstrukturen** (`PageInfo`, `SigField`, `ImageInvocation` aus
//! `pida-pdf-cos`; `Renderer` aus `pida-render`), damit Paket F unabhängig von Paket A testen
//! kann (ADR 0001 §5.3).
//!
//! # Ablauf
//!
//! 1. [`TablePos::parse`] und [`TablePos::merge`] (Request-Position über Profil-`pos`),
//! 2. [`resolve_width`] (Seite, x, Breite – vor dem Layout),
//! 3. Layout in `pida-sigblock` (Blockhöhe),
//! 4. [`resolve_position`] bzw. [`auto_place`] (Rasterscan, neue Seite, Hinweise),
//! 5. [`widget_rect`] und [`appearance_rotation`] für das Widget.
//!
//! QR-Platzhalter: [`find_qr_placeholders`] (Bilder aus dem Renderer, Namen/CTM aus `pdf-cos`)
//! und [`select_placeholder`]; leere Signaturfelder: [`find_empty_sig_fields`], Lage und Eignung des Blocks im Feld:
//! [`field_fit`]. Startzustand der
//! UI-Positionierung: [`default_start_placement`] (letzte Seite, 50 %/25 %). Seitenlayout der Oberfläche
//! „Arbeitsbereich“ (ADR 0004 §3.3): [`OccupancyGrid`] aus dem Forced-Color-Raster und [`page_layout`]
//! (Inhaltsrahmen, freie Flächen, Einrastlinien) – rein und ohne Renderer testbar.
//!
//! # Kompatibilitätsregister (Paket F, Plan §6.5)
//!
//! | ID | Abweichung von PDF-AS | Entscheidung | Test |
//! |---|---|---|---|
//! | KR-F01 | AMTS-Breite ohne `w` hängt bei manuellem `x` von `x` ab (`pw − 2x`) | korrigiert: `pw − 100` wie bei automatischer Position; [`CompatMode::PdfasQuirks`] rechnet wie PDF-AS | `amts_width_corrected_and_quirk` |
//! | KR-F02 | CropBox-Ursprung wird im Widget-Rect ignoriert, Rect mit MediaBox-Maßen | korrigiert: Rect relativ zur CropBox (Ursprung und Maße), neue Seiten mit MediaBox; QR-Position relativ zur CropBox | `cropbox_origin_is_honoured_when_corrected`, `quirk_rect_ignores_cropbox_origin`, `widget_rect_lands_where_pdfium_shows_it` |
//! | KR-F03 | Platzhalter-IDs > 127 passen nie (Integer-`==`) | korrigiert (numerischer Vergleich) | `id_matching_without_integer_cache_bug`, `ids_above_127` |
//! | KR-F04 | Rasterscan gedrehter Seiten im ungedrehten Bildformat `(int) w × (int) h` mit Startzeile `(int) h − 1 − f` (Inhalt teils außerhalb, nicht gelöschte Pixel zählen als Inhalt; Golden PDF-AS 4.4.2: A4 mit `/Rotate 90/270` und `f:80` ⇒ immer neue Seite, Querformat-MediaBox mit `/Rotate 90/270` ⇒ immer die Startzeile, `/Rotate 180` mit Breitenanteil > ½ ⇒ neue Seite) | korrigiert: Scan im gedrehten (sichtbaren) Raum; [`CompatMode::PdfasQuirks`] bildet das Rasterbild nach (Java2D-Pixelregel, Startzeile); Rest: PDF-AS sieht bei 90°/270° nur die linken `(int) w` Spalten | `auto_position_on_rotated_pages_scans_visible_space`, `forced_color_on_rotated_page_scans_visible_orientation`, `pdfas_rotated_scan_like_pdfas`, `golden_pdfas442` |
//! | KR-F05 | ungültige Platzhalterposition (x/y < 0, w < 1) bricht die ganze Suche ab | nur dieser Platzhalter wird übersprungen | `find_placeholders_in_generated_document` (Seite 5) |
//! | KR-F06 | Rasterscan färbt auch Widgets und Annotationen um | Annotationen (ohne Widgets) werden umgefärbt; Widgets zeichnet PDFium (`FPDF_FFLDraw`) in Originalfarben – sie zählen trotzdem als Inhalt | `forced_color_includes_widgets_and_annotations` (render) |
//! | KR-F07 | QR-Dekodierung mit ZXing auf `getImage()` (Masken angewandt) | `rxing` auf das Bildobjekt von `pida-render`, seit Welle 3 ebenfalls mit angewandter Maske (`/SMask`, `/Mask`; Bilder bis 4 MP; transparente Pixel gelten wie bei ZXing als weiß); ohne Treffer zweiter Versuch 2× vergrößert, weil `rxing` Module um 2 px schlechter erkennt als ZXing | `next_unused_placeholder_like_pdfas`, `decode_generated_qr_codes`, `transparent_qr_placeholders_are_decoded_with_their_mask` |
//! | KR-F08 | leere Signaturfelder nur auf oberster Ebene, ohne Sperrprüfung | auch hierarchische Felder; `ReadOnly`/`LockedContents` werden nicht angeboten | `empty_fields_filtering`, `empty_fields_like_pdfas` |
//! | KR-F09 | `x:NaN`, `w:Infinity` usw. werden akzeptiert | abgelehnt ([`PlacementError::InvalidPosition`]) | `parse_rejects_non_finite_numbers` |
//! | KR-F10 | Rasterscan mit Java2D (exakter Pixelvergleich, Seite 1:1 auf `(int) w × (int) h`) | PDFium mit Kantenglättungs-Toleranz [`SCAN_TOLERANCE`]; `pida-render` bildet die Seite seit Welle 3 ebenfalls 1:1 auf `(int) w × (int) h` ab (vorher auf `round(w) × round(h)` gestreckt: bei Seitenhöhen mit Nachkommaanteil ≥ 0,5 genau 1 pt tiefer). Golden PDF-AS 4.4.2: alle Auto-Positionen exakt außer Haarlinien knapp über einer Pixelgrenze als unterster Inhalt (PDFium zeichnet Linien mindestens 1 px breit, Java2D mit der echten Breite; Eingabe `presigned`: 1 pt tiefer, Plan §8.2 ±1 pt) | `pdfas_positions_match_golden`, `tolerance_ignores_faint_antialiasing`, `golden_pdfas442` |
//! | KR-F11 | UI-Klemmung bei Block größer als Seite liefert `x < 0` bzw. `y < h` | Block liegt links bzw. oben an | `clamp_block_like_f35` |
//! | KR-F12 | QR-Suche dekodiert jedes Bild (`getImage()`) ohne Obergrenze | der Renderer dekodiert je Seite höchstens [`pida_render::MAX_IMAGE_PIXELS_PER_PAGE`] Pixel (kleinste Bilder zuerst, jedes ≤ [`pida_render::MAX_PIXELS`]); größere Bilder werden nicht auf QR-Codes geprüft (Schutz gegen Dekompressionsbomben) | `image_objects_stay_within_the_pixel_budget` (render) |
//! | KR-F13 | vorhandenes Signaturfeld: Block mit `/BBox [W+1 H+1]` im unveränderten Feld-`/Rect`, ein Viewer streckt ihn nicht proportional auf die Feldgröße (`manySignFields_signed4.pdf`: 232 × 84 pt in 3,16 × 224 pt); keine Prüfung der Feldgröße | korrigiert: [`field_fit`] – proportional, höchstens natürliche Größe, mittig (Schreiben: `pida-pades` KR-P10); als Ziel angeboten nur mit Maßstab ≥ [`FIELD_MIN_SCALE`], Formähnlichkeit ≥ [`FIELD_MIN_SHAPE`] und höchstens [`FIELD_MAX_PAGE_SHARE`] der Seite ([`field_suitable`]); [`CompatMode::PdfasQuirks`] schreibt wie PDF-AS | `many_sign_fields_only_plausible_targets`, `common_fields_are_targets_for_every_profile`, `page_sized_field_keeps_the_natural_size`, `user_space_and_visible_space_agree_for_all_rotations` |
//! | KR-F14 | QR-Platzhalter: `x` abgerundet und Breite getrennt aufgerundet (`floor(x)`, `ceil(w)`) – der Block endet bis knapp 1 pt vor der rechten Kante des Platzhalterbilds, das daneben hervorschaut (`two_placeholder.pdf`: Bild bis 405,8 pt, Block bis 405 pt) | korrigiert: Breite mindestens `ceil(x + w) − floor(x)`, höchstens 1 pt breiter als bei PDF-AS | `placeholder_width_covers_the_right_edge`, `placeholder_position_matches_pdfas_widget_rects` (±1 pt) |
//!
//! Ownership: Paket **F** (zusammen mit `pida-render`).

pub mod coords;
pub mod field;
pub mod layout;
pub mod placeholder;
pub mod position;

pub use coords::{CoordSpace, KeyStep, PageTransform, Point, ScreenGeometry};
pub use field::{
    FIELD_MAX_PAGE_SHARE, FIELD_MIN_SCALE, FIELD_MIN_SHAPE, FieldFit, field_fit, field_suitable, obstacle_fields,
};
pub use layout::{
    BELOW_CONTENT_PT, EMPTY_PAGE_MARGIN_PT, FREE_GAP_PT, GRID_CELL_PT, Guide, GuideAxis, GuideKind, LayoutOptions,
    MAX_FREE_RECTS, OccupancyGrid, PageLayout, page_layout,
};
pub use placeholder::{
    EmptyFieldCandidate, MATRIX_TOLERANCE, MIN_QR_IMAGE_PX, PlaceholderMode, PlaceholderQuery,
    QR_PLACEHOLDER_IDENTIFIER, QR_UPSCALE_MAX_PIXELS, QrMatrix, QrPayload, QrPlaceholder, decode_qr, encode_qr,
    find_empty_sig_fields, find_qr_placeholders, parse_qr_payload, placeholder_position, qr_placeholder_text,
    select_placeholder,
};
pub use position::{
    BlockGeometry, Coord, FreeSpaceScanner, PageSelector, Placement, PlacementNotice, PositionContext, RasterScanner,
    ResolvedPosition, SCAN_TOLERANCE, SignaturePosition, TablePos, WidgetAwareScanner, WidthDecision,
    appearance_rotation, auto_place, default_start_placement, default_start_position, detect_background,
    lowest_content_row, lowest_content_row_with_tolerance, matrix_unit_rect, new_page_geometry, resolve_position,
    resolve_width, user_rect_to_visible, visible_block_rect, widget_overlaps, widget_rect, widget_rect_with,
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
    Render(#[from] pida_render::RenderError),
    /// QR-Code nicht erzeugbar („QR-Platzhalter erstellen“).
    #[error("QR-Code nicht erzeugbar: {0}")]
    Qr(String),
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
            PlacementError::Qr(_) => "placement.qr",
        }
    }
}

/// Ergebnis-Alias.
pub type Result<T, E = PlacementError> = std::result::Result<T, E>;
