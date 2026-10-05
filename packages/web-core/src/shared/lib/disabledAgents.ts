import type { BaseCodingAgent, ExecutorProfile } from 'shared/types';

type ProfilesLike = Partial<Record<string, unknown>> | null | undefined;

/** Why an agent can't be disabled right now, or `null` if it can. */
export type DisableAgentBlocker = 'default' | 'last';

/**
 * The one frontend definition of a disabled agent. Disabling only hides the
 * agent from pickers; it never blocks running it.
 */
export function isAgentDisabled(
  profiles: ProfilesLike,
  agent: string | null | undefined
): boolean {
  if (!profiles || !agent) return false;
  const profile = profiles[agent];
  return (
    typeof profile === 'object' &&
    profile !== null &&
    (profile as { disabled?: unknown }).disabled === true
  );
}

/**
 * Drop disabled agents from a picker's options, keeping their order. Agents in
 * `keep` (the current selection) always stay visible so a trigger never names
 * an agent missing from its own menu. If no agent is enabled (only possible
 * by hand-editing profiles), offer every agent rather than a dead end.
 */
export function filterEnabledAgents<T extends string>(
  agents: readonly T[],
  profiles: ProfilesLike,
  keep: readonly (T | null | undefined)[] = []
): T[] {
  if (agents.every((agent) => isAgentDisabled(profiles, agent))) {
    return [...agents];
  }
  return agents.filter(
    (agent) => !isAgentDisabled(profiles, agent) || keep.includes(agent)
  );
}

export function setAgentDisabled<
  T extends Partial<Record<string, ExecutorProfile>>,
>(profiles: T, agent: BaseCodingAgent, disabled: boolean): T {
  const nextProfile = { ...(profiles[agent] ?? {}) } as ExecutorProfile;
  if (disabled) {
    nextProfile.disabled = true;
  } else {
    delete nextProfile.disabled;
  }
  return { ...profiles, [agent]: nextProfile };
}

/**
 * The default agent and the last enabled agent can't be disabled. Re-enabling
 * is never blocked.
 */
export function getDisableAgentBlocker(
  profiles: ProfilesLike,
  agent: string,
  defaultAgent: string | null | undefined
): DisableAgentBlocker | null {
  if (!profiles || isAgentDisabled(profiles, agent)) return null;
  if (agent === defaultAgent) return 'default';
  const othersEnabled = Object.keys(profiles).some(
    (other) => other !== agent && !isAgentDisabled(profiles, other)
  );
  return othersEnabled ? null : 'last';
}
