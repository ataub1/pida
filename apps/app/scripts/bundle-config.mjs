#!/usr/bin/env node
// Erzeugt eine Tauri-Konfigurationsergänzung (JSON) für das Bundle:
//
// - PDFium aus $PIDA_PDFIUM_DIR (macOS: Contents/Frameworks, sonst Ressourcen/pdfium/); die Shell
//   findet sie dort (apps/app/src-tauri/src/pdfium.rs),
// - macOS-Signatur: tauri.conf.json signiert PIDA.app samt Frameworks ad hoc ("-", versiegelte Ressourcen;
//   nur vom Linker signiert meldete macOS die heruntergeladene App als „beschädigt“) und ohne Hardened Runtime:
//   Deren Library Validation lehnt die ad hoc signierte libpdfium.dylib ab („different Team IDs“), die App
//   liefe dann ohne Vorschau. Mit APPLE_SIGNING_IDENTITY (Developer ID, Notarisierung) schaltet dieses Skript
//   die Hardened Runtime wieder ein (Pflicht für notarytool),
// - Drittlizenz-Hinweise aus $PIDA_NOTICES_DIR (Standard: <repo>/target/notices, erzeugt mit
//   tools/third-party-notices.sh bzw. `npm run notices`): THIRD-PARTY-NOTICES.md und
//   RUST-STD-COPYRIGHT.html als Ressourcen (Plan §2.5: Hinweise je Build ins Paket).
//
//   node scripts/bundle-config.mjs                       → JSON auf stdout
//   node scripts/bundle-config.mjs --out <datei.json>    → JSON in die Datei (`tauri build --config <datei>`)
//   … --require-pdfium --require-notices                 → Abbruch, wenn etwas fehlt (Release/CI)
import { existsSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..', '..', '..');
const args = process.argv.slice(2);
const requirePdfium = args.includes('--require-pdfium');
const requireNotices = args.includes('--require-notices');

const lib = process.platform === 'win32' ? 'pdfium.dll' : process.platform === 'darwin' ? 'libpdfium.dylib' : 'libpdfium.so';
const resources = {};
const bundle = {};
let missing = false;

const pdfiumDir = process.env.PIDA_PDFIUM_DIR;
if (process.platform === 'darwin' && process.env.APPLE_SIGNING_IDENTITY) {
  bundle.macOS = { hardenedRuntime: true };
}

if (pdfiumDir && existsSync(path.join(pdfiumDir, lib))) {
  const file = path.resolve(pdfiumDir, lib);
  if (process.platform === 'darwin') {
    bundle.macOS = { ...bundle.macOS, frameworks: [file] };
  } else {
    resources[file] = `pdfium/${lib}`;
  }
} else {
  console.error(`bundle-config: PIDA_PDFIUM_DIR nicht gesetzt oder ohne ${lib}: Bundle ohne PDFium.`);
  missing ||= requirePdfium;
}

const noticesDir = process.env.PIDA_NOTICES_DIR || path.join(repo, 'target', 'notices');
const notices = ['THIRD-PARTY-NOTICES.md', 'RUST-STD-COPYRIGHT.html'].map((n) => path.join(noticesDir, n));
if (notices.every((f) => existsSync(f))) {
  for (const f of notices) resources[path.resolve(f)] = path.basename(f);
} else {
  console.error(`bundle-config: Drittlizenz-Hinweise fehlen in ${noticesDir} (npm run notices): Bundle ohne Hinweise.`);
  missing ||= requireNotices;
}

if (missing) {
  console.error('bundle-config: Pflichtbestandteile fehlen (--require-pdfium/--require-notices) – Abbruch.');
  process.exit(1);
}
if (Object.keys(resources).length > 0) bundle.resources = resources;
const json = JSON.stringify(Object.keys(bundle).length > 0 ? { bundle } : {}, null, 2);
const outIndex = args.indexOf('--out');
if (outIndex >= 0 && args[outIndex + 1]) {
  writeFileSync(args[outIndex + 1], `${json}\n`);
} else {
  process.stdout.write(json);
}
