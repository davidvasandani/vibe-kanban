import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SpinnerIcon } from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import type {
  ReauthOverview,
  ReauthRun,
  ReauthRunReport,
  ReauthTargetStatus,
} from 'shared/types';
import { SettingsCard } from './SettingsComponents';
import { useSettingsMachineClient } from './SettingsHostContext';

const POLL_MS = 3000;
/**
 * How long to keep polling after a bulk request stopped waiting on discovery.
 * The server keeps discovering and starts repairs on its own, so the card
 * keeps watching for them instead of treating the error as the end.
 */
const DETACHED_WATCH_MS = 5 * 60 * 1000;

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * Whether `incoming` may replace `current`: a later run, or the same run
 * moving forward. A slow response must never roll a newer run back (which
 * would also stop the polling that tracks it).
 */
export function isNewerRun(
  current: ReauthRun | null,
  incoming: ReauthRun
): boolean {
  if (!current) return true;
  const a = Date.parse(current.started_at);
  const b = Date.parse(incoming.started_at);
  if (b !== a) return b > a;
  if (current.outcome !== 'running') return false;
  // Same run: finishing is progress, and so is a longer (append-only)
  // transcript while it is still running.
  return (
    incoming.outcome !== 'running' ||
    incoming.transcript.length > current.transcript.length
  );
}

/** Apply newer runs from a poll or a run request to each target. */
export function mergeRuns(
  overview: ReauthOverview,
  runs: ReauthRunReport[]
): ReauthOverview {
  const byId = new Map(runs.map((r) => [r.id, r.run]));
  return {
    ...overview,
    targets: overview.targets.map((target) => {
      const run = byId.get(target.id);
      return run && isNewerRun(target.last_run, run)
        ? { ...target, last_run: run }
        : target;
    }),
  };
}

/**
 * Apply a full refresh without losing a newer run the card already holds (a
 * run started, or polled, while the refresh was in flight).
 */
export function applyRefresh(
  current: ReauthOverview | null,
  next: ReauthOverview
): ReauthOverview {
  if (!current) return next;
  const held = new Map(current.targets.map((t) => [t.id, t.last_run]));
  return {
    ...next,
    targets: next.targets.map((target) => {
      const mine = held.get(target.id);
      return mine && isNewerRun(target.last_run, mine)
        ? { ...target, last_run: mine }
        : target;
    }),
  };
}

function isRunning(target: ReauthTargetStatus): boolean {
  return target.last_run?.outcome === 'running';
}

/**
 * Credentials the selected host re-authenticates with no human involved.
 * Messages are shown exactly as the server reported them.
 */
export function ReauthSettingsCard() {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const [overview, setOverview] = useState<ReauthOverview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [starting, setStarting] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [watchUntil, setWatchUntil] = useState<number | null>(null);
  // Responses are only applied while their host is still selected: a late
  // answer from the previous host must never repaint this one, or its
  // buttons would act on the wrong machine's credentials.
  const clientRef = useRef(machineClient);
  clientRef.current = machineClient;
  // Newest refresh wins even on one host (a poll-triggered refresh can
  // overlap the initial load).
  const refreshSeq = useRef(0);

  const refresh = useCallback(async () => {
    if (!machineClient) return;
    const seq = ++refreshSeq.current;
    const current = () =>
      clientRef.current === machineClient && refreshSeq.current === seq;
    try {
      const next = await machineClient.listReauthTargets();
      if (!current()) return;
      setOverview((held) => applyRefresh(held, next));
      setLoadError(null);
    } catch (err) {
      if (current()) setLoadError(errorText(err));
    }
  }, [machineClient]);

  useEffect(() => {
    setOverview(null);
    setLoadError(null);
    setRunError(null);
    setStarting(null);
    setWatchUntil(null);
    void refresh();
  }, [refresh]);

  const anyRunning = overview?.targets.some(isRunning) ?? false;

  // While a run is active (or a detached bulk request may still start some),
  // poll only the run registry (no probing); once everything settles, one
  // full refresh picks up the re-probed states.
  const polling = anyRunning || watchUntil !== null;
  useEffect(() => {
    if (!polling || !machineClient) return;
    const handle = setInterval(async () => {
      try {
        const runs = await machineClient.listReauthRuns();
        if (clientRef.current !== machineClient) return;
        setOverview((current) =>
          current ? mergeRuns(current, runs) : current
        );
        const stillRunning = runs.some((r) => r.run.outcome === 'running');
        if (stillRunning) {
          // Runs have appeared; the running state now drives polling.
          setWatchUntil(null);
        } else if (watchUntil === null || Date.now() >= watchUntil) {
          setWatchUntil(null);
          void refresh();
        }
      } catch (err) {
        if (clientRef.current === machineClient) setRunError(errorText(err));
      }
    }, POLL_MS);
    return () => clearInterval(handle);
  }, [polling, watchUntil, machineClient, refresh]);

  const run = async (target?: string) => {
    const client = machineClient;
    if (!client) return;
    setStarting(target ?? '*');
    setRunError(null);
    try {
      const runs = await client.runReauth(target);
      if (clientRef.current !== client) return;
      setOverview((current) => (current ? mergeRuns(current, runs) : current));
      // Runs that already finished start no polling, so nothing else would
      // pick up their re-probed state.
      if (!runs.some((r) => r.run.outcome === 'running')) void refresh();
    } catch (err) {
      if (clientRef.current !== client) return;
      setRunError(errorText(err));
      // A bulk request can stop waiting while discovery carries on and
      // starts repairs; keep watching the registry for them.
      if (target === undefined) setWatchUntil(Date.now() + DETACHED_WATCH_MS);
    } finally {
      if (clientRef.current === client) setStarting(null);
    }
  };

  const sweep = overview?.sweep_interval_secs;

  return (
    <SettingsCard
      title={t('settings.reauth.title', { ns: 'settings' })}
      description={t('settings.reauth.description', { ns: 'settings' })}
    >
      {overview === null && !loadError && (
        <div className="flex items-center gap-2 text-sm text-low">
          <SpinnerIcon className="size-icon-sm animate-spin" />
          {t('settings.reauth.loading', { ns: 'settings' })}
        </div>
      )}
      {loadError && (
        <p className="text-sm text-error whitespace-pre-wrap break-words">
          {loadError}
        </p>
      )}
      {overview && (
        <div className="flex items-center justify-between gap-2">
          <p className="text-sm text-low">
            {sweep
              ? t('settings.reauth.sweepEvery', {
                  ns: 'settings',
                  minutes: Math.round(sweep / 60),
                })
              : t('settings.reauth.sweepOff', { ns: 'settings' })}
          </p>
          <Button
            size="sm"
            variant="secondary"
            disabled={starting !== null}
            onClick={() => void run()}
          >
            {t('settings.reauth.runAll', { ns: 'settings' })}
          </Button>
        </div>
      )}
      {runError && (
        <p className="text-sm text-error whitespace-pre-wrap break-words">
          {runError}
        </p>
      )}
      {overview?.targets.length === 0 && (
        <p className="text-sm text-low">
          {t('settings.reauth.empty', { ns: 'settings' })}
        </p>
      )}
      {overview?.targets.map((target) => (
        <ReauthTargetRow
          key={target.id}
          target={target}
          disabled={starting !== null || isRunning(target)}
          expanded={expanded === target.id}
          onToggle={() =>
            setExpanded((current) => (current === target.id ? null : target.id))
          }
          onRun={() => void run(target.id)}
        />
      ))}
    </SettingsCard>
  );
}

function ReauthTargetRow({
  target,
  disabled,
  expanded,
  onToggle,
  onRun,
}: {
  target: ReauthTargetStatus;
  disabled: boolean;
  expanded: boolean;
  onToggle: () => void;
  onRun: () => void;
}) {
  const { t } = useTranslation(['settings']);
  const run = target.last_run;

  return (
    <div
      className="rounded-sm border border-border p-3 space-y-2"
      data-testid={`reauth-target-${target.id}`}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-sm font-medium text-high">
              {target.label}
            </span>
            <code className="text-xs text-low">{target.id}</code>
          </div>
          <p className="text-sm text-low mt-1">
            {t(`settings.reauth.state.${target.auth_state}`, {
              ns: 'settings',
            })}
            {!target.swept &&
              ` · ${t('settings.reauth.onDemand', { ns: 'settings' })}`}
          </p>
          {target.auth_message && (
            <p className="text-xs text-low whitespace-pre-wrap break-words">
              {target.auth_message}
            </p>
          )}
          {target.refused && (
            <p className="text-xs text-error">
              {t('settings.reauth.refused', { ns: 'settings' })}
            </p>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {run?.outcome === 'running' && (
            <SpinnerIcon className="size-icon-sm animate-spin" />
          )}
          <Button
            size="sm"
            variant="secondary"
            disabled={disabled || target.auth_state === 'not_configured'}
            onClick={onRun}
          >
            {t('settings.reauth.run', { ns: 'settings' })}
          </Button>
        </div>
      </div>
      {run && (
        <ReauthRunSummary run={run} expanded={expanded} onToggle={onToggle} />
      )}
    </div>
  );
}

function ReauthRunSummary({
  run,
  expanded,
  onToggle,
}: {
  run: ReauthRun;
  expanded: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation(['settings']);
  const failed = run.outcome !== 'succeeded' && run.outcome !== 'running';
  const when = run.finished_at ?? run.started_at;

  return (
    <div className="space-y-1">
      <p className={`text-xs ${failed ? 'text-error' : 'text-low'}`}>
        {t(`settings.reauth.outcome.${run.outcome}`, { ns: 'settings' })}
        {' · '}
        {new Date(when).toLocaleString()}
        {' · '}
        {t(`settings.reauth.trigger.${run.trigger}`, { ns: 'settings' })}
      </p>
      {run.message && (
        <p
          className={`text-xs whitespace-pre-wrap break-words ${failed ? 'text-error' : 'text-low'}`}
        >
          {run.message}
        </p>
      )}
      {run.transcript.length > 0 && (
        <>
          <button
            type="button"
            className="text-xs text-low hover:text-normal underline"
            onClick={onToggle}
          >
            {expanded
              ? t('settings.reauth.hideSteps', { ns: 'settings' })
              : t('settings.reauth.showSteps', { ns: 'settings' })}
          </button>
          {expanded && (
            <pre className="text-xs text-low whitespace-pre-wrap break-words bg-secondary rounded-sm p-2 max-h-64 overflow-auto">
              {run.transcript.join('\n')}
            </pre>
          )}
        </>
      )}
    </div>
  );
}
