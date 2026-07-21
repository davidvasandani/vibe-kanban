import { describe, expect, it } from 'vitest';
import type { AwsSsoProfileStatus } from 'shared/types';
import {
  canEditAwsProfile,
  getAwsProfileLoginAction,
} from './awsProfileActions';

const base: AwsSsoProfileStatus = {
  profile: {
    name: 'ai-foundry.AdministratorAccess',
    sso_start_url: 'https://ai-foundry.awsapps.com/start',
    sso_region: 'us-east-1',
    sso_account_id: '123456789012',
    sso_role_name: 'AdministratorAccess',
    region: 'us-east-1',
    output: 'json',
  },
  auth: { status: 'unauthenticated' },
  editable: true,
};

describe('getAwsProfileLoginAction', () => {
  it('offers sign-in for an unauthenticated profile', () => {
    expect(getAwsProfileLoginAction(base)).toBe('signIn');
  });

  it('offers sign-in when the status is unknown (probe could not decide)', () => {
    expect(
      getAwsProfileLoginAction({
        ...base,
        auth: { status: 'unknown', message: 'timed out' },
      })
    ).toBe('signIn');
  });

  it('offers re-authentication after a confirming probe', () => {
    expect(
      getAwsProfileLoginAction({
        ...base,
        auth: { status: 'authenticated', identity: 'arn:aws:sts::123:x' },
      })
    ).toBe('reauthenticate');
  });

  it('hides sign-in when the AWS CLI is missing', () => {
    expect(
      getAwsProfileLoginAction({ ...base, auth: { status: 'cli_missing' } })
    ).toBeNull();
  });

  it('still allows sign-in for the read-only default profile', () => {
    expect(getAwsProfileLoginAction({ ...base, editable: false })).toBe(
      'signIn'
    );
  });
});

describe('canEditAwsProfile', () => {
  it('follows the server-declared editability', () => {
    expect(canEditAwsProfile(base)).toBe(true);
    expect(canEditAwsProfile({ ...base, editable: false })).toBe(false);
  });
});
