#!/usr/bin/env bash
# Golden-Lauf gegen PDF-AS 4.4.2 (Plan §8.2): beschafft die offizielle PDF-AS-CLI, erzeugt die
# Eingaben, signiert alle Fälle (12 Profil-IDs × Eingaben × Positionen, Hinweis- und
# Bildmarken-Varianten) mit dem Test-PKCS#12 über die Keystore-Option der CLI und schreibt die
# Manifeste nach testdata/golden/pdfas-4.4.2/.
#
# Aufruf:  bash tools/golden/run-pdfas.sh [--only <regex>] [--jobs N] [--keep-work] [--reextract]
#   --reextract  Manifeste aus target/golden/work (PDFs, Logs eines früheren Laufs mit --keep-work)
#                neu erzeugen, ohne PDF-AS erneut aufzurufen
#
# Umgebung (optional):
#   PDFOVER_JAVA           Java ≥ 17 (Default /opt/homebrew/opt/openjdk@21/bin/java) – nur Orakel
#   PDFOVER_GOLDEN_CACHE   Download-/Arbeitsordner (Default target/golden, nicht versioniert)
#   PDFOVER_REFERENCE      Referenzquellen (pdfas-config wird nur gelesen und kopiert)
#
# Netz: nur der einmalige Download der PDF-AS-CLI von apps.egiz.gv.at (Prüfsumme fest).
# Die Tests (cargo test) lesen ausschließlich testdata/golden/ und brauchen weder Java noch Netz.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=../env.sh
source "$ROOT/tools/env.sh"

PDFAS_VERSION="4.4.2"
PDFAS_ZIP="pdf-as-cli-${PDFAS_VERSION}.zip"
PDFAS_URL="https://apps.egiz.gv.at/releases/pdf-as/release/${PDFAS_VERSION}/${PDFAS_ZIP}"
# SHA-256 der offiziellen Release-Distribution (apps.egiz.gv.at/releases/pdf-as/release/4.4.2/,
# Stand 2024-09-05 17:11). Zum Vergleich: das Maven-Artefakt
# apps.egiz.gv.at/maven/at/gv/egiz/pdfas/pdf-as-cli/4.4.2/pdf-as-cli-4.4.2.zip ist ein anderer Build
# (SHA-1 4af7bd24d3a053448d001c5772ae6af7b46d9b2d laut .sha1, SHA-256
# 0c6bec14f1269a0f36a81d32ffa5f1509b717cefeea5660398baa30099b96f65) und wird nicht verwendet.
PDFAS_SHA256="09c37cc004f14537e5d0d3d83e44687a833119a5903dd03b6de5d9d907e18eee"

JAVA="${PDFOVER_JAVA:-/opt/homebrew/opt/openjdk@21/bin/java}"
CACHE="${PDFOVER_GOLDEN_CACHE:-$ROOT/target/golden}"
OUT="$ROOT/testdata/golden/pdfas-${PDFAS_VERSION}"
CONFIG="$PDFOVER_REFERENCE/pdfas-config"
KEYSTORE="$ROOT/testdata/keystores/test-rsa2048.p12"
TRUST="$ROOT/testdata/keystores/test-rsa2048.crt.pem"

ONLY=""
JOBS=""
KEEP_WORK=0
REEXTRACT=0
while [ $# -gt 0 ]; do
  case "$1" in
    --only) ONLY="$2"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    --keep-work) KEEP_WORK=1; shift ;;
    --reextract) REEXTRACT=1; KEEP_WORK=1; shift ;;
    *) echo "unbekannte Option: $1" >&2; exit 2 ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "fehlt: $1" >&2; exit 1; }; }
need qpdf
need python3
need curl
[ -x "$JAVA" ] || { echo "Java nicht gefunden: $JAVA (PDFOVER_JAVA setzen)" >&2; exit 1; }
[ -d "$CONFIG/cfg" ] || { echo "pdfas-config fehlt: $CONFIG" >&2; exit 1; }
python3 -c "import pyhanko, PIL" 2>/dev/null || { echo "pyHanko/Pillow fehlen (.venv)" >&2; exit 1; }

# 1. PDF-AS-CLI beschaffen und prüfen ----------------------------------------------------------
mkdir -p "$CACHE/download"
if [ ! -f "$CACHE/download/$PDFAS_ZIP" ]; then
  echo "Lade $PDFAS_URL"
  curl -fsS --retry 3 -o "$CACHE/download/$PDFAS_ZIP.part" "$PDFAS_URL"
  mv "$CACHE/download/$PDFAS_ZIP.part" "$CACHE/download/$PDFAS_ZIP"
fi
actual="$(shasum -a 256 "$CACHE/download/$PDFAS_ZIP" | cut -d' ' -f1)"
if [ "$actual" != "$PDFAS_SHA256" ]; then
  echo "Prüfsumme falsch: $actual (erwartet $PDFAS_SHA256)" >&2
  exit 1
fi
DIST="$CACHE/pdf-as-cli-${PDFAS_VERSION}"
if [ ! -x "$DIST/bin/pdf-as-cli" ]; then
  rm -rf "$CACHE/unpack" && mkdir -p "$CACHE/unpack"
  (cd "$CACHE/unpack" && unzip -q "../download/$PDFAS_ZIP")
  rm -rf "$DIST" && mv "$CACHE/unpack/pdf-as-cli-${PDFAS_VERSION}" "$DIST" && rm -rf "$CACHE/unpack"
fi

# 2. Arbeitsordner, Eingaben ---------------------------------------------------------------------
WORK="$CACHE/work"
if [ -z "$ONLY" ] && [ "$REEXTRACT" = 0 ]; then
  rm -rf "$WORK"
  rm -rf "$OUT/cases"
fi
mkdir -p "$WORK" "$OUT/inputs"
"$JAVA" -cp "$DIST/lib/core-3.5.0.jar" "$ROOT/tools/golden/MakeQr.java" "PDF-AS-POS;id=1" > "$WORK/qr.txt"
"$JAVA" -cp "$DIST/lib/core-3.5.0.jar" "$ROOT/tools/golden/MakeQr.java" "PDF-AS-POS;id=200" > "$WORK/qr200.txt"
python3 "$ROOT/tools/golden/make_inputs.py" "$OUT/inputs" "$WORK/qr.txt" "$WORK/qr200.txt"
if [ -z "$ONLY" ] && [ "$REEXTRACT" = 0 ]; then
  rm -f "$OUT/inputs/presigned.pdf"   # Stufe 0 signiert neu
fi

# 3. Metadaten (Versionen, Prüfsummen) -------------------------------------------------------------
export G_JAVA_VERSION G_QPDF_VERSION G_PDFAS_VERSION="$PDFAS_VERSION" G_PDFAS_URL="$PDFAS_URL" \
  G_PDFAS_SHA256="$PDFAS_SHA256"
G_JAVA_VERSION="$("$JAVA" -version 2>&1 | head -1)"
G_QPDF_VERSION="$(qpdf --version | head -1)"
python3 - "$WORK/meta.json" <<'EOF'
import importlib.metadata as md, json, os, sys, time
e = os.environ
json.dump({
  "pdfas": {"version": e["G_PDFAS_VERSION"], "url": e["G_PDFAS_URL"], "sha256": e["G_PDFAS_SHA256"]},
  "java": e["G_JAVA_VERSION"],
  "qpdf": e["G_QPDF_VERSION"],
  "python": sys.version.split()[0],
  "pyhanko": md.version("pyHanko"),
  "pillow": md.version("pillow"),
  "config": "reference/pdfas-config (Kopie je Variante, Overrides am Ende der Profildatei)",
  "keystore": "testdata/keystores/test-rsa2048.p12 (Alias test-rsa2048)",
  "timezone": "Europe/Vienna",
  "generated": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
}, open(sys.argv[1], "w"), indent=1)
EOF

# 4. Fälle ---------------------------------------------------------------------------------------
ARGS=(--dist "$DIST" --java "$JAVA" --config "$CONFIG" --keystore "$KEYSTORE" --trust "$TRUST"
      --inputs "$OUT/inputs" --out "$OUT" --work "$WORK" --meta "$WORK/meta.json")
[ -n "$ONLY" ] && ARGS+=(--only "$ONLY")
[ -n "$JOBS" ] && ARGS+=(--jobs "$JOBS")
[ "$REEXTRACT" = 1 ] && ARGS+=(--reextract)
python3 "$ROOT/tools/golden/run_cases.py" "${ARGS[@]}"

du -sh "$OUT"
if [ "$KEEP_WORK" = 0 ] && [ -z "$ONLY" ]; then
  rm -rf "$WORK/out"
fi
