//! Direkte PDFium-Aufrufe für das Forced-Color-Raster (ADR 0001 Regel 8, ADR 0060).
//!
//! **Warum `unsafe`:** `pdfium-render` 0.9.4 bietet `FPDF_RenderPageBitmapWithColorScheme_Start`
//! nur als rohe Bindung an und gibt seine Dokument-/Seiten-Handles nicht heraus. Dieses Modul
//! öffnet deshalb eine **zweite** Dokumentinstanz über eine zweite Bindung derselben Bibliothek
//! (`Library::raw`) und rendert damit. Alle Aufrufe laufen unter der prozessweiten Sperre von
//! `pdfium-render` (`thread_safe`) und zusätzlich unter der Operationssperre des Crates
//! (`pdfium::op_lock`, vom Aufrufer gehalten).
//!
//! Invarianten:
//! - Der Eingabepuffer (`Arc<[u8]>`) lebt so lange wie das Dokument-Handle (Feld in
//!   [`RawDocument`], Drop schließt zuerst das Handle).
//! - `FPDF_FORMFILLINFO` liegt auf dem Heap (`Box`) und lebt so lange wie das Formular-Handle;
//!   alle Callbacks sind `None` (PDFium prüft jeden Callback vor dem Aufruf).
//! - Jede geladene Seite, jedes Bitmap und jeder Render-Kontext wird im selben Aufruf wieder
//!   freigegeben (RAII-Wächter [`PageGuard`], [`BitmapGuard`]).
//! - Für die 1:1-Abbildung wie Java2D (KR-F10, Welle 3) wird die CropBox der Seite **in dieser
//!   zweiten Instanz** kurzzeitig auf genau den gerasterten Ausschnitt gesetzt und danach
//!   wiederhergestellt ([`CropGuard`]); das Original-PDF und die Hauptinstanz bleiben unberührt.
#![allow(unsafe_code)]

use std::os::raw::c_int;
use std::sync::Arc;

use pdfium_render::prelude::{
    FPDF_BITMAP, FPDF_BOOL, FPDF_COLORSCHEME, FPDF_DOCUMENT, FPDF_DWORD, FPDF_FORMFILLINFO, FPDF_FORMHANDLE, FPDF_PAGE,
    FPDF_PAGEOBJ_FORM, FPDF_PAGEOBJ_IMAGE, FPDF_PAGEOBJECT, FS_MATRIX, FS_RECTF, IFSDK_PAUSE, PdfiumLibraryBindings,
};

use crate::pdfium::{check_password, check_pixels};
use crate::{ForcedColorOptions, PageSize, RenderError, Result, RgbaImage};

/// `FPDF_ANNOT` aus `fpdfview.h`: Annotationen (ohne Widgets) mitrendern.
const FPDF_ANNOT: c_int = 0x01;
/// `FPDF_RENDER_TOBECONTINUED` aus `fpdf_progressive.h`.
const FPDF_RENDER_TOBECONTINUED: c_int = 1;
/// `FPDF_RENDER_FAILED`.
const FPDF_RENDER_FAILED: c_int = 3;
/// `FPDF_ERR_PASSWORD` aus `fpdfview.h`.
const FPDF_ERR_PASSWORD: u64 = 4;
/// `FPDF_ERR_SECURITY`.
const FPDF_ERR_SECURITY: u64 = 5;
/// `FPDFBitmap_BGRA` aus `fpdfview.h`.
const FPDF_BITMAP_BGRA: c_int = 4;
/// Tiefe der Form-XObjects wie `pdfium::visit_images` (gleiche Reihenfolge der Bildobjekte).
const MAX_FORM_DEPTH: usize = 32;

/// `IFSDK_PAUSE::NeedToPauseNow`: nie pausieren (PDFium verlangt ein Pause-Objekt mit
/// `version == 1`, sonst liefert der progressive Start sofort `FPDF_RENDER_FAILED`).
unsafe extern "C" fn never_pause(_this: *mut IFSDK_PAUSE) -> FPDF_BOOL {
    0
}

/// Rückruf für das Pause-Objekt. Nativ [`never_pause`]. Im Browser ruft PDFium (ein eigenes WebAssembly-Modul) den
/// Zeiger über *seine* Funktionstabelle auf – ein Zeiger aus diesem Modul träfe dort eine fremde Funktion („function
/// signature mismatch“). Die Seite legt deshalb mit `Module.addFunction(() => 0, 'ii')` eine Funktion in PDFiums
/// Tabelle an und meldet ihren Index über [`set_wasm_never_pause`].
#[cfg(not(target_arch = "wasm32"))]
fn never_pause_fn() -> Result<unsafe extern "C" fn(*mut IFSDK_PAUSE) -> FPDF_BOOL> {
    Ok(never_pause)
}

#[cfg(target_arch = "wasm32")]
static WASM_NEVER_PAUSE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Browser: Index der „nie pausieren“-Funktion in der Funktionstabelle von PDFium (`Module.addFunction`).
#[cfg(target_arch = "wasm32")]
pub fn set_wasm_never_pause(table_index: usize) {
    WASM_NEVER_PAUSE.store(table_index, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
fn never_pause_fn() -> Result<unsafe extern "C" fn(*mut IFSDK_PAUSE) -> FPDF_BOOL> {
    let index = WASM_NEVER_PAUSE.load(std::sync::atomic::Ordering::Relaxed);
    if index == 0 {
        return Err(RenderError::Render("PDFium (WebAssembly): Pause-Rückruf nicht gesetzt".into()));
    }
    // SAFETY: Unter WebAssembly ist ein Funktionszeiger ein Index in eine Funktionstabelle; aufgerufen wird er nur von
    // PDFium (über dessen Tabelle, in der die Seite die Funktion `(i32) -> i32` angelegt hat), nie aus diesem Modul.
    Ok(unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(*mut IFSDK_PAUSE) -> FPDF_BOOL>(index) })
}

fn argb(rgba: [u8; 4]) -> FPDF_DWORD {
    let v = (u32::from(rgba[3]) << 24) | (u32::from(rgba[0]) << 16) | (u32::from(rgba[1]) << 8) | u32::from(rgba[2]);
    FPDF_DWORD::from(v)
}

/// Rechteck im Benutzerraum (pt), `left < right`, `bottom < top`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UserRect {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

/// Abbildung Benutzerraum → Pixel des Forced-Color-Rasters: die Box `rect` (Seitenausschnitt,
/// der genau das Raster füllt), `/Rotate` in Vierteldrehungen, `scale` Pixel je pt. Gleiche
/// Formeln wie PDFium `CPDF_Page::UpdateDimensions`/`GetDisplayMatrix` (Gerät: y nach unten).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeviceMap {
    pub rect: UserRect,
    pub quarter_turns: i32,
    pub scale: f32,
}

impl DeviceMap {
    /// Punkt im Benutzerraum → Pixelkoordinaten (nicht gerundet).
    pub(crate) fn to_pixel(self, x: f32, y: f32) -> (f32, f32) {
        let (r, s) = (self.rect, self.scale);
        match self.quarter_turns {
            1 => ((y - r.bottom) * s, (x - r.left) * s),
            2 => ((r.right - x) * s, (y - r.bottom) * s),
            3 => ((r.top - y) * s, (r.right - x) * s),
            _ => ((x - r.left) * s, (r.top - y) * s),
        }
    }
}

/// Ausschnitt der sichtbaren Größe `visible_w × visible_h` (pt) an der **sichtbaren linken oberen
/// Ecke** der Seitenbox `bbox` – wie Java2D, das die Seite 1:1 auf ein `(int) w × (int) h`-Bild
/// zeichnet und den Rest rechts und unten abschneidet (PDF-AS, KR-F10). Je Drehung liegt die
/// sichtbare Ecke oben links an einer anderen Ecke der ungedrehten Box (PDFium `m_PageMatrix`).
pub(crate) fn top_left_crop(bbox: UserRect, quarter_turns: i32, visible_w: f32, visible_h: f32) -> UserRect {
    let UserRect { left: l, bottom: b, right: r, top: t } = bbox;
    match quarter_turns {
        // sichtbar oben links = (l, b); sichtbare x-Achse = +y, sichtbare y-Achse (unten) = +x
        1 => UserRect { left: l, bottom: b, right: l + visible_h, top: b + visible_w },
        // (r, b); x = −x, y = +y
        2 => UserRect { left: r - visible_w, bottom: b, right: r, top: b + visible_h },
        // (r, t); x = −y, y = −x
        3 => UserRect { left: r - visible_h, bottom: t - visible_w, right: r, top: t },
        // (l, t)
        _ => UserRect { left: l, bottom: t - visible_h, right: l + visible_w, top: t },
    }
}

/// Setzt die CropBox der Seite (zweite Instanz) beim Drop auf den ursprünglichen Stand zurück.
struct CropGuard {
    b: &'static dyn PdfiumLibraryBindings,
    page: FPDF_PAGE,
    original: UserRect,
}

impl Drop for CropGuard {
    fn drop(&mut self) {
        let o = self.original;
        // SAFETY: die Seite ist noch geladen (der Wächter wird vor dem `PageGuard` freigegeben).
        unsafe { self.b.FPDFPage_SetCropBox(self.page, o.left, o.bottom, o.right, o.top) };
    }
}

/// Zweite Dokumentinstanz nur für das Forced-Color-Raster.
pub(crate) struct RawDocument {
    bindings: &'static dyn PdfiumLibraryBindings,
    doc: FPDF_DOCUMENT,
    form: FPDF_FORMHANDLE,
    /// Muss so lange leben wie `form` (PDFium hält den Zeiger).
    _form_info: Box<FPDF_FORMFILLINFO>,
    /// Muss so lange leben wie `doc` (PDFium liest bei Bedarf nach).
    _data: Arc<[u8]>,
}

// SAFETY: Die Handles werden ausschließlich unter der Operationssperre des Crates benutzt und
// nie gleichzeitig aus mehreren Threads; PDFium hat keine Thread-Affinität, sondern verlangt nur
// Serialisierung.
unsafe impl Send for RawDocument {}

impl std::fmt::Debug for RawDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawDocument").field("len", &self._data.len()).finish_non_exhaustive()
    }
}

impl RawDocument {
    /// Öffnet das Dokument (Passwort nur im Speicher, nicht gespeichert).
    pub(crate) fn open(
        bindings: &'static dyn PdfiumLibraryBindings,
        data: Arc<[u8]>,
        password: Option<&str>,
    ) -> Result<Self> {
        // `pdfium-render` bricht bei NUL im Passwort ab (Panik mit dem Passwort in der Meldung).
        check_password(password)?;
        // SAFETY: `data` lebt im zurückgegebenen Objekt weiter (Invariante oben).
        let doc = unsafe { bindings.FPDF_LoadMemDocument64(&data[..], password) };
        if doc.is_null() {
            // SAFETY: reiner Statusabruf.
            #[allow(clippy::unnecessary_cast, reason = "c_ulong ist unter Windows u32, sonst u64")]
            let err = u64::from(unsafe { bindings.FPDF_GetLastError() } as u32);
            return Err(match err {
                FPDF_ERR_PASSWORD => RenderError::PasswordRequired,
                FPDF_ERR_SECURITY => RenderError::Load("Sicherheits-Handler nicht unterstützt".into()),
                code => RenderError::Load(format!("PDFium-Fehler {code}")),
            });
        }
        // SAFETY: Alle Felder von FPDF_FORMFILLINFO sind Ganzzahlen, Zeiger oder
        // `Option<extern fn>`; der Nullwert ist für alle gültig (keine Callbacks).
        let mut form_info: Box<FPDF_FORMFILLINFO> = Box::new(unsafe { std::mem::zeroed() });
        form_info.version = 2;
        // SAFETY: `doc` ist gültig, `form_info` lebt so lange wie das Handle.
        let form = unsafe { bindings.FPDFDOC_InitFormFillEnvironment(doc, form_info.as_mut()) };
        Ok(Self { bindings, doc, form, _form_info: form_info, _data: data })
    }

    /// Rendert eine Seite mit Farbschema. Liefert das Raster und die Abbildung Benutzerraum →
    /// Pixel (für das Übermalen der Bildflächen).
    ///
    /// Abbildung wie Java2D bei PDF-AS (KR-F10, Welle 3): Raster `(int)(w · s) × (int)(h · s)`
    /// (`ForcedColorOptions::raster_size`), die Seite **ohne Streckung** im Maßstab `s = dpi/72`
    /// an der sichtbaren linken oberen Ecke, der Rest rechts und unten fällt weg. Früher wurde die
    /// ganze Seite auf eine gerundete Fläche `round(w) × round(h)` abgebildet; bei Seitenhöhen mit
    /// Nachkommaanteil ≥ 0,5 (A4 841,92 pt) lag eine Unterkante auf einer Pixelgrenze dann knapp
    /// darunter, und die Auto-Position wurde 1 pt tiefer als bei PDF-AS.
    /// `FPDF_RenderPageBitmapWithColorScheme_Start` kennt nur ganzzahlige Flächen; deshalb wird
    /// die CropBox in dieser Instanz für die Dauer des Aufrufs auf den gerasterten Ausschnitt
    /// gesetzt (Fläche = Raster, Maßstab exakt `s`).
    pub(crate) fn render_forced_color(&self, page: usize, opts: &ForcedColorOptions) -> Result<(RgbaImage, DeviceMap)> {
        let index = c_int::try_from(page).map_err(|_| RenderError::Render("Seitenindex zu groß".into()))?;
        let b = self.bindings;
        // SAFETY: `doc` ist gültig; die Seite wird vom Wächter geschlossen.
        let handle = unsafe { b.FPDF_LoadPage(self.doc, index) };
        if handle.is_null() {
            return Err(RenderError::Render("Seite nicht ladbar".into()));
        }
        let page_guard = PageGuard { b, page: handle, form: self.form };
        if !self.form.is_null() {
            // SAFETY: Seite und Formular gehören zum selben Dokument.
            unsafe { b.FORM_OnAfterLoadPage(handle, self.form) };
        }
        // SAFETY: gültige Seite.
        let size = unsafe { PageSize { width: b.FPDF_GetPageWidthF(handle), height: b.FPDF_GetPageHeightF(handle) } };
        if !(size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0) {
            return Err(RenderError::Render("ungültige Seitengröße".into()));
        }
        let (bw, bh) = opts.raster_size(size);
        check_pixels(bw, bh)?;
        let scale = opts.dpi / 72.0;
        // Raster ≤ MAX_PIXELS, also passen beide Seiten in c_int
        let (area_w, area_h) = (bw as c_int, bh as c_int);
        let mut bbox = FS_RECTF { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };
        // SAFETY: gültige Seite, `bbox` ist ein gültiger Ausgabepuffer.
        if unsafe { b.FPDF_GetPageBoundingBox(handle, &mut bbox) } == 0 {
            return Err(RenderError::Render("Seitenbox nicht lesbar".into()));
        }
        let original = UserRect {
            left: bbox.left.min(bbox.right),
            bottom: bbox.bottom.min(bbox.top),
            right: bbox.left.max(bbox.right),
            top: bbox.bottom.max(bbox.top),
        };
        // SAFETY: gültige Seite.
        let quarter_turns = unsafe { b.FPDFPage_GetRotation(handle) }.rem_euclid(4);
        let crop = top_left_crop(original, quarter_turns, bw as f32 / scale, bh as f32 / scale);
        // SAFETY: gültige Seite; der Wächter stellt die Box vor dem Schließen der Seite wieder her.
        unsafe { b.FPDFPage_SetCropBox(handle, crop.left, crop.bottom, crop.right, crop.top) };
        let crop_guard = CropGuard { b, page: handle, original };
        let map = DeviceMap { rect: crop, quarter_turns, scale };

        // SAFETY: Größen sind geprüft; das Bitmap wird vom Wächter freigegeben.
        let bitmap = unsafe { b.FPDFBitmap_Create(bw as c_int, bh as c_int, 1) };
        if bitmap.is_null() {
            return Err(RenderError::Render("Bitmap nicht anlegbar".into()));
        }
        let bitmap_guard = BitmapGuard { b, bitmap };
        // SAFETY: gültiges Bitmap, Rechteck innerhalb.
        unsafe { b.FPDFBitmap_FillRect(bitmap, 0, 0, bw as c_int, bh as c_int, argb(opts.background)) };

        let fg = argb(opts.foreground);
        let scheme =
            FPDF_COLORSCHEME { path_fill_color: fg, path_stroke_color: fg, text_fill_color: fg, text_stroke_color: fg };
        let mut pause = IFSDK_PAUSE { version: 1, NeedToPauseNow: Some(never_pause_fn()?), user: std::ptr::null_mut() };
        // SAFETY: gültige Handles; `scheme` und `pause` leben bis nach dem letzten Aufruf;
        // `never_pause` pausiert nie, PDFium rendert daher in einem Durchgang.
        let mut status = unsafe {
            b.FPDF_RenderPageBitmapWithColorScheme_Start(
                bitmap, handle, 0, 0, area_w, area_h, 0, FPDF_ANNOT, &scheme, &mut pause,
            )
        };
        let mut rounds = 0;
        while status == FPDF_RENDER_TOBECONTINUED && rounds < 10_000 {
            // SAFETY: laufender Render-Kontext dieser Seite.
            status = unsafe { b.FPDF_RenderPage_Continue(handle, &mut pause) };
            rounds += 1;
        }
        // SAFETY: schließt den Render-Kontext der Seite (auch nach Fehlern zulässig).
        unsafe { b.FPDF_RenderPage_Close(handle) };
        if status == FPDF_RENDER_FAILED {
            return Err(RenderError::Render("Forced-Color-Rendering fehlgeschlagen".into()));
        }
        if !self.form.is_null() {
            // Widgets (Formularfelder, Signaturfelder) zeichnet nur FPDF_FFLDraw – in
            // Originalfarben, die Farbersetzung gilt dort nicht (ADR 0060, Register KR-F06).
            // SAFETY: gültige Handles desselben Dokuments.
            unsafe { b.FPDF_FFLDraw(self.form, bitmap, handle, 0, 0, area_w, area_h, 0, FPDF_ANNOT) };
        }

        // SAFETY: gültiges Bitmap; Puffer wird vor der Freigabe kopiert.
        let stride = unsafe { b.FPDFBitmap_GetStride(bitmap) };
        let stride = usize::try_from(stride).map_err(|_| RenderError::Render("Bitmap-Stride ungültig".into()))?;
        #[cfg(not(target_arch = "wasm32"))]
        let buffer = unsafe { b.FPDFBitmap_GetBuffer_as_slice(bitmap) };
        // Browser: PDFium liegt in einem eigenen WebAssembly-Speicher – der Puffer kommt als Kopie
        #[cfg(target_arch = "wasm32")]
        let buffer_copy = unsafe { b.FPDFBitmap_GetBuffer_as_vec(bitmap) };
        #[cfg(target_arch = "wasm32")]
        let buffer = buffer_copy.as_slice();
        let (w, h) = (bw as usize, bh as usize);
        if stride < w * 4 || buffer.len() < stride * h {
            return Err(RenderError::Render("Bitmap-Puffer zu klein".into()));
        }
        let mut data = Vec::with_capacity(w * h * 4);
        for row in buffer.chunks_exact(stride).take(h) {
            for px in row[..w * 4].as_chunks::<4>().0 {
                data.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        }
        drop(bitmap_guard);
        drop(crop_guard);
        drop(page_guard);
        let img = RgbaImage::from_raw(bw, bh, data).ok_or_else(|| RenderError::Render("Bitmap-Größe".into()))?;
        Ok((img, map))
    }
}

impl RawDocument {
    /// Bildobjekte der Seite **mit angewandter Maske** (`/SMask`, `/Mask`) in derselben
    /// Reihenfolge wie `pdfium::visit_page_images` (Seitenobjekte, Form-XObjects in Tiefensuche;
    /// Objekte ohne lesbare Matrix zählen nicht). Für Index `i` mit `wanted[i]` liefert die
    /// Funktion das Bild in Originalauflösung als RGBA mit Alpha aus der Maske – nur wenn es
    /// tatsächlich transparente Pixel hat (sonst `None`, das unmaskierte Bild gilt weiter).
    ///
    /// Wie PDF-AS (`PDImageXObject.getImage()` wendet die Maske an, KR-F07): Dazu wird die
    /// Matrix des Objekts in dieser zweiten Instanz kurz auf `[w 0 0 h 0 0]` gesetzt (Bild-
    /// pixel = Gerätepixel, keine Drehung/Verschiebung), `FPDFImageObj_GetRenderedBitmap`
    /// aufgerufen und die Matrix wiederhergestellt. Das Original-PDF bleibt unverändert.
    pub(crate) fn masked_images(
        &self,
        page: usize,
        wanted: &[bool],
        max_pixels: u64,
    ) -> Result<Vec<Option<RgbaImage>>> {
        let index = c_int::try_from(page).map_err(|_| RenderError::Render("Seitenindex zu groß".into()))?;
        let b = self.bindings;
        // SAFETY: `doc` ist gültig; die Seite wird vom Wächter geschlossen.
        let handle = unsafe { b.FPDF_LoadPage(self.doc, index) };
        if handle.is_null() {
            return Err(RenderError::Render("Seite nicht ladbar".into()));
        }
        let _page_guard = PageGuard { b, page: handle, form: std::ptr::null_mut() };
        let mut out = Vec::new();
        // SAFETY: gültige Seite.
        let count = unsafe { b.FPDFPage_CountObjects(handle) };
        for i in 0..count.max(0) {
            // SAFETY: Index im Bereich; das Objekt gehört der Seite (nicht freigeben).
            let object = unsafe { b.FPDFPage_GetObject(handle, i) };
            self.visit_masked(handle, object, 0, wanted, max_pixels, &mut out);
        }
        Ok(out)
    }

    fn visit_masked(
        &self,
        page: FPDF_PAGE,
        object: FPDF_PAGEOBJECT,
        depth: usize,
        wanted: &[bool],
        max_pixels: u64,
        out: &mut Vec<Option<RgbaImage>>,
    ) {
        if object.is_null() {
            return;
        }
        let b = self.bindings;
        let mut m = FS_MATRIX { a: 0.0, b: 0.0, c: 0.0, d: 0.0, e: 0.0, f: 0.0 };
        // SAFETY: gültiges Objekt, `m` ist ein gültiger Ausgabepuffer.
        if unsafe { b.FPDFPageObj_GetMatrix(object, &mut m) } == 0 {
            return; // wie `visit_images`: ohne Matrix kein Eintrag
        }
        // SAFETY: gültiges Objekt.
        let kind = unsafe { b.FPDFPageObj_GetType(object) };
        if kind == FPDF_PAGEOBJ_IMAGE as c_int {
            let i = out.len();
            let img = if wanted.get(i).copied().unwrap_or(false) {
                self.render_masked(page, object, m, max_pixels)
            } else {
                None
            };
            out.push(img);
        } else if kind == FPDF_PAGEOBJ_FORM as c_int && depth < MAX_FORM_DEPTH {
            // SAFETY: gültiges Form-Objekt.
            let n = unsafe { b.FPDFFormObj_CountObjects(object) };
            for j in 0..n.max(0) {
                // SAFETY: Index im Bereich; das Kind gehört dem Form-Objekt.
                let child = unsafe { b.FPDFFormObj_GetObject(object, j as _) };
                self.visit_masked(page, child, depth + 1, wanted, max_pixels, out);
            }
        }
    }

    fn render_masked(
        &self,
        page: FPDF_PAGE,
        object: FPDF_PAGEOBJECT,
        original: FS_MATRIX,
        max_pixels: u64,
    ) -> Option<RgbaImage> {
        let b = self.bindings;
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: gültiges Bildobjekt, gültige Ausgabepuffer.
        if unsafe { b.FPDFImageObj_GetImagePixelSize(object, &mut w, &mut h) } == 0
            || w == 0
            || h == 0
            || u64::from(w) * u64::from(h) > max_pixels
        {
            return None;
        }
        let unit = FS_MATRIX { a: w as f32, b: 0.0, c: 0.0, d: h as f32, e: 0.0, f: 0.0 };
        // SAFETY: gültiges Objekt; die ursprüngliche Matrix wird direkt danach zurückgesetzt.
        if unsafe { b.FPDFPageObj_SetMatrix(object, &unit) } == 0 {
            return None;
        }
        // SAFETY: Dokument, Seite und Objekt gehören zusammen; das Bitmap wird vom Wächter freigegeben.
        let bitmap = unsafe { b.FPDFImageObj_GetRenderedBitmap(self.doc, page, object) };
        // SAFETY: wie oben.
        unsafe { b.FPDFPageObj_SetMatrix(object, &original) };
        if bitmap.is_null() {
            return None;
        }
        let _bitmap_guard = BitmapGuard { b, bitmap };
        // SAFETY: gültiges Bitmap.
        let (bw, bh, format, stride) = unsafe {
            (
                b.FPDFBitmap_GetWidth(bitmap),
                b.FPDFBitmap_GetHeight(bitmap),
                b.FPDFBitmap_GetFormat(bitmap),
                b.FPDFBitmap_GetStride(bitmap),
            )
        };
        if format != FPDF_BITMAP_BGRA || bw != w as c_int || bh != h as c_int {
            return None;
        }
        let stride = usize::try_from(stride).ok()?;
        // SAFETY: gültiges Bitmap; der Puffer wird vor der Freigabe kopiert.
        #[cfg(not(target_arch = "wasm32"))]
        let buffer = unsafe { b.FPDFBitmap_GetBuffer_as_slice(bitmap) };
        // Browser: PDFium liegt in einem eigenen WebAssembly-Speicher – der Puffer kommt als Kopie
        #[cfg(target_arch = "wasm32")]
        let buffer_copy = unsafe { b.FPDFBitmap_GetBuffer_as_vec(bitmap) };
        #[cfg(target_arch = "wasm32")]
        let buffer = buffer_copy.as_slice();
        let (wu, hu) = (w as usize, h as usize);
        if stride < wu * 4 || buffer.len() < stride * hu {
            return None;
        }
        let mut data = Vec::with_capacity(wu * hu * 4);
        let mut transparent = false;
        for row in buffer.chunks_exact(stride).take(hu) {
            for px in row[..wu * 4].as_chunks::<4>().0 {
                transparent |= px[3] < 255;
                data.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        }
        if !transparent {
            return None;
        }
        RgbaImage::from_raw(w, h, data)
    }
}

impl Drop for RawDocument {
    fn drop(&mut self) {
        // SAFETY: Handles gehören diesem Objekt und werden genau einmal freigegeben; das
        // Formular vor dem Dokument (PDFium-Vorgabe).
        unsafe {
            if !self.form.is_null() {
                self.bindings.FPDFDOC_ExitFormFillEnvironment(self.form);
            }
            self.bindings.FPDF_CloseDocument(self.doc);
        }
    }
}

struct PageGuard {
    b: &'static dyn PdfiumLibraryBindings,
    page: FPDF_PAGE,
    form: FPDF_FORMHANDLE,
}

impl Drop for PageGuard {
    fn drop(&mut self) {
        // SAFETY: Seite wurde in diesem Aufruf geladen und wird genau einmal geschlossen.
        unsafe {
            if !self.form.is_null() {
                self.b.FORM_OnBeforeClosePage(self.page, self.form);
            }
            self.b.FPDF_ClosePage(self.page);
        }
    }
}

struct BitmapGuard {
    b: &'static dyn PdfiumLibraryBindings,
    bitmap: FPDF_BITMAP,
}

impl Drop for BitmapGuard {
    fn drop(&mut self) {
        // SAFETY: Bitmap wurde in diesem Aufruf angelegt und wird genau einmal freigegeben.
        unsafe { self.b.FPDFBitmap_Destroy(self.bitmap) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_packing() {
        assert_eq!(argb([152, 254, 52, 255]), 0xFF98FE34);
        assert_eq!(argb([234, 14, 184, 211]), 0xD3EA0EB8);
    }

    /// Der Ausschnitt behält die sichtbare linke obere Ecke; die Abbildung legt sie auf Pixel
    /// (0, 0) und die gegenüberliegende Ecke genau auf (Rasterbreite, Rasterhöhe).
    #[test]
    fn top_left_crop_and_device_map_per_rotation() {
        let bbox = UserRect { left: 10.0, bottom: 20.0, right: 605.32, top: 861.92 }; // 595,32 × 841,92
        for turns in 0..4 {
            let (vw, vh): (f32, f32) = if turns % 2 == 0 { (595.32, 841.92) } else { (841.92, 595.32) };
            let (rw, rh) = (vw.floor(), vh.floor());
            let crop = top_left_crop(bbox, turns, rw, rh);
            let map = DeviceMap { rect: crop, quarter_turns: turns, scale: 1.0 };
            // Ausschnitt liegt in der Box, Maße = Raster (ungedreht getauscht)
            assert!(crop.left >= bbox.left && crop.right <= bbox.right + 1e-3, "{turns}");
            assert!(crop.bottom >= bbox.bottom && crop.top <= bbox.top + 1e-3, "{turns}");
            let (cw, ch) = (crop.right - crop.left, crop.top - crop.bottom);
            let (ew, eh) = if turns % 2 == 0 { (rw, rh) } else { (rh, rw) };
            assert!((cw - ew).abs() < 1e-3 && (ch - eh).abs() < 1e-3, "{turns}: {cw}×{ch}");
            // die vier Ecken des Ausschnitts landen auf den vier Ecken des Rasters
            let mut px: Vec<(i32, i32)> =
                [(crop.left, crop.bottom), (crop.right, crop.bottom), (crop.left, crop.top), (crop.right, crop.top)]
                    .iter()
                    .map(|&(x, y)| {
                        let (u, v) = map.to_pixel(x, y);
                        (u.round() as i32, v.round() as i32)
                    })
                    .collect();
            px.sort();
            assert_eq!(px, [(0, 0), (0, rh as i32), (rw as i32, 0), (rw as i32, rh as i32)], "{turns}");
        }
        // ohne Drehung: Ursprung oben links = (left, top), 1 pt = 1 px
        let map = DeviceMap { rect: top_left_crop(bbox, 0, 595.0, 841.0), quarter_turns: 0, scale: 1.0 };
        assert_eq!(map.to_pixel(10.0, 861.92), (0.0, 0.0));
        assert_eq!(map.to_pixel(110.0, 761.92), (100.0, 100.0));
        // 90°: sichtbar oben links = (left, bottom) der ungedrehten Box
        let map = DeviceMap { rect: top_left_crop(bbox, 1, 841.0, 595.0), quarter_turns: 1, scale: 2.0 };
        assert_eq!(map.to_pixel(10.0, 20.0), (0.0, 0.0));
        assert_eq!(map.to_pixel(11.0, 20.0), (0.0, 2.0));
    }
}
