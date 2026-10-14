//! `pdfover-sigblock` – sichtbarer Signaturblock, maßgleich zu PDF-AS 4.4.2 (Plan §6.5;
//! docs/analyse/04 §2–§3, §7; docs/analyse/03 §4.7).
//!
//! Pipeline:
//! 1. [`ProfileStore`] lädt `cfg/config.properties` + `include.*` (`profiles/*.properties`),
//!    nur `sig_obj.types.X=on`, Vererbung schlüsselweise über `parent`/`augments`
//!    (docs/analyse/04 §2.1) → typisierte Profil-IR [`Profile`].
//! 2. Laufzeit-Overlays wie PDF-Over ([`ProfileOverrides`]: `SIG_LABEL`, `SIG_NOTE`, bei
//!    `BASE_LOGO` `padding`/`pos`/`imagescaletofit`, docs/analyse/04 §2.8).
//! 3. Werte auflösen ([`expr`]: OGNL-Teilmenge, `SIG_DATE` als `SimpleDateFormat`-Teilmenge;
//!    unbekannte Ausdrücke werden **abgelehnt**).
//! 4. [`layout::layout`] in `f32` mit Java-Semantik (Spaltenbreiten, gieriger Umbruch mit
//!    strikt-größer-Vergleich, Zeilenhöhe `size·n + 2p`, Bildbox, Quirks Q1–Q12).
//! 5. [`Appearance::build`]: Holder-Form `q 1.0 0.0 0.0 1.0 0.0 0.0 cm /FRM1 Do Q` mit `/Matrix` =
//!    Rotation, Inner-Form mit der Tabelle bei y = 1, BBox `[W+1 H+1]`, Fonts (Standard-14 nicht
//!    eingebettet; DejaVu als Type0/CIDFontType2-Subset), Bilder (PNG-Predictor 15, SMask; PDF/A-1
//!    ohne Alpha) – als lopdf-Objektgraph mit lokalen Referenzen.
//!
//! Die 12 von PDF-Over erzeugbaren Profil-IDs: `SIGNATURBLOCK_SMALL_{DE|EN}[_NOTE][_PDFA]`,
//! `AMTSSIGNATURBLOCK_{DE|EN}_RECOMMENDED`, `BASE_LOGO`, `INVISIBLE` (Ableitung in
//! `pdfover_config::SignatureProfile::pdfas_profile_id`).
//!
//! Ownership: Paket **C**. Profile, Fonts (DejaVu, Bitstream-Vera-Lizenz) und Bildmarken werden aus
//! `reference/pdfas-config` nach `crates/sigblock/assets/` übernommen (EUPL; `arial.ttf` nicht).

pub mod appearance;
pub mod emblem;
pub mod expr;
pub mod fonts;
pub mod input;
pub mod layout;
pub mod profile;

pub use appearance::{Appearance, BuildOptions, LOCAL_GENERATION, MaterializedAppearance};
pub use emblem::{Emblem, MAX_EMBLEM_HEIGHT, MAX_EMBLEM_WIDTH};
pub use fonts::{FontProvider, FontSpec, LoadedFont, Std14};
pub use input::{CertFields, PdfaMode, SigBlockInput};
pub use layout::Layout;
pub use profile::{
    BuiltinResources, CellSpec, DirResources, HAlign, Profile, ProfileFlags, ProfileOverrides, ProfileStore, Resources,
    Row, Style, Table, VAlign,
};

/// Die 12 Profil-IDs, die PDF-Over erzeugt (Abnahmekriterium T1, Plan §8.9).
pub const PDFOVER_PROFILE_IDS: [&str; 12] = [
    "SIGNATURBLOCK_SMALL_DE",
    "SIGNATURBLOCK_SMALL_DE_NOTE",
    "SIGNATURBLOCK_SMALL_DE_PDFA",
    "SIGNATURBLOCK_SMALL_DE_NOTE_PDFA",
    "SIGNATURBLOCK_SMALL_EN",
    "SIGNATURBLOCK_SMALL_EN_NOTE",
    "SIGNATURBLOCK_SMALL_EN_PDFA",
    "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA",
    "AMTSSIGNATURBLOCK_DE_RECOMMENDED",
    "AMTSSIGNATURBLOCK_EN_RECOMMENDED",
    "BASE_LOGO",
    "INVISIBLE",
];

/// Fehler des Signaturblocks.
#[derive(Debug, thiserror::Error)]
pub enum SigBlockError {
    /// Profil-ID unbekannt (PDF-AS 11009).
    #[error("Profil {0} existiert nicht")]
    UnknownProfile(String),
    /// Profil fehlerhaft (Tabelle verworfen, Zyklus in `parent` …).
    #[error("Profil fehlerhaft: {0}")]
    Profile(String),
    /// Ausdruck außerhalb der unterstützten OGNL-Teilmenge (wird abgelehnt, nicht geraten).
    #[error("nicht unterstützter Ausdruck: {0}")]
    UnsupportedExpression(String),
    /// Datumsformat außerhalb der unterstützten `SimpleDateFormat`-Teilmenge.
    #[error("nicht unterstütztes Datumsformat: {0}")]
    UnsupportedDatePattern(String),
    /// Font nicht ladbar.
    #[error("Font: {0}")]
    Font(String),
    /// Zeichen ohne Glyph im Font (wird ersetzt und gemeldet, Plan §6.5 Register).
    #[error("Zeichen ohne Glyph: {0:?}")]
    MissingGlyph(char),
    /// Bild nicht lesbar.
    #[error("Bild: {0}")]
    Image(String),
    /// Ressource (Bild, Font, Profil) fehlt.
    #[error("Ressource fehlt: {0}")]
    MissingResource(String),
    /// Fehler beim Erzeugen der PDF-Objekte.
    #[error("PDF: {0}")]
    Pdf(String),
    /// E/A
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
}

/// Ergebnis-Alias.
pub type Result<T, E = SigBlockError> = std::result::Result<T, E>;
