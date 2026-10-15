/**
 * PIDA-Zeichen für die Kopfzeile: das App-Symbol im Kleinformat (ADR 0005) – Ring mit Häkchen und
 * rot-weiß-rotem Akzent auf dem Grund der Wortmarke. Abgeleitet vom Siegel im Signaturblock; bewusst kein
 * Bundesadler und keine ID-Austria- oder A-Trust-Logos.
 */
export function LogoMark({ size = 28, className }: { size?: number; className?: string }) {
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} aria-hidden="true" focusable="false" className={className}>
      <rect x="0" y="0" width="32" height="32" rx="7.5" className="logo-bg" />
      <circle cx="16" cy="16" r="9.5" fill="none" stroke="#fff" strokeWidth="2" />
      <path d="M11.9 14.7 14.6 17.25 20.1 11.3" fill="none" stroke="#fff" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
      <rect x="11.94" y="19.5" width="8.12" height="0.88" fill="#c8102e" />
      <rect x="11.94" y="20.38" width="8.12" height="0.69" fill="#fff" />
      <rect x="11.94" y="21.06" width="8.12" height="0.88" fill="#c8102e" />
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
