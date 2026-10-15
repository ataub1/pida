//! `pida-sigblock` – sichtbarer Signaturblock, maßgleich zu PDF-AS 4.4.2 (Plan §6.5;
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
//! `pida_config::SignatureProfile::pdfas_profile_id`).
//!
//! Ownership: Paket **C**. Profile, Fonts (DejaVu, Bitstream-Vera-Lizenz) und Bildmarken werden aus
//! `reference/pdfas-config` nach `crates/sigblock/assets/` übernommen (EUPL; `arial.ttf` nicht) und
//! per `include_bytes!` eingebettet ([`BuiltinResources`], [`ProfileStore::builtin`]); die
//! Standard-14-Breiten stammen aus den Adobe-Core14-AFM-Dateien (`src/afm_data.rs`, generiert mit
//! `tools/gen_afm.py`). Das PIDA-Siegel (`assets/images/pida-seal.png`, eigenes Werk, EUPL-1.2) ist
//! die Standard-Bildmarke der kleinen Standardblöcke ([`assets::PIDA_SEAL_PROFILES`]); Master und
//! Generator unter `design/brand/` (ADR 0005).
//!
//! # Verifikation
//!
//! - Rechenbeispiele aus docs/analyse/04 §3.8 (`tests/layout.rs`, ±0,01 pt).
//! - Golden-Vergleich gegen von PDF-AS signierte Referenz-PDFs (`tests/golden.rs`): Tokenstrom des
//!   Inner-Forms gleich; mit `pdfas_quirks` für das aktuelle Format byte-gleich; Pixelvergleich per
//!   PDFium (`tests/render.rs`, nur mit `PIDA_PDFIUM_DIR`). Ergebnisse:
//!   `testdata/sigblock/RESULTS.md`.
//!
//! # Kompatibilitätsregister (Abweichungen nur ohne `SigBlockInput::pdfas_quirks`)
//!
//! | Thema | PDF-AS | hier |
//! |---|---|---|
//! | Standard-Bildmarke `SIGNATURBLOCK_SMALL_{DE,EN}[_NOTE][_PDFA]` | `signatur-logo_de/en.png` | PIDA-Siegel ([`assets::PIDA_SEAL`], ADR 0005; gleiches Seitenverhältnis ⇒ gleiche Blockmaße); [`ProfileStore::builtin_pdfas`] liefert die PDF-AS-Konfiguration unverändert (gilt auch mit `pdfas_quirks`) |
//! | `/TU` (Q10) | nur Texte der äußersten Tabelle (SMALL ohne Hinweis: leer) | alle Texte in Leserichtung |
//! | Nicht darstellbare Zeichen (Q11) | `URLEncoder.encode` des ganzen Werts | Zeichen → `?`, gemeldet in [`Layout::replaced_chars`] |
//! | `\n` am Wortende | Messfehler, Signatur bricht ab | wird als Umbruch behandelt |
//! | `\r\n`/`\r` (Windows-Eingaben) | kein Glyph, Signatur bricht ab | Zeilenumbruch wie `\n` |
//! | Titel-RDN | nur `title` → zusätzlich `T` | `title` **und** `T` (auch OID 2.5.4.12) |
//! | Farben | `/DeviceRGB cs … sc` | Grautöne als DeviceGray (`g`), sonst `rg` (pixelgleich) |
//! | Bildmarke/Farben bei PDF/A | DeviceRGB (verlangt RGB-OutputIntent) | `ICCBased` mit eingebettetem sRGB-Profil (Plan §6.5; konform auch bei CMYK-OutputIntent, PDFium pixelgleich) |
//! | Ausdrücke | beliebiges OGNL, Fehler ⇒ Ausdruck roh | OGNL-Teilmenge, Schachtelung ≤ 32, sonst [`SigBlockError::UnsupportedExpression`] |
//! | NaN/∞ in Stilwerten | `IllegalArgumentException` beim Schreiben | [`SigBlockError::Profile`] schon im Layout |
//! | Include-Zyklen | StackOverflowError | Datei auf dem Include-Pfad wird übersprungen, [`ProfileStore::problems`] |
//! | PDF/A + Standard-14 | nicht eingebettete Helvetica | DejaVu-Ersatz ([`fonts::FontSpec::embeddable`]) |
//! | PDF/A-1-Bilder (SB-G6) | `removeAlphaChannel`: Alpha entfällt, gespeicherte RGB-Werte bleiben (transparente Pixel meist schwarz; Golden PDF-AS 4.4.2) | Komposition auf Weiß; mit `pdfas_quirks` wie PDF-AS |
//! | Voll deckender Alphakanal (SB-G2) | SMask mit lauter 255 (LosslessFactory) | keine SMask (pixelgleich); mit `pdfas_quirks` wie PDF-AS (Farbtyp [`SourceColor`], Welle 3) |
//! | Indizierte Transparenz (SB-G3, PNG-Palette mit `tRNS`, GIF) | verworfen, Palettenfarbe sichtbar | als SMask erhalten; mit `pdfas_quirks` wie PDF-AS |
//! | Graustufen-Bildmarken (SB-G4, PNG-Graustufen, Graustufen-JPEG) | DeviceGray, aufgehellt (Java `CS_GRAY` linear → sRGB, z. B. 20 → 79); mit `tRNS`/Alphakanal ICCBased-Grau + SMask | DeviceRGB mit den Originalwerten; mit `pdfas_quirks` wie PDF-AS ([`java_linear_gray_to_srgb`]; ICCBased-Grau als aufgehelltes DeviceGray + SMask, gleiches Bild) |
//! | Palettenbilder ≤ 16 Farben (PNG 1/2/4 Bit, GIF) | Java liest sie gepackt: verworfene Transparenz zeigt Schwarz | ebenso (beim Dekodieren; sonst unsichtbar) |
//! | JPEG-Bildmarken (SB-G5) | ImageIO (libjpeg) | `zune-jpeg`, wenige Stufen Abweichung |
//! | EXIF | nur JPEG (PDF-Over) | nur JPEG (wie PDF-Over) |
//! | Logo-Größe > 120 mm (SB-G1) | Klemmung nur in der Vorschau | Klemmung auch beim Signieren ([`ProfileOverrides::base_logo`]) |
//!
//! Golden-Vergleich gegen einen frischen PDF-AS-4.4.2-Lauf (12 Profil-IDs × Eingaben × Positionen,
//! `tools/golden/run-pdfas.sh`, `tests/golden_pdfas442.rs`, docs/integration/golden-report.md):
//! Geometrie, Texte, Fonts und Linien gleich (±0,01 pt), mit `pdfas_quirks` Content-Stream
//! byte-gleich, Bilder (Farbraum, Samples, SMask) bis auf SB-G5 (JPEG-Dekoder) gleich. Alle
//! PNG-/GIF-Farbtypen prüft zusätzlich das PDF-AS-Orakel `appearance::tests::pdfas_lossless_factory_oracle`
//! (mit `PIDA_PDFAS_LIB`, PDF-AS 4.4.2 + PDFBox 2.0.34 unter Java 21). Abweichungen
//! im Standardmodus nur in den Zeilen SB-G1 … SB-G6. Ob der Standardmodus korrigiert bleibt
//! (SB-G3, SB-G4, SB-G6), entscheidet der Auftraggeber; Default: korrigiert. [`Appearance::to_standalone_pdf`]
//! zeichnet über das Holder-Form und ist mit PDFium pixelgleich zum Widget der PDF-AS-Ausgabe.

mod afm;
pub mod appearance;
pub mod assets;
pub mod emblem;
pub mod expr;
pub mod fonts;
mod icc;
pub mod input;
mod java;
pub mod layout;
pub mod profile;

pub use appearance::{Appearance, BuildOptions, LOCAL_GENERATION, MaterializedAppearance};
pub use emblem::{Emblem, MAX_EMBLEM_HEIGHT, MAX_EMBLEM_WIDTH, SourceColor, java_linear_gray_to_srgb};
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

impl SigBlockError {
    /// Stabiler Meldungscode (Fluent-ID, ADR 0001 Regel 11; die Texte trägt Paket E/Integration in
    /// `crates/i18n/locales/` ein):
    ///
    /// | Code | Deutsch | English |
    /// |---|---|---|
    /// | `sigblock-unknown-profile` | Das Signaturprofil existiert nicht. | The signature profile does not exist. |
    /// | `sigblock-profile` | Das Signaturprofil ist fehlerhaft. | The signature profile is invalid. |
    /// | `sigblock-unsupported-expression` | Das Signaturprofil enthält einen nicht unterstützten Ausdruck. | The signature profile contains an unsupported expression. |
    /// | `sigblock-unsupported-date-pattern` | Das Datumsformat des Signaturprofils wird nicht unterstützt. | The date format of the signature profile is not supported. |
    /// | `sigblock-font` | Die Schrift des Signaturblocks konnte nicht geladen werden. | The signature block font could not be loaded. |
    /// | `sigblock-missing-glyph` | Ein Zeichen ist in der Schrift des Signaturblocks nicht enthalten. | A character is not available in the signature block font. |
    /// | `sigblock-image` | Die Bildmarke konnte nicht gelesen werden. | The emblem image could not be read. |
    /// | `sigblock-missing-resource` | Eine Ressource des Signaturprofils (Bild, Schrift, Profildatei) fehlt. | A resource of the signature profile (image, font, profile file) is missing. |
    /// | `sigblock-pdf` | Das Erscheinungsbild der Signatur konnte nicht erzeugt werden. | The signature appearance could not be created. |
    /// | `sigblock-io` | Lese- oder Schreibfehler beim Signaturblock. | Read or write error in the signature block. |
    ///
    /// Dazu der Hinweis [`NOTICE_CHARS_REPLACED`] (`sigblock-chars-replaced`): „Einige Zeichen sind
    /// im Signaturblock nicht darstellbar und wurden durch „?“ ersetzt.“ / “Some characters cannot
    /// be displayed in the signature block and were replaced with “?”.”
    pub fn code(&self) -> &'static str {
        match self {
            SigBlockError::UnknownProfile(_) => "sigblock-unknown-profile",
            SigBlockError::Profile(_) => "sigblock-profile",
            SigBlockError::UnsupportedExpression(_) => "sigblock-unsupported-expression",
            SigBlockError::UnsupportedDatePattern(_) => "sigblock-unsupported-date-pattern",
            SigBlockError::Font(_) => "sigblock-font",
            SigBlockError::MissingGlyph(_) => "sigblock-missing-glyph",
            SigBlockError::Image(_) => "sigblock-image",
            SigBlockError::MissingResource(_) => "sigblock-missing-resource",
            SigBlockError::Pdf(_) => "sigblock-pdf",
            SigBlockError::Io(_) => "sigblock-io",
        }
    }
}

/// Meldungscode für den Hinweis „Zeichen ersetzt“ ([`Layout::replaced_chars`] nicht leer).
pub const NOTICE_CHARS_REPLACED: &str = "sigblock-chars-replaced";

/// Ergebnis-Alias.
pub type Result<T, E = SigBlockError> = std::result::Result<T, E>;

/// Kürzt Texte für Fehlermeldungen (Base64-Bildwerte, lange Ausdrücke aus Hinweistexten), damit
/// Logs keine großen Datenmengen oder ganzen Texte enthalten (ADR 0001 Regel 9).
pub(crate) fn shorten(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    let head: String = s.chars().take(max_chars).collect();
    format!("{head}… ({n} Zeichen)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_unique_and_prefixed() {
        let errs = [
            SigBlockError::UnknownProfile(String::new()),
            SigBlockError::Profile(String::new()),
            SigBlockError::UnsupportedExpression(String::new()),
            SigBlockError::UnsupportedDatePattern(String::new()),
            SigBlockError::Font(String::new()),
            SigBlockError::MissingGlyph('x'),
            SigBlockError::Image(String::new()),
            SigBlockError::MissingResource(String::new()),
            SigBlockError::Pdf(String::new()),
            SigBlockError::Io(std::io::Error::other("x")),
        ];
        let mut codes: Vec<&str> = errs.iter().map(SigBlockError::code).collect();
        assert!(codes.iter().all(|c| c.starts_with("sigblock-")));
        codes.push(NOTICE_CHARS_REPLACED);
        let n = codes.len();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), n);
    }

    #[test]
    fn shorten_keeps_short_and_cuts_long_texts() {
        assert_eq!(shorten("abc", 3), "abc");
        assert_eq!(shorten("äöüß", 2), "äö… (4 Zeichen)");
    }
}
