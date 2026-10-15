/**
 * Eigene, einfache Symbole (SVG, `currentColor`, dekorativ: `aria-hidden`). Keine Marken- oder
 * Logo-Grafiken Dritter (z. B. kein ID-Austria-Logo).
 */
import type { ReactElement, SVGProps } from 'react';

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function base(path: ReactElement, { size = 20, ...rest }: IconProps): ReactElement {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      {path}
    </svg>
  );
}

export const IconSettings = (p: IconProps) =>
  base(
    <>
      <circle cx="12" cy="12" r="3.2" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
    </>,
    p,
  );

export const IconDocument = (p: IconProps) =>
  base(
    <>
      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
      <path d="M14 3v5h5" />
      <path d="M9 13h6M9 17h4" />
    </>,
    p,
  );

export const IconUpload = (p: IconProps) =>
  base(
    <>
      <path d="M12 15V4" />
      <path d="m7 9 5-5 5 5" />
      <path d="M5 15v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3" />
    </>,
    p,
  );

export const IconChevronLeft = (p: IconProps) => base(<path d="m15 18-6-6 6-6" />, p);
export const IconChevronRight = (p: IconProps) => base(<path d="m9 18 6-6-6-6" />, p);
export const IconCheck = (p: IconProps) => base(<path d="m5 12 5 5 9-10" />, p);
export const IconClose = (p: IconProps) => base(<path d="M6 6l12 12M18 6 6 18" />, p);

export const IconPhone = (p: IconProps) =>
  base(
    <>
      <rect x="7" y="2.5" width="10" height="19" rx="2.2" />
      <path d="M11 18.5h2" />
    </>,
    p,
  );

export const IconCard = (p: IconProps) =>
  base(
    <>
      <rect x="2.5" y="5" width="19" height="14" rx="2" />
      <rect x="5.5" y="9" width="4" height="3.5" rx=".6" />
      <path d="M13 10h5M13 13.5h3" />
    </>,
    p,
  );

export const IconKey = (p: IconProps) =>
  base(
    <>
      <circle cx="8" cy="15" r="4" />
      <path d="m11 12 8-8M16 7l2 2M14 9l2 2" />
    </>,
    p,
  );

export const IconShield = (p: IconProps) =>
  base(
    <>
      <path d="M12 3 5 6v5c0 4.5 3 8.3 7 10 4-1.7 7-5.5 7-10V6z" />
      <path d="m9 12 2 2 4-4" />
    </>,
    p,
  );

export const IconFolder = (p: IconProps) =>
  base(<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />, p);

export const IconExternal = (p: IconProps) =>
  base(
    <>
      <path d="M14 4h6v6" />
      <path d="M20 4 11 13" />
      <path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" />
    </>,
    p,
  );

export const IconAlert = (p: IconProps) =>
  base(
    <>
      <path d="M12 3 2 20h20z" />
      <path d="M12 10v4M12 17h.01" />
    </>,
    p,
  );

export const IconInfo = (p: IconProps) =>
  base(
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5M12 8h.01" />
    </>,
    p,
  );

export const IconPagePlus = (p: IconProps) =>
  base(
    <>
      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
      <path d="M12 11v6M9 14h6" />
    </>,
    p,
  );

export const IconWand = (p: IconProps) =>
  base(
    <>
      <path d="m4 20 11-11" />
      <path d="m14 6 1-3 1 3 3 1-3 1-1 3-1-3-3-1z" />
      <path d="M19 13v2M18 14h2" />
    </>,
    p,
  );

export const IconEyeOff = (p: IconProps) =>
  base(
    <>
      <path d="M3 3l18 18" />
      <path d="M10.6 6.1A9.8 9.8 0 0 1 12 6c5 0 9 4.5 10 6-0.5.8-1.6 2.2-3.2 3.5M6.5 7.6C4.4 9 2.9 10.9 2 12c1 1.5 5 6 10 6 1.6 0 3-.4 4.3-1" />
      <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
    </>,
    p,
  );

export const IconQr = (p: IconProps) =>
  base(
    <>
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
      <path d="M14 14h3v3h-3zM20 14v.01M20 20h-3M17 20v-3" />
    </>,
    p,
  );

export const IconFido = (p: IconProps) =>
  base(
    <>
      <rect x="7" y="2.5" width="10" height="13" rx="2" />
      <circle cx="12" cy="9" r="2.2" />
      <path d="M10 15.5v4a2 2 0 0 0 4 0v-4" />
    </>,
    p,
  );

export const IconQueue = (p: IconProps) =>
  base(
    <>
      <path d="M8 6h13M8 12h13M8 18h13" />
      <path d="M3 6h.01M3 12h.01M3 18h.01" />
    </>,
    p,
  );
