//! Fonts und Metriken (docs/analyse/04 §3.6).
//!
//! - Standard-14: AFM-Breiten (Adobe Core14, Lizenzhinweis), WinAnsi, nicht eingebettet;
//!   Helvetica(-Bold) descender −207, Courier −157/600.
//! - TTF: Breite = `round(advance · 1000 / upem)` (so steht sie im `/W`-Array), Descent =
//!   `hhea.descender · 1000 / upem` (float); DejaVuSansMono 602, descent −235.84.
//! - Einbettung als Type0/CIDFontType2-Subset (Identity-H, `/W`, ToUnicode, CIDToGIDMap; CIDSet bei
//!   PDF/A-1) über `skrifa` + `subsetter`.

use std::sync::Arc;

use crate::Result;

/// Standard-14-Varianten, die PDF-AS akzeptiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Std14 {
    /// Helvetica
    Helvetica,
    /// Helvetica-Bold
    HelveticaBold,
    /// Courier
    Courier,
    /// Courier-Bold
    CourierBold,
    /// Times-Roman
    TimesRoman,
    /// Times-Bold
    TimesBold,
    /// Times-Italic
    TimesItalic,
}

impl Std14 {
    /// PostScript-Name (`/BaseFont`).
    pub fn base_font(self) -> &'static str {
        match self {
            Std14::Helvetica => "Helvetica",
            Std14::HelveticaBold => "Helvetica-Bold",
            Std14::Courier => "Courier",
            Std14::CourierBold => "Courier-Bold",
            Std14::TimesRoman => "Times-Roman",
            Std14::TimesBold => "Times-Bold",
            Std14::TimesItalic => "Times-Italic",
        }
    }
}

/// Fontbeschreibung `FACE,SIZE,STYLE` bzw. `TTF:<datei>,SIZE` (ungültig ⇒ Fallback Helvetica 8).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FontSpec {
    /// Standard-14
    Standard {
        /// Face
        face: Std14,
        /// Größe in pt
        size: f32,
    },
    /// TrueType aus `fonts/`
    TrueType {
        /// Dateiname (z. B. `DejaVuSansMono.ttf`)
        file: String,
        /// Größe in pt
        size: f32,
    },
}

impl FontSpec {
    /// Parst die PDF-AS-Syntax mit Fallback `HELVETICA,8`.
    pub fn parse(desc: &str) -> FontSpec {
        todo!("Paket C")
    }

    /// Schriftgröße.
    pub fn size(&self) -> f32 {
        match self {
            FontSpec::Standard { size, .. } | FontSpec::TrueType { size, .. } => *size,
        }
    }
}

/// Geladener Font mit Metriken.
#[derive(Debug)]
pub struct LoadedFont {
    _private: (),
}

impl LoadedFont {
    /// Breite in 1/1000 em ohne Kerning (`getStringWidth`); Fehler bei nicht kodierbaren Zeichen.
    pub fn string_width(&self, text: &str) -> Result<f32> {
        todo!("Paket C")
    }

    /// Descent in 1/1000 em (negativ).
    pub fn descent(&self) -> f32 {
        todo!("Paket C")
    }

    /// `true`, wenn alle Zeichen kodierbar sind.
    pub fn can_encode(&self, text: &str) -> bool {
        todo!("Paket C")
    }

    /// Wird eingebettet (TTF) oder nicht (Standard-14).
    pub fn is_embedded(&self) -> bool {
        todo!("Paket C")
    }
}

/// Fontquelle mit Cache je Dokument (Schlüssel Fontpfad bzw. `FACE:STYLE`).
#[derive(Debug, Default)]
pub struct FontProvider {
    _private: (),
}

impl FontProvider {
    /// Nur eingebettete Fonts (DejaVu aus `assets/fonts`, AFM-Daten).
    pub fn builtin() -> FontProvider {
        todo!("Paket C")
    }

    /// Fonts aus einer Ressourcenquelle (eigene Profile mit `TTF:<datei>` aus `<workdir>/fonts`).
    pub fn from_resources(resources: Arc<dyn crate::profile::Resources>) -> FontProvider {
        todo!("Paket C")
    }

    /// Lädt bzw. liefert aus dem Cache.
    pub fn load(&self, spec: &FontSpec) -> Result<Arc<LoadedFont>> {
        todo!("Paket C")
    }
}
