//! Bildmarke (docs/analyse/04 §7; Plan §6.5 „Bilder“).
//!
//! Eingabeformate PNG, JPEG, GIF; EXIF-Ausrichtung **für alle Formate** (bewusste Erweiterung);
//! Verkleinerung auf ≤ 480×600 px (erst Höhe, dann Breite, `(int)`-Trunkierung); Cache als PNG.
//! HEIC dekodiert die Shell über den Plattform-Decoder und übergibt RGBA.

/// Höchstbreite nach Verkleinerung.
pub const MAX_EMBLEM_WIDTH: u32 = 480;
/// Höchsthöhe nach Verkleinerung.
pub const MAX_EMBLEM_HEIGHT: u32 = 600;

/// Dekodierte Bildmarke (RGBA, 8 Bit).
#[derive(Clone, PartialEq, Eq)]
pub struct Emblem {
    /// Breite in Pixel.
    pub width: u32,
    /// Höhe in Pixel.
    pub height: u32,
    /// RGBA-Daten.
    pub rgba: Vec<u8>,
    /// Bild hat Transparenz.
    pub has_alpha: bool,
    /// SHA-256 der Originaldatei (Cache-Schlüssel).
    pub source_sha256: [u8; 32],
}

impl std::fmt::Debug for Emblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emblem")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("has_alpha", &self.has_alpha)
            .finish()
    }
}

impl Emblem {
    /// Dekodiert PNG/JPEG/GIF inklusive EXIF-Ausrichtung und Verkleinerung.
    pub fn decode(bytes: &[u8]) -> crate::Result<Emblem> {
        todo!("Paket C")
    }

    /// Aus bereits dekodiertem RGBA (z. B. HEIC über die Shell).
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>, source_sha256: [u8; 32]) -> crate::Result<Emblem> {
        todo!("Paket C")
    }

    /// Alphakanal entfernen (PDF/A-1): Komposition auf Weiß (Register-Eintrag).
    pub fn flattened_on_white(&self) -> Emblem {
        todo!("Paket C")
    }

    /// PNG für den Cache im Konfigurationsordner.
    pub fn to_png(&self) -> crate::Result<Vec<u8>> {
        todo!("Paket C")
    }
}
