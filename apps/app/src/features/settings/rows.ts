/**
 * Bereiche und Zeilen der Einstellungen (ADR 0004 §3.11 plus „Dokument“ direkt nach „Signaturbild“) und die
 * Suche über alle Bereiche: Treffer sind Zeilen, deren Beschriftung, Synonyme (Fluent-Attribut `.keywords`
 * an der Beschriftung, ADR 0004 §4.5) oder Bereichsname alle Suchwörter enthalten – ohne Groß-/Kleinschreibung
 * und Akzente. Gesucht wird nur in Zeilen, die gerade sichtbar wären (Plattform, Profil).
 */
import type { SettingsSectionId } from '../../app/context';
import type { SignatureProfile } from '../../core/types';
import type { Translator } from '../../i18n/catalog';

/** Bereiche der Seitenleiste; `document` ist neu (Entscheidung des Auftraggebers, Vorlage Prototyp fd7def3). */
export type SectionId = SettingsSectionId | 'document';

export interface SectionDef {
  id: SectionId;
  label: string;
}

export const SECTIONS: readonly SectionDef[] = [
  { id: 'signature', label: 'ng-ui-set-sec-signature' },
  { id: 'document', label: 'ng-ui-set-sec-document' },
  { id: 'signers', label: 'ng-ui-set-sec-signers' },
  { id: 'save', label: 'ng-ui-set-sec-save' },
  { id: 'appearance', label: 'ng-ui-set-sec-appearance' },
  { id: 'privacy', label: 'ng-ui-set-sec-privacy' },
  { id: 'advanced', label: 'ng-ui-set-sec-advanced' },
  { id: 'about', label: 'ng-ui-set-sec-about' },
];

/** Kennung einer Zeile. */
export type RowId =
  | 'signature.preview'
  | 'signature.profile'
  | 'signature.language'
  | 'signature.emblem'
  | 'signature.logo_size'
  | 'signature.note'
  | 'document.pdfa'
  | 'document.placeholder'
  | 'document.auto_position'
  | 'signers.default'
  | 'signers.id_austria'
  | 'signers.fido2'
  | 'signers.keystore'
  | 'signers.card'
  | 'save.target'
  | 'save.postfix'
  | 'appearance.theme'
  | 'appearance.language'
  | 'privacy.info'
  | 'privacy.certcache'
  | 'privacy.recent'
  | 'advanced.proxy'
  | 'advanced.updates'
  | 'advanced.migration'
  | 'advanced.reset'
  | 'about.info'
  | 'about.logs';

/** Umstände, von denen die Sichtbarkeit einer Zeile abhängt. */
export interface RowContext {
  mobile: boolean;
  profile: SignatureProfile;
}

export interface RowDef {
  id: RowId;
  section: SectionId;
  /** Beschriftung (i18n-ID, trägt `.keywords`) */
  label: string;
  visible?: (c: RowContext) => boolean;
}

const textProfile = (c: RowContext) => c.profile === 'SIGNATURBLOCK_SMALL' || c.profile === 'AMTSSIGNATURBLOCK';
const desktop = (c: RowContext) => !c.mobile;

export const ROWS: readonly RowDef[] = [
  { id: 'signature.preview', section: 'signature', label: 'ng-ui-set-preview' },
  { id: 'signature.profile', section: 'signature', label: 'ng-ui-set-profile' },
  { id: 'signature.language', section: 'signature', label: 'ng-ui-set-block-lang', visible: textProfile },
  // Mobile: Bildmarke nur über die Migration setzbar, bis das Plugin Bilder importiert (ADR 0004 §3.14)
  { id: 'signature.emblem', section: 'signature', label: 'ng-ui-set-emblem', visible: (c) => !c.mobile && c.profile !== 'INVISIBLE' },
  { id: 'signature.logo_size', section: 'signature', label: 'ng-ui-set-logo-size', visible: (c) => c.profile === 'BASE_LOGO' },
  { id: 'signature.note', section: 'signature', label: 'ng-ui-set-note', visible: textProfile },
  { id: 'document.pdfa', section: 'document', label: 'ng-ui-set-pdfa' },
  { id: 'document.placeholder', section: 'document', label: 'ng-ui-set-placeholder' },
  // „Automatische Positionierung“ wie PDF-Over (`signature.auto_position`, ADR 0004 Nachtrag 8)
  { id: 'document.auto_position', section: 'document', label: 'ng-ui-set-auto-position' },
  { id: 'signers.default', section: 'signers', label: 'ng-ui-set-default' },
  { id: 'signers.id_austria', section: 'signers', label: 'ng-ui-set-ida-user' },
  { id: 'signers.fido2', section: 'signers', label: 'ng-ui-set-fido2' },
  { id: 'signers.keystore', section: 'signers', label: 'ng-ui-set-keystore' },
  { id: 'signers.card', section: 'signers', label: 'ng-ui-set-card', visible: desktop },
  { id: 'save.target', section: 'save', label: 'ng-ui-set-target' },
  { id: 'save.postfix', section: 'save', label: 'ng-ui-set-postfix' },
  { id: 'appearance.theme', section: 'appearance', label: 'ng-ui-set-theme' },
  { id: 'appearance.language', section: 'appearance', label: 'ng-ui-set-ui-lang' },
  { id: 'privacy.info', section: 'privacy', label: 'ng-ui-set-privacy-info' },
  { id: 'privacy.certcache', section: 'privacy', label: 'ng-ui-set-certcache' },
  { id: 'privacy.recent', section: 'privacy', label: 'ng-ui-set-recent' },
  { id: 'advanced.proxy', section: 'advanced', label: 'ng-ui-set-proxy', visible: desktop },
  { id: 'advanced.updates', section: 'advanced', label: 'ng-ui-set-updates', visible: desktop },
  // PDF-Over gibt es nur am Desktop (Altkonfiguration `~/.pdf-over`)
  { id: 'advanced.migration', section: 'advanced', label: 'ng-ui-set-migration', visible: desktop },
  { id: 'advanced.reset', section: 'advanced', label: 'ng-ui-set-reset' },
  { id: 'about.info', section: 'about', label: 'ng-ui-set-about' },
  { id: 'about.logs', section: 'about', label: 'ng-ui-set-logs', visible: desktop },
];

/** Sichtbare Zeilen. */
export function visibleRows(c: RowContext): RowDef[] {
  return ROWS.filter((r) => !r.visible || r.visible(c));
}

/** Kleinbuchstaben ohne Akzente („Größe“ ≙ „grosse“ nicht, aber „Ä“ ≙ „a“). */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

/** Suchtext einer Zeile: Beschriftung, Synonyme (`.keywords`) und Bereichsname. */
export function haystack(t: Translator, row: RowDef): string {
  const section = SECTIONS.find((s) => s.id === row.section);
  return fold([t.t(row.label), t.attr(row.label, 'keywords') ?? '', section ? t.t(section.label) : ''].join(' '));
}

/** Zeilen, die zur Suche passen (alle Wörter); leere Suche = keine Treffer. */
export function search(t: Translator, rows: RowDef[], query: string): RowDef[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return rows.filter((r) => {
    const hay = haystack(t, r);
    return words.every((w) => hay.includes(w));
  });
}
