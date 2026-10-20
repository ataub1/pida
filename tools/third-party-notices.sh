#!/usr/bin/env bash
# PIDA – THIRD-PARTY-NOTICES erzeugen (Plan §2.5, §10.2): Drittlizenz-Hinweise für das
# Über-Fenster und die Pakete. MIT, BSD und Apache-2.0 verlangen sie bei Binärweitergabe (auch in
# den Stores).
#
#   tools/third-party-notices.sh                       # → target/notices/THIRD-PARTY-NOTICES.md
#   tools/third-party-notices.sh --out dist/notices    # anderes Zielverzeichnis
#   tools/third-party-notices.sh --pdfium win-x64      # PDFium-Lizenzen dieser Plattform; win-…: mit Windows-Teil
#
# Inhalt (in dieser Reihenfolge):
#   0. Übernommene EUPL-Bestandteile: PDF-AS-4-Konfiguration und -Bildmarken (© EGIZ, EUPL-1.1+),
#      Herkunft der PDF-Over-Textkataloge (© A-SIT)
#   1. PDFium (BSD-3-Clause) samt Lizenzen der mitgebauten Bibliotheken aus dem PDFium-Paket
#      (FreeType, ICU, libjpeg-turbo, lcms, OpenJPEG, zlib …; tools/fetch-pdfium.sh lädt es bei Bedarf),
#      ergänzt um die Unterlizenz von HarfBuzz (tools/licenses/harfbuzz-ms-use-COPYING.txt)
#   2. Schriften und Metriken aus crates/sigblock/assets/licenses (DejaVu/Bitstream Vera, Adobe AFM)
#   3. Rust-Abhängigkeiten über cargo-about 0.9.2 (tools/licenses/about.toml, about.hbs) plus die
#      NOTICE-Dateien von Apache-2.0-Crates
#   4. Rust-Standardbibliothek (in jede Binary gelinkt; MIT OR Apache-2.0): Lizenztexte aus der
#      Toolchain, vollständige Hinweise als RUST-STD-COPYRIGHT.html neben der Ausgabe
#   5. npm-Abhängigkeiten der Oberfläche (apps/app/package-lock.json, falls vorhanden; tools/npm-licenses.mjs)
#   6. nur mit --pdfium win-…: Windows-Installer und -Laufzeit (WebView2-SDK, NSIS, nsis_tauri_utils, WiX/MS-RL;
#      Texte und Herkunft in tools/licenses/windows)
#
# Benötigt: cargo, rustc, cargo-about 0.9.x (`brew install cargo-about` oder
# `cargo install --locked cargo-about@0.9.2`), python3; für Teil 5 node ≥ 18 und `npm ci` in apps/app.
# Ein Crate mit nicht zulässiger oder nicht erkennbarer Lizenz bricht ab (cargo about --fail).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/target/notices"
PDFIUM_PLATFORM=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out | --pdfium)
      if [ $# -lt 2 ] || [ -z "$2" ]; then
        echo "third-party-notices: $1 braucht einen Wert" >&2
        exit 2
      fi
      if [ "$1" = --out ]; then OUT="$2"; else PDFIUM_PLATFORM="$2"; fi
      shift ;;
    -h | --help) sed -n '2,27p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "third-party-notices: unbekanntes Argument $1" >&2; exit 2 ;;
  esac
  shift
done

command -v cargo-about >/dev/null 2>&1 \
  || { echo "third-party-notices: cargo-about fehlt (brew install cargo-about | cargo install --locked cargo-about@0.9.2)" >&2; exit 1; }
case "$(cargo-about --version 2>/dev/null)" in
  *" 0.9."*) ;;
  *) echo "third-party-notices: warnung: erwartet cargo-about 0.9.x, gefunden: $(cargo-about --version 2>/dev/null)" >&2 ;;
esac

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
NOTICES="$OUT/THIRD-PARTY-NOTICES.md"
cd "$ROOT"

# Lizenztext als UTF-8 mit LF ausgeben (einige Dateien sind ISO-8859-1, z. B. FreeType, oder haben
# CR-Zeilenenden, z. B. die Adobe-AFM-Lizenz).
emit_text() {
  python3 -I -c '
import sys
raw = open(sys.argv[1], "rb").read()
try:
    text = raw.decode("utf-8")
except UnicodeDecodeError:
    text = raw.decode("latin-1")
sys.stdout.write(text.replace("\r\n", "\n").replace("\r", "\n").rstrip() + "\n")
' "$1"
}

# --- 0. Übernommene EUPL-Bestandteile --------------------------------------------------------------
# EUPL Art. 5: Urheber- und Lizenzhinweise des übernommenen PDF-AS-Materials erhalten (Vermerk aus dem Kopf der
# PDF-AS-Quellen); die Liste der Dateien folgt crates/sigblock/assets/README.md. Der EUPL-1.2-Text liegt den Paketen
# als LICENSE.txt bei (bundle-config.mjs), der CLI über `pida-cli licenses`.
{
  echo "## Übernommene EUPL-Bestandteile (EUPL components taken over)"
  echo
  echo "### PDF-AS 4 – Konfiguration und Bildmarken (configuration and logos)"
  echo
  echo "Copyright 2014 by E-Government Innovation Center EGIZ, Graz, Austria. PDF-AS has been contracted by the"
  echo "E-Government Innovation Center EGIZ, a joint initiative of the Federal Chancellery Austria and Graz University"
  echo "of Technology. Licensed under the EUPL, Version 1.1 or – as soon they will be approved by the European"
  echo "Commission – subsequent versions of the EUPL. Quelle (source): <https://git.egiz.gv.at/pdf-as-4/>."
  echo
  echo "PIDA nutzt diese Dateien unter der EUPL-1.2 (Lizenztext: \`LICENSE.txt\` im Programmpaket, an der"
  echo "Kommandozeile \`pida-cli licenses\`). Sie sind inhaltlich unverändert aus der PDF-AS-Konfiguration von"
  echo "PDF-Over (\`PDFASConfig.zip\`) übernommen; in den Textdateien sind nur die Zeilenenden auf LF vereinheitlicht:"
  echo
  echo "PIDA uses these files under the EUPL-1.2 (license text: \`LICENSE.txt\` in the package, on the command line"
  echo "\`pida-cli licenses\`). They are taken over unchanged in content from the PDF-AS configuration shipped with"
  echo "PDF-Over (\`PDFASConfig.zip\`); only the line endings of the text files are normalised to LF:"
  echo
  echo "- \`cfg/config.properties\`, \`cfg/advancedconfig.properties\`"
  profiles=("$ROOT"/crates/sigblock/assets/cfg/profiles/*.properties)
  list=""
  for f in "${profiles[@]}"; do list="$list, \`cfg/profiles/$(basename "$f")\`"; done
  echo "- ${list#, } (${#profiles[@]} Profile)"
  echo "- \`images/signatur-logo_de.png\`, \`images/signatur-logo_en.png\`"
  echo
  echo "Beim Laden setzt PIDA in den kleinen Standardprofilen und \`BASE_LOGO\` das PIDA-Siegel und in der Amtssignatur"
  echo "eine neutrale Platzhalter-Bildmarke als Bildmarke ein (beide eigene Werke von PIDA); die Dateien selbst bleiben"
  echo "unverändert. Die Demo-Bildmarke der Amtssignatur aus PDF-AS liegt PIDA nicht bei."
  echo
  echo "### PDF-Over – Textkataloge (message catalogs)"
  echo
  echo "Ein Teil der Oberflächentexte stammt aus den Textkatalogen von PDF-Over (Copyright 2012 by A-SIT, EUPL;"
  echo "<https://github.com/a-sit/PDF-Over>), umgewandelt nach Fluent und angepasst. Part of the user interface texts"
  echo "is derived from the PDF-Over message catalogs (Copyright 2012 by A-SIT, EUPL), converted to Fluent and adapted."
  echo
} >"$TMP/0-eupl.md"

# --- 1. PDFium -----------------------------------------------------------------------------------
# --print-dir lädt bei Bedarf (SHA-256 aus tools/pdfium.lock) und liefert <plattform>/lib bzw. bin.
if [ -n "$PDFIUM_PLATFORM" ]; then
  pdfium_lib="$("$ROOT/tools/fetch-pdfium.sh" --print-dir "$PDFIUM_PLATFORM")"
else
  pdfium_lib="$("$ROOT/tools/fetch-pdfium.sh" --print-dir)"
fi
PDFIUM_DIR="$(dirname "$pdfium_lib")"
{
  echo "## PDFium ($(awk -F= '$1=="MAJOR"{ma=$2} $1=="MINOR"{mi=$2} $1=="BUILD"{b=$2} $1=="PATCH"{p=$2} END{print ma"."mi"."b"."p}' "$PDFIUM_DIR/VERSION"), bblanchon/pdfium-binaries)"
  echo
  # Nennungen, die FreeType-Lizenz (FTL) und IJG-Lizenz bei Binärweitergabe in der Dokumentation verlangen
  echo "Portions of this software are copyright © The FreeType Project (www.freetype.org). All rights reserved."
  echo "This software is based in part on the work of the Independent JPEG Group."
  echo
  echo "PDFium selbst steht unter BSD-3-Clause bzw. Apache-2.0 (pdfium.txt); die übrigen Abschnitte nennen die"
  echo "Lizenzen der mitgebauten Bibliotheken."
  echo
  echo "### pdfium-binaries (Packaging und Patches) – MIT"
  echo
  echo '~~~~text'
  emit_text "$PDFIUM_DIR/LICENSE"
  echo '~~~~'
  for f in "$PDFIUM_DIR"/licenses/*; do
    [ -f "$f" ] || continue
    echo
    echo "### PDFium: $(basename "$f")"
    echo
    echo '~~~~text'
    emit_text "$f"
    echo '~~~~'
  done
  # harfbuzz.txt verweist für abweichend lizenzierte Teile auf COPYING-Dateien in Unterordnern, die das Paket nicht
  # mitliefert. Ausgeliefert davon ist nur src/ms-use (Microsoft-Daten der USE-Tabelle, die in libpdfium steckt); die
  # übrigen (perf/, test/) gehören nicht zur Bibliothek. Stand: HarfBuzz ff36703b aus den DEPS von chromium/8100.
  if [ -f "$PDFIUM_DIR/licenses/harfbuzz.txt" ]; then
    echo
    echo "### PDFium: harfbuzz – src/ms-use/COPYING (Microsoft, MIT)"
    echo
    echo "Teile der USE-Tabelle von HarfBuzz (hb-ot-shaper-use-table.hh) sind aus diesen Daten erzeugt."
    echo "Parts of HarfBuzz's USE table (hb-ot-shaper-use-table.hh) are generated from this data."
    echo
    echo '~~~~text'
    emit_text "$ROOT/tools/licenses/harfbuzz-ms-use-COPYING.txt"
    echo '~~~~'
  fi
  echo
} >"$TMP/1-pdfium.md"

# --- 2. Schriften und Metriken -------------------------------------------------------------------
{
  echo "## Schriften und Font-Metriken (fonts and font metrics)"
  echo
  for f in "$ROOT"/crates/sigblock/assets/licenses/*; do
    [ -f "$f" ] || continue
    echo "### $(basename "$f")"
    echo
    echo '~~~~text'
    emit_text "$f"
    echo '~~~~'
    echo
  done
} >"$TMP/2-assets.md"

# --- 3. Rust ---------------------------------------------------------------------------------------
cargo fetch --locked >/dev/null
# ohne --offline: Lizenzdateien für `clarify.git` (about.toml) kommen aus den Quell-Repositories (mit Prüfsumme)
cargo about generate --locked --workspace --all-features --fail \
  -c tools/licenses/about.toml tools/licenses/about.hbs -o "$TMP/3-rust.md"
cargo about generate --locked --workspace --all-features --fail \
  -c tools/licenses/about.toml --format json -o "$TMP/rust.json"
# NOTICE-Dateien (Apache-2.0 §4d) der ausgelieferten Crates anhängen.
python3 -I - "$TMP/rust.json" >>"$TMP/3-rust.md" <<'PY'
import json, os, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
seen = set()
blocks = []
for entry in data.get("crates", []):
    pkg = entry.get("package", entry)
    manifest = pkg.get("manifest_path")
    if not manifest:
        continue
    root = os.path.dirname(manifest)
    for name in sorted(os.listdir(root)):
        if name.upper().startswith("NOTICE") and os.path.isfile(os.path.join(root, name)):
            key = (pkg.get("name"), pkg.get("version"), name)
            if key in seen:
                continue
            seen.add(key)
            text = open(os.path.join(root, name), encoding="utf-8", errors="replace").read().rstrip()
            blocks.append(f"### NOTICE: {pkg.get('name')} {pkg.get('version')} ({name})\n\n~~~~text\n{text}\n~~~~\n")
if blocks:
    print("\n## NOTICE-Dateien der Rust-Abhängigkeiten (Apache-2.0 §4d)\n")
    print("\n".join(blocks))
PY

# SPDX-Vorlagen statt Copyright-Vermerk (Lizenzbericht Befund 6): MIT und BSD verlangen den Vermerk. Erlaubt nur für
# die noch zu klärenden Crates in tools/licenses/template-exceptions.txt.
python3 -I - "$TMP/rust.json" "$ROOT/tools/licenses/template-exceptions.txt" <<'PY'
import json, re, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
allowed = {l.strip() for l in open(sys.argv[2], encoding="utf-8") if l.strip() and not l.lstrip().startswith("#")}
template = re.compile(r"<(year|copyright holders|owner|name of author)>", re.I)
found = {}
for lic in data.get("licenses", []):
    if template.search(lic.get("text", "")):
        for used in lic.get("used_by", []):
            crate = used.get("crate", {})
            found.setdefault(crate.get("name"), set()).add(f"{crate.get('version')} ({lic.get('id')})")
unexpected = sorted(set(found) - allowed)
for name in sorted(allowed - set(found)):
    print(f"third-party-notices: warnung: {name} zeigt keine Vorlage mehr – aus template-exceptions.txt entfernen",
          file=sys.stderr)
if unexpected:
    print("third-party-notices: Lizenztext nur als Vorlage ohne Copyright-Vermerk (about.toml [<crate>.clarify]):",
          file=sys.stderr)
    for name in unexpected:
        print(f"  - {name} {', '.join(sorted(found[name]))}", file=sys.stderr)
    sys.exit(1)
PY

# --- 4. Rust-Standardbibliothek --------------------------------------------------------------------
# std/core/alloc (samt compiler_builtins, libc, hashbrown …) stecken in jeder Binary, stehen aber nicht
# in Cargo.lock. Die Toolchain (Komponente rustc, auch im Profil minimal) liefert die Texte mit.
RUST_DOC="$(rustc --print sysroot)/share/doc/rust"
for f in COPYRIGHT-library.html licenses/MIT.txt licenses/Apache-2.0.txt; do
  [ -f "$RUST_DOC/$f" ] || { echo "third-party-notices: $RUST_DOC/$f fehlt (Toolchain unvollständig?)" >&2; exit 1; }
done
cp "$RUST_DOC/COPYRIGHT-library.html" "$OUT/RUST-STD-COPYRIGHT.html"
{
  echo "## Rust-Standardbibliothek (Rust standard library) – $(rustc --version)"
  echo
  echo "Die Rust-Standardbibliothek steht unter MIT oder Apache-2.0 (nach Wahl); die vollständigen"
  echo "Hinweise einschließlich der mitgelinkten Bibliotheken stehen in \`RUST-STD-COPYRIGHT.html\`."
  echo
  echo "The Rust standard library is dual-licensed under MIT or Apache-2.0; the complete notices,"
  echo "including the libraries linked into it, are in \`RUST-STD-COPYRIGHT.html\`."
  echo
  echo "### MIT"
  echo
  echo '~~~~text'
  emit_text "$RUST_DOC/licenses/MIT.txt"
  echo '~~~~'
  echo
  echo "### Apache-2.0"
  echo
  echo '~~~~text'
  emit_text "$RUST_DOC/licenses/Apache-2.0.txt"
  echo '~~~~'
  echo
} >"$TMP/4-rust-std.md"

# --- 5. npm ----------------------------------------------------------------------------------------
LOCK="$ROOT/apps/app/package-lock.json"
if [ -f "$LOCK" ]; then
  command -v node >/dev/null 2>&1 || { echo "third-party-notices: node fehlt (für $LOCK)" >&2; exit 1; }
  node "$ROOT/tools/npm-licenses.mjs" "$LOCK" --notices "$TMP/5-npm.md"
else
  echo "third-party-notices: apps/app/package-lock.json fehlt – npm-Teil übersprungen" >&2
  : >"$TMP/5-npm.md"
fi

# --- 6. Windows-Installer und -Laufzeit -----------------------------------------------------------
# Nicht in Cargo.lock bzw. im npm-Lockfile: statisch gelinkter WebView2-Loader, NSIS-Laufzeit im Setup, WiX-Teile im
# MSI (Lizenzbericht Befunde 1, 7, 8). Versionen und Quellen: tools/licenses/windows/README.md. Maßgeblich ist das
# tatsächlich verwendete PDFium-Paket (<plattform>/lib bzw. bin), nicht das Argument.
: >"$TMP/6-windows.md"
case "$(basename "$PDFIUM_DIR")" in
  win-*)
    WIN="$ROOT/tools/licenses/windows"
    {
      echo "## Windows-Installer und -Laufzeit (Windows installer and runtime)"
      echo
      echo "### Microsoft WebView2 SDK 1.0.3800.47 – BSD-3-Clause"
      echo
      echo "\`WebView2LoaderStatic.lib\` ist statisch in \`pida.exe\` gelinkt; die Bindings von \`webview2-com-sys\` sind"
      echo "aus dem SDK erzeugt. \`WebView2LoaderStatic.lib\` is statically linked into \`pida.exe\`; the bindings of"
      echo "\`webview2-com-sys\` are generated from the SDK."
      echo
      echo '~~~~text'
      emit_text "$WIN/Microsoft.Web.WebView2-LICENSE.txt"
      echo '~~~~'
      echo
      echo "### Microsoft Visual C++-Laufzeit (vcruntime) – Visual Studio \"Distributable Code\""
      echo
      echo "\`pida.exe\` enthält statisch gelinkte Teile der Microsoft-Visual-C++-Laufzeit (vcruntime; © Microsoft"
      echo "Corporation), gebaut mit Microsoft Visual Studio. Für diese Teile gilt nicht die EUPL, sondern die Bedingungen"
      echo "von Microsoft für \"Distributable Code\": Sie werden nur als Bestandteil von PIDA weitergegeben, ihre Urheber-,"
      echo "Marken- und Patenthinweise dürfen nicht entfernt werden, und soweit gesetzlich zulässig dürfen sie nicht"
      echo "zurückentwickelt, dekompiliert oder disassembliert werden. Microsoft übernimmt für sie keine Gewährleistung."
      echo "Die Universal C Runtime (UCRT) ist Teil von Windows und liegt PIDA nicht bei."
      echo
      echo "\`pida.exe\` contains statically linked parts of the Microsoft Visual C++ runtime (vcruntime; © Microsoft"
      echo "Corporation), built with Microsoft Visual Studio. These parts are not covered by the EUPL but by Microsoft's"
      echo "terms for \"Distributable Code\": they are distributed only as part of PIDA, their copyright, trademark and patent"
      echo "notices must not be removed, and to the extent permitted by law they must not be reverse engineered, decompiled"
      echo "or disassembled. Microsoft provides no warranty for them. The Universal C Runtime (UCRT) is part of Windows and"
      echo "is not included with PIDA."
      echo
      echo "### NSIS 3.13 – Installerlaufzeit des Setups (zlib/libpng; LZMA-Modul CPL-1.0; Modern UI 2)"
      echo
      echo "Setup und Uninstaller enthalten die NSIS-Installerlaufzeit samt Plugins und Grafik (zlib/libpng), die Seiten"
      echo "und Dialoge von Modern UI 2 (eigener Vermerk unten) und den LZMA-Dekompressor (Common Public License 1.0)."
      echo "Für den CPL-Teil schließt A-SIT im Namen aller Beitragenden jede Gewährleistung und Bedingung, ausdrücklich"
      echo "oder stillschweigend, sowie die Haftung für Schäden jeder Art aus (CPL §5, §6); davon abweichende Bedingungen"
      echo "bietet allein A-SIT als Herausgeber von PIDA an, nicht die übrigen Beitragenden. Der Quellcode ist von A-SIT"
      echo "erhältlich über <https://github.com/kichik/nsis/tree/v313> (unveränderter Quellcode von NSIS 3.13)."
      echo
      echo "Setup and uninstaller contain the NSIS installer runtime with plug-ins and graphics (zlib/libpng), the pages and"
      echo "dialogs of Modern UI 2 (own notice below) and the LZMA decompressor (Common Public License 1.0). For the CPL part,"
      echo "A-SIT, on behalf of all Contributors, disclaims all warranties and conditions, express or implied, and excludes"
      echo "liability for damages of any kind (CPL §5, §6); any provisions which differ from the CPL are offered by A-SIT,"
      echo "the publisher of PIDA, alone and not by any other party. The source code is available from A-SIT at"
      echo "<https://github.com/kichik/nsis/tree/v313> (unmodified source code of NSIS 3.13)."
      echo
      echo '~~~~text'
      emit_text "$WIN/NSIS-COPYING.txt"
      echo '~~~~'
      echo
      echo "#### NSIS Modern UI 2 (Contrib/Modern UI 2)"
      echo
      echo '~~~~text'
      emit_text "$WIN/NSIS-ModernUI2-License.txt"
      echo '~~~~'
      echo
      echo "### nsis_tauri_utils 0.5.3 – Apache-2.0 OR MIT (NSIS-Plugin im Setup)"
      echo
      echo '~~~~text'
      emit_text "$WIN/nsis_tauri_utils-LICENSE_MIT.txt"
      echo '~~~~'
      echo
      echo "### WiX Toolset 3.14.1 – MS-RL (WixUI-Dialoge, -Bitmaps und uica.dll im MSI)"
      echo
      echo "Das MSI enthält die WiX-UI-Bibliothek und die Custom Action \`uica.dll\` aus WiX 3.14.1 unverändert. Quellcode:"
      echo "<https://github.com/wixtoolset/wix3/releases/tag/wix3141rtm>. The MSI contains the WiX UI library and the custom"
      echo "action \`uica.dll\` from WiX 3.14.1, unmodified; source code at the link above."
      echo
      echo '~~~~text'
      emit_text "$WIN/MS-RL.txt"
      echo '~~~~'
      echo
    } >"$TMP/6-windows.md"
    ;;
esac

# --- Zusammenführen --------------------------------------------------------------------------------
# Jahr des Urheberrechtsvermerks wie „Über PIDA“ (Versionsdatum, crates/core-api/build_support/release_date.rs):
# SOURCE_DATE_EPOCH, sonst das Commit-Datum – nur aus dem Repository, das PIDA selbst verwaltet (nicht aus einem fremden,
# in dem ein Quellarchiv ausgepackt wurde) –, sonst die Dateizeit von Cargo.toml, zuletzt heute; Jahre vor 2026 zählen
# nicht (SOURCE_DATE_EPOCH=0, Dateizeiten 1970/1980)
YEAR=""
if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
  YEAR="$(python3 -I -c 'import datetime,sys; print(datetime.datetime.fromtimestamp(int(sys.argv[1]), datetime.timezone.utc).year)' "$SOURCE_DATE_EPOCH" 2>/dev/null || true)"
fi
if ! [ "${YEAR:-0}" -ge 2026 ] 2>/dev/null && git -C "$ROOT" ls-files --error-unmatch -- Cargo.toml >/dev/null 2>&1; then
  YEAR="$(git -C "$ROOT" log -1 --format=%cd --date=format:%Y 2>/dev/null || true)"
fi
if ! [ "${YEAR:-0}" -ge 2026 ] 2>/dev/null; then
  YEAR="$(python3 -I -c 'import datetime,os,sys; print(datetime.datetime.fromtimestamp(os.stat(sys.argv[1]).st_mtime, datetime.timezone.utc).year)' "$ROOT/Cargo.toml" 2>/dev/null || true)"
fi
if ! [ "${YEAR:-0}" -ge 2026 ] 2>/dev/null; then
  YEAR="$(date -u +%Y)"
fi
{
  echo "# THIRD-PARTY-NOTICES – PIDA"
  echo
  echo "PIDA – Urheberrecht © $YEAR A-SIT (Zentrum für sichere Informationstechnologie – Austria) – steht"
  echo "unter der EUPL-1.2. Es enthält bzw. nutzt die folgenden Komponenten Dritter; ihre Lizenzbedingungen"
  echo "gelten für den jeweiligen Teil."
  echo
  echo "PIDA – Copyright © $YEAR A-SIT (Secure Information Technology Center – Austria) – is licensed under"
  echo "the EUPL-1.2. It contains or uses the following third-party components; their license terms apply to"
  echo "the respective part."
  echo
  echo "Quelle (source): Cargo.lock sha256 $(python3 -I -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$ROOT/Cargo.lock")"
  echo
  # Muss gültiges UTF-8 sein; Zeilenenden einheitlich LF.
  cat "$TMP/0-eupl.md" "$TMP/1-pdfium.md" "$TMP/2-assets.md" "$TMP/3-rust.md" "$TMP/4-rust-std.md" "$TMP/5-npm.md" "$TMP/6-windows.md" | python3 -I -c '
import sys
data = sys.stdin.buffer.read().decode("utf-8")
sys.stdout.write(data.replace("\r\n", "\n").replace("\r", "\n"))
'
} >"$NOTICES"

echo "third-party-notices: $NOTICES ($(wc -c <"$NOTICES" | tr -d ' ') Bytes)" >&2
