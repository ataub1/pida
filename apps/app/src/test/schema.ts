/**
 * Kleine typisierte Laufzeit-Schemas für den Vertragstest: Jede Prüfung trägt ihren TS-Typ, sodass
 * der Compiler zusätzlich nachweist, dass das Schema **genau** den Typen aus `types.ts` entspricht.
 */

/** Prüffunktion mit Typ. */
export interface Schema<T> {
  (value: unknown, path: string, errors: string[]): void;
  /** nur für die Typableitung */
  readonly __type?: T;
}

/** Abgeleiteter Typ. */
export type Infer<S> = S extends Schema<infer T> ? T : never;

function describe(v: unknown): string {
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'array';
  return typeof v;
}

export const str: Schema<string> = (v, p, e) => {
  if (typeof v !== 'string') e.push(`${p}: string erwartet, ${describe(v)} gefunden`);
};

export const num: Schema<number> = (v, p, e) => {
  if (typeof v !== 'number' || !Number.isFinite(v)) e.push(`${p}: Zahl erwartet, ${describe(v)} gefunden`);
};

export const int: Schema<number> = (v, p, e) => {
  if (typeof v !== 'number' || !Number.isInteger(v) || v < 0) e.push(`${p}: ganze Zahl ≥ 0 erwartet`);
};

export const bool: Schema<boolean> = (v, p, e) => {
  if (typeof v !== 'boolean') e.push(`${p}: boolean erwartet, ${describe(v)} gefunden`);
};

export const unknownValue: Schema<unknown> = () => undefined;

export function lit<const L extends readonly string[]>(...values: L): Schema<L[number]> {
  return (v, p, e) => {
    if (typeof v !== 'string' || !values.includes(v)) e.push(`${p}: eines von ${values.join('|')} erwartet, ${JSON.stringify(v)} gefunden`);
  };
}

export function nullable<T>(s: Schema<T>): Schema<T | null> {
  return (v, p, e) => {
    if (v !== null) s(v, p, e);
  };
}

export function arr<T>(s: Schema<T>): Schema<T[]> {
  return (v, p, e) => {
    if (!Array.isArray(v)) {
      e.push(`${p}: Liste erwartet, ${describe(v)} gefunden`);
      return;
    }
    v.forEach((x, i) => s(x, `${p}[${i}]`, e));
  };
}

export function tuple2<A, B>(a: Schema<A>, b: Schema<B>): Schema<[A, B]> {
  return (v, p, e) => {
    if (!Array.isArray(v) || v.length !== 2) {
      e.push(`${p}: Paar erwartet`);
      return;
    }
    a(v[0], `${p}[0]`, e);
    b(v[1], `${p}[1]`, e);
  };
}

export function record<T>(s: Schema<T>): Schema<Record<string, T>> {
  return (v, p, e) => {
    if (typeof v !== 'object' || v === null || Array.isArray(v)) {
      e.push(`${p}: Objekt erwartet, ${describe(v)} gefunden`);
      return;
    }
    for (const [k, x] of Object.entries(v)) s(x, `${p}.${k}`, e);
  };
}

/**
 * Feld, das TypeScript als **optional** führt (`#[ts(optional = nullable)]`: `feld?: T | null`) – für additive
 * Vertragsfelder nach E0 (Nachträge), damit bestehende UI-Literale gültig bleiben. Der Kern (serde) liefert
 * das Feld trotzdem immer (Wert oder `null`); die Prüfung verlangt es deshalb wie jedes andere.
 */
export interface OptionalSchema<T> extends Schema<T | null> {
  /** nur für die Typableitung (Markierung „optional“) */
  readonly __optional: true;
}

export function optional<T>(s: Schema<T>): OptionalSchema<T> {
  const check: Schema<T | null> = (v, p, e) => {
    if (v !== null) s(v, p, e);
  };
  return check as OptionalSchema<T>;
}

/**
 * Wie `optional`, das Feld darf aber auch ganz fehlen: additive Felder von UI-Ereignissen, die der Kern mit
 * `#[serde(default, skip_serializing_if = "Option::is_none")]` liest und schreibt (z. B.
 * `submit_secret.password_storage`).
 */
export function omittable<T>(s: Schema<T>): OptionalSchema<T> {
  const check = optional(s);
  Object.defineProperty(check, 'omittable', { value: true });
  return check;
}

const isOmittable = (s: Schema<unknown>) => (s as { omittable?: boolean }).omittable === true;

type Props = Record<string, Schema<unknown>>;
type OptionalKeys<P extends Props> = { [K in keyof P]: P[K] extends OptionalSchema<unknown> ? K : never }[keyof P];
type ObjOf<P extends Props> = { [K in Exclude<keyof P, OptionalKeys<P>>]: Infer<P[K]> } & {
  [K in OptionalKeys<P>]?: Infer<P[K]>;
};

function checkProps(props: Props, v: Record<string, unknown>, p: string, e: string[], extra: boolean, skip: string[] = []) {
  for (const [k, s] of Object.entries(props)) {
    if (!(k in v)) {
      if (!isOmittable(s)) e.push(`${p}.${k}: fehlt`);
      continue;
    }
    s(v[k], `${p}.${k}`, e);
  }
  if (!extra) {
    for (const k of Object.keys(v)) {
      if (!(k in props) && !skip.includes(k)) e.push(`${p}.${k}: unbekanntes Feld`);
    }
  }
}

/** Objekt mit genau diesen Feldern. */
export function obj<P extends Props>(props: P): Schema<ObjOf<P>> {
  return (v, p, e) => {
    if (typeof v !== 'object' || v === null || Array.isArray(v)) {
      e.push(`${p}: Objekt erwartet, ${describe(v)} gefunden`);
      return;
    }
    checkProps(props, v as Record<string, unknown>, p, e, false);
  };
}

/** Objekt mit diesen Feldern und beliebigen weiteren (serde `flatten`). */
export function objOpen<P extends Props>(props: P): Schema<ObjOf<P> & { [unknown: string]: unknown }> {
  return (v, p, e) => {
    if (typeof v !== 'object' || v === null || Array.isArray(v)) {
      e.push(`${p}: Objekt erwartet, ${describe(v)} gefunden`);
      return;
    }
    checkProps(props, v as Record<string, unknown>, p, e, true);
  };
}

type TaggedOf<K extends string, M extends Record<string, Props>> = {
  [V in keyof M & string]: { [T in K]: V } & ObjOf<M[V]>;
}[keyof M & string];

/** Intern getaggte Enum (`#[serde(tag = "…")]`). */
export function tagged<K extends string, M extends Record<string, Props>>(tag: K, variants: M): Schema<TaggedOf<K, M>> {
  return (v, p, e) => {
    if (typeof v !== 'object' || v === null || Array.isArray(v)) {
      e.push(`${p}: Objekt erwartet, ${describe(v)} gefunden`);
      return;
    }
    const o = v as Record<string, unknown>;
    const name = o[tag];
    if (typeof name !== 'string' || !(name in variants)) {
      e.push(`${p}.${tag}: unbekannte Variante ${JSON.stringify(name)}`);
      return;
    }
    checkProps(variants[name] as Props, o, `${p}<${name}>`, e, false, [tag]);
  };
}

type ExtOf<M extends Record<string, Schema<unknown>>> = { [V in keyof M & string]: { [T in V]: Infer<M[V]> } }[keyof M & string];

/** Extern getaggte Enum (serde-Standard): Unit-Varianten als String, sonst `{ "Variante": … }`. */
export function ext<M extends Record<string, Schema<unknown>>, const U extends readonly string[] = []>(
  variants: M,
  units?: U,
): Schema<ExtOf<M> | U[number]> {
  const unitList: readonly string[] = units ?? [];
  return (v, p, e) => {
    if (typeof v === 'string') {
      if (!unitList.includes(v)) e.push(`${p}: unbekannte Unit-Variante ${JSON.stringify(v)}`);
      return;
    }
    if (typeof v !== 'object' || v === null || Array.isArray(v)) {
      e.push(`${p}: Variante erwartet, ${describe(v)} gefunden`);
      return;
    }
    const keys = Object.keys(v);
    const key = keys[0];
    if (keys.length !== 1 || key === undefined || !(key in variants)) {
      e.push(`${p}: genau eine bekannte Variante erwartet, ${JSON.stringify(keys)} gefunden`);
      return;
    }
    (variants[key] as Schema<unknown>)((v as Record<string, unknown>)[key], `${p}.${key}`, e);
  };
}

/** Prüft und liefert die Fehler. */
export function validate<T>(schema: Schema<T>, value: unknown, path = '$'): string[] {
  const errors: string[] = [];
  schema(value, path, errors);
  return errors;
}

/** `true`, wenn A und B gegenseitig zuweisbar sind. */
export type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
