import { describe, expect, it } from 'vitest';
import { normalizeSecretReference } from './secretReference';

const REFERENCE = 'op://Homelab/alderbridge nix PAT/credential';

describe('normalizeSecretReference', () => {
  it.each([
    REFERENCE,
    `  ${REFERENCE}\n`,
    `"${REFERENCE}"`,
    ` " ${REFERENCE} " `,
    `'${REFERENCE}'`,
    `“${REFERENCE}”`,
    `‘${REFERENCE}’`,
  ])('returns the bare reference for %j', (value) => {
    expect(normalizeSecretReference(value)).toBe(REFERENCE);
  });

  it('keeps an incomplete reference readable', () => {
    expect(normalizeSecretReference('"op://"')).toBe('op://');
  });

  it.each([
    '',
    '"',
    '""',
    'plain-secret',
    '"quoted literal"',
    '"op://x\'',
    '"op://x',
    '""op://x""',
    'OP://vault/item/field',
    'prefix op://vault/item/field',
  ])('returns null for literal %j', (value) => {
    expect(normalizeSecretReference(value)).toBeNull();
  });
});
