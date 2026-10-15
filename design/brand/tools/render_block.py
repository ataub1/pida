"""Rendert den sichtbaren Signaturblock einer signierten PDF-Datei mit PDFium (pypdfium2) als PNG.

Aufruf: python -I render_block.py <signiert.pdf> <block.png> [--scale 4] [--margin 6]

Gesucht wird das Widget der letzten Signatur (Feldtyp /Sig, letzte Seite mit Widget); gerendert wird
die Seite mit Annotationen, beschnitten auf das Widget-Rechteck plus Rand (in pt). Teil des
Vorschaubogens design/brand/preview.png (ADR 0005, render-block.sh).
"""

import argparse
import ctypes
import sys

import pypdfium2 as pdfium
import pypdfium2.raw as raw

FPDF_ANNOT_WIDGET = 20


def widget_rects(page):
    """Rechtecke (left, bottom, right, top) aller Widget-Annotationen der Seite."""
    rects = []
    for i in range(raw.FPDFPage_GetAnnotCount(page.raw)):
        annot = raw.FPDFPage_GetAnnot(page.raw, i)
        try:
            if raw.FPDFAnnot_GetSubtype(annot) != FPDF_ANNOT_WIDGET:
                continue
            r = raw.FS_RECTF()
            if raw.FPDFAnnot_GetRect(annot, ctypes.byref(r)):
                left, right = sorted((r.left, r.right))
                bottom, top = sorted((r.bottom, r.top))
                if right - left > 1 and top - bottom > 1:
                    rects.append((left, bottom, right, top))
        finally:
            raw.FPDFPage_CloseAnnot(annot)
    return rects


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pdf")
    ap.add_argument("png")
    ap.add_argument("--scale", type=float, default=4.0)
    ap.add_argument("--margin", type=float, default=6.0)
    a = ap.parse_args()

    doc = pdfium.PdfDocument(a.pdf)
    # Signaturfelder sind Widgets: PDFium zeichnet sie nur mit Formularumgebung
    doc.init_forms()
    found = None
    for index in range(len(doc) - 1, -1, -1):
        page = doc[index]
        rects = widget_rects(page)
        if rects:
            found = (page, rects[-1])
            break
    if found is None:
        sys.exit("kein sichtbares Signatur-Widget gefunden")
    page, (left, bottom, right, top) = found
    width, height = page.get_size()
    m = a.margin
    crop = (max(left - m, 0), max(bottom - m, 0), max(width - right - m, 0), max(height - top - m, 0))
    bitmap = page.render(scale=a.scale, crop=crop, draw_annots=True, may_draw_forms=True)
    save_png(bitmap, a.png)
    print(f"{a.png}: Widget {right - left:.2f} × {top - bottom:.2f} pt, Maßstab {a.scale}")


def save_png(bitmap, path):
    """PNG ohne Pillow (zlib + struct): BGR(A) aus PDFium nach RGB."""
    import struct
    import zlib

    w, h = bitmap.width, bitmap.height
    n = bitmap.n_channels
    stride = bitmap.stride
    buf = bytes(bitmap.buffer)
    order = bitmap.mode  # "BGR", "BGRA", "BGRX" …
    rows = bytearray()
    for y in range(h):
        rows.append(0)
        line = buf[y * stride : y * stride + w * n]
        for x in range(w):
            px = line[x * n : x * n + n]
            if order.startswith("BGR"):
                rows += bytes((px[2], px[1], px[0]))
            else:
                rows += bytes(px[:3])

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(rows), 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main()
