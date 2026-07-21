import type { AwsSsoProfileStatus } from 'shared/types';

export type AwsProfileLoginAction = 'signIn' | 'reauthenticate' | null;

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
