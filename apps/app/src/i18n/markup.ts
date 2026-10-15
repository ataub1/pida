/**
 * Link-Markup der Katalogtexte (`<a>Text</a>`, `<a href="Ziel">Text</a>`), wie
 * `pida_i18n::markup::parse`: Unvollständiges Markup bleibt Text, andere spitze Klammern auch.
 */

/** Teil eines Textes. */
export type Segment = { kind: 'text'; text: string } | { kind: 'link'; text: string; href: string | null };

/** Zerlegt einen Text in Text- und Link-Segmente. */
export function parseMarkup(text: string): Segment[] {
  const out: Segment[] = [];
  let plain = '';
  let rest = text;
  for (;;) {
    const start = rest.indexOf('<a');
    if (start < 0) break;
    const after = rest.slice(start + 2);
    const openEnd = after.indexOf('>');
    if (openEnd < 0) break;
    const attrs = after.slice(0, openEnd);
    if (!(attrs.length === 0 || /^\s/.test(attrs))) {
      // z. B. "<abbr>": kein Link
      plain += rest.slice(0, start + 2);
      rest = after;
      continue;
    }
    const body = after.slice(openEnd + 1);
    const close = body.indexOf('</a>');
    if (close < 0) break;
    plain += rest.slice(0, start);
    if (plain) {
      out.push({ kind: 'text', text: plain });
      plain = '';
    }
    out.push({ kind: 'link', text: body.slice(0, close), href: parseHref(attrs) });
    rest = body.slice(close + 4);
  }
  plain += rest;
  if (plain) out.push({ kind: 'text', text: plain });
  return out;
}

function parseHref(attrs: string): string | null {
  const i = attrs.indexOf('href');
  if (i < 0) return null;
  let v = attrs.slice(i + 4).trimStart();
  if (!v.startsWith('=')) return null;
  v = v.slice(1).trimStart();
  const quote = v[0];
  if (quote !== '"' && quote !== "'") return null;
  v = v.slice(1);
  const end = v.indexOf(quote);
  return end < 0 ? null : v.slice(0, end);
}

/** Klartext (Links → ihr Text). */
export function toPlain(text: string): string {
  return parseMarkup(text)
    .map((s) => s.text)
    .join('');
}
