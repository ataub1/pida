/**
 * Offline-Hilfe (`HELP_AVAILABLE`): Seitenleiste bzw. Blatt mit Suche während des
 * Tippens, Vorschlägen passend zur aktuellen Ansicht, Themen und Artikeln. Einstiege: „?“ in der Kopfzeile, F1,
 * „Was bedeutet das?“ an Fehlerkarten (öffnet den Artikel zum Code). Keine Anfrage nach außen.
 */
import { useMemo, useState } from 'react';
import { Button as AriaButton, Input, SearchField } from 'react-aria-components';
import { useApp } from '../../app/context';
import type { ViewState } from '../../core/types';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconArrowLeft, IconChevronRight, IconClose, IconInfo, IconSearch } from '../../ui/Icons';
import { allArticles, type Article, type TopicId } from './articles';
import { errorId } from './codes';
import { Markdown } from './markdown';
import { HelpIndex } from './search';
import './help.css';

const TOPICS: { id: TopicId; label: string }[] = [
  { id: 'pida', label: 'ng-ui-shell-help-topic-pida' },
  { id: 'id-austria', label: 'ng-ui-shell-help-topic-id-austria' },
  { id: 'pdf', label: 'ng-ui-shell-help-topic-pdf' },
  { id: 'recht', label: 'ng-ui-shell-help-topic-recht' },
  { id: 'fehler', label: 'ng-ui-shell-help-topic-fehler' },
];

/** Suchanfragen für „Passend zu dieser Ansicht“. */
function contextQueries(view: ViewState): string[] {
  switch (view.step.step) {
    case 'signing':
      return ['Vergleichswert', 'Keine Benachrichtigung in der App'];
    case 'output':
      return ['Signatur prüfen', 'Adobe Gültigkeit unbekannt'];
    case 'warning':
      return ['zertifiziertes Dokument', 'Signaturfelder'];
    default:
      if (view.batch) return ['Stapel mehrere PDFs', 'Signiermittel'];
      if (view.document) return ['unsichtbar signieren', 'Signaturfelder', 'Signiermittel'];
      return ['Signiermittel', 'Stapel mehrere PDFs', 'Umstieg von PDF-Over'];
  }
}

type Screen = { kind: 'home' } | { kind: 'topic'; topic: TopicId } | { kind: 'article'; id: string };

export function HelpPanel({ target, onClose }: { target: string | null; onClose: () => void }) {
  const { t, view, bridge, toast } = useApp();
  const index = useMemo(() => new HelpIndex(allArticles(t)), [t]);
  const byId = useMemo(() => new Map(index.articles.map((a) => [a.id, a])), [index]);
  const [query, setQuery] = useState('');
  const [stack, setStack] = useState<Screen[]>(() => [{ kind: 'home' }, ...(target && byId.has(target) ? [{ kind: 'article' as const, id: target }] : [])]);
  const screen = stack[stack.length - 1] ?? { kind: 'home' };
  const go = (next: Screen) => {
    setQuery('');
    setStack((s) => [...s, next]);
  };
  const back = () => setStack((s) => (s.length > 1 ? s.slice(0, -1) : s));
  const openLink = (url: string) => void bridge.openUrl(url).catch(() => toast(t.t('ng-ui-open-failed'), 'error'));

  const q = query.trim();
  const hits = q ? index.search(q) : [];
  const suggestions = useMemo(() => {
    const seen = new Set<string>();
    const out: Article[] = [];
    if (view.step.step === 'error') {
      const a = byId.get(`fehler:${errorId(view.step.code)}`);
      if (a) out.push(a);
    }
    for (const cq of contextQueries(view)) {
      const hit = index.search(cq, 4).find((h) => h.article.topic !== 'fehler')?.article;
      if (hit && !seen.has(hit.id) && !out.includes(hit)) {
        seen.add(hit.id);
        out.push(hit);
      }
    }
    return out.slice(0, 4);
  }, [index, byId, view]);

  const row = (a: Article) => (
    <li key={a.id}>
      <AriaButton className="help-row" onPress={() => go({ kind: 'article', id: a.id })}>
        <span className="help-row-text">
          <span className="help-row-title">{a.title}</span>
          <span className="help-row-snippet">{snippet(a)}</span>
        </span>
        <IconChevronRight size={16} />
      </AriaButton>
    </li>
  );

  const article = screen.kind === 'article' ? byId.get(screen.id) : undefined;
  const related = article ? index.search(article.title, 4).filter((h) => h.article.id !== article.id).slice(0, 3).map((h) => h.article) : [];

  return (
    <ModalShell onClose={onClose} variant="drawer" size="md" className="help-modal">
      <DialogHeader title={t.t('ng-ui-shell-help-title')} subtitle={t.t('ng-ui-shell-help-subtitle')} onClose={onClose} />
      <div className="help-search">
        <SearchField aria-label={t.t('ng-ui-shell-help-search')} value={query} onChange={setQuery} className="search-field" autoFocus={!target}>
          <IconSearch size={17} className="search-icon" />
          <Input className="input search-input" placeholder={t.t('ng-ui-shell-help-search')} />
          <AriaButton className="search-clear" aria-label={t.t('ng-ui-set-search-clear')}>
            <IconClose size={14} />
          </AriaButton>
        </SearchField>
      </div>
      <div className="help-body">
        {t.lang !== 'de' ? (
          <p className="help-note" role="note">
            <IconInfo size={15} />
            {t.t('ng-ui-shell-help-lang')}
          </p>
        ) : null}
        {q ? (
          <section aria-label={t.t('ng-ui-shell-help-results')}>
            <p className="sr-only" role="status">
              {t.t('ng-ui-set-search-hits', { n: hits.length })}
            </p>
            {hits.length ? <ul className="help-list">{hits.map((h) => row(h.article))}</ul> : <p className="muted">{t.t('ng-ui-shell-help-empty', { q })}</p>}
          </section>
        ) : screen.kind === 'article' && article ? (
          <article className="help-article" aria-labelledby="help-article-title">
            <AriaButton className="link help-back" onPress={back}>
              <IconArrowLeft size={14} />
              {t.t('ng-ui-shell-help-back')}
            </AriaButton>
            <p className="help-eyebrow">{t.t(TOPICS.find((x) => x.id === article.topic)?.label ?? 'ng-ui-shell-help-title')}</p>
            <h3 id="help-article-title" className="help-article-title">
              {article.title}
            </h3>
            <Markdown text={article.body} onLink={openLink} />
            {article.topic === 'recht' ? <p className="help-legal">{t.t('ng-ui-shell-help-legal')}</p> : null}
            {related.length ? (
              <section aria-labelledby="help-related">
                <h4 id="help-related" className="help-section">
                  {t.t('ng-ui-shell-help-related')}
                </h4>
                <ul className="help-list">{related.map(row)}</ul>
              </section>
            ) : null}
          </article>
        ) : screen.kind === 'topic' ? (
          <section aria-labelledby="help-topic">
            <AriaButton className="link help-back" onPress={back}>
              <IconArrowLeft size={14} />
              {t.t('ng-ui-shell-help-back')}
            </AriaButton>
            <h3 id="help-topic" className="help-article-title">
              {t.t(TOPICS.find((x) => x.id === screen.topic)?.label ?? '')}
            </h3>
            {screen.topic === 'fehler' ? <p className="muted">{t.t('ng-ui-shell-help-fehler-hint')}</p> : null}
            <ul className="help-list">{index.articles.filter((a) => a.topic === screen.topic).slice(0, screen.topic === 'fehler' ? 40 : 100).map(row)}</ul>
          </section>
        ) : (
          <>
            {suggestions.length ? (
              <section aria-labelledby="help-context">
                <h3 id="help-context" className="help-section">
                  {t.t('ng-ui-shell-help-context')}
                </h3>
                <ul className="help-list">{suggestions.map(row)}</ul>
              </section>
            ) : null}
            <section aria-labelledby="help-topics">
              <h3 id="help-topics" className="help-section">
                {t.t('ng-ui-shell-help-topics')}
              </h3>
              <ul className="help-topics">
                {TOPICS.map((tp) => (
                  <li key={tp.id}>
                    <AriaButton className="help-topic" onPress={() => go({ kind: 'topic', topic: tp.id })}>
                      {t.t(tp.label)}
                      <span className="help-count">{index.articles.filter((a) => a.topic === tp.id).length}</span>
                    </AriaButton>
                  </li>
                ))}
              </ul>
            </section>
          </>
        )}
      </div>
    </ModalShell>
  );
}

/** erster Satz bzw. erste Zeile ohne Markdown-Zeichen */
function snippet(a: Article): string {
  const plain = a.body.replace(/\*\*|`/g, '').replace(/^\s*[-\d.]+\s+/gm, '').replace(/\s+/g, ' ').trim();
  const first = plain.split(/(?<=[.!?])\s/)[0] ?? plain;
  return first.length > 140 ? `${first.slice(0, 137)}…` : first;
}
