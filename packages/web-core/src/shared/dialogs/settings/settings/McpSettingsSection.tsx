import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircleIcon,
  CheckIcon,
  CircleNotchIcon,
  CodeIcon,
  LockKeyIcon,
  MinusCircleIcon,
  PencilSimpleIcon,
  PlusIcon,
  TrashIcon,
  XCircleIcon,
} from '@phosphor-icons/react';
import type {
  BaseCodingAgent,
  ExecutorProfile,
  JsonValue,
  McpAuthStatusResponse,
  McpServerTestResult,
} from 'shared/types';
import { McpConfig } from 'shared/types';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { McpConfigStrategyGeneral } from '@/shared/lib/mcpStrategies';
import {
  codecForAgent,
  transportOf,
  type McpServerCodec,
} from '@/shared/lib/mcpServerCodec';
import { cn } from '@/shared/lib/utils';
import { toPrettyCase } from '@/shared/lib/string';
import { Button } from '@vibe/ui/components/Button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuTriggerButton,
} from '@vibe/ui/components/Dropdown';
import {
  SettingsCard,
  SettingsField,
  SettingsSaveBar,
  SettingsTextarea,
} from './SettingsComponents';
import { McpServerDialog } from './McpServerDialog';
import { useSettingsDirty } from './SettingsDirtyContext';
import { useSettingsMachineClient } from './SettingsHostContext';

type ServerMap = Record<string, JsonValue>;

const isObject = (v: JsonValue | undefined): v is ServerMap =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

/** Badge text for a server entry's transport. */
function transportBadge(
  codec: McpServerCodec,
  entry: JsonValue,
  customLabel: string
): string {
  const transport = transportOf(codec, entry);
  if (transport === null) return customLabel;
  if (transport === 'stdio') return 'stdio';
  return transport.toUpperCase();
}

/** Per-server connectivity status icon with a hover summary. */
function McpTestStatusIcon({
  result,
}: {
  result: McpServerTestResult | undefined;
}) {
  if (!result) return null;
  const { status } = result;
  const Icon =
    status === 'ok'
      ? CheckCircleIcon
      : status === 'auth_required'
        ? LockKeyIcon
        : status === 'unsupported'
          ? MinusCircleIcon
          : XCircleIcon;
  const color =
    status === 'ok'
      ? 'text-success'
      : status === 'auth_required'
        ? 'text-warning'
        : status === 'unsupported'
          ? 'text-low'
          : 'text-error';
  const title =
    status === 'ok'
      ? [
          `${result.tool_count ?? 0} tools`,
          result.latency_ms != null ? `${result.latency_ms}ms` : null,
          result.server_name
            ? `${result.server_name}${
                result.server_version ? ` v${result.server_version}` : ''
              }`
            : null,
        ]
          .filter(Boolean)
          .join(' · ')
      : (result.error ?? status);
  return (
    <span title={title} className="flex items-center px-1">
      <Icon className={cn('size-icon-sm', color)} weight="fill" />
    </span>
  );
}

/**
 * Inline detail line for a non-ok test result: the error text is readable
 * without hovering (FR-2), clamped and click-expandable when long, with a
 * distinct auth-required treatment and Connect action (FR-3/FR-4).
 */
function McpTestResultDetails({
  result,
  connecting,
  connectError,
  onConnect,
  connectDisabled,
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
  connectDisabled: boolean;
  loopback: boolean;
  onToggleLoopback: () => void;
  manualActive: boolean;
  manualCode: string;
  onManualCodeChange: (value: string) => void;
  onManualComplete: () => void;
  completing: boolean;
}) {
  const { t } = useTranslation('settings');
  const [expanded, setExpanded] = useState(false);
  const [connectExpanded, setConnectExpanded] = useState(false);
  if (!result || result.status === 'ok') return null;

  const authRequired = result.status === 'auth_required';
  const palette = authRequired
    ? 'border-warning/50 bg-warning/10 text-warning'
    : result.status === 'unsupported'
      ? 'border-border bg-secondary/50 text-low'
      : 'border-error/50 bg-error/10 text-error';

  return (
    <div className={cn('rounded-sm border p-2 text-xs', palette)}>
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          {authRequired && (
            <div className="font-medium">
              {t('settings.mcp.test.authRequired')}
            </div>
          )}
          {result.error && (
            <button
              type="button"
              onClick={() => setExpanded((prev) => !prev)}
              className={cn(
                'w-full text-left font-mono break-words',
                !expanded && 'line-clamp-2'
              )}
              title={result.error}
            >
              {result.error}
            </button>
          )}
        </div>
        {authRequired && (
          <Button
            variant="outline"
            size="sm"
            type="button"
            className="shrink-0"
            onClick={onConnect}
            disabled={connecting || connectDisabled}
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
        <button
          type="button"
          onClick={() => setConnectExpanded((prev) => !prev)}
          className={cn(
            'mt-2 block w-full whitespace-pre-wrap break-words rounded-sm border border-error/50 bg-error/10 p-2 text-left text-error',
            !connectExpanded && 'line-clamp-3'
          )}
          title={connectError}
        >
          {connectError}
        </button>
      )}
      {authRequired && (
        <label className="mt-2 flex items-center gap-2 text-low">
          <input
            type="checkbox"
            checked={loopback}
            onChange={onToggleLoopback}
            disabled={connecting || manualActive}
          />
          {t('settings.mcp.test.useLocalhostCallback')}
        </label>
      )}
      {manualActive && (
        <div className="mt-2 space-y-2 border-t border-current/20 pt-2">
          <p className="text-low">{t('settings.mcp.test.manualHint')}</p>
          <input
            type="text"
            value={manualCode}
            onChange={(e) => onManualCodeChange(e.target.value)}
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
            {completing ? (
              <CircleNotchIcon
                className="size-icon-xs mr-1 animate-spin"
                weight="bold"
              />
            ) : null}
            {t('settings.mcp.test.finishConnect')}
          </Button>
        </div>
      )}
    </div>
  );
}

export function McpSettingsSection() {
  const { t } = useTranslation('settings');
  const { setDirty: setContextDirty } = useSettingsDirty();
  const machineClient = useSettingsMachineClient();
  const { config, profiles } = useUserSystem();

  const [servers, setServers] = useState<ServerMap>({});
  const [originalSnapshot, setOriginalSnapshot] = useState('{}');
  const [mcpConfig, setMcpConfig] = useState<McpConfig | null>(null);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [mcpLoading, setMcpLoading] = useState(true);
  const [selectedProfile, setSelectedProfile] =
    useState<ExecutorProfile | null>(null);
  const [mcpApplying, setMcpApplying] = useState(false);
  const [mcpConfigPath, setMcpConfigPath] = useState<string>('');
  const [success, setSuccess] = useState(false);

  // Raw-JSON escape hatch.
  const [mode, setMode] = useState<'form' | 'json'>('form');
  const [jsonText, setJsonText] = useState('{}');
  const [jsonError, setJsonError] = useState<string | null>(null);

  // Connectivity test: per-server probe results keyed by server name.
  const [testResults, setTestResults] = useState<Record<
    string,
    McpServerTestResult
  > | null>(null);
  const [testing, setTesting] = useState(false);
  const [testError, setTestError] = useState<string | null>(null);
  // Tracks the current profile so an in-flight test can be discarded if the
  // user switches agents before it resolves.
  const activeProfileRef = useRef<ExecutorProfile | null>(selectedProfile);
  useEffect(() => {
    activeProfileRef.current = selectedProfile;
  }, [selectedProfile]);

  const snapshot = useMemo(() => JSON.stringify(servers), [servers]);
  const isDirty = snapshot !== originalSnapshot;

  const selectedProfileKey = useMemo(
    () =>
      selectedProfile
        ? Object.keys(profiles || {}).find(
            (key) => profiles![key] === selectedProfile
          ) || ''
        : '',
    [selectedProfile, profiles]
  );

  const codec = useMemo(
    () =>
      selectedProfileKey
        ? codecForAgent(selectedProfileKey as BaseCodingAgent)
        : null,
    [selectedProfileKey]
  );

  // Sync dirty state to context for unsaved changes confirmation.
  useEffect(() => {
    setContextDirty('mcp', isDirty);
    return () => setContextDirty('mcp', false);
  }, [isDirty, setContextDirty]);

  // Initialize selected profile when config loads.
  useEffect(() => {
    if (config?.executor_profile && profiles && !selectedProfile) {
      const currentProfile = profiles[config.executor_profile.executor];
      if (currentProfile) {
        setSelectedProfile(currentProfile);
      } else if (Object.keys(profiles).length > 0) {
        setSelectedProfile(Object.values(profiles)[0]);
      }
    }
  }, [config?.executor_profile, profiles, selectedProfile]);

  // Load MCP configuration when selected profile changes.
  useEffect(() => {
    const loadMcpServersForProfile = async (profile: ExecutorProfile) => {
      setMcpLoading(true);
      setMcpError(null);
      setMcpConfigPath('');
      setMode('form');
      setJsonError(null);
      setTestResults(null);
      setTestError(null);
      setTesting(false);
      setConnectErrors({});
      setManualFlow(null);
      setManualCode('');

      try {
        const profileKey = profiles
          ? Object.keys(profiles).find((key) => profiles[key] === profile)
          : null;
        if (!profileKey) throw new Error('Profile key not found');
        if (!machineClient) throw new Error('Machine client is required');

        const result = await machineClient.loadMcpServers({
          executor: profileKey as BaseCodingAgent,
        });
        setMcpConfig(result.mcp_config);
        const loaded = (result.mcp_config.servers ?? {}) as ServerMap;
        setServers(loaded);
        setOriginalSnapshot(JSON.stringify(loaded));
        setMcpConfigPath(result.config_path);
      } catch (err: unknown) {
        if (
          err instanceof Error &&
          err.message.includes('does not support MCP')
        ) {
          setMcpError(err.message);
        } else {
          console.error('Error loading MCP servers:', err);
        }
      } finally {
        setMcpLoading(false);
      }
    };

    if (selectedProfile) {
      loadMcpServersForProfile(selectedProfile);
    }
  }, [machineClient, profiles, selectedProfile]);

  const handleApply = useCallback(async () => {
    if (!selectedProfile || !mcpConfig || !selectedProfileKey) return;

    setMcpApplying(true);
    setMcpError(null);

    try {
      if (!machineClient) throw new Error('Machine client is required');
      await machineClient.saveMcpServers(
        { executor: selectedProfileKey as BaseCodingAgent },
        { servers }
      );
      setOriginalSnapshot(JSON.stringify(servers));
      // Saved config changed the server set; drop stale statuses.
      setTestResults(null);
      setTestError(null);
      setConnectErrors({});
      setManualFlow(null);
      setManualCode('');
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
    } catch (err) {
      setMcpError(
        err instanceof Error ? err.message : t('settings.mcp.errors.saveFailed')
      );
      console.error('Error applying MCP servers:', err);
    } finally {
      setMcpApplying(false);
    }
  }, [
    machineClient,
    mcpConfig,
    selectedProfile,
    selectedProfileKey,
    servers,
    t,
  ]);

  const handleDiscard = useCallback(() => {
    setServers(JSON.parse(originalSnapshot) as ServerMap);
    setMcpError(null);
    setJsonError(null);
    setMode('form');
  }, [originalSnapshot]);

  // Probe the servers saved on disk for the selected agent and index the
  // results by server name so each row can show its own status.
  const handleTest = useCallback(async () => {
    if (!machineClient || !selectedProfileKey) return;

    const requestedProfile = selectedProfile;
    const isStale = () => activeProfileRef.current !== requestedProfile;

    setTesting(true);
    setTestError(null);
    setTestResults(null);
    setConnectErrors({});
    setManualFlow(null);
    setManualCode('');

    try {
      const results = await machineClient.testMcpServers({
        executor: selectedProfileKey as BaseCodingAgent,
      });
      if (isStale()) return;
      const byName: Record<string, McpServerTestResult> = {};
      for (const result of results) byName[result.name] = result;
      setTestResults(byName);
    } catch (err) {
      if (isStale()) return;
      setTestError(
        err instanceof Error ? err.message : t('settings.mcp.test.failed')
      );
    } finally {
      if (!isStale()) setTesting(false);
    }
  }, [machineClient, selectedProfile, selectedProfileKey, t]);

  // OAuth Connect flow for an auth-required server: start the flow, open the
  // consent popup, poll until it resolves, then refresh state from disk (the
  // callback wrote the token behind the UI's back — a later Save must not
  // wipe it) and re-test just that server.
  const [connectingServer, setConnectingServer] = useState<string | null>(null);
  // Connect failures are shown on the originating server's card, not the
  // global test banner, so the message stays next to what it's about.
  const [connectErrors, setConnectErrors] = useState<Record<string, string>>(
    {}
  );
  // Per-card "use localhost callback" toggle, and the active manual-paste flow
  // it produces. Loopback mode registers a http://localhost callback that
  // strict-allowlist authorization servers accept; because the browser may not
  // be able to reach that loopback (e.g. VK opened on a phone), the flow is
  // finished by pasting the redirected URL/code back rather than by an
  // automatic callback.
  const [loopbackEnabled, setLoopbackEnabled] = useState<
    Record<string, boolean>
  >({});
  const [manualFlow, setManualFlow] = useState<{
    server: string;
    flowId: string;
  } | null>(null);
  const [manualCode, setManualCode] = useState('');
  const [completing, setCompleting] = useState(false);

  // Refresh a just-connected server from disk (the token was written behind
  // the UI's back — a later Save must not wipe it) and re-test just that one.
  const finalizeConnected = useCallback(
    async (serverName: string) => {
      if (!machineClient || !selectedProfileKey) return;
      const requestedProfile = selectedProfile;
      const isStale = () => activeProfileRef.current !== requestedProfile;
      const executorQuery = {
        executor: selectedProfileKey as BaseCodingAgent,
      };
      const fresh = await machineClient.loadMcpServers(executorQuery);
      if (isStale()) return;
      const freshEntry = ((fresh.mcp_config.servers ?? {}) as ServerMap)[
        serverName
      ];
      if (freshEntry !== undefined) {
        setServers((prev) => ({ ...prev, [serverName]: freshEntry }));
        setOriginalSnapshot((prev) => {
          const base = JSON.parse(prev) as ServerMap;
          base[serverName] = freshEntry;
          return JSON.stringify(base);
        });
      }
      const results = await machineClient.testMcpServers(executorQuery, {
        servers: [serverName],
      });
      if (isStale()) return;
      setTestResults((prev) => {
        const next = { ...(prev ?? {}) };
        for (const result of results) next[result.name] = result;
        return next;
      });
    },
    [machineClient, selectedProfile, selectedProfileKey]
  );

  const waitForAuthFlow = useCallback(
    async (
      flowId: string,
      popup: Window | null
    ): Promise<McpAuthStatusResponse> => {
      if (!machineClient) return { status: 'failed', error: null };
      for (;;) {
        await new Promise((resolve) => setTimeout(resolve, 1000));
        let status: McpAuthStatusResponse | null = null;
        try {
          status = await machineClient.getMcpAuthStatus(flowId);
        } catch {
          // Transient polling error; keep going until the flow TTL kicks in.
        }
        if (status && status.status !== 'pending') return status;
        if (popup?.closed) {
          // The success page closes itself, so check once more before
          // treating a closed popup as an abandoned flow.
          try {
            const final = await machineClient.getMcpAuthStatus(flowId);
            if (final.status !== 'pending') return final;
          } catch {
            // fall through to the abandoned-flow result
          }
          return {
            status: 'failed',
            error: t('settings.mcp.test.popupClosed'),
          };
        }
      }
    },
    [machineClient, t]
  );

  const handleConnect = useCallback(
    async (serverName: string) => {
      if (!machineClient || !selectedProfileKey) return;

      const requestedProfile = selectedProfile;
      const isStale = () => activeProfileRef.current !== requestedProfile;
      const executorQuery = {
        executor: selectedProfileKey as BaseCodingAgent,
      };

      setConnectingServer(serverName);
      setConnectErrors((prev) => {
        const next = { ...prev };
        delete next[serverName];
        return next;
      });
      const failConnect = (message: string) =>
        setConnectErrors((prev) => ({ ...prev, [serverName]: message }));

      // Open the popup synchronously inside the click gesture and navigate
      // it once the start request resolves — a popup opened after an await
      // can lose the transient user activation and get blocked.
      const popup = window.open(
        'about:blank',
        'vk-mcp-oauth',
        'width=600,height=700,popup=yes'
      );
      if (!popup) {
        failConnect(t('settings.mcp.test.popupBlocked'));
        setConnectingServer(null);
        return;
      }

      const useLoopback = !!loopbackEnabled[serverName];
      setManualFlow(null);
      setManualCode('');

      try {
        // Hand the probe's captured challenge to discovery — some servers
        // only send WWW-Authenticate on the JSON-RPC POST the probe makes.
        const started = await machineClient.startMcpAuth(
          executorQuery,
          serverName,
          testResults?.[serverName]?.www_authenticate,
          useLoopback
        );
        if (isStale()) {
          popup.close();
          return;
        }
        popup.location.href = started.authorize_url;

        if (started.loopback) {
          // The browser may not be able to reach the localhost callback
          // (VK opened remotely), so reveal the manual paste field. But if the
          // callback *is* reachable (same machine / port-forward) it completes
          // on its own — poll in the background so that case still finalizes
          // (refresh the snapshot so Save can't drop the token) and dismisses
          // the manual field. A closed popup is not treated as failure here;
          // pasting is the fallback.
          const { flow_id: loopbackFlowId } = started;
          setManualFlow({ server: serverName, flowId: loopbackFlowId });
          void (async () => {
            for (;;) {
              await new Promise((resolve) => setTimeout(resolve, 1000));
              if (isStale()) return;
              let status: McpAuthStatusResponse | null = null;
              try {
                status = await machineClient.getMcpAuthStatus(loopbackFlowId);
              } catch {
                // transient; keep polling until completion or popup close
              }
              if (status?.status === 'completed') {
                await finalizeConnected(serverName);
                if (!isStale()) {
                  setManualFlow((cur) =>
                    cur?.flowId === loopbackFlowId ? null : cur
                  );
                }
                return;
              }
              // On explicit failure or once the popup is gone, stop the
              // background poll and leave the manual field for the user.
              if (status?.status === 'failed' || popup.closed) return;
            }
          })();
          return;
        }

        const outcome = await waitForAuthFlow(started.flow_id, popup);
        if (isStale()) return;
        if (outcome.status !== 'completed') {
          failConnect(outcome.error ?? t('settings.mcp.test.connectFailed'));
          return;
        }
        await finalizeConnected(serverName);
      } catch (err) {
        if (!popup.closed) popup.close();
        if (isStale()) return;
        failConnect(
          err instanceof Error
            ? err.message
            : t('settings.mcp.test.connectFailed')
        );
      } finally {
        if (!isStale()) setConnectingServer(null);
      }
    },
    [
      machineClient,
      selectedProfile,
      selectedProfileKey,
      t,
      testResults,
      loopbackEnabled,
      finalizeConnected,
      waitForAuthFlow,
    ]
  );

  const handleCompleteManual = useCallback(async () => {
    if (!machineClient || !selectedProfileKey || !manualFlow) return;
    const { server, flowId } = manualFlow;
    const code = manualCode.trim();
    if (!code) return;

    setCompleting(true);
    setConnectErrors((prev) => {
      const next = { ...prev };
      delete next[server];
      return next;
    });
    try {
      await machineClient.completeMcpAuth(
        { executor: selectedProfileKey as BaseCodingAgent },
        flowId,
        code
      );
      await finalizeConnected(server);
      setManualFlow(null);
      setManualCode('');
    } catch (err) {
      setConnectErrors((prev) => ({
        ...prev,
        [server]:
          err instanceof Error
            ? err.message
            : t('settings.mcp.test.connectFailed'),
      }));
    } finally {
      setCompleting(false);
    }
  }, [
    machineClient,
    selectedProfileKey,
    manualFlow,
    manualCode,
    finalizeConnected,
    t,
  ]);

  const openDialog = useCallback(
    async (initial?: { name: string; entry: JsonValue }) => {
      if (!codec) return;
      const result = await McpServerDialog.show({
        codec,
        existingNames: Object.keys(servers),
        initial,
      });
      if (!result) return;
      setServers((prev) => {
        const next = { ...prev };
        // Renamed: drop the old key.
        if (initial && initial.name !== result.name) delete next[initial.name];
        next[result.name] = result.entry;
        return next;
      });
    },
    [codec, servers]
  );

  const removeServer = useCallback((name: string) => {
    setServers((prev) => {
      const next = { ...prev };
      delete next[name];
      return next;
    });
  }, []);

  const addPreconfigured = useCallback(
    (key: string) => {
      if (!mcpConfig) return;
      const preconf = mcpConfig.preconfigured;
      if (!isObject(preconf) || !(key in preconf)) return;
      const entry = preconf[key];
      if (entry === undefined) return;
      setServers((prev) => ({ ...prev, [key]: entry }));
    },
    [mcpConfig]
  );

  // --- JSON escape hatch ----------------------------------------------------

  const enterJsonMode = useCallback(() => {
    if (!mcpConfig) return;
    const scratch = { ...mcpConfig, servers } as McpConfig;
    const fullConfig = McpConfigStrategyGeneral.createFullConfig(scratch);
    setJsonText(JSON.stringify(fullConfig, null, 2));
    setJsonError(null);
    setMode('json');
  }, [mcpConfig, servers]);

  const applyJsonToServers = useCallback(
    (value: string): boolean => {
      if (!mcpConfig) return false;
      try {
        const parsed = JSON.parse(value);
        McpConfigStrategyGeneral.validateFullConfig(mcpConfig, parsed);
        const extracted = McpConfigStrategyGeneral.extractServersForApi(
          mcpConfig,
          parsed
        );
        setServers(extracted as ServerMap);
        setJsonError(null);
        return true;
      } catch (err) {
        setJsonError(
          err instanceof SyntaxError
            ? t('settings.mcp.errors.invalidJson')
            : err instanceof Error
              ? err.message
              : t('settings.mcp.errors.validationError')
        );
        return false;
      }
    },
    [mcpConfig, t]
  );

  const exitJsonMode = useCallback(() => {
    if (applyJsonToServers(jsonText)) setMode('form');
  }, [applyJsonToServers, jsonText]);

  const handleJsonChange = useCallback(
    (value: string) => {
      setJsonText(value);
      applyJsonToServers(value);
    },
    [applyJsonToServers]
  );

  // --- preconfigured metadata -----------------------------------------------

  const preconfiguredObj = (mcpConfig?.preconfigured ?? {}) as Record<
    string,
    unknown
  >;
  const meta =
    typeof preconfiguredObj.meta === 'object' && preconfiguredObj.meta !== null
      ? (preconfiguredObj.meta as Record<
          string,
          { name?: string; description?: string; url?: string; icon?: string }
        >)
      : {};
  const preconfiguredServers = Object.fromEntries(
    Object.entries(preconfiguredObj).filter(([k]) => k !== 'meta')
  ) as Record<string, unknown>;
  const getMetaFor = (key: string) => meta[key] || {};

  const profileOptions = profiles
    ? Object.keys(profiles)
        .sort()
        .map((key) => ({ value: key, label: toPrettyCase(key) }))
    : [];

  const notSupported = mcpError?.includes('does not support MCP');
  const serverNames = Object.keys(servers).sort();

  if (!config) {
    return (
      <div className="py-8">
        <div className="bg-error/10 border border-error/50 rounded-sm p-4 text-error">
          {t('settings.mcp.errors.loadFailed')}
        </div>
      </div>
    );
  }

  return (
    <>
      {/* Status messages */}
      {mcpError && !notSupported && (
        <div className="bg-error/10 border border-error/50 rounded-sm p-4 text-error">
          {t('settings.mcp.errors.mcpError', { error: mcpError })}
        </div>
      )}

      {success && (
        <div className="bg-success/10 border border-success/50 rounded-sm p-4 text-success font-medium">
          {t('settings.mcp.save.successMessage')}
        </div>
      )}

      <SettingsCard
        title={t('settings.mcp.title')}
        description={t('settings.mcp.description')}
      >
        <SettingsField
          label={t('settings.mcp.labels.agent')}
          description={t('settings.mcp.labels.agentHelper')}
        >
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <DropdownMenuTriggerButton
                label={
                  selectedProfileKey
                    ? toPrettyCase(selectedProfileKey)
                    : t('settings.mcp.labels.agentPlaceholder')
                }
                className="w-full justify-between"
              />
            </DropdownMenuTrigger>
            <DropdownMenuContent className="w-[var(--radix-dropdown-menu-trigger-width)]">
              {profileOptions.map((option) => (
                <DropdownMenuItem
                  key={option.value}
                  onClick={() => {
                    const profile = profiles?.[option.value];
                    if (profile) setSelectedProfile(profile);
                  }}
                >
                  {option.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </SettingsField>

        {notSupported ? (
          <div className="rounded-sm border border-warning/50 bg-warning/10 p-4">
            <h3 className="text-sm font-medium text-warning">
              {t('settings.mcp.errors.notSupported')}
            </h3>
            <div className="mt-2 text-sm text-low">
              <p>{mcpError}</p>
              <p className="mt-1">{t('settings.mcp.errors.supportMessage')}</p>
            </div>
          </div>
        ) : (
          <>
            {/* Header: title, save location, and mode toggle */}
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <label className="text-sm font-medium text-normal">
                  {t('settings.mcp.labels.servers')}
                </label>
                <p className="text-sm text-low">
                  {mcpLoading ? (
                    t('settings.mcp.loadingStates.configuration')
                  ) : (
                    <>
                      {t('settings.mcp.labels.saveLocation')}
                      {mcpConfigPath && (
                        <span className="ml-2 font-mono text-xs">
                          {mcpConfigPath}
                        </span>
                      )}
                    </>
                  )}
                </p>
              </div>
              {!mcpLoading && (
                <div className="flex shrink-0 items-center gap-1">
                  {mode === 'form' && serverNames.length > 0 && (
                    <Button
                      variant="ghost"
                      size="sm"
                      type="button"
                      className="text-low"
                      onClick={handleTest}
                      disabled={testing || isDirty}
                      title={
                        isDirty ? t('settings.mcp.test.dirtyHint') : undefined
                      }
                    >
                      {testing ? (
                        <CircleNotchIcon
                          className="size-icon-xs mr-1 animate-spin"
                          weight="bold"
                        />
                      ) : (
                        <CheckCircleIcon
                          className="size-icon-xs mr-1"
                          weight="bold"
                        />
                      )}
                      {t('settings.mcp.test.button')}
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    type="button"
                    className="text-low"
                    onClick={mode === 'json' ? exitJsonMode : enterJsonMode}
                  >
                    <CodeIcon className="size-icon-xs mr-1" weight="bold" />
                    {mode === 'json'
                      ? t('settings.mcp.json.editAsForm')
                      : t('settings.mcp.json.editAsJson')}
                  </Button>
                </div>
              )}
            </div>

            {mcpLoading ? (
              <div className="text-sm text-low py-4">
                {t('settings.mcp.loadingStates.jsonEditor')}
              </div>
            ) : mode === 'json' ? (
              <SettingsField label="" error={jsonError}>
                <SettingsTextarea
                  value={jsonText}
                  onChange={handleJsonChange}
                  rows={16}
                  monospace
                />
              </SettingsField>
            ) : (
              <div className="space-y-2">
                {serverNames.length === 0 ? (
                  <div className="rounded-sm border border-dashed border-border p-6 text-center">
                    <p className="text-sm text-low">
                      {t('settings.mcp.list.empty')}
                    </p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {serverNames.map((name) => {
                      const entry = servers[name];
                      const summary = codec ? codec.summarize(entry) : '';
                      const badge = codec
                        ? transportBadge(
                            codec,
                            entry,
                            t('settings.mcp.list.customBadge')
                          )
                        : '';
                      return (
                        <div
                          key={name}
                          className="space-y-2 rounded-sm border border-border bg-secondary/30 p-3"
                        >
                          <div className="flex items-center gap-3">
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-2">
                                <span className="font-mono text-sm text-high truncate">
                                  {name}
                                </span>
                                {badge && (
                                  <span className="shrink-0 rounded bg-secondary px-1.5 py-0.5 text-xs font-medium text-low">
                                    {badge}
                                  </span>
                                )}
                              </div>
                              {summary && (
                                <div className="mt-1 truncate font-mono text-xs text-low">
                                  {summary}
                                </div>
                              )}
                            </div>
                            <div className="flex shrink-0 items-center gap-1">
                              <McpTestStatusIcon result={testResults?.[name]} />
                              <button
                                type="button"
                                onClick={() => openDialog({ name, entry })}
                                aria-label={`Edit ${name}`}
                                className="flex items-center justify-center rounded-sm p-2 text-low hover:bg-secondary hover:text-normal"
                              >
                                <PencilSimpleIcon
                                  className="size-icon-xs"
                                  weight="bold"
                                />
                              </button>
                              <button
                                type="button"
                                onClick={() => removeServer(name)}
                                aria-label={`Remove ${name}`}
                                className="flex items-center justify-center rounded-sm p-2 text-error hover:bg-error/10"
                              >
                                <TrashIcon
                                  className="size-icon-xs"
                                  weight="bold"
                                />
                              </button>
                            </div>
                          </div>
                          <McpTestResultDetails
                            result={testResults?.[name]}
                            connecting={connectingServer === name}
                            connectError={connectErrors[name]}
                            onConnect={() => handleConnect(name)}
                            connectDisabled={
                              connectingServer !== null || isDirty
                            }
                            loopback={!!loopbackEnabled[name]}
                            onToggleLoopback={() =>
                              setLoopbackEnabled((prev) => ({
                                ...prev,
                                [name]: !prev[name],
                              }))
                            }
                            manualActive={manualFlow?.server === name}
                            manualCode={manualCode}
                            onManualCodeChange={setManualCode}
                            onManualComplete={handleCompleteManual}
                            completing={completing}
                          />
                        </div>
                      );
                    })}
                  </div>
                )}

                {testError && (
                  <div className="rounded-sm border border-error/50 bg-error/10 p-2 text-xs text-error">
                    {testError}
                  </div>
                )}

                <Button
                  variant="outline"
                  size="sm"
                  type="button"
                  onClick={() => openDialog()}
                >
                  <PlusIcon className="size-icon-xs mr-1" weight="bold" />
                  {t('settings.mcp.list.addServer')}
                </Button>
              </div>
            )}

            {/* Preconfigured servers */}
            {mode === 'form' &&
              !mcpLoading &&
              Object.keys(preconfiguredServers).length > 0 && (
                <div className="space-y-2">
                  <label className="text-sm font-medium text-normal">
                    {t('settings.mcp.labels.popularServers')}
                  </label>
                  <p className="text-sm text-low">
                    {t('settings.mcp.labels.serverHelperForm')}
                  </p>

                  <div className="grid grid-cols-2 gap-2">
                    {Object.keys(preconfiguredServers).map((key) => {
                      const metaObj = getMetaFor(key) as {
                        name?: string;
                        description?: string;
                        icon?: string;
                      };
                      const name = metaObj.name || key;
                      const description =
                        metaObj.description || 'No description';
                      const icon = metaObj.icon ? `/${metaObj.icon}` : null;
                      const added = key in servers;

                      return (
                        <button
                          key={key}
                          type="button"
                          onClick={() => addPreconfigured(key)}
                          disabled={added}
                          className={cn(
                            'flex items-start gap-3 p-3 rounded-sm border border-border/50 bg-secondary/30 text-left transition-colors',
                            added
                              ? 'opacity-60 cursor-default'
                              : 'hover:bg-secondary hover:border-border'
                          )}
                        >
                          <div className="w-6 h-6 rounded-sm border border-border bg-secondary flex items-center justify-center overflow-hidden shrink-0">
                            {icon ? (
                              <img
                                src={icon}
                                alt=""
                                className="w-full h-full object-cover"
                              />
                            ) : (
                              <span className="text-xs font-semibold text-normal">
                                {name.slice(0, 1).toUpperCase()}
                              </span>
                            )}
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="text-sm font-medium text-normal truncate">
                              {name}
                            </div>
                            <div className="text-xs text-low line-clamp-2">
                              {description}
                            </div>
                          </div>
                          {added ? (
                            <CheckIcon
                              className="size-icon-xs text-success shrink-0"
                              weight="bold"
                            />
                          ) : (
                            <PlusIcon
                              className="size-icon-xs text-low shrink-0"
                              weight="bold"
                            />
                          )}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
          </>
        )}
      </SettingsCard>

      <SettingsSaveBar
        show={isDirty && !notSupported}
        saving={mcpApplying}
        saveDisabled={!!jsonError}
        onSave={handleApply}
        onDiscard={handleDiscard}
      />
    </>
  );
}

// Alias for backwards compatibility
export { McpSettingsSection as McpSettingsSectionContent };
