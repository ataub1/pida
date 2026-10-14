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
#![allow(unsafe_code)]

use std::os::raw::c_int;
use std::sync::Arc;

use pdfium_render::prelude::{
    FPDF_BITMAP, FPDF_BOOL, FPDF_COLORSCHEME, FPDF_DOCUMENT, FPDF_DWORD, FPDF_FORMFILLINFO, FPDF_FORMHANDLE, FPDF_PAGE,
    IFSDK_PAUSE, PdfiumLibraryBindings,
};

use crate::pdfium::check_pixels;
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

/// `IFSDK_PAUSE::NeedToPauseNow`: nie pausieren (PDFium verlangt ein Pause-Objekt mit
/// `version == 1`, sonst liefert der progressive Start sofort `FPDF_RENDER_FAILED`).
unsafe extern "C" fn never_pause(_this: *mut IFSDK_PAUSE) -> FPDF_BOOL {
    0
}

fn argb(rgba: [u8; 4]) -> FPDF_DWORD {
    let v = (u32::from(rgba[3]) << 24) | (u32::from(rgba[0]) << 16) | (u32::from(rgba[1]) << 8) | u32::from(rgba[2]);
    FPDF_DWORD::from(v)
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
        // SAFETY: `data` lebt im zurückgegebenen Objekt weiter (Invariante oben).
        let doc = unsafe { bindings.FPDF_LoadMemDocument64(&data[..], password) };
        if doc.is_null() {
            // SAFETY: reiner Statusabruf.
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

    /// Rendert eine Seite mit Farbschema. Liefert das Raster und die Größe des Anzeigebereichs
    /// (für die Umrechnung Seite → Pixel).
    pub(crate) fn render_forced_color(
        &self,
        page: usize,
        opts: &ForcedColorOptions,
    ) -> Result<(RgbaImage, (i32, i32))> {
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
        let area_w = (size.width * scale).round().max(1.0);
        let area_h = (size.height * scale).round().max(1.0);
        if area_w > i32::MAX as f32 || area_h > i32::MAX as f32 {
            return Err(RenderError::Render("Raster zu groß".into()));
        }
        let (area_w, area_h) = (area_w as c_int, area_h as c_int);

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
        let mut pause = IFSDK_PAUSE { version: 1, NeedToPauseNow: Some(never_pause), user: std::ptr::null_mut() };
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
        let buffer = unsafe { b.FPDFBitmap_GetBuffer_as_slice(bitmap) };
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
        drop(page_guard);
        let img = RgbaImage::from_raw(bw, bh, data).ok_or_else(|| RenderError::Render("Bitmap-Größe".into()))?;
        Ok((img, (area_w, area_h)))
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
}
