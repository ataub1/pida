/**
 * Modale Hüllen (aus dem Prototyp, ADR 0004 §4.2): zentrierte Karte, Seitenleiste oder Palette; unter
 * 760 px wird die Karte zum Sheet von unten (CSS). Fokusfalle und Esc über React Aria. Jede Ebene trägt
 * `data-modal-layer`, damit Zugriffstasten nur in der obersten Ebene wirken (`ui/AccessKeys.ts`).
 *
 * `ConfirmDialog` ist die Rückfrage der bisherigen App im neuen Stil (gleiche Props: Titel, Inhalt,
 * Knöpfe, Escape = `onCancel`, Ton); sie ist ein `alertdialog`.
 */
import type { ReactNode } from 'react';
import { Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { useApp } from '../app/context';
import { IconButton } from './controls';
import { IconAlertCircle, IconAlert, IconClose, IconInfo } from './Icons';

export function ModalShell({
  isOpen = true,
  onClose,
  children,
  variant = 'center',
  size = 'md',
  isDismissable = true,
  isKeyboardDismissDisabled = false,
  ariaLabel,
  className,
  role = 'dialog',
}: {
  isOpen?: boolean;
  onClose: () => void;
  children: ReactNode;
  variant?: 'center' | 'drawer' | 'palette';
  size?: 'sm' | 'md' | 'lg';
  isDismissable?: boolean;
  isKeyboardDismissDisabled?: boolean;
  ariaLabel?: string;
  className?: string;
  role?: 'dialog' | 'alertdialog';
}) {
  return (
    <ModalOverlay
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      isDismissable={isDismissable}
      isKeyboardDismissDisabled={isKeyboardDismissDisabled}
      className={`overlay overlay-${variant}`}
    >
      <Modal className={`modal modal-${variant} modal-${size}${className ? ` ${className}` : ''}`}>
        <Dialog className="dialog" aria-label={ariaLabel} role={role} data-modal-layer="">
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/** Kopfzeile eines Dialogs mit Titel (labelt den Dialog) und optionalem Schließen-Knopf. */
export function DialogHeader({ title, subtitle, onClose, icon }: { title: string; subtitle?: ReactNode; onClose?: () => void; icon?: ReactNode }) {
  const { t } = useApp();
  return (
    <header className="dialog-header">
      {icon ? <span className="dialog-icon">{icon}</span> : null}
      <div className="dialog-titles">
        <Heading slot="title" level={2} className="dialog-title">
          {title}
        </Heading>
        {subtitle ? <p className="dialog-subtitle">{subtitle}</p> : null}
      </div>
      {onClose ? (
        <IconButton label={t.t('ng-ui-close')} onPress={onClose} className="dialog-close" tooltip={false}>
          <IconClose size={18} />
        </IconButton>
      ) : null}
    </header>
  );
}

/** Rückfrage als modales `alertdialog` (Tastatur: Fokus im Dialog, Escape = `onCancel`). */
export function ConfirmDialog({
  title,
  children,
  actions,
  onCancel,
  tone = 'info',
  icon,
}: {
  title: string;
  children: ReactNode;
  actions: ReactNode;
  onCancel?: () => void;
  tone?: 'info' | 'warning' | 'error';
  icon?: ReactNode;
}) {
  const glyph = icon ?? (tone === 'error' ? <IconAlertCircle size={20} /> : tone === 'warning' ? <IconAlert size={20} /> : <IconInfo size={20} />);
  return (
    <ModalShell
      onClose={() => onCancel?.()}
      isDismissable={false}
      isKeyboardDismissDisabled={!onCancel}
      role="alertdialog"
      size="md"
      className={`modal-tone-${tone}`}
    >
      <DialogHeader title={title} icon={glyph} />
      <div className="confirm-body">{children}</div>
      <div className="confirm-actions dialog-actions">{actions}</div>
    </ModalShell>
  );
}
