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

/**
 * Logo von A-SIT (Herausgeber; Vorlage https://www.a-sit.at/wp-content/themes/wp-asit/images/logo.svg, 170 × 50,
 * Farben wie im Original). Auf hellem Grund – im dunklen Farbschema mit eigener Fläche (`.asit-logo`).
 */
export function AsitLogo({ height = 28, className }: { height?: number; className?: string }) {
  return (
    <svg viewBox="0 0 170 50" height={height} width={(height * 170) / 50} role="img" aria-label="A-SIT" className={`asit-logo${className ? ` ${className}` : ''}`}>
      <path fill="#005c7a" d="M59.3 30.7h-18c-.1 0-.2-.1-.2-.2v-5c0-.1.1-.2.2-.2h18c.1 0 .2.1.2.2v5c0 .1-.1.2-.2.2zm-16.6 17L25.3 3.2c0-.1-.1-.2-.2-.2h-6.5c-.1 0-.2.1-.2.2L1 47.7v.2c.1.1.2.1.2.1h6c.1 0 .2-.1.2-.2l4.9-13.2 1.4-3.9h15.6l1.4 3.9 4.8 13.2c0 .1.1.2.2.2h6.5c.1 0 .2 0 .2-.1.3-.1.3-.2.3-.2zm-21-38.5l5.7 16.1H15.9l5.8-16.1zM83.8 49c-2.4 0-4.7-.3-6.8-.9-2.2-.6-4.2-1.6-5.9-2.9-1.7-1.3-3.1-2.9-4.1-4.9-.9-1.8-1.4-4-1.5-6.5V31c0-.1 0-.1.1-.2 0 0 .1-.1.2-.1h5.6c.1 0 .2.1.2.2v2.8c.1 1.6.4 2.9 1 4.1.7 1.3 1.6 2.4 2.8 3.2 1.2.9 2.5 1.5 4.1 1.9 1.4.4 2.8.6 4.2.6 1.5 0 2.9-.2 4.3-.6 1.5-.4 2.9-1 4.1-1.9 1.1-.8 2.1-1.9 2.8-3.2.7-1.2 1-2.7 1.1-4.4v-8.2H65.8c-.1 0-.2-.1-.2-.2v-7.8c.1-2.5.6-4.7 1.5-6.5 1-2 2.3-3.7 4.1-4.9 1.7-1.3 3.7-2.2 5.9-2.9 2-.6 4.3-.9 6.7-.9 2.4 0 4.7.3 6.8.9 2.2.6 4.2 1.6 5.9 2.9 1.7 1.3 3.1 2.9 4.1 4.9.5 1.1 1.3 3.4 1.3 5 0 .1-.1.3-.2.3l-5.7.1c-.1 0-.1-.1-.1-.2v-.2c-.2-1-.5-1.8-.9-2.6-.7-1.3-1.6-2.4-2.8-3.2C91 9 89.6 8.4 88.1 8c-1.4-.4-2.8-.6-4.2-.6-1.5 0-2.9.2-4.3.6-1.5.4-2.9 1-4.1 1.9-1.1.8-2.1 1.9-2.8 3.2-.6 1.2-1 2.6-1 4.1v2.5h30.2c.1 0 .2.1.2.2v13.4c0 2.6-.5 4.9-1.5 6.8-1 2-2.3 3.7-4.1 4.9-1.7 1.3-3.7 2.2-5.9 2.9-2.1.8-4.4 1.1-6.8 1.1zm34.7-1h-5.9c-.1 0-.2-.1-.2-.2V3.3c0-.1.1-.2.2-.2h5.9c.1 0 .2.1.2.2v44.4c0 .2-.1.3-.2.3zm27.4 0H140c-.1 0-.2-.1-.2-.2V8.5h-14.6c-.1 0-.2-.1-.2-.2v-5c0-.1.1-.2.2-.2h35.5c.1 0 .2.1.2.2v5c0 .1-.1.2-.2.2h-14.6v39.2c0 .2-.1.3-.2.3z" />
      <path fill="#cd492e" d="M85.1 38.5h-2.6c-.1 0-.2-.1-.2-.2V33c-.8-.5-1.3-1.4-1.3-2.3 0-1.5 1.3-2.8 2.8-2.8 1.5 0 2.8 1.3 2.8 2.8 0 .9-.5 1.8-1.3 2.3v5.2c0 .2-.1.3-.2.3z" />
    </svg>
  );
}
