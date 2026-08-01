import {
  type WorkerNode,
  WorkerMountStatus,
  WorkerNodeStatus,
} from 'shared/types';

/**
 * Which agents a cluster can actually run, for gating the agent picker.
 *
 * This mirrors the coordinator's `advertises_executor_profile`, but it is an
 * affordance, not an authorisation boundary: the coordinator remains the
 * enforcement point. It exists so a user learns an agent is unavailable before
 * writing a prompt, rather than from a rejection afterwards.
 *
 * `WorkerNode.capabilities` crosses the wire as `unknown` (the Rust model marks
 * it `#[ts(type = "unknown")]`), so every read here is defensive.
 */

/** Executor names that parse to a different canonical name on the backend. */
const EXECUTOR_ALIASES: Record<string, string> = {
  CURSOR: 'CURSOR_AGENT',
};

function canonicalExecutor(raw: string): string {
  const upper = raw.trim().replace(/-/g, '_').toUpperCase();
  return EXECUTOR_ALIASES[upper] ?? upper;
}

/** `EXECUTOR[:VARIANT]`, with the variant absent meaning "any variant". */
interface AdvertisedProfile {
  executor: string;
  variant: string | null;
}

function parseProfile(raw: string): AdvertisedProfile | null {
  const separator = raw.indexOf(':');
  if (separator === -1) {
    const executor = canonicalExecutor(raw);
    return executor ? { executor, variant: null } : null;
  }
  const executor = canonicalExecutor(raw.slice(0, separator));
  if (!executor) return null;
  return {
    executor,
    variant: raw
      .slice(separator + 1)
      .trim()
      .toUpperCase(),
  };
}

function readAdvertisedProfiles(capabilities: unknown): string[] {
  if (typeof capabilities !== 'object' || capabilities === null) return [];
  const profiles = (capabilities as Record<string, unknown>).executor_profiles;
  if (!Array.isArray(profiles)) return [];
  return profiles.filter((entry): entry is string => typeof entry === 'string');
}

/**
 * Profiles advertised by workers that could currently accept work, or `null`
 * when no opinion can be formed.
 *
 * Lease expiry is deliberately not re-checked here. It would degrade *closed*
 * against a clock the browser does not own, and the coordinator already flips
 * lease-expired workers to `offline`.
 *
 * Returning `null` rather than `[]` is load-bearing: an empty array would read
 * as "this cluster runs nothing" and disable every agent. A gate that cannot
 * parse its input must permit everything, never hide everything.
 */
export function clusterAdvertisedProfiles(
  workers: WorkerNode[]
): string[] | null {
  const advertised = workers
    .filter(
      (worker) =>
        worker.status === WorkerNodeStatus.online &&
        worker.mount_status === WorkerMountStatus.healthy
    )
    .flatMap((worker) => readAdvertisedProfiles(worker.capabilities));

  return advertised.length > 0 ? advertised : null;
}

/**
 * Whether `executor` can run somewhere on the cluster.
 *
 * Compares whole profiles rather than executor names. A cluster advertising
 * only `CODEX:PLAN` does not support a plain Codex request, and reporting it as
 * available would recreate the write-a-prompt-then-get-rejected dead end this
 * gate exists to remove.
 */
export function clusterSupportsExecutor(
  advertised: string[] | null,
  executor: string,
  variant?: string | null
): boolean {
  if (advertised === null) return true;

  const wanted = canonicalExecutor(executor);
  const wantedVariant = variant ? variant.trim().toUpperCase() : null;

  return advertised.some((raw) => {
    const profile = parseProfile(raw);
    if (profile === null || profile.executor !== wanted) return false;
    // A bare advertisement covers every variant of that executor.
    if (profile.variant === null) return true;
    return wantedVariant !== null && profile.variant === wantedVariant;
  });
}
