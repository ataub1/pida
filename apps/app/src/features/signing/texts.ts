/**
 * Texte und Hilfen der Signierkarte (aus der bisherigen Signieransicht übernommen): Maskenprobleme
 * (`FormProblem`, Katalog-IDs aus PDF-Over), Fortschritt, Meldungen (A-Trust-Text roh), QR-Bild und
 * die freundlichen Fehlertexte (Fluent-Attribute `.title`/`.action` der stabilen Codes, ADR 0004 §3.16).
 */
import type { FormProblem, Notice, Progress, QrImage } from '../../core/types';
import type { Translator } from '../../i18n/catalog';

/** Text eines `FormProblem` (Katalog-IDs aus PDF-Over, sonst eigene `ng-ui-*`). */
export function problemText(t: Translator, problem: FormProblem): string {
  if (typeof problem === 'string') {
    switch (problem) {
      case 'PasswordTooShort':
        return t.t('exception-PasswordTooShort');
      case 'PasswordTooLong':
        return t.t('exception-PasswordTooLong');
      case 'NoTan':
        return t.t('error-NoTan');
      case 'TanTooLong':
        return t.t('error-TanTooLong');
      case 'EnteredReferenceValue':
        return t.t('error-EnteredReferenceValue');
      case 'QrImageFailed':
        return t.t('error-FailedToLoadQRCode');
      case 'LoginRejected':
        return t.t('ng-ui-login-rejected');
    }
  }
  if ('Fido2Error' in problem) {
    // stabiler Code (`fido2-…`, FormProblem::code), Detail nur zur Diagnose
    const { code, detail } = problem.Fido2Error;
    return t.has(code) ? t.t(code) : t.t('ng-ui-fido2-failed', { detail: detail || code });
  }
  if ('Fido2Failed' in problem) {
    const code = problem.Fido2Failed;
    return t.has(code) ? t.t(code) : t.t('ng-ui-fido2-failed', { detail: code });
  }
  return `${t.t('mobileBKU-notice')} ${problem.Atrust}`;
}

/** Text eines Fortschritts (Warteschritt der Karte). */
export function progressText(t: Translator, progress: Progress): string {
  if (typeof progress === 'string') {
    switch (progress) {
      case 'Connecting':
        return t.t('ng-ui-sc-connecting');
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

/** Titel und Text einer Meldung (docs/analyse/01 §4.10; Texte von A-Trust roh). */
export function noticeTexts(t: Translator, notice: Notice): { title: string; text: string } {
  const title = notice.kind === 'Error' ? t.t('error-Title') : notice.kind === 'Warning' ? t.t('common-warning') : t.t('common-info');
  const raw = notice.text ?? '';
  const code = notice.code;
  let text: string;
  if (typeof code === 'object') {
    text = t.t('atrusterror-http_generic', { arg1: code.Http.status, arg2: raw });
  } else {
    switch (code) {
      case 'AtrustInterstitial':
        text = raw || t.t('common-info');
        break;
      case 'AtrustError':
        text = t.t('atrusterror-message', { arg1: raw });
        break;
      case 'ProxyAuthRequired':
        text = t.t('error-ProxyAuthRequired');
        break;
      case 'DocumentTooLarge':
        text = t.t('atrusterror-http_413');
        break;
      case 'ConnectionFailed':
        text = t.t('error-FailedToConnect', { arg1: raw });
        break;
      case 'LocalBkuNotReachable':
        text = t.t('error-LocalBKU');
        break;
      case 'KeystoreFileMissing':
        text = t.t('ng-keystore-file-not-found-at', { name: raw.split(/[\\/]/).pop() || raw });
        break;
      case 'KeystoreAliasMissing':
        text = t.t('error-KeyStoreAliasExist', { arg1: raw });
        break;
      case 'KeystoreError':
        text = t.t('error-KeyStore');
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
 * `.text` (optional) ersetzt in der Karte die Meldung mit technischer Ursache (`{ $detail }`, z. B. die
 * Fehlerkette der Verbindung auf Englisch) – die Ursache steht dann nur unter „Technische Details“; CLI und
 * Log behalten die vollständige Meldung.
 */
const UI_FALLBACK: Record<string, { title: string; action: string | null }> = {
  'error-PDFPwdProtected': { title: 'ng-ui-ws-locked-title', action: 'ng-ui-ws-locked-other' },
  'pdf-user-password': { title: 'ng-ui-ws-locked-title', action: 'ng-ui-ws-locked-other' },
  'error-MayNotBeAPDF': { title: 'ng-ui-ws-notpdf-title', action: 'ng-ui-ws-retry' },
  'output-save_failed': { title: 'ng-ui-fin-save-failed-title', action: 'ng-ui-ws-retry' },
};

export function friendlyError(t: Translator, code: string): { title: string; action: string | null; text: string | null } {
  const dashed = code.replace(/\./g, '-');
  const id = t.has(code) ? code : dashed;
  const title = t.attr(id, 'title');
  if (title) return { title, action: t.attr(id, 'action'), text: t.attr(id, 'text') };
  const fallback = UI_FALLBACK[id];
  if (fallback) return { title: t.t(fallback.title), action: fallback.action ? t.t(fallback.action) : null, text: null };
  return { title: t.t('ng-ui-sc-error-title'), action: null, text: null };
}
