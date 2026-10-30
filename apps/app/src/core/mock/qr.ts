/**
 * „QR-Platzhalter erstellen“ im Mock (ADR 0004 Nachtrag 24): Vorschau als SVG mit einem Schein-QR-Code (kein echter
 * QR – den erzeugt nur der Kern) und `export_qr_placeholder` wie der Kern: Nummer prüfen, mit Pfad Hinweis
 * „gespeichert“, ohne Pfad teilen (Mobile) bzw. herunterladen (Web).
 */
import type { QrPlaceholderFormat, QrPlaceholderSpec } from '../types';
import type { MockArea } from './shared';

/** Wie `QrPlaceholderSpec::validate` im Kern. */
export function qrIdValid(id: string): boolean {
  return /^\d{1,10}$/.test(id);
}

/** Wie `QrPlaceholderSpec::file_name`. */
export function qrFileName(spec: QrPlaceholderSpec, format: QrPlaceholderFormat): string {
  return `${spec.language === 'en' ? 'qr-placeholder' : 'qr-platzhalter'}-${spec.id}.${format}`;
}

/** Vorschau (Data-URL, 240 × 48). */
export function qrPlaceholderSvg(spec: QrPlaceholderSpec): string {
  const cells: string[] = [];
  let seed = [...spec.id, spec.profile, spec.language].join('').split('').reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
  for (let y = 0; y < 21; y++) {
    for (let x = 0; x < 21; x++) {
      const finder = (x < 7 && y < 7) || (x > 13 && y < 7) || (x < 7 && y > 13);
      let on: boolean;
      if (finder) {
        const fx = x > 13 ? x - 14 : x;
        const fy = y > 13 ? y - 14 : y;
        on = fx === 0 || fx === 6 || fy === 0 || fy === 6 || (fx > 1 && fx < 5 && fy > 1 && fy < 5);
      } else {
        seed = (seed * 1103515245 + 12345) >>> 0;
        on = seed % 3 === 0;
      }
      if (on) cells.push(`<rect x="${4 + x * 1.6}" y="${4 + y * 1.6}" width="1.6" height="1.6"/>`);
    }
  }
  const en = spec.language === 'en';
  const lines = en ? ['Placeholder for the', 'electronic signature', `No. ${spec.id}`] : ['Platzhalter für die', 'elektronische Signatur', `Nr. ${spec.id}`];
  const text = lines.map((l, i) => `<text x="54" y="${15 + i * 13}">${l}</text>`).join('');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 48" width="240" height="48"><rect width="240" height="48" fill="#fff" stroke="#000"/><g fill="#000">${cells.join('')}</g><g font-family="Helvetica, Arial, sans-serif" font-weight="bold" font-size="10">${text}</g></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

export const qrArea: MockArea<null> = {
  init: () => null,
  handle(e, _state, ctx) {
    if (e.type !== 'export_qr_placeholder') return false;
    if (!qrIdValid(e.spec.id)) {
      ctx.notify('ng-qr-invalid-id');
      return true;
    }
    const name = qrFileName(e.spec, e.format);
    if (e.path) ctx.notify('ng-qr-saved', { name });
    else ctx.native(`${ctx.platform().mobile ? 'share' : 'download'}:${name}`);
    return true;
  },
  project: () => ({}),
};
