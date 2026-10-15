#!/usr/bin/env node
// Erzeugt eine Tauri-Konfigurationsergänzung (JSON), die die PDFium-Bibliothek aus
// $PDFOVER_PDFIUM_DIR ins Bundle übernimmt (macOS: Contents/Frameworks, sonst Ressourcen/pdfium).
// Die Shell findet sie dort (apps/app/src-tauri/src/pdfium.rs). Ohne Variable: leere Ergänzung.
//
//   node scripts/pdfium-config.mjs                     → JSON auf stdout
//   node scripts/pdfium-config.mjs --out <datei.json>  → JSON in die Datei (für `tauri build --config <datei>`)
import { existsSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dir = process.env.PDFOVER_PDFIUM_DIR;
const lib = process.platform === 'win32' ? 'pdfium.dll' : process.platform === 'darwin' ? 'libpdfium.dylib' : 'libpdfium.so';

let overlay = {};
if (dir && existsSync(path.join(dir, lib))) {
  const file = path.resolve(dir, lib);
  overlay = {
    bundle: process.platform === 'darwin' ? { macOS: { frameworks: [file] } } : { resources: { [file]: `pdfium/${lib}` } },
  };
} else {
  console.error(`PDFOVER_PDFIUM_DIR nicht gesetzt oder ohne ${lib}: Bundle ohne PDFium.`);
}

const json = JSON.stringify(overlay, null, 2);
const outIndex = process.argv.indexOf('--out');
if (outIndex > 0 && process.argv[outIndex + 1]) {
  writeFileSync(process.argv[outIndex + 1], `${json}\n`);
} else {
  process.stdout.write(json);
}
