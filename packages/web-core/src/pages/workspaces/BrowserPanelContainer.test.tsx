// @vitest-environment jsdom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { BrowserPanelContainer } from './BrowserPanelContainer';

const state = vi.hoisted(() => ({
  sessionId: 'session-1',
  connected: true,
  generation: 1,
  owner: 'connection-1',
  url: 'https://sectigostore.com/checkout?token=secret#secret',
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
      current_url: state.url,
      control: {
        controller: { type: 'human', connection_id: state.owner },
        generation: state.generation,
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
  state.generation = 1;
  state.sendInput.mockReset().mockResolvedValue(null);
  vi.useFakeTimers();
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 16)
  );
  vi.stubGlobal('cancelAnimationFrame', clearTimeout);
  HTMLElement.prototype.setPointerCapture = vi.fn();
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});
afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.useRealTimers();
  vi.unstubAllGlobals();
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
  pointer(image, 'pointerdown', 20, 30);
  const event = new MouseEvent('pointerup', {
    bubbles: true,
    cancelable: true,
    clientX: 20,
    clientY: 30,
  });
  Object.defineProperty(event, 'pointerType', { value: 'touch' });
  Object.defineProperty(event, 'pointerId', { value: 1 });
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

it('offers an isolated local browser fallback without mutating the hosted page', () => {
  render();
  const link = container.querySelector('a')!;
  expect(link.href).toBe('https://sectigostore.com/');
  expect(link.target).toBe('_blank');
  expect(link.rel).toBe('noopener noreferrer');
  expect(container.textContent).toContain('browser.handoff.session');
  expect(container.textContent).toContain('browser.handoff.steps');
  act(() => container.querySelector('summary')!.click());
  expect(state.sendInput).not.toHaveBeenCalled();
});
it('does not offer a destination from a stale browser session', () => {
  state.sessionId = 'session-2';
  render();
  expect(container.querySelector('a')).toBeNull();
});

function pointer(target: Element, type: string, x: number, y: number, id = 1) {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX: x,
    clientY: y,
  });
  Object.defineProperties(event, {
    pointerType: { value: 'touch' },
    pointerId: { value: id },
  });
  act(() => target.dispatchEvent(event));
  return event;
}
function frame(width = 50, height = 100) {
  render();
  const image = container.querySelector('img')!;
  vi.spyOn(image, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    width,
    height,
  } as DOMRect);
  return image;
}
function wheel(
  image: Element,
  deltaX: number,
  deltaY: number,
  deltaMode = 0,
  x = 25,
  y = 50
) {
  const event = new WheelEvent('wheel', {
    bubbles: true,
    cancelable: true,
    clientX: x,
    clientY: y,
    deltaX,
    deltaY,
    deltaMode,
  });
  act(() => image.dispatchEvent(event));
  return event;
}
async function flush() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(20);
  });
}
it.each([
  [0, 4, 8],
  [1, 64, 128],
  [2, 200, 400],
])('scales wheel axes and normalizes delta mode %s', async (mode, dx, dy) => {
  const image = frame();
  expect(wheel(image, 2, 4, mode).defaultPrevented).toBe(true);
  await flush();
  expect(state.sendInput).toHaveBeenCalledWith({
    type: 'scroll',
    x: 50,
    y: 50,
    delta_x: dx,
    delta_y: dy,
  });
});
it('ignores letterbox input and read-only wheel gestures', async () => {
  const image = frame();
  expect(wheel(image, 0, 10, 0, 25, 5).defaultPrevented).toBe(false);
  pointer(image, 'pointerdown', 25, 5);
  pointer(image, 'pointermove', 25, 55);
  pointer(image, 'pointerup', 25, 55);
  state.owner = 'someone-else';
  render();
  expect(wheel(image, 0, 10).defaultPrevented).toBe(false);
  await flush();
  expect(state.sendInput).not.toHaveBeenCalled();
  expect(image.parentElement!.style.touchAction).toBe('auto');
});
it('swipes move remote content with the finger without clicking or opening the keyboard', async () => {
  const image = frame();
  pointer(image, 'pointerdown', 25, 50);
  pointer(image, 'pointermove', 15, 30);
  pointer(image, 'pointerup', 10, 20);
  await flush();
  expect(state.sendInput).toHaveBeenCalledWith({
    type: 'scroll',
    x: 50,
    y: 50,
    delta_x: 30,
    delta_y: 60,
  });
  expect(document.activeElement).not.toBe(container.querySelector('textarea'));
  expect(image.parentElement!.style.touchAction).toBe('none');
});
it('retains taps with small touch jitter', () => {
  const image = frame(100, 100);
  pointer(image, 'pointerdown', 20, 30);
  pointer(image, 'pointermove', 22, 32);
  pointer(image, 'pointerup', 22, 32);
  expect(state.sendInput).toHaveBeenCalledWith({
    type: 'click',
    x: 22,
    y: 32,
    button: 'left',
  });
  expect(document.activeElement).toBe(container.querySelector('textarea'));
});
it.each(['pointercancel', 'lostpointercapture'])(
  'cancels a gesture on %s',
  async (event) => {
    const image = frame();
    pointer(image, 'pointerdown', 25, 50);
    pointer(image, event, 25, 50);
    pointer(image, 'pointermove', 25, 30);
    pointer(image, 'pointerup', 25, 30);
    await flush();
    expect(state.sendInput).not.toHaveBeenCalled();
  }
);
it('multi-touch cancels until all fingers lift', async () => {
  const image = frame();
  pointer(image, 'pointerdown', 25, 50);
  pointer(image, 'pointerdown', 30, 50, 2);
  pointer(image, 'pointerup', 30, 50, 2);
  pointer(image, 'pointermove', 25, 30);
  pointer(image, 'pointerup', 25, 30);
  await flush();
  expect(state.sendInput).not.toHaveBeenCalled();
});
it.each(['generation', 'disconnect', 'session', 'owner'])(
  'drops pending scroll and touch on %s change',
  async (change) => {
    const image = frame();
    pointer(image, 'pointerdown', 25, 50);
    pointer(image, 'pointermove', 25, 30);
    if (change === 'generation') state.generation++;
    if (change === 'disconnect') state.connected = false;
    if (change === 'session') state.sessionId = 'session-2';
    if (change === 'owner') state.owner = 'someone-else';
    render();
    pointer(image, 'pointerup', 25, 30);
    await flush();
    expect(state.sendInput).not.toHaveBeenCalled();
  }
);
it('bounds pending work to one accumulated scroll while awaiting acknowledgement', async () => {
  let resolve!: (value: null) => void;
  state.sendInput.mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r;
      })
  );
  const image = frame();
  wheel(image, 0, 2);
  await flush();
  for (let i = 0; i < 20; i++) wheel(image, 0, 2);
  await flush();
  expect(state.sendInput).toHaveBeenCalledTimes(1);
  await act(async () => resolve(null));
  await flush();
  expect(state.sendInput).toHaveBeenCalledTimes(2);
  expect(state.sendInput).toHaveBeenLastCalledWith({
    type: 'scroll',
    x: 50,
    y: 50,
    delta_x: 0,
    delta_y: 80,
  });
});
it('does not replay accumulated work after control changes during an acknowledgement', async () => {
  let resolve!: (value: null) => void;
  state.sendInput.mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r;
      })
  );
  const image = frame();
  wheel(image, 0, 2);
  await flush();
  wheel(image, 0, 2);
  state.generation++;
  render();
  await act(async () => resolve(null));
  await flush();
  expect(state.sendInput).toHaveBeenCalledTimes(1);
});

it('keeps a swipe alive as new screencast frames render with the same dimensions', async () => {
  const image = frame();
  pointer(image, 'pointerdown', 25, 50);
  pointer(image, 'pointermove', 25, 40);
  await flush();
  render();
  pointer(image, 'pointermove', 25, 30);
  pointer(image, 'pointerup', 25, 30);
  await flush();
  expect(state.sendInput).toHaveBeenCalledTimes(2);
  expect(
    state.sendInput.mock.calls.every(([action]) => action.type === 'scroll')
  ).toBe(true);
});

it.each([1, -1])(
  'preserves fractional trackpad movement and remainders in direction %s',
  async (direction) => {
    const image = frame(100, 100);
    for (let i = 0; i < 5; i++) {
      wheel(image, 0.4 * direction, 0.4 * direction);
      await flush();
    }
    const total = state.sendInput.mock.calls.reduce(
      (sum, [action]) => sum + action.delta_y,
      0
    );
    expect(total).toBe(2 * direction);
    expect(
      state.sendInput.mock.calls.every(
        ([action]) => action.delta_x === action.delta_y
      )
    ).toBe(true);
  }
);
it('does not apply queued wheel movement to a new hit target', async () => {
  let resolve!: (value: null) => void;
  state.sendInput.mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r;
      })
  );
  const image = frame();
  wheel(image, 0, 2);
  await flush();
  wheel(image, 0, 10);
  wheel(image, 0, 20, 0, 35, 50);
  await act(async () => resolve(null));
  await flush();
  expect(state.sendInput).toHaveBeenCalledTimes(2);
  expect(state.sendInput).toHaveBeenLastCalledWith({
    type: 'scroll',
    x: 70,
    y: 50,
    delta_x: 0,
    delta_y: 40,
  });
});
