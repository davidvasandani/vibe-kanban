import { useMemo, useCallback, useState, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useDropzone } from 'react-dropzone';
import { useCreateMode } from '@/features/create-mode/model/useCreateMode';
import { AgentIcon } from '@/shared/components/AgentIcon';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { useIsRealMobile } from '@/shared/hooks/useIsMobile';
import WYSIWYGEditor from '@/shared/components/WYSIWYGEditor';
import { useCreateWorkspace } from '@/shared/hooks/useCreateWorkspace';
import { useCreateAttachments } from '@/shared/hooks/useCreateAttachments';
import { useExecutorConfig } from '@/shared/hooks/useExecutorConfig';
import { saveProjectRepoDefaults } from '@/shared/hooks/useProjectRepoDefaults';
import { getSortedExecutorVariantKeys } from '@/shared/lib/executor';
import {
  toPrettyCase,
  splitMessageToTitleDescription,
} from '@/shared/lib/string';
import {
  WorkerMountStatus,
  WorkerNodeStatus,
  type BaseCodingAgent,
  type Repo,
} from 'shared/types';
import { CreateChatBox } from '@vibe/ui/components/CreateChatBox';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
import { CreateModeRepoPickerBar } from './CreateModeRepoPickerBar';
import { ModelSelectorContainer } from '@/shared/components/ModelSelectorContainer';
import { workerNodesApi } from '@/shared/lib/api';
import {
  clusterAdvertisedProfiles,
  clusterSupportsExecutor,
} from '@/shared/lib/workerCapabilities';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@vibe/ui/components/Select';

function getRepoDisplayName(repo: Repo) {
  return repo.display_name || repo.name;
}

const BRANCH_LABEL_MAX_CHARS = 15;

function truncateBranchLabel(branch: string) {
  return branch.length > BRANCH_LABEL_MAX_CHARS
    ? `${branch.slice(0, BRANCH_LABEL_MAX_CHARS)}...`
    : branch;
}

interface CreateChatBoxContainerProps {
  onWorkspaceCreated: (workspaceId: string) => void;
}

export function CreateChatBoxContainer({
  onWorkspaceCreated,
}: CreateChatBoxContainerProps) {
  const { t } = useTranslation('common');
  const { profiles, config } = useUserSystem();
  const isMobile = useIsRealMobile();
  // On mobile keyboards there is no Shift+Enter, so force ModifierEnter mode
  // (plain Enter inserts a newline; user taps the Send button to send).
  const effectiveSendShortcut = isMobile
    ? 'ModifierEnter'
    : config?.send_message_shortcut;
  const {
    repos,
    targetBranches,
    message,
    setMessage,
    clearDraft,
    hasInitialValue,
    hasResolvedInitialRepoDefaults,
    linkedIssue,
    clearLinkedIssue,
    preferredExecutorConfig,
    executorConfig: draftConfig,
    setExecutorConfig: setDraftConfig,
    attachments: draftAttachments,
    setAttachments: setDraftAttachments,
  } = useCreateMode();

  const { createWorkspace } = useCreateWorkspace();
  const hasSelectedRepos = repos.length > 0;
  const [hasAttemptedSubmit, setHasAttemptedSubmit] = useState(false);
  const [hasInitializedStep, setHasInitializedStep] = useState(false);
  const [isSelectingRepos, setIsSelectingRepos] = useState(true);
  const [requestedWorkerNodeId, setRequestedWorkerNodeId] =
    useState('automatic');
  const { data: workerNodes = [] } = useQuery({
    queryKey: ['workerNodes'],
    queryFn: workerNodesApi.list,
    refetchInterval: 10_000,
  });

  useEffect(() => {
    if (!hasInitialValue || hasInitializedStep) return;
    if (!hasSelectedRepos && !hasResolvedInitialRepoDefaults) return;

    setIsSelectingRepos(!hasSelectedRepos);
    setHasInitializedStep(true);
  }, [
    hasInitialValue,
    hasInitializedStep,
    hasSelectedRepos,
    hasResolvedInitialRepoDefaults,
  ]);

  const showRepoPickerStep = !hasSelectedRepos || isSelectingRepos;
  const showChatStep = hasSelectedRepos && !isSelectingRepos;

  // Attachment handling - insert markdown and track attachment IDs
  const handleInsertMarkdown = useCallback(
    (markdown: string) => {
      const newMessage = message.trim()
        ? `${message}\n\n${markdown}`
        : markdown;
      setMessage(newMessage);
    },
    [message, setMessage]
  );

  const { uploadFiles, getAttachmentIds, clearAttachments, localAttachments } =
    useCreateAttachments(
      handleInsertMarkdown,
      draftAttachments,
      setDraftAttachments
    );

  const onDrop = useCallback(
    (acceptedFiles: File[]) => {
      if (acceptedFiles.length > 0) {
        uploadFiles(acceptedFiles);
      }
    },
    [uploadFiles]
  );

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    onDrop,
    disabled: createWorkspace.isPending || !hasSelectedRepos,
    noClick: true,
    noKeyboard: true,
  });

  const scratchConfig = useMemo(() => {
    if (!hasInitialValue) return undefined; // still loading
    return draftConfig ?? null;
  }, [hasInitialValue, draftConfig]);

  const {
    executorConfig,
    effectiveExecutor,
    selectedVariant,
    executorOptions,
    variantOptions,
    presetOptions,
    setOverrides: setExecutorOverrides,
  } = useExecutorConfig({
    profiles,
    lastUsedConfig: preferredExecutorConfig,
    scratchConfig,
    configExecutorProfile: config?.executor_profile,
    onPersist: (cfg) => setDraftConfig(cfg),
  });

  // Which agents the cluster can actually run. An affordance only — the
  // coordinator still enforces placement — so it degrades to "no constraint"
  // whenever the capability data cannot be read.
  const advertisedProfiles = useMemo(
    () => clusterAdvertisedProfiles(workerNodes),
    [workerNodes]
  );

  const unsupportedExecutors = useMemo(() => {
    if (advertisedProfiles === null) return undefined;
    const unsupported = new Map<BaseCodingAgent, string>();
    for (const option of executorOptions) {
      // The request is composed as `EXECUTOR:${variant ?? 'DEFAULT'}`, so the
      // variant is known for the current selection and only resolves for the
      // others once they are picked. Checking those against a guessed variant
      // would grey out agents the cluster can actually run.
      const requestedVariant =
        option === effectiveExecutor
          ? (selectedVariant ?? 'DEFAULT')
          : undefined;
      if (
        !clusterSupportsExecutor(advertisedProfiles, option, requestedVariant)
      ) {
        unsupported.set(option, t('createMode.worker.executorUnavailable'));
      }
    }
    return unsupported.size > 0 ? unsupported : undefined;
  }, [
    advertisedProfiles,
    executorOptions,
    effectiveExecutor,
    selectedVariant,
    t,
  ]);

  // Shown beside the picker when the user's *current* agent is unavailable.
  // The selection is deliberately left alone: silently switching it would
  // persist a different default than the user chose.
  const selectedExecutorUnavailable =
    effectiveExecutor !== null &&
    unsupportedExecutors?.has(effectiveExecutor) === true;

  const repoId = repos.length === 1 ? repos[0]?.id : undefined;
  const repoSummaryLabel = useMemo(() => {
    if (repos.length === 1) {
      const repo = repos[0];
      if (!repo) return '0 repositories selected';
      const selectedBranch = targetBranches[repo.id];
      const branch = selectedBranch
        ? truncateBranchLabel(selectedBranch)
        : 'Select branch';
      return `${getRepoDisplayName(repo)} · ${branch}`;
    }

    return `${repos.length} repositories selected`;
  }, [repos, targetBranches]);

  const repoSummaryTitle = useMemo(
    () =>
      repos
        .map((repo) => {
          const branch = targetBranches[repo.id] ?? 'Select branch';
          return `${getRepoDisplayName(repo)} (${branch})`;
        })
        .join('\n'),
    [repos, targetBranches]
  );

  const hasSelectedBranchesForAllRepos = repos.every(
    (repo) => !!targetBranches[repo.id]
  );

  // Determine if we can submit
  const canSubmit =
    hasSelectedRepos &&
    hasSelectedBranchesForAllRepos &&
    message.trim().length > 0 &&
    effectiveExecutor !== null;

  const handlePresetSelect = (presetId: string | null) => {
    if (!effectiveExecutor) return;
    setDraftConfig({
      ...draftConfig,
      executor: effectiveExecutor,
      variant: presetId,
    });
  };

  const handleCustomise = () => {
    SettingsDialog.show({ initialSection: 'agents' });
  };

  // Handle executor change - use saved variant if switching to default executor
  const handleExecutorChange = useCallback(
    (executor: BaseCodingAgent) => {
      const executorProfile = profiles?.[executor];
      if (!executorProfile) {
        setDraftConfig({ executor, variant: null });
        return;
      }

      const variants = getSortedExecutorVariantKeys(executorProfile);
      let targetVariant: string | null = null;

      // If switching to user's default executor, use their saved variant
      if (
        config?.executor_profile?.executor === executor &&
        config?.executor_profile?.variant
      ) {
        const savedVariant = config.executor_profile.variant;
        if (variants.includes(savedVariant)) {
          targetVariant = savedVariant;
        }
      }

      // Fallback to DEFAULT or first available
      if (!targetVariant) {
        targetVariant = variants.includes('DEFAULT')
          ? 'DEFAULT'
          : (variants[0] ?? null);
      }

      setDraftConfig({ executor, variant: targetVariant });
    },
    [profiles, setDraftConfig, config?.executor_profile]
  );

  // Handle submit
  const handleSubmit = useCallback(async () => {
    setHasAttemptedSubmit(true);
    if (!canSubmit || !executorConfig) return;

    const { title } = splitMessageToTitleDescription(message);
    const data = {
      executor_config: executorConfig,
      name: title,
      prompt: message,
      repos: repos.map((r) => ({
        repo_id: r.id,
        target_branch: targetBranches[r.id]!,
      })),
      linked_issue: linkedIssue
        ? {
            remote_project_id: linkedIssue.remoteProjectId,
            issue_id: linkedIssue.issueId,
          }
        : null,
      attachment_ids: getAttachmentIds(),
      requested_worker_node_id:
        requestedWorkerNodeId === 'automatic' ? null : requestedWorkerNodeId,
    };
    const linkToIssue = linkedIssue
      ? {
          remoteProjectId: linkedIssue.remoteProjectId,
          issueId: linkedIssue.issueId,
        }
      : undefined;

    const result = await createWorkspace.mutateAsync({
      data,
      linkToIssue,
    });

    if (result.workspace) {
      onWorkspaceCreated(result.workspace.id);
    }

    if (linkedIssue?.remoteProjectId) {
      saveProjectRepoDefaults(linkedIssue.remoteProjectId, data.repos).catch(
        (err) => console.warn('Failed to save project repo defaults:', err)
      );
    }

    clearAttachments();
    await clearDraft();
  }, [
    canSubmit,
    executorConfig,
    message,
    repos,
    targetBranches,
    createWorkspace,
    onWorkspaceCreated,
    getAttachmentIds,
    clearAttachments,
    clearDraft,
    linkedIssue,
    requestedWorkerNodeId,
  ]);

  // Determine error to display
  const displayError =
    hasAttemptedSubmit && repos.length === 0
      ? 'Add at least one repository to create a workspace'
      : hasAttemptedSubmit && !hasSelectedBranchesForAllRepos
        ? 'Select a branch for every repository before creating a workspace'
        : createWorkspace.error
          ? createWorkspace.error instanceof Error
            ? createWorkspace.error.message
            : 'Failed to create workspace'
          : null;

  // Wait for initial value to be applied before rendering
  // This ensures the editor mounts with content ready, so autoFocus works correctly
  if (!hasInitialValue) {
    return null;
  }

  return (
    <div className="relative flex flex-1 flex-col bg-primary h-full">
      <div className="flex flex-1 items-center justify-center px-base">
        <div className="flex w-chat max-w-full flex-col gap-base">
          {showRepoPickerStep && (
            <>
              <h2 className="mb-double text-center text-4xl font-medium tracking-tight text-high">
                {t('createMode.headings.repoStep')}
              </h2>
              <CreateModeRepoPickerBar
                onContinueToPrompt={() => setIsSelectingRepos(false)}
              />
            </>
          )}

          {showChatStep && (
            <>
              <h2 className="mb-double text-center text-4xl font-medium tracking-tight text-high">
                {t('createMode.headings.chatStep')}
              </h2>

              <div className="flex justify-center @container">
                <div className="flex w-full flex-col gap-half">
                  {workerNodes.length > 0 && (
                    <div className="flex items-center justify-end gap-half text-xs text-low">
                      <span>{t('createMode.worker.label')}</span>
                      <Select
                        value={requestedWorkerNodeId}
                        onValueChange={setRequestedWorkerNodeId}
                      >
                        <SelectTrigger className="h-8 w-48">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="automatic">
                            {t('createMode.worker.automatic')}
                          </SelectItem>
                          {workerNodes.map((worker) => {
                            const healthy =
                              worker.status === WorkerNodeStatus.online &&
                              worker.mount_status === WorkerMountStatus.healthy;
                            // A healthy worker that cannot run the selected
                            // agent is just as unpickable as an offline one,
                            // and saying which is which saves a round trip.
                            const runsAgent =
                              effectiveExecutor === null ||
                              clusterSupportsExecutor(
                                clusterAdvertisedProfiles([worker]),
                                effectiveExecutor,
                                selectedVariant ?? 'DEFAULT'
                              );
                            return (
                              <SelectItem
                                key={worker.id}
                                value={worker.id}
                                disabled={!healthy || !runsAgent}
                              >
                                {worker.hostname}
                                {healthy && !runsAgent
                                  ? ` — ${t(
                                      'createMode.worker.workerCannotRunAgent',
                                      { agent: effectiveExecutor }
                                    )}`
                                  : ''}
                              </SelectItem>
                            );
                          })}
                        </SelectContent>
                      </Select>
                    </div>
                  )}
                  {selectedExecutorUnavailable && (
                    <div
                      role="status"
                      className="flex justify-end text-xs text-error"
                    >
                      {t('createMode.worker.executorUnavailableNotice', {
                        agent: effectiveExecutor,
                      })}
                    </div>
                  )}
                  <CreateChatBox
                    editor={{
                      value: message,
                      onChange: setMessage,
                    }}
                    renderEditor={({
                      value,
                      onChange,
                      onCmdEnter,
                      disabled,
                      repoIds,
                      repoId,
                      executor,
                      onPasteFiles,
                      localAttachments,
                    }) => (
                      <WYSIWYGEditor
                        placeholder="Describe the task..."
                        value={value}
                        onChange={onChange}
                        onCmdEnter={onCmdEnter}
                        disabled={disabled}
                        className="min-h-double max-h-[50vh] overflow-y-auto"
                        repoIds={repoIds}
                        repoId={repoId}
                        executor={executor}
                        autoFocus
                        onPasteFiles={onPasteFiles}
                        localAttachments={localAttachments}
                        sendShortcut={effectiveSendShortcut}
                      />
                    )}
                    agentIcon={
                      <AgentIcon
                        agent={effectiveExecutor}
                        className="size-icon-xl"
                      />
                    }
                    onSend={handleSubmit}
                    isSending={createWorkspace.isPending}
                    disabled={!hasSelectedRepos}
                    executor={{
                      selected: effectiveExecutor,
                      options: executorOptions,
                      onChange: handleExecutorChange,
                      unsupported: unsupportedExecutors,
                    }}
                    formatExecutorLabel={toPrettyCase}
                    error={displayError}
                    repoIds={repos.map((r) => r.id)}
                    repoId={repoId}
                    modelSelector={
                      effectiveExecutor ? (
                        <ModelSelectorContainer
                          agent={effectiveExecutor}
                          workspaceId={undefined}
                          onAdvancedSettings={handleCustomise}
                          presets={variantOptions}
                          selectedPreset={selectedVariant}
                          onPresetSelect={handlePresetSelect}
                          onOverrideChange={setExecutorOverrides}
                          executorConfig={executorConfig}
                          presetOptions={presetOptions}
                        />
                      ) : undefined
                    }
                    onPasteFiles={uploadFiles}
                    localAttachments={localAttachments}
                    dropzone={{ getRootProps, getInputProps, isDragActive }}
                    onEditRepos={() => setIsSelectingRepos(true)}
                    repoSummaryLabel={repoSummaryLabel}
                    repoSummaryTitle={repoSummaryTitle}
                    linkedIssue={
                      linkedIssue?.simpleId
                        ? {
                            simpleId: linkedIssue.simpleId,
                            title: linkedIssue.title ?? '',
                            onRemove: clearLinkedIssue,
                          }
                        : null
                    }
                  />
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
