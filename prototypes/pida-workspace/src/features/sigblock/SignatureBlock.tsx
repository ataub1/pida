/**
 * Signaturblock wie er auf dem Dokument erscheint (angelehnt an die PDF-AS-Profile SIGNATURBLOCK_SMALL
 * und AMTSSIGNATURBLOCK, docs/analyse/04). Gesetzt in pt, skaliert per `scale` (1 = 1 px je pt).
 * Die Blocksprache ist unabhängig von der Oberflächensprache.
 */
import { DefaultEmblem } from '../../ui/Logo';
import { blockSize, effectiveNote } from '../../model/format';
import type { Lang, SigBlockSettings } from '../../model/types';

const CAPTIONS: Record<Lang, Record<'signer' | 'date' | 'verify' | 'note' | 'issuer' | 'serial' | 'method', string>> = {
  de: {
    signer: 'Unterzeichner',
    date: 'Datum/Zeit-UTC',
    verify: 'Prüfinformation',
    note: 'Hinweis',
    issuer: 'Aussteller-Zertifikat',
    serial: 'Serien-Nr.',
    method: 'Methode',
  },
  en: {
    signer: 'Signatory',
    date: 'Date/Time-UTC',
    verify: 'Verification',
    note: 'Note',
    issuer: 'Issuer certificate',
    serial: 'Serial no.',
    method: 'Method',
  },
};

const VERIFY_TEXT: Record<Lang, string> = {
  de: 'Informationen zur Prüfung der elektronischen Signatur finden Sie unter: https://www.signaturpruefung.gv.at',
  en: 'Information on the verification of the electronic signature can be found at: https://www.signaturpruefung.gv.at',
};

export interface SignatureBlockProps {
  sig: SigBlockSettings;
  signer: string;
  time: string;
  scale?: number;
  /** auf Zielgröße strecken (Signaturfeld) */
  fit?: { w: number; h: number };
  className?: string;
}

export function SignatureBlock({ sig, signer, time, scale = 1, fit, className }: SignatureBlockProps) {
  const size = blockSize(sig);
  const note = effectiveNote(sig).trim();
  const c = CAPTIONS[sig.lang];
  const sx = fit ? fit.w / size.w : 1;
  const sy = fit ? fit.h / size.h : 1;
  const s = Math.min(sx, sy);
  const outerW = (fit ? fit.w : size.w) * scale;
  const outerH = (fit ? fit.h : size.h) * scale;
  const offsetX = fit ? ((fit.w - size.w * s) / 2) * scale : 0;
  const offsetY = fit ? ((fit.h - size.h * s) / 2) * scale : 0;
  const mainH = sig.profile === 'amt' ? 92 : 49;
  const emblem = sig.logo ? <img src={sig.logo} alt="" className="sb-logo-img" /> : <DefaultEmblem />;
  const rows: [string, string][] =
    sig.profile === 'amt'
      ? [
          [c.signer, signer],
          [c.date, time],
          [c.issuer, 'a-sign-premium-mobile-07 (Demo-CA), A-Trust GmbH, AT'],
          [c.serial, '1 288 004 912'],
          [c.method, 'urn:pdfsigfilter:bka.gv.at:binaer:v1.1.0'],
          [c.verify, VERIFY_TEXT[sig.lang]],
        ]
      : [
          [c.signer, signer],
          [c.date, time],
          [c.verify, VERIFY_TEXT[sig.lang]],
        ];
  return (
    <div className={`sb-outer${className ? ` ${className}` : ''}`} style={{ width: outerW, height: outerH }} aria-hidden="true">
      <div
        className={`sb sb-${sig.profile}`}
        style={{
          width: size.w,
          height: size.h,
          transform: `translate(${offsetX}px, ${offsetY}px) scale(${scale * s})`,
        }}
      >
        <div className="sb-main" style={{ height: mainH, gridTemplateRows: sig.profile === 'amt' ? 'repeat(5, 12px) 1fr' : '13px 13px 1fr' }}>
          <div className="sb-emblem" style={{ gridColumn: 1, gridRow: `1 / span ${rows.length}` }}>
            {emblem}
          </div>
          {rows.map(([k, v], i) => (
            <div className="sb-row" key={k}>
              <span className="sb-caption">{k}</span>
              <span className={`sb-value${i === rows.length - 1 ? ' sb-value-wrap' : ''}`}>{v}</span>
            </div>
          ))}
        </div>
        {note ? (
          <div className="sb-note">
            <span className="sb-caption">{c.note}</span>
            <span className="sb-value sb-value-wrap">{note}</span>
          </div>
        ) : null}
      </div>
    </div>
  );
}
