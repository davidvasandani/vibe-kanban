import { describe, expect, it } from 'vitest';
import type { AwsSsoProfileStatus } from 'shared/types';
import {
  canEditAwsProfile,
  getAwsProfileLoginAction,
  groupAwsProfilesByAuthScope,
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
  auth_scope: {
    key: 'session:ai-foundry',
    label: 'ai-foundry',
    session_name: 'ai-foundry',
  },
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

describe('groupAwsProfilesByAuthScope', () => {
  const withName = (
    name: string,
    patch: Partial<AwsSsoProfileStatus> = {}
  ): AwsSsoProfileStatus => ({
    ...base,
    ...patch,
    profile: { ...base.profile, name },
  });

  it('groups profiles in the same named session and preserves a representative', () => {
    const profiles = [
      withName('ai-foundry.Admin'),
      withName('ai-foundry.ReadOnly'),
    ];

    const groups = groupAwsProfilesByAuthScope(profiles);

    expect(groups).toHaveLength(1);
    expect(groups[0].profiles).toHaveLength(2);
    expect(groups[0].representative.profile.name).toBe('ai-foundry.Admin');
  });

  it('groups legacy profiles sharing a start URL', () => {
    const auth_scope = {
      key: 'start-url:https://example.awsapps.com/start',
      label: 'https://example.awsapps.com/start',
      session_name: null,
    };

    expect(
      groupAwsProfilesByAuthScope([
        withName('legacy-a', { auth_scope }),
        withName('legacy-b', { auth_scope }),
      ])
    ).toHaveLength(1);
  });

  it('keeps distinct named sessions separate even when profiles share a URL', () => {
    const groups = groupAwsProfilesByAuthScope([
      withName('first.Admin', {
        auth_scope: {
          key: 'session:first',
          label: 'first',
          session_name: 'first',
        },
      }),
      withName('second.Admin', {
        auth_scope: {
          key: 'session:second',
          label: 'second',
          session_name: 'second',
        },
      }),
    ]);

    expect(groups.map(({ scope }) => scope.key)).toEqual([
      'session:first',
      'session:second',
    ]);
  });

  it('aggregates status conservatively', () => {
    const authenticated = withName('org.Admin', {
      auth: { status: 'authenticated', identity: 'arn:first' },
    });
    const status = (
      auth: AwsSsoProfileStatus['auth']
    ): AwsSsoProfileStatus[] => [
      authenticated,
      withName('org.Other', { auth }),
    ];

    expect(
      groupAwsProfilesByAuthScope(status(authenticated.auth))[0].auth
    ).toBe(authenticated.auth);
    expect(
      groupAwsProfilesByAuthScope(
        status({ status: 'unknown', message: 'timed out' })
      )[0].auth.status
    ).toBe('unknown');
    expect(
      groupAwsProfilesByAuthScope(status({ status: 'unauthenticated' }))[0].auth
        .status
    ).toBe('unauthenticated');
    expect(
      groupAwsProfilesByAuthScope(status({ status: 'cli_missing' }))[0].auth
        .status
    ).toBe('cli_missing');
  });
});
