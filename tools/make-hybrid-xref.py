#!/usr/bin/env python3
"""PDF-Over NG – erzeugt ein minimales PDF mit Hybrid-Querverweis (ISO 32000-1 §7.5.8.4).

Klassische xref-Tabelle + Trailer mit /XRefStm; das Font-Objekt 5 liegt in einem Object Stream
(Objekt 6) und ist **nur** über den xref-Stream (Objekt 7) auffindbar, die Tabelle führt es nicht.
Ein Leser, der /XRefStm ignoriert, findet die Schrift nicht. Referenz für
`pdfover_pdf_cos::XrefKind::Hybrid` und den Hybrid-Writer (Plan §6.1 K1, §6.2).

Aufruf:  python3 -I tools/make-hybrid-xref.py <ausgabe.pdf>
Deterministisch, nur Standardbibliothek, kein Netz.
"""

import sys
import zlib


def main(out_path: str) -> None:
    content = b"BT /F1 24 Tf 72 720 Td (Hybrid xref - NUR TEST) Tj ET\n"
    font = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
    # Object Stream mit genau einem Objekt (Nr. 5): Kopf "5 0 ", dann der Objektkörper.
    objstm_header = b"5 0 "
    objstm_data = objstm_header + font
    objstm_comp = zlib.compress(objstm_data, 9)

    buf = bytearray()
    offsets = {}

    def emit(num: int, body: bytes) -> None:
        offsets[num] = len(buf)
        buf.extend(b"%d 0 obj\n" % num)
        buf.extend(body)
        buf.extend(b"\nendobj\n")

    buf.extend(b"%PDF-1.5\n%\xe2\xe3\xcf\xd3\n")
    emit(1, b"<< /Type /Catalog /Pages 2 0 R >>")
    emit(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    emit(
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R "
        b"/Resources << /Font << /F1 5 0 R >> >> >>",
    )
    emit(4, b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream")
    emit(
        6,
        b"<< /Type /ObjStm /N 1 /First %d /Filter /FlateDecode /Length %d >>\nstream\n"
        % (len(objstm_header), len(objstm_comp))
        + objstm_comp
        + b"\nendstream",
    )
    # xref-Stream nur mit dem komprimierten Objekt 5 (Typ 2: Object Stream 6, Index 0).
    xref_rows = bytes([2, 0, 6, 0])  # W [1 2 1]
    xref_comp = zlib.compress(xref_rows, 9)
    emit(
        7,
        b"<< /Type /XRef /Size 8 /W [1 2 1] /Index [5 1] /Filter /FlateDecode /Length %d >>\nstream\n"
        % len(xref_comp)
        + xref_comp
        + b"\nendstream",
    )

    xref_off = len(buf)
    # Objekt 5 fehlt in der Tabelle (Unterabschnitte 0–4 und 6–7) und steht nur im xref-Stream –
    # so schreiben es Hybrid-Writer (Word, Acrobat); ein als frei markierter Eintrag würde den
    # Stream-Eintrag bei Lesern wie qpdf verdecken.
    buf.extend(b"xref\n0 5\n")
    buf.extend(b"0000000000 65535 f\r\n")
    for num in range(1, 5):
        buf.extend(b"%010d 00000 n\r\n" % offsets[num])
    buf.extend(b"6 2\n")
    for num in (6, 7):
        buf.extend(b"%010d 00000 n\r\n" % offsets[num])
    buf.extend(b"trailer\n<< /Size 8 /Root 1 0 R /XRefStm %d >>\n" % offsets[7])
    buf.extend(b"startxref\n%d\n%%%%EOF\n" % xref_off)

    with open(out_path, "wb") as f:
        f.write(bytes(buf))


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Aufruf: make-hybrid-xref.py <ausgabe.pdf>")
    main(sys.argv[1])
