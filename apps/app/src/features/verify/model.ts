/**
 * Darstellungsregeln des Prüfmodus (ADR 0004 §3.9, §4.1): Texte, Töne und Formate aus `VerifyView` – keine
 * Prüflogik (die liefert der Kern). Wortlaut nach §8 F3: PIDA sagt nie „gültig“ oder „rechtsgültig“, sondern
 * „unverändert und kryptografisch intakt“; die Qualifikation gilt nur „laut Zertifikat“, und der Hinweis auf
 * die vollständige Prüfung (signaturpruefung.gv.at) steht immer dabei.
 */
import type { QualificationClaim, RectPt, SignatureFormat, VerifiedSignatureView, VerifyView } from '../../core/types';
import type { Translator } from '../../i18n/catalog';

/** Vollständige Prüfung (Vertrauenskette, Sperrstatus) – wie auf „Fertig“. */
export const FULL_CHECK_URL = 'https://www.signaturpruefung.gv.at';

export type SigStatus = 'intact' | 'broken' | 'unverifiable';
export type SummaryTone = 'info' | 'success' | 'warning' | 'danger';

export function statusOf(sig: VerifiedSignatureView): SigStatus {
  switch (sig.integrity.kind) {
    case 'intact':
      return 'intact';
    case 'broken':
      return 'broken';
    case 'not_verifiable':
      return 'unverifiable';
  }
}

export const STATUS_LABEL: Record<SigStatus, string> = {
  intact: 'ng-ui-vf-status-intact',
  broken: 'ng-ui-vf-status-broken',
  unverifiable: 'ng-ui-vf-status-unverifiable',
};

export const STATUS_TONE: Record<SigStatus, 'success' | 'danger' | 'warning'> = {
  intact: 'success',
  broken: 'danger',
  unverifiable: 'warning',
};

/** Gesamtergebnis als Hinweis (Titel, Text, Ton). */
export function summaryOf(v: VerifyView, t: Translator): { tone: SummaryTone; title: string; text: string } {
  const n = v.signatures.length;
  const broken = v.signatures.filter((s) => s.integrity.kind === 'broken').length;
  switch (v.summary) {
    case 'no_signatures':
      return { tone: 'info', title: t.t('ng-ui-vf-summary-none'), text: t.t('ng-ui-vf-summary-none-text') };
    case 'all_intact':
      return { tone: 'success', title: t.t('ng-ui-vf-summary-intact', { n }), text: t.t('ng-ui-vf-summary-intact-text') };
    case 'some_broken':
      return { tone: 'danger', title: t.t('ng-ui-vf-summary-broken', { n: Math.max(1, broken) }), text: t.t('ng-ui-vf-summary-broken-text') };
    case 'some_not_verifiable':
      return { tone: 'warning', title: t.t('ng-ui-vf-summary-unverifiable'), text: t.t('ng-ui-vf-summary-unverifiable-text') };
    case 'modified_after_last_signature':
      return { tone: 'warning', title: t.t('ng-ui-vf-summary-modified'), text: t.t('ng-ui-vf-summary-modified-text') };
  }
}

/** Qualifikation „laut Zertifikat“ (qcStatements, nicht gegen die EU-Vertrauensliste geprüft). */
export function qualificationText(q: QualificationClaim, t: Translator): string {
  switch (q.kind) {
    case 'qualified':
      return t.t(q.qscd ? 'ng-ui-vf-qes' : 'ng-ui-vf-qc');
    case 'not_qualified':
      return t.t('ng-ui-vf-not-qualified');
    case 'unknown':
      return t.t('ng-ui-vf-qual-unknown');
  }
}

export const FORMAT_LABEL: Record<SignatureFormat, string> = {
  pades: 'ng-ui-vf-format-pades',
  pkcs7_detached: 'ng-ui-vf-format-pkcs7',
  pkcs7_sha1: 'ng-ui-vf-format-pkcs7-sha1',
  other: 'ng-ui-vf-format-other',
};

/** Ton einer Zeile: unverändert, Hinweis (spätere Revisionen), verletzt bzw. nicht prüfbar. */
export type LineTone = 'ok' | 'info' | 'bad';

/**
 * Zeilen zu Integrität und Abdeckung; spätere Revisionen werden benannt. Ob eine Ergänzung harmlos ist (z. B.
 * Validierungsdaten), unterscheidet PIDA in v1 nicht – deshalb ein neutraler Hinweis, keine Warnung.
 */
export function integrityLines(sig: VerifiedSignatureView, t: Translator): { tone: LineTone; text: string }[] {
  switch (sig.integrity.kind) {
    case 'broken':
      return [{ tone: 'bad', text: t.t('ng-ui-vf-broken') }];
    case 'not_verifiable':
      return [{ tone: 'bad', text: t.t('ng-ui-vf-unverifiable') }];
    case 'intact': {
      const c = sig.coverage;
      if (c.kind === 'whole_document') return [{ tone: 'ok', text: t.t('ng-ui-vf-whole') }];
      return [
        { tone: 'ok', text: t.t('ng-ui-vf-revision') },
        { tone: 'info', text: t.t('ng-ui-vf-later', { revisions: c.later_revisions, signatures: c.later_signatures }) },
      ];
    }
  }
}

/** Technischer Code einer verletzten bzw. nicht prüfbaren Signatur (sonst `null`). */
export function integrityCode(sig: VerifiedSignatureView): string | null {
  return sig.integrity.kind === 'intact' ? null : sig.integrity.code;
}

/** Erklärender Text zu einem Code des Kerns, falls der Katalog ihn kennt (sonst `null`, der Code steht in den Details). */
export function codeText(code: string, t: Translator): string | null {
  return t.has(code) || t.has(code.replace(/\./g, '-')) ? t.code(code) : null;
}

function locale(lang: string): string {
  return lang === 'de' ? 'de-AT' : 'en-GB';
}

/** Zeitpunkt (RFC 3339) in der Sprache der Oberfläche, z. B. „14.10.2026, 09:12“. */
export function formatDateTime(iso: string, lang: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return new Intl.DateTimeFormat(locale(lang), { dateStyle: 'medium', timeStyle: 'short' }).format(d);
}

/** Datum (RFC 3339), z. B. „20.11.2024“. */
export function formatDate(iso: string, lang: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return new Intl.DateTimeFormat(locale(lang), { dateStyle: 'medium' }).format(d);
}

/** Dezimale Seriennummer in Dreiergruppen („1 288 004 912“); anderes bleibt unverändert. */
export function formatSerial(serial: string): string {
  if (!/^\d+$/.test(serial)) return serial;
  return serial.replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
}

/** Verfahren „ECDSA P-256 · SHA-256“. */
export function methodText(sig: VerifiedSignatureView): string | null {
  const parts = [sig.algorithm, sig.digest].filter((x): x is string => !!x);
  return parts.length > 0 ? parts.join(' · ') : null;
}

/** Rechteck in `PAGE_ABS` (Ursprung links unten, sichtbare Seite) → CSS-Box auf der Seite (Ursprung links oben). */
export function boxOnPage(rect: RectPt, pageHeight: number, scale: number): { left: number; top: number; width: number; height: number } {
  return {
    left: rect.left * scale,
    top: (pageHeight - rect.top) * scale,
    width: Math.max(0, rect.right - rect.left) * scale,
    height: Math.max(0, rect.top - rect.bottom) * scale,
  };
}

/** Name einer Signatur für Überschrift und Hinweise. */
export function signerName(sig: VerifiedSignatureView, t: Translator): string {
  return sig.signer ?? t.t('ng-ui-vf-unknown-signer', { n: sig.index });
}
