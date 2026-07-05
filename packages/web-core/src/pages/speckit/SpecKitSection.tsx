import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CaretDownIcon,
  CaretRightIcon,
  CheckCircleIcon,
  CircleIcon,
  FloppyDiskIcon,
} from '@phosphor-icons/react';
import type {
  SpecKitArtifact,
  SpecKitStage,
  SpecKitTaskStatus,
  SpecKitTasks,
} from 'shared/types';
import {
  useSpecKitArtifacts,
  useSpecKitStatus,
  useToggleSpecKitTask,
  useUpdateSpecKitArtifact,
} from '@/shared/hooks/useSpecKit';

const STAGE_ORDER: SpecKitStage[] = [
  'constitution',
  'specify',
  'clarify',
  'plan',
  'tasks',
  'analyze',
  'implement',
];

/**
 * SpecKit workbench section for an existing task: a read/edit viewer over the
 * artifacts the SpecKit pipeline's execution agent writes into the task's
 * workspace (`<host>/specs/<feature-key>/`). Renders nothing (border
 * included) when the workspace is not a SpecKit workspace; the backend
 * decides applicability (`status.enabled`).
 */
export function SpecKitSection({ workspaceId }: { workspaceId: string }) {
  const { data: status } = useSpecKitStatus(workspaceId);

  if (!status?.enabled) {
    return null;
  }
  return <SpecKitBody workspaceId={workspaceId} status={status} />;
}

function SpecKitBody({
  workspaceId,
  status,
}: {
  workspaceId: string;
  status: SpecKitTaskStatus;
}) {
  const { t } = useTranslation('common');
  const [expanded, setExpanded] = useState(false);
  const { data: artifacts } = useSpecKitArtifacts(workspaceId, expanded);

  const doneStages = useMemo(
    () => new Set(status.stages.filter((s) => s.exists).map((s) => s.stage)),
    [status.stages]
  );

  const editableArtifacts = useMemo(() => {
    if (!artifacts) return [];
    return [artifacts.spec, artifacts.plan, artifacts.tasks].filter(
      (a): a is SpecKitArtifact => !!a && a.exists
    );
  }, [artifacts]);

  return (
    <div className="border-t px-4 py-3">
      <button
        type="button"
        className="flex w-full items-center gap-2 text-left"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        {expanded ? (
          <CaretDownIcon className="size-icon-xs text-low-contrast" />
        ) : (
          <CaretRightIcon className="size-icon-xs text-low-contrast" />
        )}
        <span className="text-sm font-medium">{t('speckit.title')}</span>
        <span className="text-xs text-low-contrast">
          {t('speckit.stageProgress', {
            done: doneStages.size,
            total: STAGE_ORDER.length,
          })}
        </span>
        {status.multi_repo && status.host_rel && (
          <span className="ml-auto rounded bg-secondary px-1.5 py-0.5 text-xs text-low-contrast">
            {status.host_rel}
          </span>
        )}
      </button>

      {expanded && (
        <div className="mt-3 space-y-4">
          <StageRail doneStages={doneStages} />
          {status.feature_dir && (
            <p className="text-xs text-low-contrast">{status.feature_dir}</p>
          )}
          {status.tasks && (
            <SpecKitTaskList workspaceId={workspaceId} tasks={status.tasks} />
          )}
          {editableArtifacts.length > 0 && (
            <ArtifactEditor
              workspaceId={workspaceId}
              artifacts={editableArtifacts}
            />
          )}
        </div>
      )}
    </div>
  );
}

function StageRail({ doneStages }: { doneStages: Set<SpecKitStage> }) {
  const { t } = useTranslation('common');
  return (
    <ol className="flex flex-wrap items-center gap-x-3 gap-y-1">
      {STAGE_ORDER.map((stage) => {
        const done = doneStages.has(stage);
        return (
          <li key={stage} className="flex items-center gap-1 text-xs">
            {done ? (
              <CheckCircleIcon
                className="size-icon-xs text-success"
                weight="fill"
              />
            ) : (
              <CircleIcon className="size-icon-xs text-low-contrast" />
            )}
            <span className={done ? '' : 'text-low-contrast'}>
              {t(`speckit.stages.${stage}`)}
            </span>
          </li>
        );
      })}
    </ol>
  );
}

function SpecKitTaskList({
  workspaceId,
  tasks,
}: {
  workspaceId: string;
  tasks: SpecKitTasks;
}) {
  const { t } = useTranslation('common');
  const toggle = useToggleSpecKitTask(workspaceId);

  return (
    <div>
      <p className="mb-1 text-xs font-medium text-low-contrast">
        {t('speckit.tasksProgress', {
          done: tasks.completed,
          total: tasks.total,
        })}
      </p>
      <ul className="space-y-0.5">
        {tasks.tasks.map((task) => (
          <li key={task.id} className="flex items-start gap-2 text-sm">
            <input
              type="checkbox"
              className="mt-1"
              checked={task.done}
              disabled={toggle.isPending}
              onChange={(e) =>
                toggle.mutate({ task_id: task.id, done: e.target.checked })
              }
              aria-label={task.id}
            />
            <span className={task.done ? 'text-low-contrast line-through' : ''}>
              <span className="mr-1 font-mono text-xs text-low-contrast">
                {task.id}
              </span>
              {task.parallelizable && (
                <span className="mr-1 rounded bg-secondary px-1 text-xs">
                  {t('speckit.parallelMarker')}
                </span>
              )}
              {task.description}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function ArtifactEditor({
  workspaceId,
  artifacts,
}: {
  workspaceId: string;
  artifacts: SpecKitArtifact[];
}) {
  const { t } = useTranslation('common');
  const [selectedPath, setSelectedPath] = useState(artifacts[0].relative_path);
  const selected =
    artifacts.find((a) => a.relative_path === selectedPath) ?? artifacts[0];
  const [draft, setDraft] = useState(selected.content ?? '');
  const [dirty, setDirty] = useState(false);
  const update = useUpdateSpecKitArtifact(workspaceId);

  // Re-seed the editor whenever the operator switches artifact or fresh
  // content arrives while the draft is untouched.
  useEffect(() => {
    if (!dirty) {
      setDraft(selected.content ?? '');
    }
  }, [selected.relative_path, selected.content, dirty]);

  return (
    <div>
      <div className="mb-1 flex items-center gap-2">
        {artifacts.map((artifact) => (
          <button
            key={artifact.relative_path}
            type="button"
            className={`text-xs ${
              artifact.relative_path === selected.relative_path
                ? 'font-medium underline'
                : 'text-low-contrast'
            }`}
            onClick={() => {
              setSelectedPath(artifact.relative_path);
              setDirty(false);
            }}
          >
            {artifact.name}
          </button>
        ))}
        <button
          type="button"
          className="ml-auto flex items-center gap-1 text-xs text-low-contrast disabled:opacity-50"
          disabled={!dirty || update.isPending}
          onClick={() =>
            update.mutate(
              { relative_path: selected.relative_path, content: draft },
              { onSuccess: () => setDirty(false) }
            )
          }
        >
          <FloppyDiskIcon className="size-icon-xs" />
          {update.isPending ? t('speckit.saving') : t('speckit.save')}
        </button>
      </div>
      <textarea
        className="h-48 w-full resize-y rounded border bg-background p-2 font-mono text-xs"
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          setDirty(true);
        }}
        spellCheck={false}
      />
      {update.isError && (
        <p className="mt-1 text-xs text-destructive">
          {t('speckit.saveFailed')}
        </p>
      )}
    </div>
  );
}
