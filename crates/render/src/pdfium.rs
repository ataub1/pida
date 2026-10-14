//! PDFium-Implementierung über `pdfium-render` (Feature `pdfium`).
//!
//! Sicherer Teil: Bindung, Dokumente, Seitengrößen, normales Rendern und Bildobjekte laufen
//! über die High-Level-API von `pdfium-render`. Nur das Forced-Color-Rendering braucht eine
//! PDFium-Funktion, die `pdfium-render` nicht sicher anbietet; sie liegt in [`crate::ffi`].

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use pdfium_render::prelude::{
    PdfBitmap, PdfBitmapFormat, PdfColor, PdfDocument, PdfPageObject, PdfPageObjectsCommon, PdfPoints, PdfRenderConfig,
    Pdfium, PdfiumError, PdfiumInternalError, PdfiumLibraryBindings,
};
use zeroize::Zeroizing;

use crate::ffi::RawDocument;
use crate::{
    ENV_PDFIUM_DIR, ForcedColorOptions, ImageObject, MAX_PIXELS, PageSize, RenderBackend, RenderError, Renderer,
    Result, RgbaImage, concat_matrix,
};

/// Gebundene PDFium-Bibliothek (einmal je Prozess).
pub(crate) struct Library {
    pdfium: Pdfium,
    /// Zweite Bindung derselben Bibliothek für Funktionen ohne sichere Hülle (`ffi`). Sie teilt
    /// sich mit `pdfium` die prozessweite Sperre von `pdfium-render` (`thread_safe`).
    raw: Box<dyn PdfiumLibraryBindings>,
    /// Pfad der Bibliothek (`None` = Systempfad).
    path: Option<PathBuf>,
    /// Inhalt der `VERSION`-Datei der `pdfium-binaries` (z. B. `BUILD=8100`), falls vorhanden.
    build: Option<String>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library").field("path", &self.path).field("build", &self.build).finish_non_exhaustive()
    }
}

static LIBRARY: OnceLock<Library> = OnceLock::new();
static BIND_LOCK: Mutex<()> = Mutex::new(());
/// Prozessweite Sperre für jede mehrschrittige PDFium-Operation dieses Crates.
static OP_LOCK: Mutex<()> = Mutex::new(());

fn op_lock() -> MutexGuard<'static, ()> {
    OP_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// PDFium-Backend (Feature `pdfium`).
#[derive(Debug, Clone, Copy)]
pub struct PdfiumBackend {
    lib: &'static Library,
}

impl PdfiumBackend {
    /// Bindet PDFium aus `library_dir` bzw. dem Systempfad (Desktop: neben der Binary).
    ///
    /// Ohne `library_dir`: [`ENV_PDFIUM_DIR`], dann das Verzeichnis der laufenden Binary (bei
    /// macOS-Bundles auch `../Frameworks`, `../Resources`), dann der Systempfad. Mit
    /// `library_dir` nur dieses Verzeichnis. Die Bindung gilt prozessweit; weitere Aufrufe
    /// liefern sie erneut (auch mit anderem `library_dir`, siehe [`PdfiumBackend::library_path`]).
    pub fn bind(library_dir: Option<&Path>) -> Result<Self> {
        if let Some(lib) = LIBRARY.get() {
            return Ok(Self { lib });
        }
        let _guard = BIND_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(lib) = LIBRARY.get() {
            return Ok(Self { lib });
        }
        let lib = bind_library(library_dir)?;
        let lib = LIBRARY.get_or_init(|| lib);
        Ok(Self { lib })
    }

    /// Pfad der gebundenen Bibliothek (`None` = über den Systempfad geladen).
    pub fn library_path(&self) -> Option<&Path> {
        self.lib.path.as_deref()
    }

    /// Öffnet ein Dokument und liefert den konkreten Renderer (wie [`RenderBackend::open`]).
    pub fn open_document(&self, data: Arc<[u8]>, password: Option<&str>) -> Result<PdfiumRenderer> {
        let _guard = op_lock();
        let reader = Cursor::new(Arc::clone(&data));
        let doc = self.lib.pdfium.load_pdf_from_reader(reader, password).map_err(map_load_error)?;
        let page_count = usize::try_from(doc.pages().len()).unwrap_or(0);
        Ok(PdfiumRenderer {
            lib: self.lib,
            doc: Some(doc),
            raw: Mutex::new(None),
            data,
            password: password.map(|p| Zeroizing::new(p.to_owned())),
            page_count,
        })
    }
}

impl RenderBackend for PdfiumBackend {
    fn open(&self, data: Arc<[u8]>, password: Option<&str>) -> Result<Box<dyn Renderer>> {
        Ok(Box::new(self.open_document(data, password)?))
    }

    fn describe(&self) -> String {
        let build =
            self.lib.build.as_deref().map(|b| format!("PDFium chromium/{b}")).unwrap_or_else(|| "PDFium".into());
        match &self.lib.path {
            Some(p) => format!("{build} ({})", p.display()),
            None => format!("{build} (Systempfad)"),
        }
    }
}

/// Kandidaten für das Bibliotheksverzeichnis ohne explizite Angabe.
fn default_candidates() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Some(dir) = std::env::var_os(ENV_PDFIUM_DIR).filter(|v| !v.is_empty()) {
        out.push((format!("{ENV_PDFIUM_DIR}={}", Path::new(&dir).display()), PathBuf::from(dir)));
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        out.push(("neben der Anwendung".into(), exe_dir.clone()));
        if cfg!(target_os = "macos") {
            out.push(("App-Bundle Frameworks".into(), exe_dir.join("../Frameworks")));
            out.push(("App-Bundle Resources".into(), exe_dir.join("../Resources")));
        }
    }
    out
}

fn bind_library(library_dir: Option<&Path>) -> Result<Library> {
    let file_name = Pdfium::pdfium_platform_library_name();
    let candidates: Vec<(String, PathBuf)> = match library_dir {
        Some(dir) => vec![("angegebenes Verzeichnis".into(), dir.to_path_buf())],
        None => default_candidates(),
    };
    let mut tried = Vec::new();
    for (label, dir) in &candidates {
        let path = dir.join(&file_name);
        if !path.is_file() {
            tried.push(format!("{label}: {} (nicht vorhanden)", path.display()));
            continue;
        }
        match bind_pair(|| Pdfium::bind_to_library(&path)) {
            Ok((main, raw)) => {
                let build = read_build(dir);
                return Ok(Library { pdfium: Pdfium::new(main), raw, path: Some(path), build });
            }
            Err(e) => tried.push(format!("{label}: {} ({e})", path.display())),
        }
    }
    if library_dir.is_none() {
        match bind_pair(Pdfium::bind_to_system_library) {
            Ok((main, raw)) => return Ok(Library { pdfium: Pdfium::new(main), raw, path: None, build: None }),
            Err(e) => tried.push(format!("Systempfad: {} ({e})", file_name.to_string_lossy())),
        }
    }
    Err(RenderError::LibraryNotFound(format!(
        "{} nicht gefunden oder nicht ladbar (erwartet: pdfium-binaries chromium/8100); durchsucht: {}",
        file_name.to_string_lossy(),
        tried.join("; ")
    )))
}

/// Eine Bindung der PDFium-Bibliothek.
type Bindings = Box<dyn PdfiumLibraryBindings>;

/// Erzeugt zwei Bindungen derselben Bibliothek (die erste übernimmt `Pdfium::new`).
fn bind_pair(
    bind: impl Fn() -> std::result::Result<Bindings, PdfiumError>,
) -> std::result::Result<(Bindings, Bindings), String> {
    let main = bind().map_err(|e| describe_pdfium_error(&e))?;
    let raw = bind().map_err(|e| describe_pdfium_error(&e))?;
    Ok((main, raw))
}

fn read_build(lib_dir: &Path) -> Option<String> {
    for dir in [lib_dir.to_path_buf(), lib_dir.join("..")] {
        if let Ok(text) = std::fs::read_to_string(dir.join("VERSION"))
            && let Some(build) = text.lines().find_map(|l| l.trim().strip_prefix("BUILD="))
        {
            return Some(build.trim().to_owned());
        }
    }
    None
}

fn describe_pdfium_error(e: &PdfiumError) -> String {
    match e {
        PdfiumError::LoadLibraryError(err) => format!("Bibliothek nicht ladbar: {err}"),
        other => format!("{other:?}"),
    }
}

fn map_load_error(e: PdfiumError) -> RenderError {
    match e {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => RenderError::PasswordRequired,
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FormatError) => {
            RenderError::Load("kein gültiges PDF (Formatfehler)".into())
        }
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FileError) => {
            RenderError::Load("Datei nicht lesbar".into())
        }
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::SecurityError) => {
            RenderError::Load("Sicherheits-Handler nicht unterstützt".into())
        }
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PageError) => {
            RenderError::Load("Seite nicht gefunden oder fehlerhaft".into())
        }
        other => RenderError::Load(describe_pdfium_error(&other)),
    }
}

fn render_err(context: &str, e: PdfiumError) -> RenderError {
    RenderError::Render(format!("{context}: {}", describe_pdfium_error(&e)))
}

/// Prüft die Pixelzahl eines Rasters.
pub(crate) fn check_pixels(w: u32, h: u32) -> Result<()> {
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(RenderError::Render(format!("Raster {w}×{h} überschreitet {MAX_PIXELS} Pixel")));
    }
    Ok(())
}

/// PDFium-Renderer für ein Dokument (Feature `pdfium`).
pub struct PdfiumRenderer {
    lib: &'static Library,
    /// `Option`, damit `Drop` das Dokument unter der Sperre schließen kann.
    doc: Option<PdfDocument<'static>>,
    /// Zweite, nur für das Forced-Color-Raster geöffnete Instanz (lazy).
    raw: Mutex<Option<RawDocument>>,
    data: Arc<[u8]>,
    /// Nur im Speicher, für das lazy Öffnen von `raw`; wird beim Drop überschrieben.
    password: Option<Zeroizing<String>>,
    page_count: usize,
}

impl std::fmt::Debug for PdfiumRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Regel 9: keine Dokumentinhalte, kein Passwort.
        f.debug_struct("PdfiumRenderer")
            .field("len", &self.data.len())
            .field("pages", &self.page_count)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish_non_exhaustive()
    }
}

impl Drop for PdfiumRenderer {
    fn drop(&mut self) {
        let _guard = op_lock();
        if let Ok(mut raw) = self.raw.lock() {
            raw.take();
        }
        self.doc.take();
    }
}

impl PdfiumRenderer {
    fn doc(&self) -> &PdfDocument<'static> {
        self.doc.as_ref().expect("Dokument lebt bis zum Drop")
    }

    fn check_page(&self, page: usize) -> Result<i32> {
        if page >= self.page_count {
            return Err(RenderError::PageOutOfRange { page, count: self.page_count });
        }
        i32::try_from(page).map_err(|_| RenderError::PageOutOfRange { page, count: self.page_count })
    }

    fn page_size_locked(&self, page: usize) -> Result<PageSize> {
        let index = self.check_page(page)?;
        let rect = self.doc().pages().page_size(index).map_err(|e| render_err("Seitengröße", e))?;
        Ok(PageSize { width: rect.width().value, height: rect.height().value })
    }

    /// Färbt die Bildobjekte der Seite (achsenparallele Hülle im Raster) wie eine Fläche in der
    /// Ersatzfarbe ein (Vordergrund über Hintergrund) – nur für `keep_images = false`.
    fn paint_images(
        &self,
        page: usize,
        img: &mut RgbaImage,
        size: (i32, i32),
        opts: &ForcedColorOptions,
    ) -> Result<()> {
        let color = blend(opts.foreground, [opts.background[0], opts.background[1], opts.background[2], 255]);
        let index = self.check_page(page)?;
        let pdf_page = self.doc().pages().get(index).map_err(|e| render_err("Seite laden", e))?;
        let mut matrices = Vec::new();
        for object in pdf_page.objects().iter() {
            collect_image_matrices(&object, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut matrices, 0);
        }
        let config = PdfRenderConfig::new().set_fixed_size(size.0, size.1);
        for m in matrices {
            let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
            let mut min = (i32::MAX, i32::MAX);
            let mut max = (i32::MIN, i32::MIN);
            for (u, v) in corners {
                let x = m[0] * u + m[2] * v + m[4];
                let y = m[1] * u + m[3] * v + m[5];
                let (px, py) = pdf_page
                    .points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)
                    .map_err(|e| render_err("Koordinaten", e))?;
                min = (min.0.min(px), min.1.min(py));
                max = (max.0.max(px), max.1.max(py));
            }
            let x0 = min.0.max(0) as u32;
            let y0 = min.1.max(0) as u32;
            let x1 = (max.0.max(0) as u32).min(img.width);
            let y1 = (max.1.max(0) as u32).min(img.height);
            for y in y0..y1 {
                for x in x0..x1 {
                    img.set_pixel(x, y, color);
                }
            }
        }
        Ok(())
    }
}

/// `src` (mit Alpha) über opakes `dst` legen.
fn blend(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let a = u32::from(src[3]);
    let mix = |s: u8, d: u8| ((u32::from(s) * a + u32::from(d) * (255 - a) + 127) / 255) as u8;
    [mix(src[0], dst[0]), mix(src[1], dst[1]), mix(src[2], dst[2]), 255]
}

const MAX_FORM_DEPTH: usize = 32;

fn collect_image_matrices(object: &PdfPageObject<'_>, parent: [f32; 6], out: &mut Vec<[f32; 6]>, depth: usize) {
    let Ok(m) = object.matrix() else { return };
    let m = concat_matrix([m.a(), m.b(), m.c(), m.d(), m.e(), m.f()], parent);
    if object.as_image_object().is_some() {
        out.push(m);
    } else if let Some(form) = object.as_x_object_form_object()
        && depth < MAX_FORM_DEPTH
    {
        for child in form.iter() {
            collect_image_matrices(&child, m, out, depth + 1);
        }
    }
}

/// Wandelt ein PDFium-Bitmap (Gray/BGR/BGRx/BGRA, Zeilen mit Stride) in RGBA.
fn bitmap_to_rgba(bitmap: &PdfBitmap<'_>, bytes_are_rgb_order: bool) -> Option<RgbaImage> {
    let w = u32::try_from(bitmap.width()).ok()?;
    let h = u32::try_from(bitmap.height()).ok()?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let format = bitmap.format().ok()?;
    let raw = bitmap.as_raw_bytes();
    let stride = raw.len() / h as usize;
    let bpp = match format {
        PdfBitmapFormat::Gray => 1,
        PdfBitmapFormat::BGR => 3,
        PdfBitmapFormat::BGRx | PdfBitmapFormat::BGRA => 4,
    };
    if stride < w as usize * bpp {
        return None;
    }
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for row in raw.chunks_exact(stride).take(h as usize) {
        for px in row[..w as usize * bpp].chunks_exact(bpp) {
            let rgba = match format {
                PdfBitmapFormat::Gray => [px[0], px[0], px[0], 255],
                PdfBitmapFormat::BGR | PdfBitmapFormat::BGRx => {
                    if bytes_are_rgb_order {
                        [px[0], px[1], px[2], 255]
                    } else {
                        [px[2], px[1], px[0], 255]
                    }
                }
                PdfBitmapFormat::BGRA => {
                    if bytes_are_rgb_order {
                        [px[0], px[1], px[2], px[3]]
                    } else {
                        [px[2], px[1], px[0], px[3]]
                    }
                }
            };
            data.extend_from_slice(&rgba);
        }
    }
    RgbaImage::from_raw(w, h, data)
}

fn collect_images(object: &PdfPageObject<'_>, parent: [f32; 6], out: &mut Vec<ImageObject>, depth: usize) {
    let Ok(m) = object.matrix() else { return };
    let m = concat_matrix([m.a(), m.b(), m.c(), m.d(), m.e(), m.f()], parent);
    if let Some(image) = object.as_image_object() {
        let bitmap =
            image.get_raw_bitmap().ok().and_then(|b| bitmap_to_rgba(&b, false)).unwrap_or_else(RgbaImage::empty);
        let order = out.len();
        out.push(ImageObject { order, matrix: m, bitmap });
    } else if let Some(form) = object.as_x_object_form_object()
        && depth < MAX_FORM_DEPTH
    {
        for child in form.iter() {
            collect_images(&child, m, out, depth + 1);
        }
    }
}

impl Renderer for PdfiumRenderer {
    fn page_count(&self) -> usize {
        self.page_count
    }

    fn page_size(&self, page: usize) -> Result<PageSize> {
        let _guard = op_lock();
        self.page_size_locked(page)
    }

    fn render_page(&self, page: usize, scale: f32) -> Result<RgbaImage> {
        if !(scale.is_finite() && scale > 0.0) {
            return Err(RenderError::Render(format!("ungültiger Maßstab {scale}")));
        }
        let _guard = op_lock();
        let size = self.page_size_locked(page)?;
        let w = (size.width * scale).round().max(1.0);
        let h = (size.height * scale).round().max(1.0);
        if w > i32::MAX as f32 || h > i32::MAX as f32 {
            return Err(RenderError::Render("Raster zu groß".into()));
        }
        let (w, h) = (w as u32, h as u32);
        check_pixels(w, h)?;
        let index = self.check_page(page)?;
        let pdf_page = self.doc().pages().get(index).map_err(|e| render_err("Seite laden", e))?;
        let config = PdfRenderConfig::new()
            .set_fixed_size(w as i32, h as i32)
            .set_format(PdfBitmapFormat::BGRA)
            .set_reverse_byte_order(true)
            .clear_before_rendering(true)
            .set_clear_color(PdfColor::WHITE)
            .render_annotations(true)
            .render_form_data(true);
        let bitmap = pdf_page.render_with_config(&config).map_err(|e| render_err("Rendern", e))?;
        bitmap_to_rgba(&bitmap, true).ok_or_else(|| RenderError::Render("Bitmap nicht lesbar".into()))
    }

    fn render_forced_color(&self, page: usize, opts: &ForcedColorOptions) -> Result<RgbaImage> {
        if !(opts.dpi.is_finite() && opts.dpi > 0.0) {
            return Err(RenderError::Render(format!("ungültige Auflösung {} dpi", opts.dpi)));
        }
        let _guard = op_lock();
        self.check_page(page)?;
        let mut raw = self.raw.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if raw.is_none() {
            let password = self.password.as_ref().map(|p| p.as_str());
            *raw = Some(RawDocument::open(self.lib.raw.as_ref(), Arc::clone(&self.data), password)?);
        }
        let raw = raw.as_ref().expect("eben geöffnet");
        let (mut img, area) = raw.render_forced_color(page, opts)?;
        if !opts.keep_images {
            self.paint_images(page, &mut img, area, opts)?;
        }
        Ok(img)
    }

    fn image_objects(&self, page: usize) -> Result<Vec<ImageObject>> {
        let _guard = op_lock();
        let index = self.check_page(page)?;
        let pdf_page = self.doc().pages().get(index).map_err(|e| render_err("Seite laden", e))?;
        let mut out = Vec::new();
        for object in pdf_page.objects().iter() {
            collect_images(&object, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut out, 0);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_over_opaque() {
        assert_eq!(blend([255, 0, 0, 255], [0, 0, 255, 255]), [255, 0, 0, 255]);
        assert_eq!(blend([255, 0, 0, 0], [0, 0, 255, 255]), [0, 0, 255, 255]);
        let half = blend([200, 0, 0, 128], [0, 0, 0, 255]);
        assert!(half[0] > 95 && half[0] < 105);
    }

    #[test]
    fn pixel_guard() {
        assert!(check_pixels(10_000, 10_000).is_ok());
        assert!(check_pixels(100_000, 100_000).is_err());
    }

    #[test]
    fn missing_library_dir_gives_clear_error() {
        // Nur prüfen, solange noch nichts gebunden ist (Tests laufen im selben Prozess).
        if LIBRARY.get().is_some() {
            return;
        }
        let dir = std::env::temp_dir().join("pdfover-render-gibt-es-nicht");
        let err = bind_library(Some(&dir)).unwrap_err();
        match err {
            RenderError::LibraryNotFound(msg) => {
                assert!(msg.contains("pdfover-render-gibt-es-nicht"), "{msg}");
                assert!(msg.contains("nicht vorhanden"), "{msg}");
            }
            other => panic!("unerwartet: {other:?}"),
        }
    }
}
