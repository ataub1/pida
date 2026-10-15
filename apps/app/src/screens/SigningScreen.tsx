/**
 * Signieren (docs/analyse/01 §4.5–§4.7, §4.10): Warten mit Fortschritt, die nativen ID-Austria-Masken
 * (Szenario A: Anmeldung mit „Merken“, SMS-TAN mit Vergleichswert, QR-Code, App-Bestätigung mit
 * Countdown, FIDO2), Passwortabfragen (Keystore, Proxy), Meldungen (A-Trust-Interstitial/-Fehler,
 * HTTP/Netz, lokale BKU, Keystore) und „Signaturdaten anzeigen“ (F-68). Verlässt das Dokument mit dem
 * laufenden Signiermittel das Gerät (`document_leaves_device`, ID Austria), steht unter der Maske bzw. dem
 * Fortschritt der Kurzhinweis „wird an A-Trust übermittelt“ mit „Details“ (ADR 0003) – auch dann, wenn
 * das Signiermittel ohne Auswahl gestartet wurde (Standard-Signiermittel bzw. F-54 nach „Signieren“).
 *
 * Passwörter und TANs bleiben nur im Formularzustand, werden nie geloggt und nach dem Senden
 * verworfen (Komponente wird je Anfrage neu aufgebaut).
 */
import { useState, type FormEvent, type ReactNode } from 'react';
import { Checkbox, Form, Input, Label, ProgressBar, Text, TextField } from 'react-aria-components';
import { useApp } from '../app/context';
import { IconFido, IconPhone, IconQr } from '../components/icons';
import { PrivacyNote } from '../components/Privacy';
import { QueuePanel } from '../components/QueuePanel';
import { AccessButton, ConfirmDialog, Countdown, Markup, ReferenceValue, ScreenHeading, Spinner } from '../components/ui';
import type { Translator } from '../i18n/catalog';
import type {
  FormAnswerView,
  FormContext,
  FormProblem,
  InteractionView,
  NativeForm,
  Notice,
  NoticeAnswer,
  Progress,
  QrImage,
  QueueItemView,
  SecretPrompt,
} from '../core/types';
import { isMobile, variantName } from '../core/types';

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

/** Text eines Fortschritts. */
export function progressText(t: Translator, progress: Progress): string {
  if (typeof progress === 'string') {
    switch (progress) {
      case 'Connecting':
        return t.t('ng-ui-progress-connecting');
      case 'ReadingCertificate':
        return t.t('ng-ui-progress-certificate');
      case 'WaitingForServer':
        return t.t('ng-ui-progress-server');
      case 'Signing':
        return t.t('waiting-message');
      case 'Done':
        return t.t('ng-ui-progress-done');
    }
  }
  return t.t('ng-ui-progress-document', { index: progress.Document.index, total: progress.Document.total });
}

/** Titel und Text einer Meldung (docs/analyse/01 §4.10). */
export function noticeTexts(t: Translator, notice: Notice): { title: string; text: string } {
  const title =
    notice.kind === 'Error' ? t.t('error-Title') : notice.kind === 'Warning' ? t.t('common-warning') : t.t('common-info');
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
        text = t.t('error-KeyStoreFileNotExist', { arg1: raw });
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

/**
 * Doppelklickschutz wie PDF-Over (OK deaktiviert sich nach dem Klick). Lehnt die Bridge das Ereignis
 * ab (Hinweis erscheint), wird die Maske wieder freigegeben – sonst bliebe nur „Abbrechen“.
 */
function useSubmit(request: number) {
  const { dispatch } = useApp();
  const [sent, setSent] = useState(false);
  const send = (answer: FormAnswerView) => {
    setSent(true);
    void dispatch({ type: 'submit_form', request, answer }).then((ok) => {
      if (!ok) setSent(false);
    });
  };
  return { sent, send };
}

function FormShell({ title, icon, children }: { title: string; icon: ReactNode; children: ReactNode }) {
  return (
    <section className="card signing-card" aria-labelledby="signing-form-title">
      <div className="signing-card-head">
        <span className="signing-icon" aria-hidden="true">
          {icon}
        </span>
        <h2 id="signing-form-title" className="card-title">
          {title}
        </h2>
      </div>
      {children}
    </section>
  );
}

function ProblemLine({ ctx, local, fallback }: { ctx: FormContext; local: string | null; fallback?: string }) {
  const t = useApp().t;
  const text = local ?? (ctx.problem ? problemText(t, ctx.problem) : null);
  if (text) {
    return (
      <p className="message message-error" role="alert">
        {text}
      </p>
    );
  }
  return fallback ? <p className="message">{fallback}</p> : null;
}

function SignatureDataLink({ request, ctx }: { request: number; ctx: FormContext }) {
  const { t, dispatch } = useApp();
  if (!ctx.signature_data_url) return null;
  return (
    <p className="signature-data" title={t.t('mobileBKU-show_tooltip')}>
      <Markup text={t.t('mobileBKU-show')} targets={[{ onPress: () => dispatch({ type: 'open_signature_data', request }) }]} />
    </p>
  );
}

/**
 * Knöpfe „SMS anfordern“ / „FIDO2“ (sichtbar/aktiv nach `FormContext`). Mobile: bietet A-Trust FIDO2 an,
 * gibt es auf dem Gerät aber keinen Authenticator (Plan §4.7), bleibt der Knopf sichtbar, ausgegraut, mit
 * Hinweis (`ng-fido2-unavailable-mobile`).
 */
function SwitchButtons({ ctx, send, sent, sms = true }: { ctx: FormContext; send: (a: FormAnswerView) => void; sent: boolean; sms?: boolean }) {
  const { t, view } = useApp();
  const fido2Greyed = !ctx.fido2_visible && ctx.can_switch_to_fido2 && isMobile(view);
  return (
    <>
      {sms ? (
        <AccessButton msg="tanEnter-SMS" isDisabled={sent || !ctx.can_switch_to_sms} onPress={() => send({ kind: 'switch_to_sms' })} />
      ) : null}
      {ctx.fido2_visible ? (
        <AccessButton msg="tanEnter-FIDO2" isDisabled={sent || !ctx.can_switch_to_fido2} onPress={() => send({ kind: 'switch_to_fido2' })} />
      ) : null}
      {fido2Greyed ? (
        <>
          <AccessButton msg="tanEnter-FIDO2" isDisabled aria-describedby="fido2-unavailable" />
          <p id="fido2-unavailable" className="hint">
            {t.t('ng-fido2-unavailable-mobile')}
          </p>
        </>
      ) : null}
    </>
  );
}

function LoginForm({ request, data, lastUsername, onUsername }: {
  request: number;
  data: Extract<NativeForm, { Login: unknown }>['Login'];
  lastUsername: string;
  onUsername: (u: string) => void;
}) {
  const { t, dispatch } = useApp();
  const { sent, send } = useSubmit(request);
  const [username, setUsername] = useState(data.prefill_username ?? lastUsername);
  const [password, setPassword] = useState('');
  const [remember, setRemember] = useState(data.remember);
  const [error, setError] = useState<string | null>(null);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (sent) return;
    // Prüfung wie MobileBKUEnterNumberComposite (Passwort 5–200 Zeichen); der Kern prüft erneut.
    if (username.trim().length === 0) return setError(t.t('ng-ui-username-required'));
    if (password.length < 5) return setError(t.t('exception-PasswordTooShort'));
    if (password.length > 200) return setError(t.t('exception-PasswordTooLong'));
    setError(null);
    onUsername(username);
    send({ kind: 'login', username, password, remember });
    setPassword('');
  };
  return (
    <FormShell title={t.t('simple_config-MobileBKU_Title')} icon={<IconPhone size={30} />}>
      <Form className="form" onSubmit={submit}>
        <TextField className="field" value={username} onChange={setUsername} autoFocus={username === ''} autoComplete="username">
          <Label className="field-label">{t.t('mobileBKU-number')}</Label>
          <Input className="input" spellCheck={false} />
        </TextField>
        <TextField className="field" type="password" value={password} onChange={setPassword} autoFocus={username !== ''} autoComplete="current-password">
          <Label className="field-label">{t.t('mobileBKU-password')}</Label>
          <Input className="input" />
        </TextField>
        <Checkbox className="checkbox" isSelected={remember} onChange={setRemember}>
          <span className="checkbox-box" aria-hidden="true" />
          <span>
            {t.t('mobileBKU-rememberPassword')}
            <Text slot="description" className="hint block">
              {t.t('mobileBKU-rememberPasswordNote')}
            </Text>
          </span>
        </Checkbox>
        <ProblemLine ctx={data.ctx} local={error} fallback={t.t('mobileBKU-aTrustDisclaimer')} />
        <SignatureDataLink request={request} ctx={data.ctx} />
        <div className="actions">
          <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
          <AccessButton msg="common-Ok" variant="primary" type="submit" isDisabled={sent} />
        </div>
      </Form>
    </FormShell>
  );
}

function SmsTanForm({ request, data }: { request: number; data: Extract<NativeForm, { SmsTan: unknown }>['SmsTan'] }) {
  const { t, dispatch } = useApp();
  const { sent, send } = useSubmit(request);
  const [tan, setTan] = useState('');
  const [error, setError] = useState<string | null>(null);
  const trimmed = tan.trim();
  // Live-Warnung wie PDF-Over ab mehr als 3 Zeichen
  const warn = trimmed.length > 3 && data.reference_value.startsWith(trimmed);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (sent) return;
    if (trimmed.length === 0) return setError(t.t('error-NoTan'));
    if (data.reference_value.startsWith(trimmed)) return setError(t.t('error-EnteredReferenceValue'));
    if (trimmed.length > 6) return setError(t.t('error-TanTooLong'));
    setError(null);
    send({ kind: 'tan', tan: trimmed });
    setTan('');
  };
  return (
    <FormShell title={t.t('ng-ui-sms-title')} icon={<IconPhone size={30} />}>
      <Form className="form" onSubmit={submit}>
        <ReferenceValue value={data.reference_value} />
        <TextField className="field" value={tan} onChange={setTan} autoFocus autoComplete="one-time-code">
          <Label className="field-label">{t.t('tanEnter-TAN')}</Label>
          <Input className="input input-tan" spellCheck={false} />
          {warn ? (
            <Text slot="description" className="message message-warning">
              {t.t('error-EnteredReferenceValue')}
            </Text>
          ) : null}
        </TextField>
        <ProblemLine ctx={data.ctx} local={error} />
        <SignatureDataLink request={request} ctx={data.ctx} />
        <div className="actions">
          <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
          <SwitchButtons ctx={data.ctx} send={send} sent={sent} sms={false} />
          <AccessButton msg="common-Ok" variant="primary" type="submit" isDisabled={sent} />
        </div>
      </Form>
    </FormShell>
  );
}

/** QR-Bild als `data:`-URL; nur Bild-MIME-Typen (A-Trust-Angabe wird nicht blind übernommen). */
export function qrDataUrl(image: QrImage): string {
  let binary = '';
  for (const b of image.bytes) binary += String.fromCharCode(b & 0xff);
  const mime = image.mime && /^image\/(png|jpeg|gif|webp|svg\+xml)$/i.test(image.mime) ? image.mime.toLowerCase() : 'image/png';
  return `data:${mime};base64,${btoa(binary)}`;
}

function QrForm({ request, data }: { request: number; data: Extract<NativeForm, { QrCode: unknown }>['QrCode'] }) {
  const { t, dispatch } = useApp();
  const { sent, send } = useSubmit(request);
  return (
    <FormShell title={t.t('ng-ui-qr-title')} icon={<IconQr size={30} />}>
      <ReferenceValue value={data.reference_value} />
      <p>{t.t('tanEnter-QR')}</p>
      {data.image ? (
        <img className="qr-image" src={qrDataUrl(data.image)} alt={t.t('ng-ui-qr-alt')} width={220} height={220} />
      ) : (
        <p className="message message-error" role="alert">
          {t.t('error-FailedToLoadQRCode')}
        </p>
      )}
      <Countdown seconds={data.timeout_secs} resetKey={request} />
      <ProblemLine ctx={data.ctx} local={null} />
      <SignatureDataLink request={request} ctx={data.ctx} />
      <div className="actions">
        <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
        <SwitchButtons ctx={data.ctx} send={send} sent={sent} />
        <AccessButton msg="ng-ui-continue" variant="primary" isDisabled={sent} onPress={() => send({ kind: 'continue' })} />
      </div>
    </FormShell>
  );
}

function WaitForAppForm({ request, data }: { request: number; data: Extract<NativeForm, { WaitForApp: unknown }>['WaitForApp'] }) {
  const { t, dispatch } = useApp();
  const { sent, send } = useSubmit(request);
  return (
    <FormShell title={t.t('ng-ui-app-title')} icon={<IconPhone size={30} />}>
      {data.reference_value ? <ReferenceValue value={data.reference_value} /> : null}
      <p className="lead" role="status">
        {t.t('tanEnter-FP')}
      </p>
      <div className="app-wait" aria-hidden="true">
        <Spinner size={34} />
      </div>
      <Countdown seconds={data.timeout_secs} resetKey={request} />
      <ProblemLine ctx={data.ctx} local={null} />
      <SignatureDataLink request={request} ctx={data.ctx} />
      <div className="actions">
        <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
        <SwitchButtons ctx={data.ctx} send={send} sent={sent} />
        <AccessButton msg="ng-ui-continue" isDisabled={sent} onPress={() => send({ kind: 'continue' })} />
      </div>
    </FormShell>
  );
}

function Fido2Form({ request, data }: { request: number; data: Extract<NativeForm, { Fido2: unknown }>['Fido2'] }) {
  const { t, dispatch } = useApp();
  const { sent, send } = useSubmit(request);
  return (
    <FormShell title={t.t('ng-ui-fido2-title')} icon={<IconFido size={30} />}>
      <p className="lead">{t.t('ng-ui-fido2-hint')}</p>
      {data.in_progress ? (
        <p className="status-line" role="status">
          <Spinner size={20} /> {t.t('common-working')}
        </p>
      ) : null}
      <AccessButton
        msg="mobileBKU-authorize"
        variant="primary"
        className="btn-wide"
        isDisabled={data.in_progress || sent}
        onPress={() => send({ kind: 'start_fido2' })}
      />
      <ProblemLine ctx={data.ctx} local={null} />
      <SignatureDataLink request={request} ctx={data.ctx} />
      <div className="actions">
        <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
        <AccessButton msg="tanEnter-SMS" isDisabled={sent || !data.ctx.can_switch_to_sms} onPress={() => send({ kind: 'switch_to_sms' })} />
      </div>
    </FormShell>
  );
}

const SECRET_TEXT: Record<string, { title: string; prompt: string; retry: string }> = {
  KeystorePassword: { title: 'keystore_config-KeystoreStorePass', prompt: 'keystore-KeystoreStorePassEntry', retry: 'error-KeyStoreStorePass' },
  KeyPassword: { title: 'keystore_config-KeystoreKeyPass', prompt: 'keystore-KeystoreKeyPassEntry', retry: 'error-KeyStoreKeyPass' },
  ProxyPassword: { title: 'advanced_config-Proxy_Title', prompt: 'ng-ui-proxy-password-prompt', retry: 'ng-ui-proxy-password-retry' },
};

function SecretForm({ request, prompt }: { request: number; prompt: SecretPrompt }) {
  const { t, dispatch } = useApp();
  const [secret, setSecret] = useState('');
  const [sent, setSent] = useState(false);
  const texts = SECRET_TEXT[prompt.kind] ?? { title: 'ng-ui-secret-title', prompt: 'ng-ui-secret-prompt', retry: 'ng-ui-secret-retry' };
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (sent) return;
    setSent(true);
    void dispatch({ type: 'submit_secret', request, secret }).then((ok) => {
      if (!ok) setSent(false);
    });
    setSecret('');
  };
  return (
    <FormShell title={t.t(texts.title)} icon={<IconFido size={30} />}>
      <Form className="form" onSubmit={submit}>
        {prompt.subject ? <p className="hint">{t.t('ng-ui-secret-subject', { subject: prompt.subject })}</p> : null}
        {prompt.retry ? (
          <p className="message message-error" role="alert">
            {t.t(texts.retry)}
          </p>
        ) : null}
        <TextField className="field" type="password" value={secret} onChange={setSecret} autoFocus autoComplete="off">
          <Label className="field-label">{t.t(texts.prompt)}</Label>
          <Input className="input" />
        </TextField>
        <div className="actions">
          <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
          <AccessButton msg="common-Ok" variant="primary" type="submit" isDisabled={sent} />
        </div>
      </Form>
    </FormShell>
  );
}

function NoticeDialog({ request, notice }: { request: number; notice: Notice }) {
  const { t, dispatch } = useApp();
  const { title, text } = noticeTexts(t, notice);
  const answer = (a: NoticeAnswer) => dispatch({ type: 'answer_notice', request, answer: a });
  const cancel = notice.buttons === 'Ok' ? () => answer('Ok') : () => answer('Cancel');
  return (
    <ConfirmDialog
      title={title}
      tone={notice.kind === 'Error' ? 'error' : notice.kind === 'Warning' ? 'warning' : 'info'}
      onCancel={cancel}
      actions={
        notice.buttons === 'Ok' ? (
          <AccessButton msg="common-Ok" variant="primary" onPress={() => answer('Ok')} autoFocus />
        ) : notice.buttons === 'OkCancel' ? (
          <>
            <AccessButton msg="common-Cancel" variant="quiet" onPress={() => answer('Cancel')} />
            <AccessButton msg="common-Ok" variant="primary" onPress={() => answer('Ok')} autoFocus />
          </>
        ) : (
          <>
            <AccessButton msg="common-Cancel" variant="quiet" onPress={() => answer('Cancel')} />
            <AccessButton msg="error-Retry" variant="primary" onPress={() => answer('Retry')} autoFocus />
          </>
        )
      }
    >
      <p className="pre-line">{text}</p>
    </ConfirmDialog>
  );
}

function Waiting({ progress }: { progress: Progress }) {
  const { t, dispatch } = useApp();
  const text = progressText(t, progress);
  const doc = typeof progress === 'object' ? progress.Document : null;
  return (
    <section className="card waiting" aria-labelledby="waiting-title">
      <h2 id="waiting-title" className="card-title">
        {t.t('waiting-message')}
      </h2>
      <ProgressBar
        className="progress"
        aria-label={text}
        isIndeterminate={!doc}
        value={doc ? doc.index : undefined}
        minValue={0}
        maxValue={doc ? doc.total : 100}
      >
        <div className="progress-track">
          <div className="progress-fill" style={doc ? { width: `${(100 * doc.index) / Math.max(1, doc.total)}%` } : undefined} />
        </div>
      </ProgressBar>
      <p className="status-line" role="status">
        {text}
      </p>
      <div className="actions">
        <AccessButton msg="common-Cancel" variant="quiet" onPress={() => dispatch({ type: 'cancel' })} />
      </div>
    </section>
  );
}

function FormFor({ request, form, lastUsername, onUsername }: { request: number; form: NativeForm; lastUsername: string; onUsername: (u: string) => void }) {
  if ('Login' in form) return <LoginForm key={request} request={request} data={form.Login} lastUsername={lastUsername} onUsername={onUsername} />;
  if ('SmsTan' in form) return <SmsTanForm key={request} request={request} data={form.SmsTan} />;
  if ('QrCode' in form) return <QrForm key={request} request={request} data={form.QrCode} />;
  if ('WaitForApp' in form) return <WaitForAppForm key={request} request={request} data={form.WaitForApp} />;
  return <Fido2Form key={request} request={request} data={form.Fido2} />;
}

export function SigningScreen({
  interaction,
  queue,
  documentLeavesDevice = false,
}: {
  interaction: InteractionView;
  queue: QueueItemView[];
  documentLeavesDevice?: boolean;
}) {
  const { t } = useApp();
  // Benutzername der letzten Anmeldung (für eine erneute Login-Seite, nicht persistent)
  const [lastUsername, setLastUsername] = useState('');
  const current = queue.find((q) => q.current);
  return (
    <div className="screen screen-signing" data-interaction={interaction.kind === 'form' ? variantName(interaction.form) : interaction.kind}>
      <ScreenHeading subtitle={current?.name}>{t.t('main-signature')}</ScreenHeading>
      <div className="signing-layout">
        <div className="signing-main">
          {interaction.kind === 'waiting' ? <Waiting progress={interaction.progress} /> : null}
          {interaction.kind === 'form' ? (
            <FormFor request={interaction.request} form={interaction.form} lastUsername={lastUsername} onUsername={setLastUsername} />
          ) : null}
          {interaction.kind === 'secret' ? <SecretForm key={interaction.request} request={interaction.request} prompt={interaction.prompt} /> : null}
          {interaction.kind === 'notice' ? (
            <>
              <Waiting progress="WaitingForServer" />
              <NoticeDialog key={interaction.request} request={interaction.request} notice={interaction.notice} />
            </>
          ) : null}
          {documentLeavesDevice ? <PrivacyNote /> : null}
        </div>
        <QueuePanel queue={queue.length > 1 ? queue : []} />
      </div>
    </div>
  );
}
