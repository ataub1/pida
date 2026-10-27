#!/usr/bin/env bash
# Durchstich bauen: Kern → WebAssembly (wasm32-unknown-unknown) → JavaScript-Bindung (wasm-bindgen --target web) →
# web/pkg. Für die Probe-Knöpfe kopiert das Skript ein Test-PDF und den Test-Keystore aus testdata nach web/samples.
#   prototypes/pida-web-spike/build.sh
# Benötigt: rustup-Ziel wasm32-unknown-unknown, wasm-bindgen-cli 0.2.129 (cargo install --locked wasm-bindgen-cli
# --version 0.2.129 --root target/wasm-tools im Repository).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BINDGEN="${WASM_BINDGEN:-$ROOT/target/wasm-tools/bin/wasm-bindgen}"
cd "$HERE"
cargo build --target wasm32-unknown-unknown --release
"$BINDGEN" --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/pida_web_spike.wasm
# PDFium als WebAssembly (pdfium-binaries chromium/8100, pdfium-wasm.tgz; SHA-256 0d68fef9…94c8) aus target/wasm-tools
PDFIUM_WASM="${PDFIUM_WASM:-$ROOT/target/wasm-tools/pdfium-wasm/lib}"
mkdir -p web/pdfium
cp "$PDFIUM_WASM/pdfium.js" "$PDFIUM_WASM/pdfium.wasm" web/pdfium/
mkdir -p web/samples
cp "$ROOT/testdata/pdf/pdfas/pdf-as-legacy/simple.pdf" web/samples/simple.pdf
cp "$ROOT/testdata/keystores/test-rsa2048.p12" web/samples/test-rsa2048.p12
# Vergleich Browser ↔ nativ (tests/vergleich.rs): dieselben Dokumente aus testdata
mkdir -p web/samples/vergleich
for f in pdf/pdfas/pdf-as-legacy/simple.pdf pdf/pdfas-suites/POS_AUTO/example.pdf pdf/pdfas/pdf-as-cli/rotate_090.pdf \
  pdf/pdfas-suites/POS_AUTO_WITH_NEWPAGE/example.pdf pdf/pdfas/pdf-as-web/data/dummy-pdf-signed.pdf \
  pdf/generated/qr-tall-placeholder.pdf; do
  cp "$ROOT/testdata/$f" "web/samples/vergleich/$(echo "$f" | tr '/' '_')"
done
(cd web/samples/vergleich && ls *.pdf | python3 -c 'import json,sys; print(json.dumps([l.strip() for l in sys.stdin]))' > liste.json)
ls -l web/pkg/*.wasm
