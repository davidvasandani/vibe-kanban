import type { CliToolStatus } from 'shared/types';

export type CliToolLoginAction = 'login' | 'reauthenticate' | null;

export function getCliToolLoginAction(tool: CliToolStatus): CliToolLoginAction {
  const available = tool.app !== null || tool.host !== null;
  if (!available || !tool.login_supported) return null;
  return tool.auth_state === 'authenticated' ? 'reauthenticate' : 'login';
}
