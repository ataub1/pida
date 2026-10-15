/**
 * Darstellung einer Mock-Seite: gesetzt in pt (1 px = 1 pt) und per `transform: scale()` gezoomt, damit
 * Umbruch und Lage bei jedem Zoom gleich bleiben. Inhalt ist dekorativ (`aria-hidden`); die Seite
 * selbst trägt ihren Namen am Rahmen.
 */
import { memo } from 'react';
import { SignatureBlock } from '../sigblock/SignatureBlock';
import type { Block, ExistingSignature, PageSpec } from '../../model/types';

const EXISTING_SIG = { profile: 'standard' as const, lang: 'de' as const, logo: null, logoName: null, note: null };

function BlockView({ block, signatures, highlight }: { block: Block; signatures: ExistingSignature[]; highlight: string | null }) {
  const r = block.rect;
  const style = { left: r.x, top: r.y, width: r.w, height: r.h };
  switch (block.kind) {
    case 'letterhead':
      return (
        <div className="doc-block doc-letterhead" style={style}>
          <div className="doc-org">
            <span className="doc-org-mark" />
            {block.org}
          </div>
          {block.lines.map((l) => (
            <div className="doc-small" key={l}>
              {l}
            </div>
          ))}
        </div>
      );
    case 'title':
      return (
        <div className="doc-block doc-title" style={style}>
          {block.text}
        </div>
      );
    case 'subtitle':
      return (
        <div className="doc-block doc-subtitle" style={style}>
          {block.text}
        </div>
      );
    case 'heading':
      return (
        <div className="doc-block doc-heading" style={style}>
          {block.text}
        </div>
      );
    case 'para':
      return (
        <p className="doc-block doc-para" style={style}>
          {block.text}
        </p>
      );
    case 'list':
      return (
        <ul className="doc-block doc-list" style={style}>
          {block.items.map((it) => (
            <li key={it}>{it}</li>
          ))}
        </ul>
      );
    case 'parties':
      return (
        <div className="doc-block doc-parties" style={style}>
          {block.lines.map((l) => (
            <div key={l} className={/^(und|abgeschlossen|–)/.test(l) ? 'doc-parties-muted' : 'doc-parties-strong'}>
              {l}
            </div>
          ))}
        </div>
      );
    case 'table':
      return (
        <table className="doc-block doc-table" style={style}>
          <thead>
            <tr>
              {block.columns.map((c, i) => (
                <th key={i} className={block.numeric[i] ? 'num' : undefined}>
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {block.rows.map((row, ri) => (
              <tr key={ri} className={block.totalRow && ri === block.rows.length - 1 ? 'total' : undefined}>
                {row.map((cell, ci) => (
                  <td key={ci} className={block.numeric[ci] ? 'num' : undefined}>
                    {cell}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      );
    case 'form':
      return (
        <div className="doc-block doc-form" style={style}>
          {block.rows.map(([k, v]) => (
            <div className="doc-form-row" key={k}>
              <span className="doc-form-label">{k}</span>
              <span className="doc-form-value">{v}</span>
            </div>
          ))}
        </div>
      );
    case 'sigline':
      return (
        <div className="doc-block doc-sigline" style={style}>
          {[block.left, block.right].map(([place, who]) => (
            <div className="doc-sigline-col" key={who}>
              <div className="doc-sigline-place">
                {place} <span className="doc-sigline-blank" />
              </div>
              <div className="doc-sigline-line" />
              <div className="doc-sigline-who">{who}</div>
            </div>
          ))}
        </div>
      );
    case 'footer':
      return (
        <div className="doc-block doc-footer" style={style}>
          {block.text}
        </div>
      );
    case 'signature': {
      const sig = signatures.find((s) => s.id === block.signatureId);
      return (
        <div className={`doc-block doc-signature${highlight === block.signatureId ? ' is-highlighted' : ''}`} style={style}>
          <SignatureBlock sig={EXISTING_SIG} signer={sig?.signer ?? '—'} time={sig?.time ?? ''} fit={{ w: r.w, h: r.h }} />
        </div>
      );
    }
  }
}

export const MockPage = memo(function MockPage({
  page,
  scale,
  signatures = [],
  highlight = null,
}: {
  page: PageSpec;
  scale: number;
  signatures?: ExistingSignature[];
  highlight?: string | null;
}) {
  return (
    <div className="mock-page-box" style={{ width: page.w * scale, height: page.h * scale }} aria-hidden="true">
      <div className="mock-page" lang="de" style={{ width: page.w, height: page.h, transform: `scale(${scale})` }}>
        {page.blocks.map((b, i) => (
          <BlockView key={i} block={b} signatures={signatures} highlight={highlight} />
        ))}
      </div>
    </div>
  );
});
