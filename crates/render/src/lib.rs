//! `pdfover-render` – ein Renderer für alles (Plan §2.1, §6.6–§6.8).
//!
//! Vorschau, Blockvorschau (Mini-PDF aus dem echten Appearance-Objekt), Rasterscan der
//! Auto-Positionierung (Forced-Color-Rendering) und QR-Bilddekodierung laufen über **einen**
//! Renderer, damit Vorschau = Positionierung gilt. PDFium steht hinter dem Trait [`Renderer`]
//! (Rückfall: `hayro`, Plan §2.4/§2.5).
//!
//! Die PDFium-Bibliothek wird dynamisch geladen (Desktop/Android); unter iOS als xcframework
//! eingebettet (Faktencheck 5). Auf dem Desktop kann der Renderer in einem Hilfsprozess laufen
//! (Plan §6.6) – deshalb ist [`Renderer`] `Send` und objektsicher.
//!
//! Feature `pdfium` (Default): `PdfiumBackend`/`PdfiumRenderer`. Andere Crates hängen ohne
//! Default-Features von diesem Crate ab und nutzen nur die Traits (ADR 0001 §8); so lassen sich
//! `pdfover-placement` und `pdfover-pades` ohne `pdfium-render` bauen und mit eigenen
//! Test-Renderern prüfen.
//!
//! # PDFium bereitstellen (ADR 0060)
//!
//! Gebunden wird der Build **chromium/8100** der `pdfium-binaries` (API-Stand von
//! `pdfium-render` 0.9.4: `pdfium_7881`). `PdfiumBackend::bind(None)` sucht in dieser Reihenfolge:
//!
//! 1. Verzeichnis aus der Umgebungsvariable [`ENV_PDFIUM_DIR`] (`PDFOVER_PDFIUM_DIR`), z. B.
//!    `tools/pdfium/mac-arm64/lib` im Hauptcheckout,
//! 2. neben der laufenden Binary (bei macOS-Bundles zusätzlich `../Frameworks` und
//!    `../Resources`, unter iOS `Frameworks` des App-Bundles),
//! 3. Systempfad: unter Linux/Android `dlopen("libpdfium.so")` (`ld.so` sucht nie im
//!    Arbeitsverzeichnis); unter macOS und Windows nur **absolute** Verzeichnisse
//!    (macOS: absolute Einträge von `DYLD_LIBRARY_PATH`/`DYLD_FALLBACK_LIBRARY_PATH`,
//!    `/opt/homebrew/lib`, `/usr/local/lib`, `/usr/lib`; Windows: absolute `PATH`-Einträge), weil
//!    die Suche nach dem bloßen Dateinamen dort auch das Arbeitsverzeichnis einschließt (CWE-427);
//!    unter iOS gar nicht (die Bibliothek gehört ins Bundle).
//!
//! **Mobile (Plan §5.3, ADR 0060):**
//! - **iOS:** PDFium liegt als Framework [`IOS_FRAMEWORK_BINARY`] (`pdfium.framework/pdfium`, aus
//!   `PDFium.xcframework` von `tools/fetch-pdfium.sh --xcframework`, eingebettet und mitsigniert) in
//!   `<App>.app/Frameworks`; je Verzeichnis werden das Framework und eine lose `libpdfium.dylib`
//!   (Entwicklung) versucht ([`library_paths_in`]).
//! - **Android:** `libpdfium.so` je ABI in `jniLibs/<abi>/` (`tools/fetch-pdfium.sh --jnilibs`); der
//!   Paketmanager entpackt sie nach `nativeLibraryDir`, das im Linker-Namensraum der App liegt –
//!   `dlopen("libpdfium.so")` (Schritt 3) findet sie wie `System.loadLibrary("pdfium")`. Kennt die
//!   Shell `nativeLibraryDir` (Plugin), kann sie es als `bind(Some(dir))` übergeben.
//!
//! Mit `bind(Some(dir))` wird ausschließlich `dir` verwendet. Findet sich nichts, liefert
//! `bind` [`RenderError::LibraryNotFound`] mit allen durchsuchten Orten. Die Bibliothek wird
//! **einmal je Prozess** gebunden; weitere `bind`-Aufrufe liefern dieselbe Bindung.
//! Tests, die PDFium brauchen, prüfen `PDFOVER_PDFIUM_DIR` und werden sonst übersprungen
//! (ADR 0001 Regel 6).
//!
//! # Threads
//!
//! PDFium ist nicht threadsicher. `pdfium-render` (Feature `thread_safe`) serialisiert jeden
//! einzelnen FFI-Aufruf; zusätzlich hält jede Operation dieses Crates (Öffnen, Rendern,
//! Bildobjekte, Schließen) eine prozessweite Sperre, damit mehrschrittige Abläufe (Seite laden,
//! rendern, schließen) nie verzahnt laufen. `PdfiumBackend` ist `Send + Sync` und kann aus
//! beliebig vielen Threads benutzt werden; ein `PdfiumRenderer` ist `Send`.
//!
//! Ownership: Paket **F** (zusammen mit `pdfover-placement`).

use std::sync::Arc;

#[cfg(feature = "pdfium")]
mod ffi;
#[cfg(feature = "pdfium")]
mod pdfium;

#[cfg(feature = "pdfium")]
pub use pdfium::{PdfiumBackend, PdfiumRenderer};

/// Umgebungsvariable mit dem Verzeichnis der PDFium-Bibliothek (Tests, Entwicklung).
pub const ENV_PDFIUM_DIR: &str = "PDFOVER_PDFIUM_DIR";

/// iOS: Binary des PDFium-Frameworks relativ zu `Frameworks` des App-Bundles (install name
/// `@rpath/pdfium.framework/pdfium`, `tools/fetch-pdfium.sh --xcframework`).
pub const IOS_FRAMEWORK_BINARY: &str = "pdfium.framework/pdfium";

/// Dateiname der PDFium-Bibliothek auf dem Zielsystem (`pdfium.dll`, `libpdfium.dylib`,
/// `libpdfium.so`); unter iOS die lose Variante neben [`IOS_FRAMEWORK_BINARY`].
pub fn library_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

/// Mögliche Bibliotheksdateien in einem Verzeichnis in Suchreihenfolge: unter iOS zuerst das
/// Framework ([`IOS_FRAMEWORK_BINARY`]), dann die lose `libpdfium.dylib`; sonst nur
/// [`library_file_name`].
pub fn library_paths_in(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if cfg!(target_os = "ios") {
        out.push(dir.join(IOS_FRAMEWORK_BINARY));
    }
    out.push(dir.join(library_file_name()));
    out
}

/// Obergrenze für die Pixelzahl eines Rasters bzw. dekodierten Bildes (Schutz gegen
/// Speicherüberlauf bei feindlichen Seitengrößen, Plan §7.7).
pub const MAX_PIXELS: u64 = 100_000_000;

/// Budget für die **Summe** der dekodierten Bildpixel eines `image_objects`-Aufrufs (≈ 256 MiB
/// RGBA). Schutz gegen Dekompressionsbomben, die ein kleines Bild viele Male zeichnen: ohne Budget
/// hielte das Ergebnis jede Kopie dekodiert im Speicher (ein 80-KB-PDF mit einem 9000 × 9000-Bild,
/// sechsmal gezeichnet, belegte 2 GB). Kleinere Bilder werden zuerst dekodiert (QR-Platzhalter
/// sind klein), Bilder jenseits des Budgets bleiben leer.
pub const MAX_IMAGE_PIXELS_PER_PAGE: u64 = 64_000_000;

/// Fehler des Renderers.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// PDFium-Bibliothek nicht gefunden/ladbar.
    #[error("PDFium nicht verfügbar: {0}")]
    LibraryNotFound(String),
    /// Dokument nicht ladbar.
    #[error("Dokument nicht ladbar: {0}")]
    Load(String),
    /// Passwort erforderlich.
    #[error("Passwort erforderlich")]
    PasswordRequired,
    /// Seitenindex außerhalb.
    #[error("Seite {page} außerhalb (Seitenzahl {count})")]
    PageOutOfRange {
        /// angefragt (0-basiert)
        page: usize,
        /// vorhanden
        count: usize,
    },
    /// Fehler beim Rendern.
    #[error("Rendern fehlgeschlagen: {0}")]
    Render(String),
}

impl RenderError {
    /// Stabiler Code für UI/CLI-Meldungen (ADR 0001 Regel 11; Texte im Abschlussbericht).
    pub fn code(&self) -> &'static str {
        match self {
            RenderError::LibraryNotFound(_) => "render.library_not_found",
            RenderError::Load(_) => "render.load_failed",
            RenderError::PasswordRequired => "render.password_required",
            RenderError::PageOutOfRange { .. } => "render.page_out_of_range",
            RenderError::Render(_) => "render.failed",
        }
    }
}

/// Ergebnis-Alias.
pub type Result<T, E = RenderError> = std::result::Result<T, E>;

/// RGBA-Bild, 8 Bit je Kanal, zeilenweise, ohne Padding.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Breite in Pixel.
    pub width: u32,
    /// Höhe in Pixel.
    pub height: u32,
    /// `width * height * 4` Bytes.
    pub data: Vec<u8>,
}

impl std::fmt::Debug for RgbaImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RgbaImage").field("width", &self.width).field("height", &self.height).finish()
    }
}

impl RgbaImage {
    /// Einfarbiges Bild.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..(width as usize * height as usize) {
            data.extend_from_slice(&rgba);
        }
        Self { width, height, data }
    }

    /// Leeres Bild (0 × 0), z. B. für nicht dekodierbare Bildobjekte.
    pub fn empty() -> Self {
        Self { width: 0, height: 0, data: Vec::new() }
    }

    /// Aus Rohdaten; `None`, wenn `data.len() != width * height * 4`.
    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        (data.len() as u64 == u64::from(width) * u64::from(height) * 4).then_some(Self { width, height, data })
    }

    /// `true`, wenn Breite oder Höhe 0 ist.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Pixel bei (x, y) (oben links = 0,0).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }

    /// Setzt das Pixel bei (x, y); außerhalb liegende Koordinaten werden ignoriert.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x < self.width && y < self.height {
            let i = (y as usize * self.width as usize + x as usize) * 4;
            self.data[i..i + 4].copy_from_slice(&rgba);
        }
    }

    /// Eine Pixelzeile (`width * 4` Bytes).
    pub fn row(&self, y: u32) -> &[u8] {
        let stride = self.width as usize * 4;
        let start = y as usize * stride;
        &self.data[start..start + stride]
    }

    /// Luminanz (8 Bit) für QR-Dekodierung (`rxing`, HybridBinarizer wie ZXing).
    ///
    /// Gleiche Umrechnung wie ZXing `BufferedImageLuminanceSource` (PDF-AS übergibt
    /// `PDImageXObject.getImage()` so an ZXing): vollständig transparente Pixel (α = 0) gelten
    /// als weiß, sonst `(306·R + 601·G + 117·B + 0x200) >> 10`; teiltransparente Pixel werden
    /// ohne Alpha bewertet.
    pub fn to_luma8(&self) -> Vec<u8> {
        self.data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                if p[3] == 0 {
                    0xFF
                } else {
                    ((306 * u32::from(p[0]) + 601 * u32::from(p[1]) + 117 * u32::from(p[2]) + 0x200) >> 10) as u8
                }
            })
            .collect()
    }
}

/// Seitengröße in pt, wie PDFium sie liefert (sichtbare Größe nach `/Rotate`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageSize {
    /// Breite in pt.
    pub width: f32,
    /// Höhe in pt.
    pub height: f32,
}

/// Optionen für das Forced-Color-Raster (docs/analyse/04 §4.4, Plan §6.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForcedColorOptions {
    /// Hintergrund, Default RGB(152, 254, 52) („MAGIC“).
    pub background: [u8; 4],
    /// Ersatzfarbe für alle Füll-, Strich- und Textfarben, Default (234, 14, 184, α 211).
    pub foreground: [u8; 4],
    /// Auflösung; 72 dpi ⇒ 1 px = 1 pt.
    pub dpi: f32,
    /// Bilder unverändert lassen (PDF-AS-Verhalten).
    pub keep_images: bool,
}

impl Default for ForcedColorOptions {
    fn default() -> Self {
        Self { background: [152, 254, 52, 255], foreground: [234, 14, 184, 211], dpi: 72.0, keep_images: true }
    }
}

impl ForcedColorOptions {
    /// Rastergröße in Pixel für eine sichtbare Seitengröße in pt: `(int)(pt · dpi/72)` wie
    /// PDF-AS (`new BufferedImage((int) w, (int) h)`), mindestens 1.
    pub fn raster_size(&self, page: PageSize) -> (u32, u32) {
        let s = self.dpi / 72.0;
        let w = (page.width * s).floor().max(1.0);
        let h = (page.height * s).floor().max(1.0);
        (w as u32, h as u32)
    }
}

/// Bildobjekt einer Seite (für QR-Platzhalter, Plan §6.8).
///
/// **Zuordnung zu `pdfover_pdf_cos::ImageInvocation`** (ADR 0001 §8, verbindlich für Paket A und F):
/// PDFium zählt auch Inline-Bilder (`BI … EI`) als Bildobjekte, `pdf-cos` nur `Do`-Aufrufe von
/// Bild-XObjects. Die Zuordnung erfolgt deshalb **primär über die Matrix** (gleiche Seite,
/// komponentenweise Abweichung ≤ 0,01), `order` dient nur als Tie-Breaker bei gleicher Matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageObject {
    /// 0-basierter Index unter den Bildobjekten der Seite in PDFium-Reihenfolge (Seitenobjekte in
    /// Content-Stream-Reihenfolge, Form-Objekte in Tiefensuche an ihrer Stelle; inklusive
    /// Inline-Bilder) – nicht zwingend gleich `ImageInvocation::order`, siehe oben.
    pub order: usize,
    /// Matrix des Bildobjekts (Bild = Einheitsquadrat) im Seitenraum (bei Form-Objekten bereits mit
    /// der Form-Matrix verkettet), PDF-Konvention `[a b c d e f]` – vergleichbar mit
    /// `ImageInvocation::ctm`.
    pub matrix: [f32; 6],
    /// Dekodiertes Bild in Originalauflösung (alle Filter, `FPDFImageObj_GetBitmap`). Masken
    /// (`/SMask`, `/Mask` als Stencil oder Farbschlüssel) sind seit Welle 3 **angewandt**: Pixel
    /// außerhalb der Maske haben Alpha < 255 (wie PDF-AS `PDImageXObject.getImage()`, KR-F07;
    /// nur Bilder bis 4 MP – QR-Platzhalter sind klein –, größere bleiben unmaskiert). Nicht
    /// dekodierbare oder zu große Bilder (> [`MAX_PIXELS`]) sind leer (0 × 0), ebenso Bilder,
    /// die das Budget [`MAX_IMAGE_PIXELS_PER_PAGE`] der Seite sprengen würden (dekodiert wird vom
    /// kleinsten zum größten Bild; die Größe kommt vor dem Dekodieren aus `/Width` × `/Height`).
    pub bitmap: RgbaImage,
}

/// Verkettet zwei Matrizen in PDF-Konvention: erst `first`, dann `then`
/// (`p' = p · first · then`, Zeilenvektor wie ISO 32000-1 §8.3.4).
pub fn concat_matrix(first: [f32; 6], then: [f32; 6]) -> [f32; 6] {
    let [a1, b1, c1, d1, e1, f1] = first;
    let [a2, b2, c2, d2, e2, f2] = then;
    [
        a1 * a2 + b1 * c2,
        a1 * b2 + b1 * d2,
        c1 * a2 + d1 * c2,
        c1 * b2 + d1 * d2,
        e1 * a2 + f1 * c2 + e2,
        e1 * b2 + f1 * d2 + f2,
    ]
}

/// Ein geöffnetes Dokument.
pub trait Renderer: Send {
    /// Seitenzahl.
    fn page_count(&self) -> usize;

    /// Sichtbare Seitengröße in pt.
    fn page_size(&self, page: usize) -> Result<PageSize>;

    /// Rendert eine Seite mit `scale` Pixel je pt (inklusive `/Rotate` und Annotationen).
    fn render_page(&self, page: usize, scale: f32) -> Result<RgbaImage>;

    /// Rendert eine Seite mit ersetzten Farben (`FPDF_RenderPageBitmapWithColorScheme_Start`).
    /// Raster [`ForcedColorOptions::raster_size`]; die Seite liegt ohne Streckung im Maßstab
    /// `dpi/72` an der sichtbaren linken oberen Ecke, der Rest rechts und unten fällt weg – wie
    /// Java2D bei PDF-AS (seit Welle 3, Register KR-F10 in `pdfover-placement`).
    fn render_forced_color(&self, page: usize, opts: &ForcedColorOptions) -> Result<RgbaImage>;

    /// Alle Bildobjekte der Seite mit Matrix und dekodiertem Bild (Speicherbudget siehe
    /// [`ImageObject::bitmap`]).
    fn image_objects(&self, page: usize) -> Result<Vec<ImageObject>>;
}

/// Erzeugt [`Renderer`]-Instanzen (bindet die Bibliothek einmal).
pub trait RenderBackend: Send + Sync {
    /// Öffnet ein Dokument aus Bytes.
    fn open(&self, data: Arc<[u8]>, password: Option<&str>) -> Result<Box<dyn Renderer>>;

    /// Name/Version für Diagnose („PDFium chromium/8100“).
    fn describe(&self) -> String;
}

/// Kodiert ein [`RgbaImage`] als PNG (für das Custom Protocol `pdfover-render://` der Shell).
pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>> {
    if image.is_empty() {
        return Err(RenderError::Render("leeres Bild kann nicht als PNG kodiert werden".into()));
    }
    if image.data.len() as u64 != u64::from(image.width) * u64::from(image.height) * 4 {
        return Err(RenderError::Render(format!(
            "Bilddaten passen nicht zur Größe {}×{} ({} Bytes)",
            image.width,
            image.height,
            image.data.len()
        )));
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| RenderError::Render(format!("PNG: {e}")))?;
        writer.write_image_data(&image.data).map_err(|e| RenderError::Render(format!("PNG: {e}")))?;
        writer.finish().map_err(|e| RenderError::Render(format!("PNG: {e}")))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luma_like_zxing() {
        let mut img = RgbaImage::filled(4, 1, [0, 0, 0, 255]);
        img.set_pixel(1, 0, [255, 255, 255, 255]);
        img.set_pixel(2, 0, [0, 0, 0, 0]); // transparent ⇒ weiß
        img.set_pixel(3, 0, [255, 0, 0, 128]); // teiltransparent ⇒ ohne Alpha
        let l = img.to_luma8();
        assert_eq!(l, vec![0, 255, 255, ((306 * 255 + 0x200) >> 10) as u8]);
        // Grau bleibt Grau
        let g = RgbaImage::filled(1, 1, [77, 77, 77, 255]).to_luma8();
        assert_eq!(g, vec![77]);
    }

    #[test]
    fn rgba_helpers() {
        let mut img = RgbaImage::filled(3, 2, [1, 2, 3, 4]);
        assert_eq!(img.data.len(), 24);
        assert_eq!(img.pixel(2, 1), [1, 2, 3, 4]);
        img.set_pixel(2, 1, [9, 9, 9, 9]);
        img.set_pixel(5, 5, [9, 9, 9, 9]); // ignoriert
        assert_eq!(img.pixel(2, 1), [9, 9, 9, 9]);
        assert_eq!(img.row(1).len(), 12);
        assert!(RgbaImage::from_raw(2, 2, vec![0; 15]).is_none());
        assert!(RgbaImage::from_raw(2, 2, vec![0; 16]).is_some());
        assert!(RgbaImage::empty().is_empty());
        assert_eq!(format!("{img:?}"), "RgbaImage { width: 3, height: 2 }");
    }

    #[test]
    fn png_roundtrip() {
        let mut img = RgbaImage::filled(5, 3, [10, 20, 30, 255]);
        img.set_pixel(4, 2, [200, 100, 50, 7]);
        let bytes = encode_png(&img).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (5, 3));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(&buf[..info.buffer_size()], &img.data[..]);
    }

    #[test]
    fn png_rejects_bad_input() {
        assert!(encode_png(&RgbaImage::empty()).is_err());
        let bad = RgbaImage { width: 2, height: 2, data: vec![0; 3] };
        assert!(matches!(encode_png(&bad), Err(RenderError::Render(_))));
    }

    #[test]
    fn matrix_concat() {
        let translate = [1.0, 0.0, 0.0, 1.0, 10.0, 20.0];
        let scale = [2.0, 0.0, 0.0, 3.0, 0.0, 0.0];
        // erst skalieren, dann verschieben
        assert_eq!(concat_matrix(scale, translate), [2.0, 0.0, 0.0, 3.0, 10.0, 20.0]);
        // erst verschieben, dann skalieren
        assert_eq!(concat_matrix(translate, scale), [2.0, 0.0, 0.0, 3.0, 20.0, 60.0]);
        let rot90 = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0];
        let r = concat_matrix(rot90, rot90);
        assert_eq!(r, [-1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    }

    #[test]
    fn raster_size_truncates_like_pdfas() {
        let o = ForcedColorOptions::default();
        assert_eq!(o.raster_size(PageSize { width: 595.28, height: 841.89 }), (595, 841));
        let o4 = ForcedColorOptions { dpi: 288.0, ..o };
        assert_eq!(o4.raster_size(PageSize { width: 595.28, height: 841.89 }), (2381, 3367));
        assert_eq!(o.raster_size(PageSize { width: 0.2, height: 0.0 }), (1, 1));
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(RenderError::PasswordRequired.code(), "render.password_required");
        assert_eq!(RenderError::LibraryNotFound(String::new()).code(), "render.library_not_found");
        assert_eq!(RenderError::Load(String::new()).code(), "render.load_failed");
        assert_eq!(RenderError::PageOutOfRange { page: 1, count: 1 }.code(), "render.page_out_of_range");
        assert_eq!(RenderError::Render(String::new()).code(), "render.failed");
    }
}
