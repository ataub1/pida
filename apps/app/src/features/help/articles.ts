/**
 * Offline-Hilfe: Artikel aus der Wissensbasis (`content/de/*.md`, je Abschnitt `## …` ein Artikel) und je
 * Fehlercode ein Artikel aus dem Meldungskatalog der App (Alltagstext `.text`, Titel, Aktion, genaue Meldung) – so
 * passt die Hilfe immer zu den Meldungen. Alles liegt in der App; keine Anfrage nach außen.
 */
import { SOURCES } from '../../i18n/catalog';
import type { Translator } from '../../i18n/catalog';
import { ERROR_FAMILIES } from './codes';
import pida from './content/de/01-pida.md?raw';
import recht from './content/de/02-signaturarten-recht.md?raw';
import idAustria from './content/de/03-id-austria.md?raw';
import pdf from './content/de/04-pdf-signaturen.md?raw';

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

const DOCS: { topic: TopicId; text: string }[] = [
  { topic: 'pida', text: pida },
  { topic: 'id-austria', text: idAustria },
  { topic: 'pdf', text: pdf },
  { topic: 'recht', text: recht },
];

export function slug(text: string): string {
  return text
    .toLowerCase()
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

/** Abschnitte `## Titel` eines Dokuments; der Kopf (`# …`, Entwurfshinweis) entfällt. */
function sections(topic: TopicId, text: string): Article[] {
  return text
    .split(/^## /m)
    .slice(1)
    .map((part) => {
      const [head = '', ...rest] = part.split('\n');
      const title = head.trim();
      return { id: `${topic}:${slug(title)}`, topic, title, body: rest.join('\n').trim() };
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
  if (action) lines.push('', `**Was Sie tun können:** ${action}.`);
  // die genaue Meldung (mit Platzhaltern) nur zur Einordnung der „Technischen Details“
  if (exact && exact !== text) lines.push('', `In den „Technischen Details“ steht die genaue Meldung, z. B.: ${exact.replace(/\{\s*\$\w+\s*\}|\{\$\w+\}/g, '…')}`);
  lines.push('', `Code: \`${code}\``);
  return { id: `fehler:${code}`, topic: 'fehler', title: title ?? shortTitle(text), body: lines.join('\n'), code };
}

/** Titel aus dem ersten Satz der Meldung, wenn der Katalog keinen hat. */
function shortTitle(text: string): string {
  const first = (text.split(/(?<=[.!?])\s/)[0] ?? text).replace(/[.!]$/, '').replace(/\{\s*\$\w+\s*\}/g, '…');
  return first.length > 70 ? `${first.slice(0, 67)}…` : first;
}

let cache: { lang: string; articles: Article[] } | null = null;

/** Alle Artikel (Fehlerartikel in der Sprache der Oberfläche, Wissensbasis vorerst nur Deutsch). */
export function allArticles(t: Translator): Article[] {
  if (cache && cache.lang === t.lang) return cache.articles;
  const articles = [...DOCS.flatMap((d) => sections(d.topic, d.text)), ...errorCodes().flatMap((c) => errorArticle(t, c) ?? [])];
  cache = { lang: t.lang, articles };
  return articles;
}
