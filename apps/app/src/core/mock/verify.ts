/**
 * Mock-Kern, Bereich Prüfmodus (Paket UI-B; ADR 0004 §3.9) – wie K4: `open_verify` lädt die Datei („wird
 * geprüft“, dann das Ergebnis), `verify_recent` kommt über das Signal `verify-file` aus „Zuletzt signiert“,
 * `close_verify` schließt, `sign_verified` schließt und öffnet die Datei wie `open_files` (Signal
 * `open-files`). Der Prüfmodus liegt über dem Workflow-Zustand und ändert ihn nicht; erlaubt ist er wie im
 * Kern nur im Leerlauf, beim Positionieren, auf „Fertig“ und in der Stapelübersicht (`verifyAllowed`, Freigabe
 * `actions.verify`).
 *
 * Die Ergebnisse sind Beispiele (frei erfundene Personen und Zertifikate) und hängen nur vom Dateinamen ab,
 * damit Bedienung, Tests und Screenshots jeden Zustand erreichen – Prüflogik ist das nicht:
 *
 * | Dateiname enthält            | Ergebnis                                                                 |
 * |------------------------------|--------------------------------------------------------------------------|
 * | (sonst)                      | zwei Signaturen, beide intakt, QES laut Zertifikat (`all_intact`)        |
 * | `manipul`, `veraendert`      | die zweite Signatur ist verändert (`some_broken`)                        |
 * | `sha1`                       | SHA-1-Hinweis und `adbe.pkcs7.sha1` nicht prüfbar (`some_not_verifiable`) |
 * | `ergaenzt`, `nachtrag`       | nach der letzten Signatur ergänzt (`modified_after_last_signature`)      |
 * | `unsichtbar`                 | eine unsichtbare Signatur (ohne Signaturbild)                            |
 * | `unsigniert`, `ohne-signatur`| keine Signatur (`no_signatures`)                                         |
 * | `defekt`, `kein-pdf`         | Datei nicht prüfbar (`ng-verify-open-failed`)                            |
 */
import type { StepView, VerifiedSignatureView, VerifyView, ViewState } from '../types';
import { fileNameOf, type MockArea } from './shared';
import { contractSpec, pageTileSvg, signedSpec, toRectPt, type PageSpec, type Rect } from './workspace';

/** Eigene `DocId` der Prüfdokumente (nicht in der Warteschlange, Kacheln über `pida-render://page/900/…`). */
export const VERIFY_DOC_ID = 900;

export interface VerifyState {
  view: VerifyView | null;
  path: string | null;
  pages: PageSpec[];
  /** Ladevorgang (verworfen, wenn inzwischen eine andere Datei geöffnet bzw. geschlossen wurde) */
  seq: number;
}

type Variant = 'intact' | 'broken' | 'sha1' | 'modified' | 'invisible' | 'unsigned' | 'failed';

function variantOf(fileName: string): Variant {
  const n = fileName.toLowerCase();
  if (/defekt|kein-pdf/.test(n)) return 'failed';
  if (/unsigniert|ohne-signatur/.test(n)) return 'unsigned';
  if (/manipul|veraendert|verändert/.test(n)) return 'broken';
  if (/sha-?1/.test(n)) return 'sha1';
  if (/ergaenzt|ergänzt|nachtrag/.test(n)) return 'modified';
  if (/unsichtbar/.test(n)) return 'invisible';
  return 'intact';
}

const CERT = {
  organization: null,
  issuer: 'a-sign-premium-mobile-07 (Demo-CA)',
  cert_valid_from: '2024-11-20T00:00:00+01:00',
  cert_valid_to: '2029-11-20T00:00:00+01:00',
  cert_valid_at_signing: true,
  qualification: { kind: 'qualified', qscd: true },
  format: 'pades',
  digest: 'SHA-256',
  algorithm: 'ECDSA P-256',
  warnings: [],
  trust: 'not_checked',
} satisfies Partial<VerifiedSignatureView>;

/** Seiten und Signaturen eines Beispiels (Mietvertrag mit zwei Signaturbildern auf Seite 2). */
function example(variant: Variant): { pages: PageSpec[]; signatures: VerifiedSignatureView[]; revisions: number } {
  if (variant === 'unsigned' || variant === 'failed') return { pages: contractSpec().pages, signatures: [], revisions: 1 };
  const spec = signedSpec();
  const pages = spec.pages.map((p) => ({ ...p, blocks: [...p.blocks] }));
  const last = pages.length - 1;
  const lastPage = pages[last] as PageSpec;
  const first = spec.existing[0]?.rect as Rect;
  const second: Rect = { ...first, x: lastPage.w - 56 - first.w };
  const t1 = '2026-10-12T14:03:51+02:00';
  const t2 = '2026-10-12T18:20:07+02:00';
  if (variant !== 'invisible') lastPage.blocks.push({ kind: 'signature', rect: second, signer: 'Max Mustermann', time: t2 });
  const appearance = (r: Rect) => ({ page: last, rect: toRectPt(r, lastPage.h) });
  const sig1: VerifiedSignatureView = {
    ...CERT,
    index: 1,
    field_name: 'Signature1',
    signer: 'Dr. Anna Gruber',
    serial: '1288004912',
    signing_time: t1,
    integrity: { kind: 'intact' },
    coverage: { kind: 'revision', later_revisions: 1, later_signatures: 1 },
    appearance: appearance(first),
  };
  const sig2: VerifiedSignatureView = {
    ...CERT,
    index: 2,
    field_name: 'Signature2',
    signer: 'Max Mustermann',
    serial: '1297551030',
    signing_time: t2,
    integrity: { kind: 'intact' },
    coverage: { kind: 'whole_document' },
    appearance: appearance(second),
  };
  switch (variant) {
    case 'broken':
      sig2.integrity = { kind: 'broken', code: 'cms-digest-mismatch' };
      break;
    case 'sha1':
      Object.assign(sig1, { digest: 'SHA-1', algorithm: 'RSA 2048', warnings: ['ng-verify-warning-sha1'], qualification: { kind: 'unknown' } });
      Object.assign(sig2, {
        format: 'pkcs7_sha1',
        digest: 'SHA-1',
        algorithm: 'RSA 2048',
        integrity: { kind: 'not_verifiable', code: 'pdf-signature-subfilter' },
        qualification: { kind: 'not_qualified' },
        organization: 'Muster IT-Dienstleistungen GmbH',
      });
      break;
    case 'modified':
      sig2.coverage = { kind: 'revision', later_revisions: 1, later_signatures: 0 };
      return { pages, signatures: [sig1, sig2], revisions: 3 };
    case 'invisible':
      return { pages, signatures: [sig1, { ...sig2, appearance: null, qualification: { kind: 'qualified', qscd: false } }], revisions: 2 };
    default:
      break;
  }
  return { pages, signatures: [sig1, sig2], revisions: 2 };
}

function summaryOf(signatures: VerifiedSignatureView[], variant: Variant): VerifyView['summary'] {
  if (signatures.length === 0) return 'no_signatures';
  if (signatures.some((s) => s.integrity.kind === 'broken')) return 'some_broken';
  if (signatures.some((s) => s.integrity.kind === 'not_verifiable')) return 'some_not_verifiable';
  return variant === 'modified' ? 'modified_after_last_signature' : 'all_intact';
}

/** Prüfergebnis eines Beispiels (`status: ready` bzw. `failed`) samt Seiten für die Kacheln. */
export function exampleVerify(fileName = 'Mietvertrag_signiert.pdf'): { view: VerifyView; pages: PageSpec[] } {
  const variant = variantOf(fileName);
  if (variant === 'failed') {
    return { view: { file_name: fileName, status: { state: 'failed', code: 'ng-verify-open-failed' }, doc: null, signatures: [], summary: 'no_signatures', revisions: 0 }, pages: [] };
  }
  const { pages, signatures, revisions } = example(variant);
  return {
    view: {
      file_name: fileName,
      status: { state: 'ready' },
      doc: { id: VERIFY_DOC_ID, name: fileName, page_count: pages.length, page_sizes: pages.map((p) => [p.w, p.h] as [number, number]), signed: signatures.length > 0 },
      signatures,
      summary: summaryOf(signatures, variant),
      revisions,
    },
    pages,
  };
}

/** Beispiel für `VerifyView` (Abnahme E0: „je neuem Typ ein Beispiel“). */
export function exampleVerifyView(fileName?: string): VerifyView {
  return exampleVerify(fileName).view;
}

/** Freigabe `actions.verify` wie K4: im Leerlauf, beim Positionieren, auf „Fertig“, in der Stapelübersicht. */
export function verifyAllowed(view: Pick<ViewState, 'step'>): boolean {
  const step: StepView['step'] = view.step.step;
  return step === 'open' || step === 'position' || step === 'output' || step === 'batch';
}

/** Seitenkachel eines Prüfdokuments (`null` = kein Prüfdokument mit dieser Kennung). */
export function verifyPageTile(state: VerifyState, doc: number, page: number, scale: number): string | null {
  if (state.view?.doc?.id !== doc) return null;
  const p = state.pages[page] ?? null;
  const size = state.view.doc.page_sizes[page] ?? state.view.doc.page_sizes[0] ?? [595.28, 841.89];
  return pageTileSvg(p, size, scale);
}

function open(state: VerifyState, path: string, later: (fn: () => void) => void): void {
  const fileName = fileNameOf(path);
  state.seq += 1;
  const seq = state.seq;
  state.path = path;
  state.pages = [];
  // wie der Kern: zuerst „wird geprüft“ (Laden in `spawn_blocking`), dann das Ergebnis
  state.view = { file_name: fileName, status: { state: 'loading' }, doc: null, signatures: [], summary: 'no_signatures', revisions: 0 };
  later(() => {
    if (state.seq !== seq) return;
    const r = exampleVerify(fileName);
    state.view = r.view;
    state.pages = r.pages;
  });
}

function close(state: VerifyState): void {
  state.seq += 1;
  state.view = null;
  state.path = null;
  state.pages = [];
}

export const verifyArea: MockArea<VerifyState> = {
  init: (o) => {
    if (o.scenario !== 'verify') return { view: null, path: null, pages: [], seq: 0 };
    const r = exampleVerify();
    return { view: r.view, path: '/Users/demo/Dokumente/Wohnung/Mietvertrag_signiert.pdf', pages: r.pages, seq: 0 };
  },
  handle(e, state, ctx) {
    switch (e.type) {
      case 'open_verify':
        open(state, e.path, (fn) => ctx.later(fn));
        return true;
      case 'close_verify':
        close(state);
        return true;
      case 'open_files':
        // wie K4 (Merge E4): Öffnen zeigt die Datei im Arbeitsbereich, der Prüfmodus schließt; den
        // Ablauf behandelt der Bereich Arbeitsbereich
        if (e.paths.length > 0) close(state);
        return false;
      case 'sign_verified': {
        const path = state.path;
        const ready = state.view?.status.state === 'ready';
        close(state);
        if (path && ready) ctx.emit({ kind: 'open-files', paths: [path] });
        return true;
      }
      default:
        return false;
    }
  },
  signal(s, state, ctx) {
    if (s.kind === 'verify-file') open(state, s.path, (fn) => ctx.later(fn));
  },
  project: (state) => ({ verify: state.view ? structuredClone(state.view) : null }),
};
