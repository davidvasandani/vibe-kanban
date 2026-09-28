import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowSquareOutIcon,
  CheckCircleIcon,
  SpinnerIcon,
  WarningCircleIcon,
} from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import type { CliToolId, CliToolStatus } from 'shared/types';
import { SettingsCard } from './SettingsComponents';
import { useSettingsMachineClient } from './SettingsHostContext';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { getTerminalTheme } from '@/shared/lib/terminalTheme';
import { getCliToolLoginAction } from '@/shared/lib/cliToolLogin';
import '@xterm/xterm/css/xterm.css';

type ToolAction = 'install' | 'update' | 'remove';

export function CliToolsSettingsSection() {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const [tools, setTools] = useState<CliToolStatus[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState<CliToolId | null>(null);
  const [loginTool, setLoginTool] = useState<CliToolId | null>(null);
  const [toolErrors, setToolErrors] = useState<
    Partial<Record<CliToolId, string>>
  >({});

  const refresh = useCallback(async () => {
    if (!machineClient) return;
    try {
      setTools(await machineClient.listCliTools());
      setLoadError(null);
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : String(err));
    }
  }, [machineClient]);

  useEffect(() => {
    setTools(null);
    setLoadError(null);
    setToolErrors({});
    void refresh();
  }, [refresh]);

  const runAction = async (id: CliToolId, action: ToolAction) => {
    if (!machineClient) return;
    setBusy(id);
    setToolErrors((prev) => ({ ...prev, [id]: undefined }));
    try {
      const updated =
        action === 'install'
          ? await machineClient.installCliTool(id)
          : action === 'update'
            ? await machineClient.updateCliTool(id)
            : await machineClient.removeCliTool(id);
      setTools((prev) =>
        prev ? prev.map((s) => (s.id === updated.id ? updated : s)) : prev
      );
    } catch (err) {
      setToolErrors((prev) => ({
        ...prev,
        [id]: err instanceof Error ? err.message : String(err),
      }));
    } finally {
      setBusy(null);
    }
  };

  return (
    <SettingsCard
      title={t('settings.cliTools.title', { ns: 'settings' })}
      description={t('settings.cliTools.description', { ns: 'settings' })}
    >
      {tools === null && !loadError && (
        <div className="flex items-center gap-2 text-sm text-low">
          <SpinnerIcon className="size-icon-sm animate-spin" />
          {t('settings.cliTools.loading', { ns: 'settings' })}
        </div>
      )}
      {loadError && <p className="text-sm text-error">{loadError}</p>}
      {tools?.map((tool) => (
        <CliToolRow
          key={tool.id}
          tool={tool}
          busy={busy}
          error={toolErrors[tool.id]}
          onAction={runAction}
          loginOpen={loginTool === tool.id}
          onLogin={() =>
            setLoginTool((current) => (current === tool.id ? null : tool.id))
          }
          onStatus={(updated) =>
            setTools(
              (current) =>
                current?.map((item) =>
                  item.id === updated.id ? updated : item
                ) ?? current
            )
          }
        />
      ))}
    </SettingsCard>
  );
}

function CliToolRow({
  tool,
  busy,
  error,
  onAction,
  loginOpen,
  onLogin,
  onStatus,
}: {
  tool: CliToolStatus;
  busy: CliToolId | null;
  error?: string;
  onAction: (id: CliToolId, action: ToolAction) => void;
  loginOpen: boolean;
  onLogin: () => void;
  onStatus: (status: CliToolStatus) => void;
}) {
  const { t } = useTranslation(['settings']);
  const isBusy = busy === tool.id;
  const anyBusy = busy !== null;
  const installed = tool.app !== null;
  const available = installed || tool.host !== null;
  const loginAction = getCliToolLoginAction(tool);

  return (
    <div className="rounded-sm border border-border p-3 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            {available ? (
              <CheckCircleIcon
                className="size-icon-sm text-success shrink-0"
                weight="fill"
              />
            ) : (
              <WarningCircleIcon
                className="size-icon-sm text-low shrink-0"
                weight="fill"
              />
            )}
            <span className="text-sm font-medium text-high">
              {tool.display_name}
            </span>
            <code className="text-xs text-low">{tool.binary_name}</code>
            <a
              href={tool.docs_url}
              target="_blank"
              rel="noreferrer"
              className="text-low hover:text-normal"
              aria-label={t('settings.cliTools.docs', { ns: 'settings' })}
            >
              <ArrowSquareOutIcon className="size-icon-sm" />
            </a>
          </div>
          <p className="text-sm text-low mt-1">{tool.description}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {isBusy && <SpinnerIcon className="size-icon-sm animate-spin" />}
          {loginAction && (
            <Button
              size="sm"
              variant="secondary"
              disabled={anyBusy}
              onClick={onLogin}
            >
              {loginAction === 'reauthenticate'
                ? t('settings.cliTools.actions.reauthenticate', {
                    ns: 'settings',
                  })
                : t('settings.cliTools.actions.login', { ns: 'settings' })}
            </Button>
          )}
          {!installed && tool.supported && (
            <Button
              size="sm"
              variant="secondary"
              disabled={anyBusy}
              onClick={() => onAction(tool.id, 'install')}
            >
              {t('settings.cliTools.actions.install', { ns: 'settings' })}
            </Button>
          )}
          {installed && tool.app?.outdated && (
            <Button
              size="sm"
              variant="secondary"
              disabled={anyBusy}
              onClick={() => onAction(tool.id, 'update')}
            >
              {t('settings.cliTools.actions.update', {
                ns: 'settings',
                version: tool.catalog_version,
              })}
            </Button>
          )}
          {installed && (
            <Button
              size="sm"
              variant="ghost"
              disabled={anyBusy}
              onClick={() => onAction(tool.id, 'remove')}
            >
              {t('settings.cliTools.actions.remove', { ns: 'settings' })}
            </Button>
          )}
        </div>
      </div>

      <div className="space-y-1 text-sm">
        <p className="text-normal">
          {t(`settings.cliTools.auth.${tool.auth_state}`, { ns: 'settings' })}
          {tool.auth_message ? (
            <span className="text-low"> · {tool.auth_message}</span>
          ) : null}
        </p>
        {tool.host && (
          <p className="text-normal">
            {t('settings.cliTools.status.host', {
              ns: 'settings',
              path: tool.host.path,
            })}
            {tool.host.version ? (
              <span className="text-low"> · {tool.host.version}</span>
            ) : null}
          </p>
        )}
        {tool.app && (
          <p className="text-normal">
            {t('settings.cliTools.status.app', {
              ns: 'settings',
              version: tool.app.version,
            })}
            {tool.app.outdated && (
              <span className="text-warning">
                {' '}
                ·{' '}
                {t('settings.cliTools.status.outdated', {
                  ns: 'settings',
                  version: tool.catalog_version,
                })}
              </span>
            )}
            {tool.host && (
              <span className="text-low">
                {' '}
                · {t('settings.cliTools.status.shadowed', { ns: 'settings' })}
              </span>
            )}
          </p>
        )}
        {!tool.host && !tool.app && tool.supported && (
          <p className="text-low">
            {t('settings.cliTools.status.notAvailable', {
              ns: 'settings',
              version: tool.catalog_version,
            })}
          </p>
        )}
        {!tool.supported && (
          <p className="text-low">
            {t('settings.cliTools.status.unsupported', { ns: 'settings' })}
            {tool.unsupported_reason ? `: ${tool.unsupported_reason}` : ''}
          </p>
        )}
        {error && <p className="text-error">{error}</p>}
      </div>
      {loginOpen && <CliToolLoginTerminal tool={tool} onStatus={onStatus} />}
    </div>
  );
}

function CliToolLoginTerminal({
  tool,
  onStatus,
}: {
  tool: CliToolStatus;
  onStatus: (status: CliToolStatus) => void;
}) {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const containerRef = useRef<HTMLDivElement>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const onStatusRef = useRef(onStatus);
  const [result, setResult] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    onStatusRef.current = onStatus;
  }, [onStatus]);

  useEffect(() => {
    if (!machineClient || !containerRef.current) return;
    setResult(null);
    const terminal = new Terminal({
      cursorBlink: true,
      fontSize: 12,
      fontFamily: '"IBM Plex Mono", monospace',
      theme: getTerminalTheme(),
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.loadAddon(new WebLinksAddon());
    terminal.open(containerRef.current);
    fit.fit();
    let disposed = false;
    let resizeObserver: ResizeObserver | null = new ResizeObserver(() => {
      fit.fit();
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        socketRef.current.send(
          JSON.stringify({
            type: 'resize',
            cols: terminal.cols,
            rows: terminal.rows,
          })
        );
      }
    });
    resizeObserver.observe(containerRef.current);

    void machineClient
      .openCliToolLogin(tool.id)
      .then((socket) => {
        if (disposed) {
          socket.close();
          return;
        }
        socketRef.current = socket;
        let receivedResult = false;
        socket.onmessage = (event) => {
          const message = JSON.parse(event.data);
          if (message.type === 'output') {
            const bytes = Uint8Array.from(atob(message.data), (char) =>
              char.charCodeAt(0)
            );
            terminal.write(bytes);
          } else if (message.type === 'exit') {
            receivedResult = true;
            setResult(message.outcome);
          } else if (message.type === 'status') {
            onStatusRef.current(message.tool as CliToolStatus);
          } else if (message.type === 'error') {
            receivedResult = true;
            setResult(message.message);
          }
        };
        socket.onerror = () => {
          if (!receivedResult && !disposed) {
            receivedResult = true;
            setResult(
              t('settings.cliTools.login.connectionFailed', { ns: 'settings' })
            );
          }
        };
        socket.onclose = () => {
          if (!receivedResult && !disposed) {
            receivedResult = true;
            setResult(
              t('settings.cliTools.login.connectionClosed', { ns: 'settings' })
            );
          }
        };
        terminal.onData((data) => {
          if (socket.readyState === WebSocket.OPEN) {
            const bytes = new TextEncoder().encode(data);
            const binary = Array.from(bytes, (byte) =>
              String.fromCharCode(byte)
            ).join('');
            socket.send(JSON.stringify({ type: 'input', data: btoa(binary) }));
          }
        });
      })
      .catch((error) =>
        setResult(error instanceof Error ? error.message : String(error))
      );

    return () => {
      disposed = true;
      resizeObserver?.disconnect();
      resizeObserver = null;
      socketRef.current?.close();
      socketRef.current = null;
      terminal.dispose();
    };
  }, [attempt, machineClient, t, tool.id]);

  return (
    <div className="space-y-2 border-t border-border pt-3">
      <div ref={containerRef} className="h-64 w-full rounded-sm bg-black p-1" />
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-low">
          {result ?? t('settings.cliTools.login.running', { ns: 'settings' })}
        </span>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => {
            if (result !== null) {
              setAttempt((current) => current + 1);
            } else {
              socketRef.current?.send(JSON.stringify({ type: 'cancel' }));
            }
          }}
        >
          {result !== null
            ? t('settings.cliTools.login.retry', { ns: 'settings' })
            : t('settings.cliTools.login.cancel', { ns: 'settings' })}
        </Button>
      </div>
    </div>
  );
}
