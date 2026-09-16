import { useTranslation } from 'react-i18next';
import type { Workspace, WorkspaceCreationPhase } from 'shared/types';
import { useWorkspaceCreationProgress } from '@/shared/hooks/useWorkspaceCreationProgress';

const phases = [
  'repositories',
  'context',
  'placement',
  'worktrees',
  'execution',
  'finalizing',
] as const satisfies readonly WorkspaceCreationPhase[];

export function WorkspaceCreationStatusView({
  workspace,
}: {
  workspace: Workspace;
}) {
  const { t } = useTranslation('common');
  const { data, isError } = useWorkspaceCreationProgress(workspace);
  // Never borrow phase evidence from another workspace, including during navigation.
  const progress = data?.workspace_id === workspace.id ? data : undefined;
  const failed =
    workspace.creation_status === 'failed' || progress?.status === 'failed';
  const finished = !failed && progress?.status === 'ready';
  const phaseIndex = phases.findIndex((phase) => phase === progress?.phase);
  const queued =
    workspace.creation_status === 'queued' &&
    (!progress || progress.status === 'queued');
  const active = !failed && !finished && !isError && !queued;

  if (workspace.creation_status === 'ready') return null;

  return (
    <div className="flex h-full min-h-0 overflow-y-auto p-6">
      <div className="m-auto w-full max-w-md space-y-4">
        <div role={failed ? 'alert' : 'status'} aria-live="polite">
          <h2 className="text-lg font-medium">
            {t(
              failed
                ? 'workspaceCreation.failedTitle'
                : 'workspaceCreation.creatingTitle'
            )}
          </h2>
          <p className="mt-2 text-sm text-muted-foreground">
            {failed
              ? (workspace.creation_error ??
                t('workspaceCreation.failedFallback'))
              : t('workspaceCreation.creatingBody')}
          </p>
          <p className="mt-3 text-sm">
            {failed
              ? t('workspaceCreation.stopped')
              : finished
                ? t('workspaceCreation.opening')
                : isError
                  ? t('workspaceCreation.unavailable')
                  : queued
                    ? t('workspaceCreation.queued')
                    : phaseIndex < 0
                      ? t('workspaceCreation.loading')
                      : t('workspaceCreation.backgroundTask', {
                          task: t(
                            `workspaceCreation.tasks.${phases[phaseIndex]}`
                          ),
                        })}
          </p>
        </div>
        <ol
          className="space-y-3"
          aria-label={t('workspaceCreation.stepsLabel')}
        >
          {phases.map((phase, index) => {
            const state =
              finished || index < phaseIndex
                ? 'complete'
                : index === phaseIndex
                  ? failed
                    ? 'failed'
                    : active
                      ? 'running'
                      : 'lastReported'
                  : phaseIndex < 0 && !queued
                    ? 'unknown'
                    : 'pending';
            return (
              <li key={phase} className="flex items-start gap-3 text-sm">
                <span
                  aria-hidden="true"
                  className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-xs ${
                    state === 'running'
                      ? 'animate-pulse border-current'
                      : 'border-muted-foreground/30'
                  }`}
                >
                  {state === 'complete'
                    ? '✓'
                    : state === 'failed'
                      ? '!'
                      : index + 1}
                </span>
                <span className="min-w-0 flex-1 break-words">
                  {t(`workspaceCreation.steps.${phase}`)}
                </span>
                <span className="max-w-[40%] shrink-0 text-right text-muted-foreground">
                  {t(`workspaceCreation.states.${state}`)}
                </span>
              </li>
            );
          })}
        </ol>
        {progress?.updated_at && (
          <p className="text-xs text-muted-foreground">
            {t('workspaceCreation.lastReported', {
              time: new Date(progress.updated_at).toLocaleTimeString(),
            })}
          </p>
        )}
      </div>
    </div>
  );
}
