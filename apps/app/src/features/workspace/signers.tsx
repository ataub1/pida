/**
 * Anzeige der Signiermittel (Konto-Chip, Menü, Signierkarte): Name, Beschreibung, Symbol und Farbe je
 * Backend. Verfügbarkeit, Konto und Fähigkeiten kommen aus `ViewState::signers` (ADR 0004 §3.4).
 */
import type { BackendId, SignerView, ViewState } from '../../core/types';
import { webProfile } from '../../core/web-profile';
import type { Translator } from '../../i18n/catalog';
import { IconCard, IconFileKey, IconLock, IconPhone, IconShield } from '../../ui/Icons';


/** Backends mit Oberfläche (reservierte Szenarien B/C/D erscheinen nicht). */
export const SHOWN_BACKENDS: readonly BackendId[] = ['atrust_native', 'keystore', 'local_bku'];

/** Angebotene Signiermittel; im Web ohne Chipkarte, der Keystore nur im lokalen Testbuild (`web-profile.ts`). */
export function isShownBackend(id: BackendId): boolean {
  const web = webProfile();
  if (web) return id === 'atrust_native' || (id === 'keystore' && web.keystore);
  return SHOWN_BACKENDS.includes(id);
}

/**
 * Signiermittel-Auswahl anbieten (Kern: `SignersView::choice_offered`, ADR 0004 Nachtrag 20)? Ohne Auswahl – auf
 * iOS/Android immer (nur ID Austria), am Desktop z. B. per Richtlinie – gibt es keinen Konto-Chip mit Menü, keinen
 * Abschnitt „Signieren mit“ und keine Wahl in Stapel, Ersteinrichtung und Signierkarte; „Signieren“ signiert mit
 * `signers.selected` (vom Kern gesetzt). Fehlt das Feld (ältere Daten), wie bisher mit Auswahl.
 */
export function signerChoice(view: ViewState): boolean {
  return view.signers.choice_offered !== false;
}

export function signerName(t: Translator, backend: BackendId | null): string {
  switch (backend) {
    case 'atrust_native':
      return t.t('ng-ui-insp-signer-atrust');
    case 'keystore':
      return t.t('ng-ui-insp-signer-keystore');
    case 'local_bku':
      return t.t('ng-ui-insp-signer-card');
    default:
      return t.t('ng-ui-insp-signer-none');
  }
}

export function signerDescription(t: Translator, signer: SignerView, mobile: boolean): string {
  if (!signer.available && signer.unavailable_reason) return t.code(signer.unavailable_reason);
  switch (signer.backend) {
    case 'atrust_native':
      return t.t(mobile ? 'ng-ui-insp-signer-atrust-desc-mobile' : 'ng-ui-insp-signer-atrust-desc');
    case 'keystore':
      return t.t('ng-ui-insp-signer-keystore-desc');
    case 'local_bku':
      return t.t('ng-ui-insp-signer-card-desc');
    default:
      return '';
  }
}

/** Beschriftung des Konto-Chips: „ID Austria · +43 660 ···123“ (Konto im Kern maskiert). */
export function chipLabel(t: Translator, signer: SignerView | undefined): string {
  if (!signer) return t.t('ng-ui-insp-signer-none');
  const name = signerName(t, signer.backend);
  if (signer.account) return `${name} · ${signer.account}`;
  if (signer.backend === 'local_bku') return t.t('ng-ui-insp-signer-card-reader');
  return name;
}

/** CSS-Klasse der Farbe je Signiermittel (`signer-idaustria`, `signer-keystore`, `signer-card`). */
export function signerClass(backend: BackendId | null): string {
  switch (backend) {
    case 'atrust_native':
      return 'signer-idaustria';
    case 'keystore':
      return 'signer-keystore';
    case 'local_bku':
      return 'signer-card';
    default:
      return 'signer-ask';
  }
}

/** Symbol im Konto-Chip und im Menü. */
export function SignerIcon({ backend, size = 18 }: { backend: BackendId | null; size?: number }) {
  if (backend === 'keystore') return <IconFileKey size={size} />;
  if (backend === 'local_bku') return <IconCard size={size} />;
  if (backend === 'atrust_native') return <IconPhone size={size} />;
  return <IconShield size={size} />;
}

/** Symbol im Kopf der Signierkarte (wie im Prototyp). */
export function SignerGlyph({ backend }: { backend: BackendId | null }) {
  return (
    <span className={`signer-glyph ${signerClass(backend)}`}>
      {backend === 'keystore' ? <IconLock size={20} /> : backend === 'local_bku' ? <IconShield size={20} /> : <IconPhone size={20} />}
    </span>
  );
}
