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
mkdir -p web/samples
cp "$ROOT/testdata/pdf/pdfas/pdf-as-legacy/simple.pdf" web/samples/simple.pdf
cp "$ROOT/testdata/keystores/test-rsa2048.p12" web/samples/test-rsa2048.p12
ls -l web/pkg/*.wasm
