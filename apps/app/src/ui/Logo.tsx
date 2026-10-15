/**
 * PIDA-Wortmarke: eigenes, schlichtes Zeichen (Blatt mit Eselsohr, zwei rote „Textzeilen“ ergeben
 * dezent Rot-Weiß-Rot). Bewusst kein Bundesadler und keine ID-Austria- oder A-Trust-Logos.
 */
export function LogoMark({ size = 28, className }: { size?: number; className?: string }) {
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} aria-hidden="true" focusable="false" className={className}>
      <rect x="1" y="1" width="30" height="30" rx="8" className="logo-bg" />
      <path d="M10 6.5h8.6L23 10.9V25.5H10z" fill="#fff" />
      <path d="M18.6 6.5v4.4H23" className="logo-fold" />
      <rect x="10" y="14.6" width="13" height="2.6" fill="#c8102e" />
      <rect x="10" y="19.8" width="13" height="2.6" fill="#c8102e" />
    </svg>
  );
}

export function Wordmark({ showTagline = true }: { showTagline?: boolean }) {
  return (
    <span className="wordmark">
      <LogoMark size={28} />
      <span className="wordmark-text">
        <span className="wordmark-name">PIDA</span>
        {showTagline ? <span className="wordmark-tag">PDF · ID · Austria</span> : null}
      </span>
    </span>
  );
}

/** Standard-Bildmarke im Signaturblock (neutral, ohne Hoheitszeichen). */
export function DefaultEmblem() {
  return (
    <svg viewBox="0 0 40 40" width="100%" height="100%" aria-hidden="true" focusable="false">
      <circle cx="20" cy="20" r="17.5" fill="#fff" stroke="#3a414d" strokeWidth="1.6" />
      <circle cx="20" cy="20" r="13.6" fill="none" stroke="#3a414d" strokeWidth="0.7" strokeDasharray="1.2 1.4" />
      <path d="M12.5 20.8 17.6 25.6 27.8 14.6" fill="none" stroke="#1f2630" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" />
      <rect x="15" y="30.2" width="10" height="1.3" fill="#c8102e" />
      <rect x="15" y="31.5" width="10" height="1.1" fill="#fff" />
      <rect x="15" y="32.6" width="10" height="1.3" fill="#c8102e" />
    </svg>
  );
}
