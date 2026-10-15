/**
 * Text mit Link-Markup aus den Katalogen (`<a>…</a>`, `<a href="…">…</a>`, wie `pida_i18n::markup`).
 *
 * Ziel: explizites `href`, sonst `targets[i]`, sonst der Linktext selbst, wenn er eine `https:`-URL ist.
 * Externe Ziele öffnet die Shell im Systembrowser (nur `https:`/`mailto:`). Links sind echte
 * Inline-Elemente (`role="link"`, React Aria `Link` ohne `href`; Enter löst aus): ein `<button>` wäre ein
 * atomares Inline-Element – eine lange URL bräche dann als ganzer Block um und der folgende Satzpunkt
 * landete allein in der nächsten Zeile. Als Inline-Text bricht die URL innerhalb (`.link-inline`).
 */
import { Link } from 'react-aria-components';
import { useApp } from '../app/context';
import { isAllowedExternalUrl } from '../core/bridge';
import { parseMarkup } from '../i18n/markup';

/** Ziel eines Links im Markup (Index = Reihenfolge der `<a>`). */
export type LinkTarget = { href: string } | { onPress: () => void };

export function Markup({ text, targets = [] }: { text: string; targets?: LinkTarget[] }) {
  const { bridge, toast, t } = useApp();
  let linkIndex = -1;
  return (
    <>
      {parseMarkup(text).map((seg, i) => {
        if (seg.kind === 'text') return <span key={i}>{seg.text}</span>;
        linkIndex += 1;
        const target: LinkTarget | null =
          (seg.href ? { href: seg.href } : null) ?? targets[linkIndex] ?? (isAllowedExternalUrl(seg.text) ? { href: seg.text } : null);
        if (!target) return <span key={i}>{seg.text}</span>;
        const onPress =
          'onPress' in target
            ? target.onPress
            : () => {
                bridge.openUrl(target.href).catch(() => toast(t.t('ng-ui-open-failed'), 'error'));
              };
        return (
          <Link key={i} className="link link-inline" onPress={onPress}>
            {seg.text}
            {'href' in target ? <span className="visually-hidden"> {t.t('ng-ui-link-external')}</span> : null}
          </Link>
        );
      })}
    </>
  );
}
