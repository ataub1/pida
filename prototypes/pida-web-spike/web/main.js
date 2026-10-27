// Durchstich: lädt den Kern (WebAssembly) und signiert lokal. Ergebnis als Download, Messwerte im Protokoll.
import init, { inspect, sign, preview, pdfium_status, last_plan, initialize_pdfium_render, set_pdfium_never_pause } from './pkg/pida_web_spike.js';

const $ = (id) => document.getElementById(id);
const log = (line) => { $('log').textContent += `\n${line}`; };
let pdf = null, p12 = null, pdfName = 'dokument.pdf';

// PDFium (Emscripten, klassisches Skript mit globalem `Module`) vor dem Kern laden, dann binden
const pdfiumLoaded = new Promise((resolve) => {
  window.Module = { onRuntimeInitialized: () => resolve(window.Module), locateFile: (f) => `pdfium/${f}` };
  const s = document.createElement('script');
  s.src = 'pdfium/pdfium.js';
  document.head.append(s);
});
const t0 = performance.now();
const core = await init();
const t1 = performance.now();
const pdfium = await pdfiumLoaded;
// pdfium-render erkennt den Initialisierer am Namen: ohne `Module.FPDF_InitLibrary` ruft es `PDFium_Init` (Builds von
// paulocoutinhox/pdfium-lib). Das Paket von bblanchon/pdfium-binaries exportiert nur `_FPDF_InitLibrary` – Alias setzen.
if (!pdfium._PDFium_Init && pdfium._FPDF_InitLibrary) pdfium._PDFium_Init = pdfium._FPDF_InitLibrary;
// nur mit ?debug: jeden PDFium-Aufruf protokollieren (Fehlersuche bei der Bindung)
if (new URLSearchParams(location.search).has('debug')) {
  const ccall = pdfium.ccall;
  window.__calls = [];
  pdfium.ccall = function (name, ret, types, args) {
    try {
      const r = ccall.call(this, name, ret, types, args);
      window.__calls.push(name);
      return r;
    } catch (e) {
      window.__calls.push(`FEHLER ${name}(${JSON.stringify(types)}): ${e}`);
      console.error('PDFium-Aufruf fehlgeschlagen', name, types, e);
      throw e;
    }
  };
}
const bound = initialize_pdfium_render(pdfium, core, false);
// Pause-Rückruf für das progressive Rendern in PDFiums eigener Funktionstabelle (Signatur (i32) -> i32)
set_pdfium_never_pause(pdfium.addFunction(() => 0, 'ii'));
const t2 = performance.now();
$('log').textContent = `Kern geladen (${Math.round(t1 - t0)} ms), PDFium geladen und gebunden (${Math.round(t2 - t1)} ms, ${bound ? 'ok' : 'FEHLER'}): ${pdfium_status()}`;

function show(id, bytes, label) {
  const t = performance.now();
  const png = preview(bytes, 0, 1.0);
  let img = document.getElementById(id);
  if (!img) { img = document.createElement('img'); img.id = id; img.style = 'max-width:48%;border:1px solid #8886;margin:.5rem .5rem 0 0'; document.body.append(img); }
  img.src = URL.createObjectURL(new Blob([png], { type: 'image/png' }));
  img.alt = label;
  log(`Vorschau ${label}: ${png.length} Bytes PNG in ${Math.round(performance.now() - t)} ms`);
}

async function read(file) { return new Uint8Array(await file.arrayBuffer()); }
$('pdf').onchange = async (e) => { pdf = await read(e.target.files[0]); pdfName = e.target.files[0].name; log(`PDF: ${pdfName}, ${inspect(pdf)}`); };
$('p12').onchange = async (e) => { p12 = await read(e.target.files[0]); log(`Keystore: ${e.target.files[0].name}`); };
$('sample').onclick = async () => {
  pdf = new Uint8Array(await (await fetch('samples/simple.pdf')).arrayBuffer());
  p12 = new Uint8Array(await (await fetch('samples/test-rsa2048.p12')).arrayBuffer());
  pdfName = 'simple.pdf';
  log(`Beispiel geladen: simple.pdf (${pdf.length} Bytes), ${inspect(pdf)}; Test-Keystore (${p12.length} Bytes)`);
  show('vorher', pdf, 'vorher');
};
$('sign').onclick = () => {
  if (!pdf || !p12) { log('Bitte zuerst PDF und Keystore wählen.'); return; }
  try {
    const t = performance.now();
    const out = sign(pdf, p12, $('pw').value, $('profile').value, Date.now(), new Date().getTimezoneOffset());
    log(`Signiert in ${Math.round(performance.now() - t)} ms: ${out.length} Bytes, danach ${inspect(out)}`);
    log(`Platzierung: ${last_plan()}`);
    show('nachher', out, 'nachher');
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([out], { type: 'application/pdf' }));
    a.download = `${pdfName.replace(/\.pdf$/i, '')}_${$('profile').value}_signiert.pdf`;
    a.textContent = `Herunterladen: ${a.download}`;
    a.id = 'download';
    $('log').after(a);
    window.__pidaSigned = out;
    // nur mit ?pruefung: Ergebnis an den lokalen Prüfserver (serve.py) – zum Prüfen mit qpdf, openssl, pyHanko
    if (new URLSearchParams(location.search).has('pruefung')) {
      fetch(`pruefung/${a.download}`, { method: 'PUT', body: out }).then((r) => log(`An Prüfserver: ${r.status}`));
    }
  } catch (e) {
    log(`Fehler: ${e.message ?? e}`);
  }
};

// Vergleich Browser ↔ nativ: alle Dokumente aus samples/vergleich mit fester Zeit signieren, Lage und Ergebnis an den
// Prüfserver (nur mit ?pruefung). Gegenstück: tests/vergleich.rs (gleiche Zeit, gleicher Keystore, natives PDFium).
export const VERGLEICH_ZEIT_MS = Date.UTC(2026, 9, 26, 9, 0, 0);
window.vergleich = async () => {
  const liste = await (await fetch('samples/vergleich/liste.json')).json();
  const key = new Uint8Array(await (await fetch('samples/test-rsa2048.p12')).arrayBuffer());
  const out = {};
  for (const name of liste) {
    const bytes = new Uint8Array(await (await fetch(`samples/vergleich/${name}`)).arrayBuffer());
    const t = performance.now();
    try {
      const signed = sign(bytes, key, 'test', 'SIGNATURBLOCK_SMALL_DE', VERGLEICH_ZEIT_MS, -60);
      out[name] = { ...JSON.parse(last_plan()), ms: Math.round(performance.now() - t), bytes: signed.length };
      await fetch(`pruefung/browser_${name}`, { method: 'PUT', body: signed });
    } catch (e) {
      out[name] = { fehler: String(e.message ?? e) };
    }
  }
  await fetch('pruefung/browser.json', { method: 'PUT', body: JSON.stringify(out, null, 1) });
  return out;
};
