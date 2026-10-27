/**
 * Fluent-Kataloge der Oberfläche (Plan §7.5): **dieselben Dateien** wie Kern und CLI
 * (`crates/i18n/locales/{de,en}/*.ftl`, zur Build-Zeit eingebunden). Alle Dateien einer Sprache bilden
 * **ein** Bündel (doppelte IDs sind ein Ladefehler):
 *
 * - `pdfover.ftl` – erzeugt aus den PDF-Over-Katalogen,
 * - `pida.ftl` – Meldungen des Kerns und UI-Texte der bisherigen Oberfläche (`ng-ui-*`),
 * - `ui-workspace.ftl` – UI-Texte der neuen Oberfläche, Paket UI-A (Bereiche `shell`, `start`, `ws`,
 *   `insp`, `sc`, `fin`, `batch`; ADR 0004 §4.5),
 * - `ui-tools.ftl` – UI-Texte der neuen Oberfläche, Paket UI-B (`set`, `vf`, `pal`, `sh`, `ob`, `recent`).
 *
 * Semantik wie `pida_i18n::Catalog::format`: gewählte Sprache → DE → EN → `!id!`; Zugriffstasten
 * (`.accesskey`) ohne Sprach-Fallback; keine Unicode-Isolationszeichen.
 */
import { FluentBundle, FluentNumber, FluentResource, type FluentVariable } from '@fluent/bundle';
import type { Lang } from '../core/types';
import deCore from '@locales/de/pdfover.ftl?raw';
import deNg from '@locales/de/pida.ftl?raw';
import deTools from '@locales/de/ui-tools.ftl?raw';
import deWorkspace from '@locales/de/ui-workspace.ftl?raw';
import enCore from '@locales/en/pdfover.ftl?raw';
import enNg from '@locales/en/pida.ftl?raw';
import enTools from '@locales/en/ui-tools.ftl?raw';
import enWorkspace from '@locales/en/ui-workspace.ftl?raw';

/** Quellen je Sprache (Reihenfolge: erzeugter Katalog, eigene Texte, UI-Texte der neuen Oberfläche). */
export const SOURCES: Record<Lang, { name: string; text: string }[]> = {
  de: [
    { name: 'de/pdfover.ftl', text: deCore },
    { name: 'de/pida.ftl', text: deNg },
    { name: 'de/ui-workspace.ftl', text: deWorkspace },
    { name: 'de/ui-tools.ftl', text: deTools },
  ],
  en: [
    { name: 'en/pdfover.ftl', text: enCore },
    { name: 'en/pida.ftl', text: enNg },
    { name: 'en/ui-workspace.ftl', text: enWorkspace },
    { name: 'en/ui-tools.ftl', text: enTools },
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
   * Übersetzt einen Fehler-/Meldungscode des Kerns für die Anzeige: zuerst die ID selbst, dann mit „-“ statt „.“
   * (Konvention der Kataloge), sonst `fallback` mit dem Code als Detail. Hat die Meldung einen Alltagstext (`.text`,
   * ohne technische Ursache), gilt er – die vollständige Meldung bleibt CLI, Protokoll und „Technische Details“.
   */
  code(code: string, params: Args = {}, fallback = 'ng-ui-error-unknown'): string {
    const dashed = code.replace(/\./g, '-');
    for (const id of [code, dashed]) {
      if (this.has(id)) return this.attr(id, 'text') ?? this.t(id, params);
    }
    // kein Code, sondern roher Text (z. B. eine technische Fehlermeldung): nicht anzeigen
    if (!/^[a-z0-9][a-z0-9_.-]*$/i.test(code) && fallback === 'ng-ui-error-unknown') return this.t('ng-ui-error-unexpected', params);
    return this.t(fallback, { ...params, code });
  }
}

/**
 * Zahlen ohne Tausendertrennung, wie `fluent-bundle` in Rust – sonst würde z. B. ein Security-Layer-Code
 * 6002 als „6,002“ erscheinen. Sie bleiben Fluent-Zahlen, damit Einzahl/Mehrzahl über Selektoren wirkt
 * (`{ $n -> [one] … *[other] … }`, ADR 0004 §4.5); ganzzahlige Texte ohne führende Null (Parameter der
 * Kernmeldungen kommen als Text) gelten ebenfalls als Zahl.
 */
function plainArgs(args: Args | undefined): Record<string, FluentVariable> | undefined {
  if (!args) return undefined;
  const out: Record<string, FluentVariable> = {};
  for (const [k, v] of Object.entries(args)) {
    const n = typeof v === 'number' ? v : /^(0|[1-9]\d{0,14})$/.test(v) ? Number(v) : null;
    out[k] = n !== null && Number.isFinite(n) ? new FluentNumber(n, { useGrouping: false, maximumFractionDigits: 6 }) : String(v);
  }
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
