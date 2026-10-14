//! Tabellenlayout in `f32` mit Java-Semantik (docs/analyse/04 §3.1–§3.5, Quirks Q1–Q12).
//!
//! Zielwerte (Plan §6.5, ±0,01 pt): SMALL_DE 230×49 (mit Hinweis 230×82), Standardlogo 33×34,
//! AMTS auf A4 495×120, Demo-Logo 61×61, BASE_LOGO bei 23 mm 62,118×65,167.

use crate::Result;
use crate::fonts::{FontProvider, FontSpec, LoadedFont};
use crate::input::SigBlockInput;
use crate::profile::{HAlign, Profile, Resources, VAlign};

/// Rolle einer Textzelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// Caption (`font`, `halign`/`valign`)
    Caption,
    /// Wert (`valuefont`, `valuehalign`/`valuevalign`)
    Value,
}

/// Inhalt einer Zelle nach dem Layout.
#[derive(Debug, Clone, PartialEq)]
pub enum CellContent {
    /// Text (bereits umbrochen).
    Text {
        /// Zeilen
        lines: Vec<String>,
        /// Font
        font: FontSpec,
        /// Rolle
        role: TextRole,
        /// Ausrichtung horizontal
        halign: Option<HAlign>,
        /// Ausrichtung vertikal
        valign: Option<VAlign>,
    },
    /// Bild mit berechneter Zeichengröße in ganzen pt (Q5).
    Image {
        /// Profil-Key (`SIG_LABEL`)
        key: String,
        /// Breite in pt
        draw_width: f32,
        /// Höhe in pt
        draw_height: f32,
        /// Ausrichtung
        halign: Option<HAlign>,
        /// Ausrichtung
        valign: Option<VAlign>,
    },
    /// Verschachtelte Tabelle.
    SubTable(Box<LaidOutTable>),
    /// „IMG MISSING“ bzw. leer.
    Empty,
}

/// Zelle mit Geometrie im Formularraum (y nach oben).
#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutCell {
    /// Zeile (0-basiert)
    pub row: usize,
    /// Spalte (0-basiert)
    pub col: usize,
    /// linke Kante
    pub x: f32,
    /// Oberkante
    pub top: f32,
    /// Breite
    pub width: f32,
    /// Höhe (= Zeilenhöhe)
    pub height: f32,
    /// Inhalt
    pub content: CellContent,
}

/// Tabelle nach dem Layout.
#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutTable {
    /// Name
    pub name: String,
    /// Spaltenbreiten
    pub col_widths: Vec<f32>,
    /// Zeilenhöhen
    pub row_heights: Vec<f32>,
    /// Gesamtbreite
    pub width: f32,
    /// Gesamthöhe
    pub height: f32,
    /// Padding (immer das der Tabelle, nie das der Zelle)
    pub padding: f32,
    /// Rahmenbreite (0 = keine)
    pub border: f32,
    /// Hintergrund
    pub bgcolor: Option<[u8; 3]>,
    /// Zellen
    pub cells: Vec<LaidOutCell>,
}

/// Ergebnis des Layouts.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Blockbreite W.
    pub width: f32,
    /// Blockhöhe H.
    pub height: f32,
    /// Haupttabelle.
    pub table: LaidOutTable,
    /// `/TU`: Texte der äußersten Tabelle, je Zelle mit `\n` (Q10).
    pub tu_text: String,
    /// Ersetzte Zeichen ohne Glyph (Meldung an die UI, Register).
    pub replaced_chars: Vec<char>,
}

/// Layout mit fester Breite `width` (aus `pdfover-placement`).
pub fn layout(
    profile: &Profile,
    input: &SigBlockInput,
    width: f32,
    fonts: &FontProvider,
    resources: &dyn Resources,
) -> Result<Layout> {
    todo!("Paket C")
}

/// `breakString(text, maxwidth, font, size)` exakt nach docs/analyse/04 §3.3.
pub fn break_string(text: &str, max_width: f32, font: &LoadedFont, size: f32) -> Result<Vec<String>> {
    todo!("Paket C")
}
