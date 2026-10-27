/**
 * Offline-Hilfe: Artikel aus der Wissensbasis (`content/de/*.md`, je Abschnitt `## …` ein Artikel) und je
 * Fehlercode ein Artikel aus dem Meldungskatalog der App (Alltagstext `.text`, Titel, Aktion, genaue Meldung) – so
 * passt die Hilfe immer zu den Meldungen. Alles liegt in der App; keine Anfrage nach außen.
 */
import { SOURCES } from '../../i18n/catalog';
import type { Translator } from '../../i18n/catalog';
import { ERROR_FAMILIES } from './codes';
import pidaDe from './content/de/01-pida.md?raw';
import rechtDe from './content/de/02-signaturarten-recht.md?raw';
import idAustriaDe from './content/de/03-id-austria.md?raw';
import pdfDe from './content/de/04-pdf-signaturen.md?raw';
import pidaEn from './content/en/01-pida.md?raw';
import rechtEn from './content/en/02-signaturarten-recht.md?raw';
import idAustriaEn from './content/en/03-id-austria.md?raw';
import pdfEn from './content/en/04-pdf-signaturen.md?raw';

export type TopicId = 'pida' | 'recht' | 'id-austria' | 'pdf' | 'fehler';

export interface Article {
  /** stabile Kennung, z. B. `pida:signiermittel` bzw. `fehler:backend-proxy-auth` */
  id: string;
  topic: TopicId;
  title: string;
  /** Markdown (Absätze, Listen, **fett**, `Code`, Links) */
  body: string;
  /** Code bei Fehlerartikeln */
  code?: string;
}

/** Wissensbasis je Sprache; die englischen Überschriften tragen die Kennung des deutschen Artikels (`{#…}`). */
const DOCS: Record<'de' | 'en', { topic: TopicId; text: string }[]> = {
  de: [
    { topic: 'pida', text: pidaDe },
    { topic: 'id-austria', text: idAustriaDe },
    { topic: 'pdf', text: pdfDe },
    { topic: 'recht', text: rechtDe },
  ],
  en: [
    { topic: 'pida', text: pidaEn },
    { topic: 'id-austria', text: idAustriaEn },
    { topic: 'pdf', text: pdfEn },
    { topic: 'recht', text: rechtEn },
  ],
};

export function slug(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

/** Abschnitte `## Titel` bzw. `## Titel {#kennung}` eines Dokuments; der Kopf (`# …`, Quellenhinweis) entfällt. */
function sections(topic: TopicId, text: string): Article[] {
  return text
    .split(/^## /m)
    .slice(1)
    .map((part) => {
      const [head = '', ...rest] = part.split('\n');
      const m = /^(.*?)\s*\{#([a-z0-9-]+)\}\s*$/.exec(head);
      const title = (m?.[1] ?? head).trim();
      return { id: `${topic}:${m?.[2] ?? slug(title)}`, topic, title, body: rest.join('\n').trim() };
    });
}

/** Fehlercodes aus dem deutschen Katalog (IDs der Familien). */
function errorCodes(): string[] {
  const ids = new Set<string>();
  for (const src of SOURCES.de) {
    for (const m of src.text.matchAll(/^([a-z][a-z0-9_-]*)\s*=/gm)) {
      const id = m[1] ?? '';
      if (ERROR_FAMILIES.test(id)) ids.add(id);
    }
  }
  return [...ids].sort();
}

function errorArticle(t: Translator, code: string): Article | null {
  if (!t.has(code)) return null;
  const text = t.attr(code, 'text') ?? t.t(code);
  const title = t.attr(code, 'title');
  const action = t.attr(code, 'action');
  const exact = t.attr(code, 'text') ? t.t(code) : null;
  const lines = [text];
  if (action) lines.push('', `**${t.t('ng-ui-shell-help-todo')}** ${action}.`);
  // die genaue Meldung (mit Platzhaltern) nur zur Einordnung der „Technischen Details“
  if (exact && exact !== text) lines.push('', t.t('ng-ui-shell-help-exact', { message: exact.replace(/\{\s*\$\w+\s*\}|\{\$\w+\}/g, '…') }));
  lines.push('', `${t.t('ng-ui-shell-help-code')}: \`${code}\``);
  return { id: `fehler:${code}`, topic: 'fehler', title: title ?? shortTitle(text), body: lines.join('\n'), code };
}

/** Titel aus dem ersten Satz der Meldung, wenn der Katalog keinen hat. */
function shortTitle(text: string): string {
  const first = (text.split(/(?<=[.!?])\s/)[0] ?? text).replace(/[.!]$/, '').replace(/\{\s*\$\w+\s*\}/g, '…');
  return first.length > 70 ? `${first.slice(0, 67)}…` : first;
}

let cache: { lang: string; articles: Article[] } | null = null;

/** Alle Artikel in der Sprache der Oberfläche (Fehlercodes aus dem deutschen Katalog, Texte aus dem der Sprache). */
export function allArticles(t: Translator): Article[] {
  if (cache && cache.lang === t.lang) return cache.articles;
  const docs = t.lang === 'en' ? DOCS.en : DOCS.de;
  const articles = [...docs.flatMap((d) => sections(d.topic, d.text)), ...errorCodes().flatMap((c) => errorArticle(t, c) ?? [])];
  cache = { lang: t.lang, articles };
  return articles;
}
