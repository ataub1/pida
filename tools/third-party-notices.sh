#!/usr/bin/env bash
# PIDA – THIRD-PARTY-NOTICES erzeugen (Plan §2.5, §10.2): Drittlizenz-Hinweise für das
# Über-Fenster und die Pakete. MIT, BSD und Apache-2.0 verlangen sie bei Binärweitergabe (auch in
# den Stores).
#
#   tools/third-party-notices.sh                       # → target/notices/THIRD-PARTY-NOTICES.md
#   tools/third-party-notices.sh --out dist/notices    # anderes Zielverzeichnis
#   tools/third-party-notices.sh --pdfium win-x64      # PDFium-Lizenzen dieser Plattform
#
# Inhalt (in dieser Reihenfolge):
#   1. PDFium (BSD-3-Clause) samt Lizenzen der mitgebauten Bibliotheken aus dem PDFium-Paket
#      (FreeType, ICU, libjpeg-turbo, lcms, OpenJPEG, zlib …; tools/fetch-pdfium.sh lädt es bei Bedarf)
#   2. Schriften und Metriken aus crates/sigblock/assets/licenses (DejaVu/Bitstream Vera, Adobe AFM)
#   3. Rust-Abhängigkeiten über cargo-about 0.9.2 (tools/licenses/about.toml, about.hbs) plus die
#      NOTICE-Dateien von Apache-2.0-Crates
#   4. Rust-Standardbibliothek (in jede Binary gelinkt; MIT OR Apache-2.0): Lizenztexte aus der
#      Toolchain, vollständige Hinweise als RUST-STD-COPYRIGHT.html neben der Ausgabe
#   5. npm-Abhängigkeiten der Oberfläche (apps/app/package-lock.json, falls vorhanden; tools/npm-licenses.mjs)
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
    -h | --help) sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
  echo "### PDFium – BSD-3-Clause"
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
cargo about generate --locked --offline --workspace --all-features --fail \
  -c tools/licenses/about.toml tools/licenses/about.hbs -o "$TMP/3-rust.md"
cargo about generate --locked --offline --workspace --all-features --fail \
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

# --- Zusammenführen --------------------------------------------------------------------------------
{
  echo "# THIRD-PARTY-NOTICES – PIDA"
  echo
  echo "PIDA steht unter der EUPL-1.2. Es enthält bzw. nutzt die folgenden Komponenten Dritter;"
  echo "ihre Lizenzbedingungen gelten für den jeweiligen Teil."
  echo
  echo "PIDA is licensed under the EUPL-1.2. It contains or uses the following third-party"
  echo "components; their license terms apply to the respective part."
  echo
  echo "Quelle (source): Cargo.lock sha256 $(python3 -I -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$ROOT/Cargo.lock")"
  echo
  # Muss gültiges UTF-8 sein; Zeilenenden einheitlich LF.
  cat "$TMP/1-pdfium.md" "$TMP/2-assets.md" "$TMP/3-rust.md" "$TMP/4-rust-std.md" "$TMP/5-npm.md" | python3 -I -c '
import sys
data = sys.stdin.buffer.read().decode("utf-8")
sys.stdout.write(data.replace("\r\n", "\n").replace("\r", "\n"))
'
} >"$NOTICES"

echo "third-party-notices: $NOTICES ($(wc -c <"$NOTICES" | tr -d ' ') Bytes)" >&2
