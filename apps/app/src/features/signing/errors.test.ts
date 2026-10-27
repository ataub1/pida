/**
 * Alltagstexte der Fehlercodes (ADR 0004 Nachtrag 19, Befund vom 17.10.2026: „Unbekannte Seite des Signaturdienstes
 * ({$diagnostic})“): Für **jeden** Fehlercode des Katalogs ist der Alltagstext, den Karte, Stapelpause und Stapelzeile
 * zeigen (`everydayText` über `friendlyError`), nach der Auflösung mit den Parametern des Kerns frei von rohen
 * Platzhaltern und frei von der technischen Diagnose – die steht nur unter „Technische Details“. Gegenstück im Kern:
 * `crates/i18n/tests/code_catalog.rs::error_codes_have_an_everyday_text_without_variables` (gleiche Familien).
 */
import { FluentResource } from '@fluent/bundle';
import { describe, expect, it } from 'vitest';
import { SOURCES, translator } from '../../i18n/catalog';
import { everydayText, friendlyError, noticeTexts, problemText } from './texts';

/** Familien der Fehlercodes und Fehlercodes des Ablaufs (wie im Katalogtest des Kerns). */
const ERROR_FAMILIES = ['backend-', 'atrust-', 'fido2-', 'keystore-', 'cms-', 'sigblock-', 'render-', 'placement-', 'pdf-'];
const NOT_ERRORS = ['sigblock-chars-replaced', 'placement-notice-'];
const WORKFLOW_ERRORS = [
  'ng-invariant-violated',
  'ng-cms-too-large',
  'ng-signing-declined',
  'ng-signing-cancelled',
  'ng-no-backend-available',
  'ng-default-backend-unavailable',
  'ng-backend-choice-required',
  'ng-certificate-not-valid',
  'ng-certcache-mismatch',
  'ng-document-rejected',
  'ng-pdf-no-sign-rights',
  'ng-field-locked',
  'ng-field-already-signed',
  'ng-constraint-certified',
  'ng-constraint-signed',
  'error-PrepareDocument',
  'error-Unexpected',
  'error-Initialization',
  'error-IOError',
  'error-PDFProtected',
  'error-PDFPwdProtected',
  'error-MayNotBeAPDF',
  'output-save_failed',
];

function errorCodes(): string[] {
  const ids = new Set<string>();
  for (const { text } of SOURCES.de) {
    for (const entry of new FluentResource(text).body) {
      if (!('id' in entry) || entry.id.startsWith('-')) continue;
      const id = entry.id;
      if (ERROR_FAMILIES.some((f) => id.startsWith(f)) && !NOT_ERRORS.some((n) => id.startsWith(n))) ids.add(id);
    }
  }
  for (const id of WORKFLOW_ERRORS) ids.add(id);
  return [...ids].sort();
}

/** Alles, was der Kern als Parameter eines Fehlers liefern kann (`WorkflowError::params`), als Marker. */
const MARKER = 'MARKER-DIAGNOSE';
const PARAMS = Object.fromEntries(
  ['detail', 'diagnostic', 'status', 'reason', 'code', 'info', 'format', 'invariant', 'needed', 'reserve', 'arg1', 'arg2', 'class'].map((k) => [k, MARKER]),
);

describe('Alltagstexte der Fehlercodes (Nachtrag 19)', () => {
  const codes = errorCodes();

  it('erfasst alle Familien (Plausibilität)', () => {
    for (const sample of ['backend-unknown-page', 'backend-session-closed', 'backend-http', 'sigblock-font', 'fido2-device', 'output-save_failed']) {
      expect(codes, sample).toContain(sample);
    }
    expect(codes.length).toBeGreaterThanOrEqual(80);
  });

  for (const lang of ['de', 'en'] as const) {
    it(`${lang}: kein roher Platzhalter und keine Diagnose im Alltagstext`, () => {
      const t = translator(lang);
      const problems: string[] = [];
      for (const code of codes) {
        expect(t.has(code), `${code} fehlt`).toBe(true);
        // mit allen Parametern des Kerns bzw. ganz ohne (Karte vor dem ersten Fortschritt, Mock)
        for (const params of [PARAMS, {}]) {
          const text = everydayText(t, code, params);
          if (/\{ ?\$/.test(text) || text.includes(MARKER) || text.startsWith('!')) problems.push(`${code}: ${text}`);
        }
        const { title, action } = friendlyError(t, code);
        for (const s of [title, action ?? '']) if (/\{ ?\$/.test(s)) problems.push(`${code} (Titel/Aktion): ${s}`);
      }
      expect(problems).toEqual([]);
    });
  }

  it('FIDO2-Hinweis in der Maske ohne Platzhalter (fido2-device)', () => {
    const t = translator('de');
    const text = problemText(t, { Fido2Error: { code: 'fido2-device', detail: 'CTAP2_ERR_OTHER 0x7f' } });
    expect(text).toBe('Am Sicherheitsschlüssel ist ein Fehler aufgetreten. Bitte erneut versuchen oder App bzw. SMS verwenden.');
    expect(problemText(t, { Fido2Failed: 'fido2-device' })).not.toMatch(/\{ ?\$/);
  });

  it('Sitzung abgelaufen: Titel, Aktion „Erneut versuchen“, Alltagstext (DE/EN)', () => {
    const de = friendlyError(translator('de'), 'backend-session-closed');
    expect(de).toEqual({
      title: 'Die Sitzung bei A-Trust ist abgelaufen',
      action: 'Erneut versuchen',
      text: null,
    });
    expect(everydayText(translator('de'), 'backend-session-closed', PARAMS)).toBe(
      'Die Sitzung bei A-Trust ist abgelaufen oder wurde beendet. Bitte erneut versuchen – dabei beginnt eine neue Sitzung.',
    );
    expect(friendlyError(translator('en'), 'backend-session-closed').action).toBe('Try again');
  });
});

describe('Verständliche Meldungen statt technischer Texte', () => {
  const t = translator('de');

  it('t.code zeigt den Alltagstext, nie rohen Text', () => {
    // Code mit Alltagstext: ohne technische Ursache
    expect(t.code('pdf-docmdp-p1')).not.toMatch(/DocMDP|P=1/);
    expect(t.code('ng-config-error-parse', { detail: 'TOML parse error at line 3' })).not.toContain('TOML');
    // kein Code, sondern eine rohe Fehlermeldung: allgemeiner Text ohne den Rohtext
    expect(t.code('Rückkanal: HTTP 502')).toBe(t.t('ng-ui-error-unexpected'));
    expect(t.code('TypeError: Failed to fetch')).not.toContain('TypeError');
    // unbekannter, aber gültiger Code: mit Code (für den Support)
    expect(t.code('irgendwas-neues')).toContain('irgendwas-neues');
  });

  it('Meldungen beim Signieren ohne HTTP-Zeile und rohe Verbindungsfehler', () => {
    const http = noticeTexts(t, { kind: 'Error', code: { Http: { status: 503 } }, text: 'Service Unavailable', buttons: 'Ok' } as never);
    expect(http.text).toContain('503');
    expect(http.text).not.toMatch(/HTTP|Service Unavailable/);
    const conn = noticeTexts(t, { kind: 'Error', code: 'ConnectionFailed', text: 'error sending request: dns error', buttons: 'Ok' } as never);
    expect(conn.text).not.toMatch(/dns|error sending/);
    const proxy = noticeTexts(t, { kind: 'Error', code: 'ProxyAuthRequired', text: null, buttons: 'Ok' } as never);
    expect(proxy.text).not.toMatch(/407|HTTP/);
    // Texte von A-Trust selbst bleiben wörtlich
    expect(noticeTexts(t, { kind: 'Error', code: 'AtrustError', text: 'Bitte später erneut versuchen.', buttons: 'Ok' } as never).text).toContain('Bitte später erneut versuchen.');
  });

  it('Eingabeprobleme in Alltagssprache', () => {
    expect(problemText(t, 'NoTan')).toBe('Bitte geben Sie die TAN aus der SMS ein.');
    expect(problemText(t, 'QrImageFailed')).not.toMatch(/QR code nicht laden/);
  });
});
