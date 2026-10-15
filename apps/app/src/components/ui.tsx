/**
 * Gemeinsame Bausteine: Zugriffstasten (`&`-Mnemonics aus den Katalogen, Plan §7.6), Link-Markup,
 * Countdown, Vergleichswert, modale Rückfragen.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react';
import {
  Button,
  Dialog,
  Heading,
  Modal,
  ModalOverlay,
  type ButtonProps,
} from 'react-aria-components';
import { useApp, useT } from '../app/context';
import { isAllowedExternalUrl } from '../core/bridge';
import type { Args } from '../i18n/catalog';
import { parseMarkup } from '../i18n/markup';

/** Tastenkürzel einer Zugriffstaste für `aria-keyshortcuts`. */
export function keyShortcut(key: string, isMac: boolean): string {
  return `${isMac ? 'Control+Alt' : 'Alt'}+${key.toUpperCase()}`;
}

/** Beschriftung mit unterstrichener Zugriffstaste (erstes passendes Zeichen). */
export function Mnemonic({ text, accessKey }: { text: string; accessKey: string | null }) {
  if (!accessKey) return <>{text}</>;
  const i = text.toLowerCase().indexOf(accessKey.toLowerCase());
  if (i < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <span className="mnemonic">{text.slice(i, i + 1)}</span>
      {text.slice(i + 1)}
    </>
  );
}

/** Varianten der Knöpfe. */
export type ButtonVariant = 'primary' | 'secondary' | 'quiet' | 'danger';

/** Knopf mit Text aus dem Katalog und dessen Zugriffstaste (`.accesskey`). */
export function AccessButton({
  msg,
  args,
  label,
  icon,
  variant = 'secondary',
  className,
  ...rest
}: Omit<ButtonProps, 'children' | 'className'> & {
  msg: string;
  args?: Args;
  label?: string;
  icon?: ReactNode;
  variant?: ButtonVariant;
  className?: string;
}) {
  const { t, isMac } = useApp();
  const text = label ?? t.t(msg, args);
  const key = t.accessKey(msg);
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (key) el.setAttribute('aria-keyshortcuts', keyShortcut(key, isMac));
    else el.removeAttribute('aria-keyshortcuts');
  }, [key, isMac]);
  return (
    <Button ref={ref} className={`btn btn-${variant}${className ? ` ${className}` : ''}`} data-accesskey={key ?? undefined} {...rest}>
      {icon}
      <span className="btn-label">
        <Mnemonic text={text} accessKey={key} />
      </span>
    </Button>
  );
}

/** Ziel eines Links im Markup (Index = Reihenfolge der `<a>`). */
export type LinkTarget = { href: string } | { onPress: () => void };

/**
 * Text mit Link-Markup. Ziel: explizites `href`, sonst `targets[i]`, sonst der Linktext selbst,
 * wenn er eine `https:`-URL ist. Externe Ziele öffnet die Shell im Systembrowser.
 */
export function Markup({ text, targets = [] }: { text: string; targets?: LinkTarget[] }) {
  const { bridge, toast, t } = useApp();
  let linkIndex = -1;
  return (
    <>
      {parseMarkup(text).map((seg, i) => {
        if (seg.kind === 'text') return <span key={i}>{seg.text}</span>;
        linkIndex += 1;
        const target: LinkTarget | null =
          (seg.href ? { href: seg.href } : null) ??
          targets[linkIndex] ??
          (isAllowedExternalUrl(seg.text) ? { href: seg.text } : null);
        if (!target) return <span key={i}>{seg.text}</span>;
        const onPress =
          'onPress' in target
            ? target.onPress
            : () => {
                bridge.openUrl(target.href).catch(() => toast(t.t('ng-ui-open-failed'), 'error'));
              };
        return (
          <Button key={i} className="link" onPress={onPress}>
            {seg.text}
            {'href' in target ? <span className="visually-hidden"> {t.t('ng-ui-link-external')}</span> : null}
          </Button>
        );
      })}
    </>
  );
}

/** Vergleichswert (ID Austria), gut lesbar und für Screenreader buchstabiert. */
export function ReferenceValue({ value }: { value: string }) {
  const t = useT();
  return (
    <div className="reference">
      <span className="reference-label">{t.t('tanEnter-ReferenceValue')}</span>{' '}
      <strong className="reference-value" aria-label={value.split('').join(' ')}>
        {value}
      </strong>
    </div>
  );
}

/** Formatiert Sekunden als m:ss. */
export function formatDuration(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** Countdown für Long-Polls (App, QR); `resetKey` startet neu (neue Anfrage). */
export function Countdown({ seconds, resetKey }: { seconds: number | null; resetKey: unknown }) {
  const t = useT();
  const [left, setLeft] = useState(seconds ?? 0);
  useEffect(() => {
    setLeft(seconds ?? 0);
    if (seconds == null) return undefined;
    const started = Date.now();
    const timer = setInterval(() => {
      const rest = seconds - Math.floor((Date.now() - started) / 1000);
      setLeft(rest);
      if (rest <= 0) clearInterval(timer);
    }, 1000);
    return () => clearInterval(timer);
  }, [seconds, resetKey]);
  if (seconds == null) return null;
  const expired = left <= 0;
  return (
    <p className={`countdown${expired ? ' countdown-expired' : ''}`}>
      {expired ? t.t('ng-ui-countdown-expired') : t.t('ng-ui-countdown', { time: formatDuration(left) })}
    </p>
  );
}

/** Rückfrage als modales `alertdialog` (Tastatur: Fokus im Dialog, Escape = `onCancel`). */
export function ConfirmDialog({
  title,
  children,
  actions,
  onCancel,
  tone = 'info',
}: {
  title: string;
  children: ReactNode;
  actions: ReactNode;
  onCancel?: () => void;
  tone?: 'info' | 'warning' | 'error';
}) {
  return (
    <ModalOverlay
      className="modal-overlay"
      isOpen
      isDismissable={false}
      isKeyboardDismissDisabled={!onCancel}
      onOpenChange={(open) => {
        if (!open) onCancel?.();
      }}
    >
      <Modal className={`modal modal-${tone}`}>
        <Dialog role="alertdialog" className="dialog" data-modal-layer="">
          <Heading slot="title" className="dialog-title">
            {title}
          </Heading>
          <div className="dialog-body">{children}</div>
          <div className="dialog-actions">{actions}</div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/** Kopf eines Schritts (h1, fokussierbar für den Fokuswechsel beim Schrittwechsel). */
export function ScreenHeading({ children, subtitle }: { children: ReactNode; subtitle?: ReactNode }) {
  return (
    <header className="screen-header">
      <h1 id="screen-heading" tabIndex={-1} className="screen-title">
        {children}
      </h1>
      {subtitle ? <p className="screen-subtitle">{subtitle}</p> : null}
    </header>
  );
}

/** Drehendes Symbol (Bewegung entfällt bei `prefers-reduced-motion`). */
export function Spinner({ size = 28 }: { size?: number }) {
  return <span className="spinner" style={{ width: size, height: size }} aria-hidden="true" />;
}
