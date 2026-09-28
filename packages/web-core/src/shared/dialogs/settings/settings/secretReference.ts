/** Prefix of a 1Password secret reference (`op://vault/item/field`). */
export const SECRET_REFERENCE_PREFIX = 'op://';

// Quote pairs 1Password ("Copy Secret Reference") and smart-quote
// substitution wrap around a copied reference.
const REFERENCE_QUOTES: ReadonlyArray<readonly [string, string]> = [
  ['"', '"'],
  ["'", "'"],
  ['“', '”'],
  ['‘', '’'],
];

/**
 * Returns the bare 1Password reference when the whole value is one, optionally
 * surrounded by whitespace and one matching pair of quotes. Returns null for
 * every other value, which callers must keep byte-for-byte because quotes and
 * whitespace may be part of a literal secret. Mirrors
 * `api_types::normalize_secret_reference`.
 */
export function normalizeSecretReference(value: string): string | null {
  const trimmed = value.trim();
  const pair = REFERENCE_QUOTES.find(
    ([open, close]) =>
      trimmed.length >= open.length + close.length &&
      trimmed.startsWith(open) &&
      trimmed.endsWith(close)
  );
  const unquoted = pair ? trimmed.slice(1, -1).trim() : trimmed;
  return unquoted.startsWith(SECRET_REFERENCE_PREFIX) ? unquoted : null;
}
