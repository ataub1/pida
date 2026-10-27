/**
 * Kleiner, sicherer Markdown-Renderer der Offline-Hilfe (ohne `innerHTML`): Absätze, Listen (`- `, `1. `), **fett**,
 * `Code` und Links (`https://…`, `www.…`, öffnen im Systembrowser). Mehr braucht die Wissensbasis nicht.
 */
import type { ReactNode } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { IconExternal } from '../../ui/Icons';

function inline(text: string, onLink: (url: string) => void, key: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /\*\*(.+?)\*\*|`([^`]+)`|(https?:\/\/[^\s)]+|www\.[a-z0-9.-]+\.[a-z]{2,}[^\s)]*)/gi;
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = re.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const k = `${key}-${i++}`;
    if (m[1] !== undefined) out.push(<strong key={k}>{m[1]}</strong>);
    else if (m[2] !== undefined) out.push(<code key={k}>{m[2]}</code>);
    else if (m[3] !== undefined) {
      const raw = m[3].replace(/[.,;:]+$/, '');
      const url = raw.startsWith('http') ? raw : `https://${raw}`;
      out.push(
        <AriaButton key={k} className="link help-link" onPress={() => onLink(url)}>
          {raw}
          <IconExternal size={12} />
        </AriaButton>,
      );
      last = m.index + raw.length;
      re.lastIndex = last;
      continue;
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({ text, onLink }: { text: string; onLink: (url: string) => void }) {
  const blocks: ReactNode[] = [];
  const lines = text.split('\n');
  let para: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  const flush = () => {
    if (para.length) blocks.push(<p key={`p${blocks.length}`}>{inline(para.join(' '), onLink, `p${blocks.length}`)}</p>);
    para = [];
    if (list) {
      const items = list.items.map((it, j) => <li key={j}>{inline(it, onLink, `l${blocks.length}-${j}`)}</li>);
      blocks.push(list.ordered ? <ol key={`o${blocks.length}`}>{items}</ol> : <ul key={`u${blocks.length}`}>{items}</ul>);
    }
    list = null;
  };
  for (const raw of lines) {
    const line = raw.trimEnd();
    const bullet = /^\s*-\s+(.*)$/.exec(line);
    const numbered = /^\s*\d+\.\s+(.*)$/.exec(line);
    if (bullet || numbered) {
      if (para.length) {
        blocks.push(<p key={`p${blocks.length}`}>{inline(para.join(' '), onLink, `p${blocks.length}`)}</p>);
        para = [];
      }
      const ordered = !!numbered;
      if (!list || list.ordered !== ordered) {
        flush();
        list = { ordered, items: [] };
      }
      list.items.push((bullet?.[1] ?? numbered?.[1] ?? '').trim());
    } else if (!line.trim()) {
      flush();
    } else if (list && /^\s{2,}\S/.test(raw)) {
      // Fortsetzungszeile eines Listenpunkts
      list.items[list.items.length - 1] += ` ${line.trim()}`;
    } else {
      if (list) flush();
      para.push(line.trim());
    }
  }
  flush();
  return <div className="help-md">{blocks}</div>;
}
