export function isMac(): boolean {
  // Modern API (Chrome, Edge) - not supported in Safari
  const nav = navigator as Navigator & {
    userAgentData?: { platform?: string };
  };
  if (nav.userAgentData?.platform) {
    return nav.userAgentData.platform === 'macOS';
  }
  // Fallback for Safari and older browsers
  return /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);
}

export function getModifierKey(): string {
  return isMac() ? '⌘' : 'Ctrl';
}

export function isTauriApp(): boolean {
  return '__TAURI_INTERNALS__' in window;
}

export function isTauriMac(): boolean {
  return isTauriApp() && isMac();
}

/** Detect iPad, including iPadOS Safari which masquerades as desktop Mac. */
export function isIPad(): boolean {
  if (/iPad/.test(navigator.userAgent)) return true;
  // iPadOS 13+ reports a Mac user-agent; distinguish it from a real Mac by the
  // presence of touch input.
  return /Mac/.test(navigator.userAgent) && navigator.maxTouchPoints > 1;
}

/**
 * True when running as an installed, standalone web app (added to the home
 * screen / PWA) rather than inside a browser tab.
 */
export function isStandaloneWebApp(): boolean {
  // iOS/iPadOS Safari exposes navigator.standalone; other engines report it via
  // the display-mode media query.
  const nav = navigator as Navigator & { standalone?: boolean };
  if (nav.standalone === true) return true;
  return (
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(display-mode: standalone)').matches
  );
}
