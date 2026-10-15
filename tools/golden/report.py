#!/usr/bin/env python3
"""Erzeugt docs/integration/golden-report.md aus dem Golden-Lauf (testdata/golden/pdfas-4.4.2) und den
JSON-Ergebnissen der Rust-Golden-Tests.

Ablauf:
  PIDA_GOLDEN_REPORT_DIR=target/golden/report cargo test -p pida-sigblock --test golden_pdfas442
  PIDA_GOLDEN_REPORT_DIR=target/golden/report cargo test -p pida-placement --test golden_pdfas442
  python3 tools/golden/report.py --golden testdata/golden/pdfas-4.4.2 --reports target/golden/report \
      --out docs/integration/golden-report.md

Der feste Text (Einleitung, Befunde, Register) steht in `report_text.py`; dieses Skript ergänzt die
Tabellen (Kennzahlen je Profil/Eingabe und die Tabelle je Fall) an den Platzhaltern
`<!-- TABELLE:… -->`.
"""

import argparse
import collections
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from report_text import TEMPLATE  # noqa: E402


def load(p):
    return json.loads(Path(p).read_text())


def fmt(v, nd=2):
    if v is None:
        return "–"
    s = f"{v:.{nd}f}".rstrip("0").rstrip(".")
    return s.replace(".", ",") if s not in ("-0", "") else "0"


def pct(v):
    if v is None:
        return "–"
    return f"{v * 100:.4f} %".replace(".", ",")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--golden", required=True)
    ap.add_argument("--reports", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    g = Path(a.golden)
    r = Path(a.reports)
    index = load(g / "index.json")
    cases = index["cases"]
    manifests = {c["id"]: load(g / "cases" / c["id"] / "manifest.json") for c in cases}
    structure = {o["id"]: o for o in load(r / "sigblock-structure.json")}
    quirks_bytes = {i: e for i, e in load(r / "sigblock-quirks.json")}
    pixels = {o["id"]: o for o in load(r / "sigblock-pixels.json")}
    pq = {o["id"]: o for o in load(r / "placement-quirks.json")}
    pc = {o["id"]: o for o in load(r / "placement-corrected.json")}

    tables = {}

    # Kennzahlen
    meta = index.get("meta", {})
    oracle_ok = sum(
        1 for m in manifests.values()
        if (m.get("oracles", {}).get("pyhanko") or {}).get("valid")
        and (m.get("oracles", {}).get("pyhanko") or {}).get("intact")
        and (m.get("oracles", {}).get("qpdf_check") or {}).get("ok")
    )
    kept = sum(1 for c in cases if c.get("pdf"))
    lines = [
        "| Größe | Wert |",
        "|---|---|",
        f"| PDF-AS | {meta.get('pdfas', {}).get('version', '?')} (offizielle CLI-Distribution, SHA-256 `{meta.get('pdfas', {}).get('sha256', '?')}`) |",
        f"| Java (nur Orakel) | {meta.get('java', '?')} |",
        f"| qpdf / pyHanko / Python | {meta.get('qpdf', '?')} / {meta.get('pyhanko', '?')} / {meta.get('python', '?')} |",
        f"| Lauf | {meta.get('generated', '?')}, Zeitzone {meta.get('timezone', '?')} |",
        f"| Fälle | {len(cases)} ({len({c['profile'] for c in cases})} Profil-IDs, {len({c['input'] for c in cases})} Eingaben, {len({c['position_key'] for c in cases})} Positionsarten) |",
        f"| Orakel je Ausgabe | pyHanko intact + valid + trusted und `qpdf --check` ohne Befund: {oracle_ok} von {len(cases)} |",
        f"| signierte Referenz-PDFs im Repository | {kept} (übrige Fälle nur als Manifest) |",
    ]
    tables["KENNZAHLEN"] = "\n".join(lines)

    # Signaturblock je Profil
    agg = collections.OrderedDict()
    for c in cases:
        if c["profile"] == "INVISIBLE":
            continue
        e = agg.setdefault(c["profile"], dict(n=0, same=0, doc=0, und=0, dev=0.0, qbytes=0, px=0, pxsame=0,
                                               reg=set()))
        o = structure.get(c["id"])
        e["n"] += 1
        if o:
            if not o["unexpected"] and not o["expected"]:
                e["same"] += 1
            if o["expected"]:
                e["doc"] += 1
                for _, reason in o["expected"]:
                    e["reg"].add(reason.split()[0])
            if o["unexpected"]:
                e["und"] += 1
            if not any(k in ("ops", "bbox") for k, _ in o["expected"]):
                e["dev"] = max(e["dev"], o["max_dev"])
        if not quirks_bytes.get(c["id"]):
            e["qbytes"] += 1
        p = pixels.get(c["id"])
        if p:
            e["px"] += 1
            if p["default"] and p["default"][0] == 0 and p["quirks"] and p["quirks"][0] == 0 \
                    and p["standalone"] and p["standalone"][0] == 0:
                e["pxsame"] += 1
    lines = ["| Profil | Fälle | Struktur gleich | dokumentiert abweichend (Register) | undokumentiert | max. Abweichung Koordinaten | Content-Stream byte-gleich (Quirks) | Pixel: PDFs / pixelgleich |",
             "|---|---|---|---|---|---|---|---|"]
    for p, e in agg.items():
        lines.append(f"| `{p}` | {e['n']} | {e['same']} | {e['doc']} ({', '.join(sorted(e['reg'])) or '–'}) | "
                     f"{e['und']} | {fmt(e['dev'], 6)} pt | {e['qbytes']} | {e['px']} / {e['pxsame']} |")
    tables["SIGBLOCK"] = "\n".join(lines)

    # Pixel je Profil
    pagg = collections.OrderedDict()
    for c in cases:
        p = pixels.get(c["id"])
        if not p:
            continue
        e = pagg.setdefault(c["profile"], dict(n=0, und=[0.0, 0.0, 0.0], doc=[0.0, 0.0, 0.0], reg=set()))
        e["n"] += 1
        s = structure.get(c["id"], {})
        reg = {reason.split()[0] for _, reason in s.get("expected", [])}
        emb = c.get("emblem") or ""
        is_doc = bool(reg) or emb in ("emblem-tall.png", "emblem-indexed.png", "emblem-photo.jpg") \
            or (emb == "emblem-alpha.png" and c["profile"].endswith("_PDFA")) \
            or (c.get("base_logo_mm") or 0) > 120
        for k, key in enumerate(("default", "quirks", "standalone")):
            v = p[key][0] if p[key] else 1.0
            if is_doc:
                e["doc"][k] = max(e["doc"][k], v)
                if v > 0:
                    e["reg"] |= reg
            else:
                e["und"][k] = max(e["und"][k], v)
    lines = ["| Profil | PDFs | Diff-Quote ohne dokumentierte Abweichung (Standard / Quirks / `to_standalone_pdf`) | Diff-Quote Fälle mit dokumentierter Abweichung (Standard / Quirks / `to_standalone_pdf`) |",
             "|---|---|---|---|"]
    for p, e in pagg.items():
        lines.append(f"| `{p}` | {e['n']} | {' / '.join(pct(v) for v in e['und'])} | {' / '.join(pct(v) for v in e['doc'])} |")
    tables["PIXEL"] = "\n".join(lines)

    # Positionierung je Eingabe
    def place_agg(src):
        out = collections.OrderedDict()
        for c in cases:
            o = src.get(c["id"])
            if not o:
                continue
            e = out.setdefault(c["input"], dict(n=0, same=0, doc=0, und=0, skip=0, scans=0, scan_exact=0,
                                                 dev=0.0, reg=set()))
            e["n"] += 1
            if o.get("skipped"):
                e["skip"] += 1
                continue
            if not o["errors"]:
                e["same"] += 1
                if o["scanned"]:
                    e["scans"] += 1
                    if abs(o["dy"]) < 0.01 and abs(o["dx"]) < 0.01:
                        e["scan_exact"] += 1
                e["dev"] = max(e["dev"], abs(o["dy"]), abs(o["dx"]), o["drect"])
            elif o.get("documented"):
                e["doc"] += 1
                e["reg"].add(o["documented"].split()[0])
            else:
                e["und"] += 1
        return out

    for name, src in (("PLACEMENT_QUIRKS", pq), ("PLACEMENT_CORRECTED", pc)):
        lines = ["| Eingabe | Fälle | gleich | davon Rasterscan (exakt / 1 pt) | dokumentiert abweichend (Register) | undokumentiert | übersprungen | max. Abweichung (pt) |",
                 "|---|---|---|---|---|---|---|---|"]
        for inp, e in place_agg(src).items():
            lines.append(f"| `{inp}` | {e['n']} | {e['same']} | {e['scans']} ({e['scan_exact']} / {e['scans'] - e['scan_exact']}) | "
                         f"{e['doc']} ({', '.join(sorted(e['reg'])) or '–'}) | {e['und']} | {e['skip']} | {fmt(e['dev'])} |")
        tables[name] = "\n".join(lines)

    # Tabelle je Fall
    lines = ["| Fall | Seite (PDF-AS) | Rect (PDF-AS) | Block W × H | Signaturblock | Bytes (Quirks) | Pixel (Standard / Quirks) | Position Quirks | Position korrigiert |",
             "|---|---|---|---|---|---|---|---|---|"]
    for c in sorted(cases, key=lambda c: (c["profile"], c["input"], c["position_key"], c["id"])):
        m = manifests[c["id"]]
        w = m["widget"]
        page = f"{(w.get('page_index') or 0) + 1}" + (" neu" if (m.get("file") or {}).get("new_page") else "")
        rect = w["rect"]
        rr = [min(rect[0], rect[2]), min(rect[1], rect[3]), max(rect[0], rect[2]), max(rect[1], rect[3])]
        rect_s = "[" + " ".join(fmt(v) for v in rr) + "]"
        inner = m.get("inner")
        wh = f"{fmt(abs(inner['bbox'][0]) - 1, 3)} × {fmt(abs(inner['bbox'][1]) - 1, 3)}" if inner else "–"
        s = structure.get(c["id"])
        if c["profile"] == "INVISIBLE":
            sb, qb = "unsichtbar", "–"
        else:
            if s is None:
                sb = "–"
            elif s["unexpected"]:
                sb = "**abweichend**"
            elif s["expected"]:
                sb = "dok. " + ", ".join(sorted({reason.split()[0] for _, reason in s["expected"]}))
            else:
                sb = "gleich"
            if not quirks_bytes.get(c["id"]):
                qb = "gleich"
            else:  # einzige dokumentierte Byte-Abweichung: SB-G1 (Logo > 120 mm)
                qb = "dok. SB-G1" if (c.get("base_logo_mm") or 0) > 120 else "**abweichend**"
        p = pixels.get(c["id"])
        px = f"{pct(p['default'][0] if p['default'] else None)} / {pct(p['quirks'][0] if p['quirks'] else None)}" if p else "–"

        def pos(o):
            if o is None:
                return "–"
            if o.get("skipped"):
                return "übersprungen"
            if not o["errors"]:
                if o["scanned"]:
                    return f"gleich (Scan, Δy {fmt(o['dy'], 1)})"
                return "gleich"
            if o.get("documented"):
                return "dok. " + o["documented"].split()[0]
            return "**abweichend**"

        lines.append(f"| `{c['id']}` | {page} | {rect_s} | {wh} | {sb} | {qb} | {px} | {pos(pq.get(c['id']))} | "
                     f"{pos(pc.get(c['id']))} |")
    tables["FAELLE"] = "\n".join(lines)

    text = TEMPLATE
    for k, v in tables.items():
        text = text.replace(f"<!-- TABELLE:{k} -->", v)
    Path(a.out).write_text(text)
    print(f"{a.out}: {len(cases)} Fälle")


if __name__ == "__main__":
    main()
