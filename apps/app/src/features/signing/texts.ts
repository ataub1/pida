/**
 * Texte und Hilfen der Signierkarte (aus der bisherigen Signieransicht übernommen): Maskenprobleme
 * (`FormProblem`, Katalog-IDs aus PDF-Over), Fortschritt, Meldungen (A-Trust-Text roh), QR-Bild und
 * die freundlichen Fehlertexte (Fluent-Attribute `.title`/`.action` der stabilen Codes, ADR 0004 §3.16).
 */
import type { BackendId, FormProblem, Notice, Progress, QrImage } from '../../core/types';
import type { Args, Translator } from '../../i18n/catalog';

/**
 * Text eines `FormProblem`: eigene, verständliche Texte (`ng-ui-problem-*`) statt der PDF-Over-Kataloge („Keine TAN
 * eingeben“, „Konnte den QR code nicht laden“); diese bleiben für CLI und Protokoll unverändert.
 */
export function problemText(t: Translator, problem: FormProblem): string {
  if (typeof problem === 'string') {
    switch (problem) {
      case 'PasswordTooShort':
        return t.t('ng-ui-problem-password-too-short');
      case 'PasswordTooLong':
        return t.t('ng-ui-problem-password-too-long');
      case 'NoTan':
        return t.t('ng-ui-problem-tan-missing');
      case 'TanTooLong':
        return t.t('ng-ui-problem-tan-too-long');
      case 'EnteredReferenceValue':
        return t.t('ng-ui-problem-reference-value');
      case 'QrImageFailed':
        return t.t('ng-ui-problem-qr-failed');
      case 'LoginRejected':
        return t.t('ng-ui-login-rejected');
    }
  }
  if ('Fido2Error' in problem) {
    // stabiler Code (`fido2-…`, FormProblem::code), Detail nur zur Diagnose; Alltagstext `.text` ohne Variablen
    // (z. B. `fido2-device`, ADR 0004 Nachtrag 19)
    const { code, detail } = problem.Fido2Error;
    return t.attr(code, 'text') ?? (t.has(code) ? t.t(code, { detail }) : t.t('ng-ui-fido2-failed', { detail: detail || code }));
  }
  if ('Fido2Failed' in problem) {
    const code = problem.Fido2Failed;
    return t.attr(code, 'text') ?? (t.has(code) ? t.t(code, { detail: code }) : t.t('ng-ui-fido2-failed', { detail: code }));
  }
  return t.t('ng-ui-problem-atrust', { text: problem.Atrust });
}

/**
 * Text eines Fortschritts (Warteschritt der Karte). Der Keystore liegt auf dem Gerät: statt „Verbindung wird
 * aufgebaut“ (Standard des Kerns vor dem ersten Fortschritt) „Keystore wird geöffnet“.
 */
export function progressText(t: Translator, progress: Progress, backend: BackendId | null = null): string {
  if (typeof progress === 'string') {
    switch (progress) {
      case 'Connecting':
        return t.t(backend === 'keystore' ? 'ng-ui-sc-keystore-opening' : 'ng-ui-sc-connecting');
      case 'ReadingCertificate':
        return t.t('ng-ui-sc-certificate');
      case 'WaitingForServer':
        return t.t('ng-ui-sc-waiting-server');
      case 'Signing':
        return t.t('ng-ui-sc-signing');
      case 'Done':
        return t.t('ng-ui-sc-done');
    }
  }
  return t.t('ng-ui-sc-document', { index: progress.Document.index, total: progress.Document.total });
}

/**
 * Titel und Text einer Meldung (docs/analyse/01 §4.10). Texte von A-Trust erscheinen wörtlich; technische Angaben
 * (HTTP-Statuszeile, rohe Verbindungsfehler) nicht – dafür verständliche Texte mit Handlungshinweis (`ng-ui-notice-*`).
 */
export function noticeTexts(t: Translator, notice: Notice): { title: string; text: string } {
  const title = notice.kind === 'Error' ? t.t('error-Title') : notice.kind === 'Warning' ? t.t('common-warning') : t.t('common-info');
  const raw = notice.text ?? '';
  const code = notice.code;
  let text: string;
  if (typeof code === 'object') {
    text = t.t('ng-ui-notice-http', { status: code.Http.status });
  } else {
    switch (code) {
      case 'AtrustInterstitial':
        text = raw || t.t('common-info');
        break;
      case 'AtrustError':
        text = raw ? t.t('ng-ui-notice-atrust', { text: raw }) : t.t('ng-ui-notice-connection');
        break;
      case 'ProxyAuthRequired':
        text = t.t('ng-ui-notice-proxy-auth');
        break;
      case 'DocumentTooLarge':
        text = t.t('ng-ui-notice-too-large');
        break;
      case 'ConnectionFailed':
        text = t.t('ng-ui-notice-connection');
        break;
      case 'LocalBkuNotReachable':
        text = t.t('ng-ui-notice-localbku');
        break;
      case 'KeystoreFileMissing':
        text = t.t('ng-keystore-file-not-found-at', { name: raw.split(/[\\/]/).pop() || raw });
        break;
      case 'KeystoreAliasMissing':
        text = t.t('ng-ui-notice-keystore-alias', { alias: raw });
        break;
      case 'KeystoreError':
        text = t.t('ng-ui-notice-keystore');
        break;
      case 'KeystoreConversionNeeded':
        text = t.t('ng-ui-notice-keystore-conversion');
        break;
    }
  }
  return { title, text };
}

/** QR-Bild als `data:`-URL; nur Bild-MIME-Typen (A-Trust-Angabe wird nicht blind übernommen). */
export function qrDataUrl(image: QrImage): string {
  let binary = '';
  for (const b of image.bytes) binary += String.fromCharCode(b & 0xff);
  const mime = image.mime && /^image\/(png|jpeg|gif|webp|svg\+xml)$/i.test(image.mime) ? image.mime.toLowerCase() : 'image/png';
  return `data:${mime};base64,${btoa(binary)}`;
}

/**
 * Freundliche Fehlertexte (ADR 0004 §3.16): Titel und Aktion aus den Fluent-Attributen `.title`/`.action`
 * des Codes; für die Codes des erzeugten PDF-Over-Katalogs (ohne Attribute) eigene IDs der Oberfläche.
 * `.text` ist der Alltagstext **ohne Variablen** und ersetzt in Karte, Stapelpause und Stapelzeile die Meldung mit
 * technischer Ursache (`{ $detail }`, `{ $diagnostic }` …) – die Ursache steht nur unter „Technische Details“; CLI
 * und Log behalten die vollständige Meldung. Seit Nachtrag 19 gilt `.text` auch ohne `.title` (dann allgemeiner
 * Titel); jeder Fehlercode mit Variablen hat eines (Tests `errors.test.ts`, `code_catalog.rs`).
 */
const UI_FALLBACK: Record<string, { title: string; action: string | null; text?: string }> = {
  'error-PDFPwdProtected': { title: 'ng-ui-ws-locked-title', action: 'ng-ui-ws-locked-other' },
  'pdf-user-password': { title: 'ng-ui-ws-locked-title', action: 'ng-ui-ws-locked-other' },
  'error-MayNotBeAPDF': { title: 'ng-ui-ws-notpdf-title', action: 'ng-ui-ws-retry' },
  'output-save_failed': { title: 'ng-ui-fin-save-failed-title', action: 'ng-ui-ws-retry', text: 'ng-ui-fin-save-failed-text' },
};

export function friendlyError(t: Translator, code: string): { title: string; action: string | null; text: string | null } {
  const dashed = code.replace(/\./g, '-');
  const id = t.has(code) ? code : dashed;
  const fallback = UI_FALLBACK[id];
  const text = t.attr(id, 'text') ?? (fallback?.text ? t.t(fallback.text) : null);
  const title = t.attr(id, 'title');
  if (title) return { title, action: t.attr(id, 'action'), text };
  if (fallback) return { title: t.t(fallback.title), action: fallback.action ? t.t(fallback.action) : null, text };
  return { title: t.t('ng-ui-sc-error-title'), action: null, text };
}

/**
 * Alltagstext eines Fehlercodes (Karte, Stapelpause, Stapelzeile): `.text` bzw. Ersatztext der Oberfläche, sonst
 * die Meldung mit den Parametern des Kerns. Enthält nie einen rohen Platzhalter (Nachtrag 19).
 */
export function everydayText(t: Translator, code: string, params: Args = {}): string {
  return friendlyError(t, code).text ?? t.code(code, params);
}
