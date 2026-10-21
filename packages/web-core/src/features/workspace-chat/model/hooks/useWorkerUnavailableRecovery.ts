import { useCallback, useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { WorkerNode } from 'shared/types';
import { workerNodesApi, workspacesApi } from '@/shared/lib/api';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import {
  RECOVERY_WINDOW_MS,
  asWorkerUnavailable,
  isWorkerAccepting,
  rankAlternativeWorkers,
  recoveryBackoffMs,
} from '@/shared/lib/workerRecovery';

export type ServerRecoveryPhase =
  /** Another server can take the session; polling continues. */
  | 'suggesting'
  /** No other server can; polling for any server to accept. */
  | 'waiting'
  | 'moving'
  | 'resending'
  /** The recovery window ended without a successful send. */
  | 'expired';

export interface ServerRecoveryState {
  phase: ServerRecoveryPhase;
  /** Hostname of the server that refused the send. */
  refusedWorker: string;
  /** The API's explanation, naming the server and its reason. */
  message: string;
  alternatives: WorkerNode[];
  /** Epoch ms of the next poll, while one is scheduled. */
  nextCheckAt: number | null;
  /** Epoch ms when the recovery window closes. */
  deadline: number;
  /** Last move or poll failure, shown alongside the notice. */
  error: string | null;
}

interface RecoverySession {
  prompt: string;
  refusedWorkerId: string | null;
  deadline: number;
  attempt: number;
}

interface UseWorkerUnavailableRecoveryOptions {
  sessionId: string | undefined;
  workspaceId: string | undefined;
  /** `EXECUTOR` or `EXECUTOR:VARIANT` of the session, for eligibility. */
  requiredExecutorProfile: string | undefined;
  /**
   * Resend the exact prompt that failed. Resolves true on success. A repeat
   * refusal is reported back through `start`, which continues the window.
   */
  resend: (prompt: string) => Promise<boolean>;
}

export interface UseWorkerUnavailableRecoveryResult {
  state: ServerRecoveryState | null;
  /**
   * Begin (or continue) recovery for a failed send to `originSessionId`.
   * Returns false when the error is not an unavailable execution server, or
   * the send belonged to a session that is no longer the current one.
   */
  start: (
    prompt: string,
    error: unknown,
    originSessionId: string | undefined
  ) => boolean;
  moveTo: (workerNodeId: string) => Promise<void>;
  retryNow: () => Promise<void>;
  dismiss: () => void;
}

/**
 * Recovery for a follow-up refused because the workspace's execution server
 * is unavailable (under pressure, draining, offline). Suggests eligible,
 * non-refusing alternatives the user can move the workspace to; otherwise
 * polls the worker list with capped exponential backoff for up to
 * {@link RECOVERY_WINDOW_MS} and resends automatically — only to the same
 * server, and only once its heartbeat says it accepts work again.
 */
export function useWorkerUnavailableRecovery({
  sessionId,
  workspaceId,
  requiredExecutorProfile,
  resend,
}: UseWorkerUnavailableRecoveryOptions): UseWorkerUnavailableRecoveryResult {
  const queryClient = useQueryClient();
  const [state, setState] = useState<ServerRecoveryState | null>(null);
  const sessionRef = useRef<RecoverySession | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Bumped whenever recovery is reset, so in-flight polls and resends from an
  // earlier recovery never write state.
  const generationRef = useRef(0);
  // Set by `start` while a resend is in flight: the resend was refused again
  // and recovery continues, rather than the send failing for another reason.
  const continuedRef = useRef(false);
  // A move or resend is in flight; polls must neither act nor reschedule.
  const busyRef = useRef(false);
  const resendRef = useRef(resend);
  resendRef.current = resend;
  const sessionIdRef = useRef(sessionId);
  sessionIdRef.current = sessionId;
  const profileRef = useRef(requiredExecutorProfile);
  profileRef.current = requiredExecutorProfile;
  const pollRef = useRef<(allowResend: boolean) => Promise<void>>(
    async () => undefined
  );

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const dismiss = useCallback(() => {
    generationRef.current += 1;
    clearTimer();
    busyRef.current = false;
    sessionRef.current = null;
    setState(null);
  }, [clearTimer]);

  const schedule = useCallback(
    (generation: number) => {
      const session = sessionRef.current;
      if (!session || generation !== generationRef.current || busyRef.current)
        return;
      clearTimer();
      const now = Date.now();
      if (now >= session.deadline) {
        setState((current) =>
          current ? { ...current, phase: 'expired', nextCheckAt: null } : null
        );
        return;
      }
      const delay = Math.min(
        recoveryBackoffMs(session.attempt),
        session.deadline - now
      );
      session.attempt += 1;
      setState((current) =>
        current ? { ...current, nextCheckAt: now + delay } : null
      );
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        void pollRef.current(true);
      }, delay);
    },
    [clearTimer]
  );

  const runResend = useCallback(
    async (generation: number) => {
      const session = sessionRef.current;
      if (!session) return;
      clearTimer();
      setState((current) =>
        current
          ? { ...current, phase: 'resending', nextCheckAt: null, error: null }
          : null
      );
      continuedRef.current = false;
      busyRef.current = true;
      let ok = false;
      try {
        ok = await resendRef.current(session.prompt);
      } finally {
        if (generation === generationRef.current) busyRef.current = false;
      }
      if (generation !== generationRef.current) return;
      if (continuedRef.current) {
        // Refused again: `start` already updated the state. Look for
        // alternatives now (no resend), which also schedules the next poll.
        void pollRef.current(false);
      } else if (ok) {
        dismiss();
      } else {
        // Failed for an unrelated reason: the caller shows that error.
        dismiss();
      }
    },
    [clearTimer, dismiss]
  );

  pollRef.current = async (allowResend: boolean) => {
    const generation = generationRef.current;
    const session = sessionRef.current;
    if (!session) return;
    let workers: WorkerNode[];
    try {
      workers = await workerNodesApi.list();
    } catch (error) {
      if (generation !== generationRef.current) return;
      setState((current) =>
        current
          ? {
              ...current,
              error: error instanceof Error ? error.message : String(error),
            }
          : null
      );
      schedule(generation);
      return;
    }
    if (generation !== generationRef.current || busyRef.current) return;
    const now = Date.now();
    // The request may settle after the window closed (or the tab slept past
    // it): never auto-resend once the advertised deadline has passed.
    if (now >= session.deadline) {
      clearTimer();
      setState((current) =>
        current ? { ...current, phase: 'expired', nextCheckAt: null } : null
      );
      return;
    }
    const refused = workers.find(
      (worker) => worker.id === session.refusedWorkerId
    );
    if (
      allowResend &&
      refused &&
      isWorkerAccepting(refused, profileRef.current, now)
    ) {
      await runResend(generation);
      return;
    }
    const alternatives = rankAlternativeWorkers(
      workers,
      session.refusedWorkerId,
      profileRef.current,
      now
    );
    setState((current) =>
      current && (current.phase === 'suggesting' || current.phase === 'waiting')
        ? {
            ...current,
            alternatives,
            phase: alternatives.length > 0 ? 'suggesting' : 'waiting',
            error: null,
          }
        : current
    );
    schedule(generation);
  };

  const start = useCallback(
    (
      prompt: string,
      error: unknown,
      originSessionId: string | undefined
    ): boolean => {
      // A send that settles after the user switched sessions must not start
      // recovery here: its resend would go to the wrong conversation.
      if (!originSessionId || originSessionId !== sessionIdRef.current) {
        return false;
      }
      const data = asWorkerUnavailable(error);
      if (!data || !data.retryable) return false;
      const message = error instanceof Error ? error.message : String(error);
      const existing = sessionRef.current;
      const continuing = existing !== null && existing.prompt === prompt;
      if (continuing) {
        // A resend (or a move) was refused again: same window, same backoff
        // level, but the refusing server may now be a different one.
        continuedRef.current = true;
        existing.refusedWorkerId = data.worker_node_id;
      } else {
        generationRef.current += 1;
        sessionRef.current = {
          prompt,
          refusedWorkerId: data.worker_node_id,
          deadline: Date.now() + RECOVERY_WINDOW_MS,
          attempt: 0,
        };
      }
      const session = sessionRef.current;
      if (!session) return false;
      clearTimer();
      setState((current) => ({
        phase: 'waiting',
        refusedWorker: data.worker,
        message,
        alternatives: continuing && current ? current.alternatives : [],
        nextCheckAt: null,
        deadline: session.deadline,
        error: null,
      }));
      // Look for alternatives right away, but never resend on this first
      // look: the refusing server's last heartbeat may predate its refusal.
      // During a resend, `runResend` runs this look once the send settles.
      if (!busyRef.current) void pollRef.current(false);
      return true;
    },
    [clearTimer]
  );

  const moveTo = useCallback(
    async (workerNodeId: string) => {
      const session = sessionRef.current;
      if (!session || !workspaceId) return;
      const generation = generationRef.current;
      clearTimer();
      busyRef.current = true;
      setState((current) =>
        current
          ? { ...current, phase: 'moving', nextCheckAt: null, error: null }
          : null
      );
      try {
        await workspacesApi.updateAffinity(workspaceId, {
          run_on_coordinator: false,
          requested_worker_node_id: workerNodeId,
          restart_running: false,
          operation_id: null,
        });
      } catch (error) {
        if (generation !== generationRef.current) return;
        busyRef.current = false;
        setState((current) =>
          current
            ? {
                ...current,
                phase:
                  current.alternatives.length > 0 ? 'suggesting' : 'waiting',
                error: error instanceof Error ? error.message : String(error),
              }
            : null
        );
        schedule(generation);
        return;
      }
      if (generation !== generationRef.current) return;
      busyRef.current = false;
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: ['workspacePlacement'] }),
        queryClient.invalidateQueries({ queryKey: ['workerNodes'] }),
        queryClient.invalidateQueries({ queryKey: workspaceSummaryKeys.all }),
      ]);
      await runResend(generation);
    },
    [clearTimer, queryClient, runResend, schedule, workspaceId]
  );

  const retryNow = useCallback(async () => {
    const session = sessionRef.current;
    if (!session) return;
    // A manual retry opens a fresh window if it is refused again.
    session.deadline = Date.now() + RECOVERY_WINDOW_MS;
    session.attempt = 0;
    setState((current) =>
      current ? { ...current, deadline: session.deadline } : null
    );
    await runResend(generationRef.current);
  }, [runResend]);

  // Recovery belongs to one session's failed send.
  useEffect(() => dismiss, [sessionId, dismiss]);

  return { state, start, moveTo, retryNow, dismiss };
}
