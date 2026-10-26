#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]
//! PDFium-Implementierung über `pdfium-render` (Feature `pdfium`).
//!
//! Sicherer Teil: Bindung, Dokumente, Seitengrößen, normales Rendern und Bildobjekte laufen
//! über die High-Level-API von `pdfium-render`. Nur das Forced-Color-Rendering braucht eine
//! PDFium-Funktion, die `pdfium-render` nicht sicher anbietet; sie liegt in [`crate::ffi`].

use pida_fs::PathExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use pdfium_render::prelude::{
    PdfBitmap, PdfBitmapFormat, PdfColor, PdfDocument, PdfPage, PdfPageImageObject, PdfPageObject, PdfPageObjectCommon,
    PdfPageObjectsCommon, PdfRenderConfig, Pdfium, PdfiumError, PdfiumInternalError, PdfiumLibraryBindings,
};
use zeroize::Zeroizing;

use crate::ffi::RawDocument;
use crate::{
    ENV_PDFIUM_DIR, ForcedColorOptions, ImageObject, MAX_IMAGE_PIXELS_PER_PAGE, MAX_PIXELS, PageSize, RenderBackend,
    RenderError, Renderer, Result, RgbaImage, concat_matrix,
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
    ///
    /// Ein Passwort mit NUL-Zeichen kann kein gültiges PDF-Passwort sein (PDFium erwartet einen
    /// C-String) und liefert [`RenderError::PasswordRequired`] – `pdfium-render` würde sonst mit
    /// dem Passwort im Klartext in der Panic-Meldung abbrechen und dabei seine globale Sperre
    /// vergiften (jeder weitere PDFium-Aufruf im Prozess bräche ab).
    pub fn open_document(&self, data: Arc<[u8]>, password: Option<&str>) -> Result<PdfiumRenderer> {
        check_password(password)?;
        let _guard = op_lock();
        #[cfg(not(target_arch = "wasm32"))]
        let doc = {
            let reader = std::io::Cursor::new(Arc::clone(&data));
            self.lib.pdfium.load_pdf_from_reader(reader, password).map_err(map_load_error)?
        };
        // Browser: PDFium übernimmt eine Kopie in seinen eigenen WebAssembly-Speicher
        #[cfg(target_arch = "wasm32")]
        let doc = self.lib.pdfium.load_pdf_from_byte_vec(data.to_vec(), password).map_err(map_load_error)?;
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

/// Passwörter mit NUL-Zeichen sind für PDFium nicht darstellbar (siehe
/// [`PdfiumBackend::open_document`]).
pub(crate) fn check_password(password: Option<&str>) -> Result<()> {
    if password.is_some_and(|p| p.contains('\0')) {
        return Err(RenderError::PasswordRequired);
    }
    Ok(())
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
        if cfg!(target_os = "ios") {
            out.push(("App-Bundle Frameworks".into(), exe_dir.join("Frameworks")));
        }
    }
    out.extend(system_candidates());
    out
}

/// Absolute Einträge einer Pfadliste (`PATH`, `DYLD_LIBRARY_PATH` …); leere und relative
/// Einträge (sie bezeichnen das Arbeitsverzeichnis) entfallen.
pub(crate) fn absolute_path_entries(value: &std::ffi::OsStr) -> Vec<PathBuf> {
    std::env::split_paths(value).filter(|p| p.is_absolute()).collect()
}

/// Systemverzeichnisse mit **absolutem** Pfad für Plattformen, deren Suche nach dem bloßen
/// Dateinamen auch das Arbeitsverzeichnis durchsucht (macOS-`dlopen` für nicht eingeschränkte
/// Prozesse, Windows-`LoadLibrary`): Eine `libpdfium.dylib`/`pdfium.dll` im Arbeitsverzeichnis
/// (z. B. ein entpacktes Archiv in „Downloads“) würde sonst fremden Code laden (CWE-427).
fn system_candidates() -> Vec<(String, PathBuf)> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        for var in ["DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"] {
            if let Some(v) = std::env::var_os(var) {
                dirs.extend(absolute_path_entries(&v));
            }
        }
        dirs.extend(["/opt/homebrew/lib", "/usr/local/lib", "/usr/lib"].map(PathBuf::from));
    } else if cfg!(windows)
        && let Some(v) = std::env::var_os("PATH")
    {
        dirs.extend(absolute_path_entries(&v));
    }
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for dir in dirs {
        if !out.iter().any(|(_, d)| *d == dir) {
            out.push(("Systempfad".into(), dir));
        }
    }
    out
}

/// Darf die Bibliothek über ihren bloßen Dateinamen gesucht werden? Unter Linux/Android sucht
/// `ld.so` nur in `LD_LIBRARY_PATH`, `RUNPATH`, `ld.so.cache` und den Standardverzeichnissen
/// (nie im Arbeitsverzeichnis) – unter Android gehört dazu `nativeLibraryDir` der App
/// (`jniLibs`, wie `System.loadLibrary`); macOS und Windows nutzen stattdessen [`system_candidates`],
/// iOS nur das App-Bundle.
const LEAF_NAME_SEARCH: bool = !cfg!(any(target_os = "macos", target_os = "ios", windows));
// Regression (CWE-427): auf macOS/iOS/Windows nie nach dem bloßen Dateinamen suchen.
#[cfg(any(target_os = "macos", target_os = "ios", windows))]
const _: () = assert!(!LEAF_NAME_SEARCH);

/// Browser (WebAssembly): PDFium ist ein eigenes WebAssembly-Modul, das die Seite vorher lädt und mit
/// `initialize_pdfium_render` (pdfium-render) an diesen Kern bindet – es gibt keine Datei zu suchen.
#[cfg(target_arch = "wasm32")]
fn bind_library(_library_dir: Option<&Path>) -> Result<Library> {
    match bind_pair(Pdfium::bind_to_system_library) {
        Ok((main, raw)) => Ok(Library { pdfium: Pdfium::new(main), raw, path: None, build: None }),
        Err(e) => Err(RenderError::LibraryNotFound(format!(
            "PDFium (WebAssembly) nicht gebunden – initialize_pdfium_render vor dem Kern aufrufen ({e})"
        ))),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn bind_library(library_dir: Option<&Path>) -> Result<Library> {
    let file_name = Pdfium::pdfium_platform_library_name();
    let candidates: Vec<(String, PathBuf)> = match library_dir {
        Some(dir) => vec![("angegebenes Verzeichnis".into(), dir.to_path_buf())],
        None => default_candidates(),
    };
    let mut tried = Vec::new();
    for (label, dir) in &candidates {
        for path in crate::library_paths_in(dir) {
            if !path.fs_is_file() {
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
    }
    if library_dir.is_none() && LEAF_NAME_SEARCH {
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
        if let Ok(text) = pida_fs::read_to_string(dir.join("VERSION"))
            && let Some(build) = text.lines().find_map(|l| l.trim().strip_prefix("BUILD="))
        {
            return Some(build.trim().to_owned());
        }
    }
    None
}

fn describe_pdfium_error(e: &PdfiumError) -> String {
    match e {
        #[cfg(not(target_arch = "wasm32"))]
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
    /// Zweite Instanz (lazy) für das Forced-Color-Raster und maskierte Bilder (`ffi`).
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
        // auch nach einer Panik in einem anderen Thread (vergiftete Sperre) unter `op_lock` schließen
        match self.raw.get_mut() {
            Ok(raw) => drop(raw.take()),
            Err(poisoned) => drop(poisoned.into_inner().take()),
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

    /// Gegenprobe zur Freiraumsuche (Plan §6.7): unterste Kante der Hüllrechtecke aller
    /// Seitenobjekte (`FPDFPageObj_GetBounds`, auf die sichtbare Seite beschnitten) oberhalb der
    /// Fußzeile `footer` als Abstand von der Oberkante der sichtbaren Seite in pt (1 pt-Raster
    /// wie die Freiraumsuche); `None`, wenn kein Objekt oberhalb der Fußzeile liegt.
    ///
    /// Hüllrechtecke enthalten alle gezeichneten Pixel eines Objekts, sind aber oft größer
    /// (Glyphenhüllen, Beschneidungspfade, unsichtbarer Text); die unterste belegte Rasterzeile
    /// liegt deshalb nie tiefer als diese Kante. Annotationen und Widgets zählen nicht.
    pub fn lowest_object_edge(&self, page: usize, footer: f32) -> Result<Option<f32>> {
        let _guard = op_lock();
        let size = self.page_size_locked(page)?;
        let index = self.check_page(page)?;
        let pdf_page = self.doc().pages().get(index).map_err(|e| render_err("Seite laden", e))?;
        let (w, h) = (size.width.round().max(1.0), size.height.round().max(1.0));
        if w > i32::MAX as f32 || h > i32::MAX as f32 {
            return Err(RenderError::Render("Seite zu groß".into()));
        }
        let config = PdfRenderConfig::new().set_fixed_size(w as i32, h as i32);
        let limit = h - 1.0 - footer.max(0.0);
        let mut lowest: Option<f32> = None;
        for object in pdf_page.objects().iter() {
            let Ok(b) = object.bounds() else { continue };
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (x, y) in [(b.left(), b.bottom()), (b.left(), b.top()), (b.right(), b.bottom()), (b.right(), b.top())] {
                let (px, py) = pdf_page.points_to_pixels(x, y, &config).map_err(|e| render_err("Koordinaten", e))?;
                let (px, py) = (px as f32, py as f32);
                (x0, y0, x1, y1) = (x0.min(px), y0.min(py), x1.max(px), y1.max(py));
            }
            if x1 < 0.0 || x0 > w || y1 < 0.0 || y0 > limit {
                continue;
            }
            let edge = y1.min(limit);
            lowest = Some(lowest.map_or(edge, |l| l.max(edge)));
        }
        Ok(lowest)
    }

    /// Färbt die Bildobjekte der Seite (achsenparallele Hülle im Raster) wie eine Fläche in der
    /// Ersatzfarbe ein (Vordergrund über Hintergrund) – nur für `keep_images = false`.
    fn paint_images(
        &self,
        page: usize,
        img: &mut RgbaImage,
        map: crate::ffi::DeviceMap,
        opts: &ForcedColorOptions,
    ) -> Result<()> {
        let color = blend(opts.foreground, [opts.background[0], opts.background[1], opts.background[2], 255]);
        let index = self.check_page(page)?;
        let pdf_page = self.doc().pages().get(index).map_err(|e| render_err("Seite laden", e))?;
        let mut matrices = Vec::new();
        visit_page_images(&pdf_page, &mut |_, m| matrices.push(m));
        // dieselbe Abbildung wie das Raster (1:1 ab der sichtbaren linken oberen Ecke, KR-F10)
        for m in matrices {
            let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
            let mut min = (i32::MAX, i32::MAX);
            let mut max = (i32::MIN, i32::MIN);
            for (u, v) in corners {
                let x = m[0] * u + m[2] * v + m[4];
                let y = m[1] * u + m[3] * v + m[5];
                let (u, v) = map.to_pixel(x, y);
                if !(u.is_finite() && v.is_finite()) {
                    continue;
                }
                let (px, py) = (
                    u.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32,
                    v.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32,
                );
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

impl PdfiumRenderer {
    /// Ersetzt dekodierte Bilder mit Maske (`/SMask`, `/Mask`) durch die maskierte Fassung (Alpha
    /// aus der Maske), wie PDF-AS `PDImageXObject.getImage()` (KR-F07, Welle 3). Nur Bilder bis
    /// [`MAX_MASKED_IMAGE_PIXELS`] (QR-Platzhalter sind klein), im verbleibenden Budget.
    fn apply_masks(&self, page: usize, out: &mut [ImageObject], mut left: u64) -> Result<()> {
        let mut wanted = Vec::with_capacity(out.len());
        for o in out.iter() {
            let px = u64::from(o.bitmap.width) * u64::from(o.bitmap.height);
            let want = !o.bitmap.is_empty() && px <= MAX_MASKED_IMAGE_PIXELS && px <= left;
            if want {
                left -= px;
            }
            wanted.push(want);
        }
        if !wanted.iter().any(|w| *w) {
            return Ok(());
        }
        let mut raw = self.raw.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if raw.is_none() {
            let password = self.password.as_ref().map(|p| p.as_str());
            *raw = Some(RawDocument::open(self.lib.raw.as_ref(), Arc::clone(&self.data), password)?);
        }
        let raw = raw.as_ref().expect("eben geöffnet");
        let masked = raw.masked_images(page, &wanted, MAX_MASKED_IMAGE_PIXELS)?;
        if masked.len() != out.len() {
            // andere Objektreihenfolge als in der Hauptinstanz: lieber unmaskiert lassen
            return Ok(());
        }
        for (o, m) in out.iter_mut().zip(masked) {
            if let Some(m) = m
                && (m.width, m.height) == (o.bitmap.width, o.bitmap.height)
            {
                o.bitmap = m;
            }
        }
        Ok(())
    }
}

/// Obergrenze der Pixel eines Bildes, dessen Maske angewandt wird ([`PdfiumRenderer::apply_masks`]).
const MAX_MASKED_IMAGE_PIXELS: u64 = 4_000_000;

/// `src` (mit Alpha) über opakes `dst` legen.
fn blend(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let a = u32::from(src[3]);
    let mix = |s: u8, d: u8| ((u32::from(s) * a + u32::from(d) * (255 - a) + 127) / 255) as u8;
    [mix(src[0], dst[0]), mix(src[1], dst[1]), mix(src[2], dst[2]), 255]
}

const MAX_FORM_DEPTH: usize = 32;
const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Besucht alle Bildobjekte unter `object` (Form-XObjects in Tiefensuche bis
/// [`MAX_FORM_DEPTH`]) mit ihrer Matrix im Seitenraum, in PDFium-Reihenfolge.
fn visit_images(
    object: &PdfPageObject<'_>,
    parent: [f32; 6],
    depth: usize,
    f: &mut dyn FnMut(&PdfPageImageObject<'_>, [f32; 6]),
) {
    let Ok(m) = object.matrix() else { return };
    let m = concat_matrix([m.a(), m.b(), m.c(), m.d(), m.e(), m.f()], parent);
    if let Some(image) = object.as_image_object() {
        f(image, m);
    } else if let Some(form) = object.as_x_object_form_object()
        && depth < MAX_FORM_DEPTH
    {
        for child in form.iter() {
            visit_images(&child, m, depth + 1, f);
        }
    }
}

/// Alle Bildobjekte einer Seite (deterministische Reihenfolge, siehe [`ImageObject::order`]).
fn visit_page_images(page: &PdfPage<'_>, f: &mut dyn FnMut(&PdfPageImageObject<'_>, [f32; 6])) {
    for object in page.objects().iter() {
        visit_images(&object, IDENTITY, 0, f);
    }
}

/// Pixelzahl laut `/Width` × `/Height` (ohne zu dekodieren); `None`, wenn unbekannt.
fn declared_pixels(image: &PdfPageImageObject<'_>) -> Option<u64> {
    let w = u64::try_from(image.width().ok()?).ok()?;
    let h = u64::try_from(image.height().ok()?).ok()?;
    Some(w * h)
}

/// Wählt die Bilder, die dekodiert werden: jedes höchstens [`MAX_PIXELS`], vom kleinsten zum
/// größten (bei Gleichstand in Seitenreihenfolge), solange die Summe `budget` nicht übersteigt.
/// Bilder unbekannter Größe werden nicht dekodiert.
pub(crate) fn select_within_budget(pixels: &[Option<u64>], budget: u64) -> Vec<bool> {
    let mut candidates: Vec<(u64, usize)> = pixels
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.filter(|&p| p > 0 && p <= MAX_PIXELS).map(|p| (p, i)))
        .collect();
    candidates.sort_unstable();
    let mut left = budget;
    let mut selected = vec![false; pixels.len()];
    for (p, i) in candidates {
        if p > left {
            break;
        }
        left -= p;
        selected[i] = true;
    }
    selected
}

/// Dekodiert ein Bildobjekt; das tatsächliche Ergebnis zählt gegen `left` (`/Width`/`/Height`
/// können bei JPEG von den dekodierten Maßen abweichen).
fn decode_image(image: &PdfPageImageObject<'_>, left: &mut u64) -> RgbaImage {
    let limit = (*left).min(MAX_PIXELS);
    let Some(img) = image.get_raw_bitmap().ok().and_then(|b| bitmap_to_rgba(&b, false, limit)) else {
        return RgbaImage::empty();
    };
    *left -= u64::from(img.width) * u64::from(img.height);
    img
}

/// Wandelt ein PDFium-Bitmap (Gray/BGR/BGRx/BGRA, Zeilen mit Stride) in RGBA; `None` bei mehr als
/// `max_pixels` Pixeln oder unlesbarem Format.
fn bitmap_to_rgba(bitmap: &PdfBitmap<'_>, bytes_are_rgb_order: bool, max_pixels: u64) -> Option<RgbaImage> {
    let w = u32::try_from(bitmap.width()).ok()?;
    let h = u32::try_from(bitmap.height()).ok()?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
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
        bitmap_to_rgba(&bitmap, true, MAX_PIXELS).ok_or_else(|| RenderError::Render("Bitmap nicht lesbar".into()))
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
        let (mut img, map) = raw.render_forced_color(page, opts)?;
        if !opts.keep_images {
            self.paint_images(page, &mut img, map, opts)?;
        }
        Ok(img)
    }

    fn image_objects(&self, page: usize) -> Result<Vec<ImageObject>> {
        let _guard = op_lock();
        let index = self.check_page(page)?;
        let pdf_page = self.doc().pages().get(index).map_err(|e| render_err("Seite laden", e))?;
        // 1. Durchgang: Matrizen und deklarierte Größen (nichts dekodiert), Auswahl im Budget.
        let mut declared = Vec::new();
        visit_page_images(&pdf_page, &mut |image, _| declared.push(declared_pixels(image)));
        let selected = select_within_budget(&declared, MAX_IMAGE_PIXELS_PER_PAGE);
        // 2. Durchgang in derselben Reihenfolge: nur ausgewählte Bilder dekodieren.
        let mut left = MAX_IMAGE_PIXELS_PER_PAGE;
        let mut out = Vec::with_capacity(declared.len());
        visit_page_images(&pdf_page, &mut |image, matrix| {
            let order = out.len();
            let bitmap = if selected.get(order).copied().unwrap_or(false) {
                decode_image(image, &mut left)
            } else {
                RgbaImage::empty()
            };
            out.push(ImageObject { order, matrix, bitmap });
        });
        drop(pdf_page);
        // Masken sind eine Verbesserung (KR-F07): scheitert die zweite Instanz, bleiben die Bilder
        // unmaskiert wie bisher
        let _ = self.apply_masks(page, &mut out, left);
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
    fn budget_prefers_small_images() {
        // kleinste zuerst, Summe ≤ Budget, zu große und unbekannte nie
        let px = [Some(60), Some(10), None, Some(30), Some(0), Some(MAX_PIXELS + 1), Some(10)];
        assert_eq!(select_within_budget(&px, 50), [false, true, false, true, false, false, true]);
        assert_eq!(select_within_budget(&px, 0), [false; 7]);
        assert_eq!(select_within_budget(&px, u64::MAX), [true, true, false, true, false, false, true]);
        assert!(select_within_budget(&[], 10).is_empty());
    }

    #[test]
    fn nul_in_password_is_rejected_before_pdfium() {
        assert!(matches!(check_password(Some("a\0b")), Err(RenderError::PasswordRequired)));
        assert!(check_password(Some("äöü")).is_ok());
        assert!(check_password(None).is_ok());
    }

    #[test]
    fn library_search_never_uses_the_working_directory() {
        // relative und leere Einträge bezeichnen das Arbeitsverzeichnis und entfallen
        let abs = std::env::temp_dir();
        let rel = std::ffi::OsStr::new("rel");
        let joined = std::env::join_paths([abs.as_os_str(), rel, std::ffi::OsStr::new(""), abs.as_os_str()]).unwrap();
        assert!(abs.is_absolute());
        assert_eq!(absolute_path_entries(&joined), [abs.clone(), abs]);
        for (label, dir) in system_candidates() {
            assert!(dir.is_absolute(), "{label}: {}", dir.display());
        }
    }

    #[test]
    fn library_paths_per_platform() {
        let dir = Path::new("/bundle/Frameworks");
        let paths = crate::library_paths_in(dir);
        assert_eq!(paths.last(), Some(&dir.join(crate::library_file_name())));
        assert_eq!(crate::library_file_name(), Pdfium::pdfium_platform_library_name().to_string_lossy());
        if cfg!(target_os = "ios") {
            assert_eq!(paths.first(), Some(&dir.join("pdfium.framework/pdfium")));
        } else {
            assert_eq!(paths.len(), 1);
        }
        // Mobile: iOS nur aus dem Bundle, Android über den Linker-Namensraum (nativeLibraryDir)
        assert_eq!(LEAF_NAME_SEARCH, !cfg!(any(target_os = "macos", target_os = "ios", windows)));
    }

    #[test]
    fn missing_library_dir_gives_clear_error() {
        // Nur prüfen, solange noch nichts gebunden ist (Tests laufen im selben Prozess).
        if LIBRARY.get().is_some() {
            return;
        }
        let dir = std::env::temp_dir().join("pida-render-gibt-es-nicht");
        let err = bind_library(Some(&dir)).unwrap_err();
        match err {
            RenderError::LibraryNotFound(msg) => {
                assert!(msg.contains("pida-render-gibt-es-nicht"), "{msg}");
                assert!(msg.contains("nicht vorhanden"), "{msg}");
            }
            other => panic!("unerwartet: {other:?}"),
        }
    }
}
