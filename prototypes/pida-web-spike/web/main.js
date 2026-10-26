// Durchstich: lädt den Kern (WebAssembly) und signiert lokal. Ergebnis als Download, Messwerte im Protokoll.
import init, { inspect, sign } from './pkg/pida_web_spike.js';

const $ = (id) => document.getElementById(id);
const log = (line) => { $('log').textContent += `\n${line}`; };
let pdf = null, p12 = null, pdfName = 'dokument.pdf';

const t0 = performance.now();
await init();
$('log').textContent = `Kern geladen (${Math.round(performance.now() - t0)} ms).`;

async function read(file) { return new Uint8Array(await file.arrayBuffer()); }
$('pdf').onchange = async (e) => { pdf = await read(e.target.files[0]); pdfName = e.target.files[0].name; log(`PDF: ${pdfName}, ${inspect(pdf)}`); };
$('p12').onchange = async (e) => { p12 = await read(e.target.files[0]); log(`Keystore: ${e.target.files[0].name}`); };
$('sample').onclick = async () => {
  pdf = new Uint8Array(await (await fetch('samples/simple.pdf')).arrayBuffer());
  p12 = new Uint8Array(await (await fetch('samples/test-rsa2048.p12')).arrayBuffer());
  pdfName = 'simple.pdf';
  log(`Beispiel geladen: simple.pdf (${pdf.length} Bytes), ${inspect(pdf)}; Test-Keystore (${p12.length} Bytes)`);
};
$('sign').onclick = () => {
  if (!pdf || !p12) { log('Bitte zuerst PDF und Keystore wählen.'); return; }
  try {
    const t = performance.now();
    const out = sign(pdf, p12, $('pw').value, $('profile').value, Date.now(), new Date().getTimezoneOffset());
    log(`Signiert in ${Math.round(performance.now() - t)} ms: ${out.length} Bytes, danach ${inspect(out)}`);
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
