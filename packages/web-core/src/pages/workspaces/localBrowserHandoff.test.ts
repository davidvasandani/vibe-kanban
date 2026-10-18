import { expect, it } from 'vitest';
import { localBrowserOrigin } from './localBrowserHandoff';

it('drops all session-bearing URL components including path and credentials', () => {
  expect(
    localBrowserOrigin(
      'https://user:secret@example.com/token/secret?auth=secret#secret'
    )
  ).toBe('https://example.com');
  expect(localBrowserOrigin('http://localhost:3000/cart')).toBe(
    'http://localhost:3000'
  );
});
it('rejects non-web, relative and invalid destinations', () => {
  for (const value of [
    null,
    undefined,
    '',
    '/checkout',
    'javascript:alert(1)',
    'data:text/html,secret',
    'file:///secret',
    'about:blank',
    'broken',
  ]) {
    expect(localBrowserOrigin(value)).toBeNull();
  }
});
