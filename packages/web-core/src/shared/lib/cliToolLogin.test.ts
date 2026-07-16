import { describe, expect, it } from 'vitest';
import type { CliToolStatus } from 'shared/types';
import { getCliToolLoginAction } from './cliToolLogin';

const base: CliToolStatus = {
  id: 'az',
  binary_name: 'az',
  display_name: 'Azure CLI',
  description: 'Azure CLI',
  catalog_version: '2.88.0',
  supported: true,
  unsupported_reason: null,
  host: { path: '/usr/bin/az', version: '2.88.0' },
  app: null,
  docs_url: 'https://learn.microsoft.com/',
  login_supported: true,
  auth_state: 'unauthenticated',
  auth_message: null,
};

describe('getCliToolLoginAction', () => {
  it('offers login for an available unauthenticated supported tool', () => {
    expect(getCliToolLoginAction(base)).toBe('login');
  });

  it('offers re-authentication after a successful probe', () => {
    expect(
      getCliToolLoginAction({ ...base, auth_state: 'authenticated' })
    ).toBe('reauthenticate');
  });

  it('hides login for unavailable and unsupported tools', () => {
    expect(getCliToolLoginAction({ ...base, host: null })).toBeNull();
    expect(
      getCliToolLoginAction({ ...base, login_supported: false })
    ).toBeNull();
  });
});
