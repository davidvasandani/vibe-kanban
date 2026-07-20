import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowSquareOutIcon,
  CheckCircleIcon,
  CheckIcon,
  CircleNotchIcon,
  CodeIcon,
  CopyIcon,
  LockKeyIcon,
  MinusCircleIcon,
  PencilSimpleIcon,
  PlusIcon,
  TrashIcon,
  XCircleIcon,
} from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import type {
  BaseCodingAgent,
  JsonValue,
  McpAuthStatusResponse,
  McpServerDefinition,
  McpServerTestResult,
  SharedMcpAssignmentTestResult,
  SharedMcpReadResponse,
  SharedMcpServer,
} from 'shared/types';
import { BaseCodingAgent as BaseCodingAgentValue } from 'shared/types';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useProjectContextOptional } from '@/shared/hooks/useProjectContext';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { codecForAgent, transportOf } from '@/shared/lib/mcpServerCodec';
import {
  acquireMcpDebugCreation,
  buildMcpDebugIssueRequest,
  mcpDebugAvailability,
  mcpDebugCreationKey,
  mcpDiagnosticText,
  resettableMcpDebugKeys,
  releaseMcpDebugCreation,
} from '@/shared/lib/mcpDebugIssue';
import {
  definitionFromEntry,
  draftFromSharedRead,
  indexAssignmentTests,
  inputsFromDraft,
  mergeOAuthRefresh,
  preconfiguredMcpServers,
  removedServerNames,
  resolveConflictVariant,
  sharedMcpSnapshot,
  testKey,
  testTargetsForDraft,
  type SharedMcpDraftServer,
  type SharedMcpDraftState,
} from '@/shared/lib/sharedMcpSettingsState';
import { cn } from '@/shared/lib/utils';
import { toPrettyCase } from '@/shared/lib/string';
import {
  SettingsCard,
  SettingsField,
  SettingsSaveBar,
  SettingsTextarea,
} from './SettingsComponents';
import { McpServerDialog } from './McpServerDialog';
import { useSettingsDirty } from './SettingsDirtyContext';
import { useSettingsMachineClient } from './SettingsHostContext';

type CopyStatus = 'idle' | 'success' | 'error';
type DebugStatus = 'idle' | 'creating' | 'success' | 'error';

type McpCopyState = {
  status: CopyStatus;
  error?: string;
};

type McpDebugState = {
  status: DebugStatus;
  issueId?: string;
  error?: string;
};

function entryForDialog(definition: McpServerDefinition): JsonValue {
  if (definition.transport === 'http' || definition.transport === 'sse') {
    return {
      type: definition.transport,
      ...(definition.value as Record<string, JsonValue>),
    };
  }
  return definition.value;
}

function transportBadge(definition: McpServerDefinition): string {
  const codec = codecForAgent(BaseCodingAgentValue.CLAUDE_CODE);
  const transport = transportOf(codec, entryForDialog(definition));
  if (transport === null) return 'custom';
  return transport === 'stdio' ? 'stdio' : transport.toUpperCase();
}

function McpTestStatusIcon({
  result,
}: {
  result: McpServerTestResult | undefined;
}) {
  if (!result) return null;
  const Icon =
    result.status === 'ok'
      ? CheckCircleIcon
      : result.status === 'auth_required'
        ? LockKeyIcon
        : result.status === 'unsupported'
          ? MinusCircleIcon
          : XCircleIcon;
  const color =
    result.status === 'ok'
      ? 'text-success'
      : result.status === 'auth_required'
        ? 'text-warning'
        : result.status === 'unsupported'
          ? 'text-low'
          : 'text-error';
  return (
    <span
      className="inline-flex items-center"
      title={result.error ?? result.status}
    >
      <Icon className={cn('size-icon-sm', color)} weight="fill" />
    </span>
  );
}

function TestResultDetails({
  result,
  diagnostic,
  executor,
  connecting,
  connectError,
  onConnect,
  loopback,
  onToggleLoopback,
  manualActive,
  manualCode,
  onManualCodeChange,
  onManualComplete,
  completing,
  copyState,
  onCopy,
  debugState,
  debugUnavailableReason,
  onCreateDebugIssue,
  onOpenDebugIssue,
}: {
  result: McpServerTestResult | undefined;
  diagnostic: string;
  executor: BaseCodingAgent;
  connecting: boolean;
  connectError: string | undefined;
  onConnect: () => void;
  loopback: boolean;
  onToggleLoopback: () => void;
  manualActive: boolean;
  manualCode: string;
  onManualCodeChange: (value: string) => void;
  onManualComplete: () => void;
  completing: boolean;
  copyState: McpCopyState | undefined;
  onCopy: () => void;
  debugState: McpDebugState | undefined;
  debugUnavailableReason: 'no-project' | 'no-status' | null;
  onCreateDebugIssue: () => void;
  onOpenDebugIssue: () => void;
}) {
  const { t } = useTranslation('settings');
  if (!result || result.status === 'ok') return null;
  const authRequired = result.status === 'auth_required';
  const failed = result.status === 'failed';
  const copyStatus = copyState?.status ?? 'idle';
  const debugStatus = debugState?.status ?? 'idle';

  if (failed) {
    const debugUnavailable =
      debugUnavailableReason === 'no-project'
        ? t('settings.mcp.test.debugUnavailableProject')
        : debugUnavailableReason === 'no-status'
          ? t('settings.mcp.test.debugUnavailableStatus')
          : null;
    return (
      <div className="mt-2 rounded-sm border border-error/50 bg-error/10 px-2 py-1.5 text-xs text-error">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0 font-medium">
            {t('settings.mcp.test.failedFor', {
              executor: toPrettyCase(executor),
            })}
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-1">
            <Button variant="outline" size="sm" type="button" onClick={onCopy}>
              <CopyIcon className="size-icon-xs mr-1" weight="bold" />
              {t('settings.mcp.test.copyDiagnostic')}
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              onClick={onCreateDebugIssue}
              disabled={
                debugStatus === 'creating' || debugUnavailableReason !== null
              }
              title={debugUnavailable ?? undefined}
            >
              {debugStatus === 'creating' ? (
                <CircleNotchIcon
                  className="size-icon-xs mr-1 animate-spin"
                  weight="bold"
                />
              ) : (
                <XCircleIcon className="size-icon-xs mr-1" weight="bold" />
              )}
              {debugStatus === 'creating'
                ? t('settings.mcp.test.debugCreating')
                : t('settings.mcp.test.debugIssue')}
            </Button>
          </div>
        </div>
        <pre className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-sm border border-current/20 bg-primary/80 p-2 font-mono text-xs text-high">
          {diagnostic}
        </pre>
        <div className="mt-2 space-y-1" aria-live="polite">
          {copyStatus === 'success' && (
            <div className="text-success">
              {t('settings.mcp.test.copySuccess')}
            </div>
          )}
          {copyStatus === 'error' && (
            <div className="text-error">
              {t('settings.mcp.test.copyFailure', {
                error: copyState?.error,
              })}
            </div>
          )}
          {debugUnavailable && (
            <div className="text-low">{debugUnavailable}</div>
          )}
          {debugStatus === 'error' && (
            <div className="text-error">
              {t('settings.mcp.test.debugFailure', {
                error: debugState?.error,
              })}
            </div>
          )}
          {debugStatus === 'success' && debugState?.issueId && (
            <div className="flex flex-wrap items-center gap-2 text-success">
              <span>{t('settings.mcp.test.issueCreated')}</span>
              <Button
                variant="outline"
                size="sm"
                type="button"
                onClick={onOpenDebugIssue}
              >
                <ArrowSquareOutIcon
                  className="size-icon-xs mr-1"
                  weight="bold"
                />
                {t('settings.mcp.test.openIssue')}
              </Button>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        'mt-2 rounded-sm border px-2 py-1.5 text-xs',
        authRequired
          ? 'border-warning/50 bg-warning/10 text-warning'
          : 'border-error/50 bg-error/10 text-error'
      )}
    >
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1 truncate" title={result.error ?? ''}>
          <span className="font-medium">
            {authRequired ? t('settings.mcp.test.authRequired') : result.error}
          </span>
          {authRequired && result.error && (
            <span className="ml-2 font-mono text-low">{result.error}</span>
          )}
        </div>
        {authRequired && (
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={onConnect}
            disabled={connecting}
          >
            {connecting ? (
              <CircleNotchIcon
                className="size-icon-xs mr-1 animate-spin"
                weight="bold"
              />
            ) : (
              <LockKeyIcon className="size-icon-xs mr-1" weight="bold" />
            )}
            {connecting
              ? t('settings.mcp.test.connecting')
              : t('settings.mcp.test.connect')}
          </Button>
        )}
      </div>
      {connectError && (
        <div className="mt-1 line-clamp-2 break-words text-error">
          {connectError}
        </div>
      )}
      {authRequired && (
        <details className="mt-1 text-low">
          <summary className="cursor-pointer select-none">
            {t('settings.mcp.test.connectionOptions')}
          </summary>
          <label className="mt-1 flex items-center gap-2 pl-3">
            <input
              type="checkbox"
              checked={loopback}
              onChange={onToggleLoopback}
              disabled={connecting || manualActive}
            />
            {t('settings.mcp.test.useLocalhostCallback')}
          </label>
        </details>
      )}
      {manualActive && (
        <div className="mt-2 space-y-2 border-t border-current/20 pt-2">
          <p className="text-low">{t('settings.mcp.test.manualHint')}</p>
          <input
            type="text"
            value={manualCode}
            onChange={(event) => onManualCodeChange(event.target.value)}
            placeholder={t('settings.mcp.test.manualPlaceholder')}
            className="w-full rounded-sm border border-border bg-primary px-2 py-1 font-mono text-high"
          />
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={onManualComplete}
            disabled={completing || manualCode.trim().length === 0}
          >
            {completing && (
              <CircleNotchIcon
                className="size-icon-xs mr-1 animate-spin"
                weight="bold"
              />
            )}
            {t('settings.mcp.test.finishConnect')}
          </Button>
        </div>
      )}
    </div>
  );
}

export function McpSettingsSection() {
  const { t } = useTranslation('settings');
  const { config } = useUserSystem();
  const machineClient = useSettingsMachineClient();
  const { setDirty: setContextDirty } = useSettingsDirty();
  const projectContext = useProjectContextOptional();
  const appNavigation = useAppNavigation();

  const [readModel, setReadModel] = useState<SharedMcpReadResponse | null>(
    null
  );
  const [draft, setDraft] = useState<SharedMcpDraftState>({
    servers: [],
    conflicts: [],
  });
  const [originalSnapshot, setOriginalSnapshot] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [jsonMode, setJsonMode] = useState(false);
  const [jsonText, setJsonText] = useState('');
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResults, setTestResults] = useState<
    Record<string, SharedMcpAssignmentTestResult>
  >({});
  const [copyStates, setCopyStates] = useState<Record<string, McpCopyState>>(
    {}
  );
  const [debugStates, setDebugStates] = useState<Record<string, McpDebugState>>(
    {}
  );
  const creatingDebugKeysRef = useRef(new Set<string>());
  const [connectingKey, setConnectingKey] = useState<string | null>(null);
  const [connectErrors, setConnectErrors] = useState<Record<string, string>>(
    {}
  );
  const [loopbackEnabled, setLoopbackEnabled] = useState<
    Record<string, boolean>
  >({});
  const [manualFlow, setManualFlow] = useState<{
    key: string;
    serverName: string;
    executor: BaseCodingAgent;
    flowId: string;
  } | null>(null);
  const [manualCode, setManualCode] = useState('');
  const [completing, setCompleting] = useState(false);

  const snapshot = useMemo(() => sharedMcpSnapshot(draft), [draft]);
  const isDirty = snapshot !== originalSnapshot;

  useEffect(() => {
    setContextDirty('mcp', isDirty);
    return () => setContextDirty('mcp', false);
  }, [isDirty, setContextDirty]);

  const loadShared = useCallback(async () => {
    if (!machineClient) return;
    setLoading(true);
    setError(null);
    try {
      const response = await machineClient.loadSharedMcpServers();
      const next = draftFromSharedRead(response);
      setReadModel(response);
      setDraft(next);
      setOriginalSnapshot(sharedMcpSnapshot(next));
      setJsonText(JSON.stringify(inputsFromDraft(next), null, 2));
      setTestResults({});
      setCopyStates({});
      setDebugStates({});
      setConnectErrors({});
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('settings.mcp.errors.loadFailed')
      );
    } finally {
      setLoading(false);
    }
  }, [machineClient, t]);

  useEffect(() => {
    void loadShared();
  }, [loadShared]);

  const profiles =
    readModel?.profiles.filter((profile) => profile.supports_mcp) ?? [];
  const serverByName = useMemo(() => {
    const map = new Map<string, SharedMcpServer>();
    for (const server of readModel?.servers ?? []) map.set(server.name, server);
    return map;
  }, [readModel]);
  const catalogServers = useMemo(
    () => preconfiguredMcpServers(readModel?.preconfigured ?? {}),
    [readModel]
  );

  const save = useCallback(async () => {
    if (!machineClient) return;
    setSaving(true);
    setError(null);
    try {
      const response = await machineClient.saveSharedMcpServers({
        servers: inputsFromDraft(draft),
        removed_servers: removedServerNames(readModel, draft),
        resolved_conflicts: (readModel?.conflicts ?? [])
          .filter(
            (conflict) =>
              !draft.conflicts.some((item) => item.name === conflict.name)
          )
          .map((conflict) => ({ name: conflict.name })),
      });
      const fresh = await machineClient.loadSharedMcpServers();
      const next = draftFromSharedRead(fresh);
      setReadModel(fresh);
      setDraft(next);
      setOriginalSnapshot(sharedMcpSnapshot(next));
      setTestResults({});
      setCopyStates({});
      setDebugStates({});
      setConnectErrors({});
      setSuccess(response.status === 'success');
      if (response.status !== 'success') {
        setError(
          response.outcomes
            .filter((outcome) => outcome.status === 'failed')
            .map((outcome) => `${outcome.executor}: ${outcome.error}`)
            .join('\n') || t('settings.mcp.errors.saveFailed')
        );
      }
      setTimeout(() => setSuccess(false), 3000);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('settings.mcp.errors.saveFailed')
      );
    } finally {
      setSaving(false);
    }
  }, [draft, machineClient, readModel, t]);

  const discard = useCallback(() => {
    if (!readModel) return;
    const next = draftFromSharedRead(readModel);
    setDraft(next);
    setJsonText(JSON.stringify(inputsFromDraft(next), null, 2));
    setJsonError(null);
    setError(null);
  }, [readModel]);

  const setServer = useCallback((server: SharedMcpDraftServer) => {
    setDraft((prev) => ({
      ...prev,
      servers: [
        ...prev.servers.filter((item) => item.name !== server.name),
        server,
      ].sort((a, b) => a.name.localeCompare(b.name)),
    }));
  }, []);

  const resolveConflict = useCallback(
    (conflictName: string, variantId: string) => {
      setDraft((prev) => {
        const conflict = prev.conflicts.find(
          (item) => item.name === conflictName
        );
        const variant = conflict?.variants.find(
          (item) => item.variant_id === variantId
        );
        return conflict && variant
          ? resolveConflictVariant(prev, conflict, variant)
          : prev;
      });
    },
    []
  );

  const openDialog = useCallback(
    async (server?: SharedMcpDraftServer) => {
      const codec = codecForAgent(BaseCodingAgentValue.CLAUDE_CODE);
      const result = await McpServerDialog.show({
        codec,
        profiles,
        existingNames: draft.servers
          .map((s) => s.name)
          .filter((n) => n !== server?.name),
        initial: server
          ? {
              name: server.name,
              entry: entryForDialog(server.definition),
              assignments: server.assignments,
            }
          : undefined,
      });
      if (!result) return;
      if (server && server.name !== result.name) {
        setDraft((prev) => ({
          ...prev,
          servers: prev.servers.filter((s) => s.name !== server.name),
        }));
      }
      setServer({
        name: result.name,
        definition: definitionFromEntry(result.entry),
        assignments: result.assignments,
      });
    },
    [draft.servers, profiles, setServer]
  );

  const removeServer = useCallback((name: string) => {
    setDraft((prev) => ({
      ...prev,
      servers: prev.servers.filter((server) => server.name !== name),
    }));
  }, []);

  const addPreconfigured = useCallback(
    (key: string, entry: JsonValue) => {
      const definition = definitionFromEntry(entry);
      const assignments = profiles
        .filter(
          (profile) =>
            !(
              (profile.executor === BaseCodingAgentValue.CODEX &&
                definition.transport !== 'stdio') ||
              (profile.executor === BaseCodingAgentValue.GROK &&
                definition.transport === 'sse')
            )
        )
        .slice(0, 1)
        .map((profile) => profile.executor);
      setServer({ name: key, definition, assignments });
    },
    [profiles, setServer]
  );

  const disconnectGateway = useCallback(
    async (server: SharedMcpServer) => {
      if (!machineClient) return;
      const value = server.definition.value as Record<string, JsonValue>;
      const url = typeof value.url === 'string' ? value.url : null;
      const connectionId = url?.split('/mcp-gateway/')[1]?.split(/[/?#]/)[0];
      if (!connectionId) return;
      if (!window.confirm(t('settings.mcp.auth.disconnectConfirm'))) return;
      await machineClient.disconnectSharedMcp(connectionId);
      await loadShared();
    },
    [loadShared, machineClient, t]
  );

  const testAssignments = useCallback(
    async (serverName?: string) => {
      if (!machineClient) return;
      setTesting(true);
      setError(null);
      try {
        const results = await machineClient.testSharedMcpAssignments({
          targets: testTargetsForDraft(draft, serverName),
        });
        const resultKeys = new Set(
          results.map((result) => testKey(result.server_name, result.executor))
        );
        setTestResults((prev) => ({
          ...prev,
          ...indexAssignmentTests(results),
        }));
        setCopyStates((prev) => {
          const next = { ...prev };
          for (const key of resultKeys) delete next[key];
          return next;
        });
        setDebugStates((prev) => {
          const next = { ...prev };
          for (const key of resettableMcpDebugKeys(
            resultKeys,
            creatingDebugKeysRef.current
          )) {
            delete next[key];
          }
          return next;
        });
      } catch (err) {
        setError(
          err instanceof Error ? err.message : t('settings.mcp.test.failed')
        );
      } finally {
        setTesting(false);
      }
    },
    [draft, machineClient, t]
  );

  const copyDiagnostic = useCallback(
    async (key: string, diagnostic: string) => {
      try {
        await navigator.clipboard.writeText(diagnostic);
        setCopyStates((prev) => ({
          ...prev,
          [key]: { status: 'success' },
        }));
      } catch (err) {
        setCopyStates((prev) => ({
          ...prev,
          [key]: {
            status: 'error',
            error:
              err instanceof Error
                ? err.message
                : t('settings.mcp.test.copyFailureUnknown'),
          },
        }));
      }
    },
    [t]
  );

  const createDebugIssue = useCallback(
    async (
      key: string,
      serverName: string,
      executor: BaseCodingAgent,
      diagnostic: string
    ) => {
      const availability = mcpDebugAvailability(
        projectContext !== null,
        projectContext?.statuses ?? []
      );
      if (!projectContext || !availability.available) return;

      const creationKey = mcpDebugCreationKey(projectContext.projectId, key);
      if (!acquireMcpDebugCreation(creationKey)) return;
      creatingDebugKeysRef.current.add(key);
      setDebugStates((prev) => ({
        ...prev,
        [key]: { status: 'creating' },
      }));

      try {
        const { persisted } = projectContext.insertIssue(
          buildMcpDebugIssueRequest({
            projectId: projectContext.projectId,
            status: availability.status,
            issues: projectContext.issues,
            serverName,
            executor,
            diagnostic,
          })
        );
        const issue = await persisted;
        setDebugStates((prev) => ({
          ...prev,
          [key]: { status: 'success', issueId: issue.id },
        }));
      } catch (err) {
        setDebugStates((prev) => ({
          ...prev,
          [key]: {
            status: 'error',
            error:
              err instanceof Error
                ? err.message
                : t('settings.mcp.test.debugFailureUnknown'),
          },
        }));
      } finally {
        creatingDebugKeysRef.current.delete(key);
        releaseMcpDebugCreation(creationKey);
      }
    },
    [projectContext, t]
  );

  const waitForAuthFlow = useCallback(
    async (flowId: string, popup: Window | null) => {
      if (!machineClient) return { status: 'failed', error: null } as const;
      for (;;) {
        await new Promise((resolve) => setTimeout(resolve, 1000));
        let status: McpAuthStatusResponse | null = null;
        try {
          status = await machineClient.getMcpAuthStatus(flowId);
        } catch {
          // Continue until the flow settles or expires.
        }
        if (status && status.status !== 'pending') return status;
        if (popup?.closed) {
          return {
            status: 'failed',
            error: t('settings.mcp.test.popupClosed'),
          } as McpAuthStatusResponse;
        }
      }
    },
    [machineClient, t]
  );

  const finalizeConnected = useCallback(
    async (serverName: string, executor: BaseCodingAgent) => {
      if (!machineClient) return;
      const refreshed = await machineClient.loadSharedMcpServers();
      setReadModel(refreshed);
      setDraft((prev) =>
        mergeOAuthRefresh(prev, refreshed, serverName, executor)
      );
      setOriginalSnapshot((prev) => {
        const merged = mergeOAuthRefresh(
          JSON.parse(prev) as SharedMcpDraftState,
          refreshed,
          serverName,
          executor
        );
        return sharedMcpSnapshot(merged);
      });
      await testAssignments(serverName);
    },
    [machineClient, testAssignments]
  );

  const connectAssignment = useCallback(
    async (
      serverName: string,
      executor: BaseCodingAgent,
      result: McpServerTestResult | undefined
    ) => {
      if (!machineClient) return;
      const key = testKey(serverName, executor);
      const priorConnectError = connectErrors[key];
      setConnectingKey(key);
      setConnectErrors((prev) => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
      const popup = window.open(
        'about:blank',
        'vk-mcp-oauth',
        'width=600,height=700,popup=yes'
      );
      if (!popup) {
        setConnectErrors((prev) => ({
          ...prev,
          [key]: t('settings.mcp.test.popupBlocked'),
        }));
        setConnectingKey(null);
        return;
      }
      const useLoopback = !!loopbackEnabled[key];
      setManualFlow(null);
      setManualCode('');
      try {
        let cloudflareAccess:
          | { clientId: string; clientSecret: string }
          | undefined;
        if (
          /cloudflare|http 302|interactive login/i.test(
            `${result?.error ?? ''} ${priorConnectError ?? ''}`
          )
        ) {
          const clientId = window.prompt(
            t('settings.mcp.auth.cloudflareClientId')
          );
          const clientSecret = clientId
            ? window.prompt(t('settings.mcp.auth.cloudflareClientSecret'))
            : null;
          if (clientId && clientSecret) {
            cloudflareAccess = { clientId, clientSecret };
          }
        }
        const started = await machineClient.startMcpAuth(
          { executor },
          serverName,
          result?.www_authenticate,
          useLoopback,
          cloudflareAccess
        );
        popup.location.href = started.authorize_url;
        if (started.loopback) {
          const { flow_id: flowId } = started;
          setManualFlow({ key, serverName, executor, flowId });
          void (async () => {
            for (;;) {
              await new Promise((resolve) => setTimeout(resolve, 1000));
              let status: McpAuthStatusResponse | null = null;
              try {
                status = await machineClient.getMcpAuthStatus(flowId);
              } catch {
                // Transient polling error; manual completion remains available.
              }
              if (status?.status === 'completed') {
                setManualFlow((current) =>
                  current?.flowId === flowId ? null : current
                );
                await finalizeConnected(serverName, executor);
                return;
              }
              if (status?.status === 'failed') {
                setConnectErrors((prev) => ({
                  ...prev,
                  [key]: status.error ?? t('settings.mcp.test.connectFailed'),
                }));
                setManualFlow((current) =>
                  current?.flowId === flowId ? null : current
                );
                return;
              }
              if (popup.closed) return;
            }
          })();
          return;
        }
        const outcome = await waitForAuthFlow(started.flow_id, popup);
        if (outcome.status !== 'completed') {
          setConnectErrors((prev) => ({
            ...prev,
            [key]: outcome.error ?? t('settings.mcp.test.connectFailed'),
          }));
          return;
        }
        await finalizeConnected(serverName, executor);
      } catch (err) {
        if (!popup.closed) popup.close();
        setConnectErrors((prev) => ({
          ...prev,
          [key]:
            err instanceof Error
              ? err.message
              : t('settings.mcp.test.connectFailed'),
        }));
      } finally {
        setConnectingKey(null);
      }
    },
    [
      connectErrors,
      finalizeConnected,
      loopbackEnabled,
      machineClient,
      t,
      waitForAuthFlow,
    ]
  );

  const completeManualAuth = useCallback(async () => {
    if (!machineClient || !manualFlow) return;
    const code = manualCode.trim();
    if (!code) return;
    const { key, serverName, executor, flowId } = manualFlow;
    setCompleting(true);
    setConnectErrors((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
    try {
      await machineClient.completeMcpAuth({ executor }, flowId, code);
      setManualFlow(null);
      setManualCode('');
      await finalizeConnected(serverName, executor);
    } catch (err) {
      setConnectErrors((prev) => ({
        ...prev,
        [key]:
          err instanceof Error
            ? err.message
            : t('settings.mcp.test.connectFailed'),
      }));
    } finally {
      setCompleting(false);
    }
  }, [finalizeConnected, machineClient, manualCode, manualFlow, t]);

  const enterJsonMode = useCallback(() => {
    setJsonText(JSON.stringify(inputsFromDraft(draft), null, 2));
    setJsonError(null);
    setJsonMode(true);
  }, [draft]);

  const applyJson = useCallback(
    (text: string) => {
      setJsonText(text);
      try {
        const parsed = JSON.parse(text) as SharedMcpDraftServer[];
        setDraft({ servers: parsed, conflicts: draft.conflicts });
        setJsonError(null);
      } catch {
        setJsonError(t('settings.mcp.errors.invalidJson'));
      }
    },
    [draft.conflicts, t]
  );

  if (!config) {
    return (
      <div className="rounded-sm border border-error/50 bg-error/10 p-4 text-error">
        {t('settings.mcp.errors.loadFailed')}
      </div>
    );
  }

  return (
    <>
      {error && (
        <div className="whitespace-pre-wrap rounded-sm border border-error/50 bg-error/10 p-4 text-error">
          {error}
        </div>
      )}
      {success && (
        <div className="rounded-sm border border-success/50 bg-success/10 p-4 font-medium text-success">
          {t('settings.mcp.save.successMessage')}
        </div>
      )}

      <SettingsCard
        title={t('settings.mcp.title')}
        description={t('settings.mcp.description')}
      >
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <label className="text-sm font-medium text-normal">
                {t('settings.mcp.labels.servers')}
              </label>
              <span className="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium text-low">
                {draft.servers.length}
              </span>
            </div>
            <p className="text-sm text-low">
              {t('settings.mcp.labels.assignmentsHelper')}
            </p>
          </div>
          <div className="grid w-full min-w-0 grid-cols-1 gap-1 sm:flex sm:w-auto sm:flex-wrap sm:items-center sm:shrink-0 sm:justify-end">
            <Button
              variant="ghost"
              size="sm"
              type="button"
              className="w-full justify-start sm:w-auto sm:justify-center"
              onClick={() => void testAssignments()}
              disabled={testing || isDirty || draft.servers.length === 0}
              title={isDirty ? t('settings.mcp.test.dirtyHint') : undefined}
            >
              {testing ? (
                <CircleNotchIcon
                  className="size-icon-xs mr-1 animate-spin"
                  weight="bold"
                />
              ) : (
                <CheckCircleIcon className="size-icon-xs mr-1" weight="bold" />
              )}
              {t('settings.mcp.test.button')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              type="button"
              className="w-full justify-start sm:w-auto sm:justify-center"
              onClick={jsonMode ? () => setJsonMode(false) : enterJsonMode}
            >
              <CodeIcon className="size-icon-xs mr-1" weight="bold" />
              {jsonMode
                ? t('settings.mcp.json.editAsForm')
                : t('settings.mcp.json.editAsJson')}
            </Button>
            <Button
              variant="default"
              size="sm"
              type="button"
              className="w-full justify-start sm:w-auto sm:justify-center"
              onClick={() => void openDialog()}
              disabled={loading || profiles.length === 0}
            >
              <PlusIcon className="size-icon-xs mr-1" weight="bold" />
              {t('settings.mcp.list.addServer')}
            </Button>
          </div>
        </div>

        {loading ? (
          <div className="py-4 text-sm text-low">
            {t('settings.mcp.loading')}
          </div>
        ) : jsonMode ? (
          <SettingsField label="" error={jsonError}>
            <SettingsTextarea
              value={jsonText}
              onChange={applyJson}
              rows={16}
              monospace
            />
          </SettingsField>
        ) : (
          <div className="space-y-3">
            {catalogServers.length > 0 && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-normal">
                  {t('settings.mcp.labels.popularServers')}
                </label>
                <p className="text-sm text-low">
                  {t('settings.mcp.labels.serverHelperForm')}
                </p>
                <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                  {catalogServers.map((server) => {
                    const added = draft.servers.some(
                      (draftServer) => draftServer.name === server.key
                    );
                    const icon = server.icon ? `/${server.icon}` : null;
                    return (
                      <button
                        key={server.key}
                        type="button"
                        onClick={() =>
                          addPreconfigured(server.key, server.entry)
                        }
                        disabled={added}
                        className={cn(
                          'flex items-start gap-3 rounded-sm border border-border/50 bg-secondary/30 p-3 text-left transition-colors',
                          added
                            ? 'cursor-default opacity-60'
                            : 'hover:border-border hover:bg-secondary'
                        )}
                      >
                        <div className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-sm border border-border bg-secondary">
                          {icon ? (
                            <img
                              src={icon}
                              alt=""
                              className="size-full object-cover"
                            />
                          ) : (
                            <span className="text-xs font-semibold text-normal">
                              {server.name.slice(0, 1).toUpperCase()}
                            </span>
                          )}
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-medium text-normal">
                            {server.name}
                          </div>
                          {server.description && (
                            <div className="line-clamp-2 text-xs text-low">
                              {server.description}
                            </div>
                          )}
                        </div>
                        {added ? (
                          <CheckIcon
                            className="size-icon-xs shrink-0 text-success"
                            weight="bold"
                          />
                        ) : (
                          <PlusIcon
                            className="size-icon-xs shrink-0 text-low"
                            weight="bold"
                          />
                        )}
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {draft.conflicts.length > 0 && (
              <div className="space-y-3 rounded-sm border border-warning/50 bg-warning/10 p-3 text-sm text-warning">
                <div className="font-medium">
                  {t('settings.mcp.conflicts.title')}
                </div>
                {draft.conflicts.map((conflict) => (
                  <div key={conflict.name} className="space-y-2">
                    <div>{conflict.message}</div>
                    <div className="flex flex-wrap gap-2">
                      {conflict.variants.map((variant) => {
                        const agents = variant.assignments
                          .map((assignment) =>
                            toPrettyCase(assignment.executor)
                          )
                          .join(', ');
                        return (
                          <Button
                            key={variant.variant_id}
                            variant="outline"
                            size="sm"
                            type="button"
                            disabled={
                              variant.definition.transport === 'unknown'
                            }
                            title={
                              variant.definition.transport === 'unknown'
                                ? t('settings.mcp.conflicts.customUnsupported')
                                : undefined
                            }
                            onClick={() =>
                              resolveConflict(conflict.name, variant.variant_id)
                            }
                          >
                            {t('settings.mcp.conflicts.useDefinition', {
                              agents,
                            })}
                          </Button>
                        );
                      })}
                    </div>
                    <div className="text-xs text-low">
                      {t('settings.mcp.conflicts.helper')}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {draft.servers.length === 0 ? (
              <div className="rounded-sm border border-border bg-secondary/30 p-4 text-sm text-low">
                {t('settings.mcp.list.empty')}
              </div>
            ) : (
              draft.servers.map((server) => {
                const source = serverByName.get(server.name);
                const serverResults = server.assignments
                  .map((executor) => ({
                    executor,
                    key: testKey(server.name, executor),
                    test: testResults[testKey(server.name, executor)],
                  }))
                  .filter(
                    (item) => item.test && item.test.result.status !== 'ok'
                  );
                const attentionResult =
                  serverResults.find(
                    (item) => item.test?.result.status === 'auth_required'
                  ) ?? serverResults[0];
                const attentionKey = attentionResult?.key;
                const attentionTest = attentionResult?.test;
                return (
                  <div
                    key={server.name}
                    className="w-full min-w-0 max-w-full space-y-3 rounded-sm border border-border bg-secondary/30 p-3"
                  >
                    <div className="min-w-0">
                      <div className="min-w-0">
                        <div className="flex min-w-0 flex-wrap items-center gap-2">
                          <span className="truncate font-medium text-high">
                            {server.name}
                          </span>
                          <span className="rounded-sm bg-primary px-1.5 py-0.5 font-mono text-xs text-low">
                            {transportBadge(server.definition)}
                          </span>
                          {source && (
                            <span className="rounded-sm border border-border px-1.5 py-0.5 text-xs text-low">
                              {source.auth_mode === 'shared_gateway'
                                ? t('settings.mcp.auth.sharedGateway')
                                : source.auth_mode === 'explicit_header'
                                  ? t('settings.mcp.auth.explicitHeader')
                                  : source.auth_mode === 'agent_native'
                                    ? t('settings.mcp.auth.agentNative')
                                    : t('settings.mcp.auth.none')}
                              {source.gateway_status
                                ? ` · ${source.gateway_status}`
                                : ''}
                            </span>
                          )}
                        </div>
                        {server.assignments.length === 0 ? (
                          <span className="text-xs text-low italic">
                            {t('settings.mcp.labels.noAssignments')}
                          </span>
                        ) : (
                          <div className="mt-1 flex flex-wrap gap-1">
                            {server.assignments.map((executor) => {
                              const result =
                                testResults[testKey(server.name, executor)]
                                  ?.result;
                              return (
                                <span
                                  key={executor}
                                  className="inline-flex items-center gap-1 rounded-sm bg-primary border border-border px-1.5 py-0.5 text-xs text-low"
                                >
                                  {toPrettyCase(executor)}
                                  <McpTestStatusIcon result={result} />
                                </span>
                              );
                            })}
                          </div>
                        )}
                      </div>
                      <div className="mt-3 flex max-w-full flex-wrap items-center gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          type="button"
                          onClick={() => void testAssignments(server.name)}
                          disabled={testing || isDirty}
                          title={t('settings.mcp.test.button')}
                        >
                          <CheckCircleIcon className="mr-1 size-icon-sm" />
                          {t('settings.mcp.test.button')}
                        </Button>
                        {source?.auth_mode === 'shared_gateway' && (
                          <>
                            <Button
                              variant="ghost"
                              size="sm"
                              type="button"
                              onClick={() => {
                                const executor = server.assignments[0];
                                if (executor)
                                  void connectAssignment(
                                    server.name,
                                    executor,
                                    undefined
                                  );
                              }}
                            >
                              {t('settings.mcp.auth.reconnect')}
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              type="button"
                              onClick={() => void disconnectGateway(source)}
                            >
                              {t('settings.mcp.auth.disconnect')}
                            </Button>
                          </>
                        )}
                        <Button
                          variant="ghost"
                          size="sm"
                          type="button"
                          onClick={() => void openDialog(server)}
                          title={t('settings.mcp.dialog.editTitle')}
                        >
                          <PencilSimpleIcon className="mr-1 size-icon-sm" />
                          {t('settings.mcp.edit')}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          type="button"
                          className="text-error"
                          onClick={() => {
                            if (window.confirm(t('settings.mcp.deleteConfirm')))
                              removeServer(server.name);
                          }}
                          title={t('settings.mcp.delete')}
                        >
                          <TrashIcon className="mr-1 size-icon-sm" />
                          {t('settings.mcp.delete')}
                        </Button>
                      </div>
                    </div>

                    {attentionResult && attentionKey && attentionTest && (
                      <TestResultDetails
                        result={attentionTest.result}
                        diagnostic={mcpDiagnosticText(
                          attentionTest.result.error,
                          t('settings.mcp.test.missingDiagnostic')
                        )}
                        executor={attentionResult.executor}
                        connecting={connectingKey === attentionKey}
                        connectError={connectErrors[attentionKey]}
                        onConnect={() =>
                          void connectAssignment(
                            server.name,
                            attentionResult.executor,
                            attentionTest.result
                          )
                        }
                        loopback={!!loopbackEnabled[attentionKey]}
                        onToggleLoopback={() =>
                          setLoopbackEnabled((prev) => ({
                            ...prev,
                            [attentionKey]: !prev[attentionKey],
                          }))
                        }
                        manualActive={manualFlow?.key === attentionKey}
                        manualCode={manualCode}
                        onManualCodeChange={setManualCode}
                        onManualComplete={() => void completeManualAuth()}
                        completing={completing}
                        copyState={copyStates[attentionKey]}
                        onCopy={() =>
                          void copyDiagnostic(
                            attentionKey,
                            mcpDiagnosticText(
                              attentionTest.result.error,
                              t('settings.mcp.test.missingDiagnostic')
                            )
                          )
                        }
                        debugState={debugStates[attentionKey]}
                        debugUnavailableReason={
                          projectContext === null
                            ? 'no-project'
                            : projectContext.statuses.length === 0
                              ? 'no-status'
                              : null
                        }
                        onCreateDebugIssue={() =>
                          void createDebugIssue(
                            attentionKey,
                            server.name,
                            attentionResult.executor,
                            mcpDiagnosticText(
                              attentionTest.result.error,
                              t('settings.mcp.test.missingDiagnostic')
                            )
                          )
                        }
                        onOpenDebugIssue={() => {
                          const issueId = debugStates[attentionKey]?.issueId;
                          if (projectContext && issueId) {
                            appNavigation.goToProjectIssue(
                              projectContext.projectId,
                              issueId
                            );
                          }
                        }}
                      />
                    )}
                    {serverResults.length > 1 && (
                      <div className="mt-1 space-y-1">
                        {serverResults
                          .filter((item) => item.key !== attentionKey)
                          .map((item) => {
                            if (!item.test) return null;
                            const diagnostic = mcpDiagnosticText(
                              item.test.result.error,
                              t('settings.mcp.test.missingDiagnostic')
                            );
                            return (
                              <TestResultDetails
                                key={item.key}
                                result={item.test.result}
                                diagnostic={diagnostic}
                                executor={item.executor}
                                connecting={connectingKey === item.key}
                                connectError={connectErrors[item.key]}
                                onConnect={() =>
                                  void connectAssignment(
                                    server.name,
                                    item.executor,
                                    item.test?.result
                                  )
                                }
                                loopback={!!loopbackEnabled[item.key]}
                                onToggleLoopback={() =>
                                  setLoopbackEnabled((prev) => ({
                                    ...prev,
                                    [item.key]: !prev[item.key],
                                  }))
                                }
                                manualActive={manualFlow?.key === item.key}
                                manualCode={manualCode}
                                onManualCodeChange={setManualCode}
                                onManualComplete={() =>
                                  void completeManualAuth()
                                }
                                completing={completing}
                                copyState={copyStates[item.key]}
                                onCopy={() =>
                                  void copyDiagnostic(item.key, diagnostic)
                                }
                                debugState={debugStates[item.key]}
                                debugUnavailableReason={
                                  projectContext === null
                                    ? 'no-project'
                                    : projectContext.statuses.length === 0
                                      ? 'no-status'
                                      : null
                                }
                                onCreateDebugIssue={() =>
                                  void createDebugIssue(
                                    item.key,
                                    server.name,
                                    item.executor,
                                    diagnostic
                                  )
                                }
                                onOpenDebugIssue={() => {
                                  const issueId =
                                    debugStates[item.key]?.issueId;
                                  if (projectContext && issueId) {
                                    appNavigation.goToProjectIssue(
                                      projectContext.projectId,
                                      issueId
                                    );
                                  }
                                }}
                              />
                            );
                          })}
                      </div>
                    )}
                  </div>
                );
              })
            )}
          </div>
        )}
      </SettingsCard>

      <SettingsSaveBar
        show={isDirty}
        saving={saving}
        onSave={() => void save()}
        onDiscard={discard}
        unsavedMessage={t('settings.mcp.save.unsavedChanges')}
      />
    </>
  );
}
