/**
 * Suche der Offline-Hilfe: BM25 mit grober Grundform (Endungen, Deutsch und Englisch), Synonymen der Alltagssprache und
 * Tippfehler-Toleranz über Präfixe; Fehlerartikel zählen weniger, außer die Frage klingt nach einem Fehler (sonst
 * verdrängen die vielen Codes die allgemeinen Erklärungen) – erprobt im Machbarkeitstest (prototypes/pida-chat-spike).
 */
import type { Article } from './articles';

type HelpLang = 'de' | 'en';

const STOP_DE = new Set(
  `der die das den dem des ein eine einen einem einer und oder aber ist sind war wie was wer wo wann warum ich mich mir
mein meine sie ihr ihre es er wir uns kann können darf muss mit von zu zum zur im in an auf aus bei für nicht kein keine
auch noch nur so als wenn dann dass ob am beim vom bis über unter nach vor durch gibt habe hat haben wird werden wurde
ja nein bitte`.split(/\s+/),
);

const STOP_EN = new Set(
  `the a an of to in on at for from by with and or but is are was were be been it its this that these those how what
who where when why which can could do does did not no my me you your we our i if then than as so there have has had
will would should into about yes please`.split(/\s+/),
);

/** Alltagswörter → Begriffe der Hilfe. */
const SYNONYMS_DE: Record<string, string[]> = {
  handysignatur: ['id', 'austria'],
  'handy-signatur': ['id', 'austria'],
  unterschrift: ['signatur'],
  unterschreiben: ['signieren'],
  unterzeichnen: ['signieren'],
  zertifikatsdatei: ['keystore'],
  p12: ['keystore', 'pkcs'],
  pfx: ['keystore', 'pkcs'],
  bürgerkarte: ['chipkarte'],
  karte: ['chipkarte'],
  kennwort: ['passwort'],
  gültig: ['prüfen'],
  echt: ['prüfen'],
  mac: ['macos'],
  iphone: ['ios'],
  handy: ['android', 'ios', 'app'],
  adobe: ['adobe', 'acrobat', 'reader'],
};

const SYNONYMS_EN: Record<string, string[]> = {
  handysignatur: ['id', 'austria'],
  'mobile-signature': ['id', 'austria'],
  phone: ['id', 'austria', 'app'],
  sign: ['signature', 'signing'],
  signed: ['signature'],
  esignature: ['signature'],
  card: ['smart', 'card'],
  p12: ['keystore', 'pkcs'],
  pfx: ['keystore', 'pkcs'],
  certificate: ['certificate', 'keystore'],
  passphrase: ['password'],
  valid: ['verification', 'verify'],
  genuine: ['verification', 'verify'],
  check: ['verify', 'verification'],
  mac: ['macos'],
  iphone: ['ios'],
  adobe: ['adobe', 'acrobat', 'reader'],
};

const LANGS: Record<HelpLang, { stop: Set<string>; synonyms: Record<string, string[]>; suffixes: string[] }> = {
  de: { stop: STOP_DE, synonyms: SYNONYMS_DE, suffixes: ['ungen', 'ern', 'en', 'er', 'es', 'em', 'e', 'n', 's'] },
  en: { stop: STOP_EN, synonyms: SYNONYMS_EN, suffixes: ['ations', 'ation', 'ings', 'ing', 'ies', 'ed', 'es', 's'] },
};

/** Grobe Grundform: bekannte Endung ab, wenn mindestens vier Zeichen bleiben. */
export function stem(word: string, lang: HelpLang = 'de'): string {
  for (const suf of LANGS[lang].suffixes) if (word.endsWith(suf) && word.length - suf.length >= 4) return word.slice(0, -suf.length);
  return word;
}

export function tokens(text: string, lang: HelpLang = 'de'): string[] {
  const { stop, synonyms } = LANGS[lang];
  const words = (text.toLowerCase().match(/[a-zäöüß0-9]+(?:-[a-zäöüß0-9]+)*/g) ?? []).flatMap((w) => [w, ...(w.includes('-') ? w.split('-') : [])]);
  return words.filter((w) => w.length > 1 && !stop.has(w)).flatMap((w) => [stem(w, lang), ...(synonyms[w] ?? []).map((x) => stem(x, lang))]);
}

const CODE_LIKE = /\b(backend|atrust|fido2|keystore|cms|sigblock|render|placement|pdf|pades|ng)-[a-z0-9_-]+/;
const ERRORISH = /fehler|fehlgeschlagen|meldung|kaputt|abgebrochen|geht nicht|funktioniert nicht|verweiger|sagt|zeigt|error|fail|message|broken|cancel|not working|doesn't work|refuse|declin|says|shows/;

export interface Hit {
  article: Article;
  score: number;
}

export class HelpIndex {
  private readonly docs: string[][];
  private readonly df = new Map<string, number>();
  private readonly avg: number;

  constructor(
    readonly articles: Article[],
    private readonly lang: HelpLang = 'de',
  ) {
    // Titel doppelt gewichtet
    this.docs = articles.map((a) => [...tokens(a.title, lang), ...tokens(a.title, lang), ...tokens(a.body, lang)]);
    for (const d of this.docs) for (const t of new Set(d)) this.df.set(t, (this.df.get(t) ?? 0) + 1);
    this.avg = this.docs.reduce((n, d) => n + d.length, 0) / Math.max(1, this.docs.length);
  }

  search(query: string, limit = 8): Hit[] {
    const q = tokens(query, this.lang);
    if (q.length === 0) return [];
    const lower = query.toLowerCase();
    const errorish = CODE_LIKE.test(lower) || ERRORISH.test(lower);
    const n = this.docs.length;
    const hits: Hit[] = [];
    this.docs.forEach((d, i) => {
      let s = 0;
      for (const t of q) {
        // Tippfehler und Wortanfänge („signa“ während des Tippens): Präfix ab 4 Zeichen
        const f = d.filter((w) => w === t || (t.length >= 4 && w.startsWith(t))).length;
        if (!f) continue;
        const df = this.df.get(t) ?? 1;
        const idf = Math.log(1 + (n - df + 0.5) / (df + 0.5));
        s += (idf * f * 2.4) / (f + 1.4 * (0.25 + (0.75 * d.length) / this.avg));
      }
      const article = this.articles[i];
      if (!article || s <= 0) return;
      if (article.topic === 'fehler') {
        if (article.code && lower.includes(article.code)) s *= 4;
        else if (!errorish) s *= 0.35;
      }
      hits.push({ article, score: s });
    });
    return hits.sort((a, b) => b.score - a.score).slice(0, limit);
  }
}
