/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, expect, it, vi } from 'vitest';
import { useWorkspaceSessions } from './useWorkspaceSessions';

const state = vi.hoisted(() => ({
  search: '?searchSessionId=old',
  sessions: [{ id: 'new' }, { id: 'old' }],
}));
vi.mock('@tanstack/react-router', () => ({
  useLocation: () => ({
    searchStr: state.search,
    pathname: '/workspaces/workspace',
    hash: '',
  }),
  useNavigate: () => (options: { href: string }) => {
    state.search = new URL(options.href, 'https://test').search;
  },
}));
vi.mock('@tanstack/react-query', () => ({
  useQuery: () => ({ data: state.sessions, isLoading: false }),
}));
vi.mock('@/shared/lib/api', () => ({ sessionsApi: {} }));
vi.mock('@/shared/providers/HostIdProvider', () => ({ useHostId: () => null }));
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
let root: ReturnType<typeof createRoot>;
afterEach(() => {
  act(() => root.unmount());
});
it('selects the requested historical session and preserves it through refetch', () => {
  const container = document.createElement('div');
  root = createRoot(container);
  function Probe() {
    const sessions = useWorkspaceSessions('workspace');
    return (
      <button onClick={() => sessions.selectSession('new')}>
        {sessions.selectedSessionId}
      </button>
    );
  }
  act(() => root.render(<Probe />));
  expect(container.textContent).toBe('old');
  state.sessions = [{ id: 'new' }, { id: 'old' }];
  act(() => root.render(<Probe />));
  expect(container.textContent).toBe('old');
  act(() => container.querySelector('button')!.click());
  expect(container.textContent).toBe('new');
  expect(state.search).toBe('');
  act(() => root.render(<Probe />));
  state.search = '?searchSessionId=old';
  act(() => root.render(<Probe />));
  expect(container.textContent).toBe('old');
});
