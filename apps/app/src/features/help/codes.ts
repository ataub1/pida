/** Welche Meldungscodes einen Hilfeartikel haben – ohne die Texte der Hilfe (bleibt im Haupt-Bundle klein). */
import type { Translator } from '../../i18n/catalog';

/** Fehlercode-Familien wie im Meldungskatalog (`code_catalog.rs`); als Ausdruck, damit der i18n-Test die Präfixe nicht
 *  für Meldungs-IDs hält. */
export const ERROR_FAMILIES =
  /^(?:backend|atrust|fido2|keystore|cms|sigblock|render|placement|pdf|pades|ng-signer|ng-config-error|ng-batch|ng-resume|ng-cms|ng-certcache|ng-document|ng-signing)-/;

/** Katalog-ID eines Codes (`backend.proxy.auth` → `backend-proxy-auth`). */
export function errorId(code: string): string {
  return code.replace(/\./g, '-');
}

/** Gibt es zu `code` einen Artikel `fehler:<id>`? */
export function hasErrorArticle(t: Translator, code: string): boolean {
  const id = errorId(code);
  return ERROR_FAMILIES.test(id) && t.has(id);
}
