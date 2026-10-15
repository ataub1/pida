/**
 * Signieren als Statuskarte über dem Arbeitsbereich (kein Seitenwechsel). ID Austria:
 * Datenschutzhinweis (einmalig) → Anmelden → App-Bestätigung mit Vergleichswert und Countdown
 * (alternativ FIDO2 oder SMS-TAN) → Signiert. Keystore und Chipkarte mit eigenen, kurzen Wegen.
 * Abbrechen ist jederzeit möglich (Esc, ×, „Abbrechen“). Alles simuliert, kein Netz.
 */
import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { useAppState, useDispatch, type SigningSession } from '../../app/store';
import { useTiming } from '../../app/timing';
import { useT, type MsgKey } from '../../i18n';
import { isoWithOffset, maskUser, outputName } from '../../model/format';
import type { SignerKind } from '../../model/types';
import { Badge, Button, CheckboxField, Details, Spinner, TextInput } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import {
  IconAlertCircle,
  IconCheck,
  IconClock,
  IconExternal,
  IconLock,
  IconMessage,
  IconPhone,
  IconShield,
  IconUsbKey,
  IconWand,
} from '../../ui/Icons';
import { CountdownRing, useCountdown } from './CountdownRing';

/** Vergleichswert der Demo (wie im Mock-Kern von apps/app). */
export const REFERENCE_VALUE = 'Xk7Pq';

type ErrorKind = 'unreachable' | 'timeout' | 'declined';

type Step =
  | { s: 'privacy' }
  | { s: 'login' }
  | { s: 'connecting' }
  | { s: 'app'; attempt: number }
  | { s: 'fido2'; phase: 'pin' | 'touch' }
  | { s: 'sms' }
  | { s: 'keystore' }
  | { s: 'card' }
  | { s: 'signing' }
  | { s: 'done' }
  | { s: 'error'; kind: ErrorKind; at: string; from: 1 | 0 };

type Branch = 'app' | 'fido2' | 'sms';

export function SigningCard() {
  const { signing } = useAppState();
  if (!signing) return null;
  return <SigningDialog session={signing} />;
}

function SigningDialog({ session }: { session: SigningSession }) {
  const state = useAppState();
  const dispatch = useDispatch();
  const timing = useTiming();
  const t = useT();
  const signer: SignerKind = state.signer;
  const docs = state.docs.filter((d) => session.docIds.includes(d.spec.id));
  const first = docs[0];
  const initial: Step = signer === 'keystore' ? { s: 'keystore' } : signer === 'card' ? { s: 'card' } : state.privacyAcked ? { s: 'login' } : { s: 'privacy' };
  const [step, setStep] = useState<Step>(initial);
  const [branch, setBranch] = useState<Branch>('app');
  const bodyRef = useRef<HTMLDivElement>(null);
  const response = state.demo.response;

  // Fokus beim Schrittwechsel auf das erste Eingabefeld bzw. die Überschrift des Schritts
  useEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    const target = el.querySelector<HTMLElement>('[data-autofocus] input, input[data-autofocus]') ?? el.querySelector<HTMLElement>('.step-title');
    target?.focus({ preventScroll: true });
  }, [step.s, (step as { phase?: string }).phase]);

  const goConfirm = () => {
    if (response === 'fido2' || state.settings.fido2Default) {
      setBranch('fido2');
      setStep({ s: 'fido2', phase: 'pin' });
    } else if (response === 'sms') {
      setBranch('sms');
      setStep({ s: 'sms' });
    } else {
      setBranch('app');
      setStep({ s: 'app', attempt: 1 });
    }
  };

  // Simulierte Übergänge
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    if (step.s === 'connecting') {
      timer = setTimeout(() => {
        if (state.demo.atrustDown) setStep({ s: 'error', kind: 'unreachable', at: isoWithOffset(new Date()), from: 0 });
        else goConfirm();
      }, timing.connectMs);
    } else if (step.s === 'app') {
      if (response === 'cancel') timer = setTimeout(() => setStep({ s: 'error', kind: 'declined', at: isoWithOffset(new Date()), from: 1 }), timing.declineMs);
      else if (response === 'app' && timing.autoConfirmMs !== null) timer = setTimeout(() => setStep({ s: 'signing' }), timing.autoConfirmMs);
    } else if (step.s === 'fido2' && step.phase === 'touch' && timing.fido2AutoMs !== null) {
      timer = setTimeout(() => setStep({ s: 'signing' }), timing.fido2AutoMs);
    } else if (step.s === 'signing') {
      timer = setTimeout(() => setStep({ s: 'done' }), timing.signMs);
    } else if (step.s === 'done') {
      timer = setTimeout(() => {
        // Einzelnes Dokument aus dem Stapel: Toast statt „Fertig“-Bildschirm
        if (!session.batch && state.batchMode && first) {
          dispatch({ type: 'toast', toast: { kind: 'success', title: t('batch.toast.one', { file: outputName(first.spec.fileName, state.settings.namePattern) }), text: t('done.seal') } });
        }
        dispatch({ type: 'signed' });
      }, timing.doneHoldMs);
    }
    return () => clearTimeout(timer);
  }, [step]);

  const busy = step.s === 'signing' || step.s === 'done' || step.s === 'connecting';
  const cancel = () => {
    if (step.s === 'signing' || step.s === 'done') return;
    dispatch({ type: 'cancelSigning' });
    dispatch({ type: 'toast', toast: { kind: 'info', title: t('sc.cancelled') } });
  };

  const title = t(signer === 'idaustria' ? 'sc.title.idaustria' : signer === 'keystore' ? 'sc.title.keystore' : 'sc.title.card');
  const subtitle = session.batch ? t.n('sc.batch', docs.length) : (first?.spec.fileName ?? '');

  const confirmLabel: MsgKey = branch === 'fido2' ? 'sc.step.fido2' : branch === 'sms' ? 'sc.step.sms' : 'sc.step.app';
  const steps: MsgKey[] = signer === 'idaustria' ? ['sc.step.login', confirmLabel, 'sc.step.done'] : signer === 'keystore' ? ['sc.step.unlock', 'sc.step.done'] : ['sc.step.card', 'sc.step.done'];
  const lastIdx = steps.length - 1;
  const index =
    step.s === 'signing' || step.s === 'done'
      ? lastIdx
      : step.s === 'app' || step.s === 'fido2' || step.s === 'sms'
        ? 1
        : step.s === 'error'
          ? step.from
          : 0;
  const doneAll = step.s === 'done';

  return (
    <ModalShell isOpen onClose={cancel} isDismissable={false} className="signing-modal" size="md">
      <div className={`signing-card${busy ? ' is-busy' : ''}`}>
        <DialogHeader title={title} subtitle={subtitle} onClose={busy && step.s !== 'connecting' ? undefined : cancel} icon={<SignerGlyph kind={signer} />} />
        <ol className="stepper" aria-label={t('sc.steps')}>
          {steps.map((k, i) => {
            const isDone = i < index || doneAll;
            const isCurrent = i === index && !doneAll;
            // Fehler nicht nur über die Farbe zeigen: Ausrufezeichen statt Nummer und eigener Screenreader-Text
            const isError = step.s === 'error' && isCurrent;
            return (
              <li key={k} className={`step${isDone ? ' is-done' : ''}${isCurrent ? ' is-current' : ''}${isError ? ' is-error' : ''}`} aria-current={isCurrent ? 'step' : undefined}>
                <span className="step-dot" aria-hidden="true">
                  {isDone ? <IconCheck size={14} /> : isError ? '!' : i + 1}
                </span>
                <span className="step-label">{t(k)}</span>
                <span className="sr-only">, {t(isDone ? 'sc.step.state.done' : isError ? 'sc.step.state.error' : isCurrent ? 'sc.step.state.current' : 'sc.step.state.todo')}</span>
              </li>
            );
          })}
        </ol>
        <div className="signing-body" ref={bodyRef}>
          {step.s === 'privacy' ? (
            <PrivacyStep
              onAccept={() => {
                dispatch({ type: 'ackPrivacy' });
                setStep({ s: 'login' });
              }}
              onDecline={cancel}
            />
          ) : null}
          {step.s === 'login' ? <LoginStep onSubmit={() => setStep({ s: 'connecting' })} onCancel={cancel} /> : null}
          {step.s === 'connecting' ? <BusyStep text={t('sc.connecting')} /> : null}
          {step.s === 'app' ? (
            <AppStep
              key={step.attempt}
              totalSecs={response === 'timeout' ? timing.timeoutScenarioSecs : timing.appTimeoutSecs}
              onConfirmed={() => setStep({ s: 'signing' })}
              onExpired={() => setStep({ s: 'error', kind: 'timeout', at: isoWithOffset(new Date()), from: 1 })}
              onFido2={() => {
                setBranch('fido2');
                setStep({ s: 'fido2', phase: 'pin' });
              }}
              onSms={() => {
                setBranch('sms');
                setStep({ s: 'sms' });
              }}
              onCancel={cancel}
            />
          ) : null}
          {step.s === 'fido2' ? (
            <Fido2Step
              phase={step.phase}
              onPin={() => setStep({ s: 'fido2', phase: 'touch' })}
              onTouched={() => setStep({ s: 'signing' })}
              onUseApp={() => {
                setBranch('app');
                setStep({ s: 'app', attempt: Date.now() });
              }}
              onSms={() => {
                setBranch('sms');
                setStep({ s: 'sms' });
              }}
              onCancel={cancel}
            />
          ) : null}
          {step.s === 'sms' ? <SmsStep onSubmit={() => setStep({ s: 'signing' })} onCancel={cancel} /> : null}
          {step.s === 'keystore' ? <KeystoreStep onSubmit={() => setStep({ s: 'signing' })} onCancel={cancel} /> : null}
          {step.s === 'card' ? <CardStep onDone={() => setStep({ s: 'signing' })} onCancel={cancel} /> : null}
          {step.s === 'signing' ? <BusyStep text={session.batch ? t.n('sc.signing.batch', docs.length) : t('sc.signing')} /> : null}
          {step.s === 'done' ? <DoneStep /> : null}
          {step.s === 'error' ? (
            <ErrorStep
              kind={step.kind}
              at={step.at}
              onRetry={() => {
                if (step.kind === 'unreachable') {
                  dispatch({ type: 'atrustRecovered' });
                  setStep({ s: 'connecting' });
                } else {
                  setBranch('app');
                  setStep({ s: 'app', attempt: Date.now() });
                }
              }}
              onClose={cancel}
            />
          ) : null}
        </div>
      </div>
    </ModalShell>
  );
}

function SignerGlyph({ kind }: { kind: SignerKind }) {
  return <span className={`signer-glyph signer-${kind}`}>{kind === 'idaustria' ? <IconPhone size={20} /> : kind === 'keystore' ? <IconLock size={20} /> : <IconShield size={20} />}</span>;
}

function StepTitle({ children }: { children: ReactNode }) {
  return (
    <h3 className="step-title" tabIndex={-1}>
      {children}
    </h3>
  );
}

function DemoButton({ onPress, children }: { onPress: () => void; children: ReactNode }) {
  return (
    <AriaButton className="btn btn-demo btn-md" onPress={onPress}>
      <IconWand size={16} />
      {children}
    </AriaButton>
  );
}

function ReferenceValue({ size = 'lg', onShowData }: { size?: 'lg' | 'sm'; onShowData?: () => void }) {
  const t = useT();
  return (
    <div className={`refvalue refvalue-${size}`}>
      <span className="refvalue-label">{t('sc.app.ref')}</span>
      <span className="refvalue-chars" aria-hidden="true">
        {REFERENCE_VALUE.split('').map((c, i) => (
          <span key={i} className="refvalue-char">
            {c}
          </span>
        ))}
      </span>
      <span className="sr-only">{REFERENCE_VALUE.split('').join(' ')}</span>
      {onShowData ? (
        <AriaButton className="link refvalue-data" onPress={onShowData}>
          {t('sc.app.data')}
        </AriaButton>
      ) : null}
    </div>
  );
}

/* ------------------------------------------------------------------ Schritte */

function PrivacyStep({ onAccept, onDecline }: { onAccept: () => void; onDecline: () => void }) {
  const t = useT();
  const dispatch = useDispatch();
  return (
    <div className="step-pane">
      <span className="step-hero step-hero-info" aria-hidden="true">
        <IconShield size={28} />
      </span>
      <StepTitle>{t('sc.privacy.title')}</StepTitle>
      <div className="step-prose">
        <p>{t('sc.privacy.p1')}</p>
        <p>{t('sc.privacy.p2')}</p>
        <p>
          {t('sc.privacy.p3')}{' '}
          <AriaButton className="link" onPress={() => dispatch({ type: 'toast', toast: { kind: 'info', title: t('sc.privacy.link'), text: t('sc.privacy.demo') } })}>
            {t('sc.privacy.link')}
            <IconExternal size={14} />
            <span className="sr-only"> {t('common.externalHint')}</span>
          </AriaButton>
        </p>
      </div>
      <p className="step-footnote">{t('sc.privacy.once')}</p>
      <div className="step-actions">
        <Button variant="ghost" onPress={onDecline}>
          {t('sc.privacy.decline')}
        </Button>
        <Button variant="primary" onPress={onAccept}>
          {t('sc.privacy.accept')}
        </Button>
      </div>
    </div>
  );
}

function LoginStep({ onSubmit, onCancel }: { onSubmit: () => void; onCancel: () => void }) {
  const t = useT();
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const [user, setUser] = useState(settings.rememberUser ? settings.idaUser : '');
  const [pw, setPw] = useState('');
  const [remember, setRemember] = useState(settings.rememberUser);
  const [errUser, setErrUser] = useState<string | null>(null);
  const [errPw, setErrPw] = useState<string | null>(null);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const u = user.trim();
    const eu = u ? null : t('sc.login.errUser');
    const ep = pw.length >= 5 ? null : t('sc.login.errShort');
    setErrUser(eu);
    setErrPw(ep);
    if (eu || ep) return;
    dispatch({ type: 'settings', patch: { rememberUser: remember, idaUser: remember ? u : settings.idaUser } });
    onSubmit();
  };
  return (
    <form className="step-pane" onSubmit={submit} noValidate>
      <StepTitle>{t('sc.step.login')}</StepTitle>
      <div data-autofocus={user ? undefined : true}>
        <TextInput label={t('sc.login.user')} value={user} onChange={(v) => { setUser(v); setErrUser(null); }} error={errUser} autoComplete="username" inputMode="tel" />
      </div>
      <div data-autofocus={user ? true : undefined}>
        <TextInput label={t('sc.login.password')} type="password" value={pw} onChange={(v) => { setPw(v); setErrPw(null); }} error={errPw} autoComplete="current-password" description={t('sc.login.demo')} />
      </div>
      <CheckboxField label={t('sc.login.remember')} isSelected={remember} onChange={setRemember} />
      <p className="step-footnote">{t('sc.login.provider')}</p>
      <div className="step-actions">
        <Button variant="ghost" onPress={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button variant="primary" type="submit">
          {t('sc.login.submit')}
        </Button>
      </div>
    </form>
  );
}

function BusyStep({ text }: { text: string }) {
  return (
    <div className="step-pane step-pane-center" role="status">
      <Spinner size={36} className="step-spinner" />
      <p className="step-title" tabIndex={-1}>
        {text}
      </p>
    </div>
  );
}

function AppStep({
  totalSecs,
  onConfirmed,
  onExpired,
  onFido2,
  onSms,
  onCancel,
}: {
  totalSecs: number;
  onConfirmed: () => void;
  onExpired: () => void;
  onFido2: () => void;
  onSms: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  const timing = useTiming();
  const dispatch = useDispatch();
  const remaining = useCountdown(totalSecs, timing.tickMs, onExpired);
  // Screenreader: einmalige Vorwarnung kurz vor Ablauf (der Ring selbst wird nicht laufend angesagt)
  const lowTime = totalSecs > 60 && remaining > 0 && remaining <= 60;
  return (
    <div className="step-pane">
      <div className="app-status">
        <span className="app-phone" aria-hidden="true">
          <IconPhone size={26} />
          <span className="app-ping" />
        </span>
        <div>
          <StepTitle>{t('sc.app.sent')}</StepTitle>
          <p className="step-text">{t('sc.app.instruction')}</p>
        </div>
      </div>
      <div className="app-confirm">
        <ReferenceValue onShowData={() => dispatch({ type: 'toast', toast: { kind: 'info', title: t('sc.app.data'), text: t('sc.app.data.demo') } })} />
        <CountdownRing remaining={remaining} total={totalSecs} />
      </div>
      <span className="sr-only" role="status">
        {lowTime ? t('sc.app.lowTime') : ''}
      </span>
      <p className="step-hint">
        <IconClock size={15} />
        {t('sc.app.refHint')} <span className="waiting-dots">{t('sc.app.waiting')}</span>
      </p>
      <div className="alt-row">
        <span>{t('sc.app.alternatives')}</span>
        <AriaButton className="link" onPress={onFido2}>
          <IconUsbKey size={15} />
          {t('sc.app.fido2')}
        </AriaButton>
        <span aria-hidden="true">·</span>
        <AriaButton className="link" onPress={onSms}>
          <IconMessage size={15} />
          {t('sc.app.sms')}
        </AriaButton>
      </div>
      <div className="step-actions">
        <Button variant="ghost" onPress={onCancel}>
          {t('common.cancel')}
        </Button>
        <DemoButton onPress={onConfirmed}>{t('sc.app.demo')}</DemoButton>
      </div>
    </div>
  );
}

function Fido2Step({
  phase,
  onPin,
  onTouched,
  onUseApp,
  onSms,
  onCancel,
}: {
  phase: 'pin' | 'touch';
  onPin: () => void;
  onTouched: () => void;
  onUseApp: () => void;
  onSms: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  const [pin, setPin] = useState('');
  const [err, setErr] = useState<string | null>(null);
  if (phase === 'touch') {
    return (
      <div className="step-pane step-pane-center">
        <span className="fido-key" aria-hidden="true">
          <IconUsbKey size={40} />
          <span className="fido-pulse" />
        </span>
        <StepTitle>{t('sc.fido2.touch')}</StepTitle>
        <p className="step-text">{t('sc.fido2.touchHint')}</p>
        <div className="step-actions">
          <Button variant="ghost" onPress={onCancel}>
            {t('common.cancel')}
          </Button>
          <DemoButton onPress={onTouched}>{t('sc.fido2.demo')}</DemoButton>
        </div>
      </div>
    );
  }
  return (
    <form
      className="step-pane"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        if (pin.trim().length < 4) setErr(t('sc.fido2.errPin'));
        else onPin();
      }}
    >
      <div className="app-status">
        <span className="app-phone" aria-hidden="true">
          <IconUsbKey size={26} />
        </span>
        <div>
          <StepTitle>{t('sc.fido2.title')}</StepTitle>
          <p className="step-text">{t('sc.fido2.text')}</p>
        </div>
      </div>
      <div data-autofocus>
        <TextInput label={t('sc.fido2.pin')} type="password" inputMode="numeric" value={pin} onChange={(v) => { setPin(v); setErr(null); }} error={err} autoComplete="off" />
      </div>
      <div className="alt-row">
        <AriaButton className="link" onPress={onUseApp}>
          <IconPhone size={15} />
          {t('sc.fido2.useApp')}
        </AriaButton>
        <span aria-hidden="true">·</span>
        <AriaButton className="link" onPress={onSms}>
          <IconMessage size={15} />
          {t('sc.app.sms')}
        </AriaButton>
      </div>
      <div className="step-actions">
        <Button variant="ghost" onPress={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button variant="primary" type="submit">
          {t('sc.fido2.continue')}
        </Button>
      </div>
    </form>
  );
}

function SmsStep({ onSubmit, onCancel }: { onSubmit: () => void; onCancel: () => void }) {
  const t = useT();
  const { settings } = useAppState();
  const [tan, setTan] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [resent, setResent] = useState(false);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const v = tan.trim();
    if (!v) return setErr(t('sc.sms.errEmpty'));
    if (v.length >= 3 && REFERENCE_VALUE.toLowerCase().startsWith(v.toLowerCase())) return setErr(t('sc.sms.errRef'));
    if (v.length > 6) return setErr(t('sc.sms.errLong'));
    onSubmit();
  };
  return (
    <form className="step-pane" onSubmit={submit} noValidate>
      <div className="app-status">
        <span className="app-phone" aria-hidden="true">
          <IconMessage size={26} />
        </span>
        <div>
          <StepTitle>{t('sc.step.sms')}</StepTitle>
          <p className="step-text">{t('sc.sms.text', { user: maskUser(settings.idaUser) })}</p>
        </div>
      </div>
      <ReferenceValue size="sm" />
      <div data-autofocus>
        <TextInput
          label={t('sc.sms.tan')}
          value={tan}
          onChange={(v) => {
            setTan(v);
            setErr(null);
          }}
          error={err}
          inputMode="numeric"
          autoComplete="one-time-code"
          description={t('sc.sms.demo')}
          inputClassName="input-tan"
        />
      </div>
      <div className="alt-row">
        <AriaButton className="link" onPress={() => setResent(true)}>
          {t('sc.sms.resend')}
        </AriaButton>
        {resent ? (
          <span className="alt-note" role="status">
            {t('sc.sms.resent')}
          </span>
        ) : null}
      </div>
      <div className="step-actions">
        <Button variant="ghost" onPress={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button variant="primary" type="submit">
          {t('sc.sms.submit')}
        </Button>
      </div>
    </form>
  );
}

function KeystoreStep({ onSubmit, onCancel }: { onSubmit: () => void; onCancel: () => void }) {
  const t = useT();
  const { settings } = useAppState();
  const [pw, setPw] = useState('');
  const [store, setStore] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  return (
    <form
      className="step-pane"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        if (!pw) setErr(t('sc.ks.err'));
        else onSubmit();
      }}
    >
      <StepTitle>{t('sc.step.unlock')}</StepTitle>
      <p className="step-text">{t('sc.ks.text', { file: settings.keystoreFile })}</p>
      <div data-autofocus>
        <TextInput label={t('sc.ks.password')} type="password" value={pw} onChange={(v) => { setPw(v); setErr(null); }} error={err} autoComplete="off" />
      </div>
      <CheckboxField label={t('sc.ks.store')} isSelected={store} onChange={setStore} />
      <div className="step-actions">
        <Button variant="ghost" onPress={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button variant="primary" type="submit">
          {t('sc.ks.submit')}
        </Button>
      </div>
    </form>
  );
}

function CardStep({ onDone, onCancel }: { onDone: () => void; onCancel: () => void }) {
  const t = useT();
  return (
    <div className="step-pane">
      <StepTitle>{t('sc.step.card')}</StepTitle>
      <p className="step-text">{t('sc.card.text')}</p>
      <p className="step-hint" role="status">
        <Spinner size={16} />
        {t('sc.card.waiting')}
      </p>
      <div className="step-actions">
        <Button variant="ghost" onPress={onCancel}>
          {t('common.cancel')}
        </Button>
        <DemoButton onPress={onDone}>{t('sc.card.demo')}</DemoButton>
      </div>
    </div>
  );
}

function DoneStep() {
  const t = useT();
  return (
    <div className="step-pane step-pane-center" role="status">
      <span className="done-check" aria-hidden="true">
        <IconCheck size={34} />
      </span>
      <p className="step-title" tabIndex={-1}>
        {t('sc.done')}
      </p>
    </div>
  );
}

function ErrorStep({ kind, at, onRetry, onClose }: { kind: ErrorKind; at: string; onRetry: () => void; onClose: () => void }) {
  const t = useT();
  const key = kind === 'unreachable' ? 'unreachable' : kind === 'timeout' ? 'timeout' : 'declined';
  return (
    <div className="step-pane">
      <span className="step-hero step-hero-danger" aria-hidden="true">
        <IconAlertCircle size={28} />
      </span>
      <div role="alert">
        <StepTitle>{t(`sc.err.${key}.title` as MsgKey)}</StepTitle>
        <p className="step-text">{t(`sc.err.${key}.text` as MsgKey)}</p>
      </div>
      <Details label={t('common.details')} level={4}>
        <p className="mono-small">{t(`sc.err.${key}.details` as MsgKey, { time: at })}</p>
        <Badge tone="neutral">{kind === 'unreachable' ? 'backend-network' : kind === 'timeout' ? 'backend-timeout' : 'backend-cancelled'}</Badge>
      </Details>
      <div className="step-actions">
        <Button variant="ghost" onPress={onClose}>
          {t('common.close')}
        </Button>
        <Button variant="primary" onPress={onRetry}>
          {kind === 'timeout' ? t('sc.err.timeout.retry') : t('common.retry')}
        </Button>
      </div>
    </div>
  );
}
