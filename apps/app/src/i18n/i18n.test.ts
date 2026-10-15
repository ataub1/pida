/**
 * i18n: Kataloge laden fehlerfrei, die UI-Texte (`ng-ui-*` in `crates/i18n/locales/{de,en}/pdfover-ng.ftl`)
 * sind in DE und EN vollständig und mit denselben Variablen vorhanden, jede im Quelltext verwendete
 * Meldungs-ID existiert in beiden Sprachen, Fallback und Link-Markup verhalten sich wie `pdfover-i18n`.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FluentResource } from '@fluent/bundle';
import { describe, expect, it } from 'vitest';
import { Translator, catalogs, translator, SOURCES } from './catalog';
import { parseMarkup, toPlain } from './markup';

const srcDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) return name === 'test' || name === 'fixtures' ? [] : sourceFiles(full);
    return /\.(ts|tsx)$/.test(name) && !/\.test\.tsx?$/.test(name) ? [full] : [];
  });
}

function messageIds(text: string): Map<string, Set<string>> {
  const out = new Map<string, Set<string>>();
  const resource = new FluentResource(text);
  for (const entry of resource.body) {
    if (!('id' in entry) || entry.id.startsWith('-')) continue;
    const vars = new Set<string>();
    const raw = JSON.stringify(entry);
    for (const m of raw.matchAll(/"name":"([A-Za-z0-9_]+)"\}/g)) vars.add(m[1] ?? '');
    out.set(entry.id, vars);
  }
  return out;
}

const uiIds = (text: string) => new Map([...messageIds(text)].filter(([id]) => id.startsWith('ng-ui-')));
const appDe = uiIds(SOURCES.de.find((s) => s.name === 'de/pdfover-ng.ftl')?.text ?? '');
const appEn = uiIds(SOURCES.en.find((s) => s.name === 'en/pdfover-ng.ftl')?.text ?? '');

describe('Kataloge', () => {
  it('laden ohne Syntaxfehler und ohne doppelte IDs', () => {
    expect(catalogs().issues).toEqual([]);
  });

  it('UI-Texte (ng-ui-*): gleiche IDs und Variablen in DE und EN, eine Quelle (pdfover-ng.ftl)', () => {
    expect([...appDe.keys()].sort()).toEqual([...appEn.keys()].sort());
    for (const [id, vars] of appDe) {
      expect(id.startsWith('ng-ui-'), id).toBe(true);
      expect([...vars].sort(), id).toEqual([...(appEn.get(id) ?? [])].sort());
    }
    expect(appDe.size).toBeGreaterThan(100);
  });

  it('jede im Quelltext verwendete Meldungs-ID existiert in DE und EN', () => {
    const de = translator('de');
    const en = translator('en');
    const prefixes = new Set<string>();
    for (const l of ['de', 'en'] as const) {
      for (const s of SOURCES[l]) for (const id of messageIds(s.text).keys()) prefixes.add(id.split('-')[0] ?? '');
    }
    const used = new Set<string>();
    for (const file of sourceFiles(srcDir)) {
      const text = readFileSync(file, 'utf8');
      for (const m of text.matchAll(/['"`]([A-Za-z_][A-Za-z0-9_]*-[A-Za-z0-9_.-]+)['"`]/g)) {
        const id = m[1] ?? '';
        // CSS-Klassen und Element-IDs sind keine Meldungs-IDs
        if (/(className|id|htmlFor|aria-[a-z]+|data-[a-z-]+)=\{?$/.test(text.slice(Math.max(0, (m.index ?? 0) - 16), m.index ?? 0))) continue;
        if (prefixes.has(id.split('-')[0] ?? '') && !id.includes('${')) used.add(id);
      }
    }
    // dynamische IDs: Profilnamen
    for (const p of ['SIGNATURBLOCK_SMALL', 'AMTSSIGNATURBLOCK', 'BASE_LOGO', 'INVISIBLE']) used.add(`simple_config-${p}`);
    expect(used.size).toBeGreaterThan(150);
    const missing = [...used].filter((id) => !(de.hasOwn(id) && en.hasOwn(id)));
    expect(missing).toEqual([]);
  });

  it('Shell-Fehlercodes der Rust-Seite sind übersetzt', () => {
    for (const id of ['ng-ui-core-unavailable', 'ng-ui-event-invalid', 'ng-ui-open-failed']) {
      expect(translator('de').hasOwn(id)).toBe(true);
      expect(translator('en').hasOwn(id)).toBe(true);
    }
  });
});

describe('Translator', () => {
  it('formatiert mit Argumenten ohne Isolationszeichen', () => {
    expect(translator('de').t('positioning-page', { arg1: 3, arg2: 5 })).toBe('Seite 3 von 5');
    expect(translator('en').t('error-SLError', { arg1: 6002, arg2: 'Abbruch' })).toBe('Operation aborted by signature server (6002):\nAbbruch');
  });

  it('Fallback DE → EN → !id!', () => {
    const t = translator('en');
    expect(t.t('gibt-es-nicht')).toBe('!gibt-es-nicht!');
    expect(t.has('gibt-es-nicht')).toBe(false);
  });

  it('Zugriffstasten ohne Sprach-Fallback, ein Zeichen', () => {
    expect(translator('en').accessKey('dataSourceSelection-browse')).toBe('b');
    expect(translator('de').accessKey('positioning-sign')).toBe('s');
    expect(translator('de').accessKey('output-success_message')).toBeNull();
  });

  it('Fehlercodes: ID, Punkt → Strich, sonst allgemeine Meldung', () => {
    const t = translator('de');
    expect(t.code('error-IOError')).toBe('Ein-/Ausgabe-Fehler');
    expect(t.code('error.IOError')).toBe('Ein-/Ausgabe-Fehler');
    expect(t.code('ganz.neu')).toBe('Unerwarteter Fehler (ganz.neu).');
  });

  it('eigener Translator aus Bündeln', () => {
    const t = new Translator('en', catalogs().bundles);
    expect(t.lang).toBe('en');
    expect(t.t('common-Ok')).toBe('OK');
  });
});

describe('Link-Markup', () => {
  it('zerlegt wie pdfover_i18n::markup', () => {
    expect(parseMarkup('Sie können das signierte Dokument <a>hier</a> öffnen.')).toEqual([
      { kind: 'text', text: 'Sie können das signierte Dokument ' },
      { kind: 'link', text: 'hier', href: null },
      { kind: 'text', text: ' öffnen.' },
    ]);
    expect(parseMarkup('Mail <a href="mailto:x@y.at">x@y.at</a>')).toEqual([
      { kind: 'text', text: 'Mail ' },
      { kind: 'link', text: 'x@y.at', href: 'mailto:x@y.at' },
    ]);
    expect(parseMarkup("<a href='https://a.at'>A</a>")).toEqual([{ kind: 'link', text: 'A', href: 'https://a.at' }]);
  });

  it('unvollständiges Markup und andere Tags bleiben Text', () => {
    expect(parseMarkup('a <abbr>b</abbr> <a>offen')).toEqual([{ kind: 'text', text: 'a <abbr>b</abbr> <a>offen' }]);
    expect(toPlain('x <a>y</a> z')).toBe('x y z');
  });
});
