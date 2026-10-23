#!/usr/bin/env python3
"""PIDA – Drittlizenz-Hinweise der Android-Bibliotheken (Backlog Nr. 2) für tools/third-party-notices.sh.

Das APK enthält neben dem Rust-Teil Java/Kotlin-Bibliotheken aus Gradle (AndroidX, Kotlin, Material, Jackson …).
Ihre Hinweise stehen sonst nur in den JARs/AARs – und das Android-Gradle-Plugin verwirft `META-INF/LICENSE*` und
`META-INF/NOTICE*` beim Packen. Dieses Skript liest daher:

- die Bibliotheken des Laufzeit-Klassenpfads samt POMs (auch der Eltern) und Archiven über das Init-Skript
  `tools/licenses/android-artifacts.gradle` (lädt fehlende Dateien; baut nichts; BOMs ohne Archiv zählen nicht),
- je Bibliothek die Lizenz aus dem POM (bei Bedarf aus den Eltern-POMs),
- alle LICENSE-, NOTICE- und COPYING-Dateien aus `META-INF/` der JARs bzw. der `classes.jar` in den AARs
  (Apache-2.0 §4d: NOTICE-Dateien weitergeben).

Die Gradle-Projekte (`project ':tauri-android'`, Tauri-Plugins) sind Kotlin-Quellen der Rust-Crates; ihre Lizenzen
stehen im Abschnitt der Rust-Abhängigkeiten bzw. sind eigener Code (pida-mobile, EUPL-1.2).

Ausgabe: Markdown-Abschnitt auf stdout. Bricht ab (Exit 1), wenn ein POM fehlt oder eine Lizenz nicht erkennbar bzw.
nicht zugelassen ist (wie `cargo about --fail`).

    tools/android-licenses.py --project apps/app/src-tauri/gen/android \
        --apache <Apache-2.0.txt> [--configuration universalReleaseRuntimeClasspath]
"""

import argparse
import hashlib
import io
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile

# Zulässige Lizenzen: Erkennung über den Namen bzw. die URL im POM → SPDX-ID.
LICENSES = [
    (re.compile(r"apache.*2\.0|apache-2\.0|licenses/LICENSE-2\.0", re.I), "Apache-2.0"),
    (re.compile(r"\bmit\b|opensource\.org/licenses/mit", re.I), "MIT"),
    (re.compile(r"bsd.*3|bsd-3-clause|new bsd", re.I), "BSD-3-Clause"),
    (re.compile(r"bsd.*2|bsd-2-clause|simplified bsd", re.I), "BSD-2-Clause"),
]
# POMs ohne Lizenzangabe: Lizenz mit Beleg festlegen (wie `clarify` in tools/licenses/about.toml).
CLARIFY = {
    # Kotlin-Teil des Crates rustls-platform-verifier-android (Cargo.lock; Cargo.toml: „MIT OR Apache-2.0“), aus dem
    # Maven-Archiv des Projekts; das POM nennt keine Lizenz
    "org.rustls:rustls-platform-verifier": "Apache-2.0",
}
# Bei mehreren Lizenzen (Wahlrecht) gilt Apache-2.0 vor MIT, wie bei den übrigen Hinweisen.
PREFERENCE = ["Apache-2.0", "MIT", "BSD-3-Clause", "BSD-2-Clause"]

NOTICE_FILE = re.compile(r"(^|[-_.])(LICEN[CS]E|NOTICE|COPYING)([-_.]|$)", re.I)


def fail(msg):
    print(f"android-licenses: {msg}", file=sys.stderr)
    sys.exit(1)


def artifacts(project, configuration, init_script):
    """Bibliotheken (gruppe:name:version → Archive), POMs (gruppe:name:version → Pfad) und Gradle-Projekte."""
    out = subprocess.run(
        ["./gradlew", "-q", "--no-configuration-cache", "-I", init_script, ":app:pidaLicenseArtifacts",
         f"-Ppida.configuration={configuration}"],
        cwd=project, check=True, capture_output=True, text=True,
    ).stdout
    modules, poms, projects = {}, {}, []
    for line in out.splitlines():
        kind, _, rest = line.partition(" ")
        if kind == "MODULE":
            modules.setdefault(rest.strip(), [])
        elif kind == "ARCHIVE":
            gav, _, path = rest.partition(" ")
            modules.setdefault(gav, []).append(path)
        elif kind == "POM":
            gav, _, path = rest.partition(" ")
            poms[gav] = path
        elif kind == "PROJECT":
            projects.append(rest.strip().lstrip(":"))
    if not modules:
        fail(f"keine Bibliotheken in {configuration} gefunden")
    return modules, poms, sorted(projects)


def strip_ns(root):
    for el in root.iter():
        if isinstance(el.tag, str) and "}" in el.tag:
            el.tag = el.tag.split("}", 1)[1]
    return root


def pom_licenses(poms, gav, depth=0):
    """Lizenzen (Name, URL) aus dem POM, sonst aus dem Eltern-POM (höchstens fünf Ebenen)."""
    path = poms.get(gav)
    if not path:
        return None
    raw = open(path, "rb").read()
    # POMs brauchen keine DTD: Entitäten (Expansion, externe Verweise) gar nicht erst parsen
    if b"<!DOCTYPE" in raw or b"<!ENTITY" in raw:
        fail(f"{path}: POM mit DTD bzw. Entitäten – abgelehnt")
    root = strip_ns(ET.fromstring(raw))
    found = [
        ((lic.findtext("name") or "").strip(), (lic.findtext("url") or "").strip())
        for lic in root.findall("./licenses/license")
    ]
    if found:
        return found
    parent = root.find("./parent")
    if parent is not None and depth < 5:
        parent_gav = f"{parent.findtext('groupId')}:{parent.findtext('artifactId')}:{parent.findtext('version')}"
        return pom_licenses(poms, parent_gav, depth + 1) or []
    return []


def spdx(found):
    ids = set()
    for lname, url in found:
        for pattern, sid in LICENSES:
            if pattern.search(lname) or pattern.search(url):
                ids.add(sid)
                break
        else:
            return None
    for sid in PREFERENCE:
        if sid in ids:
            return sid
    return None


def notice_files(path):
    """(Name, Text) der LICENSE/NOTICE/COPYING-Dateien in META-INF/ einer JAR bzw. der JARs einer AAR."""
    out = []

    def scan(zf, prefix=""):
        for info in zf.infolist():
            n = info.filename
            if n.endswith(".jar") and (n == "classes.jar" or n.startswith("libs/")):
                with zipfile.ZipFile(io.BytesIO(zf.read(n))) as inner:
                    scan(inner, f"{n}!")
            elif n.upper().startswith("META-INF/") and not n.endswith("/") and NOTICE_FILE.search(os.path.basename(n)):
                raw = zf.read(n)
                try:
                    text = raw.decode("utf-8")
                except UnicodeDecodeError:
                    text = raw.decode("latin-1")
                out.append((prefix + n, text.replace("\r\n", "\n").replace("\r", "\n").rstrip()))

    with zipfile.ZipFile(path) as zf:
        scan(zf)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--project", required=True, help="Gradle-Projekt der Android-App (gen/android)")
    ap.add_argument("--apache", required=True, help="Text der Apache-2.0-Lizenz")
    ap.add_argument("--mit", help="Text der MIT-Lizenz (nur nötig, wenn eine Bibliothek allein MIT ist)")
    ap.add_argument("--configuration", default="universalReleaseRuntimeClasspath")
    args = ap.parse_args()

    init_script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "licenses", "android-artifacts.gradle")
    modules, poms, projects = artifacts(args.project, args.configuration, init_script)

    rows, used, files, boms = [], set(), {}, []
    for gav in sorted(modules):
        archives = modules[gav]
        if not archives:
            # BOMs (Versionslisten) haben kein Archiv und landen nicht im APK
            boms.append(gav)
            continue
        found = pom_licenses(poms, gav)
        if found is None:
            fail(f"{gav}: kein POM aufgelöst")
        group, name, version = gav.split(":", 2)
        sid = spdx(found) if found else CLARIFY.get(f"{group}:{name}")
        if sid is None:
            fail(f"{gav}: Lizenz nicht erkennbar oder nicht zugelassen: {found}")
        used.add(sid)
        rows.append((f"{group}:{name}", version, sid))
        for archive in archives:
            for fname, text in notice_files(archive):
                key = hashlib.sha256(text.encode("utf-8")).hexdigest()
                entry = files.setdefault(key, {"name": os.path.basename(fname), "text": text, "by": []})
                who = f"{group}:{name} {version}"
                if who not in entry["by"]:
                    entry["by"].append(who)
    if boms:
        print(f"android-licenses: ohne Archiv (BOM, nicht im APK): {', '.join(boms)}", file=sys.stderr)

    p = print
    p("## Android-Bibliotheken (Android libraries)")
    p()
    p("Nur in der Android-App (APK): Java- bzw. Kotlin-Bibliotheken aus dem Laufzeit-Klassenpfad")
    p(f"`{args.configuration}`. Die Gradle-Projekte {', '.join(f'`{x}`' for x in projects)} sind Kotlin-Teile der")
    p("Rust-Crates (Lizenzen im Abschnitt der Rust-Abhängigkeiten) bzw. eigener Code von PIDA (EUPL-1.2).")
    p()
    p("Only in the Android app (APK): Java and Kotlin libraries of the runtime classpath. The Gradle projects listed")
    p("above are the Kotlin parts of the Rust crates (licenses in the section on Rust dependencies) or PIDA's own code.")
    p()
    p("| Bibliothek (library) | Version | Lizenz (license) |")
    p("|---|---|---|")
    for lib, version, sid in rows:
        p(f"| `{lib}` | {version} | {sid} |")
    p()
    texts = {"Apache-2.0": args.apache, "MIT": args.mit}
    for sid in PREFERENCE:
        if sid not in used:
            continue
        path = texts.get(sid)
        if not path:
            fail(f"Lizenztext für {sid} fehlt (Argument)")
        p(f"### {sid}")
        p()
        p("~~~~text")
        p(open(path, encoding="utf-8").read().replace("\r\n", "\n").rstrip())
        p("~~~~")
        p()
    if files:
        p("### Lizenz- und NOTICE-Dateien aus den Bibliotheken (license and NOTICE files)")
        p()
        for entry in sorted(files.values(), key=lambda e: (e["by"][0], e["name"])):
            p(f"#### {entry['name']} – {', '.join(entry['by'])}")
            p()
            p("~~~~text")
            p(entry["text"])
            p("~~~~")
            p()


if __name__ == "__main__":
    main()
