// @vitest-environment jsdom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { BrowserPanelContainer } from './BrowserPanelContainer';

const state = vi.hoisted(() => ({
  sessionId: 'session-1',
  connected: true,
  owner: 'connection-1',
  sendInput: vi.fn().mockResolvedValue(null),
  refresh: vi.fn(),
  setSession: vi.fn(),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/shared/lib/api', () => ({ browserSessionsApi: {} }));
vi.mock('@/shared/hooks/useWorkspaceContext', () => ({
  useWorkspaceContext: () => ({ selectedSession: null }),
}));
vi.mock('@/shared/hooks/useExecutionProcesses', () => ({
  useExecutionProcesses: () => ({ executionProcesses: [] }),
}));
vi.mock('@/shared/stores/useBrowserSessionStore', () => ({
  useSelectedBrowserSessionId: () => [state.sessionId, state.setSession],
  useBrowserSessionStore: () => state.refresh,
}));
vi.mock('@/shared/hooks/useBrowserSessionWs', () => ({
  useBrowserSessionWs: () => ({
    connected: state.connected,
    connectionId: 'connection-1',
    liveState: {
      session_id: 'session-1',
      status: 'running',
      control: {
        controller: { type: 'human', connection_id: state.owner },
        generation: 1,
      },
    },
    frameUrl: 'data:image/jpeg;base64,AA==',
    frameSize: { width: 100, height: 100 },
    sendInput: state.sendInput,
  }),
}));
let container: HTMLDivElement;
let root: Root;
beforeEach(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  state.sessionId = 'session-1';
  state.owner = 'connection-1';
  state.connected = true;
  state.sendInput.mockClear();
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});
afterEach(() => {
  act(() => root.unmount());
  container.remove();
  globalThis.IS_REACT_ACT_ENVIRONMENT = false;
});
function render() {
  act(() =>
    root.render(<BrowserPanelContainer workspaceId="workspace" className="" />)
  );
}
it('focuses the keyboard during a touch tap and forwards original frame coordinates', () => {
  render();
  const image = container.querySelector('img')!;
  vi.spyOn(image, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    width: 100,
    height: 100,
  } as DOMRect);
  const event = new MouseEvent('pointerup', {
    bubbles: true,
    cancelable: true,
    clientX: 20,
    clientY: 30,
  });
  Object.defineProperty(event, 'pointerType', { value: 'touch' });
  act(() => image.dispatchEvent(event));
  expect(document.activeElement).toBe(container.querySelector('textarea'));
  expect(state.sendInput).toHaveBeenCalledWith({
    type: 'click',
    x: 20,
    y: 30,
    button: 'left',
  });
});
it('removes the text bridge on disconnect, control loss and session replacement', () => {
  render();
  expect(container.querySelector('textarea')).not.toBeNull();
  state.connected = false;
  render();
  expect(container.querySelector('textarea')).toBeNull();
  state.connected = true;
  state.owner = 'another';
  render();
  expect(container.querySelector('textarea')).toBeNull();
  state.owner = 'connection-1';
  state.sessionId = 'session-2';
  render();
  expect(container.querySelector('textarea')).toBeNull();
});
it('preserves local paste shortcut default then sends clipboard text once', () => {
  render();
  const surface = container.querySelector('img')!.parentElement!;
  const key = new KeyboardEvent('keydown', {
    bubbles: true,
    cancelable: true,
    key: 'v',
    ctrlKey: true,
  });
  act(() => surface.dispatchEvent(key));
  expect(key.defaultPrevented).toBe(false);
  expect(state.sendInput).not.toHaveBeenCalled();
  const paste = new Event('paste', { bubbles: true, cancelable: true });
  Object.defineProperty(paste, 'clipboardData', {
    value: { getData: () => 'local text' },
  });
  act(() => surface.dispatchEvent(paste));
  expect(state.sendInput).toHaveBeenCalledTimes(1);
  expect(state.sendInput).toHaveBeenCalledWith({
    type: 'type',
    text: 'local text',
  });
});
