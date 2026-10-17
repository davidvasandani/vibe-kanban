/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExecutionProcess, ExecutorConfig, Session } from 'shared/types';
import { useSessionSend } from './useSessionSend';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  followUp: vi.fn(),
  createSession: vi.fn(),
}));

vi.mock('@/shared/lib/api', () => ({
  sessionsApi: { followUp: mocks.followUp },
}));

vi.mock('./useCreateSession', () => ({
  useCreateSession: () => ({
    mutateAsync: mocks.createSession,
    isPending: false,
  }),
}));

const executorConfig = {} as ExecutorConfig;
const acceptedProcess = {
  id: 'process',
  session_id: 'session',
} as ExecutionProcess;

type SendHook = ReturnType<typeof useSessionSend>;
let sendHook: SendHook;

function Harness(props: Parameters<typeof useSessionSend>[0]) {
  sendHook = useSessionSend(props);
  return null;
}

describe('useSessionSend', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.followUp.mockReset();
    mocks.createSession.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('reconciles the process returned by a successful follow-up', async () => {
    mocks.followUp.mockResolvedValue(acceptedProcess);
    const onFollowUpAccepted = vi.fn();
    act(() =>
      root.render(
        <Harness
          sessionId="session"
          workspaceId="workspace"
          isNewSessionMode={false}
          executorConfig={executorConfig}
          onFollowUpAccepted={onFollowUpAccepted}
        />
      )
    );

    let success = false;
    await act(async () => {
      success = await sendHook.send(' hello ');
    });

    expect(success).toBe(true);
    expect(mocks.followUp).toHaveBeenCalledWith(
      'session',
      expect.objectContaining({ prompt: 'hello' })
    );
    expect(onFollowUpAccepted).toHaveBeenCalledOnce();
    expect(onFollowUpAccepted).toHaveBeenCalledWith(acceptedProcess);
  });

  it('does not reconcile a failed follow-up', async () => {
    mocks.followUp.mockRejectedValue(new Error('offline'));
    const onFollowUpAccepted = vi.fn();
    act(() =>
      root.render(
        <Harness
          sessionId="session"
          workspaceId="workspace"
          isNewSessionMode={false}
          executorConfig={executorConfig}
          onFollowUpAccepted={onFollowUpAccepted}
        />
      )
    );

    let success = true;
    await act(async () => {
      success = await sendHook.send('hello');
    });

    expect(success).toBe(false);
    expect(onFollowUpAccepted).not.toHaveBeenCalled();
    expect(sendHook.error).toBe('Failed to send: offline');
  });

  it('does not reconcile new-session creation', async () => {
    mocks.createSession.mockResolvedValue({ id: 'new-session' } as Session);
    const onFollowUpAccepted = vi.fn();
    const onSelectSession = vi.fn();
    act(() =>
      root.render(
        <Harness
          sessionId={undefined}
          workspaceId="workspace"
          isNewSessionMode
          executorConfig={executorConfig}
          onFollowUpAccepted={onFollowUpAccepted}
          onSelectSession={onSelectSession}
        />
      )
    );

    let success = false;
    await act(async () => {
      success = await sendHook.send('hello');
    });

    expect(success).toBe(true);
    expect(onFollowUpAccepted).not.toHaveBeenCalled();
    expect(onSelectSession).toHaveBeenCalledWith('new-session');
  });
});
