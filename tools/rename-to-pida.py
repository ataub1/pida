#!/usr/bin/env python3
"""Umbenennung „PDF-Over NG“ → **PIDA** (PDF · ID · Austria), ADR 0002 – wiederholbar und idempotent.

Nur Python-Standardbibliothek (≥ 3.9). Bearbeitet ausschließlich Git-verfolgte Textdateien
(Binärdateien, Symlinks und Submodule bleiben unberührt) und benennt Pfade mit `git mv` um.

Aufruf im Repository-Wurzelordner (oder mit `--root <ordner>`):

    python3 tools/rename-to-pida.py               # anwenden: Inhalte, Pfade (git mv), Hinweis-Kästen
    python3 tools/rename-to-pida.py --dry-run     # nur anzeigen (mit -v: geänderte Zeilen)
    python3 tools/rename-to-pida.py --check       # Exit 1, solange Altnamen außerhalb der Ausnahmen übrig sind
    python3 tools/rename-to-pida.py --regenerate  # danach: Cargo.lock, rustfmt, package-lock.json, Bindings, Fixtures
    python3 tools/rename-to-pida.py --rules       # Mapping-Tabelle, Ausnahmen und Schutzregeln ausgeben
    python3 tools/rename-to-pida.py --self-test   # synthetisches Fixture (inkl. Mobile-Dateien) prüfen

Ablauf je Datei: (1) feste Formulierungen (Stufe „phrase“), (2) geschützte Stellen maskieren
(Original-PDF-Over-Bezüge, Testzertifikats-Namen, Pragma `rename-to-pida:keep` …), (3) allgemeine
Regeln in der Reihenfolge der Tabelle (längste/spezifischste Muster zuerst), (4) Maskierung aufheben,
(5) Hinweis-Kästen einfügen (nur wenn ihre Markierung fehlt). Ein zweiter Lauf ändert nichts.

Nicht versionierte bzw. ignorierte Dateien (z. B. Build-Ausgaben in umbenannten Ordnern wie
gen/android/…/at/pdf_over_ng/generated) bearbeitet und löscht das Skript nie; Lauf und --check melden sie
als „Hinweis“ (ohne Einfluss auf den Exit-Code).
"""

from __future__ import annotations

import argparse
import difflib
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

# ------------------------------------------------------------------------------------------------
# Mapping-Tabelle (geordnet; spezifische und längere Muster vor allgemeinen)
# ------------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class Rule:
    """Eine Ersetzung. `stage="phrase"` läuft vor dem Maskieren (feste, eindeutige Formulierungen)."""

    id: str
    pattern: str
    repl: str
    desc: str
    files: Tuple[str, ...] = ()  # nur in diesen Pfaden (Globs mit **); leer = überall
    content: bool = True
    path: bool = True
    stage: str = "general"  # "phrase" | "general"

    def regex(self) -> "re.Pattern[str]":
        return _compile(self.pattern)


RULES: Tuple[Rule, ...] = (
    # --- 1. Feste Formulierungen (Untertitel, Sätze, deren Sinn sich mit dem Namen ändert) -------
    Rule("subtitle-de", r"PDF-Over NG – PDF-Dokumente elektronisch signieren", "PIDA – PDF-Signatur mit ID Austria",
         "Untertitel DE (CLI-Hilfe, Kataloge)", path=False, stage="phrase"),
    Rule("subtitle-en", r"PDF-Over NG – sign PDF documents electronically", "PIDA – PDF signing with ID Austria",
         "Untertitel EN (CLI-Hilfe, Kataloge)", path=False, stage="phrase"),
    Rule("tauri-short-description", r'"shortDescription": "PDF-Dokumente elektronisch signieren"',
         '"shortDescription": "PIDA – PDF-Signatur mit ID Austria"', "Bundle-Kurzbeschreibung (Tauri)",
         files=("apps/app/src-tauri/tauri.conf.json",), path=False, stage="phrase"),
    Rule("tauri-long-description", r'"longDescription": "PDF-Over NG signiert',
         '"longDescription": "PIDA (PDF · ID · Austria) signiert', "Bundle-Langbeschreibung (Tauri)",
         files=("apps/app/src-tauri/tauri.conf.json",), path=False, stage="phrase"),
    Rule("paths-test", r's\.contains\("pdf-over-ng"\) \|\| s\.contains\("pdf-over ng"\)', 's.contains("pida")',
         "Plattformpfad-Test (beide Schreibweisen fallen zu „pida“ zusammen)",
         files=("crates/config/src/paths.rs",), path=False, stage="phrase"),
    Rule("android-package-comment", r"Kennung at\.pdf-over-ng: „-“ ist im Android-Paketnamen unzulässig → "
         r"at\.pdf_over_ng \(Tauri\)\.", "Kennung at.pida = Android-Paket (Tauri; ohne „-“, das dort unzulässig wäre).",
         "Kommentar in gen/android (Mobile-Welle)", path=False, stage="phrase"),
    Rule("cli-version-name", r'#\[command\(name = "(?:pdfover|pida)-cli", version, ',
         '#[command(name = "pida-cli", display_name = "PIDA", version, ', "pida-cli --version zeigt „PIDA <Version>“",
         files=("apps/cli/src/lib.rs",), path=False, stage="phrase"),
    Rule("cli-version-test", r'(    assert!\(stdout\(&o\)\.contains\(env!\("CARGO_PKG_VERSION"\)\)\);\n)\}',
         '\\1    assert!(stdout(&o).starts_with("PIDA "), "{}", stdout(&o));\n}', "Test dazu",
         files=("apps/cli/tests/cli.rs",), path=False, stage="phrase"),
    Rule("readme-title", r"\A# PDF-Over NG\n", "# PIDA\n\n**PIDA – PDF-Signatur mit ID Austria** (PDF · ID · Austria)\n",
         "README: Titel und Untertitel", files=("README.md",), path=False, stage="phrase"),
    Rule("readme-working-title", r"Arbeitstitel „PDF-Over NG“, Lizenz",
         "Name **PIDA**\n(bis 15.10.2026 Arbeitstitel „PDF-Over NG“, [ADR 0002](docs/adr/0002-name-pida.md)), Lizenz",
         "README: Name statt Arbeitstitel", files=("README.md",), path=False, stage="phrase"),
    Rule("adr0001-working-title", r"Arbeitstitel „PDF-Over NG“\.",
         "Arbeitstitel „PDF-Over NG“, seit 15.10.2026 **PIDA** ([ADR 0002](0002-name-pida.md)).",
         "ADR 0001: Verweis auf ADR 0002", files=("docs/adr/0001-workspace-und-schnittstellen.md",), path=False,
         stage="phrase"),
    # --- 2. Produkt-/Anzeigename -----------------------------------------------------------------
    Rule("display", "PDF-Over[ \u00a0]NG", "PIDA", "Anzeigename, Fenstertitel, User-Agent „PIDA v…“, Keychain-Dienst, "
         "Windows-Ordner, PIDA.app"),
    Rule("display-hyphen", r"PDF-Over-NG", "PIDA", "Anzeigename mit Bindestrich"),
    Rule("display-camel", r"(?:PDF|Pdf)Over(?:NG|Ng)", "PIDA", "Multipart-Boundary, Android-Log-Tags (PdfOverNg/…)"),
    # --- 3. Kennungen mit „NG“ ---------------------------------------------------------------------
    Rule("id-pdf-over-ng", r"pdf-over-ng", "pida", "Bundle-ID at.pida, GUI-Binary pida, Ordner/Log pida, "
         "GitHub ataub1/pida, /etc/pida"),
    Rule("id-pdf_over_ng", r"pdf_over_ng", "pida", "Android-Paket at.pida, Single-Instance-Socket at_pida_si.sock"),
    Rule("id-pdf-overng", r"pdf-overng", "pida", "Schreibweise von `directories`"),
    Rule("id-pdfover-ng", r"pdfover-ng", "pida", "Katalog pida.ftl, npm-Paket pida-app, localStorage pida.*"),
    Rule("id-pdfover_ng", r"pdfover_ng", "pida", "snake_case"),
    Rule("id-pdfoverng", r"pdfoverng", "pida", "Kotlin-Pakete at.pdfoverng.* → at.pida.*"),
    Rule("id-PDFOVER_NG", r"PDFOVER_NG", "PIDA", "Umgebungsvariable mit NG"),
    # --- 4. Präfixe: Crates, Rust-Pfade, Umgebungsvariablen, Klassen, Protokoll, Ereignisse ----------
    Rule("env", r"PDFOVER_", "PIDA_", "Umgebungs-/Shell-Variablen PDFOVER_* → PIDA_*"),
    Rule("camel", r"Pdfover", "Pida", "Typen/Klassen (PdfoverMobilePlugin → PidaMobilePlugin)"),
    Rule("kebab", r"pdfover-", "pida-", "Crates pida-<x>, pida-cli, pida-app, pida-mobile, pida-render://, "
         "pida-smoke:, Temp-Namen"),
    Rule("snake", r"pdfover_", "pida_", "Rust-Pfade pida_<x>, pida_app_lib, init_plugin_pida_mobile"),
    Rule("event", r"pdfover:", "pida:", "Tauri-Ereignisse pida:view, pida:emblem-drop"),
    Rule("log-filter", r"pdfover=", "pida=", "Log-Filter pida=debug"),
    Rule("dot", r"pdfover\.(?!(?:ftl|gui|commons|signer)\b)(?=[A-Za-z])", "pida.",
         "Punkt-Kennungen (Mobile: FileProvider ${applicationId}.pida.files, Intent-Extra pida.smoke, "
         "Gradle-Property pida.pdfiumJniLibs); nicht pdfover.ftl und Java-Pakete des Originals"),
    # --- 5. Kurzform „NG“ (NG-Ergänzung, NG-Standard …) ---------------------------------------------
    Rule("short-ng", r"(?<![\w./+@-])NG(?![\w./+@=])", "PIDA", "Kurzform in Texten (NG-Ergänzung → PIDA-Ergänzung)",
         path=False),
)

# ------------------------------------------------------------------------------------------------
# Ausnahmen
# ------------------------------------------------------------------------------------------------

# Nie gelesen, geändert oder geprüft (historische Dokumente, Testdaten-Inhalte, dieses Skript).
SKIP: Tuple[Tuple[str, str], ...] = (
    ("reference/**", "Originalquellen PDF-Over/PDF-AS (nur lesen)"),
    ("docs/analyse/**", "historische Analyse des Originals"),
    ("docs/recherche/**", "historische Recherchen"),
    ("docs/00-architektur-und-umsetzungsplan.md", "historischer Plan (nur Hinweis-Kasten, siehe INSERTS)"),
    ("docs/adr/0002-name-pida.md", "ADR der Umbenennung (enthält die Altnamen absichtlich)"),
    ("tools/rename-to-pida.py", "dieses Skript"),
    ("testdata/**", "Testdaten-Inhalte (PDFs, Golden-Manifeste, Altkonfigurationen, Zertifikate) – "
                    "Ausnahmen siehe TESTDATA_PROCESS"),
    ("**/*.pdf", "PDF-Dateien (auch reine ASCII-PDFs)"),
)

# Innerhalb von testdata/ werden nur Dokumentation, Generatoren und die Meldungs-ID-Kopie bearbeitet
# (Pfad- und Variablenangaben, die sonst brechen; Zertifikatsnamen schützt PROTECT).
TESTDATA_PROCESS: Tuple[str, ...] = (
    "testdata/**/README.md",
    "testdata/README.md",
    "testdata/**/RESULTS.md",
    "testdata/**/*.sh",
    "testdata/**/*.py",
    "testdata/**/*.java",
    "testdata/i18n/wave1-message-ids.json",  # Kopie von docs/integration/… (Test vergleicht beide)
)

# Nicht textuell umgeschrieben, sondern neu erzeugt (--regenerate); --check prüft sie trotzdem.
DERIVED: Tuple[Tuple[str, str], ...] = (
    ("Cargo.lock", "cargo update --workspace"),
    ("apps/app/package-lock.json", "npm install --package-lock-only"),
)


@dataclass(frozen=True)
class Protect:
    id: str
    pattern: str
    desc: str


# Geschützte Stellen (vor den allgemeinen Regeln maskiert, von --check nicht gemeldet).
PROTECT: Tuple[Protect, ...] = (
    Protect("pragma", r"[^\n]*rename-to-pida:keep[^\n]*", "Zeilen mit dem Pragma `rename-to-pida:keep`"),
    Protect("asit-java", r"at[./]asit[./]pdfover[\w./-]*", "Java-Pakete/Pfade des Originals (at.asit.pdfover.*)"),
    Protect("test-subjects", r"PDF-Over NG (?:Test(?:daten|inhalt)? |NUR TEST)",
            "Namen der Testzertifikate/-inhalte („PDF-Over NG Test RSA (NUR TEST)“) – stehen so in den "
            "binären Testdaten und Golden-Manifesten"),
    Protect("working-title", r"Arbeitstitel[^\n]{0,40}?PDF-Over NG", "historischer Hinweis auf den Arbeitstitel"),
    Protect("legacy-messages", r"\w*pdfover_messages\w*", "Konverter der PDF-Over-Meldungen (convert_pdfover_messages)"),
    Protect("legacy-header", r"\w*pdfover_header\w*", "Kopf des erzeugten PDF-Over-Katalogs"),
    Protect("legacy-profiles", r"\w*pdfover_profile\w*", "Tests über die PDF-Over-Profile"),
    Protect("legacy-consts", r"PDFOVER_(?:PROFILE_IDS|UNUSED_KEYS)\w*",
            "Konstanten mit den Profil-IDs bzw. ungenutzten Schlüsseln von PDF-Over"),
    Protect("fixture-path", r"pdfover-nicht-vorhanden", "Pfad aus testdata/config/special-cases.config"),
)

# Pfadbestandteile, die nie umbenannt werden.
PATH_PROTECT: Tuple[Protect, ...] = (
    Protect("legacy-dir", r"\.pdf-over(?=/|$)", "Altordner ~/.pdf-over (Testdaten legacy-home)"),
    Protect("legacy-fixtures", r"pdf-over-generated[\w.-]*", "von PDF-Over erzeugte Altkonfigurationen"),
    Protect("legacy-catalog", r"pdfover\.ftl", "aus PDF-Over erzeugter Katalog (pdfover.ftl)"),
    Protect("golden-case", r"__pdfover(?=/|$)", "Golden-Fall „Position wie PDF-Over“"),
    Protect("asit-path", r"at/asit/pdfover", "Java-Pfade des Originals"),
)

# Bewusst bleibende Original-Bezüge, die keine Regel trifft (nur für --check).
ALLOWED: Tuple[Protect, ...] = (
    Protect("legacy-catalog", r"pdfover\.ftl", "Katalog mit den Meldungen aus PDF-Over"),
    Protect("legacy-java", r"pdfover\.(?:gui|commons|signer)\b", "Java-Pakete des Originals"),
    Protect("golden-case", r"__pdfover\b", "Golden-Fall-IDs (Position wie PDF-Over)"),
    Protect("position-key", r"[\"'`]pdfover[\"'`]", "Positionsschlüssel „pdfover“ der Golden-Fälle"),
    Protect("legacy-storage", r"\b(?:from|as)_pdfover\b", "Umrechnung der PDF-Over-Passwortspeicherarten"),
    Protect("legacy-tests", r"\w*_pdf_over\w*", "Testnamen „…_like_pdf_over“"),
)

# Restsuche für --check (Altnamen, die keine Regel erfasst hätte).
DETECT: Tuple[Protect, ...] = (
    Protect("old-name", "(?i)pdf[\\s\u00a0_\\-\u2011]?over[\\s\u00a0_\\-\u2011]?ng(?![a-z])", "„PDF-Over NG“ in jeder Schreibweise"),
    Protect("old-token", r"(?i)pdf_?over", "pdfover/PDFOVER/Pdfover/pdf_over"),
)


@dataclass(frozen=True)
class Insert:
    """Idempotenter Einschub: steht `marker` nicht in der Datei, kommt `text` nach der ersten Ankerzeile."""

    id: str
    file: str
    anchor: str  # Regex für die Zeile, nach der eingefügt wird
    marker: str
    text: str
    desc: str


PLAN_NOTICE = (
    "\n"
    "> **Hinweis (15.10.2026): Arbeitstitel PDF-Over NG heißt seit 15.10.2026 PIDA** (PDF · ID · Austria),\n"
    "> siehe [ADR 0002](adr/0002-name-pida.md). Dieser Plan ist ein historisches Dokument und bleibt sonst\n"
    "> unverändert: Namen wie `pdfover-<crate>`, `pdfover-cli`, `pdf-over-ng`, `at.pdf-over-ng` und\n"
    "> `PDFOVER_*` lauten heute `pida-<crate>`, `pida-cli`, `pida`, `at.pida` und `PIDA_*`.\n"
)

_ENV_ADOPT = ("PDFIUM_DIR", "REFERENCE", "HOME", "PYTHON", "JAVA", "VERAPDF_JAR", "PDFAS_LIB", "NOTICES_DIR",
              "GOLDEN_CACHE", "LOG")
ENV_TRANSITION = (
    "\n"
    "# Übergang zu PIDA (ADR 0002): Werte früherer Variablen aus der aufrufenden Umgebung übernehmen, sofern\n"
    "# die neue Variable nicht gesetzt ist, und alle alten entfernen – danach gibt es nur noch PIDA_*.\n"
    + "".join(
        '[ -n "${PIDA_%s:-}" ] || [ -z "${PDFOVER_%s:-}" ] || export PIDA_%s="$PDFOVER_%s"  # rename-to-pida:keep\n'
        % (v, v, v, v) for v in _ENV_ADOPT)
    + "if env | grep -q '^PDFOVER_'; then  # rename-to-pida:keep\n"
    "  # shellcheck disable=SC2046 # Wortaufteilung gewollt: ein Variablenname je Wort\n"
    "  unset $(env | sed -n 's/^\\(PDFOVER_[A-Z0-9_]*\\)=.*/\\1/p')  # rename-to-pida:keep\n"
    "fi\n"
    "\n"
)

DEV_NOTICE = (
    "\n"
    "> Name: **PIDA** (PDF · ID · Austria), bis 15.10.2026 Arbeitstitel „PDF-Over NG“ – siehe\n"
    "> [ADR 0002](adr/0002-name-pida.md). Die Umbenennung ist das Skript `tools/rename-to-pida.py`\n"
    "> (`--dry-run`, `--check`, `--regenerate`); nach Zusammenführungen mit Ständen unter dem alten Namen\n"
    "> erneut ausführen (ADR 0002 §5).\n"
)

INSERTS: Tuple[Insert, ...] = (
    Insert("plan-notice", "docs/00-architektur-und-umsetzungsplan.md", r"^# .*$", "heißt seit 15.10.2026 PIDA",
           PLAN_NOTICE, "Hinweis-Kasten oben im historischen Plan"),
    Insert("env-transition", "tools/env.sh", r"^export PATH=.*$", "# Übergang zu PIDA (ADR 0002)", ENV_TRANSITION,
           "Übergang alter PDFOVER_*-Variablen in tools/env.sh"),
    Insert("dev-notice", "docs/entwicklung.md", r"^# .*$", "[ADR 0002](adr/0002-name-pida.md). Die Umbenennung",
           DEV_NOTICE, "Hinweis in docs/entwicklung.md"),
)

# Abgeleitete Dateien neu erzeugen (--regenerate). Reihenfolge ist wichtig (Lockfile vor den Tests).
REGENERATE: Tuple[Tuple[str, Sequence[str], str, Dict[str, str]], ...] = (
    ("Cargo.lock", ("cargo", "update", "--workspace"), ".", {}),
    # kürzere Namen (pida_* statt pdfover_*) lassen umbrochene Zeilen in die Breite 120 passen
    ("rustfmt", ("cargo", "fmt", "--all"), ".", {}),
    ("package-lock.json", ("npm", "install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"),
     "apps/app", {}),
    ("Fluent-Kataloge pdfover.ftl", ("cargo", "test", "-p", "pida-i18n", "--test", "catalogs"), ".",
     {"PIDA_I18N_BLESS": "1"}),
    ("generated.ts", ("cargo", "test", "-p", "pida-core-api", "--features", "ts", "--test", "ts_bindings"), ".",
     {"PIDA_BLESS": "1"}),
    ("view-states.json", ("cargo", "test", "-p", "pida-app", "--test", "serde_contract"), ".", {"PIDA_BLESS": "1"}),
    ("ui-events.json (braucht npm ci)", ("npx", "vitest", "run", "src/core/contract.test.ts"), "apps/app",
     {"PIDA_BLESS": "1"}),
)

# ------------------------------------------------------------------------------------------------
# Hilfen
# ------------------------------------------------------------------------------------------------

_RE_CACHE: Dict[str, "re.Pattern[str]"] = {}


def _compile(pattern: str) -> "re.Pattern[str]":
    rx = _RE_CACHE.get(pattern)
    if rx is None:
        rx = re.compile(pattern, re.MULTILINE)
        _RE_CACHE[pattern] = rx
    return rx


def glob_regex(glob: str) -> "re.Pattern[str]":
    """Glob mit `**` (beliebig viele Ordner), `*` (ohne /) und `?` als Regex über den ganzen Pfad."""
    key = "glob:" + glob
    rx = _RE_CACHE.get(key)
    if rx is not None:
        return rx
    out = []
    i = 0
    while i < len(glob):
        c = glob[i]
        if glob.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif glob.startswith("**", i):
            out.append(".*")
            i += 2
        elif c == "*":
            out.append("[^/]*")
            i += 1
        elif c == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(c))
            i += 1
    rx = re.compile("^" + "".join(out) + "$")
    _RE_CACHE[key] = rx
    return rx


def matches_any(path: str, globs: Iterable[str]) -> bool:
    return any(glob_regex(g).match(path) for g in globs)


def is_skipped(path: str) -> bool:
    if path.startswith("testdata/") and matches_any(path, TESTDATA_PROCESS) and not path.endswith(".pdf"):
        return False
    return matches_any(path, (g for g, _ in SKIP))


def is_derived(path: str) -> bool:
    return any(path == g for g, _ in DERIVED)


def read_text(path: Path) -> Optional[str]:
    """Inhalt als Text oder None (binär: NUL-Byte oder kein UTF-8)."""
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if b"\0" in data:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


class Masker:
    """Ersetzt geschützte Stellen durch Platzhalter (NUL-Bytes kommen in Textdateien nicht vor)."""

    def __init__(self, text: str, protections: Iterable[Protect]):
        self.saved: List[str] = []
        spans: List[Tuple[int, int]] = []
        for p in protections:
            for m in _compile(p.pattern).finditer(text):
                if m.end() > m.start():
                    spans.append((m.start(), m.end()))
        spans.sort()
        merged: List[Tuple[int, int]] = []
        for s, e in spans:
            if merged and s <= merged[-1][1]:
                merged[-1] = (merged[-1][0], max(merged[-1][1], e))
            else:
                merged.append((s, e))
        out = []
        pos = 0
        for s, e in merged:
            out.append(text[pos:s])
            out.append("\0%d\0" % len(self.saved))
            self.saved.append(text[s:e])
            pos = e
        out.append(text[pos:])
        self.text = "".join(out)

    def restore(self, text: str) -> str:
        return re.sub("\0(\\d+)\0", lambda m: self.saved[int(m.group(1))], text)


def rules_for(path: str, kind: str, stage: str) -> List[Rule]:
    out = []
    for r in RULES:
        if r.stage != stage:
            continue
        if kind == "content" and not r.content:
            continue
        if kind == "path" and not r.path:
            continue
        if r.files and not matches_any(path, r.files):
            continue
        out.append(r)
    return out


def transform_text(path: str, text: str) -> Tuple[str, Dict[str, int]]:
    """Wendet Phrasen, Maskierung und Regeln an; liefert neuen Text und Trefferzahlen je Regel."""
    counts: Dict[str, int] = {}
    for r in rules_for(path, "content", "phrase"):
        text, n = r.regex().subn(r.repl, text)
        if n:
            counts[r.id] = counts.get(r.id, 0) + n
    masker = Masker(text, PROTECT)
    work = masker.text
    for r in rules_for(path, "content", "general"):
        work, n = r.regex().subn(r.repl, work)
        if n:
            counts[r.id] = counts.get(r.id, 0) + n
    return masker.restore(work), counts


def apply_inserts(path: str, text: str) -> Tuple[str, List[str]]:
    applied = []
    for ins in INSERTS:
        if ins.file != path or ins.marker in text:
            continue
        m = _compile(ins.anchor).search(text)
        if not m:
            continue
        end = m.end()
        if end < len(text) and text[end] == "\n":
            end += 1
        block = ins.text if ins.text.endswith("\n") else ins.text + "\n"
        text = text[:end] + block + text[end:]
        applied.append(ins.id)
    return text, applied


def transform_path(path: str) -> str:
    masker = Masker(path, PATH_PROTECT)
    work = masker.text
    for r in rules_for(path, "path", "phrase") + rules_for(path, "path", "general"):
        work = r.regex().sub(r.repl, work)
    return masker.restore(work)


def residual_hits(text: str) -> List[Tuple[int, str]]:
    """Altnamen, die nach Maskierung und Erlaubnisliste übrig bleiben: (Zeile, Ausschnitt)."""
    masker = Masker(text, PROTECT + ALLOWED)
    work = masker.text
    hits = []
    for d in DETECT:
        for m in _compile(d.pattern).finditer(work):
            line_no = work.count("\n", 0, m.start()) + 1
            line_start = work.rfind("\n", 0, m.start()) + 1
            line_end = work.find("\n", m.end())
            line = masker.restore(work[line_start: line_end if line_end >= 0 else len(work)])
            hits.append((line_no, line.strip()))
    hits.sort()
    # pro Zeile nur einmal melden
    seen = set()
    unique = []
    for h in hits:
        if h[0] not in seen:
            seen.add(h[0])
            unique.append(h)
    return unique


# ------------------------------------------------------------------------------------------------
# Repository
# ------------------------------------------------------------------------------------------------


def git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], check=check, capture_output=True)


def tracked_entries(root: Path) -> List[Tuple[str, str]]:
    """Alle Git-verfolgten Einträge als (Modus, Pfad), relativ zu `root`."""
    out = git(root, "ls-files", "-z", "--stage").stdout.decode("utf-8", "surrogateescape")
    entries = []
    for entry in out.split("\0"):
        if not entry:
            continue
        meta, _, name = entry.partition("\t")
        entries.append((meta.split(" ", 1)[0], name))
    return entries


def tracked_files(root: Path) -> List[str]:
    """Git-verfolgte reguläre Dateien (ohne Symlinks/Submodule), relativ zu `root`."""
    return sorted({name for mode, name in tracked_entries(root) if mode in ("100644", "100755")})


@dataclass
class Plan:
    content: Dict[str, Tuple[str, str, Dict[str, int], List[str]]] = field(default_factory=dict)
    renames: List[Tuple[str, str]] = field(default_factory=list)
    residual: List[Tuple[str, int, str]] = field(default_factory=list)
    derived_stale: List[str] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)


def build_plan(root: Path, with_residual: bool = False) -> Plan:
    plan = Plan()
    files = tracked_files(root)
    tracked = set(files)
    targets: Dict[str, str] = {}
    for rel in files:
        skipped = is_skipped(rel)
        has_insert = any(ins.file == rel for ins in INSERTS)
        if skipped and not has_insert:
            continue
        text = read_text(root / rel)
        if text is None:
            continue
        if is_derived(rel):
            if with_residual:
                hits = residual_hits(text)
                if hits:
                    plan.derived_stale.append(rel)
                    plan.residual.extend((rel, n, s) for n, s in hits[:3])
            continue
        new, counts = (text, {}) if skipped else transform_text(rel, text)
        new, inserted = apply_inserts(rel, new)
        if new != text:
            plan.content[rel] = (text, new, counts, inserted)
        if with_residual and not skipped:
            plan.residual.extend((rel, n, s) for n, s in residual_hits(new))
        if not skipped:
            dst = transform_path(rel)
            if dst != rel:
                if dst in tracked or dst in targets.values():
                    plan.errors.append("Ziel existiert bereits: %s -> %s" % (rel, dst))
                elif (root / dst).exists() and not _same_file(root / rel, root / dst):
                    plan.errors.append("Ziel liegt (unverfolgt) im Weg: %s -> %s" % (rel, dst))
                else:
                    targets[rel] = dst
    plan.renames = sorted(targets.items())
    if with_residual:
        for rel in files:
            if is_skipped(rel) or rel in targets:
                continue
            masker = Masker(rel, PATH_PROTECT + ALLOWED)
            if any(_compile(d.pattern).search(masker.text) for d in DETECT):
                plan.residual.append((rel, 0, "Pfad enthält einen Altnamen"))
        # Symlinks und Submodule bearbeitet das Skript nicht – mit Altnamen von Hand prüfen
        for mode, rel in tracked_entries(root):
            if mode not in ("100644", "100755") and not is_skipped(rel):
                masker = Masker(rel, PATH_PROTECT + ALLOWED)
                if any(_compile(d.pattern).search(masker.text) for d in DETECT):
                    plan.residual.append((rel, 0, "Symlink/Submodul (Modus %s) mit Altnamen – von Hand umbenennen" % mode))
    return plan


def _same_file(a: Path, b: Path) -> bool:
    try:
        return a.samefile(b)  # Groß-/Kleinschreibung auf case-insensitiven Dateisystemen
    except OSError:
        return False


def execute(root: Path, plan: Plan) -> List[str]:
    """Wendet den Plan an; liefert Altordner, die danach noch (unverfolgte/ignorierte) Dateien enthalten."""
    for rel, (_, new, _, _) in sorted(plan.content.items()):
        p = root / rel
        mode = p.stat().st_mode
        p.write_bytes(new.encode("utf-8"))
        os.chmod(p, mode)
    for src, dst in plan.renames:
        (root / dst).parent.mkdir(parents=True, exist_ok=True)
        git(root, "mv", "--", src, dst)
    # leere Altordner entfernen
    for src, _ in plan.renames:
        d = (root / src).parent
        while d != root and d.is_dir() and not any(d.iterdir()):
            d.rmdir()
            d = d.parent
    # Altordner, in denen nur noch Unverfolgtes liegt (Build-Ausgaben wie gen/android/…/at/pdf_over_ng/generated,
    # jniLibs/…/libpdfover_app_lib.so, plugins/pdfover-mobile/.tauri): nicht löschen, nur melden.
    left = set()
    for src, _ in plan.renames:
        old = renamed_ancestor(src)
        if old and (root / old).is_dir():
            left.add(old)
    return sorted(left)


def renamed_ancestor(rel: str) -> Optional[str]:
    """Oberster Ordner von `rel`, dessen Name sich durch die Umbenennung ändert (sonst None)."""
    parts = rel.split("/")[:-1]
    for i in range(1, len(parts) + 1):
        d = "/".join(parts[:i])
        if transform_path(d) != d:
            return d
    return None


# Beim Durchsuchen der Arbeitskopie nach Altordnern übersprungen (groß bzw. nie umbenannt).
# `dist/` enthält veröffentlichte bzw. heruntergeladene Bundles (Dateinamen der damaligen Version, bleiben).
_WALK_PRUNE = {".git", ".claude", "node_modules", "target", ".venv", "build", ".gradle", "reference", "Pods",
               "DerivedData", "dist"}


def stale_paths(root: Path, tracked: Iterable[str]) -> List[Tuple[str, int]]:
    """Unverfolgte bzw. ignorierte Ordner und Dateien mit Altnamen in der Arbeitskopie (z. B. liegen gebliebene
    Build-Ausgaben wie gen/android/…/at/pdf_over_ng/generated oder jniLibs/…/libpdfover_app_lib.so):
    (Pfad, Anzahl Dateien) – Ordner mit „/“ am Ende, Dateien mit Anzahl 1."""
    tracked = set(tracked)
    tracked_dirs = set()
    for rel in tracked:
        parts = rel.split("/")[:-1]
        for i in range(1, len(parts) + 1):
            tracked_dirs.add("/".join(parts[:i]))
    found: List[Tuple[str, int]] = []
    for dirpath, dirnames, filenames in os.walk(root):
        base = Path(dirpath).relative_to(root).as_posix()
        base = "" if base == "." else base
        keep = []
        for name in sorted(dirnames):
            rel = (base + "/" + name) if base else name
            if (name in _WALK_PRUNE or is_skipped(rel + "/x") or (root / rel).is_symlink()
                    or (root / rel / ".git").exists()):  # andere Repositories/Worktrees
                continue
            if rel not in tracked_dirs and _old_named(name):
                count = sum(len(fs) for _, _, fs in os.walk(root / rel))
                found.append((rel + "/", count))
                continue
            keep.append(name)
        dirnames[:] = keep
        for name in sorted(filenames):
            rel = (base + "/" + name) if base else name
            if rel not in tracked and not is_skipped(rel) and _old_named(name):
                found.append((rel, 1))
    return found


def _old_named(component: str) -> bool:
    """Enthält ein einzelner Pfadbestandteil einen Altnamen (nach Pfad-Schutz- und Erlaubnisliste)?"""
    masked = Masker(component, PATH_PROTECT + ALLOWED).text
    return any(_compile(d.pattern).search(masked) for d in DETECT)


def untracked_hits(root: Path, known: Sequence[str] = ()) -> List[Tuple[str, int, str]]:
    """Nicht versionierte, nicht ignorierte Dateien mit Altnamen (bearbeitet das Skript nicht); Dateien in
    Ordnern mit Altnamen werden je Ordner zusammengefasst, bereits gemeldete Ordner/Dateien (`known`) entfallen."""
    out = git(root, "ls-files", "-z", "--others", "--exclude-standard", check=False).stdout
    hits: List[Tuple[str, int, str]] = []
    dirs: Dict[str, int] = {}
    for rel in sorted(set(out.decode("utf-8", "surrogateescape").split("\0"))):
        if not rel or is_skipped(rel):
            continue
        parts = rel.split("/")
        top = next(("/".join(parts[:i + 1]) for i in range(len(parts) - 1) if _old_named(parts[i])), None)
        if top is not None:
            if not any(top == k or top.startswith(k + "/") for k in known):
                dirs[top] = dirs.get(top, 0) + 1
            continue
        if _old_named(parts[-1]):
            if rel not in known:
                hits.append((rel, 0, "Dateiname enthält einen Altnamen"))
            continue
        text = read_text(root / rel)
        if text is not None:
            hits.extend((rel, n, s) for n, s in residual_hits(text)[:1])
    hits.extend((d + "/", 0, "%d Datei(en) in einem Ordner mit Altnamen" % n) for d, n in sorted(dirs.items()))
    return hits


def print_local_hints(left: Sequence[str], untracked: Sequence[Tuple[str, int, str]],
                      stale: Sequence[Tuple[str, int]] = ()) -> None:
    """Hinweise (kein Fehler, Exit-Code unverändert) zu Dingen außerhalb der Versionsverwaltung."""
    limit = 25
    if len(stale) > limit:
        print("  Hinweis: %d weitere nicht versionierte Pfade mit Altnamen (nur die ersten %d gezeigt)"
              % (len(stale) - limit, limit))
        stale = stale[:limit]
    if len(untracked) > limit:
        print("  Hinweis: %d weitere nicht versionierte Dateien mit Altnamen (nur die ersten %d gezeigt)"
              % (len(untracked) - limit, limit))
        untracked = untracked[:limit]
    for d in left:
        print("  Hinweis: %s/ enthält nach der Umbenennung noch unverfolgte bzw. ignorierte Dateien (Build-Ausgaben?)"
              " – prüfen und ggf. löschen; Tauri erzeugt gen/android- und gen/apple-Ausgaben neu." % d)
    for d, n in stale:
        what = ("Ordner mit Altnamen ohne versionierte Dateien: %s (%d Datei(en))" % (d, n) if d.endswith("/")
                else "nicht versionierte Datei mit Altnamen: " + d)
        print("  Hinweis: %s – prüfen und ggf. löschen (alte Build-Ausgaben? unter gen/android baut Gradle sonst "
              "veraltete Kotlin-Quellen mit, jniLibs landen im APK)." % what)
    for rel, n, s in untracked:
        where = "%s:%d" % (rel, n) if n else rel
        print("  Hinweis: nicht versioniert (vom Skript nicht bearbeitet, erst `git add`): %s: %s" % (where, s[:120]))


def print_plan(plan: Plan, verbose: bool, out=sys.stdout) -> None:
    for rel, (old, new, counts, inserted) in sorted(plan.content.items()):
        detail = ", ".join("%s×%d" % (k, v) for k, v in sorted(counts.items()))
        if inserted:
            detail = (detail + ", " if detail else "") + "Einschub " + "+".join(inserted)
        print("  M %s  (%s)" % (rel, detail), file=out)
        if verbose:
            for line in difflib.unified_diff(old.splitlines(), new.splitlines(), lineterm="", n=0):
                if line.startswith(("---", "+++", "@@")):
                    continue
                print("      " + line, file=out)
    for src, dst in plan.renames:
        print("  R %s -> %s" % (src, dst), file=out)


# ------------------------------------------------------------------------------------------------
# Befehle
# ------------------------------------------------------------------------------------------------


def cmd_apply(root: Path, dry_run: bool, verbose: bool) -> int:
    plan = build_plan(root)
    for e in plan.errors:
        print("FEHLER: " + e, file=sys.stderr)
    if plan.errors:
        return 2
    total = sum(sum(c.values()) for _, _, c, _ in plan.content.values())
    print("%s: %d Datei(en) mit %d Ersetzung(en), %d Pfad(e) umzubenennen"
          % ("Geplant" if dry_run else "Umbenennung", len(plan.content), total, len(plan.renames)))
    print_plan(plan, verbose)
    if not dry_run:
        left = execute(root, plan)
        stale = [g for g, _ in DERIVED if (root / g).exists()]
        if plan.content or plan.renames:
            print("Fertig. Abgeleitete Dateien neu erzeugen: python3 tools/rename-to-pida.py --regenerate "
                  "(%s), danach --check." % ", ".join(stale))
        print_local_hints(left, untracked_hits(root, left))
    return 0


def cmd_check(root: Path, verbose: bool) -> int:
    plan = build_plan(root, with_residual=True)
    for e in plan.errors:
        print("FEHLER: " + e, file=sys.stderr)
    problems = len(plan.content) + len(plan.renames) + len(plan.residual) + len(plan.errors)
    # Hinweise ohne Einfluss auf den Exit-Code: Arbeitskopie außerhalb der Versionsverwaltung
    stale = stale_paths(root, tracked_files(root))
    hints = (untracked_hits(root, [d.rstrip("/") for d, _ in stale]), stale)
    if not problems:
        print("check: keine Altnamen außerhalb der Ausnahmen.")
        print_local_hints((), hints[0], hints[1])
        return 0
    print("check: %d Datei(en) mit ausstehenden Ersetzungen, %d Pfad(e) umzubenennen, %d Restfundstelle(n)%s:"
          % (len(plan.content), len(plan.renames), len(plan.residual),
             (" (veraltet: " + ", ".join(plan.derived_stale) + " → --regenerate)") if plan.derived_stale else ""))
    print_plan(plan, verbose)
    for rel, n, s in plan.residual:
        print("  ? %s:%d: %s" % (rel, n, s[:160]))
    print_local_hints((), hints[0], hints[1])
    return 1


def cmd_regenerate(root: Path) -> int:
    for what, cmd, cwd, env in REGENERATE:
        full_env = dict(os.environ)
        full_env.update(env)
        prefix = " ".join("%s=%s" % kv for kv in env.items())
        print("==> %s: %s%s (in %s)" % (what, prefix + " " if prefix else "", " ".join(cmd), cwd), flush=True)
        if shutil.which(cmd[0]) is None:
            print("FEHLER: %s nicht gefunden" % cmd[0], file=sys.stderr)
            return 2
        r = subprocess.run(list(cmd), cwd=str(root / cwd), env=full_env)
        if r.returncode != 0:
            print("FEHLER: %s fehlgeschlagen (Exit %d)" % (what, r.returncode), file=sys.stderr)
            return r.returncode
    return 0


def cmd_rules() -> int:
    print("Mapping-Tabelle (Reihenfolge = Anwendung):")
    for r in RULES:
        scope = ("nur " + ", ".join(r.files)) if r.files else "überall"
        where = "+".join(k for k, on in (("Inhalt", r.content), ("Pfad", r.path)) if on)
        print("  %-24s %-8s %-60s → %s   [%s; %s]  %s"
              % (r.id, r.stage, r.pattern, r.repl.replace("\n", "\\n"), where, scope, r.desc))
    print("\nNie bearbeitet:")
    for g, why in SKIP:
        print("  %-48s %s" % (g, why))
    print("  in testdata/ bearbeitet: " + ", ".join(TESTDATA_PROCESS))
    print("\nNeu erzeugt statt umgeschrieben: " + ", ".join("%s (%s)" % d for d in DERIVED))
    print("\nGeschützt (Inhalt):")
    for p in PROTECT:
        print("  %-18s %-55s %s" % (p.id, p.pattern, p.desc))
    print("\nGeschützt (Pfade):")
    for p in PATH_PROTECT:
        print("  %-18s %-55s %s" % (p.id, p.pattern, p.desc))
    print("\nErlaubte Original-Bezüge (--check):")
    for p in ALLOWED:
        print("  %-18s %-55s %s" % (p.id, p.pattern, p.desc))
    print("\nEinschübe:")
    for i in INSERTS:
        print("  %-18s %-48s %s" % (i.id, i.file, i.desc))
    return 0


# ------------------------------------------------------------------------------------------------
# Selbsttest mit synthetischem Fixture (auch für die erst nach dem Mobile-Merge kommenden Dateien)
# ------------------------------------------------------------------------------------------------

FIXTURE: Dict[str, str] = {
    "Cargo.toml": (
        "# PDF-Over NG – Cargo-Workspace\n[workspace]\nmembers = [\"apps/app/src-tauri\", \"apps/app/plugins/pdfover-mobile\"]\n"
        "[workspace.package]\nauthors = [\"PDF-Over NG contributors\"]\nrepository = \"https://github.com/ataub1/pdf-over-ng\"\n"
        "[workspace.dependencies]\npdfover-core-api = { path = \"crates/core-api\" }\n"
        "tauri-plugin-pdfover-mobile = { path = \"apps/app/plugins/pdfover-mobile\" }\n"
    ),
    "apps/app/src-tauri/Cargo.toml": (
        "[package]\nname = \"pdfover-app\"\ndefault-run = \"pdf-over-ng\"\n[lib]\nname = \"pdfover_app_lib\"\n"
        "[[bin]]\nname = \"pdf-over-ng\"\n"
    ),
    "apps/app/src-tauri/tauri.conf.json": (
        "{\n  \"productName\": \"PDF-Over NG\",\n  \"identifier\": \"at.pdf-over-ng\",\n  \"mainBinaryName\": \"pdf-over-ng\",\n"
        "  \"shortDescription\": \"PDF-Dokumente elektronisch signieren\",\n"
        "  \"csp\": \"img-src pdfover-render: http://pdfover-render.localhost\"\n}\n"
    ),
    "apps/cli/src/main.rs": (
        "//! pdfover-cli – kompatibel zu PDF-Over (-i -o -b), liest ~/.pdf-over/PDF-Over.config\n"
        "use pdfover_config::legacy;\n"
        "const HELP: &str = \"PDF-Over NG – PDF-Dokumente elektronisch signieren\";\n"
        "const IMPORT: &str = \"Einstellungen aus PDF-Over übernehmen\";\n"
        "fn main() { let _ = std::env::var(\"PDFOVER_HOME\"); println!(\"pdfover-smoke: ok\"); }\n"
        "fn legacy() { pdfover_i18n::convert::convert_pdfover_messages(); let _ = pdfover_sigblock::PDFOVER_PROFILE_IDS; }\n"
        "// Original: at.asit.pdfover.gui.Main, pdf-over-gui/src/main/java/at/asit/pdfover/gui/Main.java\n"
        "const CN: &str = \"PDF-Over NG Test RSA (NUR TEST)\"; // Testzertifikat\n"
        "const UA: &str = \"PDF-Over NG v1\"; // NG-Ergänzung\n"
    ),
    # Mobile-Plugin (kommt erst mit dem Mobile-Merge)
    "apps/app/plugins/pdfover-mobile/Cargo.toml": (
        "[package]\nname = \"tauri-plugin-pdfover-mobile\"\nlinks = \"tauri-plugin-pdfover-mobile\"\n"
    ),
    "apps/app/plugins/pdfover-mobile/src/lib.rs": (
        "pub const PLUGIN_NAME: &str = \"pdfover-mobile\";\npub const ANDROID_PACKAGE: &str = \"at.pdfoverng.mobile\";\n"
        "pub struct PdfoverMobile;\npub trait PdfoverMobileExt { fn pdfover_mobile(&self) -> PdfoverMobile; }\n"
        "#[cfg(target_os = \"ios\")]\ntauri::ios_plugin_binding!(init_plugin_pdfover_mobile);\n"
    ),
    "apps/app/plugins/pdfover-mobile/android/build.gradle.kts": "android {\n    namespace = \"at.pdfoverng.mobile\"\n}\n",
    "apps/app/plugins/pdfover-mobile/android/src/main/java/at/pdfoverng/mobile/PdfoverMobilePlugin.kt": (
        "// PDF-Over NG – native Android-Seite des Plugins `pdfover-mobile`\npackage at.pdfoverng.mobile\n"
        "@TauriPlugin\nclass PdfoverMobilePlugin(private val activity: Activity) : Plugin(activity)\n"
    ),
    "apps/app/plugins/pdfover-mobile/ios/Package.swift": (
        "let package = Package(\n  name: \"tauri-plugin-pdfover-mobile\",\n  targets: [.target(name: \"tauri-plugin-pdfover-mobile\")])\n"
    ),
    "apps/app/plugins/pdfover-mobile/ios/Sources/PdfoverMobilePlugin.swift": (
        "class PdfoverMobilePlugin: Plugin {}\n@_cdecl(\"init_plugin_pdfover_mobile\")\n"
        "func initPlugin() -> Plugin { return PdfoverMobilePlugin() }\n"
    ),
    # erzeugte Mobile-Projekte (tauri android/ios init)
    "apps/app/src-tauri/gen/android/settings.gradle": "rootProject.name = \"pdf-over-ng\"\n",
    "apps/app/src-tauri/gen/android/app/src/main/java/at/pdf_over_ng/MainActivity.kt": (
        "package at.pdf_over_ng\n\nclass MainActivity : TauriActivity()\n"
        "const val TAG = \"PdfOverNg\"\nconst val EXTRA_SMOKE = \"pdfover.smoke\" // setzt PDFOVER_SMOKE=1\n"
    ),
    "apps/app/plugins/pdfover-mobile/android/src/main/AndroidManifest.xml": (
        "<provider android:name=\"at.pdfoverng.mobile.PdfoverFileProvider\"\n"
        "    android:authorities=\"${applicationId}.pdfover.files\">\n"
        "  <meta-data android:resource=\"@xml/pdfover_share_paths\" />\n</provider>\n"
    ),
    "apps/app/plugins/pdfover-mobile/android/src/main/res/xml/pdfover_share_paths.xml": (
        "<paths><files-path name=\"pdfover_out\" path=\"Documents/\" /></paths>\n"
    ),
    "apps/app/plugins/pdfover-mobile/android/src/main/java/at/pdfoverng/mobile/SecureStore.kt": (
        "package at.pdfoverng.mobile\nprivate const val TAG = \"PdfOverNg/SecureStore\"\n"
        "private const val PREFS = \"pdfover_secure_store\"\n"
    ),
    "apps/app/plugins/pdfover-mobile/ios/Sources/SecureStore.swift": (
        "// PDF-Over NG – Keychain (`PDF-Over NG/<konto>`)\n"
        "static var service: String { (Bundle.main.bundleIdentifier ?? \"at.pdf-over-ng\") + \".secrets\" }\n"
    ),
    "apps/app/plugins/pdfover-mobile/.gitignore": "/.tauri\n/android/build\n",
    "apps/app/src-tauri/gen/android/app/.gitignore": "/src/main/**/generated\n/src/main/jniLibs/**/*.so\n",
    "apps/app/src-tauri/gen/android/app/src/main/res/values/themes.xml": (
        "<resources><style name=\"Theme.pdfover_app\" parent=\"Theme.Material3.DayNight.NoActionBar\" /></resources>\n"
    ),
    "apps/app/src-tauri/gen/android/app/src/main/AndroidManifest.xml": (
        "<!-- PDF-Over NG – Manifest. FileProvider bringt das Plugin pdfover-mobile mit. -->\n"
        "<application android:theme=\"@style/Theme.pdfover_app\" />\n"
    ),
    "apps/app/src-tauri/gen/android/buildSrc/src/main/java/at/pdf_over_ng/kotlin/BuildTask.kt": (
        "package at.pdf_over_ng.kotlin\n"
    ),
    "apps/app/src-tauri/gen/android/gradlew.bat": "@rem PDF-Over NG\r\n@rem pdfover-app\r\n",
    "apps/app/src-tauri/gen/android/gradlew": "#!/bin/sh\n# pdfover-app\n",
    "apps/app/src-tauri/tauri.android.conf.json": "{\n  \"identifier\": \"at.pdf-over-ng\",\n  \"bundle\": {}\n}\n",
    "apps/app/src-tauri/tauri.ios.conf.json": "{\n  \"productName\": \"PDF-Over NG\",\n  \"bundle\": {\"iOS\": {}}\n}\n",
    # Xcode-Projekt, wie es `tauri ios init` aus dem Crate-Namen pdfover-app erzeugt
    "apps/app/src-tauri/gen/apple/pdfover-app.xcodeproj/project.pbxproj": (
        "/* pdfover-app_iOS.app */ = {path = \"pdfover-app_iOS.app\"; };\n"
        "/* pdfover-app_iOS.entitlements */ = {path = \"pdfover-app_iOS.entitlements\"; };\n"
        "PRODUCT_BUNDLE_IDENTIFIER = \"at.pdf-over-ng\"; PRODUCT_NAME = \"PDF-Over NG\";\n"
    ),
    "apps/app/src-tauri/gen/apple/pdfover-app.xcodeproj/xcshareddata/xcschemes/pdfover-app_iOS.xcscheme": (
        "<BuildableReference BuildableName = \"pdfover-app_iOS.app\" ReferencedContainer = "
        "\"container:pdfover-app.xcodeproj\" />\n"
    ),
    "apps/app/src-tauri/gen/apple/Sources/pdfover-app/bindings/bindings.h": "namespace ffi { extern \"C\" { void start_app(); } }\n",
    "apps/app/src-tauri/gen/android/app/build.gradle.kts": (
        "// Anderer Ordner: -Ppdfover.pdfiumJniLibs=<pfad>.\n"
        "val pdfiumJniLibs = findProperty(\"pdfover.pdfiumJniLibs\")\n"
        "val devCleartext = System.getenv(\"PDFOVER_ANDROID_DEV_CLEARTEXT\") == \"1\"\n"
        "android {\n    namespace = \"at.pdf_over_ng\"\n    defaultConfig {\n"
        "        // Kennung at.pdf-over-ng: „-“ ist im Android-Paketnamen unzulässig → at.pdf_over_ng (Tauri).\n"
        "        applicationId = \"at.pdf_over_ng\"\n    }\n}\n"
    ),
    "apps/app/src-tauri/gen/apple/pdfover-app_iOS/pdfover-app_iOS.entitlements": "<plist><dict/></plist>\n",
    "apps/app/src-tauri/gen/android/app/src/main/res/values/strings.xml": (
        "<resources>\n    <string name=\"app_name\">PDF-Over NG</string>\n"
        "    <string name=\"main_activity_title\">PDF-Over NG</string>\n</resources>\n"
    ),
    "apps/app/src-tauri/gen/apple/project.yml": (
        "name: pdf-over-ng\noptions:\n  bundleIdPrefix: at.pdf-over-ng\ntargets:\n  pdf-over-ng_iOS:\n"
        "    sources:\n      - path: Sources\n      - path: pdf-over-ng_iOS\n"
        "    settings:\n      PRODUCT_BUNDLE_IDENTIFIER: at.pdf-over-ng\n      PRODUCT_NAME: PDF-Over NG\n"
    ),
    "apps/app/src-tauri/gen/apple/pdf-over-ng.xcodeproj/project.pbxproj": (
        "/* pdf-over-ng_iOS */ = {isa = PBXNativeTarget; productName = \"PDF-Over NG\"; libapp = libpdfover_app_lib.a; };\n"
    ),
    "apps/app/src-tauri/gen/apple/pdf-over-ng_iOS/Info.plist": "<key>CFBundleName</key><string>PDF-Over NG</string>\n",
    "apps/app/src-tauri/gen/apple/Sources/pdf-over-ng/main.mm": "#include \"bindings/bindings.h\"\n",
    # Ausnahmen
    "crates/i18n/locales/de/pdfover.ftl": "### PDF-Over NG – deutscher Katalog\n### aus `pdfover_i18n::convert` (PDF-Over)\n",
    "crates/i18n/locales/de/pdfover-ng.ftl": "ng-ui-starting = PDF-Over NG wird gestartet\nng-ui-import = Einstellungen aus PDF-Over übernehmen\n",
    "docs/analyse/01-original.md": "PDF-Over NG ersetzt PDF-Over; pdfover-cli; at.asit.pdfover.gui\n",
    "docs/00-architektur-und-umsetzungsplan.md": "# PDF-Over NG – Architektur- und Umsetzungsplan\n\nArbeitstitel ist **„PDF-Over NG“**.\n",
    "testdata/config/legacy-home/.pdf-over/cfg/config.properties": "KEYSTORE_FILE=/tmp/pdfover-nicht-vorhanden/test.p12\n",
    "testdata/cades/make-testdata.sh": (
        "#!/bin/sh\n# PDF-Over NG, Paket B – Testdaten für pdfover-cades\nO=\"/C=AT/O=PDF-Over NG Test (NUR TEST)\"\n"
        "echo \"$PDFOVER_ROOT\"\n"
    ),
    "testdata/golden/pdfas-4.4.2/cases/X__a4__pdfover/manifest.json": "{\"name\":\"PDF-Over NG Test RSA (NUR TEST)\",\"position_key\":\"pdfover\"}\n",
    "tools/env.sh": (
        "# Entwicklungsumgebung für PDF-Over NG\nexport PATH=\"$HOME/.cargo/bin:$PATH\"\nPDFOVER_ROOT=\"$(pwd)\"\nexport PDFOVER_ROOT\n"
    ),
    "docs/entwicklung.md": "# Entwicklung – PDF-Over NG\n\nText.\n",
}

EXPECTED: Dict[str, Sequence[str]] = {
    "Cargo.toml": ("# PIDA – Cargo-Workspace", "\"apps/app/plugins/pida-mobile\"", "PIDA contributors",
                   "github.com/ataub1/pida\"", "pida-core-api = {", "tauri-plugin-pida-mobile = { path = \"apps/app/plugins/pida-mobile\" }"),
    "apps/app/src-tauri/Cargo.toml": ("name = \"pida-app\"", "default-run = \"pida\"", "name = \"pida_app_lib\"", "name = \"pida\"\n"),
    "apps/app/src-tauri/tauri.conf.json": ("\"productName\": \"PIDA\"", "\"identifier\": \"at.pida\"", "\"mainBinaryName\": \"pida\"",
                                           "\"shortDescription\": \"PIDA – PDF-Signatur mit ID Austria\"",
                                           "pida-render: http://pida-render.localhost"),
    "apps/cli/src/main.rs": ("//! pida-cli – kompatibel zu PDF-Over (-i -o -b), liest ~/.pdf-over/PDF-Over.config",
                             "use pida_config::legacy;", "\"PIDA – PDF-Signatur mit ID Austria\"",
                             "\"Einstellungen aus PDF-Over übernehmen\"", "var(\"PIDA_HOME\")", "pida-smoke: ok",
                             "pida_i18n::convert::convert_pdfover_messages()", "pida_sigblock::PDFOVER_PROFILE_IDS",
                             "at.asit.pdfover.gui.Main, pdf-over-gui/src/main/java/at/asit/pdfover/gui/Main.java",
                             "\"PDF-Over NG Test RSA (NUR TEST)\"", "\"PIDA v1\"; // PIDA-Ergänzung"),
    "apps/app/plugins/pida-mobile/Cargo.toml": ("name = \"tauri-plugin-pida-mobile\"", "links = \"tauri-plugin-pida-mobile\""),
    "apps/app/plugins/pida-mobile/src/lib.rs": ("\"pida-mobile\"", "\"at.pida.mobile\"", "pub struct PidaMobile;",
                                                "trait PidaMobileExt { fn pida_mobile(&self) -> PidaMobile; }",
                                                "ios_plugin_binding!(init_plugin_pida_mobile)"),
    "apps/app/plugins/pida-mobile/android/build.gradle.kts": ("namespace = \"at.pida.mobile\"",),
    "apps/app/plugins/pida-mobile/android/src/main/java/at/pida/mobile/PidaMobilePlugin.kt": (
        "// PIDA – native Android-Seite des Plugins `pida-mobile`", "package at.pida.mobile", "class PidaMobilePlugin("),
    "apps/app/plugins/pida-mobile/ios/Package.swift": ("name: \"tauri-plugin-pida-mobile\"",),
    "apps/app/plugins/pida-mobile/ios/Sources/PidaMobilePlugin.swift": ("class PidaMobilePlugin: Plugin",
                                                                       "@_cdecl(\"init_plugin_pida_mobile\")"),
    "apps/app/src-tauri/gen/android/settings.gradle": ("rootProject.name = \"pida\"",),
    "apps/app/src-tauri/gen/android/app/build.gradle.kts": (
        "namespace = \"at.pida\"", "applicationId = \"at.pida\"", "-Ppida.pdfiumJniLibs=<pfad>",
        "findProperty(\"pida.pdfiumJniLibs\")", "getenv(\"PIDA_ANDROID_DEV_CLEARTEXT\")",
        "// Kennung at.pida = Android-Paket (Tauri; ohne „-“, das dort unzulässig wäre).\n"),
    "apps/app/plugins/pida-mobile/android/src/main/res/xml/pida_share_paths.xml": ("name=\"pida_out\"",),
    "apps/app/plugins/pida-mobile/android/src/main/java/at/pida/mobile/SecureStore.kt": (
        "package at.pida.mobile\n", "TAG = \"PIDA/SecureStore\"", "PREFS = \"pida_secure_store\""),
    "apps/app/plugins/pida-mobile/ios/Sources/SecureStore.swift": ("// PIDA – Keychain (`PIDA/<konto>`)",
                                                                  "?? \"at.pida\") + \".secrets\""),
    "apps/app/plugins/pida-mobile/.gitignore": ("/.tauri\n",),
    "apps/app/src-tauri/gen/android/app/.gitignore": ("/src/main/**/generated\n",),
    "apps/app/src-tauri/gen/android/app/src/main/res/values/themes.xml": ("name=\"Theme.pida_app\"",),
    "apps/app/src-tauri/gen/android/app/src/main/AndroidManifest.xml": (
        "<!-- PIDA – Manifest. FileProvider bringt das Plugin pida-mobile mit. -->",
        "android:theme=\"@style/Theme.pida_app\""),
    "apps/app/src-tauri/gen/android/buildSrc/src/main/java/at/pida/kotlin/BuildTask.kt": ("package at.pida.kotlin\n",),
    "apps/app/src-tauri/gen/android/gradlew": ("# pida-app\n",),
    "apps/app/src-tauri/tauri.android.conf.json": ("\"identifier\": \"at.pida\"",),
    "apps/app/src-tauri/tauri.ios.conf.json": ("\"productName\": \"PIDA\"",),
    "apps/app/src-tauri/gen/apple/pida-app.xcodeproj/project.pbxproj": (
        "/* pida-app_iOS.app */ = {path = \"pida-app_iOS.app\"; };",
        "path = \"pida-app_iOS.entitlements\"", "PRODUCT_BUNDLE_IDENTIFIER = \"at.pida\"; PRODUCT_NAME = \"PIDA\";"),
    "apps/app/src-tauri/gen/apple/pida-app.xcodeproj/xcshareddata/xcschemes/pida-app_iOS.xcscheme": (
        "BuildableName = \"pida-app_iOS.app\"", "\"container:pida-app.xcodeproj\""),
    "apps/app/src-tauri/gen/apple/Sources/pida-app/bindings/bindings.h": ("start_app",),
    "apps/app/src-tauri/gen/android/app/src/main/java/at/pida/MainActivity.kt": (
        "package at.pida\n", "TAG = \"PIDA\"", "EXTRA_SMOKE = \"pida.smoke\" // setzt PIDA_SMOKE=1"),
    "apps/app/plugins/pida-mobile/android/src/main/AndroidManifest.xml": ("${applicationId}.pida.files",),
    "apps/app/src-tauri/gen/apple/pida-app_iOS/pida-app_iOS.entitlements": ("<plist>",),
    "apps/app/src-tauri/gen/android/app/src/main/res/values/strings.xml": ("<string name=\"app_name\">PIDA</string>",),
    "apps/app/src-tauri/gen/apple/project.yml": ("name: pida\n", "bundleIdPrefix: at.pida", "pida_iOS:", "PRODUCT_NAME: PIDA"),
    "apps/app/src-tauri/gen/apple/pida.xcodeproj/project.pbxproj": ("/* pida_iOS */", "productName = \"PIDA\"", "libpida_app_lib.a"),
    "apps/app/src-tauri/gen/apple/pida_iOS/Info.plist": ("<string>PIDA</string>",),
    "apps/app/src-tauri/gen/apple/Sources/pida/main.mm": ("bindings.h",),
    "crates/i18n/locales/de/pdfover.ftl": ("### PIDA – deutscher Katalog", "`pida_i18n::convert` (PDF-Over)"),
    "crates/i18n/locales/de/pida.ftl": ("ng-ui-starting = PIDA wird gestartet", "ng-ui-import = Einstellungen aus PDF-Over übernehmen"),
    "docs/analyse/01-original.md": ("PDF-Over NG ersetzt PDF-Over; pdfover-cli; at.asit.pdfover.gui\n",),
    "docs/00-architektur-und-umsetzungsplan.md": ("# PDF-Over NG – Architektur- und Umsetzungsplan\n\n> **Hinweis (15.10.2026): "
                                                  "Arbeitstitel PDF-Over NG heißt seit 15.10.2026 PIDA**",
                                                  "Arbeitstitel ist **„PDF-Over NG“**."),
    "testdata/config/legacy-home/.pdf-over/cfg/config.properties": ("/tmp/pdfover-nicht-vorhanden/test.p12",),
    "testdata/cades/make-testdata.sh": ("# PIDA, Paket B – Testdaten für pida-cades", "O=PDF-Over NG Test (NUR TEST)",
                                        "\"$PIDA_ROOT\""),
    "testdata/golden/pdfas-4.4.2/cases/X__a4__pdfover/manifest.json": ("PDF-Over NG Test RSA", "\"pdfover\""),
    "tools/env.sh": ("# Entwicklungsumgebung für PIDA", "PIDA_ROOT=", "export PIDA_ROOT", "# Übergang zu PIDA (ADR 0002)",
                     "unset $(env | sed -n 's/^\\(PDFOVER_"),
    "docs/entwicklung.md": ("# Entwicklung – PIDA\n\n> Name: **PIDA**",),
}


def _run_self(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(Path(__file__).resolve()), "--root", str(root), *args],
                          capture_output=True, text=True)


def cmd_self_test() -> int:
    failures: List[str] = []

    def expect(cond: bool, what: str) -> None:
        if not cond:
            failures.append(what)

    with tempfile.TemporaryDirectory(prefix="pida-rename-selftest-") as tmp:
        root = Path(tmp)
        git(root, "init", "-q")
        for rel, text in FIXTURE.items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        (root / "apps/app/src-tauri/icons").mkdir(parents=True)
        binary = b"\x89PNG\r\n\x1a\n\0pdfover-ng PDF-Over NG\0"
        (root / "apps/app/src-tauri/icons/icon.png").write_bytes(binary)
        (root / "notes-untracked.txt").write_text("pdfover-cli PDF-Over NG\n", encoding="utf-8")
        os.chmod(root / "apps/app/src-tauri/gen/android/gradlew", 0o755)
        files = [rel for rel in FIXTURE] + ["apps/app/src-tauri/icons/icon.png"]
        git(root, "-c", "core.autocrlf=false", "add", "--", *files)
        git(root, "-c", "user.name=selftest", "-c", "user.email=selftest@example.invalid", "-c", "commit.gpgsign=false",
            "commit", "-q", "-m", "Fixture")
        # ignorierte Build-Ausgaben in Ordnern, die umbenannt werden (bleiben liegen, werden gemeldet)
        stale_kt = root / "apps/app/src-tauri/gen/android/app/src/main/java/at/pdf_over_ng/generated/TauriActivity.kt"
        stale_kt.parent.mkdir(parents=True)
        stale_kt.write_text("package at.pdf_over_ng\n", encoding="utf-8")
        stale_so = root / "apps/app/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/libpdfover_app_lib.so"
        stale_so.parent.mkdir(parents=True)
        stale_so.write_bytes(b"\x7fELF\0")
        (root / "apps/app/plugins/pdfover-mobile/.tauri").mkdir(parents=True)
        (root / "apps/app/plugins/pdfover-mobile/.tauri/tauri-api.txt").write_text("x\n", encoding="utf-8")

        r = _run_self(root, "--check")
        expect(r.returncode == 1, "--check vor dem Lauf muss Exit 1 liefern (war %d)" % r.returncode)
        expect("tauri-plugin-pdfover-mobile" in r.stdout or "pdfover-mobile" in r.stdout,
               "--check listet die Mobile-Dateien")

        before = {rel: (root / rel).read_text(encoding="utf-8") for rel in FIXTURE}
        r = _run_self(root, "--dry-run", "-v")
        expect(r.returncode == 0, "--dry-run Exit 0 (%s)" % r.stderr.strip())
        expect(all((root / rel).read_text(encoding="utf-8") == before[rel] for rel in FIXTURE), "--dry-run ändert nichts")
        expect("R apps/app/plugins/pdfover-mobile/ios/Sources/PdfoverMobilePlugin.swift -> "
               "apps/app/plugins/pida-mobile/ios/Sources/PidaMobilePlugin.swift" in r.stdout, "--dry-run zeigt git mv")
        expect("+tauri-plugin-pida-mobile = {" in r.stdout, "--dry-run -v zeigt Inhaltsänderungen")

        r = _run_self(root)
        expect(r.returncode == 0, "Lauf 1 Exit 0 (%s)" % r.stderr.strip())
        for old_dir in ("apps/app/plugins/pdfover-mobile/", "apps/app/src-tauri/gen/android/app/src/main/java/at/pdf_over_ng/"):
            expect("Hinweis: %s enthält" % old_dir in r.stdout, "Lauf 1 meldet liegen gebliebenen Altordner " + old_dir)
        expect("buildSrc/src/main/java/at/pdf_over_ng/ enthält" not in r.stdout
               and not (root / "apps/app/src-tauri/gen/android/buildSrc/src/main/java/at/pdf_over_ng").exists(),
               "leere Altordner werden entfernt und nicht gemeldet")
        expect(stale_kt.read_text(encoding="utf-8") == "package at.pdf_over_ng\n", "Unverfolgtes bleibt unangetastet")
        expect((root / "apps/app/src-tauri/gen/android/gradlew.bat").read_bytes() == b"@rem PIDA\r\n@rem pida-app\r\n",
               "CRLF-Zeilenenden bleiben erhalten")
        expect(os.access(root / "apps/app/src-tauri/gen/android/gradlew", os.X_OK), "Ausführungsrecht bleibt erhalten")
        for rel, needles in EXPECTED.items():
            p = root / rel
            if not p.exists():
                failures.append("fehlt nach dem Lauf: " + rel)
                continue
            text = p.read_text(encoding="utf-8")
            for n in needles:
                expect(n in text, "%s enthält %r" % (rel, n))
        for old in ("apps/app/plugins/pdfover-mobile", "apps/app/src-tauri/gen/android/app/src/main/java/at/pdf_over_ng"):
            expect(not git(root, "ls-files", "--", old).stdout.strip(), "keine verfolgte Datei mehr unter " + old)
        for old in ("apps/app/src-tauri/gen/apple/pdf-over-ng.xcodeproj", "apps/app/src-tauri/gen/apple/pdfover-app.xcodeproj",
                    "apps/app/src-tauri/gen/apple/Sources/pdfover-app", "crates/i18n/locales/de/pdfover-ng.ftl"):
            expect(not (root / old).exists(), "Altpfad entfernt: " + old)
        expect((root / "apps/app/src-tauri/icons/icon.png").read_bytes() == binary, "Binärdatei unverändert")
        expect((root / "notes-untracked.txt").read_text(encoding="utf-8") == "pdfover-cli PDF-Over NG\n",
               "unverfolgte Datei unverändert")
        status = git(root, "status", "--porcelain").stdout.decode()
        expect("R  apps/app/plugins/pdfover-mobile/ios/Sources/PdfoverMobilePlugin.swift -> "
               "apps/app/plugins/pida-mobile/ios/Sources/PidaMobilePlugin.swift" in status
               or "RM apps/app/plugins/pdfover-mobile/ios/Sources/PdfoverMobilePlugin.swift -> "
               "apps/app/plugins/pida-mobile/ios/Sources/PidaMobilePlugin.swift" in status,
               "Umbenennung über git mv im Index (%s)" % status.strip().splitlines()[:3])

        snapshot = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file() and ".git" not in p.parts}
        r = _run_self(root)
        expect(r.returncode == 0 and "0 Datei(en) mit 0 Ersetzung(en), 0 Pfad(e)" in r.stdout,
               "Lauf 2 ändert nichts (idempotent): " + r.stdout.strip().splitlines()[0] if r.stdout else "Lauf 2")
        after = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file() and ".git" not in p.parts}
        expect(snapshot == after, "Lauf 2 lässt alle Dateien unverändert")

        r = _run_self(root, "--check")
        expect(r.returncode == 0, "--check nach dem Lauf Exit 0:\n" + r.stdout)
        expect("Ordner mit Altnamen ohne versionierte Dateien: apps/app/src-tauri/gen/android/app/src/main/java/"
               "at/pdf_over_ng/" in r.stdout, "--check weist auf liegen gebliebene Build-Ausgaben hin:\n" + r.stdout)
        expect("nicht versionierte Datei mit Altnamen: apps/app/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/"
               "libpdfover_app_lib.so" in r.stdout, "--check weist auf ignorierte Dateien mit Altnamen hin:\n" + r.stdout)
        expect("nicht versioniert (vom Skript nicht bearbeitet, erst `git add`): notes-untracked.txt:1" in r.stdout,
               "--check weist auf unverfolgte Dateien mit Altnamen hin:\n" + r.stdout)

        # Symlink mit Altnamen (bearbeitet das Skript nicht) meldet --check
        (root / "pdfover-link").symlink_to("README-nicht-vorhanden")
        git(root, "add", "pdfover-link")
        r = _run_self(root, "--check")
        expect(r.returncode == 1 and "pdfover-link:0: Symlink/Submodul" in r.stdout, "--check meldet Symlinks:\n" + r.stdout)
        git(root, "rm", "-q", "--cached", "pdfover-link")
        (root / "pdfover-link").unlink()

        # Restsuche meldet einen Altnamen, den keine Regel erfasst
        (root / "docs/x.md").write_text("Siehe PDFOVER (alt) und pdf over ng.\n", encoding="utf-8")
        git(root, "add", "docs/x.md")
        r = _run_self(root, "--check")
        expect(r.returncode == 1 and "docs/x.md:1" in r.stdout, "--check meldet Restfundstellen:\n" + r.stdout)

    if failures:
        print("Selbsttest FEHLGESCHLAGEN (%d):" % len(failures))
        for f in failures:
            print("  - " + f)
        return 1
    print("Selbsttest ok (%d Fixture-Dateien, %d erwartete Pfade)." % (len(FIXTURE), len(EXPECTED)))
    return 0


# ------------------------------------------------------------------------------------------------


def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Umbenennung PDF-Over NG → PIDA (ADR 0002)")
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true", help="geplante Inhalts- und Pfadänderungen anzeigen")
    mode.add_argument("--check", action="store_true", help="Exit 1, wenn Altnamen außerhalb der Ausnahmen übrig sind")
    mode.add_argument("--regenerate", action="store_true", help="abgeleitete Dateien neu erzeugen (cargo, npm, Bless)")
    mode.add_argument("--rules", action="store_true", help="Mapping-Tabelle und Ausnahmen ausgeben")
    mode.add_argument("--self-test", action="store_true", help="synthetisches Fixture prüfen")
    ap.add_argument("--root", type=Path, default=None, help="Repository-Wurzel (Standard: Git-Wurzel des Skripts)")
    ap.add_argument("-v", "--verbose", action="store_true", help="geänderte Zeilen zeigen")
    args = ap.parse_args(argv)

    if args.rules:
        return cmd_rules()
    if args.self_test:
        return cmd_self_test()
    root = args.root
    if root is None:
        top = subprocess.run(["git", "-C", str(Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"],
                             capture_output=True, text=True, check=True)
        root = Path(top.stdout.strip())
    root = root.resolve()
    if args.check:
        return cmd_check(root, args.verbose)
    if args.regenerate:
        return cmd_regenerate(root)
    return cmd_apply(root, args.dry_run, args.verbose)


if __name__ == "__main__":
    sys.exit(main())
