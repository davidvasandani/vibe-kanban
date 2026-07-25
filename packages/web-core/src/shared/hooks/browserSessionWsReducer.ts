import type {
  BrowserActionResult,
  BrowserControlState,
  BrowserSessionError,
  BrowserSessionLiveState,
  BrowserWsServerMessage,
} from 'shared/types';

export type BrowserWsEvent =
  | { kind: 'json'; message: BrowserWsServerMessage }
  | { kind: 'binary'; data: ArrayBuffer };

export type BrowserFrameMeta = {
  seq: number;
  width: number;
  height: number;
};

export type BrowserWsEffect =
  | {
      kind: 'command_result';
      commandId: string | null;
      ok: boolean;
      result: BrowserActionResult | null;
      control: BrowserControlState | null;
      error: BrowserSessionError | null;
    }
  | { kind: 'frame'; meta: BrowserFrameMeta; data: ArrayBuffer };

export type BrowserWsReducerState = {
  connectionId: string | null;
  liveState: BrowserSessionLiveState | null;
  // Set by a 'frame' JSON message; consumed by the NEXT binary message,
  // which carries that frame's JPEG bytes.
  pendingFrameMeta: BrowserFrameMeta | null;
  lastError: BrowserSessionError | null;
};

export const initialBrowserWsReducerState: BrowserWsReducerState = {
  connectionId: null,
  liveState: null,
  pendingFrameMeta: null,
  lastError: null,
};

export function browserSessionWsReduce(
  state: BrowserWsReducerState,
  event: BrowserWsEvent
): { state: BrowserWsReducerState; effects: BrowserWsEffect[] } {
  if (event.kind === 'binary') {
    if (!state.pendingFrameMeta) {
      // Binary message without a preceding frame meta; ignore.
      return { state, effects: [] };
    }
    return {
      state: { ...state, pendingFrameMeta: null },
      effects: [
        { kind: 'frame', meta: state.pendingFrameMeta, data: event.data },
      ],
    };
  }

  const message = event.message;
  switch (message.type) {
    case 'ready':
      return {
        state: {
          ...state,
          connectionId: message.connection_id,
          liveState: message.state,
          lastError: null,
        },
        effects: [],
      };
    case 'state':
      return {
        state: { ...state, liveState: message.state },
        effects: [],
      };
    case 'frame':
      return {
        state: {
          ...state,
          pendingFrameMeta: {
            seq: message.seq,
            width: message.width,
            height: message.height,
          },
        },
        effects: [],
      };
    case 'command_result': {
      const liveState =
        message.control && state.liveState
          ? { ...state.liveState, control: message.control }
          : state.liveState;
      return {
        state: {
          ...state,
          liveState,
          lastError: message.ok ? state.lastError : message.error,
        },
        effects: [
          {
            kind: 'command_result',
            commandId: message.command_id,
            ok: message.ok,
            result: message.result,
            control: message.control,
            error: message.error,
          },
        ],
      };
    }
    default:
      // Unknown/future server message types are ignored.
      return { state, effects: [] };
  }
}
