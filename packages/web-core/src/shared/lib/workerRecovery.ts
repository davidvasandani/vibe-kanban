import type {
  WorkerNode,
  WorkerUnavailableErrorData,
  WorkerUnavailableReasonCode,
} from 'shared/types';
import { ApiError } from '@/shared/lib/api';
import { isWorkerEligible } from '@/shared/lib/workerPlacement';

/** How long the chat keeps waiting for an execution server to recover. */
export const RECOVERY_WINDOW_MS = 10 * 60 * 1000;

const BACKOFF_INITIAL_MS = 5_000;
const BACKOFF_MAX_MS = 60_000;

const REASON_CODES: readonly WorkerUnavailableReasonCode[] = [
  'not_registered',
  'offline',
  'lease_expired',
  'refusing',
];

/**
 * The typed payload of a 503 "execution server unavailable" response, or
 * `null` for any other failure (including future reason codes, which fall
 * back to the plain error message).
 */
export function asWorkerUnavailable(
  error: unknown
): WorkerUnavailableErrorData | null {
  if (!(error instanceof ApiError) || error.status !== 503) return null;
  const data = error.error_data as
    | Partial<WorkerUnavailableErrorData>
    | undefined;
  if (
    !data ||
    typeof data.worker !== 'string' ||
    !REASON_CODES.includes(data.reason as WorkerUnavailableReasonCode)
  ) {
    return null;
  }
  return {
    worker_node_id:
      typeof data.worker_node_id === 'string' ? data.worker_node_id : null,
    worker: data.worker,
    reason: data.reason as WorkerUnavailableReasonCode,
    retryable: data.retryable !== false,
  };
}

function snapshot(worker: WorkerNode): Record<string, unknown> {
  const value = worker.resource_snapshot;
  return value && typeof value === 'object'
    ? (value as Record<string, unknown>)
    : {};
}

function numericMetric(worker: WorkerNode, key: string): number {
  const value = snapshot(worker)[key];
  return typeof value === 'number' && Number.isFinite(value)
    ? value
    : Number.POSITIVE_INFINITY;
}

/**
 * Whether a worker would admit a new execution for this session right now:
 * the Server Affinity picker's eligibility rule, plus its own heartbeat not
 * reporting an admission refusal.
 */
export function isWorkerAccepting(
  worker: WorkerNode,
  requiredExecutorProfile: string | undefined,
  now = Date.now()
): boolean {
  return (
    isWorkerEligible(worker, now, requiredExecutorProfile) &&
    snapshot(worker).admission_refusing !== true
  );
}

/**
 * Servers that could continue the session instead of the refusing one,
 * best first (lowest load, then fewest executions, then id — the order the
 * scheduler would prefer).
 */
export function rankAlternativeWorkers(
  workers: readonly WorkerNode[],
  refusedWorkerId: string | null,
  requiredExecutorProfile: string | undefined,
  now = Date.now()
): WorkerNode[] {
  return workers
    .filter(
      (worker) =>
        worker.id !== refusedWorkerId &&
        isWorkerAccepting(worker, requiredExecutorProfile, now)
    )
    .sort(
      (left, right) =>
        numericMetric(left, 'load_1m') - numericMetric(right, 'load_1m') ||
        numericMetric(left, 'active_execution_count') -
          numericMetric(right, 'active_execution_count') ||
        left.id.localeCompare(right.id)
    );
}

/** Delay before poll `attempt` (0-based): 5 s doubling, capped at 60 s. */
export function recoveryBackoffMs(attempt: number): number {
  const exponent = Math.max(0, Math.floor(attempt));
  return Math.min(BACKOFF_MAX_MS, BACKOFF_INITIAL_MS * 2 ** exponent);
}
