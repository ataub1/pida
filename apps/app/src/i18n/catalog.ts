/**
 * Fluent-Kataloge der Oberfläche (Plan §7.5): **dieselben Dateien** wie Kern und CLI
 * (`crates/i18n/locales/{de,en}/pdfover.ftl` und `pida.ftl`, zur Build-Zeit eingebunden). Die
 * UI-Texte (IDs `ng-ui-*`) stehen in `pida.ftl` – eine Quelle für alle Oberflächen.
 *
 * Semantik wie `pida_i18n::Catalog::format`: gewählte Sprache → DE → EN → `!id!`; Zugriffstasten
 * (`.accesskey`) ohne Sprach-Fallback; keine Unicode-Isolationszeichen.
 */
import { FluentBundle, FluentResource, type FluentVariable } from '@fluent/bundle';
import type { Lang } from '../core/types';
import deCore from '@locales/de/pdfover.ftl?raw';
import deNg from '@locales/de/pida.ftl?raw';
import enCore from '@locales/en/pdfover.ftl?raw';
import enNg from '@locales/en/pida.ftl?raw';

/** Quellen je Sprache (Reihenfolge: erzeugter Katalog, eigene Texte inkl. UI-Texte `ng-ui-*`). */
export const SOURCES: Record<Lang, { name: string; text: string }[]> = {
  de: [
    { name: 'de/pdfover.ftl', text: deCore },
    { name: 'de/pida.ftl', text: deNg },
  ],
  en: [
    { name: 'en/pdfover.ftl', text: enCore },
    { name: 'en/pida.ftl', text: enNg },
  ],
};

/** Argumente einer Nachricht. */
export type Args = Record<string, string | number>;

/** Fehler beim Laden (Syntax, doppelte IDs) – im Test geprüft, zur Laufzeit nur gemeldet. */
export interface CatalogIssue {
  source: string;
  message: string;
}

function buildBundle(lang: Lang, issues: CatalogIssue[]): FluentBundle {
  const bundle = new FluentBundle(lang, { useIsolating: false });
  for (const { name, text } of SOURCES[lang]) {
    const resource = new FluentResource(text);
    // Doppelte IDs wären ein Katalogfehler (Test); zur Laufzeit gewinnt die erste.
    for (const error of bundle.addResource(resource)) {
      issues.push({ source: name, message: error.message });
    }
  }
  return bundle;
}

/** Übersetzer für eine Sprache. */
export class Translator {
  readonly lang: Lang;
  private readonly chain: FluentBundle[];
  private readonly own: FluentBundle;

  constructor(lang: Lang, bundles: Record<Lang, FluentBundle>) {
    this.lang = lang;
    const order: Lang[] = [lang, ...(['de', 'en'] as Lang[]).filter((l) => l !== lang)];
    this.chain = order.map((l) => bundles[l]);
    this.own = bundles[lang];
  }

  /** Nachricht vorhanden (mit Fallback)? */
  has(id: string): boolean {
    return this.chain.some((b) => b.getMessage(id)?.value != null);
  }

  /** Nachricht in genau dieser Sprache vorhanden (ohne Fallback)? */
  hasOwn(id: string): boolean {
    return this.own.getMessage(id)?.value != null;
  }

  /** Formatiert `id`; fehlt die Nachricht, `!id!` (wie PDF-Over `Messages.getString`). */
  t(id: string, args?: Args): string {
    for (const bundle of this.chain) {
      const message = bundle.getMessage(id);
      if (message?.value) {
        const errors: Error[] = [];
        return bundle.formatPattern(message.value, plainArgs(args), errors);
      }
    }
    return `!${id}!`;
  }

  /** Attribut (z. B. `accesskey`) in der gewählten Sprache. */
  attr(id: string, name: string): string | null {
    const attr = this.own.getMessage(id)?.attributes[name];
    if (!attr) return null;
    return this.own.formatPattern(attr, undefined, []);
  }

  /** Zugriffstaste (ein Zeichen, klein) oder `null`. */
  accessKey(id: string): string | null {
    const key = this.attr(id, 'accesskey');
    return key && key.length === 1 ? key.toLowerCase() : null;
  }

  /**
   * Übersetzt einen Fehler-/Meldungscode des Kerns: zuerst die ID selbst, dann mit „-“ statt „.“
   * (Konvention der Kataloge), sonst `fallback` mit dem Code als Detail.
   */
  code(code: string, params: Args = {}, fallback = 'ng-ui-error-unknown'): string {
    if (this.has(code)) return this.t(code, params);
    const dashed = code.replace(/\./g, '-');
    if (this.has(dashed)) return this.t(dashed, params);
    return this.t(fallback, { ...params, code });
  }
}

/**
 * Zahlen als einfacher Text (ohne Tausendertrennung), wie `fluent-bundle` in Rust – sonst würde
 * z. B. ein Security-Layer-Code 6002 als „6,002“ erscheinen.
 */
function plainArgs(args: Args | undefined): Record<string, FluentVariable> | undefined {
  if (!args) return undefined;
  const out: Record<string, FluentVariable> = {};
  for (const [k, v] of Object.entries(args)) out[k] = typeof v === 'number' ? String(v) : v;
  return out;
}

let cache: { bundles: Record<Lang, FluentBundle>; issues: CatalogIssue[] } | null = null;

/** Gebündelte Kataloge (einmal je Prozess). */
export function catalogs(): { bundles: Record<Lang, FluentBundle>; issues: CatalogIssue[] } {
  if (!cache) {
    const issues: CatalogIssue[] = [];
    cache = { bundles: { de: buildBundle('de', issues), en: buildBundle('en', issues) }, issues };
    for (const issue of issues) console.warn(`i18n: ${issue.source}: ${issue.message}`);
  }
  return cache;
}

const translators = new Map<Lang, Translator>();

/** Übersetzer für `lang` (zwischengespeichert). */
export function translator(lang: Lang): Translator {
  let t = translators.get(lang);
  if (!t) {
    t = new Translator(lang, catalogs().bundles);
    translators.set(lang, t);
  }
  return t;
}
