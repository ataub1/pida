/**
 * Mock-Kern: gemeinsame Schnittstelle der Bereichsmodule (ADR 0004 §4.8, in E0 festgelegt).
 *
 * Jeder Bereich besitzt seinen Zustandsausschnitt; `shared.ts` kennt nur die Typen und kleine, reine
 * Hilfen. `index.ts` (UI-A) hält die Ausschnitte `{ flow (Kern, Arbeitsbereich, Signieren, Stapel),
 * settings, recent, firstrun }`, verteilt **jedes** Ereignis an alle Bereiche, leitet Signale
 * weiter und setzt die Projektionen zum `ViewState` zusammen. Kein Bereich schreibt in den Ausschnitt eines
 * anderen; bereichsübergreifend gibt es nur Signale (`MockSignal`) und die lesenden Zugriffe in `MockCtx`.
 * Neue Signalarten sind ein Nachtrag über den Orchestrator.
 *
 * Der Mock ist keine zweite Fachlogik: Regeln nur so weit, wie Bedienung und Tests sie brauchen. Keine
 * echten Zugangsdaten, kein Netz.
 */
import type { BackendId, BatchFlowView, Capabilities, InteractionView, Lang, OpenFilesMode, PlatformView, ResumeOfferView, Settings, UiEvent, ViewState } from '../types';

/** Szenarien wie im Prototyp (`?scenario=`). */
export type MockScenario = 'start' | 'blank' | 'contract' | 'field' | 'signed' | 'batch' | 'error-password' | 'error-atrust';

export const MOCK_SCENARIOS: readonly MockScenario[] = ['start', 'blank', 'contract', 'field', 'signed', 'batch', 'error-password', 'error-atrust'];

/** Ablauf der ID-Austria-Freigabe nach der Anmeldung (`decline` = in der App abgelehnt, `timeout` = abgelaufen). */
export type MockSigningFlow = 'app' | 'sms' | 'qr' | 'fido2' | 'decline' | 'timeout';

/** Optionen des Mock-Kerns. */
export interface MockOptions {
  /** simulierte Latenz in ms (Standard 350; Tests: 0) */
  delayMs?: number;
  /** UI-Sprache (sonst Einstellung bzw. `de`) */
  language?: Lang;
  /** Ausgangszustand wie im Prototyp (Standard `start`) */
  scenario?: MockScenario;
  /** QR-Platzhalter bzw. leere Signaturfelder in neu geöffneten Dokumenten */
  placeholder?: 'none' | 'fields' | 'qr';
  /** ID-Austria-Ablauf nach der Anmeldung */
  signingFlow?: MockSigningFlow;
  /** lokale BKU erreichbar */
  localBkuAvailable?: boolean;
  /**
   * Richtlinie `allowed_backends` (Desktop): nur diese Signiermittel sind freigegeben, die anderen gesperrt mit
   * `ng-signer-policy`. Bleibt nur eines, gibt es keine Signiermittel-Auswahl (ADR 0004 Nachtrag 20).
   */
  allowedBackends?: BackendId[];
  /** App-Bestätigung bzw. Chipkarte schließen sich nach einiger Zeit selbst ab; Tests: `false` */
  autoCompleteApp?: boolean;
  /** Einstellungen beim Start */
  settings?: Settings;
  /** vorhandene Dateien (Überschreiben-Rückfrage) */
  existingFiles?: string[];
  /** Antwort des Dateidialogs (`null` = abgebrochen) */
  pickFilesResult?: string[] | null;
  /** Antwort des Speichern-Dialogs (`null` = abgebrochen; sonst Vorschlag) */
  pickSaveResult?: string | null;
  /** Antwort des Ordnerdialogs */
  pickDirectoryResult?: string | null;
  /** FIDO2 verfügbar */
  fido2Available?: boolean;
  /** Schlüsselbund verfügbar */
  keychainAvailable?: boolean;
  /** Passwort des Keystores bereits gemerkt: die Einstellungen zeigen seine Schlüssel sofort (Testpasswort „test“) */
  keystorePasswordKnown?: boolean;
  /** Start direkt in diesem Zustand (Tests; Ereignisse werden nur protokolliert) */
  initialView?: ViewState;
  /** Warnungen der Eingangsanalyse (Codes, z. B. `pdf-usage-rights`) → Phase `warning` */
  warnings?: string[];
  /** Import-Angebot aus `~/.pdf-over` (Pfad der Altdatei) */
  migrationOffer?: string | null;
  /** die Altdatei enthält Klartext-Passwörter */
  migrationSecrets?: boolean;
  /** Einstellungen schreibgeschützt (`-c` mit Altdatei, unlesbare Datei) */
  readOnly?: boolean;
  /** Plattform (Standard: Desktop/macOS); Mobile: Teilen/Exportieren, nur ID Austria (kein Keystore), kein FIDO2 */
  platform?: 'desktop' | 'ios' | 'android';
  /** Angebot „Signatur fortsetzen“ beim Start (Mobile) */
  resumeOffer?: ResumeOfferView | null;
  /** Antwort der Mobile-Dateiauswahl (`[]` = abgebrochen) */
  pickMobileFilesResult?: string[];
  /** Mobile-Dateiauswahl: gewählt, aber von der nativen Seite nicht kopiert (`pickMobileDocuments`) */
  pickMobileSkipped?: number;
  /** eigener Datenschutztext per Richtlinie (`ViewState::privacy_text`) */
  privacyText?: string | null;
  /** Beispieldaten der späteren Etappen liefern (`recent`, `first_run`; Abnahme E0 „je neuem Typ ein Beispiel“) */
  examples?: boolean;
  /** Ersteinrichtung beim Start (Beispiel `FirstRunView`) */
  firstRun?: boolean;
  /**
   * Stapelübersicht bei ≥ 2 Dokumenten (`CoreOptions::batch_overview`, ADR 0004 §3.14; Standard an wie in
   * der App nach A2). `false` = mehrere Dokumente nacheinander über „Fertig“ → „Nächstes Dokument“.
   */
  batchOverview?: boolean;
  /**
   * Störungen im Stapellauf je Dateiname, jeweils nur beim ersten Versuch (ADR 0004 §3.8): `decline` =
   * Ablehnung in der App (Pause „Dokument“), `network` = Verbindungsfehler nach der Anmeldung (Pause
   * „Verbindung“, gleiche Sitzung), `session` = Sitzungsfehler (Pause mit neuer Sitzung), `too-large` =
   * nicht wiederholbarer Dokumentfehler (Zeile „Fehler“, der Lauf geht weiter).
   */
  batchFaults?: Record<string, MockBatchFault>;
  /** Ausgabeordner, die es nicht gibt (Stapel: Ordnerwahl vor dem Start, F-82) */
  missingFolders?: string[];
  /** Update-Prüfung („Über PIDA“, ADR 0004 Nachtrag 21; Standard `up_to_date`), siehe `mock/about.ts` */
  update?: MockUpdateScenario;
}

/** Ergebnis der simulierten Update-Prüfung (`MockOptions::update`, Browser-Vorschau `?update=`). */
export type MockUpdateScenario = 'up_to_date' | 'available' | 'failed' | 'offline' | 'unchecked' | 'checking';

/** Störung eines Dokuments im Stapellauf (`MockOptions::batchFaults`). */
export type MockBatchFault = 'decline' | 'network' | 'session' | 'too-large';

/** Bereichsübergreifende Signale statt direkter Zugriffe auf fremde Ausschnitte (ADR 0004 §4.8). */
export type MockSignal =
  | { kind: 'settings-changed'; settings: Settings } // settings → workspace (Blockgröße, Vorschau), signers
  | { kind: 'signed'; file_name: string; path: string } // signing/batch → recent
  | { kind: 'open-files'; paths: string[]; mode?: OpenFilesMode } // recent (SignRecent, Nachtrag 15) → workspace/batch
  | { kind: 'first-run-done' }; // firstrun → shared

/** Lesender Zugriff und Dienste für alle Bereiche. */
export interface MockCtx {
  /** Signal an alle Bereiche */
  emit(signal: MockSignal): void;
  /** aktuelle Einstellungen (Ausschnitt `settings`) */
  settings(): Settings;
  /** Plattform der Oberfläche */
  platform(): PlatformView;
  /** Hinweis ohne Rückfrage (`ViewState::notices`, gemeinsame Liste der Projektion) */
  notify(code: string, params?: Record<string, string>): void;
  /** später ausführen (simulierte Latenz × `factor`) und danach neu projizieren */
  later(fn: () => void, factor?: number): void;
  /** Aufruf eines nativen Dienstes protokollieren (Tests: `MockCore::nativeCalls`, z. B. `openRecent:<Pfad>`) */
  native(call: string): void;
}

/** Ein Bereich des Mock-Kerns mit eigenem Zustandsausschnitt. */
export interface MockArea<S> {
  init(options: MockOptions): S;
  /** `true` = Ereignis behandelt (jedes Ereignis geht an alle Bereiche) */
  handle(event: UiEvent, state: S, ctx: MockCtx): boolean;
  /** Signal eines anderen Bereichs (optional) */
  signal?(signal: MockSignal, state: S, ctx: MockCtx): void;
  project(state: S, ctx: MockCtx): Partial<ViewState>;
}

/* ---------------------------------------------------------------- reine Hilfen */

/**
 * „Passwort merken?“ bei der ersten Verwendung des Keystores (wie `CoreTask::interaction_view`): die
 * Keystore-Passwortabfrage fragt, solange `keystore.storage_chosen` falsch ist.
 */
export function withStorageQuestion(i: InteractionView, settings: Settings, keychainAvailable: boolean): InteractionView {
  if (i.kind !== 'secret' || i.prompt.kind !== 'KeystorePassword') return i;
  return { ...i, ask_storage: !settings.keystore.storage_chosen, keychain_available: keychainAvailable };
}

export const A4: [number, number] = [595.28, 841.89];

/** App-Sandbox der Mobile-Plattformen (Beispielpfad). */
export const SANDBOX = '/var/mobile/Containers/Data/Application/DEMO';

/** Anzeigename eines Pfads (beide Trenner). */
export function fileNameOf(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Ordner eines Pfads (beide Trenner). */
export function folderOf(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i > 0 ? path.slice(0, i) : '';
}

export function joinPath(dir: string, name: string): string {
  if (!dir) return name;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

/** `<Basisname><Postfix>.<Erweiterung>` (F-81) – nur Simulation, die Regel liegt im Kern. */
export function outputFileName(input: string, postfix: string): string {
  const name = fileNameOf(input);
  const dot = name.lastIndexOf('.');
  if (dot <= 0) return `${name}${postfix}`;
  return `${name.slice(0, dot)}${postfix}${name.slice(dot)}`;
}

export function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

export function escapeXml(s: string): string {
  return s.replace(/[<>&"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

/** RFC 3339 mit lokalem Offset (wie der Kern, `2026-10-15T10:42:17+02:00`). */
export function isoWithOffset(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0');
  const off = -d.getTimezoneOffset();
  const sign = off >= 0 ? '+' : '-';
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}${sign}${p(Math.floor(Math.abs(off) / 60))}:${p(Math.abs(off) % 60)}`;
}

/** Fähigkeiten der Signiermittel wie im Kern (Mobile: ID Austria ohne FIDO2). */
export function capabilitiesFor(id: BackendId, mobile = false): Capabilities {
  switch (id) {
    case 'atrust_native':
      return {
        input: 'Document',
        output: 'Cms',
        certificate: 'OnlyForVisibleBlock',
        interaction: 'NativeForms',
        server: 'None',
        max_batch: 1,
        login_once_per_batch: true,
        document_leaves_device: true,
        qualification: 'Qes',
        factors: { app: true, sms: true, qr: true, fido2: !mobile, card: false },
        cms_reserve: 16384,
      };
    case 'local_bku':
      return {
        input: 'Document',
        output: 'Cms',
        certificate: 'OnlyForVisibleBlock',
        interaction: 'External',
        server: 'None',
        max_batch: 1,
        login_once_per_batch: false,
        document_leaves_device: false,
        qualification: 'Qes',
        factors: { app: false, sms: false, qr: false, fido2: false, card: true },
        cms_reserve: 16384,
      };
    default:
      return {
        input: { Digest: { algs: ['Sha256', 'Sha384', 'Sha512'] } },
        output: 'RawSignature',
        certificate: 'Required',
        interaction: 'Secret',
        server: 'None',
        // wie `backend-keystore`: eine Freigabe für den ganzen Stapel (Passwort einmal)
        max_batch: 65535,
        login_once_per_batch: true,
        document_leaves_device: false,
        qualification: 'AdES',
        factors: { app: false, sms: false, qr: false, fido2: false, card: false },
        cms_reserve: null,
      };
  }
}

/**
 * Ablauf eines Stapellaufs aus den Fähigkeiten wie der Kern (`BatchFlowView::from_capabilities`, ADR 0004
 * Nachtrag 12) – nur für Mock und Testdaten; die Oberfläche liest `SignerView::batch_flow`.
 */
export function batchFlowFor(capabilities: Capabilities | null): BatchFlowView | null {
  if (!capabilities) return null;
  if (capabilities.max_batch > 1) return 'unlock_once';
  return capabilities.login_once_per_batch ? 'login_then_confirm_each' : 'confirm_each';
}

/** Telefonnummer bzw. Benutzername maskiert wie im Kern (`SignerView::account`): „+43 660 ···123“. */
export function maskAccount(user: string): string {
  const compact = user.replace(/\s+/g, '');
  const m = /^(\+\d{2})(\d{3})(\d+)$/.exec(compact);
  if (m && m[3]) return `${m[1]} ${m[2]} ···${m[3].slice(-3)}`;
  if (user.length > 6) return `${user.slice(0, 3)}···${user.slice(-2)}`;
  return user;
}
