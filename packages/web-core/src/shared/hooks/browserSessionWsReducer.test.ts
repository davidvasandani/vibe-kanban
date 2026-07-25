import { describe, expect, it } from 'vitest';
import { BrowserSessionStatus } from 'shared/types';
import type {
  BrowserSessionLiveState,
  BrowserWsServerMessage,
} from 'shared/types';
import {
  browserSessionWsReduce,
  initialBrowserWsReducerState,
  type BrowserWsReducerState,
} from './browserSessionWsReducer';

const liveState = (
  overrides: Partial<BrowserSessionLiveState> = {}
): BrowserSessionLiveState => ({
  session_id: 'session-1',
  workspace_id: 'workspace-1',
  status: BrowserSessionStatus.running,
  current_url: 'https://example.com',
  page_title: 'Example',
  control: {
    controller: { type: 'none' },
    generation: 1,
    lease_expires_at: null,
  },
  expires_at: null,
  ...overrides,
});

const reduceJson = (
  state: BrowserWsReducerState,
  message: BrowserWsServerMessage
) => browserSessionWsReduce(state, { kind: 'json', message });

describe('browserSessionWsReduce', () => {
  it('handles ready by storing connection id and live state', () => {
    const { state, effects } = reduceJson(initialBrowserWsReducerState, {
      type: 'ready',
      connection_id: 'conn-1',
      state: liveState(),
    });

    expect(state.connectionId).toBe('conn-1');
    expect(state.liveState?.session_id).toBe('session-1');
    expect(effects).toEqual([]);
  });

  it('handles state by replacing live state', () => {
    const ready = reduceJson(initialBrowserWsReducerState, {
      type: 'ready',
      connection_id: 'conn-1',
      state: liveState(),
    }).state;

    const { state, effects } = reduceJson(ready, {
      type: 'state',
      state: liveState({ page_title: 'Updated' }),
    });

    expect(state.liveState?.page_title).toBe('Updated');
    expect(state.connectionId).toBe('conn-1');
    expect(effects).toEqual([]);
  });

  it('pairs a frame meta message with the next binary message', () => {
    const meta = reduceJson(initialBrowserWsReducerState, {
      type: 'frame',
      seq: 7,
      width: 1280,
      height: 720,
    });
    expect(meta.state.pendingFrameMeta).toEqual({
      seq: 7,
      width: 1280,
      height: 720,
    });
    expect(meta.effects).toEqual([]);

    const data = new ArrayBuffer(3);
    const frame = browserSessionWsReduce(meta.state, {
      kind: 'binary',
      data,
    });
    expect(frame.state.pendingFrameMeta).toBeNull();
    expect(frame.effects).toEqual([
      { kind: 'frame', meta: { seq: 7, width: 1280, height: 720 }, data },
    ]);
  });

  it('ignores binary messages without a preceding frame meta', () => {
    const { state, effects } = browserSessionWsReduce(
      initialBrowserWsReducerState,
      { kind: 'binary', data: new ArrayBuffer(1) }
    );
    expect(state).toBe(initialBrowserWsReducerState);
    expect(effects).toEqual([]);
  });

  it('emits command_result effects and records errors', () => {
    const ready = reduceJson(initialBrowserWsReducerState, {
      type: 'ready',
      connection_id: 'conn-1',
      state: liveState(),
    }).state;

    const control = {
      controller: { type: 'agent', execution_id: 'exec-1' } as const,
      generation: 2,
      lease_expires_at: null,
    };
    const { state, effects } = reduceJson(ready, {
      type: 'command_result',
      command_id: 'cmd-1',
      ok: false,
      result: null,
      control,
      error: {
        code: 'CONTROL_CONFLICT',
        controller: control.controller,
        generation: 2,
      },
    });

    expect(state.lastError?.code).toBe('CONTROL_CONFLICT');
    expect(state.liveState?.control).toEqual(control);
    expect(effects).toHaveLength(1);
    expect(effects[0]).toMatchObject({
      kind: 'command_result',
      commandId: 'cmd-1',
      ok: false,
    });
  });
});
