#!/usr/bin/env python3
"""Extrahiert aus einer von PDF-AS signierten Datei das Manifest eines Golden-Falls (Plan §8.2).

Quellen:
- `qpdf --json=2 --decode-level=generalized --json-stream-data=inline` (Objektgraph, dekodierte Streams)
- `qpdf --check` (Struktur)
- pyHanko `validate_pdf_signature` (Integrität, Änderungsniveau; Testzertifikat als Vertrauensanker)
- optional das DEBUG-Log von PDF-AS (PositioningInstruction, feste Tabellenbreite, CropBox)

Manifest (JSON):
  page            Zielseite (0-basiert), Seitenzahl vor/nach, MediaBox/CropBox/Rotate der Zielseite
  widget          /Rect (wie geschrieben), /F, /T, /TU
  holder          /BBox, /Matrix, Inhalt
  inner           /BBox, Inhalt (Text), Operationen: rect, line, image, text (Font, Größe, Positionen,
                  kodierte Bytes, Unicode), Fonts (BaseFont ohne Subset-Präfix, Subtype, Descriptor,
                  /W), Bilder (Maße, Farbraum, SMask, SHA-256 der dekodierten Samples)
  signature       /Filter, /SubFilter, /Name, /Reason, /Location, Länge /Contents
  sig_date        SIG_DATE-Text aus dem Block (Zeitstempel ist variabel, die Tests bauen damit nach)
  oracles         qpdf --check, pyHanko (intact/valid/coverage/modification_level)
  pdfas_log       Werte aus dem DEBUG-Log (falls übergeben)

Aufruf: extract.py <signiert.pdf> <manifest.json> [--log <pdfas.log>] [--input-pages N]
        [--trust <zert.pem>] [--case <case.json>]
"""

import argparse
import base64
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

# ------------------------------------------------------------------------------------------------
# qpdf-JSON-Objektgraph
# ------------------------------------------------------------------------------------------------


class Doc:
    def __init__(self, pdf: Path):
        raw = subprocess.run(
            ["qpdf", "--json=2", "--decode-level=generalized", "--json-stream-data=inline", str(pdf)],
            capture_output=True,
        )
        if raw.returncode not in (0, 3):  # 3 = Warnungen
            raise RuntimeError(f"qpdf --json fehlgeschlagen: {raw.stderr.decode(errors='replace')[:500]}")
        top = json.loads(raw.stdout)["qpdf"]
        self.objects = top[1]
        self.trailer = self.objects["trailer"]["value"]

    def get(self, ref):
        """Objekt hinter `ref` ("N G R") oder `ref` selbst."""
        if isinstance(ref, str) and re.fullmatch(r"\d+ \d+ R", ref):
            o = self.objects.get(f"obj:{ref}")
            if o is None:
                return None
            if "value" in o:
                return o["value"]
            return {"__stream__": True, "dict": o["stream"]["dict"], "data": o["stream"].get("data")}
        return ref

    def dict_of(self, ref):
        o = self.get(ref)
        if isinstance(o, dict) and o.get("__stream__"):
            return o["dict"]
        return o

    def stream_data(self, ref):
        o = self.get(ref)
        if isinstance(o, dict) and o.get("__stream__"):
            return base64.b64decode(o["data"] or "")
        return None


def pdf_str(v):
    """qpdf-JSON-String ("u:…" Unicode, "b:…" Hex) → (Text, Bytes)."""
    if v is None:
        return None
    if isinstance(v, str) and v.startswith("u:"):
        return v[2:]
    if isinstance(v, str) and v.startswith("b:"):
        b = bytes.fromhex(v[2:])
        try:
            return b.decode("latin-1")
        except Exception:
            return v
    return v


def name(v):
    return v[1:] if isinstance(v, str) and v.startswith("/") else v


def strip_subset(n):
    if n and len(n) > 7 and n[6] == "+" and n[:6].isalpha() and n[:6].isupper():
        return n[7:]
    return n


def nums(a):
    return [float(x) for x in a] if isinstance(a, list) else None


# ------------------------------------------------------------------------------------------------
# Seiten und Felder
# ------------------------------------------------------------------------------------------------


def pages(doc):
    root = doc.dict_of(doc.trailer["/Root"])
    out = []

    def walk(ref, inherited):
        d = doc.dict_of(ref)
        inh = dict(inherited)
        for k in ("/MediaBox", "/CropBox", "/Rotate", "/Resources"):
            if k in d:
                inh[k] = doc.get(d[k])
        if name(d.get("/Type")) == "Pages" or "/Kids" in d:
            for kid in doc.get(d["/Kids"]):
                walk(kid, inh)
        else:
            mb = nums(inh.get("/MediaBox")) or [0, 0, 612, 792]
            out.append({
                "ref": ref,
                "media_box": mb,
                "crop_box": nums(inh.get("/CropBox")) or mb,
                "rotate": int(inh.get("/Rotate", 0) or 0),
            })

    walk(root["/Pages"], {})
    return out


def sig_fields(doc):
    root = doc.dict_of(doc.trailer["/Root"])
    af = doc.dict_of(root.get("/AcroForm")) if "/AcroForm" in root else None
    out = []
    if not af:
        return out

    def walk(ref, ft, prefix):
        d = doc.dict_of(ref)
        ft = d.get("/FT", ft)
        t = pdf_str(d.get("/T"))
        full = f"{prefix}.{t}" if prefix and t else (t or prefix)
        kids = doc.get(d.get("/Kids")) if "/Kids" in d else None
        if kids and any("/T" in doc.dict_of(k) for k in kids):
            for k in kids:
                walk(k, ft, full)
            return
        if name(ft) != "Sig":
            return
        widgets = [ref] if name(d.get("/Subtype")) == "Widget" else list(kids or [])
        out.append({"ref": ref, "name": full, "dict": d, "widgets": widgets})

    for f in doc.get(af.get("/Fields", [])) or []:
        walk(f, None, None)
    return out


# ------------------------------------------------------------------------------------------------
# Content-Stream
# ------------------------------------------------------------------------------------------------

DELIM = b"()<>[]{}/%"
WS = b" \t\r\n\x0c\x00"


def tokenize(data: bytes):
    i, n = 0, len(data)
    while i < n:
        c = data[i]
        if c in WS:
            i += 1
        elif c == ord("%"):
            while i < n and data[i] not in b"\r\n":
                i += 1
        elif c == ord("("):
            depth, j, buf = 1, i + 1, bytearray()
            while j < n:
                ch = data[j]
                if ch == ord("\\"):
                    nx = data[j + 1]
                    esc = {ord("n"): 10, ord("r"): 13, ord("t"): 9, ord("b"): 8, ord("f"): 12}
                    if nx in esc:
                        buf.append(esc[nx])
                        j += 2
                    elif ord("0") <= nx <= ord("7"):
                        k = j + 1
                        while k < n and k < j + 4 and ord("0") <= data[k] <= ord("7"):
                            k += 1
                        buf.append(int(data[j + 1:k], 8) & 0xFF)
                        j = k
                    elif nx in b"\r\n":
                        j += 2
                        if nx == 13 and j < n and data[j] == 10:
                            j += 1
                    else:
                        buf.append(nx)
                        j += 2
                    continue
                if ch == ord("("):
                    depth += 1
                elif ch == ord(")"):
                    depth -= 1
                    if depth == 0:
                        break
                buf.append(ch)
                j += 1
            yield ("str", bytes(buf), "literal")
            i = j + 1
        elif c == ord("<") and i + 1 < n and data[i + 1] == ord("<"):
            yield ("dict_start", None, None)
            i += 2
        elif c == ord(">") and i + 1 < n and data[i + 1] == ord(">"):
            yield ("dict_end", None, None)
            i += 2
        elif c == ord("<"):
            j = data.index(b">", i)
            hx = bytes(b for b in data[i + 1:j] if b not in WS)
            if len(hx) % 2:
                hx += b"0"
            yield ("str", bytes.fromhex(hx.decode()), "hex")
            i = j + 1
        elif c == ord("["):
            yield ("arr_start", None, None)
            i += 1
        elif c == ord("]"):
            yield ("arr_end", None, None)
            i += 1
        elif c == ord("/"):
            j = i + 1
            while j < n and data[j] not in WS and data[j] not in DELIM:
                j += 1
            yield ("name", data[i + 1:j].decode("latin-1"), None)
            i = j
        else:
            j = i
            while j < n and data[j] not in WS and data[j] not in DELIM:
                j += 1
            t = data[i:j].decode("latin-1")
            try:
                yield ("num", float(t), t)
            except ValueError:
                yield ("op", t, None)
            i = max(j, i + 1)


def operations(data: bytes):
    """Liste (Operator, Operanden) – Arrays als Python-Listen, Dicts als dict."""
    ops, stack, cont = [], [], []
    for kind, v, extra in tokenize(data):
        if kind in ("arr_start", "dict_start"):
            cont.append((kind, stack))
            stack = []
        elif kind == "arr_end":
            k, parent = cont.pop()
            parent.append(("arr", stack))
            stack = parent
        elif kind == "dict_end":
            k, parent = cont.pop()
            items = stack
            d = {}
            for a in range(0, len(items) - 1, 2):
                d[items[a][1]] = items[a + 1][1]
            parent.append(("dict", d))
            stack = parent
        elif kind == "op":
            ops.append((v, stack))
            stack = []
        else:
            stack.append((kind, v) if kind != "num" else ("num", v))
    return ops


def mul(m1, m2):
    """m1 × m2 (Zeilenvektor-Konvention wie PDF)."""
    a, b, c, d, e, f = m1
    A, B, C, D, E, F = m2
    return [a * A + b * C, a * B + b * D, c * A + d * C, c * B + d * D, e * A + f * C + E, e * B + f * D + F]


def r5(v):
    return round(float(v), 5)


# ------------------------------------------------------------------------------------------------
# Fonts
# ------------------------------------------------------------------------------------------------

WIN_ANSI_EXTRA = {
    0x80: "€", 0x82: "‚", 0x83: "ƒ", 0x84: "„", 0x85: "…", 0x86: "†",
    0x87: "‡", 0x88: "ˆ", 0x89: "‰", 0x8A: "Š", 0x8B: "‹", 0x8C: "Œ",
    0x8E: "Ž", 0x91: "‘", 0x92: "’", 0x93: "“", 0x94: "”", 0x95: "•",
    0x96: "–", 0x97: "—", 0x98: "˜", 0x99: "™", 0x9A: "š", 0x9B: "›",
    0x9C: "œ", 0x9E: "ž", 0x9F: "Ÿ",
}


def parse_cmap(data: bytes):
    m = {}
    text = data.decode("latin-1")
    for block in re.findall(r"beginbfchar(.*?)endbfchar", text, re.S):
        for src, dst in re.findall(r"<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>", block):
            m[int(src, 16)] = bytes.fromhex(dst).decode("utf-16-be", errors="replace")
    for block in re.findall(r"beginbfrange(.*?)endbfrange", text, re.S):
        for lo, hi, dst in re.findall(r"<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>\s*(<[0-9A-Fa-f]+>|\[[^\]]*\])", block):
            lo, hi = int(lo, 16), int(hi, 16)
            if dst.startswith("["):
                items = re.findall(r"<([0-9A-Fa-f]+)>", dst)
                for k, it in enumerate(items):
                    m[lo + k] = bytes.fromhex(it).decode("utf-16-be", errors="replace")
            else:
                base = bytes.fromhex(dst[1:-1])
                for k in range(hi - lo + 1):
                    b = bytearray(base)
                    b[-1] = (b[-1] + k) & 0xFF
                    m[lo + k] = bytes(b).decode("utf-16-be", errors="replace")
    return m


class Font:
    def __init__(self, doc, res_name, ref):
        d = doc.dict_of(ref)
        self.res = res_name
        self.subtype = name(d.get("/Subtype"))
        self.base_font_raw = name(d.get("/BaseFont"))
        self.base_font = strip_subset(self.base_font_raw)
        self.encoding = name(doc.get(d.get("/Encoding"))) if isinstance(doc.get(d.get("/Encoding")), str) else None
        self.two_byte = self.subtype == "Type0"
        self.to_unicode = parse_cmap(doc.stream_data(d["/ToUnicode"])) if "/ToUnicode" in d else {}
        self.widths = {}
        self.dw = 1000.0
        info = {
            "res": res_name,
            "subtype": self.subtype,
            "base_font": self.base_font,
            "subset_prefix": self.base_font_raw != self.base_font,
            "encoding": self.encoding,
            "to_unicode": "/ToUnicode" in d,
        }
        desc_ref = d.get("/FontDescriptor")
        if self.two_byte:
            desc = doc.dict_of(doc.get(d["/DescendantFonts"])[0])
            info["cid_subtype"] = name(desc.get("/Subtype"))
            info["cid_to_gid_map"] = (
                name(desc.get("/CIDToGIDMap")) if isinstance(desc.get("/CIDToGIDMap"), str)
                and desc.get("/CIDToGIDMap", "").startswith("/") else ("stream" if "/CIDToGIDMap" in desc else None)
            )
            self.dw = float(desc.get("/DW", 1000))
            w = doc.get(desc.get("/W")) or []
            w = [doc.get(x) for x in w]
            k = 0
            while k < len(w):
                first = int(w[k])
                if isinstance(w[k + 1], list):
                    for j, val in enumerate(w[k + 1]):
                        self.widths[first + j] = float(doc.get(val))
                    k += 2
                else:
                    last, val = int(w[k + 1]), float(w[k + 2])
                    for c in range(first, last + 1):
                        self.widths[c] = val
                    k += 3
            info["dw"] = self.dw
            info["w"] = {str(c): v for c, v in sorted(self.widths.items())}
            desc_ref = desc.get("/FontDescriptor")
        elif "/Widths" in d:
            fc = int(d.get("/FirstChar", 0))
            for j, val in enumerate(doc.get(d["/Widths"])):
                self.widths[fc + j] = float(doc.get(val))
        if desc_ref is not None:
            fd = doc.dict_of(desc_ref)
            info["descriptor"] = {
                k[1:]: (nums(doc.get(v)) if isinstance(doc.get(v), list) else
                        (float(doc.get(v)) if isinstance(doc.get(v), (int, float)) else name(doc.get(v))))
                for k, v in fd.items()
                if k in ("/Ascent", "/Descent", "/CapHeight", "/Flags", "/FontBBox", "/ItalicAngle", "/StemV",
                         "/FontWeight", "/XHeight")
            }
            info["embedded"] = next((k[1:] for k in ("/FontFile", "/FontFile2", "/FontFile3") if k in fd), None)
            info["cid_set"] = "/CIDSet" in fd
        else:
            info["embedded"] = None
        self.info = info

    def codes(self, b: bytes):
        if self.two_byte:
            return [int.from_bytes(b[i:i + 2], "big") for i in range(0, len(b) - 1, 2)]
        return list(b)

    def text(self, b: bytes):
        out = []
        for c in self.codes(b):
            if c in self.to_unicode:
                out.append(self.to_unicode[c])
            elif not self.two_byte:
                out.append(WIN_ANSI_EXTRA.get(c, bytes([c]).decode("cp1252", errors="replace")))
            else:
                out.append("�")
        return "".join(out)

    def width(self, b: bytes):
        """Breite in Texteinheiten/1000 (nur bekannte Breiten; Standard-14 ohne /Widths ⇒ None)."""
        if not self.widths and not self.two_byte:
            return None
        return sum(self.widths.get(c, self.dw if self.two_byte else 0.0) for c in self.codes(b))


# ------------------------------------------------------------------------------------------------
# Inner-Form auswerten
# ------------------------------------------------------------------------------------------------


def image_info(doc, ref):
    o = doc.get(ref)
    d = o["dict"]
    data = doc.stream_data(ref) or b""
    info = {
        "width": int(d.get("/Width", 0)),
        "height": int(d.get("/Height", 0)),
        "bpc": int(d.get("/BitsPerComponent", 0)),
        "color_space": name(doc.get(d.get("/ColorSpace"))) if isinstance(doc.get(d.get("/ColorSpace")), str)
        else (name(doc.get(d.get("/ColorSpace"))[0]) if isinstance(doc.get(d.get("/ColorSpace")), list) else None),
        "filter": name(d.get("/Filter")) if isinstance(d.get("/Filter"), str) else d.get("/Filter"),
        "decoded_len": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }
    if "/SMask" in d:
        sm = doc.stream_data(d["/SMask"]) or b""
        smd = doc.dict_of(d["/SMask"])
        info["smask"] = {"width": int(smd.get("/Width", 0)), "height": int(smd.get("/Height", 0)),
                         "sha256": hashlib.sha256(sm).hexdigest()}
    if "/Mask" in d:
        info["mask"] = True
    return info


def analyse_form(doc, ref):
    o = doc.get(ref)
    d = o["dict"]
    content = doc.stream_data(ref) or b""
    res = doc.dict_of(d.get("/Resources")) or {}
    fonts = {}
    for k, v in (doc.dict_of(res.get("/Font")) or {}).items():
        fonts[name(k)] = Font(doc, name(k), v)
    xobjects = {}
    for k, v in (doc.dict_of(res.get("/XObject")) or {}).items():
        xd = doc.dict_of(v)
        xobjects[name(k)] = (name(xd.get("/Subtype")), v)
    images = {n: image_info(doc, r) for n, (st, r) in xobjects.items() if st == "Image"}

    ops_out, texts = [], []
    ctm = [1, 0, 0, 1, 0, 0]
    gstack = []
    lw = 1.0
    fill, stroke = ["g", [0.0]], ["G", [0.0]]
    path = []  # Liste von Segmenten
    cur = None
    text_block = None
    font, size = None, 0.0
    tm = [1, 0, 0, 1, 0, 0]
    tlm = [1, 0, 0, 1, 0, 0]
    leading = 0.0
    color_ops = []
    for op, args in operations(content):
        vals = [a[1] for a in args]
        if op == "q":
            gstack.append((list(ctm), lw, list(fill), list(stroke)))
        elif op == "Q":
            ctm, lw, fill, stroke = gstack.pop()
        elif op == "cm":
            ctm = mul([float(x) for x in vals[:6]], ctm)
        elif op == "w":
            lw = float(vals[0])
            ops_out.append({"op": "line_width", "w": r5(lw)})
        elif op in ("g", "rg", "k", "sc", "scn"):
            fill = [op, [float(x) for x in vals if isinstance(x, float)]]
            color_ops.append(op)
        elif op in ("G", "RG", "K", "SC", "SCN"):
            stroke = [op, [float(x) for x in vals if isinstance(x, float)]]
            color_ops.append(op)
        elif op in ("cs", "CS"):
            color_ops.append(f"{op}:{vals[0]}")
        elif op == "re":
            x, y, w, h = (float(v) for v in vals[:4])
            path.append(("re", [x, y, w, h]))
        elif op == "m":
            cur = (float(vals[0]), float(vals[1]))
            path.append(("m", list(cur)))
        elif op == "l":
            p = (float(vals[0]), float(vals[1]))
            path.append(("l", [cur[0], cur[1], p[0], p[1]]))
            cur = p
        elif op in ("f", "F", "f*", "S", "s", "B", "B*", "b", "b*", "n"):
            for kind, v in path:
                if kind == "re":
                    ops_out.append({"op": "rect", "x": r5(v[0]), "y": r5(v[1]), "w": r5(v[2]), "h": r5(v[3]),
                                    "paint": op, "fill": fill[1] if op != "S" else None})
                elif kind == "l":
                    ops_out.append({"op": "line", "x1": r5(v[0]), "y1": r5(v[1]), "x2": r5(v[2]), "y2": r5(v[3]),
                                    "width": r5(lw), "paint": op, "stroke": stroke[1]})
            path = []
        elif op == "Do":
            n = vals[0]
            st = xobjects.get(n, (None, None))[0]
            ops_out.append({"op": "image" if st == "Image" else "xobject", "name": n,
                            "matrix": [r5(x) for x in ctm]})
        elif op == "BT":
            tm = [1, 0, 0, 1, 0, 0]
            tlm = [1, 0, 0, 1, 0, 0]
            text_block = {"font": None, "res": None, "size": None, "lines": []}
        elif op == "ET":
            if text_block is not None and text_block["lines"]:
                texts.append(text_block)
                ops_out.append({"op": "text", "index": len(texts) - 1})
            text_block = None
        elif op == "Tf":
            font = fonts.get(vals[0])
            size = float(vals[1])
            if text_block is not None:
                text_block["font"] = font.base_font if font else vals[0]
                text_block["res"] = vals[0]
                text_block["size"] = r5(size)
        elif op == "TL":
            leading = float(vals[0])
        elif op in ("Td", "TD"):
            tx, ty = float(vals[0]), float(vals[1])
            if op == "TD":
                leading = -ty
            tlm = mul([1, 0, 0, 1, tx, ty], tlm)
            tm = list(tlm)
        elif op == "Tm":
            tlm = [float(x) for x in vals[:6]]
            tm = list(tlm)
        elif op in ("T*", "'", '"'):
            tlm = mul([1, 0, 0, 1, 0, -leading], tlm)
            tm = list(tlm)
        if op in ("Tj", "'", '"', "TJ"):
            if op == "TJ":
                # Kerning-Zahlen im Array verschieben die Position (PDFBox schreibt hier nur Tj)
                items = vals[0] if isinstance(vals[0], list) else []
                strings = []
                for kind, val in items:
                    if kind == "str":
                        strings.append(val)
                    elif kind == "num" and not strings:
                        tm = mul([1, 0, 0, 1, -val / 1000.0 * size, 0], tm)
            else:
                strings = [vals[-1]]
            for b in strings:
                w = font.width(b) if font else None
                line = {"x": r5(tm[4]), "y": r5(tm[5]), "hex": b.hex(), "text": font.text(b) if font else None,
                        "width": r5(w / 1000.0 * size) if w is not None else None}
                if text_block is not None:
                    text_block["lines"].append(line)
                if w is not None:
                    tm = mul([1, 0, 0, 1, w / 1000.0 * size, 0], tm)
    return {
        "bbox": nums(doc.get(d.get("/BBox"))),
        "matrix": nums(doc.get(d.get("/Matrix"))) if "/Matrix" in d else None,
        "content": content.decode("latin-1"),
        "ops": ops_out,
        "texts": texts,
        "fonts": {n: f.info for n, f in sorted(fonts.items())},
        "images": images,
        "color_ops": sorted(set(color_ops)),
        "proc_set": [name(x) for x in (doc.get(res.get("/ProcSet")) or [])],
    }


# ------------------------------------------------------------------------------------------------
# Orakel
# ------------------------------------------------------------------------------------------------


def qpdf_check(pdf):
    r = subprocess.run(["qpdf", "--check", str(pdf)], capture_output=True, text=True)
    lines = [ln for ln in (r.stdout + r.stderr).splitlines() if ln.strip()]
    return {"exit": r.returncode, "ok": r.returncode == 0,
            "warnings": [ln for ln in lines if "WARNING" in ln.upper()][:10]}


def pyhanko_validate(pdf, trust_pem):
    try:
        from pyhanko.keys import load_cert_from_pemder
        from pyhanko.pdf_utils.reader import PdfFileReader
        from pyhanko.sign.validation import validate_pdf_signature
        from pyhanko_certvalidator import ValidationContext
    except Exception as e:  # pragma: no cover - pyHanko fehlt
        return {"skipped": f"pyHanko nicht verfügbar: {e}"}
    try:
        cert = load_cert_from_pemder(str(trust_pem))
        vc = ValidationContext(trust_roots=[cert], allow_fetching=False)
        with open(pdf, "rb") as f:
            r = PdfFileReader(f, strict=False)
            sigs = r.embedded_signatures
            st = validate_pdf_signature(sigs[-1], vc)
            return {
                "signatures": len(sigs),
                "intact": bool(st.intact),
                "valid": bool(st.valid),
                "trusted": bool(st.trusted),
                "coverage": str(st.coverage.name) if st.coverage else None,
                "modification_level": str(st.modification_level.name) if st.modification_level else None,
                "md_algorithm": st.md_algorithm,
            }
    except Exception as e:
        return {"error": f"{type(e).__name__}: {str(e)[:300]}"}


LOG_PATTERNS = {
    "positioning": re.compile(
        r"PositioningInstruction \[page=(\d+), make_new_page=(\w+), x=([-\d.E]+), y=([-\d.E]+), r=([-\d.E]+)\]"),
    "fixed_width": re.compile(r"Generating Table with fixed With ([-\d.E]+) got width ([-\d.E]+)"),
    "crop_box": re.compile(r"Original CropBox: \[([^\]]*)\]"),
    "rotated_crop_box": re.compile(r"Rotated CropBox: \[([^\]]*)\]"),
    "rect": re.compile(r"rectangle of signature has been created: \[([^\]]*)\]"),
    "signature_positioning": re.compile(r"using signature Positioning: (.*)$"),
    "request_positioning": re.compile(r"using Positioning: (.*)$"),
    "placeholder": re.compile(r"Found Placeholder at: (\S+)"),
}


def parse_log(path):
    out = {}
    text = Path(path).read_text(errors="replace")
    lines = text.splitlines()
    pos_idx = None
    for i, ln in enumerate(lines):
        m = LOG_PATTERNS["positioning"].search(ln)
        if m:
            pos_idx = i
            out["positioning"] = {"page": int(m.group(1)), "make_new_page": m.group(2) == "true",
                                  "x": float(m.group(3)), "y": float(m.group(4)), "r": float(m.group(5))}
    widths = []
    for i, ln in enumerate(lines):
        m = LOG_PATTERNS["fixed_width"].search(ln)
        if m and (pos_idx is None or i < pos_idx):
            widths.append((float(m.group(1)), float(m.group(2))))
    if widths:
        out["fixed_width"] = widths[-1][0]
        out["table_width"] = widths[-1][1]
    for key in ("crop_box", "rotated_crop_box", "rect"):
        vals = [LOG_PATTERNS[key].search(ln) for ln in lines]
        vals = [m.group(1) for m in vals if m]
        if vals:
            out[key] = [float(x) for x in vals[-1].split(",")]
    for key in ("signature_positioning", "request_positioning", "placeholder"):
        vals = [LOG_PATTERNS[key].search(ln) for ln in lines]
        vals = [m.group(1).strip() for m in vals if m]
        if vals:
            out[key] = vals[-1]
    if out.get("request_positioning") == "null":
        out["request_positioning"] = None
    out["error"] = next((ln.strip()[:300] for ln in lines if "PDF-AS Error" in ln or "Exception:" in ln), None)
    return out


# ------------------------------------------------------------------------------------------------
# Hauptteil
# ------------------------------------------------------------------------------------------------

DATE_RE = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:[+-]\d{2}:\d{2}|Z)")


def page_index_of(pgs, ref):
    for i, p in enumerate(pgs):
        if p["ref"] == ref:
            return i
    return None


def extract(pdf: Path, log=None, input_pages=None, trust=None, case=None):
    doc = Doc(pdf)
    size = pdf.stat().st_size
    pgs = pages(doc)
    fields = sig_fields(doc)
    newest = None
    for f in fields:
        v = doc.dict_of(f["dict"].get("/V")) if "/V" in f["dict"] else None
        if not v:
            continue
        br = [int(x) for x in doc.get(v.get("/ByteRange", []))]
        if len(br) == 4 and br[2] + br[3] == size:
            newest = (f, v, br)
    if newest is None:
        raise RuntimeError("keine Signatur über die ganze Datei gefunden")
    f, v, br = newest
    wref = f["widgets"][0]
    w = doc.dict_of(wref)
    page = page_index_of(pgs, w.get("/P"))
    manifest = {
        "case": case,
        "file": {"size": size, "pages": len(pgs), "input_pages": input_pages,
                 "new_page": (len(pgs) > input_pages) if input_pages is not None else None},
        "page": None,
        "widget": {
            "rect": nums(doc.get(w.get("/Rect"))),
            "flags": w.get("/F"),
            "field_name": f["name"],
            "tu": pdf_str(w.get("/TU", f["dict"].get("/TU"))),
            "page_index": page,
        },
        "signature": {
            "filter": name(v.get("/Filter")),
            "sub_filter": name(v.get("/SubFilter")),
            "name": pdf_str(v.get("/Name")),
            "reason": pdf_str(v.get("/Reason")),
            "location": pdf_str(v.get("/Location")),
            "m": pdf_str(v.get("/M")),
            "contents_bytes": len(bytes.fromhex(v["/Contents"][2:])) if str(v.get("/Contents", "")).startswith("b:")
            else None,
            "byte_range": br,
        },
    }
    if page is not None:
        p = pgs[page]
        manifest["page"] = {"index": page, "media_box": p["media_box"], "crop_box": p["crop_box"],
                            "rotate": p["rotate"]}
    ap = doc.dict_of(w.get("/AP")) if "/AP" in w else None
    holder_ref = ap.get("/N") if ap else None
    if holder_ref is not None:
        hd = doc.get(holder_ref)["dict"]
        hres = doc.dict_of(hd.get("/Resources")) or {}
        hx = doc.dict_of(hres.get("/XObject")) or {}
        manifest["holder"] = {
            "bbox": nums(doc.get(hd.get("/BBox"))),
            "matrix": nums(doc.get(hd.get("/Matrix"))) if "/Matrix" in hd else None,
            "content": (doc.stream_data(holder_ref) or b"").decode("latin-1"),
            "xobjects": sorted(name(k) for k in hx),
        }
        inner = None
        for k, ref in sorted(hx.items()):
            inner = analyse_form(doc, ref)
            inner["name"] = name(k)
            break
        manifest["inner"] = inner
        if inner:
            dates = [ln["text"] for t in inner["texts"] for ln in t["lines"]
                     if ln.get("text") and DATE_RE.fullmatch(ln["text"].strip())]
            manifest["sig_date"] = dates[0].strip() if dates else None
    manifest["oracles"] = {"qpdf_check": qpdf_check(pdf)}
    if trust:
        manifest["oracles"]["pyhanko"] = pyhanko_validate(pdf, trust)
    if log:
        manifest["pdfas_log"] = parse_log(log)
    return manifest


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pdf")
    ap.add_argument("manifest")
    ap.add_argument("--log")
    ap.add_argument("--input-pages", type=int)
    ap.add_argument("--trust")
    ap.add_argument("--case")
    a = ap.parse_args()
    case = json.loads(Path(a.case).read_text()) if a.case else None
    m = extract(Path(a.pdf), a.log, a.input_pages, a.trust, case)
    Path(a.manifest).write_text(json.dumps(m, indent=1, ensure_ascii=False, sort_keys=True) + "\n")


if __name__ == "__main__":
    sys.exit(main())
