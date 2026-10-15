/**
 * Ablagefläche für PDFs (React Aria DropZone: Maus, Tastatur und Screenreader). Es werden nur
 * Dateinamen übernommen; Inhalte werden im Prototyp nicht gelesen.
 */
import type { ReactNode } from 'react';
import { DropZone, type DropItem } from 'react-aria-components';
import { useDispatch } from '../../app/store';
import { useT } from '../../i18n';

export function isPdfName(name: string): boolean {
  return /\.pdf$/i.test(name);
}

export function PdfDropZone({
  label,
  onFiles,
  children,
  className,
}: {
  label: string;
  onFiles: (names: string[]) => void;
  children: (state: { isDropTarget: boolean }) => ReactNode;
  className?: string;
}) {
  const dispatch = useDispatch();
  const t = useT();
  return (
    <DropZone
      aria-label={label}
      className={`dropzone${className ? ` ${className}` : ''}`}
      getDropOperation={() => 'copy'}
      onDrop={(e) => {
        const files = e.items.filter((i: DropItem) => i.kind === 'file') as Extract<DropItem, { kind: 'file' }>[];
        const names = files.map((f) => f.name);
        const pdfs = names.filter(isPdfName);
        names.filter((n) => !isPdfName(n)).forEach((n) => dispatch({ type: 'toast', toast: { kind: 'error', title: t('file.notPdf', { name: n }) } }));
        if (pdfs.length) onFiles(pdfs);
      }}
    >
      {({ isDropTarget }) => children({ isDropTarget })}
    </DropZone>
  );
}
