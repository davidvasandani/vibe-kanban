import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircleIcon,
  CircleNotchIcon,
  CodeIcon,
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
  SharedMcpProfile,
  SharedMcpReadResponse,
  SharedMcpServer,
} from 'shared/types';
import { BaseCodingAgent as BaseCodingAgentValue } from 'shared/types';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { codecForAgent, transportOf } from '@/shared/lib/mcpServerCodec';
import {
  definitionFromEntry,
  draftFromSharedRead,
  indexAssignmentTests,
  inputsFromDraft,
  mergeOAuthRefresh,
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
}: {
  result: McpServerTestResult | undefined;
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
}) {
  const { t } = useTranslation('settings');
  if (!result || result.status === 'ok') return null;
  const authRequired = result.status === 'auth_required';
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
        existingNames: draft.servers
          .map((item) => item.name)
          .filter((name) => name !== server?.name),
        initial: server
          ? { name: server.name, entry: entryForDialog(server.definition) }
          : undefined,
      });
      if (!result) return;
      const definition = definitionFromEntry(result.entry);
      const assignments = server?.assignments.length
        ? server.assignments
        : profiles
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
      if (server && server.name !== result.name) {
        setDraft((prev) => ({
          ...prev,
          servers: prev.servers.filter((item) => item.name !== server.name),
        }));
      }
      setServer({
        name: result.name,
        definition,
        assignments,
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

  const toggleAssignment = useCallback(
    (serverName: string, profile: SharedMcpProfile) => {
      setDraft((prev) => ({
        ...prev,
        servers: prev.servers.map((server) => {
          if (server.name !== serverName) return server;
          const assigned = server.assignments.includes(profile.executor);
          return {
            ...server,
            assignments: assigned
              ? server.assignments.filter(
                  (executor) => executor !== profile.executor
                )
              : [...server.assignments, profile.executor],
          };
        }),
      }));
    },
    []
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
        setTestResults((prev) => ({
          ...prev,
          ...indexAssignmentTests(results),
        }));
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
            <label className="text-sm font-medium text-normal">
              {t('settings.mcp.labels.servers')}
            </label>
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
                    className="w-full min-w-0 max-w-full rounded-sm border border-border bg-secondary/30 p-3"
                  >
                    <div className="flex min-w-0 flex-col items-start gap-2 sm:flex-row sm:justify-between">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
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
                        <div className="mt-1 text-xs text-low">
                          {server.assignments.length}{' '}
                          {t('settings.mcp.labels.assignments')}
                        </div>
                      </div>
                      <div className="flex max-w-full flex-wrap items-center gap-1 sm:shrink-0">
                        <Button
                          variant="ghost"
                          size="icon"
                          type="button"
                          onClick={() => void testAssignments(server.name)}
                          disabled={testing || isDirty}
                          title={t('settings.mcp.test.button')}
                        >
                          <CheckCircleIcon className="size-icon-sm" />
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
                          size="icon"
                          type="button"
                          onClick={() => void openDialog(server)}
                          title={t('settings.mcp.dialog.editTitle')}
                        >
                          <PencilSimpleIcon className="size-icon-sm" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          type="button"
                          className="text-error"
                          onClick={() => {
                            if (window.confirm(t('settings.mcp.deleteConfirm')))
                              removeServer(server.name);
                          }}
                          title={t('settings.mcp.delete')}
                        >
                          <TrashIcon className="size-icon-sm" />
                        </Button>
                      </div>
                    </div>

                    {attentionResult && attentionKey && attentionTest && (
                      <TestResultDetails
                        result={attentionTest.result}
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
                      />
                    )}
                    {serverResults.length > 1 && (
                      <div className="mt-1 space-y-0.5 px-2 text-xs text-low">
                        {serverResults
                          .filter((item) => item.key !== attentionKey)
                          .map((item) => (
                            <div
                              key={item.key}
                              className="flex min-w-0 gap-1"
                              title={item.test?.result.error ?? ''}
                            >
                              <span className="shrink-0 font-medium">
                                {toPrettyCase(item.executor)}:
                              </span>
                              <span className="truncate font-mono">
                                {item.test?.result.error ??
                                  item.test?.result.status}
                              </span>
                            </div>
                          ))}
                      </div>
                    )}

                    <div className="mt-2 grid gap-1 sm:grid-cols-2 lg:grid-cols-3">
                      {profiles.map((profile) => {
                        const compatibility = source?.compatibility.find(
                          (item) => item.executor === profile.executor
                        );
                        const incompatible =
                          compatibility?.compatible === false;
                        const assigned = server.assignments.includes(
                          profile.executor
                        );
                        const key = testKey(server.name, profile.executor);
                        const result = testResults[key]?.result;
                        return (
                          <div
                            key={profile.executor}
                            className="rounded-sm border border-border bg-primary px-2 py-1.5"
                          >
                            <label className="flex items-center gap-2 text-sm">
                              <input
                                type="checkbox"
                                checked={assigned}
                                disabled={incompatible}
                                onChange={() =>
                                  toggleAssignment(server.name, profile)
                                }
                              />
                              <span className="min-w-0 flex-1 truncate">
                                {toPrettyCase(profile.executor)}
                              </span>
                              {assigned && (
                                <McpTestStatusIcon result={result} />
                              )}
                            </label>
                            {incompatible && (
                              <div className="mt-1 text-xs text-error">
                                {compatibility?.reason}
                              </div>
                            )}
                            {assigned && testResults[key]?.gateway_status && (
                              <div className="mt-1 text-xs text-low">
                                Gateway: {testResults[key].gateway_status} ·
                                Upstream: {testResults[key].upstream_status}
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
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
