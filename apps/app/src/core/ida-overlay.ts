/**
 * ID Austria im Browser (Konzept Webdienst, Variante E): A-Trust-Overlay und Rückkanal für die SL-Befehlskette
 * des Kerns (`pida-backend-atrust-web`, Rückruf `sl(art, id, xml)` aus `apps/web`).
 *
 * - **Erster Befehl:** Sitzung beim Rückkanal anlegen (`POST api/sl/sessions` → Kennung und Token), Overlay mit
 *   einem iframe öffnen und den Befehl per Formular-POST (`XMLRequest`, `DataURL`) an A-Trust schicken – kein
 *   CORS nötig; Anmeldung, TAN, App und FIDO2 laufen auf der echten A-Trust-Seite (`allow` für WebAuthn).
 *   Funktioniert das iframe nicht (z. B. Drittanbieter-Cookies in Safari), öffnet „In eigenem Fenster öffnen“
 *   denselben Befehl in einem Popup.
 * - **Antwort:** A-Trust schickt sie an die DataURL; die Seite holt sie per Long-Poll
 *   (`GET api/sl/sessions/<id>/response`, Bearer-Token) und gibt sie dem Kern.
 * - **Weitere Befehle:** `POST …/command` – der Rückkanal antwortet A-Trust damit (dieselbe Anmeldung).
 * - **Ende:** `POST …/finish` (A-Trust zeigt die Abschlussseite), Overlay schließen; Abbrechen → `…/abort`.
 *
 * Das Overlay zeigt die Herkunft der A-Trust-Seite und überdeckt sie nicht (Clickjacking-Schutz, Konzept).
 */
import { translator, type Translator } from '../i18n/catalog';
import type { Lang } from './types';
import './ida-overlay.css';

/** Antwort an den Kern (`sl_resolve`). */
export type SlResolve = (id: number, ok: boolean, response: Uint8Array, error: string) => void;

interface RelayConfig {
  /** SL-Endpunkt von A-Trust (bzw. Testgegenstelle des Servers) */
  sl_endpoint: string;
}

interface Session {
  id: string;
  token: string;
  /** erster Befehl (für „In eigenem Fenster öffnen“, solange noch keine Antwort kam) */
  first: string | null;
  overlay: Overlay;
  abort: AbortController;
}

/** Längste Wartezeit auf eine Antwort von A-Trust (Anmeldung, TAN, App-Bestätigung). */
const ANSWER_TIMEOUT_MS = 10 * 60 * 1000;
const IFRAME_NAME = 'pida-atrust';
const POPUP_NAME = 'pida-atrust-popup';

class Overlay {
  readonly root: HTMLDivElement;
  private readonly frame: HTMLIFrameElement;
  private readonly popupButton: HTMLButtonElement;
  private readonly note: HTMLParagraphElement;
  private readonly title: HTMLParagraphElement;
  private readonly cancel: HTMLButtonElement;
  /** Fokus vor dem Öffnen (Knopf „Anmelden“ bzw. Signierkarte), kommt beim Schließen zurück */
  private readonly returnFocus: Element | null;
  private popup: Window | null = null;

  constructor(t: Translator, host: string, onPopup: () => void, onCancel: () => void) {
    this.returnFocus = document.activeElement;
    this.root = document.createElement('div');
    this.root.className = 'ida-overlay';
    this.root.setAttribute('role', 'dialog');
    this.root.setAttribute('aria-modal', 'true');
    // über dem Dialog der Signierkarte (react-aria): nicht `inert`, Fokus darf ins iframe, Klicks schließen nichts
    this.root.setAttribute('data-react-aria-top-layer', 'true');
    this.root.setAttribute('aria-label', t.t('ng-web-ida-title'));
    const panel = document.createElement('div');
    panel.className = 'ida-panel';
    const head = document.createElement('div');
    head.className = 'ida-head';
    const titles = document.createElement('div');
    const title = document.createElement('p');
    title.className = 'ida-title';
    title.textContent = t.t('ng-web-ida-title');
    // Ziel des Fokus beim Öffnen: Screenreader nennen den Dialog, Tab führt zu den Schaltflächen und ins iframe
    title.tabIndex = -1;
    this.title = title;
    const origin = document.createElement('p');
    origin.className = 'ida-origin';
    origin.textContent = t.t('ng-web-ida-origin', { host });
    titles.append(title, origin);
    this.popupButton = document.createElement('button');
    this.popupButton.type = 'button';
    this.popupButton.className = 'btn btn-ghost btn-sm';
    this.popupButton.textContent = t.t('ng-web-atrust-popup');
    this.popupButton.addEventListener('click', onPopup);
    const cancel = (this.cancel = document.createElement('button'));
    cancel.type = 'button';
    cancel.className = 'btn btn-secondary btn-sm';
    cancel.textContent = t.t('ng-web-atrust-close');
    cancel.addEventListener('click', onCancel);
    head.append(titles, this.popupButton, cancel);
    this.note = document.createElement('p');
    this.note.className = 'ida-note';
    this.note.setAttribute('role', 'status');
    this.note.hidden = true;
    this.frame = document.createElement('iframe');
    this.frame.name = IFRAME_NAME;
    this.frame.className = 'ida-frame';
    this.frame.title = t.t('ng-web-ida-title');
    // FIDO2 auf der A-Trust-Seite (WebAuthn in einem Cross-Origin-iframe)
    this.frame.allow = 'publickey-credentials-get *';
    this.frame.referrerPolicy = 'no-referrer';
    panel.append(head, this.note, this.frame);
    // Fokusfalle (WCAG 2.1.2/2.4.3): Wächter vor und nach dem Panel – Tab aus dem iframe heraus führt zurück
    // zu den Schaltflächen, Umschalt+Tab davor ins iframe; die Signierkarte dahinter ist für Tab nicht erreichbar
    const guard = (onFocus: () => void) => {
      const g = document.createElement('span');
      g.tabIndex = 0;
      g.className = 'ida-guard';
      g.addEventListener('focus', onFocus);
      return g;
    };
    this.root.append(
      guard(() => this.focusLast()),
      panel,
      guard(() => this.focusFirst()),
    );
    this.root.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') onCancel();
    });
    document.body.append(this.root);
    this.title.focus();
  }

  private focusFirst(): void {
    (this.popupButton.hidden ? this.cancel : this.popupButton).focus();
  }

  private focusLast(): void {
    if (this.frame.isConnected) this.frame.focus();
    else this.cancel.focus();
  }

  /** Formular-POST an A-Trust ins iframe bzw. Popup. */
  submit(endpoint: string, xml: string, dataUrl: string, target: string): void {
    const form = document.createElement('form');
    form.method = 'post';
    form.action = endpoint;
    form.target = target;
    form.acceptCharset = 'UTF-8';
    form.hidden = true;
    for (const [name, value] of [
      ['XMLRequest', xml],
      ['DataURL', dataUrl],
    ] as const) {
      const input = document.createElement('input');
      input.type = 'hidden';
      input.name = name;
      input.value = value;
      form.append(input);
    }
    document.body.append(form);
    form.submit();
    form.remove();
  }

  /** Popup statt iframe; `false` = vom Browser blockiert. */
  openPopup(t: Translator): boolean {
    this.popup = window.open('about:blank', POPUP_NAME, 'popup,width=520,height=760');
    if (!this.popup) {
      this.showNote(t.t('ng-web-atrust-popup-blocked'));
      return false;
    }
    this.frame.remove();
    this.popupButton.hidden = true;
    this.showNote(t.t('ng-web-atrust-popup-hint'));
    // der Knopf ist weg: Fokus nicht ins Leere fallen lassen
    this.cancel.focus();
    return true;
  }

  /** Nach der ersten Antwort gibt es keinen neuen Anlauf im Popup mehr (die Kette läuft schon). */
  chainRunning(): void {
    this.popupButton.hidden = true;
  }

  private showNote(text: string): void {
    this.note.textContent = text;
    this.note.hidden = false;
  }

  close(): void {
    this.popup?.close();
    const hadFocus = this.root.contains(document.activeElement);
    this.root.remove();
    if (hadFocus && this.returnFocus instanceof HTMLElement && this.returnFocus.isConnected) this.returnFocus.focus();
  }
}

/** Rückruf `sl(art, id, xml)` für den Kern. */
export function createSlTransport(
  base: string,
  resolve: SlResolve,
  lang: () => Lang,
  /** „Abbrechen“ im Overlay: den Signiervorgang im Kern still beenden (wie das × der Signierkarte) */
  onUserCancel: () => void,
): (kind: string, id: number, payload: string) => void {
  const api = (path: string) => new URL(`api/sl/${path}`, base).href;
  let session: Session | null = null;
  let config: RelayConfig | null = null;

  const headers = (s: Session) => ({ Authorization: `Bearer ${s.token}` });

  const end = (s: Session, outcome: 'done' | 'aborted') => {
    s.abort.abort();
    s.overlay.close();
    if (session === s) session = null;
    void fetch(api(`sessions/${s.id}/${outcome === 'done' ? 'finish' : 'abort'}`), { method: 'POST', headers: headers(s) }).catch(() => undefined);
  };

  /** Long-Poll bis zur Antwort von A-Trust. */
  const awaitAnswer = async (s: Session): Promise<Uint8Array> => {
    const deadline = Date.now() + ANSWER_TIMEOUT_MS;
    while (Date.now() < deadline) {
      const r = await fetch(api(`sessions/${s.id}/response`), { headers: headers(s), signal: s.abort.signal, cache: 'no-store' });
      if (r.status === 200) return new Uint8Array(await r.arrayBuffer());
      if (r.status === 204) continue;
      throw new Error(`Rückkanal: HTTP ${r.status}`);
    }
    throw new Error('timeout');
  };

  const fail = (id: number, s: Session | null, e: unknown) => {
    const cancelled = s?.abort.signal.aborted || (e instanceof DOMException && e.name === 'AbortError');
    const code = cancelled ? 'cancelled' : e instanceof Error && e.message === 'timeout' ? 'timeout' : e instanceof Error ? e.message : String(e);
    if (s && session === s) end(s, 'aborted');
    resolve(id, false, new Uint8Array(), code);
  };

  const start = async (id: number, xml: string) => {
    let s: Session | null = null;
    try {
      config ??= (await fetch(new URL('api/config', base).href, { cache: 'no-store' }).then((r) => {
        if (!r.ok) throw new Error(`Rückkanal: HTTP ${r.status}`);
        return r.json();
      })) as RelayConfig;
      // absolut (A-Trust) bzw. relativ zur Seite (Testgegenstelle des Servers)
      const endpoint = new URL(config.sl_endpoint, base).href;
      const created = await fetch(api('sessions'), { method: 'POST' }).then((r) => {
        if (!r.ok) throw new Error(`Rückkanal: HTTP ${r.status}`);
        return r.json() as Promise<{ id: string; token: string }>;
      });
      const t = translator(lang());
      const dataUrl = api(`data/${created.id}`);
      const abort = new AbortController();
      const overlay = new Overlay(
        t,
        new URL(endpoint).host,
        () => {
          if (s?.first && overlay.openPopup(t)) overlay.submit(endpoint, s.first, dataUrl, POPUP_NAME);
        },
        () => {
          // zuerst der Kern (beendet den Vorgang still und schließt die Kette über `close`); trennen nur als
          // Rückfall – sonst käme der Verbindungsabbruch vor dem Abbruch an und gälte als „abgelehnt“
          onUserCancel();
          const current = s;
          setTimeout(() => {
            if (current && session === current) abort.abort();
          }, 1500);
        },
      );
      s = { id: created.id, token: created.token, first: xml, overlay, abort };
      session = s;
      overlay.submit(endpoint, xml, dataUrl, IFRAME_NAME);
      const answer = await awaitAnswer(s);
      s.first = null;
      overlay.chainRunning();
      resolve(id, true, answer, '');
    } catch (e) {
      fail(id, s, e);
    }
  };

  const next = async (s: Session, id: number, xml: string) => {
    try {
      const r = await fetch(api(`sessions/${s.id}/command`), {
        method: 'POST',
        headers: { ...headers(s), 'Content-Type': 'text/xml; charset=utf-8' },
        body: xml,
        signal: s.abort.signal,
      });
      if (!r.ok) throw new Error(`Rückkanal: HTTP ${r.status}`);
      resolve(id, true, await awaitAnswer(s), '');
    } catch (e) {
      fail(id, s, e);
    }
  };

  return (kind, id, payload) => {
    if (kind === 'exchange') {
      if (session) void next(session, id, payload);
      else void start(id, payload);
    } else if (kind === 'close' && session) {
      end(session, payload === 'done' ? 'done' : 'aborted');
    }
  };
}
