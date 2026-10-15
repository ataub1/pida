/** Modale Hüllen: zentrierte Karte, Seitenleiste (Einstellungen) oder Palette. Fokusfalle und Esc über React Aria. */
import type { ReactNode } from 'react';
import { Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { useT } from '../i18n';
import { IconButton } from './controls';
import { IconClose } from './Icons';

export function ModalShell({
  isOpen,
  onClose,
  children,
  variant = 'center',
  size = 'md',
  isDismissable = true,
  ariaLabel,
  className,
  role = 'dialog',
}: {
  isOpen: boolean;
  onClose: () => void;
  children: ReactNode;
  variant?: 'center' | 'drawer' | 'palette';
  size?: 'sm' | 'md' | 'lg';
  isDismissable?: boolean;
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
      className={`overlay overlay-${variant}`}
    >
      <Modal className={`modal modal-${variant} modal-${size}${className ? ` ${className}` : ''}`}>
        <Dialog className="dialog" aria-label={ariaLabel} role={role}>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/** Kopfzeile eines Dialogs mit Titel (labelt den Dialog) und optionalem Schließen-Knopf. */
export function DialogHeader({ title, subtitle, onClose, icon }: { title: string; subtitle?: ReactNode; onClose?: () => void; icon?: ReactNode }) {
  const t = useT();
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
        <IconButton label={t('common.close')} onPress={onClose} className="dialog-close" tooltip={false}>
          <IconClose size={18} />
        </IconButton>
      ) : null}
    </header>
  );
}
