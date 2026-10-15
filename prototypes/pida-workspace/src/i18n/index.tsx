/** Kleine i18n-Schicht: `t(key, vars)`, `tn(base, n)` für Einzahl/Mehrzahl, Sprache aus den Einstellungen. */
import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { I18nProvider } from 'react-aria-components';
import type { Lang } from '../model/types';
import { de, type MsgKey } from './de';
import { en } from './en';

export type { MsgKey };
export type Vars = Record<string, string | number>;

const CATALOGS: Record<Lang, Record<MsgKey, string>> = { de, en };

export function translate(lang: Lang, key: MsgKey, vars?: Vars): string {
  const raw = CATALOGS[lang][key] ?? de[key] ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w[\w-]*)\}/g, (m, name: string) => (name in vars ? String(vars[name]) : m));
}

export interface T {
  lang: Lang;
  (key: MsgKey, vars?: Vars): string;
  /** Einzahl/Mehrzahl: `base` ohne Endung, Schlüssel `${base}_one` / `${base}_other`, Variable `n`. */
  n: (base: string, n: number, vars?: Vars) => string;
}

function makeT(lang: Lang): T {
  const fn = ((key: MsgKey, vars?: Vars) => translate(lang, key, vars)) as T;
  fn.lang = lang;
  fn.n = (base, n, vars) => translate(lang, `${base}_${n === 1 ? 'one' : 'other'}` as MsgKey, { n, ...vars });
  return fn;
}

const I18nContext = createContext<T>(makeT('de'));

export function LangProvider({ lang, children }: { lang: Lang; children: ReactNode }) {
  const t = useMemo(() => makeT(lang), [lang]);
  return (
    <I18nContext.Provider value={t}>
      <I18nProvider locale={lang === 'de' ? 'de-AT' : 'en-GB'}>{children}</I18nProvider>
    </I18nContext.Provider>
  );
}

export function useT(): T {
  return useContext(I18nContext);
}
