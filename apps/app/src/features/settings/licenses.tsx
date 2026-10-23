/**
 * Lizenzen in der App (Backlog Nr. 2; Einstellungen › Über PIDA › „Lizenzen anzeigen“): EUPL-1.2 von PIDA, die
 * Drittlizenz-Hinweise (THIRD-PARTY-NOTICES) und die Hinweise der Rust-Standardbibliothek – auf allen Plattformen im
 * eigenen Dialog statt über die Standardanwendung (Android hat keine Ressourcendateien, `bridge.licenseText` liest die
 * in das Programm eingebetteten Texte). Die HTML-Hinweise der Rust-Standardbibliothek erscheinen als Text (kein
 * eingebettetes HTML: die CSP erlaubt keine Frames, und so läuft nichts davon aus).
 */
import { useEffect, useState } from 'react';
import { Tab, TabList, TabPanel, Tabs } from 'react-aria-components';
import { useApp } from '../../app/context';
import type { LicenseKind } from '../../core/bridge';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { Spinner } from '../../ui/controls';

const KINDS: { kind: LicenseKind; msg: string }[] = [
  { kind: 'eupl', msg: 'ng-ui-licenses-tab-eupl' },
  { kind: 'notices', msg: 'ng-ui-licenses-tab-notices' },
  { kind: 'rust_std', msg: 'ng-ui-licenses-tab-rust-std' },
];

const BLOCK = new Set(['P', 'DIV', 'LI', 'UL', 'OL', 'PRE', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'TR', 'TABLE', 'BR', 'HR', 'SECTION', 'BLOCKQUOTE']);

/** HTML als Text (Absätze und Überschriften als Zeilen); ausgeführt und geladen wird nichts (DOMParser, inert). */
export function htmlToText(html: string): string {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const out: string[] = [];
  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      out.push((node.textContent ?? '').replace(/\s+/g, ' '));
      return;
    }
    if (!(node instanceof Element) || node.tagName === 'SCRIPT' || node.tagName === 'STYLE' || node.tagName === 'TITLE') return;
    const block = BLOCK.has(node.tagName);
    if (block) out.push('\n');
    if (node.tagName === 'LI') out.push('• ');
    if (node.tagName === 'PRE') {
      out.push(node.textContent ?? '');
    } else {
      node.childNodes.forEach(walk);
    }
    if (block) out.push('\n');
  };
  walk(doc.body);
  return out
    .join('')
    .split('\n')
    .map((l) => l.trim())
    .join('\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

type Loaded = { state: 'loading' } | { state: 'ready'; text: string } | { state: 'failed'; code: string };

function LicenseText({ kind }: { kind: LicenseKind }) {
  const { t, bridge } = useApp();
  const [loaded, setLoaded] = useState<Loaded>({ state: 'loading' });
  useEffect(() => {
    let alive = true;
    bridge
      .licenseText(kind)
      .then((text) => alive && setLoaded({ state: 'ready', text: kind === 'rust_std' ? htmlToText(text) : text.trimEnd() }))
      .catch((e: unknown) => alive && setLoaded({ state: 'failed', code: e instanceof Error ? e.message : 'ng-ui-open-failed' }));
    return () => {
      alive = false;
    };
  }, [bridge, kind]);
  if (loaded.state === 'loading') {
    return (
      <p className="licenses-status" role="status">
        <Spinner size={16} />
        {t.t('ng-ui-licenses-loading')}
      </p>
    );
  }
  if (loaded.state === 'failed') {
    return (
      <p className="licenses-status" role="status">
        {t.t(loaded.code === 'ng-ui-licenses-missing' ? 'ng-ui-licenses-missing' : 'ng-ui-open-failed')}
      </p>
    );
  }
  return (
    // fokussierbar, damit sich der lange Text mit der Tastatur rollen lässt
    <pre className="licenses-text" tabIndex={0} data-testid={`licenses-${kind}`}>
      {loaded.text}
    </pre>
  );
}

export function LicensesDialog({ onClose }: { onClose: () => void }) {
  const { t } = useApp();
  return (
    <ModalShell onClose={onClose} size="lg" className="licenses-modal">
      <DialogHeader title={t.t('ng-ui-licenses-title')} subtitle={t.t('ng-ui-licenses-subtitle')} onClose={onClose} />
      <Tabs className="licenses-tabs">
        <TabList className="licenses-tablist" aria-label={t.t('ng-ui-licenses-title')}>
          {KINDS.map((k) => (
            <Tab key={k.kind} id={k.kind} className="licenses-tab">
              {t.t(k.msg)}
            </Tab>
          ))}
        </TabList>
        {KINDS.map((k) => (
          <TabPanel key={k.kind} id={k.kind} className="licenses-panel">
            <LicenseText kind={k.kind} />
          </TabPanel>
        ))}
      </Tabs>
    </ModalShell>
  );
}
