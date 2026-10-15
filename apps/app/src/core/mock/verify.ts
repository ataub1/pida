/**
 * Mock-Kern, Bereich Prüfmodus (Paket UI-B, Platzhalter aus U0; ADR 0004 §3.9). Liefert mit
 * `examples` bzw. dem Szenario `verify` ein Beispiel für `VerifyView` (zwei Signaturen, eine davon nach dem
 * Signieren verändert) und behandelt Öffnen, Schließen und „Dieses Dokument signieren“ (Signal
 * `open-files`). B4 baut darauf den Prüfmodus.
 */
import type { VerifyView } from '../types';
import { fileNameOf, type MockArea } from './shared';

export interface VerifyState {
  view: VerifyView | null;
  path: string | null;
}

/** Beispielergebnis (frei erfundene Personen und Zertifikate). */
export function exampleVerifyView(fileName = 'Kaufvertrag_Fahrzeug_signiert.pdf'): VerifyView {
  const base = {
    organization: null,
    issuer: 'a-sign-premium-mobile-07 (Demo-CA)',
    cert_valid_from: '2024-11-20T00:00:00+01:00',
    cert_valid_to: '2029-11-20T00:00:00+01:00',
    cert_valid_at_signing: true,
    qualification: { kind: 'qualified' as const, qscd: true },
    format: 'pades' as const,
    digest: 'SHA-256',
    algorithm: 'ECDSA P-256',
    warnings: [],
    trust: 'not_checked' as const,
  };
  return {
    file_name: fileName,
    status: { state: 'ready' },
    doc: { id: 900, name: fileName, page_count: 1, page_sizes: [[595.28, 841.89]], signed: true },
    signatures: [
      {
        ...base,
        index: 1,
        field_name: 'Signature1',
        signer: 'Max Mustermann',
        serial: '1288004912',
        signing_time: '2026-10-14T09:12:44+02:00',
        integrity: { kind: 'intact' },
        coverage: { kind: 'revision', later_revisions: 1, later_signatures: 1 },
        appearance: { page: 0, rect: { left: 56, bottom: 160, right: 286, top: 242 } },
      },
      {
        ...base,
        index: 2,
        field_name: 'Signature2',
        signer: 'Sabine Hofer',
        serial: '1297551030',
        signing_time: '2026-10-14T17:40:02+02:00',
        integrity: { kind: 'broken', code: 'cms-digest-mismatch' },
        coverage: { kind: 'whole_document' },
        appearance: { page: 0, rect: { left: 309, bottom: 160, right: 539, top: 242 } },
      },
    ],
    summary: 'some_broken',
    revisions: 3,
  };
}

export const verifyArea: MockArea<VerifyState> = {
  init: (o) => ({ view: o.examples || o.scenario === 'verify' ? exampleVerifyView() : null, path: null }),
  handle(e, state, ctx) {
    switch (e.type) {
      case 'open_verify':
        state.path = e.path;
        state.view = exampleVerifyView(fileNameOf(e.path));
        return true;
      case 'close_verify':
        state.view = null;
        state.path = null;
        return true;
      case 'sign_verified':
        if (state.path) ctx.emit({ kind: 'open-files', paths: [state.path] });
        state.view = null;
        state.path = null;
        return true;
      default:
        return false;
    }
  },
  project: (state) => ({ verify: state.view ? structuredClone(state.view) : null }),
};
