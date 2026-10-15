/** Formatierung und abgeleitete Werte: Dateinamen, Signaturblock-Maße, Standardhinweise, Datumsangaben. */
import type { Lang, SigBlockSettings, SigProfile } from './types';

/** Basisname ohne Erweiterung. */
export function stem(fileName: string): string {
  const dot = fileName.lastIndexOf('.');
  return dot > 0 ? fileName.slice(0, dot) : fileName;
}

/**
 * Ausgabedatei nach Namensmuster. Platzhalter: `{name}` (Basisname), `{datum}`/`{date}` (JJJJ-MM-TT).
 * Unzulässige Zeichen werden ersetzt; leeres Muster ergibt `{name}_signiert`.
 */
export function outputName(fileName: string, pattern: string, now = new Date()): string {
  const p = pattern.trim() || '{name}_signiert';
  const date = isoDate(now);
  const base = p.replace(/\{name\}/g, stem(fileName)).replace(/\{datum\}|\{date\}/g, date);
  const safe = base.replace(/[\\/:*?"<>|]/g, '_').trim() || stem(fileName);
  return `${safe}.pdf`;
}

export function isoDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** ISO 8601 mit lokalem Offset, wie im PDF-AS-Signaturblock (`2026-10-15T10:42:17+02:00`). */
export function isoWithOffset(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0');
  const off = -d.getTimezoneOffset();
  const sign = off >= 0 ? '+' : '-';
  const oh = p(Math.floor(Math.abs(off) / 60));
  const om = p(Math.abs(off) % 60);
  return `${isoDate(d)}T${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}${sign}${oh}:${om}`;
}

/** Lesbares Datum/Zeit in der UI-Sprache. */
export function humanDateTime(iso: string, lang: Lang): string {
  const d = new Date(iso);
  return new Intl.DateTimeFormat(lang === 'de' ? 'de-AT' : 'en-GB', { dateStyle: 'medium', timeStyle: 'short' }).format(d);
}

export function humanDate(iso: string, lang: Lang): string {
  const d = new Date(iso);
  return new Intl.DateTimeFormat(lang === 'de' ? 'de-AT' : 'en-GB', { dateStyle: 'medium' }).format(d);
}

/** Standard-Signaturhinweis je Profil und Blocksprache (Texte aus dem PDF-Over-Katalog). */
export function defaultNote(profile: SigProfile, lang: Lang): string {
  if (profile === 'amt') {
    return lang === 'de'
      ? 'Dieses Dokument wurde amtssigniert. Auch ein Ausdruck dieses Dokuments hat gemäß § 20 E-Government-Gesetz die Beweiskraft einer öffentlichen Urkunde.'
      : 'This document was signed/sealed with an official signature. According to § 20 Austrian E-Government-Act a printout of this document is assumed to be authentic.';
  }
  return lang === 'de'
    ? 'Dieses mit einer qualifizierten elektronischen Signatur versehene Dokument hat gemäß Art. 25 Abs. 2 der Verordnung (EU) Nr. 910/2014 vom 23. Juli 2014 („eIDAS-VO“) die gleiche Rechtswirkung wie ein handschriftlich unterschriebenes Dokument.'
    : 'This document is signed with a qualified electronic signature. According to Art. 25 para. 2 of the Regulation (EU) No 910/2014 of 23 July 2014 ("eIDAS-Regulation") it shall have the equivalent legal effect of a handwritten signature.';
}

export function effectiveNote(s: SigBlockSettings): string {
  return s.note ?? defaultNote(s.profile, s.lang);
}

/** Größe des Signaturblocks in pt (angelehnt an PDF-AS: SMALL 230 × 49 bzw. 82 mit Hinweis). */
export function blockSize(s: SigBlockSettings): { w: number; h: number } {
  const hasNote = effectiveNote(s).trim().length > 0;
  if (s.profile === 'amt') return { w: 400, h: hasNote ? 116 : 92 };
  return { w: 230, h: hasNote ? 82 : 49 };
}

/** Telefonnummer maskiert für Konto-Chips: „+43 660 1234123“ → „+43 660 ···123“. */
export function maskUser(user: string): string {
  const compact = user.replace(/\s+/g, '');
  const m = /^(\+\d{2})(\d{3})(\d+)$/.exec(compact);
  if (m && m[3]) return `${m[1]} ${m[2]} ···${m[3].slice(-3)}`;
  if (user.length > 6) return `${user.slice(0, 3)}···${user.slice(-2)}`;
  return user;
}

export function formatTime(totalSecs: number): string {
  const s = Math.max(0, Math.ceil(totalSecs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
