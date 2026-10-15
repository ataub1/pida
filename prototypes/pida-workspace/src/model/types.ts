/**
 * Datenmodell des Prototyps. Alle Koordinaten in PDF-Punkten (1 pt = 1/72 Zoll), Ursprung **oben links**
 * der Seite (wie `PAGE_REL` in PDF-Over, docs/analyse/01 §4.3). Seiten sind 0-basiert, angezeigt 1-basiert.
 */

export type Lang = 'de' | 'en';
export type ThemePref = 'system' | 'light' | 'dark';
export type SignerKind = 'idaustria' | 'keystore' | 'card';

/** Antwort des simulierten Signiermittels (Demo-Steuerung). */
export type DemoResponse = 'app' | 'fido2' | 'sms' | 'cancel' | 'timeout';

export type ScenarioId =
  | 'start'
  | 'blank'
  | 'contract'
  | 'field'
  | 'signed'
  | 'batch'
  | 'verify'
  | 'error-password'
  | 'error-atrust';

export type View = 'start' | 'workspace' | 'batch' | 'done' | 'verify';

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Inhaltselemente einer Mock-Seite. `rect` ist die belegte Fläche (für Freiflächen/Überdeckung). */
export type Block =
  | { kind: 'letterhead'; rect: Rect; org: string; lines: string[] }
  | { kind: 'title'; rect: Rect; text: string }
  | { kind: 'subtitle'; rect: Rect; text: string }
  | { kind: 'heading'; rect: Rect; text: string }
  | { kind: 'para'; rect: Rect; text: string }
  | { kind: 'list'; rect: Rect; items: string[] }
  | { kind: 'parties'; rect: Rect; lines: string[] }
  | { kind: 'table'; rect: Rect; columns: string[]; rows: string[][]; numeric: boolean[]; totalRow: boolean }
  | { kind: 'form'; rect: Rect; rows: [string, string][] }
  | { kind: 'sigline'; rect: Rect; left: [string, string]; right: [string, string] }
  | { kind: 'footer'; rect: Rect; text: string }
  | { kind: 'signature'; rect: Rect; signatureId: string };

export interface PageSpec {
  w: number;
  h: number;
  blocks: Block[];
}

/** Leeres Signaturfeld (AcroForm) oder QR-Code-Platzhalter von PDF-AS. */
export interface SignatureField {
  id: string;
  kind: 'field' | 'qr';
  /** technischer Feldname (AcroForm `/T`) */
  name: string;
  /** Anzeigename */
  label: string;
  page: number;
  rect: Rect;
}

/** Vorhandene Signatur (Prüfergebnis, Mock). */
export interface ExistingSignature {
  id: string;
  signer: string;
  role: string;
  /** ISO 8601 mit Offset */
  time: string;
  status: 'valid' | 'invalid' | 'unknown';
  level: 'QES' | 'AdES';
  method: string;
  issuer: string;
  serial: string;
  validFrom: string;
  validTo: string;
  algorithm: string;
  format: string;
  page: number | null;
  rect: Rect | null;
  /** Dokument seit dieser Signatur unverändert (bzw. nur signaturbezogene Ergänzungen) */
  coversWholeDocument: boolean;
}

export interface DocSpec {
  id: string;
  fileName: string;
  folder: string;
  sizeKb: number;
  pages: PageSpec[];
  fields: SignatureField[];
  signatures: ExistingSignature[];
  /** mit Kennwort zum Öffnen geschützt (Demo-Kennwort „demo“) */
  locked: boolean;
}

/** Position des Signaturblocks; `fieldId` gesetzt = im Signaturfeld bzw. Platzhalter. */
export interface Placement {
  page: number;
  x: number;
  y: number;
  fieldId: string | null;
}

export type DocStatus = 'ready' | 'positioned' | 'signing' | 'done' | 'error';

export interface DocState {
  spec: DocSpec;
  status: DocStatus;
  placement: Placement | null;
  invisible: boolean;
  newPage: boolean;
  unlocked: boolean;
  outputName: string | null;
}

export type SigProfile = 'standard' | 'amt';

export interface SigBlockSettings {
  profile: SigProfile;
  lang: Lang;
  /** Bildmarke als Data-URL (lokal gelesen, nie hochgeladen) */
  logo: string | null;
  logoName: string | null;
  /** `null` = Standardhinweis des Profils in der Blocksprache */
  note: string | null;
}

export type SaveTarget = 'original' | 'folder' | 'ask';
export type ProxyMode = 'system' | 'none' | 'manual';

export interface Settings {
  theme: ThemePref;
  uiLang: Lang;
  sigBlock: SigBlockSettings;
  defaultSigner: SignerKind | 'ask';
  idaUser: string;
  rememberUser: boolean;
  fido2Default: boolean;
  keystoreFile: string;
  keystoreAlias: string;
  saveTarget: SaveTarget;
  saveFolder: string;
  namePattern: string;
  proxy: ProxyMode;
  proxyHost: string;
  proxyPort: string;
  autoUpdate: boolean;
  debugLog: boolean;
  placeholderSearch: boolean;
}

export interface RecentItem {
  id: string;
  fileName: string;
  /** ISO-Zeitpunkt */
  signedAt: string;
  signer: SignerKind;
  pages: number;
}

export interface Toast {
  id: number;
  kind: 'success' | 'info' | 'error';
  title: string;
  text?: string;
  action?: { label: string; run: () => void };
}
