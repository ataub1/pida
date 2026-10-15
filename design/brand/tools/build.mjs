// Erzeugt alle abgeleiteten Fassungen von Siegel und App-Symbol (ADR 0005) aus den beiden Mastern
// design/brand/pida-seal.svg und design/brand/pida-app-icon.svg:
//
//   design/brand/variants/*.svg            Varianten des App-Symbols (iOS dunkel/getönt, Android, Desktop)
//   design/brand/favicon.svg               Favicon, dazu als apps/app/public/app-icon.svg (index.html)
//   design/brand/pida-seal-1024.png        Siegel, transparent
//   design/brand/pida-seal-sigblock.png    Siegel für den Signaturblock (652 × 684 px = 2 × signatur-logo_de/en.png)
//   design/brand/pida-app-icon-1024.png    App-Symbol, deckend (ohne Alphakanal)
//   design/brand/pida-app-icon-play-512.png  App-Symbol für den Google-Play-Eintrag
//   crates/sigblock/assets/images/pida-seal.png              = pida-seal-sigblock.png
//   apps/app/src-tauri/icons/**                              Desktop (über `tauri icon`), iOS, Android
//   apps/app/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/**   iOS (eine Größe, hell/dunkel/getönt)
//   apps/app/src-tauri/gen/android/app/src/main/res/{mipmap-*,values}/**  Android (adaptiv + monochrom)
//   design/brand/preview.png               Vorschaubogen (nur mit --block <png>, siehe render-block.sh)
//
// Aufruf (Repo-Wurzel):
//   npm --prefix design/brand/tools ci
//   node design/brand/tools/build.mjs [--block <signaturblock.png>] [--tauri <tauri.js>]
//
// Für die Desktop-Symbole (icns/ico) wird die Tauri-CLI der App verwendet
// (apps/app/node_modules/@tauri-apps/cli/tauri.js, sonst --tauri bzw. TAURI_CLI_JS).
// Nicht `tauri icon` direkt auf die App anwenden: es überschreibt die Mobile-Symbole mit
// Tauri-Vorgaben (weißer Android-Hintergrund, keine monochrome Ebene, keine iOS-Varianten).

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { Resvg } from '@resvg/resvg-js';
import { PNG } from 'pngjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const brand = path.resolve(here, '..');
const root = path.resolve(brand, '../..');
const tauriDir = path.join(root, 'apps/app/src-tauri');

const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};

// ------------------------------------------------------------------------------------------------
// Farben (prototypes/pida-workspace/src/styles/tokens.css)
// ------------------------------------------------------------------------------------------------

/** Tinte der Oberfläche (`--c-primary`, `--c-logo-bg`) – Grund des App-Symbols. */
const INK = '#16191f';
/** Rot des Akzents (rot-weiß-rot), wie im Siegel. */
const RED = '#c8102e';
/** Breite des Vorschaubogens in px. */
const PREVIEW_W = 2560;

// ------------------------------------------------------------------------------------------------
// Master lesen
// ------------------------------------------------------------------------------------------------

const sealSvg = fs.readFileSync(path.join(brand, 'pida-seal.svg'), 'utf8');
const iconSvg = fs.readFileSync(path.join(brand, 'pida-app-icon.svg'), 'utf8');

/** Inhalt des Siegels (ohne `<svg>`-Hülle, Kommentar und Titel), Koordinaten 0…40. */
const sealInner = sealSvg
  .replace(/^[\s\S]*?<svg[^>]*>/, '')
  .replace(/<\/svg>\s*$/, '')
  .replace(/<!--[\s\S]*?-->/g, '')
  .replace(/<title>[\s\S]*?<\/title>/, '')
  .split('\n')
  .map((l) => l.trim())
  .filter(Boolean)
  .join('\n');

const glyphMatch = iconSvg.match(/<!-- glyph:start -->([\s\S]*?)<!-- glyph:end -->/);
if (!glyphMatch) throw new Error('pida-app-icon.svg: Markierungen glyph:start/glyph:end fehlen');
/** Zeichen des App-Symbols (Ring, Häkchen, Akzent), Koordinaten 0…1024, Mitte 512/512. */
const glyph = glyphMatch[1]
  .trim()
  .split('\n')
  .map((l) => l.trim())
  .join('\n');

/** Einfarbige Fassung: alles in `color`, der weiße Mittelstreifen des Akzents bleibt frei. */
function monoGlyph(color) {
  return glyph
    .replace(/<rect id="accent-white"[^>]*\/>\s*/, '')
    .replaceAll('#ffffff', color)
    .replaceAll(RED, color);
}

/** Zeichen um die Mitte skaliert und verschoben (Mitte des Zeichens = 512/512). */
function placed(inner, scale, cx = 512, cy = 512) {
  return `<g transform="translate(${cx} ${cy}) scale(${scale}) translate(-512 -512)">\n    ${inner.replaceAll('\n', '\n    ')}\n  </g>`;
}

function svgDoc(title, body, { w = 1024, h = 1024, comment } = {}) {
  const c = comment ? `  <!-- ${comment} -->\n` : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">\n${c}  <title>${title}</title>\n  ${body}\n</svg>\n`;
}

const GENERATED = 'Erzeugt von design/brand/tools/build.mjs aus pida-app-icon.svg – nicht von Hand ändern.';

// ------------------------------------------------------------------------------------------------
// Varianten (SVG)
// ------------------------------------------------------------------------------------------------

/** Android-Vordergrund: 108-dp-Fläche; Ringaußenmaß 0,72 · 672 px ≈ 51 dp (Safe Zone: Kreis 66 dp). */
const ANDROID_SCALE = 0.72;

const variants = {
  // iOS 18+: dunkle Fassung mit transparentem Grund (das System legt den dunklen Verlauf darunter)
  'pida-app-icon-ios-dark': svgDoc('PIDA – App-Symbol (iOS, dunkel)', glyph, { comment: GENERATED }),
  // iOS 18+: getönte Fassung – Graustufen auf Schwarz, das System färbt nach Helligkeit ein
  'pida-app-icon-ios-tinted': svgDoc(
    'PIDA – App-Symbol (iOS, getönt)',
    `<rect width="1024" height="1024" fill="#000000"/>\n  ${monoGlyph('#ffffff')}`,
    { comment: GENERATED },
  ),
  // Android: Vordergrund (transparent) und monochrome Ebene (Android 13+ Themen-Symbole)
  'pida-app-icon-android-foreground': svgDoc(
    'PIDA – App-Symbol (Android, Vordergrund)',
    placed(glyph, ANDROID_SCALE),
    { comment: GENERATED },
  ),
  'pida-app-icon-android-monochrome': svgDoc(
    'PIDA – App-Symbol (Android, monochrom)',
    placed(monoGlyph('#ffffff'), ANDROID_SCALE),
    { comment: GENERATED },
  ),
  // Android bis 7.1 (nur Fallback, minSdk 28 verwendet die adaptive Fassung)
  'pida-app-icon-android-legacy': svgDoc(
    'PIDA – App-Symbol (Android, klassisch)',
    `<rect x="40" y="40" width="944" height="944" rx="176" fill="${INK}"/>\n  ${placed(glyph, 0.92)}`,
    { comment: GENERATED },
  ),
  'pida-app-icon-android-round': svgDoc(
    'PIDA – App-Symbol (Android, rund)',
    `<circle cx="512" cy="512" r="472" fill="${INK}"/>\n  ${placed(glyph, 0.92)}`,
    { comment: GENERATED },
  ),
  // macOS: Raster 824 × 824 auf 1024 (Rand 100 px), Eckenradius 185,4 px
  'pida-app-icon-macos': svgDoc(
    'PIDA – App-Symbol (macOS)',
    `<rect x="100" y="100" width="824" height="824" rx="185.4" fill="${INK}"/>\n  ${placed(glyph, 824 / 1024)}`,
    { comment: GENERATED },
  ),
  // Windows/Linux: abgerundetes Quadrat mit schmalem Rand
  'pida-app-icon-desktop': svgDoc(
    'PIDA – App-Symbol (Windows/Linux)',
    `<rect x="32" y="32" width="960" height="960" rx="200" fill="${INK}"/>\n  ${placed(glyph, 960 / 1024)}`,
    { comment: GENERATED },
  ),
};

/** Favicon: abgerundetes Quadrat, Zeichen etwas größer (16/32 px). */
const favicon = svgDoc(
  'PIDA',
  `<rect width="1024" height="1024" rx="224" fill="${INK}"/>\n  ${placed(glyph, 1.12)}`,
  { comment: GENERATED },
);

// ------------------------------------------------------------------------------------------------
// Rastern
// ------------------------------------------------------------------------------------------------

const fontDir = path.join(root, 'crates/sigblock/assets/fonts');
const fontOptions = {
  loadSystemFonts: false,
  fontFiles: [path.join(fontDir, 'DejaVuSansCondensed.ttf'), path.join(fontDir, 'DejaVuSansCondensed-Bold.ttf')],
  defaultFontFamily: 'DejaVu Sans Condensed',
};

/** SVG → straight RGBA (über PNG, damit die Werte nicht vormultipliziert sind). */
function rasterize(svg, width) {
  const png = new Resvg(svg, { fitTo: { mode: 'width', value: width }, font: fontOptions }).render().asPng();
  return PNG.sync.read(png);
}

/**
 * PNG schreiben. `alpha: false` entfernt den Alphakanal (iOS-Symbole müssen deckend sein und dürfen
 * keinen Alphakanal haben); `opaque: true` prüft nur, dass alles deckend ist, und behält den Kanal
 * (Google Play: 32-Bit-PNG). Voll transparente Pixel bekommen Weiß als Farbe, damit Programme, die
 * eine Transparenz verwerfen, keinen schwarzen Grund zeigen (vgl. PDF-AS `removeAlphaChannel`).
 */
function encode(img, { alpha = true, opaque = !alpha } = {}) {
  const data = Buffer.from(img.data);
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) {
      data[i] = 255;
      data[i + 1] = 255;
      data[i + 2] = 255;
    }
  }
  if (opaque) {
    for (let i = 3; i < data.length; i += 4) {
      if (data[i] !== 255) throw new Error('deckendes Symbol erwartet, aber Transparenz gefunden');
    }
  }
  const out = new PNG({ width: img.width, height: img.height });
  out.data = data;
  return PNG.sync.write(out, {
    colorType: alpha ? 6 : 2,
    inputColorType: 6,
    inputHasAlpha: true,
    deflateLevel: 9,
  });
}

const written = [];
function write(rel, bytes) {
  const file = path.join(root, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes);
  written.push(rel);
}

function writePng(rel, svg, width, opts) {
  const bytes = encode(rasterize(svg, width), opts);
  write(rel, bytes);
  return bytes;
}

// SVG-Varianten
for (const [name, svg] of Object.entries(variants)) write(`design/brand/variants/${name}.svg`, svg);
write('design/brand/favicon.svg', favicon);
write('apps/app/public/app-icon.svg', favicon);

// Siegel
const seal1024 = svgDoc('PIDA – Siegel', sealInner.replaceAll('\n', '\n  '), { w: 40, h: 40 });
writePng('design/brand/pida-seal-1024.png', seal1024, 1024);
// Signaturblock: 652 × 684 px = 2 × 326 × 342 px von signatur-logo_de/en.png (PDF-AS), Siegel 652 × 652
// mittig (wie `DefaultEmblem` mit viewBox 40 × 40 in einer 652 × 684-Fläche). Gleiches Seitenverhältnis
// mit Faktor 2 (Zweierpotenz: die Gleitkomma-Rechnung des Layouts ist exakt dieselbe) ⇒ gleiche
// Zeilenhöhen und Blockmaße für jede Blockbreite; doppelte Auflösung für breite Blöcke und „nur Bildmarke“.
const SEAL_K = 2;
const [SEAL_W, SEAL_H] = [326 * SEAL_K, 342 * SEAL_K];
const sealBlock = svgDoc(
  'PIDA – Siegel (Signaturblock)',
  `<g transform="translate(0 ${(SEAL_H - SEAL_W) / 2}) scale(${SEAL_W / 40})">\n    ${sealInner.replaceAll('\n', '\n    ')}\n  </g>`,
  { w: SEAL_W, h: SEAL_H },
);
const sealBlockPng = writePng('design/brand/pida-seal-sigblock.png', sealBlock, SEAL_W);
write('crates/sigblock/assets/images/pida-seal.png', sealBlockPng);

// App-Symbol (deckend); 512 px für Google Play (Store-Eintrag, Google legt die Maske selbst an).
// App Store (Asset-Katalog): ohne Alphakanal; Google Play verlangt „32-bit PNG (with alpha)“ – deckend.
const icon1024 = writePng('design/brand/pida-app-icon-1024.png', iconSvg, 1024, { alpha: false });
writePng('design/brand/pida-app-icon-play-512.png', iconSvg, 512, { opaque: true });

// iOS: Asset-Katalog mit einer Größe (Xcode erzeugt die übrigen), hell/dunkel/getönt (iOS 18+)
const appiconset = 'apps/app/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset';
for (const f of fs.readdirSync(path.join(root, appiconset))) {
  if (f.endsWith('.png')) fs.rmSync(path.join(root, appiconset, f));
}
write(`${appiconset}/AppIcon-1024.png`, icon1024);
writePng(`${appiconset}/AppIcon-1024-dark.png`, variants['pida-app-icon-ios-dark'], 1024);
writePng(`${appiconset}/AppIcon-1024-tinted.png`, variants['pida-app-icon-ios-tinted'], 1024, { alpha: false });
const contents = {
  images: [
    { filename: 'AppIcon-1024.png', idiom: 'universal', platform: 'ios', size: '1024x1024' },
    {
      appearances: [{ appearance: 'luminosity', value: 'dark' }],
      filename: 'AppIcon-1024-dark.png',
      idiom: 'universal',
      platform: 'ios',
      size: '1024x1024',
    },
    {
      appearances: [{ appearance: 'luminosity', value: 'tinted' }],
      filename: 'AppIcon-1024-tinted.png',
      idiom: 'universal',
      platform: 'ios',
      size: '1024x1024',
    },
  ],
  info: { author: 'xcode', version: 1 },
};
write(`${appiconset}/Contents.json`, `${JSON.stringify(contents, null, 2)}\n`);

// iOS-Symbolordner der Tauri-CLI (Aufbau wie `tauri icon`): alle Einzelgrößen,
// direkt aus dem Vektor gerastert, deckend
const iosSizes = {
  'AppIcon-20x20@1x.png': 20,
  'AppIcon-20x20@2x.png': 40,
  'AppIcon-20x20@2x-1.png': 40,
  'AppIcon-20x20@3x.png': 60,
  'AppIcon-29x29@1x.png': 29,
  'AppIcon-29x29@2x.png': 58,
  'AppIcon-29x29@2x-1.png': 58,
  'AppIcon-29x29@3x.png': 87,
  'AppIcon-40x40@1x.png': 40,
  'AppIcon-40x40@2x.png': 80,
  'AppIcon-40x40@2x-1.png': 80,
  'AppIcon-40x40@3x.png': 120,
  'AppIcon-60x60@2x.png': 120,
  'AppIcon-60x60@3x.png': 180,
  'AppIcon-76x76@1x.png': 76,
  'AppIcon-76x76@2x.png': 152,
  'AppIcon-83.5x83.5@2x.png': 167,
  'AppIcon-512@2x.png': 1024,
};
for (const [name, px] of Object.entries(iosSizes)) {
  writePng(`apps/app/src-tauri/icons/ios/${name}`, iconSvg, px, { alpha: false });
}

// Android: adaptiv (Vordergrund + Hintergrundfarbe + monochrom) und klassische Fallbacks
const densities = { mdpi: 1, hdpi: 1.5, xhdpi: 2, xxhdpi: 3, xxxhdpi: 4 };
const adaptiveXml = `<?xml version="1.0" encoding="utf-8"?>
<!-- PIDA – App-Symbol (ADR 0005), erzeugt von design/brand/tools/build.mjs -->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
  <background android:drawable="@color/ic_launcher_background"/>
  <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
  <monochrome android:drawable="@mipmap/ic_launcher_monochrome"/>
</adaptive-icon>
`;
const backgroundXml = `<?xml version="1.0" encoding="utf-8"?>
<!-- PIDA – Hintergrund des App-Symbols: Tinte der Oberfläche (ADR 0005) -->
<resources>
  <color name="ic_launcher_background">${INK.toUpperCase()}</color>
</resources>
`;
for (const res of ['apps/app/src-tauri/icons/android', 'apps/app/src-tauri/gen/android/app/src/main/res']) {
  for (const [d, f] of Object.entries(densities)) {
    const dir = `${res}/mipmap-${d}`;
    writePng(`${dir}/ic_launcher.png`, variants['pida-app-icon-android-legacy'], Math.round(48 * f));
    writePng(`${dir}/ic_launcher_round.png`, variants['pida-app-icon-android-round'], Math.round(48 * f));
    writePng(`${dir}/ic_launcher_foreground.png`, variants['pida-app-icon-android-foreground'], Math.round(108 * f));
    writePng(`${dir}/ic_launcher_monochrome.png`, variants['pida-app-icon-android-monochrome'], Math.round(108 * f));
  }
  write(`${res}/mipmap-anydpi-v26/ic_launcher.xml`, adaptiveXml);
  write(`${res}/mipmap-anydpi-v26/ic_launcher_round.xml`, adaptiveXml);
  write(`${res}/values/ic_launcher_background.xml`, backgroundXml);
}

// Desktop über die Tauri-CLI: icns aus der macOS-Fassung, alles andere aus der Windows/Linux-Fassung
const tauriJs =
  opt('--tauri') ??
  process.env.TAURI_CLI_JS ??
  path.join(root, 'apps/app/node_modules/@tauri-apps/cli/tauri.js');
if (!fs.existsSync(tauriJs)) {
  throw new Error(`Tauri-CLI nicht gefunden (${tauriJs}): npm --prefix apps/app ci oder --tauri <tauri.js>`);
}
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'pida-icons-'));
function tauriIcon(svgName, outName) {
  const src = path.join(tmp, `${outName}.png`);
  fs.writeFileSync(src, encode(rasterize(variants[svgName], 1024)));
  const out = path.join(tmp, outName);
  try {
    execFileSync(process.execPath, [tauriJs, 'icon', src, '-o', out], { stdio: 'pipe' });
  } catch (e) {
    process.stderr.write(e.stderr ?? '');
    throw e;
  }
  return out;
}
const macOut = tauriIcon('pida-app-icon-macos', 'macos');
const deskOut = tauriIcon('pida-app-icon-desktop', 'desktop');
write('apps/app/src-tauri/icons/icon.icns', fs.readFileSync(path.join(macOut, 'icon.icns')));
for (const f of fs.readdirSync(deskOut)) {
  if (f.endsWith('.png') || f === 'icon.ico') write(`apps/app/src-tauri/icons/${f}`, fs.readFileSync(path.join(deskOut, f)));
}
fs.rmSync(tmp, { recursive: true, force: true });

// ------------------------------------------------------------------------------------------------
// Vorschaubogen
// ------------------------------------------------------------------------------------------------

const blockPng = opt('--block');
if (blockPng) {
  write('design/brand/preview.png', encode(rasterize(previewSheet(fs.readFileSync(blockPng)), PREVIEW_W)));
} else {
  console.log('Hinweis: ohne --block <png> bleibt design/brand/preview.png unverändert (siehe render-block.sh).');
}

console.log(`${written.length} Dateien geschrieben:`);
for (const f of written) console.log(`  ${f}`);

// ------------------------------------------------------------------------------------------------

function dataUri(bytes) {
  return `data:image/png;base64,${Buffer.from(bytes).toString('base64')}`;
}

function pngSize(bytes) {
  const img = PNG.sync.read(Buffer.from(bytes));
  return { w: img.width, h: img.height };
}

function readRel(rel) {
  return fs.readFileSync(path.join(root, rel));
}

/** SVG-ID aus einer Beschriftung. */
function slug(s) {
  return s.normalize('NFKD').replace(/[^A-Za-z0-9]+/g, '-');
}

function previewSheet(block) {
  const LIGHT = '#f4f5f7';
  const DARK = '#0e1116';
  const TEXT = '#14171c';
  const MUTED = '#5f6673';
  const pad = 64;
  const parts = [];
  let y = pad;
  const text = (x, ty, s, { size = 28, weight = 400, fill = TEXT, anchor = 'start' } = {}) =>
    parts.push(
      `<text x="${x}" y="${ty}" font-family="DejaVu Sans Condensed" font-size="${size}" font-weight="${weight}" fill="${fill}" text-anchor="${anchor}">${s}</text>`,
    );
  const image = (x, iy, bytes, w, h, extra = '') =>
    parts.push(`<image x="${x}" y="${iy}" width="${w}" height="${h}" href="${dataUri(bytes)}" ${extra}/>`);

  text(pad, y + 44, 'PIDA – Siegel und App-Symbol', { size: 52, weight: 700 });
  text(pad, y + 92, 'ADR 0005 · Master: design/brand/pida-seal.svg, design/brand/pida-app-icon.svg · eigenes Werk des Projekts, EUPL-1.2', {
    size: 26,
    fill: MUTED,
  });
  y += 150;

  // 1. Siegel im Signaturblock
  text(pad, y + 30, '1  Siegel im Signaturblock – PDFium-Render einer mit pida-cli (Test-Keystore) signierten Datei', {
    size: 32,
    weight: 700,
  });
  y += 56;
  const b = pngSize(block);
  parts.push(`<rect x="${pad}" y="${y}" width="${b.w + 48}" height="${b.h + 48}" fill="#ffffff" stroke="#d5d9df"/>`);
  image(pad + 24, y + 24, block, b.w, b.h);
  const sx = pad + b.w + 120;
  const seal = encode(rasterize(seal1024, 256));
  const checker = `<pattern id="chk" width="32" height="32" patternUnits="userSpaceOnUse"><rect width="32" height="32" fill="#ffffff"/><rect width="16" height="16" fill="#e3e6eb"/><rect x="16" y="16" width="16" height="16" fill="#e3e6eb"/></pattern>`;
  parts.push(`<defs>${checker}</defs>`);
  for (const [i, [bg, label]] of [
    ['url(#chk)', 'transparent'],
    ['#ffffff', 'auf Weiß'],
    [INK, 'auf Tinte'],
  ].entries()) {
    const x = sx + i * 300;
    parts.push(`<rect x="${x}" y="${y}" width="256" height="256" fill="${bg}" stroke="#d5d9df"/>`);
    image(x, y, seal, 256, 256);
    text(x + 128, y + 292, label, { size: 24, fill: MUTED, anchor: 'middle' });
  }
  text(pad, y + b.h + 48 + 40, `Block ${b.w} × ${b.h} px (Render 4 ×); Bildmarke ${SEAL_W} × ${SEAL_H} px (2 × PDF-AS-Logo, gleiches Seitenverhältnis), Blockmaße unverändert`, {
    size: 24,
    fill: MUTED,
  });
  y += Math.max(b.h + 48, 300) + 90;

  // 2. App-Symbol in Pixelgrößen, hell und dunkel (iOS-Maske zur Ansicht)
  text(pad, y + 30, '2  App-Symbol – 1024 / 180 / 120 / 87 / 58 / 40 / 29 px (iOS-Maske nur zur Ansicht)', {
    size: 32,
    weight: 700,
  });
  y += 56;
  const sizes = [1024, 180, 120, 87, 58, 40, 29];
  const rowH = 1024 + 120;
  for (const [ri, bg] of [LIGHT, DARK].entries()) {
    const ry = y + ri * (rowH + 32);
    parts.push(`<rect x="${pad}" y="${ry}" width="${PREVIEW_W - 2 * pad}" height="${rowH}" rx="24" fill="${bg}"/>`);
    let x = pad + 48;
    for (const s of sizes) {
      const iy = ry + 48 + (1024 - s) / 2;
      const r = (s * 0.2237).toFixed(2);
      const id = `m${ri}-${s}`;
      parts.push(`<clipPath id="${id}"><rect x="${x}" y="${iy}" width="${s}" height="${s}" rx="${r}"/></clipPath>`);
      image(x, iy, encode(rasterize(iconSvg, s), { alpha: false }), s, s, `clip-path="url(#${id})"`);
      text(x + s / 2, ry + 48 + 1024 + 48, `${s}`, { size: 24, fill: ri ? '#c4cad3' : MUTED, anchor: 'middle' });
      x += s + (s >= 120 ? 72 : 56);
    }
  }
  y += 2 * rowH + 32 + 72;

  // 3. Android: adaptive Masken und monochrome Ebene (Themen-Symbole, Android 13+)
  text(pad, y + 30, '3  Android adaptiv – Masken Kreis, Squircle, Tropfen; monochrome Ebene (Android 13+)', {
    size: 32,
    weight: 700,
  });
  y += 56;
  const fg = readRel('apps/app/src-tauri/gen/android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_foreground.png');
  const mono = readRel('apps/app/src-tauri/gen/android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_monochrome.png');
  // 108 dp = 432 px (xxxhdpi), sichtbar sind die mittleren 72 dp = 288 px; Maskenpfade aus AOSP (100 × 100)
  const masks = {
    Kreis: 'M50,0A50,50,0,1,1,50,100A50,50,0,1,1,50,0Z',
    Squircle: 'M50,0C10,0 0,10 0,50 0,90 10,100 50,100 90,100 100,90 100,50 100,10 90,0 50,0Z',
    Tropfen: 'M50,0L88,0C94.4,0 100,5.4 100,12L100,50C100,77.6 77.6,100 50,100C22.4,100 0,77.6 0,50C0,22.4 22.4,0 50,0Z',
  };
  const tile = 288;
  const panelH = tile + 160;
  const panel = () =>
    parts.push(`<rect x="${pad}" y="${y}" width="${PREVIEW_W - 2 * pad}" height="${panelH}" rx="24" fill="${LIGHT}"/>`);
  panel();
  let x = pad + 48;
  let ty = y + 48;
  const label = (s, w = tile) => text(x + w / 2, ty + tile + 48, s, { size: 24, fill: MUTED, anchor: 'middle' });
  const masked = (id, d, inner) =>
    parts.push(
      `<clipPath id="${id}"><path transform="translate(${x} ${ty}) scale(${tile / 100})" d="${d}"/></clipPath>`,
      `<g clip-path="url(#${id})">${inner}</g>`,
    );
  for (const [name, d] of Object.entries(masks)) {
    masked(
      `a-${slug(name)}`,
      d,
      `<rect x="${x}" y="${ty}" width="${tile}" height="${tile}" fill="${INK}"/><image x="${x - 72}" y="${ty - 72}" width="432" height="432" href="${dataUri(fg)}"/>`,
    );
    label(name);
    x += tile + 72;
  }
  // monochrom: Ebene als Maske, Farben wie die Material-You-Themen (hell und dunkel)
  parts.push(
    `<mask id="mono" maskUnits="userSpaceOnUse" x="0" y="0" width="432" height="432"><image x="0" y="0" width="432" height="432" href="${dataUri(mono)}"/></mask>`,
  );
  for (const [name, bgc, fgc] of [
    ['monochrom, Thema hell', '#d8e2ff', '#1b3a6b'],
    ['monochrom, Thema dunkel', '#2d3138', '#c3d3f2'],
  ]) {
    masked(
      `m-${slug(name)}`,
      masks.Kreis,
      `<rect x="${x}" y="${ty}" width="${tile}" height="${tile}" fill="${bgc}"/><g transform="translate(${x - 72} ${ty - 72})"><rect width="432" height="432" fill="${fgc}" mask="url(#mono)"/></g>`,
    );
    label(name);
    x += tile + 72;
  }
  y += panelH + 72;

  // 4. iOS 18+ dunkel/getönt (Asset-Katalog) und Desktop
  text(pad, y + 30, '4  iOS 18+ dunkel und getönt (Asset-Katalog), Desktop (macOS-Raster, Windows/Linux)', {
    size: 32,
    weight: 700,
  });
  y += 56;
  panel();
  x = pad + 48;
  ty = y + 48;
  const dark = readRel(`${appiconset}/AppIcon-1024-dark.png`);
  const tinted = readRel(`${appiconset}/AppIcon-1024-tinted.png`);
  const r = (tile * 0.2237).toFixed(2);
  parts.push(
    `<defs><linearGradient id="iosdark" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3a3d44"/><stop offset="1" stop-color="#141518"/></linearGradient></defs>`,
  );
  for (const [name, bytes, bg] of [
    ['iOS dunkel (Systemgrund)', dark, 'url(#iosdark)'],
    ['iOS getönt (Graustufen)', tinted, null],
  ]) {
    parts.push(
      `<clipPath id="i-${slug(name)}"><rect x="${x}" y="${ty}" width="${tile}" height="${tile}" rx="${r}"/></clipPath>`,
      `<g clip-path="url(#i-${slug(name)})">${bg ? `<rect x="${x}" y="${ty}" width="${tile}" height="${tile}" fill="${bg}"/>` : ''}<image x="${x}" y="${ty}" width="${tile}" height="${tile}" href="${dataUri(bytes)}"/></g>`,
    );
    label(name);
    x += tile + 72;
  }
  image(x, ty, encode(rasterize(variants['pida-app-icon-macos'], tile)), tile, tile);
  label('macOS (icns)');
  x += tile + 72;
  image(x, ty + (tile - 256) / 2, readRel('apps/app/src-tauri/icons/128x128@2x.png'), 256, 256);
  label('Windows/Linux (256 px)');
  x += tile + 72;
  image(x, ty + (tile - 64) / 2, readRel('apps/app/src-tauri/icons/32x32.png'), 32, 32);
  image(x + 56, ty + (tile - 64) / 2 - 16, readRel('apps/app/src-tauri/icons/64x64.png'), 64, 64);
  text(x + 60, ty + tile + 48, '32 / 64 px', { size: 24, fill: MUTED, anchor: 'middle' });
  y += panelH + pad;

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${PREVIEW_W}" height="${y}" viewBox="0 0 ${PREVIEW_W} ${y}"><rect width="${PREVIEW_W}" height="${y}" fill="#ffffff"/>${parts.join('')}</svg>`;
}
