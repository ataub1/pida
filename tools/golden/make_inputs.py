#!/usr/bin/env python3
"""Erzeugt die Eingabedokumente und Bildmarken für den PDF-AS-4.4.2-Golden-Lauf (Plan §8.2).

Deterministisch (keine Zeitstempel, feste /ID): zweimaliges Ausführen ergibt byte-gleiche Dateien.
Seiteninhalt besteht aus Rechtecken mit ganzzahligen Koordinaten (keine Schrift), damit der
Rasterscan der Auto-Positionierung (PDF-AS: Java2D, hier: PDFium) nicht von der Glyphenrasterung
abhängt (KR-F10).

Aufruf: make_inputs.py <ausgabeordner> <qr-matrix-datei> <qr-matrix-datei-id200>
  <qr-matrix-datei>: Ausgabe von MakeQr.java für "PDF-AS-POS;id=1" (Zeilen aus 0/1)
  <qr-matrix-datei-id200>: dasselbe für "PDF-AS-POS;id=200" (Register KR-F03: IDs > 127)

Bildmarken (Pillow): emblem-wide.png, emblem-tall.png, emblem-alpha.png, emblem-photo.jpg,
emblem-indexed.png.
"""

import io
import sys
import zlib
from pathlib import Path

from PIL import Image, ImageDraw

A4 = (595.32, 841.92)  # MS-Word-A4 (bewusst nicht ganzzahlig: f32-Arithmetik der Positionierung)
A5_LANDSCAPE = (595.32, 419.53)


def num(v):
    s = f"{v:.4f}".rstrip("0").rstrip(".")
    return s if s not in ("-0", "") else "0"


class Pdf:
    """Minimaler PDF-1.7-Schreiber (klassische xref-Tabelle)."""

    def __init__(self):
        self.objs = []

    def reserve(self):
        self.objs.append(None)
        return len(self.objs)

    def set(self, n, body):
        self.objs[n - 1] = body if isinstance(body, bytes) else body.encode("latin-1")

    def add(self, body):
        n = self.reserve()
        self.set(n, body)
        return n

    def add_stream(self, dict_entries, data, compress=True):
        if compress:
            data = zlib.compress(data, 9)
            dict_entries = dict_entries + " /Filter /FlateDecode"
        head = f"<< {dict_entries} /Length {len(data)} >>\nstream\n".encode("latin-1")
        return self.add(head + data + b"\nendstream")

    def finish(self, root):
        out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
        offsets = []
        for i, body in enumerate(self.objs, start=1):
            assert body is not None, f"Objekt {i} nicht gesetzt"
            offsets.append(len(out))
            out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
        xref = len(out)
        out += f"xref\n0 {len(self.objs) + 1}\n0000000000 65535 f \n".encode()
        for off in offsets:
            out += f"{off:010d} 00000 n \n".encode()
        fid = "50444F4E47474F4C44454E3434322121"  # fest: „PDONGGOLDEN442!!“
        out += (
            f"trailer\n<< /Size {len(self.objs) + 1} /Root {root} 0 R /ID [<{fid}> <{fid}>] >>\n"
            f"startxref\n{xref}\n%%EOF\n"
        ).encode()
        return bytes(out)


def bars(width, height, top_margin, bottom, step=14, seed=0):
    """„Textzeilen“ als Balken im sichtbaren Raum (Breite×Höhe), von oben bis `bottom`."""
    ops = []
    # Titelbalken
    ops.append(f"0.15 g 72 {num(height - top_margin - 24)} {num(width - 144)} 18 re f")
    y = height - top_margin - 48
    i = seed
    while y >= bottom:
        w = (width - 144) * (0.55 + 0.45 * (((i * 37) % 11) / 10))
        ops.append(f"0.35 g 72 {num(y)} {num(int(w))} 7 re f")
        y -= step
        i += 1
    return "\n".join(ops)


def page_dict(parent, mediabox, content_ref, extra=""):
    mb = " ".join(num(v) for v in mediabox)
    return (
        f"<< /Type /Page /Parent {parent} 0 R /MediaBox [{mb}] /Contents {content_ref} 0 R "
        f"/Resources << /ProcSet [/PDF /ImageB] {extra} >> >>"
    )


def build(pages):
    """pages: Liste von dicts mit mediabox, content, optional cropbox, rotate, xobjects (Name->Objektnr. via fn)."""
    pdf = Pdf()
    catalog = pdf.reserve()
    tree = pdf.reserve()
    kids = []
    for p in pages:
        extra = ""
        if "image" in p:
            img_ref = p["image"](pdf)
            extra = f"/XObject << /Im1 {img_ref} 0 R >>"
        content = pdf.add_stream("", p["content"].encode("latin-1"))
        d = page_dict(tree, p["mediabox"], content, extra)
        if "cropbox" in p:
            d = d[:-2] + f" /CropBox [{' '.join(num(v) for v in p['cropbox'])}] >>"
        if p.get("rotate"):
            d = d[:-2] + f" /Rotate {p['rotate']} >>"
        kids.append(pdf.add(d))
    pdf.set(tree, f"<< /Type /Pages /Kids [{' '.join(f'{k} 0 R' for k in kids)}] /Count {len(kids)} >>")
    pdf.set(catalog, f"<< /Type /Catalog /Pages {tree} 0 R >>")
    return pdf.finish(catalog)


def visible_to_user(rotate, w0, h0):
    """cm-Matrix sichtbarer Raum → Benutzerraum (wie PageMapping::to_user im Placement-Crate)."""
    return {
        0: "1 0 0 1 0 0 cm",
        90: f"0 1 -1 0 {num(w0)} 0 cm",
        180: f"-1 0 0 -1 {num(w0)} {num(h0)} cm",
        270: f"0 -1 1 0 0 {num(h0)} cm",
    }[rotate]


def gray_image(pdf, w, h, pixels):
    return pdf.add_stream(f"/Type /XObject /Subtype /Image /Width {w} /Height {h} "
                          f"/ColorSpace /DeviceGray /BitsPerComponent 8", bytes(pixels))


def gradient_image(pdf):
    w, h = 64, 32
    px = [int(40 + 180 * x / (w - 1)) for _ in range(h) for x in range(w)]
    return gray_image(pdf, w, h, px)


def qr_placeholder_image(matrix):
    """Generatorähnliches Platzhalterbild (300×60 px): QR links, Textbalken rechts (analyse/04 §5.5)."""
    n = len(matrix)
    w, h, border = 300, 60, 4
    module = (h - 2 * border) // n
    px = [255] * (w * h)
    for my, row in enumerate(matrix):
        for mx, bit in enumerate(row):
            if bit == "1":
                for dy in range(module):
                    for dx in range(module):
                        x = border + mx * module + dx
                        y = border + my * module + dy
                        px[y * w + x] = 0
    # „Platzhalter für die / elektronische Signatur“ als Balken
    for (x0, y0, x1, y1) in [(80, 14, 280, 22), (80, 30, 250, 38), (80, 44, 160, 50)]:
        for y in range(y0, y1):
            for x in range(x0, x1):
                px[y * w + x] = 0
    return w, h, px


def write_pdfs(out, qr_matrix, qr200_matrix):
    pw, ph = A4
    docs = {}
    docs["a4"] = [dict(mediabox=[0, 0, pw, ph], content=bars(pw, ph, 30, 560))]
    docs["a4-landscape"] = [dict(mediabox=[0, 0, ph, pw], content=bars(ph, pw, 30, 420, seed=3))]
    for rot in (90, 180, 270):
        vw, vh = (ph, pw) if rot in (90, 270) else (pw, ph)
        content = "q " + visible_to_user(rot, pw, ph) + "\n" + bars(vw, vh, 30, vh - 260, seed=rot) + "\nQ"
        docs[f"a4-rot{rot}"] = [dict(mediabox=[0, 0, pw, ph], rotate=rot, content=content)]
    # CropBox ≠ MediaBox (Ursprung verschoben); Balken außerhalb der CropBox sind unsichtbar
    crop = [40, 60, 40 + pw, 60 + ph]
    content = ("0 g 0 0 700 30 re f\n0 g 0 970 700 30 re f\n"
               "q 1 0 0 1 40 60 cm\n" + bars(pw, ph, 30, 560, seed=5) + "\nQ")
    docs["a4-cropbox"] = [dict(mediabox=[0, 0, 700, 1000], cropbox=crop, content=content)]
    # viel Inhalt: halbe Seite (Auto-Position darunter) mit Bild
    content = bars(pw, ph, 30, 430, step=11, seed=7) + "\nq 64 0 0 32 400 600 cm /Im1 Do Q"
    docs["content-half"] = [dict(mediabox=[0, 0, pw, ph], content=content, image=gradient_image)]
    # bis über die Fußzeile (f:80) – Block passt nicht ⇒ neue Seite
    docs["content-full"] = [dict(mediabox=[0, 0, pw, ph], content=bars(pw, ph, 30, 100, step=11, seed=9))]
    # zwei Seiten, letzte Seite A5 quer (neue Seite übernimmt deren MediaBox)
    a5w, a5h = A5_LANDSCAPE
    docs["two-pages"] = [
        dict(mediabox=[0, 0, pw, ph], content=bars(pw, ph, 30, 560, seed=11)),
        dict(mediabox=[0, 0, a5w, a5h], content=bars(a5w, a5h, 20, a5h - 120, seed=13)),
    ]
    # QR-Platzhalter „PDF-AS-POS;id=1“: 300×60 px auf 225×45 pt bei (300, 150)
    qw, qh, qpx = qr_placeholder_image(qr_matrix)
    content = bars(pw, ph, 30, 560, seed=15) + "\nq 225 0 0 45 300 150 cm /Im1 Do Q"
    docs["qr"] = [dict(mediabox=[0, 0, pw, ph], content=content,
                       image=lambda pdf: gray_image(pdf, qw, qh, qpx))]
    # dasselbe mit id=200: PDF-AS vergleicht Integer-Objekte mit == (Cache bis 127) und findet ihn nicht
    q2w, q2h, q2px = qr_placeholder_image(qr200_matrix)
    docs["qr-id200"] = [dict(mediabox=[0, 0, pw, ph], content=content,
                             image=lambda pdf: gray_image(pdf, q2w, q2h, q2px))]
    # Rasterscan gedrehter Seiten (KR-F04): PDF-AS legt das Rasterbild in ungedrehten Maßen
    # (int) w × (int) h an und löscht mit clearRect(0, 0, (int) w, (int) h) im gedrehten Raum.
    # Querformat-MediaBox mit /Rotate 90/270 (sichtbar hoch): Spalten rechts der gelöschten Fläche
    # bleiben transparent ⇒ jede Zeile gilt als belegt.
    for rot in (90, 270):
        content = "q " + visible_to_user(rot, ph, pw) + "\n" + bars(pw, ph, 30, ph - 260, seed=20 + rot) + "\nQ"
        docs[f"landscape-rot{rot}"] = [dict(mediabox=[0, 0, ph, pw], rotate=rot, content=content)]
    # /Rotate 180 mit Breiten-Nachkommaanteil > 0,5: Spalte 0 wird nicht gelöscht
    ww = 595.8
    content = "q " + visible_to_user(180, ww, ph) + "\n" + bars(ww, ph, 30, ph - 260, seed=31) + "\nQ"
    docs["wide-rot180"] = [dict(mediabox=[0, 0, ww, ph], rotate=180, content=content)]
    # Inhalt nur nahe dem unteren Rand (sichtbar y 100 … 107): PDF-AS beginnt die Suche in der
    # ungedrehten Zeile (int) h − 1 − (int) f, also tiefer als ein Scan im sichtbaren Raum
    low = "0.35 g 72 100 {} 7 re f"
    docs["a4-rot270-low"] = [dict(mediabox=[0, 0, pw, ph], rotate=270,
                                  content="q " + visible_to_user(270, pw, ph) + "\n" + low.format(num(int(ph) - 300))
                                  + "\nQ")]
    docs["rot90-842-low"] = [dict(mediabox=[0, 0, 595, 842], rotate=90,
                                  content="q " + visible_to_user(90, 595, 842) + "\n" + low.format(542) + "\nQ")]
    # /Rotate 90 mit Höhen-Nachkommaanteil genau 0,5 (Pixelmitte auf der Kante des gelöschten
    # Rechtecks), leere Seite
    docs["half-rot90"] = [dict(mediabox=[0, 0, pw, 841.5], rotate=90, content="")]
    for name, pages in docs.items():
        (out / f"{name}.pdf").write_bytes(build(pages))
    return sorted(docs)


def write_emblems(out):
    # breit, RGB
    im = Image.new("RGB", (400, 120), (255, 255, 255))
    d = ImageDraw.Draw(im)
    d.rectangle([10, 10, 389, 109], outline=(200, 20, 40), width=6)
    d.ellipse([30, 25, 100, 95], fill=(20, 90, 200))
    for i in range(6):
        d.rectangle([130 + i * 42, 40, 160 + i * 42, 80], fill=(30 + i * 30, 120, 60))
    im.save(out / "emblem-wide.png", optimize=False)
    # hoch, Graustufen
    im = Image.new("L", (120, 360), 255)
    d = ImageDraw.Draw(im)
    for i in range(9):
        d.rectangle([10, 10 + i * 38, 109, 38 + i * 38], fill=20 + i * 25)
    im.save(out / "emblem-tall.png", optimize=False)
    # RGBA mit Transparenz
    im = Image.new("RGBA", (240, 240), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.ellipse([10, 10, 229, 229], fill=(220, 40, 40, 255))
    d.rectangle([80, 80, 159, 159], fill=(255, 255, 255, 128))
    im.save(out / "emblem-alpha.png", optimize=False)
    # JPEG (Farbverlauf)
    im = Image.new("RGB", (320, 240))
    im.putdata([(x * 255 // 319, y * 255 // 239, 128) for y in range(240) for x in range(320)])
    buf = io.BytesIO()
    im.save(buf, "JPEG", quality=90, subsampling=0)
    (out / "emblem-photo.jpg").write_bytes(buf.getvalue())
    # indiziert mit transparenter Farbe (tRNS)
    im = Image.new("P", (200, 100), 0)
    im.putpalette([255, 255, 255, 0, 120, 60, 200, 30, 30] + [0] * (256 * 3 - 9))
    d = ImageDraw.Draw(im)
    d.rectangle([20, 20, 179, 79], fill=1)
    d.ellipse([70, 30, 129, 69], fill=2)
    im.save(out / "emblem-indexed.png", transparency=0, optimize=False)
    return ["emblem-alpha.png", "emblem-indexed.png", "emblem-photo.jpg", "emblem-tall.png", "emblem-wide.png"]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    matrices = []
    for arg in sys.argv[2:4]:
        matrix = [line.strip() for line in Path(arg).read_text().splitlines() if line.strip()]
        assert matrix and all(set(r) <= {"0", "1"} and len(r) == len(matrix) for r in matrix), "QR-Matrix ungültig"
        matrices.append(matrix)
    names = write_pdfs(out, *matrices)
    emblems = write_emblems(out)
    print("Eingaben:", ", ".join(names))
    print("Bildmarken:", ", ".join(emblems))


if __name__ == "__main__":
    main()
