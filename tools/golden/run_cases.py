#!/usr/bin/env python3
"""Fallmatrix und Ausführung des PDF-AS-4.4.2-Golden-Laufs (Plan §8.2). Einstieg: run-pdfas.sh.

Je Fall wird PDF-AS so aufgerufen, wie PDF-Over es täte:
- Profil-ID (alle 12 aus pdfover_sigblock::PDFOVER_PROFILE_IDS), Positionsstring wie PDF-Over
  (`p:auto;x:auto;y:auto`, `p:%d;x:%f;y:%f`, …) bzw. weitere PDF-AS-Positionsstrings,
- Laufzeit-Overrides von PDF-Over (`PdfAs4Signer.prepare`) als Zeilen am Ende der Profildatei einer
  Konfigurationskopie: `value.SIG_LABEL` (Bildmarke), `value.SIG_NOTE` (Hinweis), bei BASE_LOGO
  `table.main.Style.padding=0`, `pos=w:<tw>;f:0`, `table.main.Style.imagescaletofit=<tw>;<th>`,
- QR-Platzhalter: PlaceholderSign.java (Platzhaltersuche wie PDF-Over, Modus 1, ID 1).

Signiert wird mit dem Test-PKCS#12 (RSA) über die Keystore-Option der CLI (`-c ks`).
"""

import argparse
import concurrent.futures as cf
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import extract  # noqa: E402

PROFILES = [
    "SIGNATURBLOCK_SMALL_DE",
    "SIGNATURBLOCK_SMALL_DE_NOTE",
    "SIGNATURBLOCK_SMALL_DE_PDFA",
    "SIGNATURBLOCK_SMALL_DE_NOTE_PDFA",
    "SIGNATURBLOCK_SMALL_EN",
    "SIGNATURBLOCK_SMALL_EN_NOTE",
    "SIGNATURBLOCK_SMALL_EN_PDFA",
    "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA",
    "AMTSSIGNATURBLOCK_DE_RECOMMENDED",
    "AMTSSIGNATURBLOCK_EN_RECOMMENDED",
    "BASE_LOGO",
    "INVISIBLE",
]

POSITIONS = {
    "auto": "p:auto;x:auto;y:auto",          # PDF-Over: Auto-Positionierung
    "manual": "p:1;x:100;y:700",
    "pdfover": "p:1;x:50.500000;y:300.250000",  # PDF-Over-Format p:%d;x:%f;y:%f
    "new": "p:new",
    "beyond": "p:2;x:100;y:700",             # PDF-Over „Neue Seite“: p:<n+1>;x;y
    "last": "p:last",
    "footer0": "p:auto;x:auto;y:auto;f:0",
    "footer300": "p:auto;x:auto;y:auto;f:300",
    "width": "p:1;x:100;y:700;w:300",
    "rot90": "p:1;x:100;y:500;r:90",
    "none": None,                            # nur Profil-pos
    "placeholder": None,                     # QR-Platzhalter (Position aus dem Platzhalter)
}

INPUT_POSITIONS = {
    "a4": ["auto", "manual", "pdfover", "new", "beyond", "last", "footer0", "width", "rot90", "none"],
    "a4-landscape": ["auto", "manual", "new"],
    "a4-rot90": ["auto", "manual", "new"],
    "a4-rot180": ["auto", "manual", "new"],
    "a4-rot270": ["auto", "manual", "new"],
    "a4-cropbox": ["auto", "manual", "new"],
    "content-half": ["auto", "footer0", "footer300"],
    "content-full": ["auto", "last", "footer0"],
    "two-pages": ["auto", "manual", "new"],
    "presigned": ["auto", "new", "manual"],
    "qr": ["placeholder"],
    "qr-id200": ["placeholder"],
}

# gesuchte Platzhalter-ID je Eingabe (PDF-Over: ID des beim Öffnen gefundenen Platzhalters)
PLACEHOLDER_IDS = {"qr": "1", "qr-id200": "200"}

NOTE_PROFILES = [
    "SIGNATURBLOCK_SMALL_DE_NOTE",
    "SIGNATURBLOCK_SMALL_DE_NOTE_PDFA",
    "SIGNATURBLOCK_SMALL_EN_NOTE",
    "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA",
    "AMTSSIGNATURBLOCK_DE_RECOMMENDED",
    "AMTSSIGNATURBLOCK_EN_RECOMMENDED",
]

NOTES = {
    "note-custom": "Testhinweis: Dieses Dokument wurde im Golden-Lauf signiert (äöüß €).",
    "note-long": (
        "Erste Zeile mit einem überlangen Wort Donaudampfschifffahrtsgesellschaftskapitänsmütze"
        "nabzeichen und mehreren  Leerzeichen.\nZweite Zeile nach Zeilenumbruch.\n\nNach einer Leerzeile: "
        "https://www.signaturpruefung.gv.at/ und Ende."
    ),
}

EMBLEMS = ["emblem-wide.png", "emblem-tall.png", "emblem-alpha.png", "emblem-photo.jpg", "emblem-indexed.png"]

# PDF-Over: Constants.PDF_UNITS_PER_MM = 595.0 / 210.0, DEFAULT_LOGO_ONLY_SIZE = 23
PDF_UNITS_PER_MM = 595.0 / 210.0
DEFAULT_LOGO_MM = 23.0

TIMEZONE = "Europe/Vienna"
JVM_OPTS = ["-Djava.awt.headless=true", f"-Duser.timezone={TIMEZONE}", "-Duser.language=de", "-Duser.country=AT"]


def case_list():
    cases = []

    def add(profile, inp, pos, note=None, emblem=None, logo_mm=None):
        parts = [profile, inp, pos]
        if note:
            parts.append(note)
        if emblem:
            parts.append(Path(emblem).stem)
        if logo_mm is not None and logo_mm != DEFAULT_LOGO_MM:
            parts.append(f"logo{int(logo_mm)}mm")
        cid = "__".join(parts)
        cases.append({
            "id": cid,
            "profile": profile,
            "input": inp,
            "position_key": pos,
            "position": POSITIONS[pos],
            "note_key": note,
            "note": NOTES.get(note) if note else None,
            "emblem": emblem,
            "base_logo_mm": (logo_mm if logo_mm is not None else DEFAULT_LOGO_MM) if profile == "BASE_LOGO" else None,
            "placeholder": pos == "placeholder",
            "placeholder_id": PLACEHOLDER_IDS.get(inp) if pos == "placeholder" else None,
        })

    for profile in PROFILES:
        for inp, poss in INPUT_POSITIONS.items():
            for pos in poss:
                add(profile, inp, pos)
    for profile in NOTE_PROFILES:
        for note in NOTES:
            add(profile, "a4", "manual", note=note)
    for profile in PROFILES:
        if profile == "INVISIBLE":
            continue
        for em in EMBLEMS:
            add(profile, "a4", "manual", emblem=em)
    for mm in (50.0, 150.0):
        add("BASE_LOGO", "a4", "manual", logo_mm=mm)
    return cases


# ------------------------------------------------------------------------------------------------
# Konfigurationsvarianten
# ------------------------------------------------------------------------------------------------


def prop_escape(value: str) -> str:
    """java.util.Properties-Wert (ISO-8859-1 mit \\uXXXX)."""
    out = []
    for i, ch in enumerate(value):
        if ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif ch == " " and i == 0:
            out.append("\\ ")
        elif ord(ch) < 0x20 or ord(ch) > 0x7E:
            out.append(f"\\u{ord(ch):04X}")
        else:
            out.append(ch)
    return "".join(out)


def java_double_to_string(v: float) -> str:
    """Double.toString (JDK ≥ 19: kürzeste Darstellung) für 1e-3 ≤ |v| < 1e7."""
    assert 1e-3 <= abs(v) < 1e7
    s = repr(float(v))
    return s if "." in s else s + ".0"


def emblem_px(path: Path):
    from PIL import Image

    with Image.open(path) as im:
        return im.size


def base_logo_overrides(case, inputs: Path):
    """PdfAs4Signer.prepare: BASE_LOGO-Overrides aus Emblemgröße und targetLogoSize."""
    if case["emblem"]:
        ew, eh = emblem_px(inputs / case["emblem"])
    else:
        ew, eh = 1, 1
    ar = float(ew) / eh
    tw = case["base_logo_mm"] * PDF_UNITS_PER_MM
    th = case["base_logo_mm"] * PDF_UNITS_PER_MM
    if ar < 1:
        tw *= ar
    else:
        th /= ar
    tws, ths = java_double_to_string(tw), java_double_to_string(th)
    return {
        "table.main.Style.padding": "0",
        "pos": f"w:{tws};f:0",
        "table.main.Style.imagescaletofit": f"{tws};{ths}",
    }, (ew, eh)


def variant_key(case):
    return (case["profile"], case["note_key"], case["emblem"], case["base_logo_mm"])


_VARIANT_LOCK = threading.Lock()


def make_variant(case, config_src: Path, inputs: Path, work: Path):
    with _VARIANT_LOCK:
        return _make_variant(case, config_src, inputs, work)


def _make_variant(case, config_src: Path, inputs: Path, work: Path):
    key = variant_key(case)
    h = hashlib.sha256(json.dumps(key).encode()).hexdigest()[:12]
    vdir = work / "cfg" / f"{case['profile']}-{h}"
    if vdir.exists():
        return vdir, json.loads((vdir / "overrides.json").read_text())
    tmp = vdir.with_suffix(".tmp")
    if tmp.exists():
        shutil.rmtree(tmp)
    tmp.mkdir(parents=True)
    shutil.copytree(config_src / "cfg", tmp / "cfg")
    shutil.copytree(config_src / "images", tmp / "images")
    os.symlink(config_src / "fonts", tmp / "fonts")
    overrides = {}
    if case["emblem"]:
        dst = tmp / "images" / case["emblem"]
        shutil.copyfile(inputs / case["emblem"], dst)
        overrides["value.SIG_LABEL"] = str(vdir / "images" / case["emblem"])
    if case["note"] is not None:
        overrides["value.SIG_NOTE"] = case["note"]
    if case["profile"] == "BASE_LOGO":
        o, _ = base_logo_overrides(case, inputs)
        overrides.update(o)
    if overrides:
        pf = tmp / "cfg" / "profiles" / f"{case['profile']}.properties"
        with open(pf, "a", encoding="latin-1") as f:
            f.write("\n# PDF-Over-Laufzeit-Overrides (Golden-Lauf, PdfAs4Signer.prepare)\n")
            for k, v in overrides.items():
                f.write(f"sig_obj.{case['profile']}.{k}={prop_escape(v)}\n")
    (tmp / "overrides.json").write_text(json.dumps(overrides, ensure_ascii=False, indent=1))
    tmp.rename(vdir)
    return vdir, overrides


# ------------------------------------------------------------------------------------------------
# Ausführung
# ------------------------------------------------------------------------------------------------


class Runner:
    def __init__(self, a):
        self.a = a
        self.dist = Path(a.dist)
        self.java = a.java
        self.java_home = str(Path(a.java).resolve().parent.parent)

    def sign(self, case, cfg: Path, inp: Path, out: Path, log: Path):
        if case["placeholder"]:
            cmd = [self.java, *JVM_OPTS, "-cp", str(self.dist / "lib") + "/*", str(Path(__file__).parent /
                   "PlaceholderSign.java"), str(cfg), case["profile"], case["position"] or "-", self.a.keystore,
                   self.a.alias, self.a.password, case.get("placeholder_id") or "1", str(inp), str(out)]
            env = dict(os.environ)
        else:
            cmd = [str(self.dist / "bin" / "pdf-as-cli"), "-m", "sign", "-c", "ks", "-ksf", self.a.keystore,
                   "-ksa", self.a.alias, "-kssp", self.a.password, "-kskp", self.a.password, "-kst", "PKCS12",
                   "-conf", str(cfg), "-p", case["profile"]]
            if case["position"] is not None:
                cmd += ["-pos", case["position"]]
            cmd += ["-o", str(out), str(inp)]
            env = dict(os.environ, JAVA_HOME=self.java_home, JAVA_OPTS=" ".join(JVM_OPTS))
        with open(log, "wb") as lf:
            r = subprocess.run(cmd, stdout=lf, stderr=subprocess.STDOUT, env=env, timeout=300)
        return r.returncode


def input_path(a, name):
    return Path(a.inputs) / f"{name}.pdf"


def page_count(pdf: Path):
    r = subprocess.run(["qpdf", "--show-npages", str(pdf)], capture_output=True, text=True)
    return int(r.stdout.strip()) if r.returncode in (0, 3) else None


def run_case(runner, case, a, work: Path, out_root: Path):
    cfg, overrides = make_variant(case, Path(a.config), Path(a.inputs), work)
    inp = input_path(a, case["input"])
    pdf = work / "out" / f"{case['id']}.pdf"
    log = work / "logs" / f"{case['id']}.log"
    t0 = time.time()
    if a.reextract and pdf.exists() and log.exists():
        rc = 0  # nur Manifest neu extrahieren (PDF und Log aus einem früheren Lauf)
    else:
        rc = runner.sign(case, cfg, inp, pdf, log)
    dt = time.time() - t0
    entry = dict(case)
    entry["overrides"] = overrides
    entry["seconds"] = round(dt, 1)
    case_dir = out_root / "cases" / case["id"]
    case_dir.mkdir(parents=True, exist_ok=True)
    if rc != 0 or not pdf.exists() or pdf.stat().st_size == 0:
        err = extract.parse_log(log)
        tail = [ln.strip()[:300] for ln in log.read_text(errors="replace").splitlines()
                if "Error" in ln or "Exception" in ln][:5]
        entry["status"] = "error"
        entry["error"] = {"exit": rc, "log": err, "lines": tail}
        (case_dir / "manifest.json").write_text(json.dumps({"case": entry}, ensure_ascii=False, indent=1,
                                                           sort_keys=True) + "\n")
        return entry
    if case["emblem"] and case["profile"] == "BASE_LOGO":
        _, entry["emblem_px"] = base_logo_overrides(case, Path(a.inputs))
    m = extract.extract(pdf, log=log, input_pages=page_count(inp), trust=a.trust, case=entry)
    entry["status"] = "ok"
    m["case"] = entry
    (case_dir / "manifest.json").write_text(json.dumps(m, ensure_ascii=False, separators=(",", ":"),
                                                       sort_keys=True) + "\n")
    return entry


def keep_pdf(case):
    """Signierte PDFs, die für den Pixelvergleich im Repository bleiben (Größe < 25 MB)."""
    if case["input"] == "a4" and case["position_key"] == "manual":
        return True
    if case["profile"] == "SIGNATURBLOCK_SMALL_DE" and case["input"] in ("a4-rot90", "a4-cropbox", "qr") \
            and case["position_key"] in ("manual", "placeholder"):
        return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dist", required=True)
    ap.add_argument("--java", required=True)
    ap.add_argument("--config", required=True, help="pdfas-config (Kopie bzw. Original, nur gelesen)")
    ap.add_argument("--keystore", required=True)
    ap.add_argument("--alias", default="test-rsa2048")
    ap.add_argument("--password", default="test")
    ap.add_argument("--trust", required=True)
    ap.add_argument("--inputs", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 4) // 2))
    ap.add_argument("--only", help="Regex auf Fall-IDs")
    ap.add_argument("--meta", help="JSON mit Werkzeugversionen/Prüfsummen für index.json")
    ap.add_argument("--reextract", action="store_true",
                    help="vorhandene Ausgaben (work/out, work/logs) nur neu extrahieren, nicht neu signieren")
    a = ap.parse_args()

    work, out_root = Path(a.work), Path(a.out)
    for d in ("out", "logs", "cfg"):
        (work / d).mkdir(parents=True, exist_ok=True)
    runner = Runner(a)

    # Stufe 0: vorsigniertes Dokument (PDF-AS signiert content-half mit SMALL_DE)
    pre = input_path(a, "presigned")
    if not pre.exists():
        seed = {"id": "presign", "profile": "SIGNATURBLOCK_SMALL_DE", "input": "content-half",
                "position": "p:1;x:300;y:200", "note": None, "note_key": None, "emblem": None,
                "base_logo_mm": None, "placeholder": False}
        cfg, _ = make_variant(seed, Path(a.config), Path(a.inputs), work)
        rc = runner.sign(seed, cfg, input_path(a, "content-half"), pre, work / "logs" / "presign.log")
        if rc != 0:
            sys.exit("Vorsignatur fehlgeschlagen, siehe " + str(work / "logs" / "presign.log"))

    cases = case_list()
    if a.only:
        rx = re.compile(a.only)
        cases = [c for c in cases if rx.search(c["id"])]
    print(f"{len(cases)} Fälle, {a.jobs} parallel", flush=True)
    results = []
    with cf.ThreadPoolExecutor(max_workers=a.jobs) as ex:
        futs = {ex.submit(run_case, runner, c, a, work, out_root): c for c in cases}
        for i, fut in enumerate(cf.as_completed(futs), 1):
            c = futs[fut]
            try:
                e = fut.result()
            except Exception as exc:  # Extraktionsfehler
                e = dict(c, status="error", error={"extract": f"{type(exc).__name__}: {exc}"})
                (out_root / "cases" / c["id"]).mkdir(parents=True, exist_ok=True)
                (out_root / "cases" / c["id"] / "manifest.json").write_text(
                    json.dumps({"case": e}, ensure_ascii=False, indent=1, sort_keys=True) + "\n")
            results.append(e)
            if e["status"] != "ok" or i % 25 == 0:
                print(f"[{i}/{len(cases)}] {e['id']}: {e['status']}", flush=True)

    # signierte PDFs behalten bzw. entfernen
    for e in results:
        dst = out_root / "cases" / e["id"] / "signed.pdf"
        src = work / "out" / f"{e['id']}.pdf"
        if e["status"] == "ok" and keep_pdf(e) and src.exists():
            shutil.copyfile(src, dst)
            e["pdf"] = f"cases/{e['id']}/signed.pdf"
        elif dst.exists():
            dst.unlink()

    # Index (bei --only mit vorhandenem Index zusammenführen)
    idx_path = out_root / "index.json"
    old = json.loads(idx_path.read_text()) if (a.only and idx_path.exists()) else {"cases": []}
    by_id = {c["id"]: c for c in old.get("cases", [])}
    for e in results:
        by_id[e["id"]] = {k: e.get(k) for k in ("id", "profile", "input", "position_key", "position", "note_key",
                                               "emblem", "base_logo_mm", "placeholder", "placeholder_id", "status",
                                               "pdf")}
    meta = json.loads(Path(a.meta).read_text()) if a.meta else old.get("meta", {})
    index = {
        "meta": meta,
        "notes": NOTES,
        "positions": POSITIONS,
        "timezone": TIMEZONE,
        "cases": [by_id[k] for k in sorted(by_id)],
    }
    idx_path.write_text(json.dumps(index, ensure_ascii=False, indent=1, sort_keys=True) + "\n")
    bad = [e["id"] for e in results if e["status"] != "ok"]
    print(f"fertig: {len(results) - len(bad)} ok, {len(bad)} Fehler", flush=True)
    for b in bad:
        print("  Fehler:", b)


if __name__ == "__main__":
    main()
