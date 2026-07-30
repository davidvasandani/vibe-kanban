import type {
  AwsAuthStatus,
  AwsSsoAuthScope,
  AwsSsoProfileStatus,
} from 'shared/types';

export type AwsProfileLoginAction = 'signIn' | 'reauthenticate' | null;

export type AwsAuthScopeGroup = {
  scope: AwsSsoAuthScope;
  profiles: AwsSsoProfileStatus[];
  representative: AwsSsoProfileStatus;
  auth: AwsAuthStatus;
};

export function getAwsProfileLoginAction(
  status: AwsSsoProfileStatus
): AwsProfileLoginAction {
  if (status.auth.status === 'cli_missing') return null;
  return status.auth.status === 'authenticated' ? 'reauthenticate' : 'signIn';
}

/** `[default]` is list/sign-in only; VK never rewrites it. */
export function canEditAwsProfile(status: AwsSsoProfileStatus): boolean {
  return status.editable;
}

function aggregateAwsAuthStatus(
  profiles: AwsSsoProfileStatus[]
): AwsAuthStatus {
  const cliMissing = profiles.find(({ auth }) => auth.status === 'cli_missing');
  if (cliMissing) return cliMissing.auth;

  const unauthenticated = profiles.find(
    ({ auth }) => auth.status === 'unauthenticated'
  );
  if (unauthenticated) return unauthenticated.auth;

  const unknown = profiles.find(({ auth }) => auth.status === 'unknown');
  if (unknown) return unknown.auth;

  return profiles[0].auth;
}

/** Groups profiles by the backend-authored AWS CLI token-cache identity. */
export function groupAwsProfilesByAuthScope(
  profiles: AwsSsoProfileStatus[]
): AwsAuthScopeGroup[] {
  const groups = new Map<string, AwsAuthScopeGroup>();

  for (const profile of profiles) {
    const existing = groups.get(profile.auth_scope.key);
    if (existing) {
      existing.profiles.push(profile);
      existing.auth = aggregateAwsAuthStatus(existing.profiles);
      continue;
    }
    groups.set(profile.auth_scope.key, {
      scope: profile.auth_scope,
      profiles: [profile],
      representative: profile,
      auth: profile.auth,
    });
  }

  return [...groups.values()];
}
