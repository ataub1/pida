#!/usr/bin/env bash
# Vorschaubogen design/brand/preview.png (ADR 0005): signiert eine Testdatei mit pida-cli (Test-Keystore
# aus testdata/keystores, „NUR TEST“; eigener PIDA_HOME im Temp-Ordner, keine Netzzugriffe), rendert den
# Signaturblock mit PDFium (pypdfium2, render_block.py) und erzeugt mit build.mjs alle Fassungen samt
# Vorschaubogen.
#
# Aufruf (Repo-Wurzel): design/brand/tools/render-block.sh [weitere Optionen für build.mjs, z. B. --tauri <tauri.js>]
# Voraussetzungen: tools/env.sh (Rust, .venv mit pypdfium2), npm --prefix design/brand/tools ci
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
# shellcheck source=/dev/null
source "$root/tools/env.sh"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cargo build --quiet --manifest-path "$root/Cargo.toml" -p pida-cli
cli="${CARGO_TARGET_DIR:-$root/target}/debug/pida-cli"

mkdir -p "$tmp/home"
export PIDA_HOME="$tmp/home"
# Passwort des Test-Keystores (testdata/keystores/NUR-TEST-KEINE-ECHTEN-SCHLUESSEL.txt)
export PIDA_BRAND_KSPASS=test
"$cli" -i "$root/testdata/pdf/pdfas/pdf-as-legacy/simple.pdf" -o "$tmp/signed.pdf" \
  -b KS -ks "$root/testdata/keystores/test-rsa2048.p12" --kspass-env PIDA_BRAND_KSPASS -a --headless

"${PIDA_PYTHON:-python3}" -I "$root/design/brand/tools/render_block.py" "$tmp/signed.pdf" "$tmp/block.png"
node "$root/design/brand/tools/build.mjs" --block "$tmp/block.png" "$@"
