#!/usr/bin/env bash
# Web-Variante bauen (Konzept Webdienst, Variante E, Etappe 1): Oberfläche (apps/app, `vite build --mode web`) und
# Kern als WebAssembly (dieses Crate → wasm32-unknown-unknown → wasm-bindgen --target web) nach apps/web/dist.
#   apps/web/build.sh
# Danach den Server starten (Oberfläche, SL-Rückkanal; lokal mit Testgegenstelle statt A-Trust):
#   cargo run -p pida-web-server -- --mock-atrust testdata/keystores/test-rsa2048.p12 --mock-password test
#   → http://127.0.0.1:1442/   (verstecktes Testmerkmal Keystore: http://127.0.0.1:1442/?test-keystore)
# Benötigt: rustup-Ziel wasm32-unknown-unknown, wasm-bindgen-cli 0.2.129 (cargo install --locked wasm-bindgen-cli
# --version 0.2.129 --root target/wasm-tools); PDFium als WebAssembly lädt tools/fetch-pdfium.sh (Plattform wasm,
# Hash in tools/pdfium.lock). Mit PIDA_WEB_NOTICES=<Ordner> (tools/third-party-notices.sh --pdfium wasm) kommen die
# Drittlizenz-Hinweise nach dist/notices („Über PIDA › Lizenzen“).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BINDGEN="${WASM_BINDGEN:-$ROOT/target/wasm-tools/bin/wasm-bindgen}"
PDFIUM_WASM="${PDFIUM_WASM:-$("$ROOT/tools/fetch-pdfium.sh" wasm --print-dir)}"
DIST="$HERE/dist"
(cd "$ROOT/apps/app" && npm run build:web)
cargo build --manifest-path "$HERE/Cargo.toml" --target wasm32-unknown-unknown --release
"$BINDGEN" --target web --no-typescript --out-dir "$DIST/pkg" "$ROOT/target/wasm32-unknown-unknown/release/pida_web.wasm"
mkdir -p "$DIST/pdfium"
cp "$PDFIUM_WASM/pdfium.js" "$PDFIUM_WASM/pdfium.wasm" "$DIST/pdfium/"
if [ -n "${PIDA_WEB_NOTICES:-}" ]; then
  mkdir -p "$DIST/notices"
  cp "$PIDA_WEB_NOTICES/THIRD-PARTY-NOTICES.md" "$PIDA_WEB_NOTICES/RUST-STD-COPYRIGHT.html" "$DIST/notices/"
fi
ls -l "$DIST/pkg/"*.wasm "$DIST/pdfium/pdfium.wasm"
