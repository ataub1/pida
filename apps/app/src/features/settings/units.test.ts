/**
 * Reine Hilfen der Einstellungen (Paket UI-B, ADR 0004 E3): Suche mit Synonymen (`.keywords`), Sichtbarkeit
 * der Zeilen je Plattform/Profil, Keystore-Typ aus der Endung (PDF-Over), Logo-Größe in cm, Richtlinien-Sperren
 * aus dem vorgeschlagenen Vertragsfeld `locked`, Wertzugriff über Punktpfade.
 */
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../../core/defaults';
import { translator } from '../../i18n/catalog';
import { settingsView } from '../../test/views';
import { logoCm } from './controls';
import { ROWS, SECTIONS, fold, search, visibleRows } from './rows';
import { keystoreTypeFromFile } from './SignerRows';
import { lockedReason, valueAt } from './state';

const desktop = visibleRows({ mobile: false, profile: 'SIGNATURBLOCK_SMALL' });
const ids = (rows: { id: string }[]) => rows.map((r) => r.id);

describe('Bereiche und Zeilen', () => {
  it('„Dokument“ direkt nach „Signaturbild“; PDF/A und Platzhalter nicht unter „Erweitert“', () => {
    const order = SECTIONS.map((s) => s.id);
    expect(order).toEqual(['signature', 'document', 'signers', 'save', 'appearance', 'privacy', 'advanced', 'about']);
    expect(ROWS.filter((r) => r.section === 'document').map((r) => r.id)).toEqual(['document.pdfa', 'document.placeholder', 'document.auto_position']);
    expect(ROWS.filter((r) => r.section === 'advanced').map((r) => r.id)).not.toContain('document.placeholder');
  });

  it('jede Zeile hat eine Beschriftung mit Synonymen in DE und EN', () => {
    for (const lang of ['de', 'en'] as const) {
      const t = translator(lang);
      for (const r of ROWS) {
        expect(t.hasOwn(r.label), `${lang} ${r.label}`).toBe(true);
        expect(t.attr(r.label, 'keywords')?.length ?? 0, `${lang} ${r.label}.keywords`).toBeGreaterThan(2);
      }
    }
  });

  it('Mobile ohne Proxy, Updates, Protokolle, Chipkarte und Bildmarke; Profilzeilen je Profil', () => {
    const mobile = ids(visibleRows({ mobile: true, profile: 'SIGNATURBLOCK_SMALL' }));
    for (const id of ['advanced.proxy', 'advanced.updates', 'advanced.migration', 'about.logs', 'signers.card', 'signature.emblem']) expect(mobile).not.toContain(id);
    expect(ids(desktop)).toContain('advanced.migration');
    expect(mobile).toContain('signers.keystore');
    expect(ids(desktop)).toContain('signature.note');
    expect(ids(desktop)).not.toContain('signature.logo_size');
    const logo = ids(visibleRows({ mobile: false, profile: 'BASE_LOGO' }));
    expect(logo).toContain('signature.logo_size');
    expect(logo).not.toContain('signature.note');
    expect(logo).not.toContain('signature.language');
    const invisible = ids(visibleRows({ mobile: false, profile: 'INVISIBLE' }));
    expect(invisible).not.toContain('signature.emblem');
    expect(invisible).toContain('signature.preview');
  });
});

describe('Suche', () => {
  const de = translator('de');
  const en = translator('en');
  const hit = (t: typeof de, q: string) => ids(search(t, desktop, q));

  it('findet „Logo“, „Handy“, „Proxy“ (Abnahme E3 Punkt 3)', () => {
    expect(hit(de, 'Logo')).toEqual(['signature.emblem']);
    expect(hit(de, 'Handy')).toEqual(['signers.id_austria']);
    expect(hit(de, 'PDF-Over')).toEqual(['advanced.migration']);
    expect(hit(en, 'import')).toEqual(['advanced.migration']);
    expect(hit(de, 'Proxy')).toEqual(['advanced.proxy']);
  });

  it('Dokument: „pdf/a“, „platzhalter“, „qr“, „signaturfeld“; englisch „signature field“', () => {
    expect(hit(de, 'pdf/a')).toEqual(['document.pdfa']);
    expect(hit(de, 'PDF/A')).toEqual(['document.pdfa']);
    expect(hit(de, 'platzhalter')).toEqual(['document.placeholder']);
    expect(hit(de, 'qr')).toEqual(['document.placeholder']);
    expect(hit(de, 'signaturfeld')).toEqual(['document.placeholder']);
    expect(hit(en, 'signature field')).toEqual(['document.placeholder']);
    expect(hit(en, 'pdf/a')).toEqual(['document.pdfa']);
  });

  it('alle Wörter, ohne Groß-/Kleinschreibung und Akzente; Bereichsname zählt; leer = nichts', () => {
    expect(fold('Größe ÄÖÜ')).toBe('große aou');
    expect(hit(de, 'bürgerkarte')).toEqual(expect.arrayContaining(['signers.card']));
    expect(hit(de, 'BURGERKARTE')).toEqual(expect.arrayContaining(['signers.card']));
    expect(hit(de, 'datenschutz')).toEqual(['privacy.info', 'privacy.certcache', 'privacy.recent']);
    expect(hit(de, 'zuletzt signiert')).toEqual(['privacy.recent']);
    expect(hit(de, 'xyz')).toEqual([]);
    expect(hit(de, '   ')).toEqual([]);
  });
});

describe('Hilfen', () => {
  it('Keystore-Typ folgt der Dateiendung (PDF-Over KeystoreConfigurationComposite)', () => {
    expect(keystoreTypeFromFile('/a/b/id.P12')).toBe('PKCS12');
    expect(keystoreTypeFromFile('C:\\Keys\\id.pfx')).toBe('PKCS12');
    expect(keystoreTypeFromFile('id.pkcs12')).toBe('PKCS12');
    expect(keystoreTypeFromFile('/a/keystore.jks')).toBe('JCEKS');
    expect(keystoreTypeFromFile('/a/keystore.ks ')).toBe('JCEKS');
    expect(keystoreTypeFromFile('/a.p12/keystore')).toBeNull();
    expect(keystoreTypeFromFile('/a/keystore.bin')).toBeNull();
  });

  it('Logo-Größe: mm → cm mit Grenzen 1–20 cm', () => {
    expect(logoCm(23)).toBe(2.3);
    expect(logoCm(200)).toBe(20);
    expect(logoCm(500)).toBe(20);
    expect(logoCm(3)).toBe(1);
    expect(logoCm(Number.NaN)).toBe(2.3);
  });

  it('Richtlinien-Sperren aus `locked` (Pfad oder ganzer Abschnitt), ohne Feld frei', () => {
    const view = settingsView().settings!;
    expect(lockedReason(view, 'general.recent_documents')).toBeNull();
    const locked = { ...view, locked: { 'general.recent_documents': 'ng-recent-policy', proxy: 'ng-ui-set-policy' } };
    expect(lockedReason(locked, 'general.recent_documents')).toBe('ng-recent-policy');
    expect(lockedReason(locked, 'proxy.port')).toBe('ng-ui-set-policy');
    expect(lockedReason(locked, 'general.update_check')).toBeNull();
  });

  it('Punktpfade', () => {
    const s = defaultSettings();
    expect(valueAt(s, 'signature.profile')).toBe('SIGNATURBLOCK_SMALL');
    expect(valueAt(s, 'placeholder.kind')).toBe('marker');
    expect(valueAt(s, 'nichts.da')).toBeUndefined();
  });
});
