// Start at the site origin: paths as well as queries can carry session tokens.
// This is a separate local session, never an export of hosted browser state.
export function localBrowserOrigin(
  value: string | null | undefined
): string | null {
  if (!value) return null;
  try {
    const url = new URL(value);
    if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
    return url.origin;
  } catch {
    return null;
  }
}
