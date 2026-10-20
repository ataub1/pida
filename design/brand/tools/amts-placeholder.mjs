// Erzeugt crates/sigblock/assets/images/amtssignatur-platzhalter.png aus design/brand/amtssignatur-platzhalter.svg
// (neutrale Platzhalter-Bildmarke der Amtssignatur, Backlog Nr. 10): 472 × 470 px = 2 × amtsignatur-logo_demo.png
// (gleiches Seitenverhältnis, daher gleiche Blockmaße), transparent außerhalb des Rahmens.
//
//   npm --prefix design/brand/tools ci
//   node design/brand/tools/amts-placeholder.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { Resvg } from '@resvg/resvg-js';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const svg = fs.readFileSync(path.join(root, 'design/brand/amtssignatur-platzhalter.svg'), 'utf8');
const png = new Resvg(svg, { fitTo: { mode: 'width', value: 472 }, font: { loadSystemFonts: false } }).render();
if (png.width !== 472 || png.height !== 470) throw new Error(`unerwartete Größe ${png.width} × ${png.height}`);
const out = path.join(root, 'crates/sigblock/assets/images/amtssignatur-platzhalter.png');
fs.writeFileSync(out, png.asPng());
console.log(`${path.relative(root, out)}: ${png.width} × ${png.height}`);
